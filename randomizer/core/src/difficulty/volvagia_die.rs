use crate::ainb::build::Spec;
use crate::ainb::{audit, root as graph_root, Ainb, Plug};
use crate::byml::{Byml, Node};
use crate::difficulty::backup::{feed, mark, LIFT, MAKE, MAT33, MAT34, PLACE, SPLIT, WAKE};
use crate::difficulty::hook::{both, flag, PARALLEL};
use crate::difficulty::spawn::OVER;
use crate::error::{Error as E, Res};

pub const DIE_MODULE: &str = "AI/Enemy_DungeonBoss_Volvagia.action.interruptdie.module.ainb";
pub const FORCE: &str = "OneShotDropSetForceDropPos";
pub const RESERVE: &str = "OneShotDropReserveDropItemCreate";
pub const PAY: &str = "OneshotIncreaseRupee";
pub const DISCONNECT: &str = "OneShotActorHolderDisconnectActorLink";
pub const PAY_VALUE: i32 = 50;
pub const RISE: f32 = 1.5;
pub const MODE: Mode = Mode::Spawn;
pub const SEED: &str = "ootrr volvagia roams die";
pub const ADDED_SPAWN: usize = 8;
pub const ADDED_DROP: usize = 5;
pub const ADDED_PAY: usize = 1;
pub const HOLDER_KEY: &str = "ActorHolderRef";
pub const HOLDER_OWN: &str = "Component/ActorHolderParam/Enemy_DungeonBoss_Volvagia.game__component__ActorHolderParam.bgyml";
pub const HOLDER_CLONE: &str = "Component/ActorHolderParam/Enemy_Volvagia_Roam.game__component__ActorHolderParam.bgyml";
pub const HOLDER_PEER: (&str, &str) = ("Npc_Hylia_Dampe_001", "Component/ActorHolderParam/Npc_Hylia_Dampe_001.game__component__ActorHolderParam.bgyml");
pub const HOLDER_PEER_GRAPH: &str = "AI/Npc_Hylia_Dampe_001.event.root.ainb";
pub const HELD: &str = "ActorHoldSettings";
pub const SIGN_KEY: &str = "AppearSign";
pub const RUPEE_KEY: &str = "RupeePurple";
pub const RUPEE: &str = "Obj_Rupee_Purple";
pub const RUPEE_VALUE: i32 = 50;
pub const UP_DROP: &str = "IsForceUpDrop";
pub const RECREATE: &str = "IsAutoRecreate";
pub const FAR_KEEP: u32 = 2_296_867_427;
pub const WITH_HOLDER: u32 = 3_707_532_803;
pub const HEAP_ROOM: i32 = 0x10000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Spawn,
    Drop,
    Pay,
}

impl Mode {
    pub fn added(&self) -> usize {
        match self {
            Mode::Spawn => ADDED_SPAWN,
            Mode::Drop => ADDED_DROP,
            Mode::Pay => ADDED_PAY,
        }
    }

    pub fn holds(&self) -> bool {
        *self == Mode::Spawn
    }
}

