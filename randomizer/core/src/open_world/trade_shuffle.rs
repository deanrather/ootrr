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
//! - The Pocket Egg only hatches when it is held and no Pocket Cucco is ([`EGG_ENTRY`]). It is not
//!   in the pool: the Pocket Cucco stands in for it ([`pool`]).
//! - The slot's list is earliest first ([`pouch_order`]).
//! - Each age has its own trade items ([`ADULT`], [`CHILD`]). The slot lists the adult ones
//!   first and draws the first one that is out whatever Link's age, so an adult trade item found
//!   as a child hid the Weird Egg, the cucco and Zelda's Letter. While Link is a child every adult
//!   trade item is parked: a get parks it at once ([`get`]), the game's own equipment swap of an
//!   era change parks them all and brings the adult's one back out on the way to adulthood
//!   ([`era`]), and the menu keeps the rule every frame. The child's three items follow the same
//!   one-out rule with the child's item button, so the child switches between them the same two
//!   ways, and the guard and Talon put the other one on the button after their take
//!   ([`KID_NPCS`]). A parked Weird Egg comes out before its hatch takes it ([`kid_hatch`]).

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

/// The child's trade items, in the order the game hands them out: same slot, the child's button.
/// None of them has a step of the adult quest.
pub const KID: [Trade; 3] = [
    t("Weird Egg", "Item_WeirdEgg", "WeirdEgg", 31, None),
    t("Wake-Up Cucco", "Item_AlarmCucco", "AlarmCucco", 32, None),
    t("Zeldas Letter", "Item_ZeldaLetter", "ZeldaLetter", 33, None),
];
pub const KID_BUTTON: &str = "PlayerStatus.AssignableItem.Child";
/// True while Link is a child.
pub const AGE: &str = "Sequence_IsChildhood";

/// The trade items of one age and that age's item button. Only one item of a group is out at a
/// time; while Link is a child the whole adult group is parked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    pub items: &'static [Trade],
    pub button: &'static str,
}

pub const ADULT: Group = Group { items: &ORDER, button: BUTTON };
pub const CHILD: Group = Group { items: &KID, button: KID_BUTTON };

/// The trade item that the egg hatch hands out. In the pool it stands in for the egg.
pub const HATCHED: &str = "Pocket Cucco";
/// Not in the pool: the Pocket Cucco takes its place, so the two never both turn up. A plando or
/// an older seed can still hold one, and it hatches as before ([`egg`]).
pub const EGG: &str = "Pocket Egg";

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
/// The game's equipment swap, run by every era change right before Link's age flips.
pub const ERA_FLOW: &str = "ReplaceEquipmentForEraChange";
/// The child's quest flow, whose hatch takes the Weird Egg.
pub const KID_QUEST: &str = "Goto_HyruleCastle";
/// The flows that take a child trade item: the Kakariko gate guard (Zelda's Letter) and Talon's
/// wake up scene (the cucco).
pub const KID_NPCS: [&str; 2] = ["Npc_Hylia_Soldier_003", "Dm_HrlCastle_0020"];

pub const POUCH_UI: &str = "UI/Pouch/PouchTable/PouchTable.game__ui__PouchTable.bgyml";
pub const TRADE_BUTTON: &str = "Pa_ItemIcon_23";

pub const NOTE: &str = "Adult trade shuffle: the ten adult trade items, Pocket Cucco to Claim Check, are shuffled and the ten trade hand ins give only their shuffled item (Anju's first gift is in the pool as the Pocket Cucco, there is no Pocket Egg). Every trade item you find stays until its NPC takes it, so you can hand them over in any order. The quest follows the earliest trade item you hold (Grog and Fado appear for it), and only the trade item you picked shows in the menu, the L/R list and on the item button. To switch, press A on the menu's trade slot again, or show the trade item where nobody takes it: the next one you hold comes up. Each age sees its own trade items: as a child the slot holds the Weird Egg, the cucco and Zelda's Letter (switched the same way) and the adult trade items wait for adult Link.";

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

