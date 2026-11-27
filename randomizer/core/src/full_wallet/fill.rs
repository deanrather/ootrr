use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Cont, Event, Flow, Kind, Val};
use crate::error::{Error as E, Res};
use crate::on_get::GetType;

pub const PREFIX: &str = "OOTRRFullWallet";
pub const FILL: i32 = 1000;
pub const HOLD: i32 = 60;
pub const UI: &str = "UISystemActor";
pub const OPEN: &str = "EventOpenRupeeDisplay";
pub const STABLE: &str = "EventWaitStableNumDisplay";
pub const CLOSE: &str = "EventCloseRupeeDisplay";
pub const POUCH: &str = "PouchSystemActor";
pub const ADD: &str = "EventTriggerIncreaseRupee";
pub const WAITER: &str = "EventSystemActor";
pub const WAIT: &str = "EventWait";

fn b(v: bool) -> Val {
    Val::Bool(u32::from(v))
}

fn kv(k: &str, v: Val) -> (String, Val) {
    (k.to_string(), v)
}

fn push(f: &mut Flow, kind: Kind) -> Res<u16> {
    let at: u16 = u16::try_from(f.events.len()).ok().filter(|i| *i != NONE).ok_or(E::Format("full wallets: the flow has too many events"))?;
    f.events.push(Event { name: format!("{PREFIX}{at}"), kind });
    Ok(at)
}

fn act(f: &mut Flow, who: &str, what: &str, items: Vec<(String, Val)>, next: u16) -> Res<u16> {
    let (a, x): (u16, u16) = crate::start_state::slot(f, who, what)?;
    push(f, Kind::Action { next, actor: a, act: x, params: Some(Cont { items }) })
}

pub fn fill(f: &mut Flow, get_type: GetType, then: u16) -> Res<u16> {
    let mut next: u16 = then;
    if get_type == GetType::Gift {
        next = act(f, UI, CLOSE, vec![kv("IsWaitEnd", b(true))], next)?;
        next = act(f, WAITER, WAIT, vec![kv("IsWaitEnd", b(true)), kv("Frames", Val::Int(HOLD))], next)?;
    }
    next = act(f, UI, STABLE, vec![kv("IsWaitEnd", b(true))], next)?;
    next = act(f, POUCH, ADD, vec![kv("Value", Val::Int(FILL))], next)?;
    if get_type == GetType::Gift {
        let open: Vec<(String, Val)> = vec![kv("IsWaitEnd", b(true)), kv("IsNeedMaxNum", b(false)), kv("IsWaitOpenAnim", b(true)), kv("IsWaitChoiceBtnIn", b(false))];
        next = act(f, UI, OPEN, open, next)?;
    }
    Ok(next)
}