fn xyz(x: f32, y: f32, z: f32) -> Vec<u8> {
    [x, y, z].iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn key(a: &mut Ainb, k: &str) -> Vec<u8> {
    a.pool.add(k).to_le_bytes().to_vec()
}

fn plug_in(a: &mut Ainb, parent: usize, child: usize) -> Res<()> {
    let name: u32 = a.pool.add("");
    let at: usize = a.params.get(parent).ok_or(E::Format("volvagia roams: no such parent"))?.group(2).len();
    a.params[parent].add_plug(2, at, Plug { a: child as u32, b: name, extra: Vec::new() })
}

pub fn rupee_entry() -> Node {
    Node::Map(vec![
        ("Actor".to_string(), Node::Str(format!("Work/Actor/{RUPEE}.engine__actor__ActorParam.gyml"))),
        ("AvoidFarDeleteType".to_string(), Node::U32(FAR_KEEP)),
        ("DependencyWithHolder".to_string(), Node::U32(WITH_HOLDER)),
        (RECREATE.to_string(), Node::Bool(false)),
        ("IsEnableSleepDeleteWithHolder".to_string(), Node::Bool(true)),
    ])
}

pub fn holder(base: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    let held: &mut Node = y.root.get_mut(HELD).ok_or(E::Format("volvagia roams: the Volvagia holder has no entries, the game files differ"))?;
    if held.get(SIGN_KEY).is_none() {
        return Err(E::Msg(format!("volvagia roams: the Volvagia holder has no {SIGN_KEY}, the game files differ")));
    }
    if held.get(RUPEE_KEY).is_some() {
        return Err(E::Format("volvagia roams: the Volvagia holder already holds the rupee"));
    }
    held.set(RUPEE_KEY, rupee_entry());
    let out: Vec<u8> = y.write()?;
    if Byml::parse(&out)?.root.get(HELD).and_then(|h| h.get(RUPEE_KEY)) != Some(&rupee_entry()) {
        return Err(E::Format("volvagia roams: the holder does not read back"));
    }
    Ok(out)
}

fn position(a: &mut Ainb, s: &dyn Fn(&str) -> String) -> Res<usize> {
    let place: usize = a.add_node(&Spec::query(PLACE, &s("place")).out_ptr("Mat34F", MAT34))?;
    let split: usize = a.add_node(&Spec::query(SPLIT, &s("split")).linked_ptr("Matrix34f", MAT34, place).out(4, "Translation").spare_ptr("Rotation", MAT33))?;
    feed(a, place, split, "Mat34F")?;
    let lift: usize = a.add_node(&Spec::query(LIFT, &s("lift")).linked(4, "InputA", split, 0).constant(4, "InputB", xyz(0.0, RISE, 0.0)).out(4, "Result"))?;
    feed(a, split, lift, "Translation")?;
    Ok(lift)
}

pub fn spawn_nodes(a: &mut Ainb) -> Res<usize> {
    let s = |n: &str| -> String { format!("{SEED} rupee {n}") };
    let k: Vec<u8> = key(a, RUPEE_KEY);
    let lift: usize = position(a, &s)?;
    let make: usize = a.add_node(&Spec::query(MAKE, &s("make")).out_ptr("OverwriteParam", OVER))?;
    let up_key: Vec<u8> = key(a, UP_DROP);
    let up: usize = a.add_node(&mark(&s("up"), up_key, true, make))?;
    feed(a, make, up, "OverwriteParam")?;
    let wake: usize = a.add_node(
        &Spec::node(WAKE, &s("wake"))
            .prop(1, "ExecuteEveryFrame", flag(false))
            .prop(1, "IsForeachActor", flag(false))
            .idle(0, "Index", vec![0; 4])
            .constant(3, "Key", k.clone())
            .linked(4, "Pos", lift, 0)
            .idle(4, "Scale", xyz(1.0, 1.0, 1.0))
            .linked_ptr("OverwriteParam", OVER, up)
            .idle_ptr("Rot", MAT33),
    )?;
    feed(a, lift, wake, "Result")?;
    feed(a, up, wake, "OverwriteParam")?;
    let cut: usize = a.add_node(&Spec::node(DISCONNECT, &s("let go")).prop(1, "ExecuteEveryFrame", flag(false)).prop(1, "IsForeachActor", flag(false)).idle(0, "Index", vec![0; 4]).constant(3, "Key", k))?;
    a.add_node(&both(&s("drop"), wake, cut))
}

pub fn drop_nodes(a: &mut Ainb) -> Res<[usize; 2]> {
    let s = |n: &str| -> String { format!("{SEED} {n}") };
    let lift: usize = position(a, &s)?;
    let force: usize = a.add_node(&Spec::node(FORCE, &s("force drop pos")).prop(1, "ExecuteEveryFrame", flag(false)).prop(1, "IsForceAllPostureType", flag(true)).linked(4, "Pos", lift, 0))?;
    feed(a, lift, force, "Result")?;
    let reserve: usize = a.add_node(&Spec::node(RESERVE, &s("reserve")).prop(1, "ExecuteEveryFrame", flag(false)))?;
    Ok([force, reserve])
}

pub fn pay_node(a: &mut Ainb) -> Res<usize> {
    a.add_node(&Spec::node(PAY, &format!("{SEED} pay")).prop(1, "ExecuteEveryFrame", flag(false)).constant(0, "Value", PAY_VALUE.to_le_bytes().to_vec()))
}

pub fn die(raw: &[u8], m: Mode) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    let top: usize = graph_root(&a).ok_or(E::Format("volvagia roams: the Volvagia die module has no root"))?;
    let ours: usize = [FORCE, RESERVE, PAY, DISCONNECT, WAKE].iter().map(|n| a.find_els(n).len()).sum();
    if a.el_type(top) != PARALLEL || ours > 0 {
        return Err(E::Format("volvagia roams: the Volvagia die module is not the plain parallel this edit was written for"));
    }
    let kids: Vec<usize> = match m {
        Mode::Spawn => vec![spawn_nodes(&mut a)?],
        Mode::Drop => drop_nodes(&mut a)?.to_vec(),
        Mode::Pay => vec![pay_node(&mut a)?],
    };
    for k in kids {
        plug_in(&mut a, top, k)?;
    }
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(E::Msg(format!("volvagia roams: {}", bad.join("; "))));
    }
    let out: Vec<u8> = a.write()?;
    if Ainb::parse(&out)?.write()? != out {
        return Err(E::Format("volvagia roams: the Volvagia die module does not read back"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::romfs::Romfs;
    use crate::sarc::{self, Entry};
    use std::path::PathBuf as PB;

    fn rf() -> Option<Romfs> {
        let p: PB = ootrr_game::test_romfs::path();
        if !p.join("Pack/Actor").is_dir() {
            return None;
        }
        Romfs::open(&p).ok()
    }

    fn pack(rf: &mut Romfs, name: &str) -> Vec<Entry> {
        sarc::parse(&rf.read_zs(&format!("Pack/Actor/{name}.pack.zs")).unwrap()).unwrap()
    }

    fn base() -> Option<Vec<u8>> {
        let mut r: Romfs = rf()?;
        Some(sarc::get(&pack(&mut r, "Enemy_DungeonBoss_Volvagia"), DIE_MODULE).unwrap().data.clone())
    }

    fn names(a: &Ainb) -> Vec<String> {
        (0..a.els.len()).map(|i| a.el_name(i).to_string()).collect()
    }

    fn kids(a: &Ainb, i: usize) -> Vec<usize> {
        a.params[i].group(2).iter().map(|p| p.a as usize).collect()
    }

    fn shape(a: &Ainb, i: usize) -> Vec<String> {
        crate::ainb::show::show(a)
            .split("\nelement ")
            .find(|b| b.starts_with(&format!("{i} ")))
            .unwrap()
            .lines()
            .skip(1)
            .filter(|l| l.contains("prop ") || l.contains(" in ") || l.contains(" out "))
            .map(|l| l.split(" = ").next().unwrap().split(" from ").next().unwrap().split(" flags ").next().unwrap().trim().to_string())
            .collect()
    }

    fn values(a: &Ainb, i: usize) -> Vec<String> {
        crate::ainb::show::show(a)
            .split("\nelement ")
            .find(|b| b.starts_with(&format!("{i} ")))
            .unwrap()
            .lines()
            .skip(1)
            .filter(|l| l.contains("prop ") || l.contains(" in "))
            .map(|l| l.split(" from ").next().unwrap().trim().to_string())
            .collect()
    }

    fn graph(es: &[Entry], p: &str) -> Ainb {
        Ainb::parse(&sarc::get(es, p).unwrap().data).unwrap()
    }

    #[test]
    fn the_shipped_mode_leaves_the_rupee_and_only_it_holds_an_actor() {
        assert_eq!(MODE, Mode::Spawn);
        assert!(Mode::Spawn.holds() && !Mode::Pay.holds() && !Mode::Drop.holds());
        assert_eq!(RUPEE_VALUE, PAY_VALUE);
    }

    #[test]
    fn the_rupee_edit_wakes_and_lets_go_of_one_purple_rupee_at_her_position_from_the_top_parallel_and_nothing_else() {
        let Some(raw) = base() else { return };
        let old: Ainb = Ainb::parse(&raw).unwrap();
        let out: Vec<u8> = die(&raw, Mode::Spawn).unwrap();
        let a: Ainb = Ainb::parse(&out).unwrap();
        assert!(audit(&a).is_empty());
        assert_eq!(a.write().unwrap(), out);
        assert_eq!(a.els.len(), old.els.len() + ADDED_SPAWN);
        assert_eq!(names(&a)[..old.els.len()], names(&old)[..]);
        assert_eq!(names(&a)[old.els.len()..], [PLACE, SPLIT, LIFT, MAKE, crate::difficulty::backup::MARK, WAKE, DISCONNECT, ""].map(|s| s.to_string())[..]);
        let top: usize = graph_root(&a).unwrap();
        assert_eq!(top, graph_root(&old).unwrap());
        let was: Vec<usize> = kids(&old, top);
        let now: Vec<usize> = kids(&a, top);
        assert_eq!(now[..was.len()], was[..]);
        assert_eq!(now.len(), was.len() + 1);
        let pair: usize = *now.last().unwrap();
        assert_eq!(a.el_type(pair), PARALLEL);
        assert_eq!(kids(&a, pair).iter().map(|k| a.el_name(*k)).collect::<Vec<_>>(), vec![WAKE, DISCONNECT]);
        for i in 0..old.els.len() {
            if i != top {
                assert_eq!(a.params[i].plugs.len(), old.params[i].plugs.len(), "{i}");
            }
        }
        let w: usize = a.find_els(WAKE)[0];
        let d: usize = a.find_els(DISCONNECT)[0];
        assert_eq!(a.in_str(w, "Key"), Some(RUPEE_KEY));
        assert_eq!(a.in_str(d, "Key"), Some(RUPEE_KEY));
        let txt: String = crate::ainb::show::show(&a);
        assert!(txt.contains(&format!("in vec3 InputB = (0, {RISE}, 0)")), "{txt}");
        assert!(txt.contains(&format!("in vec3 Pos = (0, 0, 0) from {}.0", old.els.len() + 2)), "{txt}");
        assert!(txt.contains(&format!("prop str Key = \"{UP_DROP}\"")), "{txt}");
        for n in [FORCE, RESERVE, PAY, "ExecuteDeathSignalDeathProcedure"] {
            assert!(a.find_els(n).is_empty(), "{n}");
        }
        for m in [Mode::Spawn, Mode::Drop, Mode::Pay] {
            assert!(die(&out, m).is_err());
        }
    }

    #[test]
    fn the_rupee_nodes_have_the_fields_and_values_dampe_gives_his_purple_rupee() {
        let Some(mut r) = rf() else { return };
        let theirs: Ainb = graph(&pack(&mut r, HOLDER_PEER.0), HOLDER_PEER_GRAPH);
        let mine: Ainb = Ainb::parse(&die(&base().unwrap(), Mode::Spawn).unwrap()).unwrap();
        let tw: usize = theirs.find_els(WAKE).into_iter().find(|i| theirs.in_str(*i, "Key") == Some(RUPEE_KEY)).unwrap();
        let td: usize = theirs.find_els(DISCONNECT).into_iter().find(|i| theirs.in_str(*i, "Key") == Some(RUPEE_KEY)).unwrap();
        let mw: usize = mine.find_els(WAKE)[0];
        let md: usize = mine.find_els(DISCONNECT)[0];
        assert_eq!(shape(&mine, mw), shape(&theirs, tw));
        assert_eq!(values(&mine, md), values(&theirs, td));
        let wv: Vec<String> = values(&mine, mw);
        let tv: Vec<String> = values(&theirs, tw);
        assert_eq!(wv.iter().filter(|l| !l.contains("Pos")).collect::<Vec<_>>(), tv.iter().filter(|l| !l.contains("Pos")).collect::<Vec<_>>());
        for n in [MAKE, crate::difficulty::backup::MARK] {
            let t: usize = theirs.find_els(n)[0];
            let m: usize = mine.find_els(n)[0];
            assert_eq!(shape(&mine, m), shape(&theirs, t), "{n}");
        }
        let tm: usize = theirs.find_els(crate::difficulty::backup::MARK).into_iter().find(|i| values(&theirs, *i).iter().any(|l| l.contains(UP_DROP))).unwrap();
        let mm: usize = mine.find_els(crate::difficulty::backup::MARK)[0];
        assert_eq!(values(&mine, mm), values(&theirs, tm));
        let pair: usize = *kids(&mine, graph_root(&mine).unwrap()).last().unwrap();
        let theirs_pair: usize = (0..theirs.els.len()).find(|i| theirs.el_type(*i) == PARALLEL && kids(&theirs, *i) == vec![tw, td]).unwrap();
        assert_eq!(values(&mine, pair), values(&theirs, theirs_pair));
    }

    #[test]
    fn the_holder_adds_dampes_purple_rupee_once_and_keeps_the_appear_sign() {
        let Some(mut r) = rf() else { return };
        let v: Vec<Entry> = pack(&mut r, "Enemy_DungeonBoss_Volvagia");
        let raw: &[u8] = &sarc::get(&v, HOLDER_OWN).unwrap().data;
        let made: Byml = Byml::parse(&holder(raw).unwrap()).unwrap();
        let old: Byml = Byml::parse(raw).unwrap();
        let held: &Node = made.root.get(HELD).unwrap();
        assert_eq!(held.get(SIGN_KEY), old.root.get(HELD).unwrap().get(SIGN_KEY));
        let Node::Map(m) = held else { panic!() };
        assert_eq!(m.len(), 2);
        let d: Vec<Entry> = pack(&mut r, HOLDER_PEER.0);
        let dampe: Byml = Byml::parse(&sarc::get(&d, HOLDER_PEER.1).unwrap().data).unwrap();
        let mut theirs: Node = dampe.root.get(HELD).unwrap().get(RUPEE_KEY).unwrap().clone();
        assert_eq!(theirs.get(RECREATE), Some(&Node::Bool(true)));
        theirs.set(RECREATE, Node::Bool(false));
        assert_eq!(held.get(RUPEE_KEY), Some(&theirs));
        assert!(r.root.join(format!("Pack/Actor/{RUPEE}.pack.zs")).is_file());
        assert!(holder(&made.write().unwrap()).is_err());
    }

    #[test]
    fn the_drop_edit_adds_a_forced_drop_spot_at_her_position_and_a_reserve_to_the_top_parallel_and_nothing_else() {
        let Some(raw) = base() else { return };
        let old: Ainb = Ainb::parse(&raw).unwrap();
        let out: Vec<u8> = die(&raw, Mode::Drop).unwrap();
        let a: Ainb = Ainb::parse(&out).unwrap();
        assert!(audit(&a).is_empty());
        assert_eq!(a.write().unwrap(), out);
        assert_eq!(a.els.len(), old.els.len() + ADDED_DROP);
        assert_eq!(names(&a)[old.els.len()..], [PLACE, SPLIT, LIFT, FORCE, RESERVE].map(|s| s.to_string())[..]);
        let top: usize = graph_root(&a).unwrap();
        let was: Vec<usize> = kids(&old, top);
        let now: Vec<usize> = kids(&a, top);
        assert_eq!(now[..was.len()], was[..]);
        assert_eq!(now[was.len()..].iter().map(|k| a.el_name(*k)).collect::<Vec<_>>(), vec![FORCE, RESERVE]);
        let txt: String = crate::ainb::show::show(&a);
        assert!(txt.contains("prop bool IsForceAllPostureType = true"), "{txt}");
        assert!(txt.contains(&format!("in vec3 Pos = (0, 0, 0) from {}.0", old.els.len() + 2)), "{txt}");
        assert!(die(&out, Mode::Drop).is_err());
    }

    #[test]
    fn the_pay_edit_adds_one_fifty_rupee_payment_to_the_top_parallel() {
        let Some(raw) = base() else { return };
        let old: Ainb = Ainb::parse(&raw).unwrap();
        let out: Vec<u8> = die(&raw, Mode::Pay).unwrap();
        let a: Ainb = Ainb::parse(&out).unwrap();
        assert!(audit(&a).is_empty());
        assert_eq!(a.els.len(), old.els.len() + ADDED_PAY);
        let top: usize = graph_root(&a).unwrap();
        assert_eq!(kids(&a, top).last().map(|k| a.el_name(*k)), Some(PAY));
        assert!(a.find_els(FORCE).is_empty() && a.find_els(RESERVE).is_empty() && a.find_els(WAKE).is_empty());
        let txt: String = crate::ainb::show::show(&a);
        assert!(txt.contains(&format!("in s32 Value = {PAY_VALUE}")), "{txt}");
    }

    #[test]
    fn the_drop_nodes_have_the_fields_the_game_gives_them() {
        let Some(mut r) = rf() else { return };
        let glob: Vec<Entry> = sarc::parse(&r.read_zs("Pack/AI.Global.Product.111.pack.zs").unwrap()).unwrap();
        let theirs: Ainb = graph(&glob, "AI/SetupForceDropPos.module.ainb");
        let ll: Ainb = graph(&pack(&mut r, "Enemy_LikeLike"), "AI/Enemy_LikeLike.action.die.module.ainb");
        let mine: Ainb = Ainb::parse(&die(&base().unwrap(), Mode::Drop).unwrap()).unwrap();
        assert_eq!(shape(&mine, mine.find_els(FORCE)[0]), shape(&theirs, theirs.find_els(FORCE)[0]));
        assert_eq!(shape(&mine, mine.find_els(RESERVE)[0]), shape(&ll, ll.find_els(RESERVE)[0]));
    }
}
