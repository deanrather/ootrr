//! Adult trade shuffle (setting `adult_trade_shuffle`, Ship of Harkinian style): every adult trade
//! item is its own item, found anywhere, kept until its NPC takes it. Design and the dump findings
//! behind it: docs/adult-trade-shuffle.md.
//!
//! - The ten trade spots give only their shuffled item: crate::flow_locations::plan replaces their
//!   chain gifts like any NPC gift (Opts::trades) instead of cloning them.
//! - The hand ins are item driven and run at any quest step (checked with the flow simulator), but
//!   Grog and Fado stand only at some steps, Anju offers her first gift only at Ready, and
//!   Biggoron's sword would end the quest. So the quest step goes by the earliest trade item the
//!   player holds ([`ORDER`], [`settle`]): every step request of the trade NPCs becomes a settle,
//!   Anju's first gift is followed by one, and so is every item get (GetPouchContent).
//! - Only one adult trade item is out at a time. The menu's trade slot, the L/R list and the HUD
//!   all draw the first trade item of the slot's list whose pouch count is above 0, so the one
//!   the player picked has count 1 and every other held one is parked at [`PARKED`]; held means
//!   any count but 0 ([`holds`]). A new trade item is parked while the button's own is out
//!   ([`park_others`]), a hand in brings the nearest parked one before it out and onto the button ([`fall_back`]), and the
//!   menu keeps the same rule every frame ([`menu`]).
//! - Showing an adult trade item where nobody takes it (ShowItem:EntryPoint0) brings the next held
//!   trade item out and onto the item button and parks the shown one ([`cycle`]), then settles.
//!   In the item menu a second A press on the trade slot does the same ([`menu`]), and each trade
//!   item's description says so ([`text`]).
//! - The Pocket Egg only hatches when it is held and no Pocket Cucco is ([`EGG_ENTRY`]).
//! - The slot's list is earliest first ([`pouch_order`]).

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Actor, Bf, Case, Cont, Event, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::logic_items::ItemId;
use crate::romfs::Romfs;
use crate::slice::data::Data;
use crate::staging::Out;
use crate::world::{LocId, Placement, World};

pub const SETTING: &str = "adult_trade_shuffle";
pub const LOGIC: (&str, &str) = ("adult_trade_shuffle", "true");

pub const QUEST: &str = "SubEvent_GetSwordBigGoron";
pub const STEP: &str = "Step_SubEvent_GetSwordBigGoron";
pub const BUTTON: &str = "PlayerStatus.AssignableItem.Adult";
/// The settle runs only once Anju's first adult gift is out: before it the quest is at Ready and
/// Anju offers that gift only then.
pub const STARTED: &str = "SubEvent_GetSwordBigGoron_IsAfter_Ready_Exp";
const PREFIX: &str = "OOTRRTradeShuffle";

/// One adult trade item: logic name, actor, item-button enum value, the `Item` index
/// EventPlayerSetSelectedItem takes (the enum without Empty) and the quest step it stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trade {
    pub name: &'static str,
    pub actor: &'static str,
    pub slot: &'static str,
    pub index: i32,
    pub step: Option<&'static str>,
}

const fn t(name: &'static str, actor: &'static str, slot: &'static str, index: i32, step: Option<&'static str>) -> Trade {
    Trade { name, actor, slot, index, step }
}

/// Earliest first. The Pocket Egg has no step: nobody takes it, and counting it would keep Grog
/// and Fado away until it hatches, on an engine timer.
pub const ORDER: [Trade; 11] = [
    t("Pocket Egg", "Item_PocketEgg", "PocketEgg", 34, None),
    t("Pocket Cucco", "Item_PocketCucco", "PocketCucco", 35, Some("EggHatched")),
    t("Cojiro", "Item_CojiroCucco", "CojiroCucco", 36, Some("Step03")),
    t("Odd Mushroom", "Item_OddMushroom", "OddMushroom", 37, Some("Step04")),
    t("Odd Potion", "Item_OddPowder", "OddPowder", 38, Some("Step05")),
    t("Poachers Saw", "Item_PoacherSaw", "PoacherSaw", 39, Some("Step06")),
    t("Broken Sword", "Item_BrokenSword", "BrokenSword", 40, Some("Step07")),
    t("Prescription", "Item_Prescription", "Prescription", 41, Some("Step08")),
    t("Eyeball Frog", "Item_EyeballFrog", "EyeballFrog", 42, Some("Step09")),
    t("Eyedrops", "Item_EyeDrop", "EyeDrop", 43, Some("Step10")),
    t("Claim Check", "Item_ClaimCheck", "ClaimCheck", 44, Some("Step11")),
];

