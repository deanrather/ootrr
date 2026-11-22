use crate::ainb::build::Spec;
use crate::ainb::{audit, root as graph_root, Ainb, Plug};
use crate::difficulty::backup::{feed, LIFT, MAT33, MAT34, PLACE, SPLIT};
use crate::difficulty::hook::{flag, PARALLEL};
use crate::error::{Error as E, Res};

pub const DIE_MODULE: &str = "AI/Enemy_DungeonBoss_Volvagia.action.interruptdie.module.ainb";
pub const FORCE: &str = "OneShotDropSetForceDropPos";
pub const RESERVE: &str = "OneShotDropReserveDropItemCreate";
pub const PAY: &str = "OneshotIncreaseRupee";
pub const PAY_VALUE: i32 = 50;
pub const RISE: f32 = 1.5;
pub const MODE_VAR: &str = "OOTRR_VOLVAGIA_DROP";
pub const SEED: &str = "ootrr volvagia roams die";
pub const ADDED_DROP: usize = 5;
pub const ADDED_PAY: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Drop,
    Pay,
}

impl Mode {
    pub fn read(v: Option<&str>) -> Mode {
        match v.map(|s| s.trim()) {
            Some("pay") => Mode::Pay,
            _ => Mode::Drop,
        }
    }

    pub fn added(&self) -> usize {
        match self {
            Mode::Drop => ADDED_DROP,
            Mode::Pay => ADDED_PAY,
        }
    }
}

pub fn mode() -> Mode {
    Mode::read(std::env::var(MODE_VAR).ok().as_deref())
}

