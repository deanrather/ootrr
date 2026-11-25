use crate::byml::{Byml, Node};
use crate::difficulty::ambush::{actor, floats, fresh, list, text, vec3};
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::placement::hash_of;
use crate::qol::Have;
use crate::romfs::Romfs;
use crate::staging::{Out, BANC_DICT};
use crate::zs;

pub const MAP: &str = "Banc/HyruleField/HyruleField_Child_Dynamic.bcett.byml.zs";
pub const BRIDGE_MAP: &str = "Banc/HyruleField/HyruleField_Child_Static.bcett.byml.zs";
pub const BRIDGE: &str = "Land_HrlField_DrawBridge_A_01";
pub const SEED: &str = "ootrr climbable drawbridge chains";
pub const SIDES: [f32; 2] = [-1.0, 1.0];
pub const CHAIN_X: f32 = 3.73;
pub const OVERLAP: f32 = 0.4;
pub const NET: &str = "Obj_SpiritTemple_MoveClimbWall_B_01";
pub const NET_SIZE: [f32; 3] = [5.48, 3.0, 0.3];
pub const NET_WIDTH: f32 = 1.4;

pub const PROFILE: [[f32; 2]; 13] = [[15.0, 0.0], [13.0, 0.15], [11.2, 0.45], [9.5, 1.12], [6.5, 2.9], [5.5, 3.6], [4.5, 4.27], [2.5, 6.0], [1.5, 7.0], [0.5, 8.1], [-0.5, 9.3], [-1.5, 10.5], [-2.4, 11.4]];

pub const NOTE: &str = "In the child Hyrule Field, a strip of the Spirit Temple's climbable wire net runs along each of the two chains of the lowered Hyrule Castle drawbridge, from the end of the bridge up to the gatehouse, so Link can climb them. Only while the bridge is down (by day). The adult castle has no chains, so nothing changes there.";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub at: [f32; 3],
    pub rot: [f32; 3],
    pub scale: [f32; 3],
}

pub fn nets(origin: [f32; 3], side: f32) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for w in PROFILE.windows(2) {
        let dz: f32 = w[1][0] - w[0][0];
        let dy: f32 = w[1][1] - w[0][1];
        let len: f32 = (dz * dz + dy * dy).sqrt();
        let dir: [f32; 2] = [dz / len, dy / len];
        let pitch: f32 = dy.atan2(-dz);
        let normal: [f32; 2] = [pitch.sin(), pitch.cos()];
        let start: [f32; 2] = [w[0][0] - dir[0] * OVERLAP, w[0][1] - dir[1] * OVERLAP];
        let half: f32 = NET_SIZE[2] / 2.0;
        out.push(Piece {
            at: [origin[0] + side * CHAIN_X, origin[1] + start[1] - normal[1] * half, origin[2] + start[0] - normal[0] * half],
            rot: [pitch - std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            scale: [NET_WIDTH / NET_SIZE[0], (len + OVERLAP * 2.0) / NET_SIZE[1], 1.0],
        });
    }
    out
}

pub fn twin(p: &Piece) -> Piece {
    let (sn, c): (f32, f32) = p.rot[0].sin_cos();
    let long: f32 = p.scale[1] * NET_SIZE[1];
    Piece { at: [p.at[0], p.at[1] + c * long, p.at[2] + sn * long], rot: [p.rot[0] + std::f32::consts::PI, 0.0, 0.0], scale: p.scale }
}

pub fn bridge(y: &Byml) -> Res<(Node, [f32; 3])> {
    let acts: &[Node] = y.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]);
    let b: &Node = acts.iter().find(|a| text(a, "Gyaml") == BRIDGE).ok_or_else(|| E::Msg(format!("drawbridge chains: the map has no {BRIDGE}, the game files differ")))?;
    let at: [f32; 3] = vec3(b, "Translate").ok_or_else(|| E::Msg(format!("drawbridge chains: {BRIDGE} has no position")))?;
    let presence: Node = b.get("Presence").cloned().ok_or_else(|| E::Msg(format!("drawbridge chains: {BRIDGE} has no Presence")))?;
    Ok((presence, at))
}

