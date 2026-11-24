use crate::bfevfl::{Bf, Case, Event, Flow, Kind};
use crate::error::{Error, Res};

pub const SETTING: &str = "free_scarecrow";
pub const NUM: u16 = 163;
pub const LOGIC_KEY: &str = "scarecrow_behavior";
pub const LOGIC_ON: &str = "free";
pub const FLOW: &str = "Dm_SY_Ocarina";
const FREE: &str = "PlayOcarinaFree";
const CALL: &str = "Event109";
const IN_AREA: &str = "EventQueryCheckContactAreaCanCallPierre";
const NO_AREA: u32 = 0;

pub const NOTE: &str = "Free Scarecrow's Song: taking out the Ocarina in one of Pierre's call areas makes him appear there at once, with no song to record or play. PlayOcarinaFree in Dm_SY_Ocarina starts with a copy of the Scarecrow's Song area check (Event109): in a call area it moves to that spot's AppearPierre event, anywhere else the Ocarina plays as usual. A call area goes once its Pierre is out. The logic treats the Scarecrow's Song as known (OoTR's Free Scarecrow's Song). Bonooru and the adult Pierre at Lake Hylia still talk as usual.";

pub fn patch_flow(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == FLOW).ok_or(Error::Format("free scarecrow: Dm_SY_Ocarina has no flow of its name"))?;
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| f.events[i].name == CALL).collect();
    let [call] = hits.as_slice() else {
        return Err(Error::Msg(format!("free scarecrow: expected one {CALL} in {FLOW}, found {}", hits.len())));
    };
    let call: usize = *call;
    let asks: bool = match &f.events[call].kind {
        Kind::Switch { actor, query, cases, .. } => !cases.is_empty() && !cases.iter().any(|c| c.val == NO_AREA) && f.actors.get(*actor as usize).and_then(|a| a.queries.get(*query as usize)).is_some_and(|q| q == IN_AREA),
        _ => false,
    };
    if !asks {
        return Err(Error::Format("free scarecrow: Event109 no longer asks for the Pierre call area"));
    }
    let e: usize = f.entries.iter().position(|e| e.name == FREE).ok_or(Error::Format("free scarecrow: no PlayOcarinaFree entry"))?;
    let at: u16 = crate::dungeon_credit::idx(f.events.len())?;
    let mut gate: Event = f.events[call].clone();
    gate.name = "OOTRRFreeScarecrow".to_string();
    if let Kind::Switch { cases, .. } = &mut gate.kind {
        cases.insert(0, Case { val: NO_AREA, ev: f.entries[e].main });
    }
    f.events.push(gate);
    f.entries[e].main = at;
    Ok(())
}
