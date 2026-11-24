use crate::bfevfl::{Bf, Event, Flow, Kind, Val};
use crate::error::{Error, Res};

pub const SETTING: &str = "faster_ocarina_minigame";
pub const NUM: u16 = 165;
pub const FLOW: &str = "Npc_SkullKid_Child_02";
pub const ENTRY: &str = "SessionStart";
pub const GRADE: &str = "Minigame_SkullKidSession_CurrentGradeSoundNumForSession";
pub const PLAYED: i32 = 6;
pub const LAST: i32 = 8;
pub const MAX_RALLY: &str = "Minigame_SkullKidSession_MaxRallyCount";
pub const RALLIES: i32 = 1;
pub const HEART: &str = "Minigame_SkullKidSession_IsGetHeartPiece";
const SET_INT: &str = "EventTriggerSetGameDataInt";
const BOOL_QUERY: &str = "EventQueryIsGameDataBool";
const DATA: &str = "GameDataActor";
const INT_QUERY: &str = "EventQueryIsGameDataInt";
const LESS: u32 = 0;

pub const NOTE: &str = "Faster Ocarina minigame: the Skull Kids in the Lost Woods play one turn of 6 notes, and winning it gives the check. CurrentGradeSoundNumForSession starts at 6 and MaxRallyCount at 1 (save defaults, and SessionStart in Npc_SkullKid_Child_02 sets both), the example prep builds 6 notes at once (the 8-note +3 way), and clearing goes to the Heart Piece branch instead of raising the game to 8. The Blue Rupee and Red Rupee prizes are gone. Always on.";

fn named(f: &Flow, name: &str) -> Res<usize> {
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| f.events[i].name == name).collect();
    match hits.as_slice() {
        [i] => Ok(*i),
        _ => Err(Error::Msg(format!("faster ocarina minigame: expected one {name} in {FLOW}, found {}", hits.len()))),
    }
}

fn call(f: &Flow, e: &Event) -> Option<String> {
    match &e.kind {
        Kind::Switch { actor, query, .. } => f.actors.get(*actor as usize).and_then(|a| a.queries.get(*query as usize).map(|q| format!("{}.{q}", a.name))),
        Kind::Action { actor, act, .. } => f.actors.get(*actor as usize).and_then(|a| a.actions.get(*act as usize).map(|q| format!("{}.{q}", a.name))),
        _ => None,
    }
}

fn param<'a>(e: &'a Event, k: &str) -> Option<&'a Val> {
    e.kind.params().and_then(|c| c.get(k))
}

pub fn defaults() -> Vec<crate::start_defaults::Edit> {
    vec![crate::start_defaults::Edit::Int(GRADE.to_string(), PLAYED), crate::start_defaults::Edit::Int(MAX_RALLY.to_string(), RALLIES)]
}

fn set_int(name: &str, v: i32) -> crate::bfevfl::Cont {
    crate::bfevfl::Cont {
        items: vec![
            ("GameDataName".to_string(), Val::Str(name.to_string())),
            ("Index".to_string(), Val::Int(-1)),
            ("Value".to_string(), Val::Int(v)),
        ],
    }
}

pub fn patch(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == FLOW).ok_or(Error::Format("faster ocarina minigame: Npc_SkullKid_Child_02 has no flow of its name"))?;
    let idx = |i: usize| crate::dungeon_credit::idx(i);
    let ask: usize = named(f, "Event64")?;
    let ok: bool = call(f, &f.events[ask]).as_deref() == Some(&format!("{DATA}.{BOOL_QUERY}")) && param(&f.events[ask], "GameDataName") == Some(&Val::Str(HEART.to_string()));
    if !ok {
        return Err(Error::Format("faster ocarina minigame: Event64 no longer asks for the Heart Piece flag"));
    }
    let way: usize = named(f, "Event68")?;
    let reaches: bool = matches!(&f.events[way].kind, Kind::Action { next, .. } if f.events.get(*next as usize).is_some_and(|e| matches!(e.kind, Kind::Action { next, .. } if next as usize == ask)));
    if !reaches {
        return Err(Error::Format("faster ocarina minigame: Event68 no longer leads to the Heart Piece check"));
    }
    let eight: usize = named(f, "Event10")?;
    let ok: bool = call(f, &f.events[eight]).as_deref() == Some(&format!("{DATA}.{INT_QUERY}")) && param(&f.events[eight], "GameDataName") == Some(&Val::Str(GRADE.to_string())) && param(&f.events[eight], "Value") == Some(&Val::Int(LAST));
    if !ok {
        return Err(Error::Format("faster ocarina minigame: Event10 no longer asks for the 8-note game"));
    }
    let to: u16 = idx(way)?;
    match &mut f.events[eight].kind {
        Kind::Switch { cases, .. } if cases.iter().any(|c| c.val == LESS) => {
            for c in cases.iter_mut().filter(|c| c.val == LESS) {
                c.ev = to;
            }
        }
        _ => return Err(Error::Format("faster ocarina minigame: Event10 has no below-8 case")),
    }
    let prep: usize = named(f, "Event143")?;
    let plus3: u16 = match &f.events[prep].kind {
        Kind::Switch { cases, .. } => cases.iter().find(|c| c.val == LAST as u32).map(|c| c.ev),
        _ => None,
    }
    .ok_or(Error::Format("faster ocarina minigame: Event143 no longer branches on the 8-note game"))?;
    match &mut f.events[prep].kind {
        Kind::Switch { cases, .. } if cases.iter().any(|c| c.val == PLAYED as u32) => {
            for c in cases.iter_mut().filter(|c| c.val == PLAYED as u32) {
                c.ev = plus3;
            }
        }
        _ => return Err(Error::Format("faster ocarina minigame: Event143 has no 6-note case")),
    }
    let mut n: usize = 0;
    for ev in f.events.iter_mut() {
        if param(ev, "GameDataName") == Some(&Val::Str(MAX_RALLY.to_string())) {
            if let Some(c) = ev.kind.params_mut() {
                for (k, v) in c.items.iter_mut() {
                    if k == "Value" {
                        *v = Val::Int(RALLIES);
                        n += 1;
                    }
                }
            }
        }
    }
    if n != 2 {
        return Err(Error::Msg(format!("faster ocarina minigame: expected two MaxRallyCount sets, found {n}")));
    }
    let (a, set): (u16, u16) = crate::dungeon_credit::action_slot(f, DATA, SET_INT)?;
    let e: usize = f.entries.iter().position(|e| e.name == ENTRY).ok_or(Error::Format("faster ocarina minigame: no SessionStart entry"))?;
    let at: u16 = idx(f.events.len())?;
    f.events.push(Event { name: "OOTRRSixNotes".to_string(), kind: Kind::Action { next: at + 1, actor: a, act: set, params: Some(set_int(GRADE, PLAYED)) } });
    f.events.push(Event { name: "OOTRROneTurn".to_string(), kind: Kind::Action { next: f.entries[e].main, actor: a, act: set, params: Some(set_int(MAX_RALLY, RALLIES)) } });
    f.entries[e].main = at;
    Ok(())
}