pub fn ramp(y: &mut Byml, origin: [f32; 3], presence: &Node, raws: &[&[u8]], taken: &mut Vec<u64>) -> Res<usize> {
    let mut made: Vec<Node> = Vec::new();
    for (i, side) in SIDES.iter().enumerate() {
        for (k, piece) in nets(origin, *side).iter().enumerate() {
            for (j, q) in [*piece, twin(piece)].iter().enumerate() {
                let h: u64 = next(raws, taken, &format!("{SEED} net {i} {k} {j}"));
                let mut s: Node = actor(NET, h, q.at, q.rot);
                s.set("Scale", floats(q.scale));
                s.set("Presence", presence.clone());
                made.push(s);
            }
        }
    }
    let count: usize = made.len();
    list(y, "Actors")?.extend(made);
    Ok(count)
}

fn source(rf: &Romfs, have: Have, p: &str) -> Res<Vec<u8>> {
    match have(p) {
        Some(b) => Ok(b),
        None => rf.read(p),
    }
}

fn next(raws: &[&[u8]], taken: &mut Vec<u64>, seed: &str) -> u64 {
    let mut n: u32 = 0;
    loop {
        let h: u64 = fresh(raws, &format!("{seed} {n}"));
        if !taken.contains(&h) {
            taken.push(h);
            return h;
        }
        n += 1;
    }
}

pub fn stage(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    let z: Vec<u8> = source(rf, have, MAP)?;
    let dict: u32 = dict_id(&z).unwrap_or(BANC_DICT);
    let land: Vec<u8> = rf.zs.decompress(&z)?;
    let fixed: Vec<u8> = rf.zs.decompress(&source(rf, have, BRIDGE_MAP)?)?;
    let mut yd: Byml = Byml::parse(&land)?;
    let (presence, origin): (Node, [f32; 3]) = bridge(&Byml::parse(&fixed)?)?;
    let mut taken: Vec<u64> = Vec::new();
    for m in [&Byml::parse(&fixed)?, &yd] {
        for a in m.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]) {
            taken.extend(hash_of(a));
        }
    }
    ramp(&mut yd, origin, &presence, &[&fixed, &land], &mut taken)?;
    let db: Vec<u8> = yd.write()?;
    Ok(vec![Out { path: MAP.to_string(), old: land.len(), new: db.len(), z: zs::compress_raw(&db, Some(dict))? }])
}