/// After a new trade item came in: when the group's item button holds one of its trade items and
/// that one is out, every other out trade item of the group is parked, so the slot keeps showing
/// the button's item. With another item on the button nothing is parked (the slot then shows the
/// earliest one that is out).
pub fn park_others(f: &mut Flow, g: &Group, then: u16) -> Res<u16> {
    let mut at: u16 = then;
    for x in g.items.iter().rev() {
        let park: u16 = set_count(f, x.actor, PARKED, at)?;
        let out: u16 = int3(f, &count_name(x.actor), 0, at, at, park)?;
        at = is_enum(f, g.button, x.slot, at, out)?;
    }
    let mut gate: u16 = then;
    for x in g.items.iter().rev() {
        let out: u16 = int3(f, &count_name(x.actor), 0, then, then, at)?;
        gate = is_enum(f, g.button, x.slot, out, gate)?;
    }
    Ok(gate)
}

/// After trade item `k` of the group was handed in, the item button still names it and would show
/// nothing. A trade item that is out goes on the button; with none out, the nearest parked one
/// before `k` in the group (wrapping: the one a forward switch came from) comes out and goes on
/// the button.
pub fn fall_back(f: &mut Flow, g: &Group, k: usize, then: u16) -> Res<u16> {
    let (pa, px) = action(f, PLAYER, "EventPlayerSetSelectedItem")?;
    let n: usize = g.items.len();
    let mut un: u16 = then;
    for d in (1..n).rev() {
        let j: usize = (k + n - d) % n;
        let select: u16 = push(f, Kind::Action { next: then, actor: pa, act: px, params: cont(vec![("Item", Val::Int(g.items[j].index)), ("IsWaitEnd", Val::Bool(1))]) })?;
        let show: u16 = set_count(f, g.items[j].actor, 1, select)?;
        un = int3(f, &count_name(g.items[j].actor), 0, show, un, un)?;
    }
    let mut check: u16 = un;
    for j in (0..n).rev().filter(|&j| j != k) {
        let select: u16 = push(f, Kind::Action { next: then, actor: pa, act: px, params: cont(vec![("Item", Val::Int(g.items[j].index)), ("IsWaitEnd", Val::Bool(1))]) })?;
        check = int3(f, &count_name(g.items[j].actor), 0, check, check, select)?;
    }
    Ok(check)
}

/// Parks every trade item of the group that is out.
pub fn park_all(f: &mut Flow, g: &Group, then: u16) -> Res<u16> {
    let mut at: u16 = then;
    for x in g.items.iter().rev() {
        let park: u16 = set_count(f, x.actor, PARKED, at)?;
        at = int3(f, &count_name(x.actor), 0, at, at, park)?;
    }
    Ok(at)
}

/// Brings one trade item of the group out, for the age that is about to use it: the one its item
/// button names when that one is parked; else, with none out, the earliest parked one. The button
/// is left alone (it belongs to the age, and this runs before the age flips).
pub fn bring_out(f: &mut Flow, g: &Group, then: u16) -> Res<u16> {
    let mut first: u16 = then;
    for x in g.items.iter().rev() {
        let show: u16 = set_count(f, x.actor, 1, then)?;
        first = int3(f, &count_name(x.actor), 0, show, first, first)?;
    }
    let mut none_out: u16 = first;
    for x in g.items.iter().rev() {
        none_out = int3(f, &count_name(x.actor), 0, none_out, none_out, then)?;
    }
    let mut gate: u16 = none_out;
    for x in g.items.iter().rev() {
        let show: u16 = set_count(f, x.actor, 1, then)?;
        let own: u16 = int3(f, &count_name(x.actor), 0, show, none_out, then)?;
        gate = is_enum(f, g.button, x.slot, own, gate)?;
    }
    Ok(gate)
}