/// The trade item that the egg hatch hands out, not a trade spot's gift.
pub const HATCHED: &str = "Pocket Cucco";

pub fn item(actor: &str) -> bool {
    ORDER.iter().any(|x| x.actor == actor)
}

pub fn by_name(name: &str) -> Option<&'static Trade> {
    ORDER.iter().find(|x| x.name == name)
}

/// The NPC flows whose quest step requests become settles (Talon keeps his own, see
/// crate::open_world::trade). Anju's first gift request stays and is followed by a settle.
pub const NPCS: [&str; 9] = [
    "Npc_Hylia_049_Adult",
    "Npc_Hylia_017_Adult",
    "Npc_Hylia_053_Adult",
    "Npc_Kokiri_Fado_Adult",
    "Npc_Hylia_012_Adult",
    "Npc_Goron_Biggoron_Adult",
    "Npc_Zora_KingZora_adult",
    "Npc_Hylia_045_Adult",
    "Dm_DeathMtTrail_0050",
];
/// Anju's first gift moves the quest from Ready; that request stays.
pub const KEEP: &str = "StartSubEvent";

pub const SHOW_FLOW: &str = "ShowItem";
pub const SHOW_ENTRY: &str = "EntryPoint0";
pub const GET_FLOW: &str = crate::starter_bags::FLOW;
pub const GET_ADD: (usize, &str) = (0, "Event0");
pub const EGG_ENTRY: &str = "WaitEggHatch_Npc_Navi_StepStart";

pub const POUCH_UI: &str = "UI/Pouch/PouchTable/PouchTable.game__ui__PouchTable.bgyml";
pub const TRADE_BUTTON: &str = "Pa_ItemIcon_23";

pub const NOTE: &str = "Adult trade shuffle: the eleven adult trade items are shuffled and the ten trade hand ins give only their shuffled item. Every trade item you find stays until its NPC takes it, so you can hand them over in any order. The quest follows the earliest trade item you hold (Grog and Fado appear for it), and only the trade item you picked shows in the menu, the L/R list and on the item button. To switch, press A on the menu's trade slot again, or show the trade item where nobody takes it: the next one you hold comes up. The Pocket Egg only hatches while you hold no Pocket Cucco.";

const PROGRESS: &str = "EventExecuteProgressChallengeStep";
const ROLLBACK: &str = "EventExecuteRollbackChallengeStep";
const CHALLENGE: &str = "ChallengeSystemActor";
const DATA: &str = "GameDataActor";
const PLAYER: &str = "Player_Event";

fn idx(n: usize) -> Res<u16> {
    u16::try_from(n).ok().filter(|&i| i != NONE).ok_or(Error::Format("trade shuffle: the flow has too many events"))
}

fn holder(f: &mut Flow, name: &str) -> Res<usize> {
    if let Some(i) = f.actors.iter().position(|a| a.name == name && a.sub.is_empty()) {
        return Ok(i);
    }
    let t: &Actor = f.actors.first().ok_or_else(|| Error::Msg(format!("trade shuffle: the flow {} has no actors", f.name)))?;
    let a: Actor = Actor { name: name.to_string(), sub: String::new(), arg: String::new(), actions: Vec::new(), queries: Vec::new(), params: t.params.clone(), ep: t.ep, cut: t.cut };
    f.actors.push(a);
    Ok(f.actors.len() - 1)
}

fn action(f: &mut Flow, actor: &str, name: &str) -> Res<(u16, u16)> {
    let i: usize = holder(f, actor)?;
    let a: &mut Actor = &mut f.actors[i];
    let j: usize = match a.actions.iter().position(|s| s == name) {
        Some(j) => j,
        None => {
            a.actions.push(name.to_string());
            a.actions.len() - 1
        }
    };
    Ok((idx(i)?, idx(j)?))
}

fn query(f: &mut Flow, actor: &str, name: &str) -> Res<(u16, u16)> {
    let i: usize = holder(f, actor)?;
    let a: &mut Actor = &mut f.actors[i];
    let j: usize = match a.queries.iter().position(|s| s == name) {
        Some(j) => j,
        None => {
            a.queries.push(name.to_string());
            a.queries.len() - 1
        }
    };
    Ok((idx(i)?, idx(j)?))
}

