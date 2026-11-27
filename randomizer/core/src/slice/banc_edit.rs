use std::collections::{BTreeMap as BM, BTreeSet as BS};

use crate::boss_heart::{heart_ids, swap, CHEST, HEART};
use crate::byml::Byml;
use crate::dungeon_credit::place as place_key;
use crate::error::{Error as E, Res};
use crate::progressive::{guard_family, guarded, route, Size};
use crate::special;
use crate::slice::banc::{find, set_drop_ev, Found};
use crate::slice::data::Ev;
use crate::slice::pickups as pu;
use crate::slice::chest_targets::Target;



pub struct Edited {
    pub file: String,
    pub old_len: usize,
    pub bytes: Vec<u8>,
    pub hits: Vec<(usize, Found)>,
    pub used: Vec<(String, Size)>,
    pub hearts: Vec<(u64, Vec<String>)>,
    pub wraps: Vec<(u64, String)>,
    pub touched: Vec<u64>,
}

pub fn edit_file(file: &str, raw: &[u8], ts: &[Target]) -> Res<Option<Edited>> {
    edit_file_with(file, raw, ts, false, &BS::new(), &BM::new(), &BS::new())
}

pub fn edit_file_with(file: &str, raw: &[u8], ts: &[Target], keys: bool, small: &BS<u64>, lifts: &BM<String, f32>, wrapped: &BS<u64>) -> Res<Option<Edited>> {
    let mut y: Byml = Byml::parse(raw)?;
    let mut hits: Vec<(usize, Found)> = Vec::new();
    let mut used: Vec<(String, Size)> = Vec::new();
    let mut hearts: Vec<(u64, Vec<String>)> = Vec::new();
    let mut wraps: Vec<(u64, String)> = Vec::new();
    let mut touched: Vec<u64> = Vec::new();
    for (i, t) in ts.iter().enumerate() {
        let n: usize = if t.old_actor == HEART && t.heart_chest {
            let chest: &str = if small.contains(&t.hash) { crate::chest_size::PAIRS[0].0 } else { CHEST };
            let n: usize = swap(&mut y, t.hash, &t.new_actor, chest);
            if n > 0 {
                hearts.push((t.hash, heart_ids(&y, t.hash)));
            }
            n
        } else if t.old_actor == crate::slice::trap_chests::ACTOR {
            crate::slice::trap_chests::open(&mut y, t.hash, &t.new_actor)?
        } else {
            match pu::swap_gyaml(&mut y, t.hash, &t.old_actor, &t.new_actor) {
                0 => set_drop_ev(&mut y, t.hash, &t.old_actor, &t.new_actor, t.ev)?,
                k => {
                    if let Some(dy) = lifts.get(&t.new_actor) {
                        crate::pickup_pack::lift::raise(&mut y, t.hash, *dy);
                    }
                    k
                }
            }
        };
        if n > 0 {
            crate::slice::treasure::set_view(&mut y, t.hash, &t.new_actor);
            if let Some(k) = t.key {
                place_key(&mut y, t.hash, k).ok_or(E::Format("edited key chest vanished"))?;
            }
            if special::of(t.hash).is_some() {
                touched.push(t.hash);
            }
            if let Some(f) = t.family {
                if special::of(t.hash).is_some() {
                    let g: String = find(&y, file, t.hash).into_iter().next().map(|x| x.gyaml).unwrap_or_default();
                    used.push((f.to_string(), Size::of_gyaml(&g)));
                    wraps.push((t.hash, f.to_string()));
                } else {
                    let size: Size = route(&mut y, t.hash, f).ok_or(E::Format("edited actor vanished"))?;
                    used.push((f.to_string(), size));
                }
            } else if t.key.is_none() && guarded(&t.new_actor) && t.hash == crate::slice::treasure::FIRST {
                crate::slice::treasure::route_guard(&mut y, &t.new_actor)?;
            } else if t.key.is_none() && guarded(&t.new_actor) && (t.ev != Some(Ev::Keep) || special::sized(t.hash).is_some()) {
                let fam: String = guard_family(&t.new_actor);
                if special::sized(t.hash).is_some() {
                    // A summoned chest starts its open event from its story flow, which the special
                    // stage points at the guard, as it does for an upgrade chain.
                    let g: String = find(&y, file, t.hash).into_iter().next().map(|x| x.gyaml).unwrap_or_default();
                    used.push((fam.clone(), Size::of_gyaml(&g)));
                    wraps.push((t.hash, fam));
                } else {
                    let size: Size = route(&mut y, t.hash, &fam).ok_or(E::Format("edited actor vanished"))?;
                    used.push((fam, size));
                }
            }
            let f: Found = find(&y, file, t.hash).into_iter().next().ok_or(E::Format("edited actor vanished"))?;
            hits.push((i, f));
        }
    }
    let swept: usize = if keys { crate::dungeon_credit::sweep_except(&mut y, file, wrapped)? } else { 0 };
    if hits.is_empty() && swept == 0 {
        return Ok(None);
    }
    Ok(Some(Edited { file: file.to_string(), old_len: raw.len(), bytes: y.write()?, hits, used, hearts, wraps, touched }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::byml::Node;

    fn chest(hash: u64, drop: &str, gyaml: &str) -> Node {
        Node::Map(vec![
            ("Dynamic".to_string(), Node::Map(vec![("Drop__DropActor".to_string(), Node::Str(drop.to_string())), ("GetEventName".to_string(), Node::Str("Story".to_string()))])),
            ("Gyaml".to_string(), Node::Str(gyaml.to_string())),
            ("Hash".to_string(), Node::U64(hash)),
            ("Translate".to_string(), Node::Arr(vec![Node::F32(1.0), Node::F32(2.0), Node::F32(3.0)])),
        ])
    }

    fn target(hash: u64, old: &str, new: &str) -> Target {
        Target {
            name: "Chest".to_string(),
            hash,
            files: vec!["Banc/Area/a.bcett.byml.zs".to_string()],
            old_item: "Old".to_string(),
            new_item: "Bombs (20)".to_string(),
            old_actor: old.to_string(),
            new_actor: new.to_string(),
            ev: Some(Ev::Keep),
            family: None,
            key: None,
            heart_chest: false,
        }
    }

    #[test]
    fn ammo_in_a_summoned_story_chest_takes_the_bomb_bag_guard() {
        const OTHER: u64 = 77;
        let list: Vec<Node> = vec![
            chest(special::DAMPE_HOOK, "Item_Hookshot", "Obj_TBox_A_02_NoBake"),
            chest(special::SPIRIT, "Armor_Adult_Gauntlet_Silver", "Obj_TBox_A_02"),
            chest(special::ICE_ARROWS, "Item_IceArrow", "Obj_TBox_A_01"),
            chest(OTHER, "Obj_Heart_Piece", "Obj_TBox_A_02"),
        ];
        let raw: Vec<u8> = Byml { root: Node::Map(vec![("Actors".to_string(), Node::Arr(list))]), version: 7, big: false }.write().unwrap();
        let bombs: &str = "Item_BombBundle_Small";
        let ts: Vec<Target> = vec![
            target(special::DAMPE_HOOK, "Item_Hookshot", bombs),
            target(special::SPIRIT, "Armor_Adult_Gauntlet_Silver", "TBox_ArrowBundle_Medium"),
            target(special::ICE_ARROWS, "Item_IceArrow", "TBox_DekuSeedBundle"),
            target(OTHER, "Obj_Heart_Piece", bombs),
        ];
        let e: Edited = edit_file("Banc/Area/a.bcett.byml.zs", &raw, &ts).unwrap().unwrap();
        assert_eq!(
            e.wraps,
            vec![
                (special::DAMPE_HOOK, guard_family(bombs)),
                (special::SPIRIT, guard_family("TBox_ArrowBundle_Medium")),
                (special::ICE_ARROWS, guard_family("TBox_DekuSeedBundle")),
            ]
        );
        assert!(e.used.contains(&(guard_family(bombs), Size::Big)));
        assert!(e.used.contains(&(guard_family("TBox_DekuSeedBundle"), Size::Small)));
        assert_eq!(e.touched, vec![special::DAMPE_HOOK, special::SPIRIT]);
        let back: Byml = Byml::parse(&e.bytes).unwrap();
        for h in [special::DAMPE_HOOK, OTHER] {
            let f: Found = find(&back, "f", h).into_iter().next().unwrap();
            assert_eq!(f.drop.as_deref(), Some(bombs));
        }
        let list: &[Node] = back.root.get("Actors").and_then(|a| a.as_arr()).unwrap();
        let other: &Node = list.iter().find(|a| crate::placement::hash_of(a) == Some(OTHER)).unwrap();
        assert_eq!(other.get("Dynamic").and_then(|d| d.get("GetEventName")).and_then(|v| v.as_str()), Some("Story"));
    }
}
