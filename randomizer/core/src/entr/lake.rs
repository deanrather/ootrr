use std::path::Path as P;

use crate::ainb::Ainb;
use crate::entr::layout::Layout;
use crate::entr::pools::find;
use crate::door_open::multi_entry;
use crate::error::{Error as E, Res};
use crate::romfs::Romfs;
use crate::staging::Out;

pub const MODULE: &str = "Logic/HyruleField_LakeHylia_Common_Prefab_6a9d.logic.module.ainb";
pub const WATER: &str = "Water Temple";
pub const FLAG: &str = "HavePlayedEvent.Dm_WaterTemple_0000";
pub const QUEST: &str = "Work/Challenge/SageOfWater.game__challenge__Challenge.gyml";
pub const STEP: &str = "Step05";
pub const NOTE: &str = "The Water Temple is not behind the Lake Hylia gate, so the gate opens only from its own switch.";

const BOOL: usize = 1;
const SET: &str = "TriggerGameDataSetBoolTrue";
const CHECK: &str = "HoldChallengeCurrentStepCheckOrder";
const DRIVE: &str = "IsDrive";
const GATE: &str = "ActorLogicGateDoor";
const SHUTTER: &str = "Obj_LakeHrl_SwitchShutter_A_01";
const READ: &str = "HoldGameDataGetBoolSimple";

pub fn moved(l: &Layout) -> Res<bool> {
    l.check()?;
    let w: usize = find(WATER).ok_or(E::Format("lake gate: the Water Temple is not in the dungeon pool"))?;
    Ok(l.at[w] != w)
}

fn is_step(a: &Ainb, j: usize) -> bool {
    a.find_els(CHECK).contains(&j) && a.in_str(j, "ChallengeName") == Some(QUEST) && a.in_str(j, "ChallengeStepName") == Some(STEP)
}

pub fn triggers(a: &Ainb) -> Vec<usize> {
    a.find_els(SET)
        .into_iter()
        .filter(|&i| a.in_str(i, "GameDataNameBool") == Some(FLAG))
        .filter(|&i| a.find_in(i, BOOL, DRIVE).and_then(|g| a.input(BOOL, g)).is_some_and(|x| x.src >= 0 && is_step(a, x.src as usize)))
        .collect()
}

pub fn multis(a: &Ainb, i: usize) -> Vec<usize> {
    let mut v: Vec<usize> = Vec::new();
    for g in a.input_ids(i, BOOL) {
        if let Some(x) = a.input(BOOL, g).filter(|x| x.src <= -100) {
            let b: usize = (-100 - x.src as i32) as usize;
            v.extend(b..b + x.plug.max(0) as usize);
        }
    }
    v
}

pub fn gate_or(a: &Ainb) -> Option<usize> {
    let doors: Vec<usize> = a.find_els(GATE).into_iter().filter(|&d| a.prop_str(d, "InstanceName").is_some_and(|n| n.starts_with(SHUTTER))).collect();
    let [d] = doors[..] else { return None };
    let src = |n: &str| a.find_in(d, BOOL, n).and_then(|g| a.input(BOOL, g)).map(|x| x.src).unwrap_or(-1);
    let (o, c): (i16, i16) = (src("isOpen"), src("isClose"));
    if o < 0 || o != c {
        return None;
    }
    Some(o as usize)
}

pub fn flag_read(a: &Ainb) -> Option<usize> {
    let r: Vec<usize> = a.find_els(READ).into_iter().filter(|&j| a.in_str(j, "GameDataNameBool") == Some(FLAG)).collect();
    let [j] = r[..] else { return None };
    Some(j)
}

pub fn gate_steps(a: &Ainb) -> Vec<usize> {
    let Some(o) = gate_or(a) else { return Vec::new() };
    multis(a, o).into_iter().filter(|&j| multi_entry(a, j).is_some_and(|(n, _)| is_step(a, n as usize))).collect()
}

