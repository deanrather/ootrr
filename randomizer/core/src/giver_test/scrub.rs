use std::fs;
use std::path::{Path as P, PathBuf as PB};

use crate::byml::{Byml, Node};
use crate::difficulty::ambush::{euler, floats, fresh, list, text, vec3};
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::giver_test::LINE;
use crate::modpack::{Host, Pack};
use crate::pickup_pack::write::start;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst, TABLE_PATH};
use crate::settings::seed::SeedFile;
use crate::slice::edit::Built;
use crate::staging::MARGIN;
use crate::zs;
use crate::TITLE_ID;

pub const MAP: &str = "Banc/KokiriForest/KokiriForest_Child_Static.bcett.byml.zs";
pub const LAND: &str = "Banc/KokiriForest/KokiriForest_Child_Dynamic.bcett.byml.zs";
pub const BODY: &str = "Enemy_Scrub_Business";
pub const LEAF: &str = "Enemy_Scrub_Business_Leaf";
pub const SALE: &str = "Enemy_Scrub_Business_DekuStickPouch_01";
pub const AT: [f32; 3] = [-14.0, 110.49, 4.0];
pub const TURN: f32 = 1.5707964;
pub const SEED: &str = "ootrr giver test scrub";

fn hash(n: &Node) -> Option<u64> {
    match n.get("Hash") {
        Some(Node::U64(h)) => Some(*h),
        _ => None,
    }
}

fn moved(n: &Node, id: u64, at: [f32; 3]) -> Node {
    let mut c: Node = n.clone();
    c.set("Hash", Node::U64(id));
    c.set("Phive", Node::Map(vec![("Placement".to_string(), Node::Map(vec![("ID".to_string(), Node::U64(id))]))]));
    c.set("SRTHash", Node::U32(crate::murmur::murmur3_32(&id.to_le_bytes(), 0)));
    c.set("Translate", floats(at));
    c.set("Rotate", floats(euler(0.0, TURN)));
    c
}

pub fn add(y: &mut Byml, raws: &[&[u8]]) -> Res<usize> {
    let acts: &mut Vec<Node> = list(y, "Actors")?;
    let bodies: Vec<usize> = (0..acts.len()).filter(|i| text(&acts[*i], "Gyaml") == BODY).collect();
    let [b] = bodies.as_slice() else {
        return Err(E::Msg(format!("{LINE}: the Kokiri Forest map has {} {BODY} actors and not one, the game files differ", bodies.len())));
    };
    let body: Node = acts[*b].clone();
    let id: u64 = hash(&body).ok_or_else(|| E::Msg(format!("{LINE}: the {BODY} has no hash")))?;
    let sale: &str = body.get("Dynamic").map(|d| text(d, "TalkEventName")).unwrap_or("");
    if sale != SALE {
        return Err(E::Msg(format!("{LINE}: the Kokiri Forest {BODY} sells {sale:?} and not {SALE}, the game files differ")));
    }
    let leaves: Vec<usize> = (0..acts.len())
        .filter(|i| {
            text(&acts[*i], "Gyaml") == LEAF
                && acts[*i].get("Links").and_then(|l| l.as_arr()).is_some_and(|l| l.iter().any(|r| matches!(r.get("Dst"), Some(Node::U64(d)) if *d == id)))
        })
        .collect();
    let [l] = leaves.as_slice() else {
        return Err(E::Msg(format!("{LINE}: the Kokiri Forest {BODY} has {} leaves and not one, the game files differ", leaves.len())));
    };
    let leaf: Node = acts[*l].clone();
    if vec3(&body, "Translate").is_none() || hash(&leaf).is_none() {
        return Err(E::Msg(format!("{LINE}: the Kokiri Forest scrub pair has no position, the game files differ")));
    }
    let new_body: u64 = fresh(raws, &format!("{SEED} body"));
    let taken: [u8; 8] = new_body.to_le_bytes();
    let mut more: Vec<&[u8]> = raws.to_vec();
    more.push(&taken);
    let new_leaf: u64 = fresh(&more, &format!("{SEED} leaf"));
    let a: Node = moved(&body, new_body, AT);
    let mut c: Node = moved(&leaf, new_leaf, AT);
    let mut r: Node = Node::Map(Vec::new());
    r.set("Dst", Node::U64(new_body));
    r.set("Gyaml", Node::Str("Reference".to_string()));
    r.set("Name", Node::Str("Reference".to_string()));
    r.set("Src", Node::U64(new_leaf));
    c.set("Links", Node::Arr(vec![r]));
    acts.push(a);
    acts.push(c);
    Ok(2)
}

fn local(base: &P, path: &str) -> PB {
    path.split('/').fold(base.to_path_buf(), |a, s| a.join(s))
}