fn push(f: &mut Flow, kind: Kind) -> Res<u16> {
    let i: u16 = idx(f.events.len())?;
    f.events.push(Event { name: format!("{PREFIX}{i}"), kind });
    Ok(i)
}

fn cont(items: Vec<(&str, Val)>) -> Option<Cont> {
    Some(Cont { items: items.into_iter().map(|(k, v)| (k.to_string(), v)).collect() })
}

fn s(v: &str) -> Val {
    Val::Str(v.to_string())
}

/// A yes/no question: case 0 is yes, as in the game's own bool queries.
fn ask(f: &mut Flow, actor: &str, q: &str, params: Option<Cont>, yes: u16, no: u16) -> Res<u16> {
    let (a, x) = query(f, actor, q)?;
    push(f, Kind::Switch { actor: a, query: x, cases: vec![Case { val: 0, ev: yes }, Case { val: 1, ev: no }], params })
}

/// The pouch count of a held trade item that is parked: the menu's trade slot (and the L/R list
/// and the HUD with it) draws the first trade item of its list whose count is above 0, so only
/// one adult trade item is out (count 1) at a time, the one the player picked, and the others
/// wait at this count. A parked item still counts as held everywhere in this module.
pub const PARKED: i32 = -1;

pub fn count_name(actor: &str) -> String {
    format!("PouchData.{actor}.StockNum")
}

/// A three-way test of an int against `value`: below, equal, above (cases 0, 1, 2).
fn int3(f: &mut Flow, name: &str, value: i32, below: u16, equal: u16, above: u16) -> Res<u16> {
    let (a, x) = query(f, DATA, "EventQueryIsGameDataInt")?;
    let cases: Vec<Case> = vec![Case { val: 0, ev: below }, Case { val: 1, ev: equal }, Case { val: 2, ev: above }];
    push(f, Kind::Switch { actor: a, query: x, cases, params: cont(vec![("Index", Val::Int(-1)), ("GameDataName", s(name)), ("Value", Val::Int(value))]) })
}

/// Held: out or parked, so any count but 0.
fn holds(f: &mut Flow, actor: &str, yes: u16, no: u16) -> Res<u16> {
    int3(f, &count_name(actor), 0, yes, no, yes)
}

fn set_count(f: &mut Flow, actor: &str, v: i32, next: u16) -> Res<u16> {
    let (a, x) = action(f, DATA, "EventTriggerSetGameDataInt")?;
    push(f, Kind::Action { next, actor: a, act: x, params: cont(vec![("Index", Val::Int(-1)), ("GameDataName", s(&count_name(actor))), ("Value", Val::Int(v))]) })
}

/// After a new trade item came in: when the item button's own trade item is out, every other out
/// trade item is parked, so the slot keeps showing the button's item. With another item on the
/// button nothing is parked (the slot then shows the earliest one that is out).
pub fn park_others(f: &mut Flow, then: u16) -> Res<u16> {
    let mut at: u16 = then;
    for x in ORDER.iter().rev() {
        let park: u16 = set_count(f, x.actor, PARKED, at)?;
        let out: u16 = int3(f, &count_name(x.actor), 0, at, at, park)?;
        at = is_enum(f, BUTTON, x.slot, at, out)?;
    }
    let mut gate: u16 = then;
    for x in ORDER.iter().rev() {
        let out: u16 = int3(f, &count_name(x.actor), 0, then, then, at)?;
        gate = is_enum(f, BUTTON, x.slot, out, gate)?;
    }
    Ok(gate)
}

/// After trade item `k` was handed in, the item button still names it and would show nothing.
/// A trade item that is out goes on the button; with none out, the nearest parked one before `k`
/// in [`ORDER`] (wrapping: the one a forward switch came from) comes out and goes on the button.
pub fn fall_back(f: &mut Flow, k: usize, then: u16) -> Res<u16> {
    let (pa, px) = action(f, PLAYER, "EventPlayerSetSelectedItem")?;
    let n: usize = ORDER.len();
    let mut un: u16 = then;
    for d in (1..n).rev() {
        let j: usize = (k + n - d) % n;
        let select: u16 = push(f, Kind::Action { next: then, actor: pa, act: px, params: cont(vec![("Item", Val::Int(ORDER[j].index)), ("IsWaitEnd", Val::Bool(1))]) })?;
        let show: u16 = set_count(f, ORDER[j].actor, 1, select)?;
        un = int3(f, &count_name(ORDER[j].actor), 0, show, un, un)?;
    }
    let mut check: u16 = un;
    for j in (0..n).rev().filter(|&j| j != k) {
        let select: u16 = push(f, Kind::Action { next: then, actor: pa, act: px, params: cont(vec![("Item", Val::Int(ORDER[j].index)), ("IsWaitEnd", Val::Bool(1))]) })?;
        check = int3(f, &count_name(ORDER[j].actor), 0, check, check, select)?;
    }
    Ok(check)
}