pub fn cut(a: &mut Ainb) -> Res<usize> {
    let hits: Vec<usize> = triggers(a);
    let [i] = hits[..] else {
        return Err(E::Msg(format!("lake gate: expected one {FLAG} write driven by the Water Sage {STEP} check, found {}, the module is not the game's", hits.len())));
    };
    let o: usize = gate_or(a).ok_or(E::Format("lake gate: the shutter is not opened by one OR element"))?;
    let f: usize = flag_read(a).ok_or(E::Format("lake gate: the module does not read the gate flag once"))?;
    let steps: Vec<usize> = gate_steps(a);
    let flags: Vec<usize> = multis(a, o).into_iter().filter(|&j| multi_entry(a, j).is_some_and(|(n, _)| n as usize == f)).collect();
    let ([s], [p]) = (&steps[..], &flags[..]) else {
        return Err(E::Format("lake gate: the shutter does not open on the gate flag or the Water Sage step, the module is not the game's"));
    };
    if !a.force_bool(i, DRIVE, false)? {
        return Err(E::Format("lake gate: the flag write had no link to cut"));
    }
    let from: [u8; 4] = a.multi.get(8 * p..8 * p + 4).and_then(|b| b.try_into().ok()).ok_or(E::Format("lake gate: bad multi entry"))?;
    a.multi[8 * s..8 * s + 4].copy_from_slice(&from);
    Ok(i)
}

pub fn patch(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    cut(&mut a)?;
    let out: Vec<u8> = a.write()?;
    if Ainb::parse(&out)?.write()? != out {
        return Err(E::Format("lake gate: the edited logic module does not read back as written"));
    }
    Ok(out)
}

