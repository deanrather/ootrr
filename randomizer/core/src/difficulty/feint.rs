pub mod graph;

use std::collections::BTreeSet as Set;

use crate::difficulty::hook;
use crate::error::{Error, Res};
use crate::qol::{edit_zs, Have};
use crate::romfs::Romfs;
use crate::sarc::{self, Entry};
use crate::staging::Out;
use crate::zs;

pub use graph::{patch, taken, Hold, Shape};

pub const SETTING: &str = "enemy_feints";
pub const NUM: u16 = 166;
pub const HEAP_EXTRA: i32 = 0x8000;
pub const LABEL: &str = "enemy feints";
pub const FREEZE_VAR: &str = "OOTRR_FEINT_FREEZE";
pub const SINGLE_HOLD: [f32; 1] = [28.0];
pub const TRIPLE_HOLDS: [f32; 2] = [28.0, 70.0];
pub const JUMP_HOLD: [f32; 1] = [12.0];
pub const LIZALFOS_HOLD: [f32; 1] = [28.0];
pub const DINOLFOS_HOLD: [f32; 1] = [26.0];
pub const KNUCKLE_HOLD: [f32; 1] = [20.0];
pub const KNUCKLE_GAP_HOLD: f32 = 10.0;
pub const BABA_HOLD: [f32; 1] = [10.0];

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub kind: &'static str,
    pub graph: &'static str,
    pub shape: Shape,
}

pub const TARGETS: [Target; 8] = [
    Target { kind: "Stalfos swing", graph: "AI/Stalfos.action.BasicAttack.module.ainb", shape: Shape { els: 27, state: 9, body: 0, holds: &SINGLE_HOLD, gap: None } },
    Target { kind: "Stalfos combo", graph: "AI/Stalfos.action.ConsecutiveAttack.module.ainb", shape: Shape { els: 48, state: 11, body: 1, holds: &TRIPLE_HOLDS, gap: None } },
    Target { kind: "Stalfos jump", graph: "AI/Stalfos.action.JumpAttack.module.ainb", shape: Shape { els: 145, state: 16, body: 69, holds: &JUMP_HOLD, gap: None } },
    Target { kind: "Lizalfos swing", graph: "AI/Lizalfos.action.BasicAttack.module.ainb", shape: Shape { els: 26, state: 12, body: 1, holds: &LIZALFOS_HOLD, gap: None } },
    Target { kind: "Dinolfos swing", graph: "AI/Dinolfos.action.BasicAttack.module.ainb", shape: Shape { els: 28, state: 15, body: 4, holds: &DINOLFOS_HOLD, gap: None } },
    Target { kind: "Iron Knuckle overhead", graph: "AI/Enemy_IronKnuckle.action.LargeAttack.module.ainb", shape: Shape { els: 40, state: 17, body: 0, holds: &KNUCKLE_HOLD, gap: None } },
    Target { kind: "Iron Knuckle double", graph: "AI/Enemy_IronKnuckle.action.BasicAttack.module.ainb", shape: Shape { els: 26, state: 9, body: 0, holds: &KNUCKLE_HOLD, gap: Some((10, 1, KNUCKLE_GAP_HOLD)) } },
    Target { kind: "Deku Baba bite", graph: "AI/Dekubaba.action.basicattack.module.ainb", shape: Shape { els: 78, state: 28, body: 0, holds: &BABA_HOLD, gap: None } },
];

pub fn percent(choice: &str) -> Option<i64> {
    match choice {
        "quarter" => Some(25),
        "half" => Some(50),
        "three_quarters" => Some(75),
        _ => None,
    }
}

pub fn note(pct: i64) -> Option<String> {
    (pct > 0).then(|| format!("About {pct} in 100 attacks by Stalfos, Lizalfos, Dinolfos, Iron Knuckles and Deku Babas are feints: the enemy winds up, then sways back and forth around its ready pose for about two thirds of a second to a second and a third, then strikes. Stalfos combos can hold before the second swing instead, and the Iron Knuckle can also hold before its second swing. The whole body sways, legs included. Experimental."))
}

pub fn hold_mode() -> Hold {
    match std::env::var(FREEZE_VAR) {
        Ok(v) if v.trim() == "1" => Hold::Freeze,
        _ => Hold::Sway,
    }
}

pub fn module_of(graph: &str) -> &str {
    graph.strip_prefix("AI/").and_then(|g| g.strip_suffix(".ainb")).unwrap_or(graph)
}

pub fn target_of(name: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|t| t.graph == name)
}

pub fn callers(es: &[Entry]) -> Set<String> {
    es.iter().filter(|e| e.name.ends_with(".ainb") && target_of(&e.name).is_none() && TARGETS.iter().any(|t| hook::has(&e.data, module_of(t.graph)))).map(|e| e.name.clone()).collect()
}

pub fn runs(es: &[Entry], shared: &Set<String>) -> bool {
    es.iter().any(|e| target_of(&e.name).is_some()) || !callers(es).is_empty() || !hook::shared_of(es, shared).is_empty()
}

