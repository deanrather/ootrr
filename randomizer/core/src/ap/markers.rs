use crate::bfevfl::{Bf, Event, Flow, Kind};
use crate::dungeon_credit::{action_slot, set_bool};
use crate::error::{Error, Res};

pub const HOLDER: &str = "GameDataActor";
pub const SET: &str = "EventTriggerSetGameDataBool";
pub const PREFIX: &str = "OOTRR_AP_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hook {
    pub file: &'static str,
    pub flow: &'static str,
    pub after: &'static str,
}

pub const SEED: Hook = Hook { file: crate::intro_skip::FILE, flow: crate::intro_skip::FLOW, after: "Event3" };
pub const GOAL: Hook = Hook { file: crate::ganon_trials::FLOW_FILE, flow: crate::ganon_trials::FLOW, after: "Event37" };

pub fn seed_flag(id: &str) -> String {
    format!("OOTRR.Seed.{id}")
}

pub fn goal_flag(id: &str) -> String {
    format!("OOTRR.Goal.{id}")
}

fn next_of(k: &mut Kind) -> Option<&mut u16> {
    match k {
        Kind::Action { next, .. } | Kind::Sub { next, .. } | Kind::Join { next } => Some(next),
        _ => None,
    }
}

pub fn mark_after(f: &mut Flow, after: &str, flag: &str) -> Res<u16> {
    let i: usize = f.events.iter().position(|e| e.name == after).ok_or_else(|| Error::Msg(format!("markers: {} has no event {after}", f.name)))?;
    let (a, set): (u16, u16) = action_slot(f, HOLDER, SET)?;
    let at: u16 = u16::try_from(f.events.len()).map_err(|_| Error::Format("markers: the flow is full"))?;
    let slot: &mut u16 = next_of(&mut f.events[i].kind).ok_or_else(|| Error::Msg(format!("markers: {after} in {} does not lead to one next event", f.name)))?;
    let then: u16 = *slot;
    *slot = at;
    f.events.push(Event { name: format!("{PREFIX}{at}"), kind: Kind::Action { next: then, actor: a, act: set, params: Some(set_bool(flag.to_string())) } });
    Ok(at)
}

pub fn patch(b: &mut Bf, h: Hook, flag: &str) -> Res<u16> {
    let f: &mut Flow = b.flows.iter_mut().find(|x| x.name == h.flow).ok_or_else(|| Error::Msg(format!("markers: {} has no {} flow", h.file, h.flow)))?;
    mark_after(f, h.after, flag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::romfs::Romfs;

    fn edit_raw(raw: &[u8], h: Hook, flag: &str) -> Res<Vec<u8>> {
        let mut b: Bf = Bf::parse(raw)?;
        patch(&mut b, h, flag)?;
        let out: Vec<u8> = b.write()?;
        if Bf::parse(&out)? != b {
            return Err(Error::Format("markers: the patched flow does not parse back to the edited model"));
        }
        Ok(out)
    }
    use crate::bfevfl::Val;

    fn real() -> Option<Romfs> {
        let p = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(Romfs::open(&p).unwrap())
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn flag_of(f: &Flow, at: u16) -> Option<String> {
        let e: &Event = &f.events[at as usize];
        let Kind::Action { actor, act, params, .. } = &e.kind else { return None };
        let a = &f.actors[*actor as usize];
        if a.name != HOLDER || a.actions[*act as usize] != SET {
            return None;
        }
        params.as_ref()?.items.iter().find(|(k, _)| k == "GameDataName").and_then(|(_, v)| if let Val::Str(s) = v { Some(s.clone()) } else { None })
    }

    #[test]
    fn both_markers_sit_right_after_their_hook_and_keep_the_rest_of_the_chain() {
        let Some(mut rf) = real() else { return };
        for (h, flag) in [(SEED, seed_flag("0123456789ab")), (GOAL, goal_flag("0123456789ab"))] {
            let raw: Vec<u8> = rf.read_zs(h.file).unwrap();
            let before: Bf = Bf::parse(&raw).unwrap();
            let f0: &Flow = before.flows.iter().find(|f| f.name == h.flow).unwrap();
            let i: usize = f0.events.iter().position(|e| e.name == h.after).unwrap();
            let Kind::Action { next: old, .. } = f0.events[i].kind else { panic!("{} is not an action", h.after) };
            let after: Bf = Bf::parse(&edit_raw(&raw, h, &flag).unwrap()).unwrap();
            let f: &Flow = after.flows.iter().find(|f| f.name == h.flow).unwrap();
            let Kind::Action { next: new, .. } = f.events[i].kind else { panic!() };
            assert_eq!(new as usize, f0.events.len());
            assert_eq!(flag_of(f, new), Some(flag.clone()));
            let Kind::Action { next: then, .. } = f.events[new as usize].kind else { panic!() };
            assert_eq!(then, old);
            assert_eq!(&f.events[..f0.events.len()].iter().enumerate().filter(|(j, _)| *j != i).map(|(_, e)| e.clone()).collect::<Vec<_>>(), &f0.events.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, e)| e.clone()).collect::<Vec<_>>());
        }
    }

    #[test]
    fn a_missing_hook_or_flow_is_an_error() {
        let Some(mut rf) = real() else { return };
        let raw: Vec<u8> = rf.read_zs(SEED.file).unwrap();
        assert!(edit_raw(&raw, Hook { after: "NoSuchEvent", ..SEED }, "X").is_err());
        assert!(edit_raw(&raw, Hook { flow: "NoSuchFlow", ..SEED }, "X").is_err());
    }
}
