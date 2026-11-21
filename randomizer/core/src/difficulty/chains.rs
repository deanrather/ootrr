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
pub const STEP: &str = "Obj_MovableBox_A_01";
pub const BASE: [f32; 3] = [1.49, 1.5, 1.49];
pub const SEED: &str = "ootrr climbable drawbridge chains";
pub const SIDES: [f32; 2] = [-1.0, 1.0];
pub const CHAIN_X: f32 = 3.73;
pub const WIDTH: f32 = 1.2;
pub const THICK: f32 = 0.5;
pub const OVERLAP: f32 = 0.4;
pub const PITCH_SIGN: f32 = 1.0;

pub const PROFILE: [[f32; 2]; 11] = [[11.2, 0.45], [9.5, 1.12], [6.5, 2.9], [5.5, 3.6], [4.5, 4.27], [2.5, 6.0], [1.5, 7.0], [0.5, 8.1], [-0.5, 9.3], [-1.5, 10.5], [-2.4, 11.4]];

pub const NOTE: &str = "In the child Hyrule Field, a narrow invisible-collision ramp (wooden planks) runs up each of the two chains of the lowered Hyrule Castle drawbridge, from the end of the bridge to the gatehouse, so Link can climb them. Only while the bridge is down (by day). The adult castle has no chains, so nothing changes there.";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub at: [f32; 3],
    pub rot: [f32; 3],
    pub scale: [f32; 3],
}

pub fn pieces(origin: [f32; 3], side: f32) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for w in PROFILE.windows(2) {
        let dz: f32 = w[1][0] - w[0][0];
        let dy: f32 = w[1][1] - w[0][1];
        let len: f32 = (dz * dz + dy * dy).sqrt();
        let pitch: f32 = dy.abs().atan2(dz.abs());
        let (s, c): (f32, f32) = pitch.sin_cos();
        let mid: [f32; 2] = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
        let normal: [f32; 2] = [s, c];
        out.push(Piece {
            at: [origin[0] + side * CHAIN_X, origin[1] + mid[1] - normal[1] * THICK, origin[2] + mid[0] - normal[0] * THICK],
            rot: [pitch * PITCH_SIGN, 0.0, 0.0],
            scale: [WIDTH / BASE[0], THICK / BASE[1], (len + OVERLAP * 2.0) / BASE[2]],
        });
    }
    out
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
        for (k, piece) in pieces(origin, *side).iter().enumerate() {
            let h: u64 = next(raws, taken, &format!("{SEED} step {i} {k}"));
            let mut s: Node = actor(STEP, h, piece.at, piece.rot);
            s.set("Scale", floats(piece.scale));
            s.set("Presence", presence.clone());
            made.push(s);
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

    #[test]
    fn the_pieces_follow_the_profile_without_gaps() {
        let o: [f32; 3] = [0.0, 150.35, 34.0];
        let ps: Vec<Piece> = pieces(o, 1.0);
        assert_eq!(ps.len(), PROFILE.len() - 1);
        for (k, p) in ps.iter().enumerate() {
            let w: &[[f32; 2]] = &PROFILE[k..k + 2];
            let len: f32 = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            assert!((p.scale[2] * BASE[2] - len - OVERLAP * 2.0).abs() < 1e-4);
            assert!(p.rot[0] * PITCH_SIGN > 0.0 && p.rot[1] == 0.0 && p.rot[2] == 0.0);
            let (s, c): (f32, f32) = (p.rot[0] * PITCH_SIGN).sin_cos();
            let top: [f32; 2] = [p.at[2] - o[2] + s * THICK, p.at[1] - o[1] + c * THICK];
            let mid: [f32; 2] = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
            assert!((top[0] - mid[0]).abs() < 1e-3 && (top[1] - mid[1]).abs() < 1e-3, "piece {k}: {top:?} vs {mid:?}");
            assert!((p.at[0] - CHAIN_X).abs() < 1e-4);
        }
        let left: Vec<Piece> = pieces(o, -1.0);
        assert!(left.iter().zip(ps.iter()).all(|(l, r)| (l.at[0] + r.at[0]).abs() < 1e-4 && l.at[1] == r.at[1] && l.at[2] == r.at[2]));
    }

    #[test]
    fn a_positive_pitch_about_x_lifts_the_gatehouse_end() {
        let p: Piece = pieces([0.0, 0.0, 0.0], 1.0)[0];
        let (s, c): (f32, f32) = p.rot[0].sin_cos();
        let toward_gatehouse: [f32; 2] = [-c, s];
        assert!(toward_gatehouse[0] < 0.0 && toward_gatehouse[1] > 0.0);
        assert!(p.scale[0] * BASE[0] >= 1.0 && WIDTH >= 1.0);
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
        assert_eq!(acts_d.len(), old_d.len() + SIDES.len() * (PROFILE.len() - 1));
        assert_eq!(&acts_d[..old_d.len()], old_d);
        assert_eq!(count(&now_d, STEP), count(&before_d, STEP) + SIDES.len() * (PROFILE.len() - 1));
        let (presence, _): (Node, [f32; 3]) = bridge(&before).unwrap();
        assert!(acts_d[old_d.len()..].iter().all(|s| s.get("Presence") == Some(&presence)));
        let mut hashes: Vec<u64> = acts_d.iter().filter_map(hash_of).collect();
        let n: usize = hashes.len();
        hashes.sort();
        hashes.dedup();
        assert_eq!(hashes.len(), n);
    }
}