fn is_enum(f: &mut Flow, data: &str, value: &str, yes: u16, no: u16) -> Res<u16> {
    ask(f, DATA, "EventQueryIsGameDataEnum", cont(vec![("Index", Val::Int(-1)), ("GameDataName", s(data)), ("Value", s(value))]), yes, no)
}

fn exp(f: &mut Flow, name: &str, yes: u16, no: u16) -> Res<u16> {
    ask(f, DATA, "EventQueryIsGameDataBoolExp", cont(vec![("GameDataName", s(name))]), yes, no)
}

fn after_exp(step: &str) -> String {
    format!("SubEvent_GetSwordBigGoron_IsAfter_{step}_Exp")
}

fn request(f: &mut Flow, back: bool, step: &str, next: u16) -> Res<u16> {
    let (a, x) = action(f, CHALLENGE, if back { ROLLBACK } else { PROGRESS })?;
    let key: &str = if back { "ChallengeStep" } else { "StepName" };
    let p: Option<Cont> = cont(vec![
        ("ChallengeName", s(QUEST)),
        ("IsIgnoreCompletedChallenge", Val::Bool(0)),
        ("IsIgnoreChallengeProgressUI", Val::Bool(1)),
        (key, s(step)),
        ("IsWaitEnd", Val::Bool(1)),
    ]);
    push(f, Kind::Action { next, actor: a, act: x, params: p })
}

/// Appends the settle and returns its first event, which ends at `then`: once the quest is past
/// Ready, the first held trade item of [`ORDER`] that has a step moves the quest to that step
/// (back or forward); with none held the quest stays.
pub fn settle(f: &mut Flow, then: u16) -> Res<u16> {
    let mut at: u16 = then;
    for x in ORDER.iter().rev() {
        let Some(step) = x.step else { continue };
        let fwd: u16 = request(f, false, step, then)?;
        let back: u16 = request(f, true, step, then)?;
        let dir: u16 = exp(f, &after_exp(step), back, fwd)?;
        let there: u16 = is_enum(f, STEP, step, then, dir)?;
        at = holds(f, x.actor, there, at)?;
    }
    exp(f, STARTED, at, then)
}

fn wait(f: &mut Flow, next: u16) -> Res<u16> {
    let (a, x) = match f.actors.iter().position(|a| a.sub.is_empty() && a.actions.iter().any(|s| s == "EventWait")) {
        Some(i) => {
            let j: usize = f.actors[i].actions.iter().position(|s| s == "EventWait").unwrap_or(0);
            (idx(i)?, idx(j)?)
        }
        None => action(f, "GameSystemActor", "EventWait")?,
    };
    push(f, Kind::Action { next, actor: a, act: x, params: cont(vec![("Frames", Val::Int(1))]) })
}

/// Appends the button switch and returns its first event, ending at `then`: an adult whose item
/// button holds an adult trade item gets the next held one of [`ORDER`] (wrapping): it comes out
/// (count 1), goes on the button, and the shown one is parked. Then a settle.
pub fn cycle(f: &mut Flow, then: u16) -> Res<u16> {
    let end: u16 = settle(f, then)?;
    let (pa, px) = action(f, PLAYER, "EventPlayerSetSelectedItem")?;
    let n: usize = ORDER.len();
    let mut sel: u16 = end;
    for i in (0..n).rev() {
        // The shown item is parked once the next one is out and on the button.
        let park: u16 = set_count(f, ORDER[i].actor, PARKED, end)?;
        let mut look: u16 = end;
        for k in (1..n).rev() {
            let j: usize = (i + k) % n;
            let set: u16 = push(f, Kind::Action { next: park, actor: pa, act: px, params: cont(vec![("Item", Val::Int(ORDER[j].index)), ("IsWaitEnd", Val::Bool(1))]) })?;
            let out: u16 = set_count(f, ORDER[j].actor, 1, set)?;
            look = holds(f, ORDER[j].actor, out, look)?;
        }
        sel = is_enum(f, BUTTON, ORDER[i].slot, look, sel)?;
    }
    ask(f, PLAYER, "EventQueryPlayerIsChild", None, then, sel)
}

