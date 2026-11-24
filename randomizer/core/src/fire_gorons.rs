use crate::ainb::Ainb;
use crate::error::{Error as E, Res};
use crate::romfs::Romfs;
use crate::staging::Out;

pub const NOTE: &str = "The jailed Gorons of the Fire Temple are gone. Their cells still open with their switches, and the chests in them open without talking to a Goron first.";

const BOOL: usize = 1;
const NPC: &str = "ActorLogicNpc";
const TBOX: &str = "ActorLogicTreasureBox";

pub const GORONS: [(&str, &str); 9] = [
    ("Room_02_Prefab_f1d5", "Npc_Goron_005_Adult_FireTemple_e40a"),
    ("Room_04_Prefab_c7fa", "Npc_Goron_002_Adult_FireTemple_6846"),
    ("Room_05_Prefab_812f", "Npc_Goron_008_Adult_FireTemple_26ba"),
    ("Room_10_Prefab_1fcc", "Npc_Goron_004_Adult_FireTemple_8eea"),
    ("Room_11_Prefab_6c9c", "Npc_Goron_009_Adult_FireTemple_127c"),
    ("Room_17_Prefab_68e8", "Npc_Goron_010_Adult_FireTemple_e8eb"),
    ("Room_20_Prefab_4850", "Npc_Goron_001_Adult_FireTemple_1f3b"),
    ("Room_22_Prefab_f820", "Npc_Goron_003_Adult_FireTemple_e347"),
    ("Room_23_Prefab_612a", "Npc_Goron_007_Adult_FireTemple_6279"),
];

pub const CHESTS: [(&str, &str); 9] = [
    ("Room_02_Prefab_3004", "Obj_TBox_A_01_435b"),
    ("Room_04_Prefab_d826", "Obj_TBox_A_01_d31e"),
    ("Room_05_Prefab_5b8f", "Obj_TBox_A_01_c516"),
    ("Room_10_Prefab_b3bc", "Obj_TBox_A_01_ade4"),
    ("Room_11_Prefab_c20f", "Obj_TBox_A_01_58b1"),
    ("Room_17_Prefab_a4ce", "Obj_TBox_A_03_30dc"),
    ("Room_20_Prefab_7421", "Obj_TBox_A_01_ac82"),
    ("Room_22_Prefab_e0c9", "Obj_TBox_A_01_221a"),
    ("Room_23_Prefab_efdf", "Obj_TBox_A_01_e720"),
];

pub fn path(room: &str) -> String {
    format!("Logic/FireTemple_Level_{room}.logic.module.ainb")
}

fn named(a: &Ainb, kind: &str, instance: &str) -> Res<usize> {
    a.find_els(kind).into_iter().find(|&i| a.prop_str(i, "InstanceName") == Some(instance)).ok_or_else(|| E::Msg(format!("fire temple gorons: no {instance}, the game files differ")))
}

fn source(a: &Ainb, i: usize, name: &str) -> Res<i16> {
    let g: usize = a.find_in(i, BOOL, name).ok_or_else(|| E::Msg(format!("fire temple gorons: element {i} has no {name}")))?;
    let x = a.input(BOOL, g).ok_or(E::Format("fire temple gorons: input out of range"))?;
    if x.src < 0 {
        return Err(E::Msg(format!("fire temple gorons: {name} of element {i} is already a constant, the module is not the game's")));
    }
    Ok(x.src)
}

pub fn remove(a: &mut Ainb, goron: &str) -> Res<usize> {
    let i: usize = named(a, NPC, goron)?;
    let src: i16 = source(a, i, "Logic_Delete")?;
    let hit: Vec<usize> = (0..a.els.len())
        .filter(|&j| a.find_in(j, BOOL, "Logic_Delete").and_then(|g| a.input(BOOL, g)).is_some_and(|x| x.src == src))
        .collect();
    for &j in &hit {
        a.force_bool(j, "Logic_Delete", true)?;
    }
    Ok(hit.len())
}

