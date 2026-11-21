use crate::ainb::build::{Spec, QUERY};
use crate::ainb::{audit, Ainb};
use crate::byml::{Byml, Node};
use crate::difficulty::ambush::{actor, floats, fresh, link, list, text, vec3};
use crate::difficulty::gates::{id_for, AND_NODE, AREA, AREA_NODE, BOOL, CREATE, DELETE, GRAPH as SOURCE, HIT_NODE, INT, NAME, NODE, POINTER, RESPAWN, SHARED, STR};
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::murmur::murmur3_32;
use crate::placement::hash_of;
use crate::qol::Have;
use crate::romfs::Romfs;
use crate::staging::{Out, BANC_DICT};
use crate::zs;

pub const MAP: &str = "Banc/HyruleField/HyruleField_Child_Static.bcett.byml.zs";
pub const BRIDGE: &str = "Land_HrlField_DrawBridge_A_01";
pub const STEP: &str = "Obj_MovableBox_A_01";
pub const BASE: [f32; 3] = [1.49, 1.5, 1.49];
pub const BOLT: &str = "PhantomGanon_Bullet_Thunder";
pub const PREFIX: &str = "Gimmick_ChainTop_";
pub const SEED: &str = "ootrr climbable drawbridge chains";
pub const SIDES: [f32; 2] = [-1.0, 1.0];
pub const CHAIN_X: f32 = 3.73;
pub const WIDTH: f32 = 0.9;
pub const THICK: f32 = 0.5;
pub const OVERLAP: f32 = 0.25;
pub const TOP_BOX: [f32; 3] = [2.4, 3.0, 3.0];
pub const TOP_FROM_END: f32 = 1.2;
pub const BOLT_INSET: f32 = 1.6;
pub const BOLT_LIFT: f32 = 0.0;
pub const PITCH_SIGN: f32 = -1.0;

pub const PROFILE: [[f32; 2]; 11] = [[11.2, 0.45], [9.5, 1.12], [6.5, 2.9], [5.5, 3.6], [4.5, 4.27], [2.5, 6.0], [1.5, 7.0], [0.5, 8.1], [-0.5, 9.3], [-1.5, 10.5], [-2.4, 11.4]];

pub const NOTE: &str = "In the child Hyrule Field, an invisible narrow ramp runs up each of the two chains of the lowered Hyrule Castle drawbridge, from the end of the bridge to the gatehouse, so Link can climb them. At the top of a chain, lightning strikes him and knocks him off into the moat; the strike hurts as much as the Phantom Ganon's lightning, which is not enough to kill from full health. Only while the bridge is down (by day). The adult castle has no chains, so nothing changes there.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub module: String,
    pub graph: String,
    pub bolts: Vec<String>,
    pub areas: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub at: [f32; 3],
    pub rot: [f32; 3],
    pub scale: [f32; 3],
}