fn repack(path: &str, z: &[u8], old: usize, es: &[Entry]) -> Res<Out> {
    let out: Vec<u8> = sarc::write(es)?;
    Ok(Out { path: path.to_string(), old, new: out.len(), z: zs::compress_raw(&out, zs::dict_of(z))? })
}

fn patch_pack(es: &mut [Entry], p: f32, how: Hold, hit: &mut Set<&'static str>) -> Res<bool> {
    let mut any: bool = false;
    for e in es.iter_mut() {
        let Some(t) = target_of(&e.name) else {
            continue;
        };
        e.data = patch(&e.data, &t.shape, p, how, t.kind)?;
        hit.insert(t.graph);
        any = true;
    }
    Ok(any)
}

pub fn stage(rf: &mut Romfs, have: Have, dir: &str, pct: i64) -> Res<(Set<String>, Vec<Out>)> {
    let p: f32 = pct as f32 / 100.0;
    let how: Hold = hold_mode();
    let mut hit: Set<&'static str> = Set::new();
    let (z, old, mut es): (Vec<u8>, usize, Vec<Entry>) = hook::opened(rf, have, hook::GLOBAL)?;
    let shared: Set<String> = callers(&es);
    let mut outs: Vec<Out> = Vec::new();
    if patch_pack(&mut es, p, how, &mut hit)? {
        outs.push(repack(hook::GLOBAL, &z, old, &es)?);
    }
    let mut who: Set<String> = Set::new();
    for f in rf.list(dir)? {
        let Some(name) = hook::actor_of(&f).map(str::to_string) else {
            continue;
        };
        let (z, old, mut es): (Vec<u8>, usize, Vec<Entry>) = hook::opened(rf, have, &f)?;
        if !runs(&es, &shared) {
            continue;
        }
        who.insert(name);
        if patch_pack(&mut es, p, how, &mut hit)? {
            outs.push(repack(&f, &z, old, &es)?);
        }
    }
    if let Some(t) = TARGETS.iter().find(|t| !hit.contains(t.graph)) {
        return Err(Error::Msg(format!("{LABEL}: no pack holds {}", t.graph)));
    }
    if who.is_empty() {
        return Err(Error::Msg(format!("{LABEL}: no actor runs an attack module")));
    }
    outs.push(edit_zs(rf, have, hook::ACTOR_INFO, |raw| hook::grow_heap(raw, &who, HEAP_EXTRA, LABEL))?);
    Ok((who, outs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ainb::Ainb;
    use crate::byml::{Byml, Node};
    use crate::pickup_pack::rows;

    fn dump() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from)?;
        if p.is_dir() { Some(p) } else { None }
    }

    fn none(_p: &str) -> Option<Vec<u8>> {
        None
    }

    #[test]
    fn the_levels_read_and_default_off() {
        assert_eq!((percent("off"), percent("quarter"), percent("half"), percent("three_quarters"), percent("x")), (None, Some(25), Some(50), Some(75), None));
        assert!(note(0).is_none());
        assert!(note(75).unwrap().contains("75 in 100"));
        use crate::settings::{Schema, Settings, Value};
        let sc: Schema = crate::settings::list::build();
        let s: &crate::settings::Setting = sc.settings.iter().find(|s| s.id == SETTING).unwrap();
        let ids: Vec<&str> = s.choices.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["off", "quarter", "half", "three_quarters"], "the new level is last, so old strings keep their meaning");
        let mut bad: Vec<String> = Vec::new();
        let o: crate::difficulty::Opts = crate::difficulty::read(&Settings::default(), &mut bad);
        assert!(bad.is_empty() && o.feints == 0 && !o.any());
        for (c, n) in [("half", 50), ("three_quarters", 75)] {
            let mut s: Settings = Settings::default();
            s.set(SETTING, Value::s(c)).unwrap();
            let back: Settings = Settings::decode(&s.encode().unwrap()).unwrap().settings;
            let o: crate::difficulty::Opts = crate::difficulty::read(&back, &mut bad);
            assert!(bad.is_empty() && o.feints == n && o.any() && !o.packs());
            assert_eq!(o.notes().len(), 1);
        }
        let mut s: Settings = Settings::default();
        s.set(SETTING, Value::s("off")).unwrap();
        assert_eq!(s.encode().unwrap(), Settings::default().encode().unwrap());
    }

    #[test]
    fn module_names_come_from_the_graph_path() {
        assert_eq!(module_of("AI/Stalfos.action.BasicAttack.module.ainb"), "Stalfos.action.BasicAttack.module");
        assert!(target_of("AI/Nothing.ainb").is_none());
        let mut seen: Set<&str> = Set::new();
        assert!(TARGETS.iter().all(|t| seen.insert(t.graph)), "each graph is listed once");
    }

    #[test]
    fn every_enemy_actor_gets_room_and_only_the_attack_modules_change() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let (who, outs): (Set<String>, Vec<Out>) = stage(&mut rf, &none, "Pack/Actor", 25).unwrap();
        for x in ["Enemy_Stalfos", "Enemy_Stalfos_AlwaysRespawn", "Enemy_Stalfos_ForChild", "Enemy_Stalfos_Weak", "Enemy_Lizalfos", "Enemy_MiddleBoss_Lizalfos", "Enemy_Dinolfos", "Enemy_IronKnuckle_Black", "Enemy_IronKnuckle_Silver", "Enemy_IronKnuckle_White", "Enemy_MiddleBoss_IronKnuckle_Nabooru", "Enemy_Dekubaba", "Enemy_Dekubaba_Withered"] {
            assert!(who.contains(x), "{x} runs an attack module: {who:?}");
        }
        for x in ["Player_Child", "Enemy_Keese", "Enemy_Moblin_Club", "Enemy_Wolfos", "Enemy_Octorok"] {
            assert!(!who.contains(x), "{x} runs no edited module");
        }
        let paths: Vec<&str> = outs.iter().map(|x| x.path.as_str()).collect();
        assert_eq!((paths[0], paths[paths.len() - 1]), (hook::GLOBAL, hook::ACTOR_INFO), "{paths:?}");
        let mut t: crate::rsizetable::Rst = crate::rsizetable::load(&mut rf).unwrap();
        let before: Vec<(&str, Option<u32>)> = TARGETS.iter().map(|x| (x.graph, t.get(x.graph))).collect();
        let mut graphs: Set<String> = Set::new();
        for o in &outs[..outs.len() - 1] {
            assert_eq!(zs::dict_of(&o.z), zs::dict_of(&rf.read(&o.path).unwrap()), "{}", o.path);
            assert!(o.new > o.old, "{}", o.path);
            let now: Vec<Entry> = sarc::parse(&rf.zs.decompress(&o.z).unwrap()).unwrap();
            let was: Vec<Entry> = sarc::parse(&rf.read_zs(&o.path).unwrap()).unwrap();
            assert_eq!(now.len(), was.len());
            for (x, y) in now.iter().zip(was.iter()) {
                assert_eq!(x.name, y.name);
                if x.data != y.data {
                    assert!(target_of(&x.name).is_some(), "{}: only attack modules change, not {}", o.path, x.name);
                    assert!(taken(&Ainb::parse(&x.data).unwrap()));
                    graphs.insert(x.name.clone());
                }
            }
        }
        assert_eq!(graphs.len(), TARGETS.len(), "every attack module is edited: {graphs:?}");
        assert_eq!(outs.len(), 1 + 4 + 1 + 4 + 2 + 1, "{paths:?}");
        let table: Byml = Byml::parse(&rf.zs.decompress(&outs[outs.len() - 1].z).unwrap()).unwrap();
        let old: Byml = rf.read_byml(hook::ACTOR_INFO).unwrap();
        let heap = |y: &Byml, n: &str| -> Option<Node> { rows::find(y, n)?.get(rows::HEAP).cloned() };
        for n in ["Enemy_Stalfos", "Enemy_Dinolfos", "Enemy_IronKnuckle_Black", "Enemy_Dekubaba"] {
            let Some(Node::I32(h)) = heap(&old, n) else { panic!("{n}") };
            assert_eq!(heap(&table, n), Some(Node::I32(h + HEAP_EXTRA)), "{n}");
        }
        for o in &outs {
            crate::staging::grow_out(&mut rf, &mut t, o).unwrap();
        }
        for (g, was) in before {
            assert!(was.is_some() && t.get(g) > was, "{g}: its own size table entry grew");
        }
    }

    #[test]
    fn feints_stack_on_regen_in_the_shared_pack() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let o: crate::difficulty::Opts = crate::difficulty::Opts { regen: true, feints: 50, ..crate::difficulty::Opts::default() };
        let outs: Vec<Out> = crate::difficulty::stage(&mut rf, &o, &none, "t").unwrap();
        let g: &Out = outs.iter().find(|x| x.path == hook::GLOBAL).unwrap();
        let es: Vec<Entry> = sarc::parse(&rf.zs.decompress(&g.z).unwrap()).unwrap();
        assert!(taken(&Ainb::parse(&sarc::get(&es, TARGETS[0].graph).unwrap().data).unwrap()));
        let base: Ainb = Ainb::parse(&sarc::get(&es, "AI/EnemyBase.utility.root.ainb").unwrap().data).unwrap();
        assert!(!base.find_els(crate::difficulty::regen::HEAL).is_empty(), "regen is kept");
        let table: Byml = Byml::parse(&rf.zs.decompress(&outs.iter().find(|x| x.path == hook::ACTOR_INFO).unwrap().z).unwrap()).unwrap();
        let before: Byml = rf.read_byml(hook::ACTOR_INFO).unwrap();
        let h = |y: &Byml| -> Option<Node> { rows::find(y, "Enemy_Stalfos")?.get(rows::HEAP).cloned() };
        let Some(Node::I32(v)) = h(&before) else { panic!() };
        assert_eq!(h(&table), Some(Node::I32(v + HEAP_EXTRA + crate::difficulty::regen::HEAP_EXTRA)));
    }
}
