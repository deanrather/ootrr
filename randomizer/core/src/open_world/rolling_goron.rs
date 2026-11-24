use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::gerudo_card::{schedule_path, SCHEDULE_PACK};
use crate::qol::Have;
use crate::romfs::Romfs;
use crate::staging::Out;

pub const OWN: &str = "Npc_Goron_DaruniaJr_Adult";
pub const ROLL: &str = "SageOfFire_StopDaruniaJr_Npc_Goron_DaruniaJr_Adult";
pub const CRY: &str = "SageOfFire_TalkDaruniaJr_Npc_Goron_DaruniaJr_Adult";
pub const STOPPED: &str = "SageOfFire_Step02_DaruniaJr_FirstTalked";
pub const GIVEN: &str = "SageOfFire_Step02_DoorOpen";

const CONDS: &str = "ConditionalSchedule";
const DEFAULT: &str = "DefaultSchedule";

fn while_not(s: &Byml, flag: &str) -> Res<Node> {
    let mut n: Node = s.root.get(DEFAULT).cloned().ok_or(Error::Format("rolling goron: a SageOfFire schedule has no default, the game files differ"))?;
    n.set("ConditionFlag", Node::Str(flag.to_string()));
    n.set("IsReverseCondition", Node::Bool(true));
    Ok(n)
}

pub fn merged(own: &[u8], roll: &[u8], cry: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(own)?;
    let list: &[Node] = y.root.get(CONDS).and_then(|c| c.as_arr()).ok_or(Error::Format("rolling goron: his own schedule has no conditions, the game files differ"))?;
    if list.iter().any(|n| n.get("ConditionFlag").and_then(|f| f.as_str()) == Some(STOPPED)) {
        return Err(Error::Msg("rolling goron: his own schedule already reads the stop".to_string()));
    }
    let mut v: Vec<Node> = vec![while_not(&Byml::parse(roll)?, STOPPED)?, while_not(&Byml::parse(cry)?, GIVEN)?];
    v.extend(list.iter().cloned());
    y.root.set(CONDS, Node::Arr(v));
    y.write()
}

fn member(es: &[crate::sarc::Entry], name: &str) -> Res<Vec<u8>> {
    let f: String = schedule_path(name);
    crate::sarc::get(es, &f).map(|e| e.data.clone()).ok_or_else(|| Error::Msg(format!("rolling goron: the schedule pack has no {f}, the game files differ")))
}

pub fn stage(rf: &mut Romfs, outs: &mut Vec<Out>, have: Have) -> Res<()> {
    let es: Vec<crate::sarc::Entry> = crate::sarc::parse(&rf.read_zs(SCHEDULE_PACK)?)?;
    let roll: Vec<u8> = member(&es, ROLL)?;
    let cry: Vec<u8> = member(&es, CRY)?;
    super::fortress::merge_plain(rf, outs, have, &schedule_path(OWN), |raw| merged(raw, &roll, &cry))
}