fn is_request(f: &Flow, e: &Event) -> Option<String> {
    let Kind::Action { actor, act, .. } = &e.kind else { return None };
    let a: &str = f.actors.get(*actor as usize).and_then(|x| x.actions.get(*act as usize)).map(|s| s.as_str())?;
    if a != PROGRESS || crate::open_world::guard::text(&e.kind, "ChallengeName") != Some(QUEST) {
        return None;
    }
    crate::open_world::guard::text(&e.kind, "StepName").map(|s| s.to_string())
}

/// Every trade quest step request of the flow becomes a settle; Anju's first gift request stays
/// and is followed by one. Returns how many requests it touched.
pub fn npc(f: &mut Flow) -> Res<usize> {
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(Error::Msg(format!("trade shuffle: {} is already patched", f.name)));
    }
    let hits: Vec<(usize, String)> = f.events.iter().enumerate().filter_map(|(i, e)| is_request(f, e).map(|s| (i, s))).collect();
    for (i, step) in &hits {
        let next: u16 = match &f.events[*i].kind {
            Kind::Action { next, .. } => *next,
            _ => continue,
        };
        let then: u16 = if next == NONE { wait(f, NONE)? } else { next };
        let first: u16 = settle(f, then)?;
        if step == KEEP {
            if let Kind::Action { next, .. } = &mut f.events[*i].kind {
                *next = first;
            }
        } else {
            let k: Kind = f.events[first as usize].kind.clone();
            f.events[*i].kind = k;
        }
    }
    // Some scenes take the item after their step request (the Eye Drops on the trail): every take
    // of a trade item is followed by a settle too, after another trade item took its place on the
    // button.
    let takes: Vec<usize> = (0..f.events.len()).filter(|&i| takes_trade(f, &f.events[i])).collect();
    for i in &takes {
        let next: u16 = match &f.events[*i].kind {
            Kind::Action { next, .. } => *next,
            _ => continue,
        };
        let Some(k) = taken(&f.events[*i]) else { continue };
        let then: u16 = if next == NONE { wait(f, NONE)? } else { next };
        let settled: u16 = settle(f, then)?;
        let first: u16 = fall_back(f, k, settled)?;
        if let Kind::Action { next, .. } = &mut f.events[*i].kind {
            *next = first;
        }
    }
    Ok(hits.len() + takes.len())
}

/// The trade item a pouch action names, as its place in [`ORDER`].
fn taken(e: &Event) -> Option<usize> {
    match e.kind.params()?.get("Actor")? {
        Val::Str(s) => ORDER.iter().position(|x| x.actor == s),
        _ => None,
    }
}

fn takes_trade(f: &Flow, e: &Event) -> bool {
    let Kind::Action { actor, act, params, .. } = &e.kind else { return false };
    let a: Option<&str> = f.actors.get(*actor as usize).and_then(|x| x.actions.get(*act as usize)).map(|s| s.as_str());
    if a != Some("EventTriggerAddOrRemovePouchContent") {
        return false;
    }
    let p: Option<&Cont> = params.as_ref();
    let n: bool = matches!(p.and_then(|c| c.get("Num")), Some(Val::Int(n)) if *n < 0);
    let who: bool = matches!(p.and_then(|c| c.get("Actor")), Some(Val::Str(s)) if item(s));
    n && who
}

pub fn npc_bf(b: &mut Bf, flow: &str) -> Res<usize> {
    let f: &mut Flow = crate::open_world::guard::flow_mut(b, flow)?;
    npc(f)
}