fn xyz(x: f32, y: f32, z: f32) -> Vec<u8> {
    [x, y, z].iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn plug_in(a: &mut Ainb, parent: usize, child: usize) -> Res<()> {
    let name: u32 = a.pool.add("");
    let at: usize = a.params.get(parent).ok_or(E::Format("volvagia roams: no such parent"))?.group(2).len();
    a.params[parent].add_plug(2, at, Plug { a: child as u32, b: name, extra: Vec::new() })
}

pub fn drop_nodes(a: &mut Ainb) -> Res<[usize; 2]> {
    let s = |n: &str| -> String { format!("{SEED} {n}") };
    let place: usize = a.add_node(&Spec::query(PLACE, &s("place")).out_ptr("Mat34F", MAT34))?;
    let split: usize = a.add_node(&Spec::query(SPLIT, &s("split")).linked_ptr("Matrix34f", MAT34, place).out(4, "Translation").spare_ptr("Rotation", MAT33))?;
    feed(a, place, split, "Mat34F")?;
    let lift: usize = a.add_node(&Spec::query(LIFT, &s("lift")).linked(4, "InputA", split, 0).constant(4, "InputB", xyz(0.0, RISE, 0.0)).out(4, "Result"))?;
    feed(a, split, lift, "Translation")?;
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
    if a.el_type(top) != PARALLEL || a.find_els(FORCE).len() + a.find_els(RESERVE).len() + a.find_els(PAY).len() > 0 {
        return Err(E::Format("volvagia roams: the Volvagia die module is not the plain parallel this edit was written for"));
    }
    let kids: Vec<usize> = match m {
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

    fn base() -> Option<Vec<u8>> {
        let p: PB = ootrr_game::test_romfs::path();
        if !p.join("Pack/Actor").is_dir() {
            return None;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let es: Vec<Entry> = sarc::parse(&rf.read_zs("Pack/Actor/Enemy_DungeonBoss_Volvagia.pack.zs").unwrap()).unwrap();
        Some(sarc::get(&es, DIE_MODULE).unwrap().data.clone())
    }

    fn names(a: &Ainb) -> Vec<String> {
        (0..a.els.len()).map(|i| a.el_name(i).to_string()).collect()
    }

    fn kids(a: &Ainb, i: usize) -> Vec<usize> {
        a.params[i].group(2).iter().map(|p| p.a as usize).collect()
    }

    #[test]
    fn the_switch_picks_the_drop_unless_it_says_pay() {
        assert_eq!(Mode::read(None), Mode::Drop);
        assert_eq!(Mode::read(Some("")), Mode::Drop);
        assert_eq!(Mode::read(Some("drop")), Mode::Drop);
        assert_eq!(Mode::read(Some(" pay ")), Mode::Pay);
        assert_eq!(Mode::read(Some("PAY")), Mode::Drop);
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
        assert_eq!(names(&a)[..old.els.len()], names(&old)[..]);
        assert_eq!(names(&a)[old.els.len()..], [PLACE, SPLIT, LIFT, FORCE, RESERVE].map(|s| s.to_string())[..]);
        let top: usize = graph_root(&a).unwrap();
        assert_eq!(top, graph_root(&old).unwrap());
        let was: Vec<usize> = kids(&old, top);
        let now: Vec<usize> = kids(&a, top);
        assert_eq!(now[..was.len()], was[..]);
        assert_eq!(now[was.len()..].iter().map(|k| a.el_name(*k)).collect::<Vec<_>>(), vec![FORCE, RESERVE]);
        for i in 0..old.els.len() {
            if i != top {
                assert_eq!(a.params[i].plugs.len(), old.params[i].plugs.len(), "{i}");
            }
        }
        assert!(a.find_els(PAY).is_empty());
        let txt: String = crate::ainb::show::show(&a);
        assert!(txt.contains("prop bool IsForceAllPostureType = true"), "{txt}");
        assert!(txt.contains(&format!("in vec3 InputB = (0, {RISE}, 0)")), "{txt}");
        assert!(txt.contains(&format!("in vec3 Pos = (0, 0, 0) from {}.0", old.els.len() + 2)), "{txt}");
        assert!(!txt.contains("ExecuteDeathSignalDeathProcedure"));
        assert!(die(&out, Mode::Drop).is_err());
        assert!(die(&out, Mode::Pay).is_err());
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
        assert!(a.find_els(FORCE).is_empty() && a.find_els(RESERVE).is_empty());
        let txt: String = crate::ainb::show::show(&a);
        assert!(txt.contains(&format!("in s32 Value = {PAY_VALUE}")), "{txt}");
    }

    #[test]
    fn the_added_nodes_have_the_fields_the_game_gives_them() {
        let p: PB = ootrr_game::test_romfs::path();
        if !p.join("Pack/Actor").is_dir() {
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let glob: Vec<Entry> = sarc::parse(&rf.read_zs("Pack/AI.Global.Product.111.pack.zs").unwrap()).unwrap();
        let theirs: Ainb = Ainb::parse(&sarc::get(&glob, "AI/SetupForceDropPos.module.ainb").unwrap().data).unwrap();
        let like: Vec<Entry> = sarc::parse(&rf.read_zs("Pack/Actor/Enemy_LikeLike.pack.zs").unwrap()).unwrap();
        let ll: Ainb = Ainb::parse(&sarc::get(&like, "AI/Enemy_LikeLike.action.die.module.ainb").unwrap().data).unwrap();
        let raw: Vec<u8> = base().unwrap();
        let mine: Ainb = Ainb::parse(&die(&raw, Mode::Drop).unwrap()).unwrap();
        let shape = |a: &Ainb, name: &str| -> Vec<String> {
            let i: usize = a.find_els(name)[0];
            crate::ainb::show::show(a).split("\nelement ").find(|b| b.starts_with(&format!("{i} "))).unwrap().lines().skip(1).filter(|l| l.contains("prop ") || l.contains(" in ")).map(|l| l.split(" = ").next().unwrap().trim().to_string()).collect()
        };
        assert_eq!(shape(&mine, FORCE), shape(&theirs, FORCE));
        assert_eq!(shape(&mine, RESERVE), shape(&ll, RESERVE));
    }
}