pub fn plan() -> Plan {
    let mut taken: Vec<String> = Vec::new();
    let mut bolts: Vec<String> = Vec::new();
    let mut areas: Vec<String> = Vec::new();
    for i in 0..SIDES.len() {
        let b: String = id_for(BOLT, &format!("{SEED} bolt {i}"), &taken);
        taken.push(b.clone());
        bolts.push(b);
        let a: String = id_for(AREA, &format!("{SEED} area {i}"), &taken);
        taken.push(a.clone());
        areas.push(a);
    }
    let tag: u32 = murmur3_32(format!("{SEED} module").as_bytes(), 0) & 0xffff;
    let name: String = format!("{PREFIX}{tag:04x}");
    Plan { module: format!("Logic/Module/{PREFIX}Drawbridge/{name}.logic.module.ain"), graph: format!("Logic/{name}.logic.module.ainb"), bolts, areas }
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

pub fn top(origin: [f32; 3], side: f32) -> [f32; 3] {
    let end: [f32; 2] = PROFILE[PROFILE.len() - 1];
    let prev: [f32; 2] = PROFILE[PROFILE.len() - 2];
    let dz: f32 = end[0] - prev[0];
    let dy: f32 = end[1] - prev[1];
    let len: f32 = (dz * dz + dy * dy).sqrt();
    let back: f32 = TOP_FROM_END / len;
    [origin[0] + side * CHAIN_X, origin[1] + end[1] - dy * back + TOP_BOX[1] / 2.0 - 0.5, origin[2] + end[0] - dz * back]
}

pub fn bolt_at(origin: [f32; 3], side: f32) -> [f32; 3] {
    let t: [f32; 3] = top(origin, side);
    [t[0] - side * BOLT_INSET, t[1] + BOLT_LIFT, t[2]]
}

fn pool(a: &mut Ainb, s: &str) -> Vec<u8> {
    a.pool.add(s).to_le_bytes().to_vec()
}

pub fn graph(raw: &[u8]) -> Res<Out> {
    let p: Plan = plan();
    let mut a: Ainb = Ainb::parse(raw)?;
    for id in p.bolts.iter().chain(p.areas.iter()) {
        if (0..a.els.len()).any(|i| a.prop_str(i, NAME) == Some(id.as_str())) {
            return Err(E::Msg(format!("drawbridge chains: the graph already has a node named {id}")));
        }
    }
    let zero: Vec<u8> = 0u32.to_le_bytes().to_vec();
    let one: Vec<u8> = 1u32.to_le_bytes().to_vec();
    let empty: Vec<u8> = pool(&mut a, "");
    let player: Vec<u8> = pool(&mut a, "Player");
    for i in 0..SIDES.len() {
        let seed_a: String = format!("{SEED} {i} area node");
        let seed_h: String = format!("{SEED} {i} hit");
        let seed_i: String = format!("{SEED} {i} inside");
        let seed_b: String = format!("{SEED} {i} bolt");
        let area_name: Vec<u8> = pool(&mut a, &p.areas[i]);
        let bolt_name: Vec<u8> = pool(&mut a, &p.bolts[i]);
        let mut area: Spec = Spec::node(AREA_NODE, &seed_a)
            .prop(STR, "AddLayer", empty.clone())
            .prop(STR, NAME, area_name)
            .prop(STR, "Layer", player.clone())
            .idle(BOOL, CREATE, one.clone())
            .idle(BOOL, DELETE, zero.clone())
            .idle(BOOL, RESPAWN, zero.clone())
            .spare(BOOL, "Dead")
            .spare(BOOL, "Logic_Alive")
            .out_ptr("SensorHitInfo", POINTER);
        area.role = QUERY;
        let area_at: usize = a.add_node(&area)?;
        let mut hit: Spec = Spec::node(HIT_NODE, &seed_h)
            .prop(BOOL, "NoEventPause", zero.clone())
            .idle(INT, "ActorNum", one.clone())
            .idle(BOOL, "IsRecordContactActorLink", zero.clone())
            .idle(BOOL, "SendMessage", zero.clone())
            .constant(STR, "ActorName", player.clone())
            .linked_ptr("SensorHitInfoMulti", POINTER, area_at)
            .out(BOOL, "IsHit")
            .spare(BOOL, "IsNotHit")
            .spare_ptr("ContactActorLinkPtr", POINTER);
        hit.role = QUERY;
        let hit_at: usize = a.add_node(&hit)?;
        let mut inside: Spec = Spec::node(AND_NODE, &seed_i).linked(BOOL, "BoolMulti", hit_at, 0).idle(BOOL, "UpdateOnce", zero.clone()).spare(BOOL, "False").out(BOOL, "True");
        inside.role = QUERY;
        let inside_at: usize = a.add_node(&inside)?;
        let mut bolt: Spec = Spec::node(NODE, &seed_b)
            .prop(STR, NAME, bolt_name)
            .linked(BOOL, CREATE, inside_at, 1)
            .idle(BOOL, DELETE, zero.clone())
            .linked(BOOL, RESPAWN, inside_at, 1)
            .spare(BOOL, "Dead")
            .spare(BOOL, "Logic_Alive");
        bolt.role = QUERY;
        a.add_node(&bolt)?;
    }
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(E::Msg(format!("the drawbridge chains graph no longer checks out: {}", bad.join(", "))));
    }
    let g: Vec<u8> = a.write()?;
    Ok(Out { path: p.graph, old: 0, new: g.len(), z: g })
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

pub fn place(y: &mut Byml, raws: &[&[u8]]) -> Res<usize> {
    let origin: [f32; 3] = bridge(y)?.1;
    let p: Plan = plan();
    let mut taken: Vec<u64> = Vec::new();
    for a in y.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]) {
        taken.extend(hash_of(a));
    }
    let group: u64 = next(raws, &mut taken, &format!("{SEED} group"));
    let mut made: Vec<Node> = Vec::new();
    let mut hashes: Vec<u64> = Vec::new();
    for (i, side) in SIDES.iter().enumerate() {
        let area_h: u64 = next(raws, &mut taken, &format!("{SEED} area {i}"));
        let bolt_h: u64 = next(raws, &mut taken, &format!("{SEED} bolt {i}"));
        let mut area: Node = actor(AREA, area_h, top(origin, *side), [0.0, 0.0, 0.0]);
        area.set("Scale", floats(TOP_BOX));
        made.push(area);
        made.push(actor(BOLT, bolt_h, bolt_at(origin, *side), [0.0, 0.0, 0.0]));
        hashes.push(bolt_h);
        hashes.push(area_h);
    }
    let count: usize = made.len();
    list(y, "Actors")?.extend(made);
    let mut all: Vec<u64> = hashes.clone();
    all.sort();
    for name in SHARED {
        if y.root.get(name).is_some() {
            list(y, name)?.push(Node::Arr(all.iter().map(|h| Node::U64(*h)).collect()));
        }
    }
    let mut refs: Vec<Node> = Vec::new();
    for i in 0..SIDES.len() {
        refs.push(link(&p.bolts[i], hashes[2 * i]));
        refs.push(link(&p.areas[i], hashes[2 * i + 1]));
    }
    let mut g: Node = Node::Map(Vec::new());
    g.set("Hash", Node::U64(group));
    g.set("Logic", Node::Str(p.module.clone()));
    g.set("References", Node::Arr(refs));
    let gs: &mut Vec<Node> = list(y, "AiGroups")?;
    let i: usize = gs.iter().position(|x| hash_of(x).is_some_and(|v| v > group)).unwrap_or(gs.len());
    gs.insert(i, g);
    Ok(count)
}