pub fn after(seed: &SeedFile, romfs: &P, load: &P, mod_name: &str, host: Host, mut b: Built) -> Res<Built> {
    if seed.giver_test.is_none() {
        return Ok(b);
    }
    let mut rf: Romfs = Romfs::open(romfs)?;
    let root: PB = b.root.join("romfs");
    let own: PB = root.join(TABLE_PATH);
    let mut rst: Rst = if own.is_file() { Rst::parse(&rf.zs.decompress(&fs::read(&own)?)?)? } else { rsizetable::load(&mut rf)? };
    let pk: Pack = Pack::create_for(load, TITLE_ID, mod_name, host)?;
    if pk.root != b.root {
        return Err(E::Msg(format!("{LINE}: the scrub went to {} and not to the seed mod {}", pk.root.display(), b.root.display())));
    }
    let modded: bool = local(&root, MAP).is_file();
    let first: Vec<u8> = start(&mut rf, Some(&root), MAP)?;
    let land: Vec<u8> = start(&mut rf, Some(&root), LAND)?;
    let mut y: Byml = Byml::parse(&first)?;
    add(&mut y, &[&first, &land])?;
    let raw: Vec<u8> = y.write()?;
    let old: usize = if modded { first.len() } else { rf.read_zs(MAP)?.len() };
    pk.write_romfs(MAP, &zs::compress_raw(&raw, dict_id(&rf.read(MAP)?))?)?;
    if raw.len() > old {
        rst.grow(MAP, old as u64, raw.len() as u64, MARGIN)?;
    }
    b.counts.files += usize::from(!modded);
    pk.write_romfs(TABLE_PATH, &rst.to_zs()?)?;
    b.report.push_str(&format!("{LINE}: a Business Scrub of the Lost Woods sale row stands in the Kokiri Forest village at {} {} {}\n", AT[0], AT[1], AT[2]));
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::difficulty::ground;

    fn root() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Banc").is_dir() {
            Some(p)
        } else {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
            None
        }
    }

    #[test]
    fn the_scrub_pair_joins_the_village_map_as_a_linked_copy_of_the_maps_own() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let first: Vec<u8> = rf.read_zs(MAP).unwrap();
        let land: Vec<u8> = rf.read_zs(LAND).unwrap();
        let mut y: Byml = Byml::parse(&first).unwrap();
        let before: usize = y.root.get("Actors").unwrap().as_arr().unwrap().len();
        assert_eq!(add(&mut y, &[&first, &land]).unwrap(), 2);
        let back: Byml = Byml::parse(&y.write().unwrap()).unwrap();
        let acts: &[Node] = back.root.get("Actors").unwrap().as_arr().unwrap();
        assert_eq!(acts.len(), before + 2);
        let (body, leaf): (&Node, &Node) = (&acts[before], &acts[before + 1]);
        assert_eq!((text(body, "Gyaml"), text(leaf, "Gyaml")), (BODY, LEAF));
        assert_eq!(vec3(body, "Translate"), Some(AT));
        assert_eq!(vec3(leaf, "Translate"), Some(AT));
        let id: u64 = hash(body).unwrap();
        assert!(matches!(leaf.get("Links").unwrap().as_arr().unwrap()[0].get("Dst"), Some(Node::U64(d)) if *d == id));
        assert!(matches!(leaf.get("Links").unwrap().as_arr().unwrap()[0].get("Src"), Some(Node::U64(s)) if Some(*s) == hash(leaf)));
        assert_eq!(text(body.get("Dynamic").unwrap(), "TalkEventName"), SALE);
        let all: Vec<u64> = acts.iter().filter_map(hash).collect();
        let mut sorted: Vec<u64> = all.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len(), "every hash is unique");
        assert!(add(&mut Byml::parse(&first).unwrap(), &[&first, &land]).is_ok());
    }

    #[test]
    fn the_scrub_stands_on_the_villages_walk_mesh_clear_of_the_arrival_points_and_scene_triggers() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let g: ground::Ground = ground::load(&mut rf, "KokiriForest_Child").unwrap().unwrap();
        let (h, _): (f32, u32) = g.floor_near(AT[0], AT[1], AT[2], 0.5, 0.5).expect("a walkable face under the scrub");
        assert!((h - AT[1]).abs() < 0.1, "floor {h}");
        assert!(g.clear(AT) >= 4.0, "clear {}", g.clear(AT));
        assert!(g.steep(AT, 1.5).is_some_and(|s| s < 12.0));
        let y: Byml = rf.read_byml(MAP).unwrap();
        for a in y.root.get("Actors").unwrap().as_arr().unwrap() {
            let n: &str = text(a, "Gyaml");
            let Some(q) = vec3(a, "Translate") else { continue };
            let d: f32 = ((q[0] - AT[0]).powi(2) + (q[2] - AT[2]).powi(2)).sqrt();
            if n == "DestinationAnchor" {
                assert!(d >= 5.0, "{n} at {d}");
            }
            if n.contains("Request") || n == "LocationArea" {
                assert!(d >= 5.0, "{n} at {d}");
            }
        }
        let far: f32 = ((AT[0] - 0.0).powi(2) + (AT[2] + 10.0).powi(2)).sqrt().min(((AT[0] + 4.4).powi(2) + (AT[2] + 10.0).powi(2)).sqrt());
        assert!(far > 12.0, "the shop and Saria scene triggers are {far} away");
    }
}