/// Is Link a child? Case 0 is yes.
fn child(f: &mut Flow, yes: u16, no: u16) -> Res<u16> {
    ask(f, DATA, "EventQueryIsGameDataBool", cont(vec![("Index", Val::Int(-1)), ("GameDataName", s(AGE))]), yes, no)
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

/// The switch within one group, ending at `end`: with trade item i of the group on its item
/// button, the next held one (wrapping) comes out (count 1), goes on the button, and the shown
/// one is parked. With no trade item of the group on the button nothing changes.
fn turn(f: &mut Flow, g: &Group, end: u16) -> Res<u16> {
    let (pa, px) = action(f, PLAYER, "EventPlayerSetSelectedItem")?;
    let n: usize = g.items.len();
    let mut sel: u16 = end;
    for i in (0..n).rev() {
        // The shown item is parked once the next one is out and on the button.
        let park: u16 = set_count(f, g.items[i].actor, PARKED, end)?;
        let mut look: u16 = end;
        for k in (1..n).rev() {
            let j: usize = (i + k) % n;
            let set: u16 = push(f, Kind::Action { next: park, actor: pa, act: px, params: cont(vec![("Item", Val::Int(g.items[j].index)), ("IsWaitEnd", Val::Bool(1))]) })?;
            let out: u16 = set_count(f, g.items[j].actor, 1, set)?;
            look = holds(f, g.items[j].actor, out, look)?;
        }
        sel = is_enum(f, g.button, g.items[i].slot, look, sel)?;
    }
    Ok(sel)
}

/// Appends the button switch and returns its first event, ending at `then`: Link's own trade
/// items take turns ([`turn`]): the adult's, followed by a settle, or the child's.
pub fn cycle(f: &mut Flow, then: u16) -> Res<u16> {
    let end: u16 = settle(f, then)?;
    let adult: u16 = turn(f, &ADULT, end)?;
    let kid: u16 = turn(f, &CHILD, then)?;
    ask(f, PLAYER, "EventQueryPlayerIsChild", None, kid, adult)
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
        let first: u16 = fall_back(f, &ADULT, k, settled)?;
        if let Kind::Action { next, .. } = &mut f.events[*i].kind {
            *next = first;
        }
    }
    Ok(hits.len() + takes.len())
}

/// After every take of a child trade item in the flow, another child trade item goes on the
/// button ([`fall_back`]). Returns how many takes it followed.
pub fn kid_npc(f: &mut Flow) -> Res<usize> {
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(Error::Msg(format!("trade shuffle: {} is already patched", f.name)));
    }
    let takes: Vec<(usize, usize)> = (0..f.events.len()).filter_map(|i| removes(f, &f.events[i], &CHILD).map(|k| (i, k))).collect();
    if takes.is_empty() {
        return Err(Error::Msg(format!("trade shuffle: {} takes no child trade item, the game files differ", f.name)));
    }
    for (i, k) in &takes {
        let next: u16 = match &f.events[*i].kind {
            Kind::Action { next, .. } => *next,
            _ => continue,
        };
        let then: u16 = if next == NONE { wait(f, NONE)? } else { next };
        let first: u16 = fall_back(f, &CHILD, *k, then)?;
        if let Kind::Action { next, .. } = &mut f.events[*i].kind {
            *next = first;
        }
    }
    Ok(takes.len())
}

pub fn kid_npc_bf(b: &mut Bf, flow: &str) -> Res<usize> {
    let f: &mut Flow = crate::open_world::guard::flow_mut(b, flow)?;
    kid_npc(f)
}

/// The trade item a pouch action names, as its place in [`ORDER`].
fn taken(e: &Event) -> Option<usize> {
    match e.kind.params()?.get("Actor")? {
        Val::Str(s) => ORDER.iter().position(|x| x.actor == s),
        _ => None,
    }
}

fn takes_trade(f: &Flow, e: &Event) -> bool {
    removes(f, e, &ADULT).is_some()
}

/// The place in the group of the trade item this event takes from the pouch.
fn removes(f: &Flow, e: &Event, g: &Group) -> Option<usize> {
    let Kind::Action { actor, act, params, .. } = &e.kind else { return None };
    let a: Option<&str> = f.actors.get(*actor as usize).and_then(|x| x.actions.get(*act as usize)).map(|s| s.as_str());
    if a != Some("EventTriggerAddOrRemovePouchContent") {
        return None;
    }
    let p: &Cont = params.as_ref()?;
    if !matches!(p.get("Num"), Some(Val::Int(n)) if *n < 0) {
        return None;
    }
    match p.get("Actor")? {
        Val::Str(s) => g.items.iter().position(|x| x.actor == s),
        _ => None,
    }
}

pub fn npc_bf(b: &mut Bf, flow: &str) -> Res<usize> {
    let f: &mut Flow = crate::open_world::guard::flow_mut(b, flow)?;
    npc(f)
}

/// ShowItem:EntryPoint0 ends with the button switch.
pub fn show(b: &mut Bf) -> Res<()> {
    show_with(b, false)
}

fn show_with(b: &mut Bf, kids_only: bool) -> Res<()> {
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
    let first: u16 = if kids_only {
        let kid: u16 = turn(f, &CHILD, stop)?;
        ask(f, PLAYER, "EventQueryPlayerIsChild", None, kid, stop)?
    } else {
        cycle(f, stop)?
    };
    if let Kind::Sub { next, .. } = &mut f.events[end].kind {
        *next = first;
    }
    Ok(())
}