fn source(rf: &Romfs, have: Have, p: &str) -> Res<Vec<u8>> {
    match have(p) {
        Some(b) => Ok(b),
        None => rf.read(p),
    }
}

fn one(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    let z: Vec<u8> = source(rf, have, MAP)?;
    let dict: u32 = dict_id(&z).unwrap_or(BANC_DICT);
    let raw: Vec<u8> = rf.zs.decompress(&z)?;
    let dynamic: String = MAP.replace("_Static", "_Dynamic");
    let land: Vec<u8> = rf.zs.decompress(&source(rf, have, &dynamic)?)?;
    let mut y: Byml = Byml::parse(&raw)?;
    let mut yd: Byml = Byml::parse(&land)?;
    let (presence, origin): (Node, [f32; 3]) = bridge(&y)?;
    place(&mut y, &[&raw, &land])?;
    let mut taken: Vec<u64> = Vec::new();
    for m in [&y, &yd] {
        for a in m.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]) {
            taken.extend(hash_of(a));
        }
    }
    ramp(&mut yd, origin, &presence, &[&raw, &land], &mut taken)?;
    let bytes: Vec<u8> = y.write()?;
    let db: Vec<u8> = yd.write()?;
    Ok(vec![
        Out { path: dynamic, old: land.len(), new: db.len(), z: zs::compress_raw(&db, Some(dict))? },
        Out { path: MAP.to_string(), old: raw.len(), new: bytes.len(), z: zs::compress_raw(&bytes, Some(dict))? },
    ])
}