pub fn budget(rf: &mut Romfs, t: &mut crate::rsizetable::Rst) -> Res<usize> {
    let base: crate::rsizetable::Rst = crate::rsizetable::load(rf)?;
    let own: String = schedule_path(OWN);
    let mut need: u32 = 0;
    for n in [OWN, ROLL, CRY] {
        let f: String = schedule_path(n);
        need += base.get(&f).ok_or_else(|| Error::Msg(format!("rolling goron: the size table has no {f}")))?;
    }
    if t.get(&own).is_none_or(|cur| cur < need) {
        t.set_size(&own, need)?;
        return Ok(1);
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf as PB;

    fn real() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(p)
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn events(n: &Node) -> Vec<String> {
        n.get("EventEntry").and_then(|e| e.as_arr()).unwrap().iter().filter_map(|e| e.get("EventName").and_then(|s| s.as_str()).map(str::to_string)).collect()
    }

    #[test]
    fn he_rolls_until_stopped_then_cries_until_the_gift_then_talks_as_before() {
        let Some(root) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let es: Vec<crate::sarc::Entry> = crate::sarc::parse(&rf.read_zs(SCHEDULE_PACK).unwrap()).unwrap();
        let own: Vec<u8> = rf.read(&schedule_path(OWN)).unwrap();
        let (roll, cry): (Vec<u8>, Vec<u8>) = (member(&es, ROLL).unwrap(), member(&es, CRY).unwrap());
        let out: Vec<u8> = merged(&own, &roll, &cry).unwrap();
        let y: Byml = Byml::parse(&out).unwrap();
        let o: Byml = Byml::parse(&own).unwrap();
        let list: &[Node] = y.root.get(CONDS).unwrap().as_arr().unwrap();
        for (i, flag, from) in [(0, STOPPED, &roll), (1, GIVEN, &cry)] {
            let n: &Node = &list[i];
            assert_eq!(n.get("ConditionFlag").and_then(|f| f.as_str()), Some(flag));
            assert_eq!(n.get("IsReverseCondition"), Some(&Node::Bool(true)));
            let d: Byml = Byml::parse(from).unwrap();
            assert_eq!(n.get("ActionSchedule"), d.root.get(DEFAULT).unwrap().get("ActionSchedule"));
            assert_eq!(events(n), events(d.root.get(DEFAULT).unwrap()));
        }
        let a: &Node = &list[0].get("ActionSchedule").unwrap().as_arr().unwrap()[0];
        assert_eq!(a.get("Setting").and_then(|s| s.get("IsTurnToTerror")), Some(&Node::Bool(true)));
        assert_eq!(a.get("Setting").and_then(|s| s.get("MovingAS")).and_then(|s| s.as_str()), Some("Run_GoronRoll_02"));
        assert_eq!(&list[2..], o.root.get(CONDS).unwrap().as_arr().unwrap());
        assert_eq!(y.root.get(DEFAULT), o.root.get(DEFAULT));
        assert!(merged(&out, &roll, &cry).is_err());
    }

    #[test]
    fn the_flags_are_saved_and_set_by_both_ends_of_the_stop() {
        let Some(root) = real() else {
            return;
        };
        let names: String = std::fs::read_to_string(PB::from(env!("CARGO_MANIFEST_DIR")).join("../../data/flags.json")).unwrap();
        for f in [STOPPED, GIVEN] {
            let line: &str = names.lines().find(|l| l.contains(&format!("\"name\":\"{f}\""))).unwrap();
            assert!(line.contains("\"save\":0"), "{f} is not saved");
        }
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let b: crate::bfevfl::Bf = crate::bfevfl::Bf::parse(&rf.read_zs("Event/EventFlow/SageOfFire.bfevfl.zs").unwrap()).unwrap();
        let f: &crate::bfevfl::Flow = b.flows.iter().find(|f| f.name == "SageOfFire").unwrap();
        let sets = |flag: &str| -> usize {
            f.events
                .iter()
                .filter(|e| matches!(e.kind, crate::bfevfl::Kind::Action { .. }))
                .filter(|e| e.kind.params().and_then(|p| p.get("GameDataName")) == Some(&crate::bfevfl::Val::Str(flag.to_string())))
                .count()
        };
        assert_eq!(sets(STOPPED), 2);
        assert_eq!(sets(GIVEN), 1);
    }

    #[test]
    fn the_merged_schedule_gets_all_three_budgets() {
        let Some(root) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let base: crate::rsizetable::Rst = crate::rsizetable::load(&mut rf).unwrap();
        let mut t: crate::rsizetable::Rst = base.clone();
        assert_eq!(budget(&mut rf, &mut t).unwrap(), 1);
        let sum: u32 = [OWN, ROLL, CRY].iter().map(|n| base.get(&schedule_path(n)).unwrap()).sum();
        assert_eq!(t.get(&schedule_path(OWN)).unwrap(), sum);
        assert_eq!(budget(&mut rf, &mut t).unwrap(), 0);
    }
}
