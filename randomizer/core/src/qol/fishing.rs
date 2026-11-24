use crate::bfevfl::{Bf, Event, Flow, Kind, Val};
use crate::error::{Error, Res};

pub const SETTING: &str = "fishing_any_fish";
pub const NUM: u16 = 162;
pub const FLOW: &str = "MiniGame_Fishing";
pub const SIZE: &str = "MiniGame_Fishing_KeepFishSize";
pub const SINKING: &str = "MiniGame_Fishing_UseSinkingLure";
pub const LOACH: &str = "MiniGame_Fishing_KeepLoach";
pub const LIMITS: [i32; 3] = [40, 50, 60];
pub const PRIZE_TALK: &str = "EventFlowMsg/MiniGame_Fishing:Talk_145";
const INT_QUERY: &str = "GameDataActor.EventQueryIsGameDataInt";
const BOOL_QUERY: &str = "GameDataActor.EventQueryIsGameDataBool";
const TALK: &str = "Npc_EventStarter.EventTalk";
const YES: u32 = 0;
const NO: u32 = 1;

pub const NOTE: &str = "The fishing pond owner gives the child prize and the adult prize for any fish you hand in, whatever its size, a loach or a fish caught with the sinking lure. The size checks at 40, 50 and 60 in MiniGame_Fishing read 0, and the loach and sinking lure branches go the normal prize way. The size record and the texts are unchanged. Always on.";

fn call(f: &Flow, e: &Event) -> Option<String> {
    match &e.kind {
        Kind::Switch { actor, query, .. } => f.actors.get(*actor as usize).and_then(|a| a.queries.get(*query as usize).map(|q| format!("{}.{q}", a.name))),
        Kind::Action { actor, act, .. } => f.actors.get(*actor as usize).and_then(|a| a.actions.get(*act as usize).map(|q| format!("{}.{q}", a.name))),
        _ => None,
    }
}

fn reads(f: &Flow, e: &Event, query: &str, flag: &str) -> bool {
    call(f, e).as_deref() == Some(query) && e.kind.params().and_then(|c| c.get("GameDataName")) == Some(&Val::Str(flag.to_string()))
}

fn value(e: &Event) -> Option<i32> {
    match e.kind.params().and_then(|c| c.get("Value")) {
        Some(Val::Int(n)) => Some(*n),
        _ => None,
    }
}

fn case(e: &Event, v: u32) -> Option<u16> {
    match &e.kind {
        Kind::Switch { cases, .. } => cases.iter().find(|c| c.val == v).map(|c| c.ev),
        _ => None,
    }
}

fn set_case(e: &mut Event, v: u32, to: u16) {
    if let Kind::Switch { cases, .. } = &mut e.kind {
        match cases.iter_mut().find(|c| c.val == v) {
            Some(c) => c.ev = to,
            None => cases.push(crate::bfevfl::Case { val: v, ev: to }),
        }
    }
}

fn talks(f: &Flow, i: u16, msg: &str) -> bool {
    f.events.get(i as usize).is_some_and(|e| call(f, e).as_deref() == Some(TALK) && e.kind.params().and_then(|c| c.get("MessageId")) == Some(&Val::Str(msg.to_string())))
}

pub fn patch(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == FLOW).ok_or(Error::Format("any fish: MiniGame_Fishing has no MiniGame_Fishing flow"))?;
    let mut first: Option<u16> = None;
    for limit in LIMITS {
        let hits: Vec<usize> = (0..f.events.len()).filter(|&i| reads(f, &f.events[i], INT_QUERY, SIZE) && value(&f.events[i]) == Some(limit)).collect();
        let [i] = hits.as_slice() else {
            return Err(Error::Msg(format!("any fish: expected one fish size check at {limit}, found {}", hits.len())));
        };
        if limit == LIMITS[0] {
            first = Some(crate::dungeon_credit::idx(*i)?);
        }
        if let Some(c) = f.events[*i].kind.params_mut() {
            for (k, v) in c.items.iter_mut() {
                if k == "Value" {
                    *v = Val::Int(0);
                }
            }
        }
    }
    let first: u16 = first.ok_or(Error::Format("any fish: no first size check"))?;
    let lures: Vec<usize> = (0..f.events.len()).filter(|&i| reads(f, &f.events[i], BOOL_QUERY, SINKING) && case(&f.events[i], NO).is_some_and(|t| talks(f, t, PRIZE_TALK))).collect();
    if lures.len() != 2 {
        return Err(Error::Msg(format!("any fish: expected two sinking lure checks before the prizes, found {}", lures.len())));
    }
    for i in lures {
        let to: u16 = case(&f.events[i], NO).ok_or(Error::Format("any fish: a sinking lure check lost its case"))?;
        set_case(&mut f.events[i], YES, to);
    }
    let loach: Vec<usize> = (0..f.events.len()).filter(|&i| reads(f, &f.events[i], BOOL_QUERY, LOACH) && case(&f.events[i], NO) == Some(first)).collect();
    let [l] = loach.as_slice() else {
        return Err(Error::Msg(format!("any fish: expected one loach check before the size checks, found {}", loach.len())));
    };
    set_case(&mut f.events[*l], YES, first);
    Ok(())
}