pub fn stage(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    let was: Vec<u8> = source(rf, have, SOURCE)?;
    let mut v: Vec<Out> = vec![graph(&was)?];
    v.extend(one(rf, have)?);
    Ok(v)
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
    fn the_top_area_and_the_bolt_sit_at_the_top_of_each_chain() {
        let o: [f32; 3] = [0.0, 150.35, 34.0];
        for side in SIDES {
            let t: [f32; 3] = top(o, side);
            let end: [f32; 2] = PROFILE[PROFILE.len() - 1];
            assert!((t[2] - (o[2] + end[0])).abs() < TOP_FROM_END + 0.01);
            assert!((t[1] - (o[1] + end[1])).abs() < TOP_BOX[1]);
            let b: [f32; 3] = bolt_at(o, side);
            assert!(b[0].abs() < t[0].abs() && (b[0] - t[0]).signum() == -side);
        }
    }

    #[test]
    fn the_names_are_unique_and_stable() {
        let p: Plan = plan();
        assert_eq!(p, plan());
        let mut ids: Vec<String> = p.bolts.iter().chain(p.areas.iter()).cloned().collect();
        ids.push(p.graph.clone());
        ids.push(p.module.clone());
        let n: usize = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert_eq!(p.bolts.len(), SIDES.len());
    }

    #[test]
    fn the_graph_checks_out_and_names_both_areas_and_bolts() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let rf: Romfs = Romfs::open(&root).unwrap();
        let was: Vec<u8> = rf.read(SOURCE).unwrap();
        let before: Ainb = Ainb::parse(&was).unwrap();
        let out: Out = graph(&was).unwrap();
        let a: Ainb = Ainb::parse(&out.z).unwrap();
        assert!(audit(&a).is_empty());
        assert!(a.els.len() >= before.els.len() + SIDES.len() * 4);
        assert_eq!(out.z, graph(&was).unwrap().z);
        let p: Plan = plan();
        for id in p.bolts.iter().chain(p.areas.iter()) {
            assert!((0..a.els.len()).any(|i| a.prop_str(i, NAME) == Some(id.as_str())));
        }
    }

    #[test]
    fn only_the_child_field_maps_and_the_graph_are_written() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let all: Vec<Out> = stage(&mut rf, &none).unwrap();
        assert_eq!(all.len(), 3);
        let dynamic: String = MAP.replace("_Static", "_Dynamic");
        let map: &Out = all.iter().find(|o| o.path == MAP).unwrap();
        let land: &Out = all.iter().find(|o| o.path == dynamic).unwrap();
        assert!(all.iter().any(|o| o.path.starts_with("Logic/")));
        let before: Byml = rf.read_byml(MAP).unwrap();
        let before_d: Byml = rf.read_byml(&dynamic).unwrap();
        let now: Byml = Byml::parse(&rf.zs.decompress(&map.z).unwrap()).unwrap();
        let now_d: Byml = Byml::parse(&rf.zs.decompress(&land.z).unwrap()).unwrap();
        let old: &[Node] = before.root.get("Actors").unwrap().as_arr().unwrap();
        let acts: &[Node] = now.root.get("Actors").unwrap().as_arr().unwrap();
        assert_eq!(acts.len(), old.len() + SIDES.len() * 2);
        assert_eq!(&acts[..old.len()], old);
        assert_eq!(count(&now, STEP), count(&before, STEP));
        assert_eq!(count(&now, BOLT), SIDES.len());
        assert_eq!(count(&now, AREA), count(&before, AREA) + SIDES.len());
        let old_d: &[Node] = before_d.root.get("Actors").unwrap().as_arr().unwrap();
        let acts_d: &[Node] = now_d.root.get("Actors").unwrap().as_arr().unwrap();
        assert_eq!(acts_d.len(), old_d.len() + SIDES.len() * (PROFILE.len() - 1));
        assert_eq!(&acts_d[..old_d.len()], old_d);
        assert_eq!(count(&now_d, STEP), count(&before_d, STEP) + SIDES.len() * (PROFILE.len() - 1));
        let (presence, _): (Node, [f32; 3]) = bridge(&before).unwrap();
        let steps: Vec<&Node> = acts_d[old_d.len()..].iter().collect();
        assert!(steps.iter().all(|s| s.get("Presence") == Some(&presence)));
        let mut hashes: Vec<u64> = acts.iter().chain(acts_d.iter()).filter_map(hash_of).collect();
        let n: usize = hashes.len();
        hashes.sort();
        hashes.dedup();
        assert_eq!(hashes.len(), n);
        let gs: &[Node] = now.root.get("AiGroups").unwrap().as_arr().unwrap();
        let keys: Vec<u64> = gs.iter().filter_map(hash_of).collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
        let p: Plan = plan();
        let g: &Node = gs.iter().find(|g| text(g, "Logic") == p.module).unwrap();
        assert_eq!(g.get("References").unwrap().as_arr().unwrap().len(), SIDES.len() * 2);
    }
}