/// The child switch alone (no adult trade shuffle): showing a child trade item where nobody takes
/// it puts the next held one on the child's button; an adult's show stays the game's.
pub fn show_kid(b: &mut Bf) -> Res<()> {
    show_with(b, true)
}

/// The child's gets alone: a new child trade item is parked while the button's own is out, at
/// either age. Adult trade items stay the game's (one at a time, in the chain).
pub fn get_kid(b: &mut Bf) -> Res<()> {
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
    let first: u16 = park_others(f, &CHILD, then)?;
    if let Kind::Action { next, .. } = &mut f.events[i].kind {
        *next = first;
    }
    Ok(())
}

/// The child's switch between the Weird Egg, the cucco and Zelda's Letter without adult trade
/// shuffle: the same one-out rule, the A press on the menu's trade slot and the show-to-nobody
/// switch, the guard and Talon putting the other one on the button after their take, a parked egg
/// coming out for its hatch, and the menu hint on the three captions.
pub fn stage_kid(rf: &mut Romfs, late: &mut Vec<Out>, work: &std::path::Path) -> Res<()> {
    crate::story_skip::stage_late(rf, late, work, SHOW_FLOW, show_kid)?;
    crate::story_skip::stage_late(rf, late, work, GET_FLOW, get_kid)?;
    crate::story_skip::stage_late(rf, late, work, KID_QUEST, kid_hatch)?;
    for flow in KID_NPCS {
        crate::story_skip::stage_late(rf, late, work, flow, |b| kid_npc_bf(b, flow).map(|_| ()))?;
    }
    late_plain(rf, late, work, menu::FILE, menu::patch_kid)?;
    for l in crate::hints::languages(rf)? {
        crate::story_skip::stage_late_file(rf, late, work, &l, text::archive_kid)?;
    }
    Ok(())
}

/// The item gets, right after GetPouchContent's pouch add. A child parks every adult trade item
/// that is out, so a new one never shows in the child's slot; an adult parks a new adult trade
/// item when the button's own is out. The child's own trade items follow the same rule with the
/// child's button, at either age. Then a settle.
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
    let own: u16 = park_others(f, &CHILD, settled)?;
    let adult: u16 = park_others(f, &ADULT, own)?;
    let kid: u16 = park_all(f, &ADULT, own)?;
    let first: u16 = child(f, kid, adult)?;
    if let Kind::Action { next, .. } = &mut f.events[i].kind {
        *next = first;
    }
    Ok(())
}

/// The era change: the game's equipment swap runs right before Link's age flips, so its end sets
/// the trade slot up for the age to come. On the way to childhood every adult trade item is
/// parked and one of the child's comes out; on the way to adulthood one of the adult's comes out
/// (the adult button's own when it is held).
pub fn era(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = crate::open_world::guard::flow_mut(b, ERA_FLOW)?;
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(Error::Format("trade shuffle: the era change is already patched"));
    }
    let main: u16 = f.entries.iter().find(|e| e.name == ERA_FLOW).map(|e| e.main).ok_or(Error::Format("trade shuffle: the era change has no entry"))?;
    let mut ends: Vec<usize> = Vec::new();
    let mut seen: Vec<bool> = vec![false; f.events.len()];
    let mut todo: Vec<u16> = vec![main];
    while let Some(i) = todo.pop() {
        if i == NONE || seen[i as usize] {
            continue;
        }
        seen[i as usize] = true;
        match &f.events[i as usize].kind {
            Kind::Action { next, .. } => {
                if *next == NONE {
                    ends.push(i as usize);
                }
                todo.push(*next);
            }
            Kind::Switch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
            _ => return Err(Error::Format("trade shuffle: the era change is no longer a plain chain of checks and writes")),
        }
    }
    let [end] = ends.as_slice() else {
        return Err(Error::Msg(format!("trade shuffle: the era change should end on one event, found {}", ends.len())));
    };
    let end: usize = *end;
    let stop: u16 = wait(f, NONE)?;
    let to_adult: u16 = bring_out(f, &ADULT, stop)?;
    let kid: u16 = bring_out(f, &CHILD, stop)?;
    let to_child: u16 = park_all(f, &ADULT, kid)?;
    let first: u16 = child(f, to_adult, to_child)?;
    if let Kind::Action { next, .. } = &mut f.events[end].kind {
        *next = first;
    }
    Ok(())
}

