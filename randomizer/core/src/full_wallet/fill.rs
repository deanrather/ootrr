use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Case, Cont, Event, Flow, Kind, Val};
use crate::error::{Error as E, Res};

pub const PREFIX: &str = "OOTRRFullWallet";
pub const FILL: i32 = 1000;
pub const HOLD: i32 = 60;
pub const NAME: &str = "UI_MessageTargetActorName";
pub const UI: &str = "UISystemActor";
pub const OPEN: &str = "EventOpenRupeeDisplay";
pub const STABLE: &str = "EventWaitStableNumDisplay";
pub const CLOSE: &str = "EventCloseRupeeDisplay";
pub const POUCH: &str = "PouchSystemActor";
pub const ADD: &str = "EventTriggerIncreaseRupee";
pub const WAITER: &str = "EventSystemActor";
pub const WAIT: &str = "EventWait";
pub const DATA: &str = "GameDataActor";
pub const ASK: &str = "EventQueryIsGameDataString64";
pub const SET: &str = "EventTriggerSetGameDataString64";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    Popup,
    Shop,
}

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

pub fn ours(f: &Flow, at: u16) -> bool {
    at != NONE && f.events.get(usize::from(at)).is_some_and(|e| e.name.starts_with(PREFIX))
}

pub fn fill(f: &mut Flow, show: Show, then: u16) -> Res<u16> {
    let mut next: u16 = then;
    if show == Show::Popup {
        next = act(f, UI, CLOSE, vec![kv("IsWaitEnd", b(true))], next)?;
        next = act(f, WAITER, WAIT, vec![kv("IsWaitEnd", b(true)), kv("Frames", Val::Int(HOLD))], next)?;
    }
    next = act(f, UI, STABLE, vec![kv("IsWaitEnd", b(true))], next)?;
    next = act(f, POUCH, ADD, vec![kv("Value", Val::Int(FILL))], next)?;
    if show == Show::Popup {
        let open: Vec<(String, Val)> = vec![kv("IsWaitEnd", b(true)), kv("IsNeedMaxNum", b(false)), kv("IsWaitOpenAnim", b(true)), kv("IsWaitChoiceBtnIn", b(false))];
        next = act(f, UI, OPEN, open, next)?;
    }
    Ok(next)
}

pub fn asked(f: &mut Flow, names: &[String], hit: u16, miss: u16) -> Res<u16> {
    let (a, q): (u16, u16) = crate::progressive::ensure(f, DATA, ASK)?;
    let mut at: u16 = miss;
    for n in names.iter().rev() {
        let c: Cont = Cont { items: vec![kv("GameDataName", Val::Str(NAME.to_string())), kv("Index", Val::Int(-1)), kv("Value", Val::Str(n.clone()))] };
        at = push(f, Kind::Switch { actor: a, query: q, cases: vec![Case { val: 0, ev: hit }, Case { val: 1, ev: at }], params: Some(c) })?;
    }
    Ok(at)
}

pub fn named(f: &mut Flow, arg: &str, next: u16) -> Res<u16> {
    let settle: u16 = act(f, WAITER, WAIT, vec![kv("IsWaitEnd", b(true)), kv("Frames", Val::Int(1))], next)?;
    act(f, DATA, SET, vec![kv("GameDataName", Val::Str(NAME.to_string())), kv("Value", Val::Arg(arg.to_string())), kv("Index", Val::Int(-1))], settle)
}