/// ShowItem:EntryPoint0 ends with the button switch.
pub fn show(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = crate::open_world::guard::flow_mut(b, SHOW_FLOW)?;
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(Error::Format("trade shuffle: ShowItem is already patched"));
    }
    let main: u16 = f.entries.iter().find(|e| e.name == SHOW_ENTRY).map(|e| e.main).ok_or(Error::Format("trade shuffle: ShowItem has no EntryPoint0"))?;
    // The entry ends on its NpcCommon:ShowItem_Close call (a fork, its join, then the close).
    let mut ends: Vec<usize> = Vec::new();
    let mut seen: Vec<bool> = vec![false; f.events.len()];
    let mut todo: Vec<u16> = vec![main];
    while let Some(i) = todo.pop() {
        if i == NONE || seen[i as usize] {
            continue;
        }
        seen[i as usize] = true;
        match &f.events[i as usize].kind {
            Kind::Action { next, .. } | Kind::Join { next } => todo.push(*next),
            Kind::Sub { next, entry, .. } => {
                if *next == NONE && entry == "ShowItem_Close" {
                    ends.push(i as usize);
                }
                todo.push(*next);
            }
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
            Kind::Fork { join, .. } => todo.push(*join),
            Kind::Label { .. } => {}
        }
    }
    let [end] = ends.as_slice() else {
        return Err(Error::Msg(format!("trade shuffle: EntryPoint0 of ShowItem should end on one ShowItem_Close, found {}", ends.len())));
    };
    let end: usize = *end;
    let stop: u16 = wait(f, NONE)?;
    let first: u16 = cycle(f, stop)?;
    if let Kind::Sub { next, .. } = &mut f.events[end].kind {
        *next = first;
    }
    Ok(())
}

/// The item gets: right after GetPouchContent's pouch add, a new trade item is parked when the
/// button's own trade item is out, then a settle.
pub fn get(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = crate::open_world::guard::flow_mut(b, GET_FLOW)?;
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(Error::Format("trade shuffle: GetPouchContent is already patched"));
    }
    let (i, name) = GET_ADD;
    let then: u16 = match f.events.get(i) {
        Some(Event { name: n, kind: Kind::Action { next, .. } }) if n == name => *next,
        _ => return Err(Error::Msg(format!("trade shuffle: event {i} of GetPouchContent is not the pouch add {name}, the game files differ"))),
    };
    let then: u16 = if then == NONE { wait(f, NONE)? } else { then };
    let settled: u16 = settle(f, then)?;
    let first: u16 = park_others(f, settled)?;
    if let Kind::Action { next, .. } = &mut f.events[i].kind {
        *next = first;
    }
    Ok(())
}

/// The egg hatch runs only for a held Pocket Egg and no held Pocket Cucco (else the quest flow
/// would hand out a free Pocket Cucco); otherwise it settles instead. A parked egg comes out
/// first, so the hatch takes it from a count of 1.
pub fn egg(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = crate::open_world::guard::flow_mut(b, QUEST)?;
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(Error::Format("trade shuffle: the quest flow is already patched"));
    }
    let k: usize = f.entries.iter().position(|e| e.name == EGG_ENTRY).ok_or(Error::Format("trade shuffle: the quest flow has no egg hatch entry"))?;
    let hatch: u16 = f.entries[k].main;
    let stop: u16 = wait(f, NONE)?;
    let skip: u16 = settle(f, stop)?;
    let out: u16 = set_count(f, ORDER[0].actor, 1, hatch)?;
    let cucco: u16 = holds(f, ORDER[1].actor, skip, out)?;
    let first: u16 = holds(f, ORDER[0].actor, cucco, skip)?;
    f.entries[k].main = first;
    Ok(())
}

/// The pause menu's trade button lists the adult trade items earliest first, so the item the menu
/// shows (the first held one) is the earliest held. The child items keep their place after them.
pub fn pouch_order(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    let items: &mut Vec<Node> = y.root.get_mut("ItemArray").and_then(Node::as_arr_mut).ok_or(Error::Format("trade shuffle: the pouch UI table has no ItemArray"))?;
    let e: &mut Node = items
        .iter_mut()
        .find(|e| e.get("BtnName").and_then(Node::as_str) == Some(TRADE_BUTTON))
        .ok_or(Error::Format("trade shuffle: the pouch UI table has no trade button"))?;
    let list: &mut Vec<Node> = e.get_mut("Actor").and_then(Node::as_arr_mut).ok_or(Error::Format("trade shuffle: the trade button has no actor list"))?;
    let name = |n: &Node| n.get("ActorShort").and_then(Node::as_str).map(|s| s.to_string()).unwrap_or_default();
    let mut adult: Vec<Node> = Vec::new();
    for x in ORDER.iter() {
        let at: usize = list.iter().position(|n| name(n) == x.actor).ok_or_else(|| Error::Msg(format!("trade shuffle: the trade button does not list {}", x.actor)))?;
        adult.push(list.remove(at));
    }
    adult.append(list);
    *list = adult;
    y.write()
}