pub fn stage(rf: &Romfs, l: &Layout, late: &mut Vec<Out>, work: &P) -> Res<bool> {
    if !moved(l)? {
        return Ok(false);
    }
    if late.iter().any(|o| o.path == MODULE) {
        return Err(E::Msg(format!("lake gate: {MODULE} is already edited in the late files")));
    }
    let staged: std::path::PathBuf = MODULE.split('/').fold(work.to_path_buf(), |a, s| a.join(s));
    let raw: Vec<u8> = match std::fs::read(&staged) {
        Ok(b) => b,
        Err(_) => rf.read(MODULE)?,
    };
    let out: Vec<u8> = patch(&raw)?;
    late.push(Out { path: MODULE.to_string(), old: raw.len(), new: out.len(), z: out });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ainb::audit;
    use crate::byml::{Byml, Node};
    use crate::entr::pools::DUNGEONS;
    use std::path::PathBuf as PB;

    fn real() -> Option<Romfs> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(Romfs::open(&p).unwrap())
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn swap(a: &str, b: &str) -> Layout {
        let mut l: Layout = Layout::vanilla();
        l.at.swap(find(a).unwrap(), find(b).unwrap());
        l
    }

    type Link = (usize, usize, String, i16, Vec<u8>);

    fn links(a: &Ainb) -> Vec<Link> {
        let mut v: Vec<Link> = Vec::new();
        for i in 0..a.els.len() {
            for k in 0..6 {
                for g in a.input_ids(i, k) {
                    let x = a.input(k, g).unwrap();
                    v.push((i, k, a.s(x.name).to_string(), x.src, x.val));
                }
            }
        }
        v
    }

    fn gate_reads(a: &Ainb) -> Vec<(u16, u16)> {
        let o: usize = gate_or(a).unwrap();
        let f: i16 = a.find_in(o, BOOL, "BoolMulti").and_then(|g| a.input(BOOL, g)).map(|x| x.src).unwrap();
        assert!(f <= -100);
        multis(a, o).into_iter().map(|j| multi_entry(a, j).unwrap()).collect()
    }

    fn opens_on_flag(a: &Ainb) -> bool {
        let f: Option<usize> = flag_read(a);
        f.is_some() && gate_reads(a).iter().any(|(n, p)| Some(*n as usize) == f && *p == 0)
    }

    fn readers_of(a: &Ainb, e: usize) -> usize {
        let mut n: usize = 0;
        for i in 0..a.els.len() {
            for k in 0..6 {
                n += a.input_ids(i, k).into_iter().filter(|&g| a.input(k, g).is_some_and(|x| x.src == e as i16)).count();
            }
            n += multis(a, i).into_iter().filter(|&j| multi_entry(a, j).is_some_and(|(m, _)| m as usize == e)).count();
        }
        n
    }

    fn steps_el(a: &Ainb) -> usize {
        let v: Vec<usize> = (0..a.els.len()).filter(|&j| is_step(a, j)).collect();
        assert_eq!(v.len(), 1);
        v[0]
    }

    fn named(a: &Ainb, kind: &str, inst: &str) -> usize {
        let v: Vec<usize> = a.find_els(kind).into_iter().filter(|&j| a.prop_str(j, "InstanceName").is_some_and(|n| n.starts_with(inst))).collect();
        assert_eq!(v.len(), 1, "{kind} {inst}");
        v[0]
    }

    fn scene_request(a: &Ainb) -> (usize, Vec<(u16, u16)>) {
        let r: usize = named(a, "ActorLogicEventRequestActor", "EventRequestActor_4eef");
        let x = a.input(BOOL, a.find_in(r, BOOL, "EventRequestTrigger").unwrap()).unwrap();
        assert!(x.src >= 0);
        let and: usize = x.src as usize;
        assert_eq!(a.el_name(and), "OperateAND");
        (r, multis(a, and).into_iter().map(|j| multi_entry(a, j).unwrap()).collect())
    }

    #[test]
    fn only_a_layout_that_moves_the_water_temple_counts() {
        assert!(!moved(&Layout::vanilla()).unwrap());
        assert!(!moved(&swap("Deku Tree", "Fire Temple")).unwrap());
        assert!(moved(&swap("Water Temple", "Forest Temple")).unwrap());
        let w: usize = find(WATER).unwrap();
        for d in (0..DUNGEONS.len()).filter(|d| *d != w) {
            let l: Layout = swap(WATER, DUNGEONS[d].name);
            assert!(moved(&l).unwrap(), "{}", DUNGEONS[d].name);
        }
    }

    #[test]
    fn the_game_writes_the_gate_flag_from_the_water_sage_step_and_the_patch_cuts_only_that() {
        let Some(rf) = real() else {
            return;
        };
        let raw: Vec<u8> = rf.read(MODULE).unwrap();
        let a: Ainb = Ainb::parse(&raw).unwrap();
        assert_eq!(a.write().unwrap(), raw);
        let hits: Vec<usize> = triggers(&a);
        println!("unfixed module: {} write(s) of {FLAG} driven by the Water Sage quest past {STEP}, elements {hits:?}", hits.len());
        assert_eq!(hits.len(), 1);
        assert!(opens_on_flag(&a));
        let steps: Vec<usize> = gate_steps(&a);
        println!("unfixed module: the shutter's OR also reads the {STEP} check directly at multi entries {steps:?}; gate reads {:?}", gate_reads(&a));
        assert_eq!(steps.len(), 1);
        let out: Vec<u8> = patch(&raw).unwrap();
        println!("module bytes: {} unpatched, {} patched", raw.len(), out.len());
        let b: Ainb = Ainb::parse(&out).unwrap();
        assert!(triggers(&b).is_empty());
        assert!(gate_steps(&b).is_empty());
        assert!(opens_on_flag(&b));
        let f: u16 = flag_read(&b).unwrap() as u16;
        assert!(gate_reads(&b).iter().all(|e| *e == (f, 0)), "{:?}", gate_reads(&b));
        assert_eq!(readers_of(&b, steps_el(&a)), 0);
        let md: Vec<usize> = (0..a.multi.len() / 8).filter(|&j| multi_entry(&a, j) != multi_entry(&b, j)).collect();
        assert_eq!(md, steps);
        assert_eq!(a.multi.len(), b.multi.len());
        assert!(audit(&b).is_empty(), "{:?}", audit(&b));
        let i: usize = hits[0];
        let x = b.input(BOOL, b.find_in(i, BOOL, DRIVE).unwrap()).unwrap();
        assert_eq!((x.src, x.val.iter().any(|v| *v != 0)), (-1, false));
        assert_eq!(b.in_str(i, "GameDataNameBool"), Some(FLAG));
        let names = |x: &Ainb| -> Vec<(u16, String, u32)> { (0..x.els.len()).map(|j| (x.el_type(j), x.el_name(j).to_string(), x.el_hash(j))).collect() };
        assert_eq!(names(&a), names(&b));
        let la: Vec<Link> = links(&a);
        let lb: Vec<Link> = links(&b);
        assert_eq!(la.len(), lb.len());
        let diff: Vec<(&Link, &Link)> = la.iter().zip(lb.iter()).filter(|(p, q)| p != q).collect();
        assert_eq!(diff.len(), 1);
        assert_eq!((diff[0].0 .0, diff[0].0 .2.as_str()), (i, DRIVE));
        for j in (0..a.params.len()).filter(|j| *j != i) {
            assert_eq!(a.params[j], b.params[j], "element {j}");
        }
        assert!(patch(&out).is_err());
    }

    #[test]
    fn nothing_is_staged_unless_the_water_temple_moved() {
        let Some(rf) = real() else {
            return;
        };
        let work: &P = P::new("no such work folder");
        for l in [Layout::vanilla(), swap("Deku Tree", "Fire Temple"), swap("Jabu Jabus Belly", "Shadow Temple")] {
            let mut late: Vec<Out> = Vec::new();
            assert!(!stage(&rf, &l, &mut late, work).unwrap());
            assert!(late.is_empty());
        }
        let mut late: Vec<Out> = Vec::new();
        assert!(stage(&rf, &swap(WATER, "Forest Temple"), &mut late, work).unwrap());
        assert_eq!(late.len(), 1);
        assert_eq!(late[0].path, MODULE);
        assert!(triggers(&Ainb::parse(&late[0].z).unwrap()).is_empty());
        assert!(stage(&rf, &swap(WATER, "Forest Temple"), &mut late, work).is_err());
    }

    #[test]
    fn the_module_is_the_only_lake_logic_that_writes_the_flag_and_both_ages_use_it() {
        let Some(mut rf) = real() else {
            return;
        };
        let mut writers: Vec<String> = Vec::new();
        for e in std::fs::read_dir(rf.root.join("Logic")).unwrap() {
            let p: PB = e.unwrap().path();
            let n: String = p.file_name().unwrap().to_string_lossy().to_string();
            if !n.ends_with(".ainb") {
                continue;
            }
            let a: Ainb = Ainb::parse(&std::fs::read(&p).unwrap()).unwrap();
            if a.find_els(SET).into_iter().any(|i| a.in_str(i, "GameDataNameBool") == Some(FLAG)) {
                writers.push(format!("Logic/{n}"));
            }
        }
        assert_eq!(writers, vec![MODULE.to_string()]);
        let stem: &str = MODULE.trim_start_matches("Logic/").trim_end_matches(".logic.module.ainb");
        let mut users: Vec<String> = Vec::new();
        for age in ["Adult", "Child"] {
            for part in ["Static", "Dynamic"] {
                let f: String = format!("Banc/HyruleField/HyruleField_{age}_{part}.bcett.byml.zs");
                let y: Byml = Byml::parse(&rf.read_zs(&f).unwrap()).unwrap();
                let groups: &[Node] = y.root.get("AiGroups").and_then(|g| g.as_arr()).unwrap_or(&[]);
                if groups.iter().any(|g| g.get("Logic").and_then(|l| l.as_str()).is_some_and(|l| l.contains(stem))) {
                    users.push(f);
                }
            }
        }
        println!("maps that run {MODULE}: {users:?}");
        assert!(users.iter().any(|u| u.contains("_Adult_")), "{users:?}");
        assert!(users.iter().any(|u| u.contains("_Child_")), "{users:?}");
    }

    #[test]
    fn the_switch_plays_the_gate_scene_and_after_the_patch_nothing_reads_the_water_sage_step() {
        let Some(mut rf) = real() else {
            return;
        };
        let raw: Vec<u8> = rf.read(MODULE).unwrap();
        let a: Ainb = Ainb::parse(&raw).unwrap();
        let b: Ainb = Ainb::parse(&patch(&raw).unwrap()).unwrap();
        let st: usize = steps_el(&a);
        println!("readers of the {STEP} check: unfixed {}, patched {}", readers_of(&a, st), readers_of(&b, st));
        assert_eq!(readers_of(&a, st), 2);
        assert_eq!(readers_of(&b, st), 0);
        for m in [&a, &b] {
            let f: u16 = flag_read(m).unwrap() as u16;
            let sw: usize = named(m, "ActorLogicBootUpForceControlActor", "Obj_LakeHrl_SwitchHit_A_01");
            let (_, req): (usize, Vec<(u16, u16)>) = scene_request(m);
            assert_eq!(req.len(), 2);
            assert_eq!(req[1], (sw as u16, 3));
            let (n, p): (usize, u16) = (req[0].0 as usize, req[0].1);
            let x = m.input(BOOL, m.find_in(n, BOOL, "BoolMulti").unwrap()).unwrap();
            assert_eq!((m.el_name(n), x.src, x.plug, p), ("OperateAND", f as i16, 0, 0));
        }
        assert_eq!(scene_request(&a), scene_request(&b));
        let ev: &str = FLAG.trim_start_matches("HavePlayedEvent.");
        for age in ["Adult", "Child"] {
            let f: String = format!("Banc/HyruleField/HyruleField_{age}_Static.bcett.byml.zs");
            let y: Byml = Byml::parse(&rf.read_zs(&f).unwrap()).unwrap();
            let groups: &[Node] = y.root.get("AiGroups").and_then(|g| g.as_arr()).unwrap();
            let stem: &str = MODULE.trim_start_matches("Logic/").trim_end_matches(".logic.module.ainb");
            let g: Vec<&Node> = groups.iter().filter(|g| g.get("Logic").and_then(|l| l.as_str()).is_some_and(|l| l.contains(stem))).collect();
            assert_eq!(g.len(), 1, "{f}");
            let refs: &[Node] = g[0].get("References").and_then(|r| r.as_arr()).unwrap();
            let ids: Vec<&str> = refs.iter().filter_map(|r| r.get("Id").and_then(|i| i.as_str())).collect();
            assert_eq!(ids, vec!["EventRequestActor_4eef", "Obj_LakeHrl_SwitchHit_A_01_e78f", "Obj_LakeHrl_SwitchShutter_A_01_62c7", "Obj_LakeHrl_GatePillar_A_01_1a55"], "{f}");
            let h: &Node = refs[0].get("Reference").unwrap();
            let actors: &[Node] = y.root.get("Actors").and_then(|x| x.as_arr()).unwrap();
            let act: Vec<&Node> = actors.iter().filter(|x| x.get("Hash") == Some(h)).collect();
            assert_eq!(act.len(), 1, "{f}");
            assert_eq!(act[0].get("Gyaml").and_then(|x| x.as_str()), Some("EventRequestActor"));
            assert_eq!(act[0].get("Dynamic").and_then(|d| d.get("RequestEventName")).and_then(|x| x.as_str()), Some(ev), "{f}");
        }
    }
}