pub fn unlock(a: &mut Ainb, chest: &str) -> Res<()> {
    let i: usize = named(a, TBOX, chest)?;
    source(a, i, "Unlock")?;
    a.force_bool(i, "Unlock", true)?;
    Ok(())
}

fn rewrite(rf: &mut Romfs, room: &str, f: impl FnOnce(&mut Ainb) -> Res<()>) -> Res<Out> {
    let p: String = path(room);
    let raw: Vec<u8> = rf.read(&p)?;
    let mut a: Ainb = Ainb::parse(&raw)?;
    f(&mut a)?;
    let out: Vec<u8> = a.write()?;
    if Ainb::parse(&out)?.write()? != out {
        return Err(E::Msg(format!("fire temple gorons: {p} does not round trip")));
    }
    Ok(Out { path: p, old: raw.len(), new: out.len(), z: out })
}

pub fn stage(rf: &mut Romfs) -> Res<Vec<Out>> {
    let mut out: Vec<Out> = Vec::new();
    for (room, goron) in GORONS {
        out.push(rewrite(rf, room, |a| remove(a, goron).map(|_| ()))?);
    }
    for (room, chest) in CHESTS {
        out.push(rewrite(rf, room, |a| unlock(a, chest))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::byml::{Byml, Node};
    use crate::settings::{Settings, Value};
    use crate::slice::data::Data;
    use std::path::{Path as P, PathBuf as PB};

    const SCENE: &str = "Banc/FireTemple/FireTemple_Static.bcett.byml.zs";
    const EVENTS: &str = "RSDB/EventSetting.Product.111.rstbl.byml.zs";
    const BOSS_KEY: &str = "Fire Temple Boss Key Chest";

    fn romfs() -> Option<Romfs> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Romfs::open(&p).ok()
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn constant(a: &Ainb, kind: &str, instance: &str, name: &str) -> Option<bool> {
        let i: usize = named(a, kind, instance).ok()?;
        let x = a.input(BOOL, a.find_in(i, BOOL, name)?)?;
        (x.src < 0).then(|| x.val.iter().any(|&b| b != 0))
    }

    #[test]
    fn each_goron_and_its_wall_are_gone_and_each_chest_is_unlocked() {
        let Some(mut rf) = romfs() else {
            return;
        };
        let outs: Vec<Out> = stage(&mut rf).unwrap();
        assert_eq!(outs.len(), 18);
        for (room, goron) in GORONS {
            let mut a: Ainb = Ainb::parse(&rf.read(&path(room)).unwrap()).unwrap();
            assert_eq!(remove(&mut a, goron).unwrap(), 2, "{room}");
            assert_eq!(constant(&a, NPC, goron, "Logic_Delete"), Some(true), "{room}");
            let o: &Out = outs.iter().find(|o| o.path == path(room)).unwrap();
            let b: Ainb = Ainb::parse(&o.z).unwrap();
            assert_eq!(constant(&b, NPC, goron, "Logic_Delete"), Some(true), "{room}");
            assert!(remove(&mut a, goron).is_err(), "{room}");
        }
        for (room, chest) in CHESTS {
            let o: &Out = outs.iter().find(|o| o.path == path(room)).unwrap();
            let b: Ainb = Ainb::parse(&o.z).unwrap();
            assert_eq!(constant(&b, TBOX, chest, "Unlock"), Some(true), "{room}");
        }
    }

    fn built(d: &Data, root: &P, pairs: &[(&str, Value)], seed: &str) -> PB {
        let mut s: Settings = Settings::default();
        for (k, v) in pairs {
            s.set(k, v.clone()).unwrap();
        }
        let g = crate::slice::generate::generate_data(d, &s, seed, &crate::slice::GenOpts::default()).unwrap();
        let load: PB = PB::from(env!("CARGO_MANIFEST_DIR")).join("../../target/fire_gorons_mods");
        crate::slice::edit::build(&g.seed_file, root, &load, seed).unwrap().root.join("romfs")
    }

    fn file(rf: &Romfs, out: &P, r: &str) -> Vec<u8> {
        let p: PB = r.split('/').fold(out.to_path_buf(), |a, x| a.join(x));
        std::fs::read(&p).unwrap_or_else(|_| rf.read(r).unwrap())
    }

    fn table(rf: &mut Romfs, out: &P, r: &str) -> Byml {
        let z: Vec<u8> = file(rf, out, r);
        Byml::parse(&rf.zs.decompress(&z).unwrap()).unwrap()
    }

    fn module(logic: &str) -> String {
        format!("Logic/{}b", logic.rsplit('/').next().unwrap_or(""))
    }

    fn reads_goron(a: &Ainb) -> bool {
        a.find_els("HoldGameDataGetBoolSimple").into_iter().any(|i| a.in_str(i, "GameDataNameBool").is_some_and(|n| n.starts_with("Npc_Goron_") && n.ends_with("_Delete")))
    }

    fn openable(rf: &mut Romfs, out: &P, hash: u64, tag: &str) {
        let y: Byml = table(rf, out, SCENE);
        let acts: &[Node] = y.root.get("Actors").and_then(|a| a.as_arr()).unwrap();
        let chest: &Node = acts.iter().find(|a| a.get("Hash") == Some(&Node::U64(hash))).unwrap();
        assert_eq!(chest.get("Gyaml").and_then(|g| g.as_str()), Some("Obj_TBox_A_03"), "{tag}");
        let mut bound: usize = 0;
        for g in y.root.get("AiGroups").and_then(|a| a.as_arr()).unwrap() {
            let Some(logic) = g.get("Logic").and_then(|l| l.as_str()).filter(|l| l.contains("/FireTemple_Level_")) else {
                continue;
            };
            let a: Ainb = Ainb::parse(&file(rf, out, &module(logic))).unwrap();
            let gorons: bool = reads_goron(&a);
            for r in g.get("References").and_then(|r| r.as_arr()).unwrap_or(&[]) {
                let Some(id) = r.get("Id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if named(&a, TBOX, id).is_err() {
                    continue;
                }
                let mine: bool = r.get("Reference") == Some(&Node::U64(hash));
                if mine {
                    bound += 1;
                }
                if mine || gorons {
                    assert_eq!(constant(&a, TBOX, id, "Unlock"), Some(true), "{tag}: {logic} {id}");
                }
            }
        }
        assert_eq!(bound, 1, "{tag}");
        if let Some(e) = chest.get("Dynamic").and_then(|d| d.get("GetEventName")).and_then(|v| v.as_str()) {
            let t: Byml = table(rf, out, EVENTS);
            assert!(t.root.as_arr().unwrap().iter().any(|r| r.get("__RowId").and_then(|v| v.as_str()) == Some(e)), "{tag}: {e}");
        }
    }

    #[test]
    fn the_boss_key_chest_is_unlocked_in_full_builds() {
        let Some(mut rf) = romfs() else {
            return;
        };
        let d: Data = Data::load().unwrap();
        let hash: u64 = d.cat.locs.iter().find(|l| l.name == BOSS_KEY).unwrap().refs[0].hash;
        let named: [(&str, Value); 3] = [("shuffle_bosskeys", Value::s("vanilla")), (crate::chest_size::SETTING, Value::Bool(true)), (crate::debug_names::SETTING, Value::Bool(true))];
        let cases: [(&str, &[(&str, Value)]); 2] = [("fire boss key chest default", &[]), ("fire boss key chest named", &named)];
        for (seed, pairs) in cases {
            let root: PB = rf.root.clone();
            let out: PB = built(&d, &root, pairs, seed);
            openable(&mut rf, &out, hash, seed);
        }
    }
}