/// The pool: each shuffled trade spot adds its chain item and one junk item becomes the Pocket
/// Cucco (the egg hatch gives it in the vanilla chain). Returns the notes.
pub fn pool(d: &Data, w: &World, free: &[LocId], base: &mut Placement) -> Res<Vec<String>> {
    let mut n: usize = 0;
    for l in free {
        let name: &str = &w.locs[*l].name;
        if !crate::flow_extra::trade::is(name) {
            continue;
        }
        let v: &str = d.flow.by_name(name).map(|x| x.vanilla.as_str()).ok_or_else(|| Error::Msg(format!("trade shuffle: {name} is not a flow location")))?;
        let it: ItemId = w.item(v).ok_or_else(|| Error::Msg(format!("trade shuffle: {v} is not a logic item")))?;
        base.put(*l, it);
        n += 1;
    }
    if n == 0 {
        return Ok(Vec::new());
    }
    let cucco: ItemId = w.item(HATCHED).ok_or_else(|| Error::Msg(format!("trade shuffle: {HATCHED} is not a logic item")))?;
    let spot: Option<LocId> = free.iter().copied().find(|l| !crate::flow_extra::trade::is(&w.locs[*l].name) && base.get(*l).is_some_and(|i| JUNK.contains(&w.items.names[i].as_str())));
    let Some(spot) = spot else {
        return Err(Error::Format("trade shuffle: no junk item left in the pool for the Pocket Cucco"));
    };
    base.put(spot, cucco);
    Ok(vec![format!("Adult trade shuffle: {n} trade spots add their trade item to the pool, and one junk item became the {HATCHED}.")])
}

/// The junk items whose copies the trade spots and the Pocket Cucco take from the pool.
pub const JUNK: [&str; 3] = ["Rupees (5)", "Rupee (1)", "Rupees (20)"];

/// The seed build's actor count per placed item: the trade spots joined the pool with their trade
/// item where the vanilla data has junk, and one junk became the Pocket Cucco ([]). A trade
/// item is its own actor; a junk item drops the copies it gave up.
pub fn fit_actors(d: &Data, item: &str, want: usize, acts: &mut Vec<String>) {
    if let Some(x) = by_name(item) {
        acts.clear();
        acts.extend(std::iter::repeat_n(x.actor.to_string(), want));
        return;
    }
    if JUNK.contains(&item) && acts.len() > want {
        let _ = d;
        acts.truncate(want);
    }
}

/// Stages every flow edit and the menu order on top of what the build already wrote (`work`).
pub fn stage(rf: &mut Romfs, late: &mut Vec<Out>, work: &std::path::Path) -> Res<()> {
    for flow in NPCS {
        crate::story_skip::stage_late(rf, late, work, flow, |b| npc_bf(b, flow).map(|_| ()))?;
    }
    crate::story_skip::stage_late(rf, late, work, SHOW_FLOW, show)?;
    crate::story_skip::stage_late(rf, late, work, GET_FLOW, get)?;
    crate::story_skip::stage_late(rf, late, work, QUEST, egg)?;
    late_plain(rf, late, work, POUCH_UI, pouch_order)?;
    late_plain(rf, late, work, menu::FILE, menu::patch)?;
    for l in crate::hints::languages(rf)? {
        crate::story_skip::stage_late_file(rf, late, work, &l, text::archive)?;
    }
    Ok(())
}

/// Edits a plain (uncompressed) file: the late copy when one is staged, else the work copy or
/// the game's.
fn late_plain(rf: &Romfs, late: &mut Vec<Out>, work: &std::path::Path, path: &str, f: impl FnOnce(&[u8]) -> Res<Vec<u8>>) -> Res<()> {
    if let Some(o) = late.iter_mut().find(|o| o.path == path) {
        o.z = f(&o.z)?;
        o.new = o.z.len();
        return Ok(());
    }
    let have = |p: &str| -> Option<Vec<u8>> { std::fs::read(p.split('/').fold(work.to_path_buf(), |a, s| a.join(s))).ok() };
    late.push(crate::qol::edit_plain(rf, &have, path, f)?);
    Ok(())
}

pub mod menu;
pub mod text;

#[cfg(test)]
mod tests;