pub fn count(y: &Byml, gyaml: &str) -> usize {
    y.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]).iter().filter(|a| text(a, "Gyaml") == gyaml).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump() -> Option<std::path::PathBuf> {
        std::env::var("OOTRR_ROMFS").ok().map(std::path::PathBuf::from).filter(|p| p.join("Banc").is_dir())
    }

    fn none(_: &str) -> Option<Vec<u8>> {
        None
    }

    #[test]
    fn the_profile_runs_from_the_bridge_end_up_to_the_gatehouse() {
        assert!(PROFILE.windows(2).all(|w| w[1][0] < w[0][0] && w[1][1] > w[0][1]));
        let steepest: f32 = PROFILE.windows(2).map(|w| ((w[1][1] - w[0][1]) / (w[0][0] - w[1][0])).atan().to_degrees()).fold(0.0, f32::max);
        assert!(steepest < 60.0, "{steepest}");
    }

    fn up(p: &Piece) -> [f32; 2] {
        let (s, c): (f32, f32) = p.rot[0].sin_cos();
        [s, c]
    }

    fn face(p: &Piece, o: [f32; 3]) -> [f32; 2] {
        let (s, c): (f32, f32) = p.rot[0].sin_cos();
        let n: [f32; 2] = [c, -s];
        [p.at[2] - o[2] + n[0] * NET_SIZE[2] / 2.0, p.at[1] - o[1] + n[1] * NET_SIZE[2] / 2.0]
    }

    #[test]
    fn the_nets_lie_on_the_chain_line_without_gaps() {
        let o: [f32; 3] = [0.0, 150.35, 34.0];
        let ps: Vec<Piece> = nets(o, 1.0);
        assert_eq!(ps.len(), PROFILE.len() - 1);
        let mut reach: f32 = f32::MAX;
        for (k, p) in ps.iter().enumerate() {
            let w: &[[f32; 2]] = &PROFILE[k..k + 2];
            let len: f32 = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            let long: f32 = p.scale[1] * NET_SIZE[1];
            assert!((long - len - OVERLAP * 2.0).abs() < 1e-4);
            let d: [f32; 2] = up(p);
            let want: [f32; 2] = [(w[1][0] - w[0][0]) / len, (w[1][1] - w[0][1]) / len];
            assert!((d[0] - want[0]).abs() < 1e-4 && (d[1] - want[1]).abs() < 1e-4, "piece {k}: {d:?} vs {want:?}");
            let a: [f32; 2] = face(p, o);
            let b: [f32; 2] = [a[0] + d[0] * long, a[1] + d[1] * long];
            for q in w {
                let t: f32 = (q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1];
                let off: f32 = ((q[0] - a[0]) - d[0] * t).hypot((q[1] - a[1]) - d[1] * t);
                assert!(off < 1e-3 && t > 0.0 && t < long, "piece {k} misses {q:?}: off {off}, t {t}");
            }
            assert!(a[0] > PROFILE[k][0] && (k == 0 || a[0] > reach), "piece {k} leaves a gap");
            reach = b[0];
            assert!((p.at[0] - CHAIN_X).abs() < 1e-4);
            assert!(p.rot[1] == 0.0 && p.rot[2] == 0.0);
            assert!(p.scale[0] * NET_SIZE[0] >= 1.0 && p.scale[2] == 1.0);
        }
        assert!(reach <= PROFILE[PROFILE.len() - 1][0]);
        let left: Vec<Piece> = nets(o, -1.0);
        assert!(left.iter().zip(ps.iter()).all(|(l, r)| (l.at[0] + r.at[0]).abs() < 1e-4 && l.at[1] == r.at[1] && l.at[2] == r.at[2]));
    }

    #[test]
    fn each_twin_covers_the_same_strip_facing_the_other_way() {
        for p in nets([0.0, 150.0, 34.0], 1.0) {
            let t: Piece = twin(&p);
            let long: f32 = p.scale[1] * NET_SIZE[1];
            let (s, c): (f32, f32) = t.rot[0].sin_cos();
            let back: [f32; 3] = [t.at[0], t.at[1] + c * long, t.at[2] + s * long];
            for k in 0..3 {
                assert!((back[k] - p.at[k]).abs() < 1e-3, "{back:?} vs {:?}", p.at);
            }
            assert!((t.rot[0].sin() + p.rot[0].sin()).abs() < 1e-4 && (t.rot[0].cos() + p.rot[0].cos()).abs() < 1e-4);
        }
    }

    #[test]
    fn the_top_net_points_up_the_chain_and_its_face_looks_up() {
        let ps: Vec<Piece> = nets([0.0, 0.0, 0.0], 1.0);
        for p in &ps {
            let d: [f32; 2] = up(p);
            assert!(d[0] < 0.0 && d[1] > 0.0);
            assert!(-p.rot[0].sin() > 0.0);
        }
        let top: f32 = ps.iter().map(|p| up(p)[1].asin().to_degrees()).fold(0.0, f32::max);
        assert!(top < 60.0, "{top}");
    }

    #[test]
    fn only_the_child_field_dynamic_map_is_written() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let all: Vec<Out> = stage(&mut rf, &none).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].path, MAP);
        let before_d: Byml = rf.read_byml(MAP).unwrap();
        let before: Byml = rf.read_byml(BRIDGE_MAP).unwrap();
        let now_d: Byml = Byml::parse(&rf.zs.decompress(&all[0].z).unwrap()).unwrap();
        let old_d: &[Node] = before_d.root.get("Actors").unwrap().as_arr().unwrap();
        let acts_d: &[Node] = now_d.root.get("Actors").unwrap().as_arr().unwrap();
        assert_eq!(acts_d.len(), old_d.len() + SIDES.len() * (PROFILE.len() - 1) * 2);
        assert_eq!(&acts_d[..old_d.len()], old_d);
        assert_eq!(count(&now_d, NET), count(&before_d, NET) + SIDES.len() * (PROFILE.len() - 1) * 2);
        let (presence, _): (Node, [f32; 3]) = bridge(&before).unwrap();
        assert!(acts_d[old_d.len()..].iter().all(|s| s.get("Presence") == Some(&presence)));
        let mut hashes: Vec<u64> = acts_d.iter().filter_map(hash_of).collect();
        let n: usize = hashes.len();
        hashes.sort();
        hashes.dedup();
        assert_eq!(hashes.len(), n);
    }
}