/// The Weird Egg's hatch takes the egg from a count of 1: a parked egg comes out first, and with
/// no egg held (the hatch runs on the child quest's step, whoever holds the egg) nothing is
/// taken, so the count never drops below the parked value.
pub fn kid_hatch(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = crate::open_world::guard::flow_mut(b, KID_QUEST)?;
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(Error::Format("trade shuffle: the child quest flow is already patched"));
    }
    let egg: &Trade = &KID[0];
    let takes: Vec<usize> = (0..f.events.len()).filter(|&i| removes(f, &f.events[i], &CHILD) == Some(0)).collect();
    let [at] = takes.as_slice() else {
        return Err(Error::Msg(format!("trade shuffle: the child quest flow should take the Weird Egg once, found {}", takes.len())));
    };
    let at: usize = *at;
    let after: u16 = match &f.events[at].kind {
        Kind::Action { next, .. } => *next,
        _ => NONE,
    };
    let after: u16 = if after == NONE { wait(f, NONE)? } else { after };
    let take: u16 = idx(f.events.len())?;
    let kind: Kind = f.events[at].kind.clone();
    f.events.push(Event { name: format!("{PREFIX}{take}"), kind });
    let out: u16 = set_count(f, egg.actor, 1, take)?;
    let guard: u16 = int3(f, &count_name(egg.actor), 0, out, after, take)?;
    f.events[at].kind = f.events[guard as usize].kind.clone();
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

/// The pool: each shuffled trade spot adds its chain item, and Anju's first gift adds the Pocket
/// Cucco in place of the Pocket Egg ([`EGG`]): the egg is not in the pool, so nothing hatches
/// into a second Pocket Cucco. When Anju's first gift is not shuffled, one junk item becomes the
/// Pocket Cucco. Returns the notes.
pub fn pool(d: &Data, w: &World, free: &[LocId], base: &mut Placement) -> Res<Vec<String>> {
    let cucco: ItemId = w.item(HATCHED).ok_or_else(|| Error::Msg(format!("trade shuffle: {HATCHED} is not a logic item")))?;
    let mut n: usize = 0;
    let mut hatched: bool = false;
    for l in free {
        let name: &str = &w.locs[*l].name;
        if !crate::flow_extra::trade::is(name) {
            continue;
        }
        let v: &str = d.flow.by_name(name).map(|x| x.vanilla.as_str()).ok_or_else(|| Error::Msg(format!("trade shuffle: {name} is not a flow location")))?;
        let it: ItemId = if v == EGG {
            hatched = true;
            cucco
        } else {
            w.item(v).ok_or_else(|| Error::Msg(format!("trade shuffle: {v} is not a logic item")))?
        };
        base.put(*l, it);
        n += 1;
    }
    if n == 0 {
        return Ok(Vec::new());
    }
    if hatched {
        return Ok(vec![format!("Adult trade shuffle: {n} trade spots add their trade item to the pool. Anju's first gift adds the {HATCHED}: there is no {EGG}.")]);
    }
    let spot: Option<LocId> = free.iter().copied().find(|l| !crate::flow_extra::trade::is(&w.locs[*l].name) && base.get(*l).is_some_and(|i| JUNK.contains(&w.items.names[i].as_str())));
    let Some(spot) = spot else {
        return Err(Error::Format("trade shuffle: no junk item left in the pool for the Pocket Cucco"));
    };
    base.put(spot, cucco);
    Ok(vec![format!("Adult trade shuffle: {n} trade spots add their trade item to the pool, and one junk item became the {HATCHED}. There is no {EGG}.")])
}

/// The junk items whose copies the trade spots and the Pocket Cucco take from the pool.
pub const JUNK: [&str; 3] = ["Rupees (5)", "Rupee (1)", "Rupees (20)"];

/// The seed build's actor count per placed item: the trade spots joined the pool with their trade
/// item where the vanilla data has junk ([`pool`]). A trade item is its own actor; a junk item
/// drops the copies it gave up.
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
    crate::story_skip::stage_late(rf, late, work, ERA_FLOW, era)?;
    crate::story_skip::stage_late(rf, late, work, KID_QUEST, kid_hatch)?;
    for flow in KID_NPCS {
        crate::story_skip::stage_late(rf, late, work, flow, |b| kid_npc_bf(b, flow).map(|_| ()))?;
    }
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
