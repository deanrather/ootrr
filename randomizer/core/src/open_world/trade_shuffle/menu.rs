//! The adult trade slot in the item menu. The slot draws the first trade item of its list
//! (`UI/Pouch/PouchTable`) whose pouch count is above 0, whatever is on the item button, and it
//! picks when the items page is built (on opening the menu and on entering the tab). The L/R list
//! and the HUD follow the same pouch. So only one adult trade item is ever out (count 1), the one
//! the player picked, and the other held ones are parked at [`PARKED`] (see the parent module:
//! the flows park and unpark on gets, hand ins and the show-to-nobody switch).
//!
//! This module is the menu's part, in its own sequence graph (`Sequence/PlusMenu.module`), which
//! reads the controller every frame and can write game data (a pouch count is the game data int
//! `PouchData.<actor>.StockNum`, read live by the pouch):
//!
//! - Keeping ([`keep`]), every frame for an adult: with a held trade item on the item button, that
//!   one is out and every other one that is out is parked; with none on the button and none out,
//!   the earliest parked one comes out; a handed-in item still named by the button is replaced
//!   by a held one. A change asks for a redraw (a save from before this
//!   rule, or a trade item picked up off the ground, has several out when the page is built).
//! - Switching ([`switch`]), on the item tab (category [`ITEMS`]): an A press while trade item k is
//!   on the button starts a short wait ([`pend`]`(k)`, counted up by one per frame for [`WAIT`]
//!   frames) that lets the menu assign whatever the press picked. After the wait, if a trade item
//!   is on the button (the press was on the trade slot), the next held trade item after k goes on
//!   the button. The frame after, the keeping swaps which one is out, and the item screen is
//!   closed, left closed for a few frames and opened again the way the menu itself closes and
//!   opens ([`redraw`]), so the page is built again and the slot shows the new item.
//!
//! Counts are only ever tested for an exact value (1 out, [`PARKED`], 0 not held): the compare
//! selector names its branches from its constant's side (Bigger: the constant is the bigger one),
//! which an earlier version of this module read the other way round and so never parked a thing.
//!
//! The state between frames lives in [`MEMO`], the main game's item-button hold counter: only the
//! main game reads it, and it resets it whenever no item button is held. [`idle`]`(k)` notes trade
//! item k on the button, [`idle`]`(ORDER.len())` none. Which grid slot the cursor is on is not
//! readable from a graph, so a press that changes nothing on the item tab (an empty slot) also
//! switches. Nothing is added to the menu's opening sequence: it waits for each of its steps to
//! finish, and a selector there froze the menu.

use super::{count_name, BUTTON, ORDER, PARKED};
use crate::ainb::build::{Spec, QUERY};
use crate::ainb::{audit, Ainb};
use crate::error::{Error, Res};

pub const FILE: &str = "Sequence/PlusMenu.module.ainb";
/// Borrowed scratch value: the main game's item-button hold counter. The menu graph never reads
/// it and the main game resets it whenever no item button is held.
pub const MEMO: &str = "Sequence_ShortCutOpenHoldCounter";
/// The item tab of the pouch menu: the game opens it after loading a save, and the action guides
/// that ask to equip gear open category 3 instead. Confirmed in game.
pub const ITEMS: i32 = 4;
pub const CHILD: &str = "Sequence_IsChildhood";
/// The item menu screen.
pub const SCREEN: &str = "PlusMenu_00";
/// The option the menu opens its screen with when it starts (coming back from Save and Options
/// it uses 2, which keeps what the screen showed).
pub const OPEN_OPTION: i32 = 0;
/// Frames between the press and the switch.
pub const WAIT: f32 = 8.0;
const IDLE: f32 = -6000.0;
const PEND: f32 = -7000.0;
/// The note asking for a redraw of the item screen: counted up for [`CLOSE_AFTER`] frames, then
/// the screen closes and the note moves to [`REOPEN`], counted up for [`OPEN_AFTER`] frames
/// before the screen opens again.
pub const REDRAW: f32 = -8000.0;
pub const REOPEN: f32 = -8100.0;
/// Frames from the counts changing to the close, and from the close to the open. Closing and
/// opening in one frame (as an earlier version did) replayed the screen's opening but kept the
/// page it had: the screen has to be gone for a moment, as when the menu is really left.
pub const CLOSE_AFTER: f32 = 2.0;
pub const OPEN_AFTER: f32 = 5.0;
const GAP: f32 = 20.0;

/// The note for trade item `k` on the button; `ORDER.len()` stands for no trade item.
pub fn idle(k: usize) -> f32 {
    IDLE - GAP * k as f32
}

/// The wait after a press with trade item `k` on the button.
pub fn pend(k: usize) -> f32 {
    PEND - GAP * k as f32
}

const TRIG: &str = "SeqQueryControllerTrigMaskChecker";
const DIRECTION: &str = "SeqQueryGetButtonInputDirection";
const DUMMY: &str = "SeqExecuteGenericDummyAction";
const GET_FLOAT: &str = "SeqQueryGetNonStructGameDataFloat";
const SET_FLOAT: &str = "SeqOneShotSetNonStructGameDataFloat";
const ADD_FLOAT: &str = "SeqExecuteAddNonStructGameDataFloat";
const GET_ENUM: &str = "SeqQueryGetNonStructGameDataEnum";
const SET_ENUM: &str = "SeqOneShotSetNonStructGameDataEnum";
const GET_INT: &str = "SeqQueryGetNonStructGameDataInt";
const SET_INT: &str = "SeqOneShotSetNonStructGameDataInt";
const GET_BOOL: &str = "SeqQueryGetNonStructGameDataBool";
const CLOSE: &str = "SeqOneShotCloseScreen";
const OPEN: &str = "SeqOneShotOpenScreen";
const HIT_TEST: &str = "SeqOneShotSetEnableScreenButtonHitTest";
const SET_CATEGORY: &str = "SeqOneShotSetCurrentMinusMenuCategory";
const CATEGORY: &str = "SeqQueryGetCurrentMinusMenuCategory";
const COMPARE: &str = "SeqSelectorMathCompareInt";
const IS_EQUAL: &str = "SeqQueryIsEqualU32";
const EITHER: &str = "SeqQueryLogicalDisjunction";
const HASH: &str = "SeqQueryCalcStringHashMurMur3";
const U32: &str = "const u32";
/// The default plug label of a switch, as the game's own switches name it.
const OTHER: &str = "その他";

/// Bool outputs of the trigger checker, in name order.
pub const A: i16 = 0;
pub const ZR: i16 = 15;

const S32: usize = 0;
const BOOL: usize = 1;
const F32: usize = 2;
const STR: usize = 3;
#[cfg(test)]
const PTR: usize = 5;
const S32_SWITCH: u16 = 1;
const SEQUENCE: u16 = 2;
const PARALLEL: u16 = 3;
const F32_SWITCH: u16 = 4;
const BOOL_SELECTOR: u16 = 7;
const STATE: u16 = 500;

fn yes(b: bool) -> Vec<u8> {
    vec![b as u8, 0, 0, 0]
}

fn int(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn text(a: &mut Ainb, s: &str) -> Vec<u8> {
    a.pool.add(s).to_le_bytes().to_vec()
}

#[cfg(test)]
fn kids(a: &Ainb, i: usize) -> Vec<usize> {
    a.params[i].group(2).iter().map(|p| p.a as usize).collect()
}

/// The position in `plugs` of the child plug named `name`.
fn child(a: &Ainb, i: usize, name: &str) -> Res<usize> {
    let p = &a.params[i];
    let b: usize = p.pl[2][1] as usize;
    p.group(2).iter().position(|x| a.s(x.b) == name).map(|j| b + j).ok_or(Error::Format("trade menu: a node lacks a child plug"))
}

fn source(a: &Ainb, i: usize, k: usize) -> Option<(usize, i16)> {
    let x = a.input(k, a.find_in(i, k, "Input")?)?;
    usize::try_from(x.src).ok().map(|s| (s, x.plug))
}

fn selector(a: &mut Ainb, seed: &str, src: usize, at: i16, yes_el: usize, no_el: usize) -> Res<usize> {
    let mut s: Spec = Spec::composite(BOOL_SELECTOR, seed)
        .prop(S32, "CalculateTiming", int(0))
        .prop(BOOL, "ChildFrameSync", yes(false))
        .prop(BOOL, "InputValue", yes(false))
        .prop(BOOL, "IsNoSelectWhenChildBusy", yes(false))
        .linked(BOOL, "Input", src, at)
        .kid(yes_el, "True")
        .kid(no_el, "False");
    s.link_extra = 8;
    a.add_node(&s)
}

fn parallel(a: &mut Ainb, seed: &str, kids: &[usize]) -> Res<usize> {
    let mut s: Spec = Spec::composite(PARALLEL, seed).prop(S32, "EndPolicy", int(0)).prop(S32, "ResultPolicy", int(0));
    for &k in kids {
        s = s.kid(k, "");
    }
    a.add_node(&s)
}

/// Runs its children one after the other, as the menu's own sequences do.
fn sequence(a: &mut Ainb, seed: &str, kids: &[usize]) -> Res<usize> {
    let mut s: Spec = Spec::composite(SEQUENCE, seed)
        .prop(S32, "BusyPolicy", int(0))
        .prop(S32, "NumLoop", int(0))
        .prop(S32, "PlayPolicy", int(0))
        .prop(S32, "ResultPolicy", int(0))
        .prop(BOOL, "IsDelayJudge", yes(false))
        .prop(BOOL, "IsLoop", yes(false))
        .prop(BOOL, "IsUpdateNextInFrame", yes(true));
    for &k in kids {
        s = s.kid(k, "");
    }
    a.add_node(&s)
}

/// An int compare selector, laid out like the game's own: children Bigger, Equal, Smaller, then
/// the plug of its linked input in group 5.
fn compare(a: &mut Ainb, seed: &str, value_a: i32, src: usize, bigger: usize, equal: usize, smaller: usize) -> Res<usize> {
    let s: Spec = Spec::node(COMPARE, seed)
        .prop(BOOL, "NoChangeWhenChildIsBusy", yes(false))
        .prop(BOOL, "SelectChildOnEnterOnly", yes(false))
        .constant(S32, "ValueA", int(value_a))
        .linked(S32, "ValueB", src, 0)
        .kid(bigger, "Bigger")
        .kid(equal, "Equal")
        .kid(smaller, "Smaller");
    let id: usize = a.add_node(&s)?;
    let p = &mut a.params[id];
    let link = p.plugs.remove(0);
    p.plugs.push(link);
    p.pl = [[0, 0], [0, 0], [3, 0], [0, 3], [0, 3], [1, 3], [0, 4], [0, 4], [0, 4], [0, 4]];
    Ok(id)
}

fn const_hash(a: &mut Ainb, seed: &str, s: &str) -> Res<usize> {
    let v: Vec<u8> = text(a, s);
    let mut n: Spec = Spec::node(HASH, seed).prop(BOOL, "IsCalcHashOnInit", yes(true)).prop(BOOL, "OutputOnlyOnEnter", yes(false)).constant(STR, "String", v).out_ptr("MurMur3Hash", U32);
    n.role = QUERY;
    a.add_node(&n)
}

fn set_memo(a: &mut Ainb, seed: &str, v: f32) -> Res<usize> {
    let name: Vec<u8> = text(a, MEMO);
    a.add_node(
        &Spec::node(SET_FLOAT, seed)
            .prop(BOOL, "ExecuteEveryFrame", yes(false))
            .idle(S32, "GameDataIndex", int(-1))
            .constant(F32, "Value", v.to_le_bytes().to_vec())
            .constant(STR, "GameDataFullName", name),
    )
}

/// A float switch over `src` (output 0): each case runs its child while the value is inside
/// [min, max]; otherwise `other` runs. Plug extras as in the game's own float switch.
fn float_switch(a: &mut Ainb, seed: &str, src: usize, cases: &[(f32, f32, usize)], other: usize) -> Res<usize> {
    let mut s: Spec = Spec::composite(F32_SWITCH, seed)
        .prop(S32, "CalculateTiming", int(0))
        .prop(S32, "PlugJudgeType", int(0))
        .prop(BOOL, "ChildFrameSync", yes(false))
        .prop(BOOL, "IsNoSelectWhenChildBusy", yes(false))
        .prop(F32, "InputValue", 0f32.to_le_bytes().to_vec())
        .linked(F32, "Input", src, 0);
    s.link_extra = 8;
    for &(_, _, k) in cases {
        s = s.kid(k, "");
    }
    s = s.kid(other, "");
    let id: usize = a.add_node(&s)?;
    let label: u32 = a.pool.add(OTHER);
    let b: usize = a.params[id].pl[2][1] as usize;
    for (j, &(lo, hi, _)) in cases.iter().enumerate() {
        let mut e: Vec<u8> = vec![0; 32];
        e[4..8].copy_from_slice(&lo.to_le_bytes());
        e[12..16].copy_from_slice(&hi.to_le_bytes());
        a.params[id].plugs[b + j].extra = e;
    }
    let mut e: Vec<u8> = vec![0; 32];
    e[4..8].copy_from_slice(&label.to_le_bytes());
    a.params[id].plugs[b + cases.len()].extra = e;
    Ok(id)
}


/// Runs `yes` while the int `src` reads exactly `value`, else `no`. Only the Equal branch tells
/// them apart, so the direction of Bigger and Smaller does not matter.
fn equals(a: &mut Ainb, seed: &str, value: i32, src: usize, yes_el: usize, no_el: usize) -> Res<usize> {
    compare(a, seed, value, src, no_el, yes_el, no_el)
}

/// The nodes every branch reads.
struct Parts {
    dummy: usize,
    memo: usize,
    /// "Trade item k is on the button", one per ORDER entry.
    on: Vec<usize>,
    /// "Some trade item is on the button".
    any: usize,
    /// Each trade item's pouch count.
    stock: Vec<usize>,
    /// Puts trade item k on the button and asks for a redraw.
    write: Vec<usize>,
    /// Puts trade item k on the button.
    plain: Vec<usize>,
    /// Asks for a redraw of the item screen.
    ask: usize,
    /// Ends a wait that switched nothing.
    clear: usize,
    child: usize,
}

fn parts(a: &mut Ainb) -> Res<Parts> {
    let dummy: usize = a.add_node(&Spec::node(DUMMY, "OOTRRTradeMenu#dummy"))?;
    let memo_name: Vec<u8> = text(a, MEMO);
    let memo: usize = a.add_node(
        &Spec::query(GET_FLOAT, "OOTRRTradeMenu#memo")
            .idle(S32, "GameDataIndex", int(-1))
            .idle(BOOL, "IsDirect", yes(false))
            .constant(STR, "GameDataFullName", memo_name)
            .out(F32, "Value"),
    )?;
    let button_name: Vec<u8> = text(a, BUTTON);
    let button: usize = a.add_node(
        &Spec::query(GET_ENUM, "OOTRRTradeMenu#button")
            .idle(S32, "GameDataIndex", int(-1))
            .constant(BOOL, "IsDirect", yes(true))
            .constant(STR, "GameDataFullName", button_name.clone())
            .out_ptr("Value", U32),
    )?;
    let ask: usize = set_memo(a, "OOTRRTradeMenu#ask_redraw", REDRAW)?;
    let clear: usize = set_memo(a, "OOTRRTradeMenu#no_switch", idle(ORDER.len()))?;
    let mut on: Vec<usize> = Vec::new();
    let mut stock: Vec<usize> = Vec::new();
    let mut write: Vec<usize> = Vec::new();
    let mut plain: Vec<usize> = Vec::new();
    for (k, t) in ORDER.iter().enumerate() {
        let h: usize = const_hash(a, &format!("OOTRRTradeMenu#slot{k}"), t.slot)?;
        on.push(a.add_node(&Spec::query(IS_EQUAL, &format!("OOTRRTradeMenu#on{k}")).linked_ptr("ValueA", U32, button).linked_ptr("ValueB", U32, h).out(BOOL, "IsEqual"))?);
        let count: Vec<u8> = text(a, &count_name(t.actor));
        stock.push(a.add_node(
            &Spec::query(GET_INT, &format!("OOTRRTradeMenu#stock{k}"))
                .idle(S32, "GameDataIndex", int(-1))
                .constant(BOOL, "IsDirect", yes(true))
                .constant(STR, "GameDataFullName", count)
                .out(S32, "Value"),
        )?);
        let set: usize = a.add_node(
            &Spec::node(SET_ENUM, &format!("OOTRRTradeMenu#write{k}"))
                .prop(BOOL, "ExecuteEveryFrame", yes(false))
                .idle(S32, "GameDataIndex", int(-1))
                .constant(STR, "GameDataFullName", button_name.clone())
                .linked_ptr("Value", U32, h),
        )?;
        write.push(parallel(a, &format!("OOTRRTradeMenu#put{k}"), &[set, ask])?);
        plain.push(set);
    }
    let mut any: usize = on[0];
    for (k, &o) in on.iter().enumerate().skip(1) {
        any = a.add_node(&Spec::query(EITHER, &format!("OOTRRTradeMenu#any{k}")).linked(BOOL, "ValueA", any, 0).linked(BOOL, "ValueB", o, 0).out(BOOL, "Result"))?;
    }
    let child_name: Vec<u8> = text(a, CHILD);
    let child: usize = a.add_node(
        &Spec::query(GET_BOOL, "OOTRRTradeMenu#child")
            .idle(S32, "GameDataIndex", int(-1))
            .idle(BOOL, "IsDirect", yes(false))
            .constant(STR, "GameDataFullName", child_name)
            .out(BOOL, "Value"),
    )?;
    Ok(Parts { dummy, memo, on, any, stock, write, plain, ask, clear, child })
}

fn set_count(a: &mut Ainb, seed: &str, k: usize, v: i32) -> Res<usize> {
    let name: Vec<u8> = text(a, &count_name(ORDER[k].actor));
    a.add_node(
        &Spec::node(SET_INT, seed)
            .prop(BOOL, "ExecuteEveryFrame", yes(false))
            .idle(S32, "GameDataIndex", int(-1))
            .constant(S32, "Value", int(v))
            .constant(STR, "GameDataFullName", name),
    )
}

/// Keeps one trade item out, every frame:
/// - a held trade item k on the button: k comes out if it is parked, and every other trade item
///   that is out is parked;
/// - no trade item on the button: if none is out, the earliest parked one comes out;
/// - a trade item on the button that is not held (it was handed in, the button would show
///   nothing): a trade item that is out goes on the button; with none out, the nearest parked one
///   before it in ORDER (wrapping) comes out, and goes on the button the frame after. The hand in
///   flows do the same at once (`fall_back` in the parent module); this is the safety net.
///
/// It runs before the switch in the frame, so a change is redrawn in the same frame on the item
/// tab; on another tab the request waits for the item tab.
fn keep(a: &mut Ainb, p: &Parts) -> Res<usize> {
    let n: usize = ORDER.len();
    let mut show: Vec<usize> = Vec::new();
    let mut park: Vec<usize> = Vec::new();
    for k in 0..n {
        // Every change asks for a redraw: the page shows what the counts were when it was built.
        let out: usize = set_count(a, &format!("OOTRRTradeMenu#show{k}"), k, 1)?;
        show.push(parallel(a, &format!("OOTRRTradeMenu#show_redraw{k}"), &[out, p.ask])?);
        let away: usize = set_count(a, &format!("OOTRRTradeMenu#park{k}"), k, PARKED)?;
        park.push(parallel(a, &format!("OOTRRTradeMenu#park_redraw{k}"), &[away, p.ask])?);
    }
    let mut first_parked: usize = p.dummy;
    for k in (0..n).rev() {
        first_parked = equals(a, &format!("OOTRRTradeMenu#parked{k}"), PARKED, p.stock[k], show[k], first_parked)?;
    }
    let mut none_out: usize = first_parked;
    for k in (0..n).rev() {
        none_out = equals(a, &format!("OOTRRTradeMenu#out{k}"), 1, p.stock[k], p.dummy, none_out)?;
    }
    let mut pick: usize = none_out;
    for k in (0..n).rev() {
        let mut each: Vec<usize> = vec![equals(a, &format!("OOTRRTradeMenu#own{k}"), PARKED, p.stock[k], show[k], p.dummy)?];
        for j in (0..n).filter(|&j| j != k) {
            each.push(equals(a, &format!("OOTRRTradeMenu#other{k}_{j}"), 1, p.stock[j], park[j], p.dummy)?);
        }
        let tidy: usize = parallel(a, &format!("OOTRRTradeMenu#tidy{k}"), &each)?;
        let mut before: usize = p.dummy;
        for d in (1..n).rev() {
            let j: usize = (k + n - d) % n;
            before = equals(a, &format!("OOTRRTradeMenu#before{k}_{d}"), PARKED, p.stock[j], show[j], before)?;
        }
        let mut stale: usize = before;
        for j in (0..n).rev().filter(|&j| j != k) {
            stale = equals(a, &format!("OOTRRTradeMenu#take_over{k}_{j}"), 1, p.stock[j], p.plain[j], stale)?;
        }
        let held: usize = equals(a, &format!("OOTRRTradeMenu#gone{k}"), 0, p.stock[k], stale, tidy)?;
        pick = selector(a, &format!("OOTRRTradeMenu#button{k}"), p.on[k], 0, held, pick)?;
    }
    Ok(pick)
}

/// Puts the next held (out or parked) trade item after `k` on the button, tried in ORDER and
/// wrapping; with no other one held the wait just ends.
fn next_after(a: &mut Ainb, p: &Parts, k: usize) -> Res<usize> {
    let n: usize = ORDER.len();
    let mut next: usize = p.clear;
    for d in (1..n).rev() {
        let j: usize = (k + d) % n;
        next = equals(a, &format!("OOTRRTradeMenu#held{k}_{d}"), 0, p.stock[j], next, p.write[j])?;
    }
    Ok(next)
}

/// Notes which trade item is on the button.
fn record(a: &mut Ainb, p: &Parts) -> Res<usize> {
    let n: usize = ORDER.len();
    let mut pick: usize = set_memo(a, "OOTRRTradeMenu#note_none", idle(n))?;
    for k in (0..n).rev() {
        let set: usize = set_memo(a, &format!("OOTRRTradeMenu#note{k}"), idle(k))?;
        pick = selector(a, &format!("OOTRRTradeMenu#is{k}"), p.on[k], 0, set, pick)?;
    }
    Ok(pick)
}

/// The redraw, as cases of the note: wait, close the item screen the way leaving the menu does
/// (a direct close), wait with the screen gone, then open it the way the menu's start does and
/// run what entering the item tab runs (button hit tests on, the tab's category set). Then the
/// note starts over.
fn redraw(a: &mut Ainb, tick: usize) -> Res<Vec<(f32, f32, usize)>> {
    let half: f32 = 0.5;
    let name: Vec<u8> = text(a, SCREEN);
    let close: usize = a.add_node(&Spec::node(CLOSE, "OOTRRTradeMenu#close").prop(BOOL, "ExecuteEveryFrame", yes(false)).constant(BOOL, "IsDirect", yes(true)).constant(STR, "ScreenName", name.clone()))?;
    let gone: usize = set_memo(a, "OOTRRTradeMenu#closed", REOPEN)?;
    let shut: usize = sequence(a, "OOTRRTradeMenu#shut", &[close, gone])?;
    let open: usize = a.add_node(&Spec::node(OPEN, "OOTRRTradeMenu#open").prop(BOOL, "ExecuteEveryFrame", yes(false)).constant(S32, "Option", int(OPEN_OPTION)).constant(STR, "ScreenName", name.clone()))?;
    let hit: usize = a.add_node(&Spec::node(HIT_TEST, "OOTRRTradeMenu#hit_test").prop(BOOL, "ExecuteEveryFrame", yes(false)).constant(BOOL, "IsEnable", yes(true)).constant(STR, "ScreenName", name))?;
    let tab: usize = a.add_node(&Spec::node(SET_CATEGORY, "OOTRRTradeMenu#tab").prop(BOOL, "ExecuteEveryFrame", yes(false)).constant(S32, "Category", int(ITEMS)).idle(BOOL, "IsBasedOnInputKey", yes(false)))?;
    let clear: usize = set_memo(a, "OOTRRTradeMenu#redrawn", idle(ORDER.len()))?;
    let again: usize = sequence(a, "OOTRRTradeMenu#reopen", &[open, hit, tab, clear])?;
    Ok(vec![
        (REDRAW - half, REDRAW + CLOSE_AFTER - half, tick),
        (REDRAW + CLOSE_AFTER - half, REDRAW + GAP, shut),
        (REOPEN - half, REOPEN + OPEN_AFTER - half, tick),
        (REOPEN + OPEN_AFTER - half, REOPEN + GAP, again),
    ])
}

/// The A switch, on the item tab only.
fn switch(a: &mut Ainb, p: &Parts, trig: usize) -> Res<usize> {
    let n: usize = ORDER.len();
    let half: f32 = 0.5;
    let mut start: Vec<(f32, f32, usize)> = Vec::new();
    for k in 0..n {
        let s: usize = set_memo(a, &format!("OOTRRTradeMenu#start{k}"), pend(k))?;
        start.push((idle(k) - half, idle(k) + half, s));
    }
    let press: usize = float_switch(a, "OOTRRTradeMenu#press", p.memo, &start, p.dummy)?;
    let one: Vec<u8> = 1f32.to_le_bytes().to_vec();
    let memo_name: Vec<u8> = text(a, MEMO);
    let tick: usize = a.add_node(&Spec::node(ADD_FLOAT, "OOTRRTradeMenu#tick").idle(S32, "GameDataIndex", int(-1)).constant(F32, "Value", one).constant(STR, "GameDataFullName", memo_name))?;
    let mut wait: Vec<(f32, f32, usize)> = Vec::new();
    for k in 0..n {
        // Every way out of the wait sets the note once: a switch asks for the redraw, a press
        // that put another item on the button (or found no other trade item) starts over.
        let next: usize = next_after(a, p, k)?;
        let done: usize = selector(a, &format!("OOTRRTradeMenu#picked_trade{k}"), p.any, 0, next, p.clear)?;
        wait.push((pend(k) - half, pend(k) + WAIT - half, tick));
        wait.push((pend(k) + WAIT - half, pend(k) + GAP / 2.0, done));
    }
    wait.extend(redraw(a, tick)?);
    let note: usize = record(a, p)?;
    let idle_frame: usize = float_switch(a, "OOTRRTradeMenu#wait", p.memo, &wait, note)?;
    let pressed: usize = selector(a, "OOTRRTradeMenu#pressed", trig, A, press, idle_frame)?;
    let cat: usize = a.add_node(
        &Spec::query(CATEGORY, "OOTRRTradeMenu#category").idle(BOOL, "IsBasedOnInputKey", yes(false)).constant(BOOL, "IsMinus", yes(false)).out(S32, "Category"),
    )?;
    equals(a, "OOTRRTradeMenu#items", ITEMS, cat, pressed, p.dummy)
}

/// The menu's idle branch: the default case of the cursor-direction switch that the last menu
/// button test (ZR) falls through to. Returns the switch and its default plug position.
fn idle_branch(a: &Ainb, trig: usize) -> Res<(usize, usize)> {
    let last: Vec<usize> = (0..a.els.len()).filter(|&i| a.el_type(i) == BOOL_SELECTOR && source(a, i, BOOL) == Some((trig, ZR))).collect();
    let [last] = last.as_slice() else {
        return Err(Error::Format("trade menu: the menu does not test ZR once"));
    };
    let sw: usize = a.params[*last].plugs[child(a, *last, "False")?].a as usize;
    if a.el_type(sw) != S32_SWITCH {
        return Err(Error::Format("trade menu: after ZR the menu no longer switches on the cursor"));
    }
    let src: usize = a.params[sw].group(5).first().map(|p| p.a as usize).ok_or(Error::Format("trade menu: the cursor switch has no input"))?;
    if a.el_name(src) != DIRECTION {
        return Err(Error::Format("trade menu: the cursor switch does not read the cursor direction"));
    }
    let b: usize = a.params[sw].pl[2][1] as usize;
    let n: usize = a.params[sw].pl[2][0] as usize;
    let at: usize = b + n.checked_sub(1).ok_or(Error::Format("trade menu: the cursor switch has no cases"))?;
    let dflt: usize = a.params[sw].plugs[at].a as usize;
    if a.el_name(dflt) != DUMMY {
        return Err(Error::Format("trade menu: the menu's idle frame is no longer empty"));
    }
    Ok((sw, at))
}

/// The menu's update state, whose Leave child runs when the menu closes or changes tab.
fn leave(a: &Ainb) -> Res<(usize, usize)> {
    let st: Vec<usize> = (0..a.els.len()).filter(|&i| a.el_type(i) == STATE && a.params[i].group(2).iter().any(|p| a.s(p.b) == "Leave")).collect();
    let [st] = st.as_slice() else {
        return Err(Error::Format("trade menu: the menu has no single update state"));
    };
    Ok((*st, child(a, *st, "Leave")?))
}

pub fn patch(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    if !a.find_els(SET_ENUM).is_empty() {
        return Err(Error::Format("trade menu: the item menu already writes the item button"));
    }
    let trig: usize = match a.find_els(TRIG).as_slice() {
        [i] => *i,
        _ => return Err(Error::Format("trade menu: the menu has no single trigger checker")),
    };
    let (sw, at) = idle_branch(&a, trig)?;
    let (st, out) = leave(&a)?;
    let p: Parts = parts(&mut a)?;

    let kept: usize = keep(&mut a, &p)?;
    let menu: usize = switch(&mut a, &p, trig)?;
    let adult: usize = parallel(&mut a, "OOTRRTradeMenu#adult_frame", &[kept, menu])?;
    let root: usize = selector(&mut a, "OOTRRTradeMenu#adult", p.child, 0, p.dummy, adult)?;
    a.params[sw].plugs[at].a = root as u32;

    // Leaving hands the borrowed counter back at zero.
    let old: usize = a.params[st].plugs[out].a as usize;
    let reset: usize = set_memo(&mut a, "OOTRRTradeMenu#reset", 0.0)?;
    let bye: usize = parallel(&mut a, "OOTRRTradeMenu#leave", &[old, reset])?;
    a.params[st].plugs[out].a = bye as u32;

    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(Error::Msg(format!("trade menu: graph audit: {}", bad.join("; "))));
    }
    a.write()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::murmur::murmur3_32;
    use crate::romfs::Romfs;

    fn real() -> Option<Romfs> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.join(FILE).is_file() {
            Romfs::open(&p).ok()
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn kid(a: &Ainb, i: usize, name: &str) -> usize {
        a.params[i].plugs[child(a, i, name).unwrap()].a as usize
    }

    fn f32_in(a: &Ainb, i: usize) -> f32 {
        let v = a.input(F32, a.find_in(i, F32, "Value").unwrap()).unwrap().val;
        f32::from_le_bytes([v[0], v[1], v[2], v[3]])
    }

    fn link(a: &Ainb, i: usize, k: usize, name: &str) -> usize {
        a.input(k, a.find_in(i, k, name).unwrap()).unwrap().src as usize
    }

    fn cases(a: &Ainb, i: usize) -> Vec<(f32, f32, usize)> {
        let g = a.params[i].group(2);
        let r = |e: &[u8], o: usize| f32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]);
        g[..g.len() - 1].iter().map(|p| (r(&p.extra, 4), r(&p.extra, 12), p.a as usize)).collect()
    }

    /// Bytes per pointer input record: name, class, source, output, flags, zero.
    const PTR_IN: usize = 20;

    /// What the graph reads and writes: the item button, the trade counts, the note, and what a
    /// frame did to the screen.
    #[derive(Clone, Debug, PartialEq)]
    struct World {
        button: String,
        counts: Vec<i32>,
        memo: f32,
        child: bool,
        category: i32,
        press_a: bool,
        screen: Vec<String>,
    }

    fn world(button: &str, held: &[(usize, i32)]) -> World {
        let mut counts: Vec<i32> = vec![0; ORDER.len()];
        for &(k, c) in held {
            counts[k] = c;
        }
        World { button: button.to_string(), counts, memo: 0.0, child: false, category: ITEMS, press_a: false, screen: Vec::new() }
    }

    fn item(a: &Ainb, i: usize) -> usize {
        let name: &str = a.in_str(i, "GameDataFullName").unwrap();
        ORDER.iter().position(|t| count_name(t.actor) == name).unwrap()
    }

    fn u32_of(a: &Ainb, w: &World, i: usize) -> u32 {
        match a.el_name(i) {
            GET_ENUM => {
                assert_eq!(a.in_str(i, "GameDataFullName"), Some(BUTTON));
                murmur3_32(w.button.as_bytes(), 0)
            }
            HASH => murmur3_32(a.in_str(i, "String").unwrap().as_bytes(), 0),
            other => panic!("no u32 from {other}"),
        }
    }

    fn bool_of(a: &Ainb, w: &World, i: usize, plug: i16) -> bool {
        match a.el_name(i) {
            IS_EQUAL => u32_of(a, w, link(a, i, PTR, "ValueA")) == u32_of(a, w, link(a, i, PTR, "ValueB")),
            EITHER => bool_of(a, w, link(a, i, BOOL, "ValueA"), 0) || bool_of(a, w, link(a, i, BOOL, "ValueB"), 0),
            GET_BOOL => {
                assert_eq!(a.in_str(i, "GameDataFullName"), Some(CHILD));
                w.child
            }
            TRIG => plug == A && w.press_a,
            other => panic!("no bool from {other}"),
        }
    }

    fn int_of(a: &Ainb, w: &World, i: usize) -> i32 {
        match a.el_name(i) {
            GET_INT => w.counts[item(a, i)],
            CATEGORY => w.category,
            other => panic!("no int from {other}"),
        }
    }

    /// Runs node `i` for one frame, as the game would: selectors pick by their input, the compare
    /// selector names its branch from its constant's side.
    fn run(a: &Ainb, w: &mut World, i: usize) {
        match (a.el_type(i), a.el_name(i)) {
            (BOOL_SELECTOR, _) => {
                let (src, plug) = source(a, i, BOOL).unwrap();
                let b: bool = bool_of(a, w, src, plug);
                run(a, w, kid(a, i, if b { "True" } else { "False" }));
            }
            (PARALLEL, _) | (SEQUENCE, _) => {
                for k in kids(a, i) {
                    run(a, w, k);
                }
            }
            (F32_SWITCH, _) => {
                let src: usize = source(a, i, F32).unwrap().0;
                assert_eq!((a.el_name(src), a.in_str(src, "GameDataFullName")), (GET_FLOAT, Some(MEMO)));
                let hit: Option<usize> = cases(a, i).into_iter().find(|&(lo, hi, _)| lo <= w.memo && w.memo <= hi).map(|c| c.2);
                run(a, w, hit.unwrap_or(a.params[i].group(2).last().unwrap().a as usize));
            }
            (0, COMPARE) => {
                let c: i32 = a.in_s32(i, "ValueA").unwrap();
                let v: i32 = int_of(a, w, link(a, i, S32, "ValueB"));
                assert_eq!(a.params[i].group(5)[0].a as usize, link(a, i, S32, "ValueB"));
                run(a, w, kid(a, i, if c > v { "Bigger" } else if c == v { "Equal" } else { "Smaller" }));
            }
            (0, SET_INT) => w.counts[item(a, i)] = a.in_s32(i, "Value").unwrap(),
            (0, SET_FLOAT) => {
                assert_eq!(a.in_str(i, "GameDataFullName"), Some(MEMO));
                w.memo = f32_in(a, i);
            }
            (0, ADD_FLOAT) => {
                assert_eq!(a.in_str(i, "GameDataFullName"), Some(MEMO));
                w.memo += f32_in(a, i);
            }
            (0, SET_ENUM) => {
                assert_eq!(a.in_str(i, "GameDataFullName"), Some(BUTTON));
                w.button = a.in_str(link(a, i, PTR, "Value"), "String").unwrap().to_string();
            }
            (0, DUMMY) => {}
            (0, CLOSE) | (0, HIT_TEST) => {
                assert_eq!(a.in_str(i, "ScreenName"), Some(SCREEN));
                w.screen.push(a.el_name(i).to_string());
            }
            (0, OPEN) => {
                assert_eq!((a.in_str(i, "ScreenName"), a.in_s32(i, "Option")), (Some(SCREEN), Some(OPEN_OPTION)));
                w.screen.push(OPEN.to_string());
            }
            (0, SET_CATEGORY) => {
                assert_eq!(a.in_s32(i, "Category"), Some(ITEMS));
                w.screen.push(SET_CATEGORY.to_string());
            }
            (t, n) => panic!("the frame reached a node it should not: type {t} {n}"),
        }
    }

    /// Frames enough for a press, its wait, the switch and the whole redraw.
    const LONG: usize = (WAIT + CLOSE_AFTER + OPEN_AFTER) as usize + 8;

    struct Menu {
        a: Ainb,
        root: usize,
    }

    impl Menu {
        fn frame(&self, w: &mut World) {
            w.screen.clear();
            run(&self.a, w, self.root);
            w.press_a = false;
        }

        fn frames(&self, w: &mut World, n: usize) {
            for _ in 0..n {
                self.frame(w);
            }
        }

        /// Presses A and runs until the wait is over, the switch is made and the note is back.
        /// Returns what the screen was asked to do on the way.
        fn press(&self, w: &mut World) -> Vec<String> {
            w.press_a = true;
            let mut did: Vec<String> = Vec::new();
            for _ in 0..LONG {
                self.frame(w);
                did.extend(w.screen.iter().cloned());
            }
            did
        }

        /// Runs until a redraw that is under way is over; returns what the screen was asked to do.
        fn settle(&self, w: &mut World) -> Vec<String> {
            let mut did: Vec<String> = Vec::new();
            for _ in 0..LONG {
                self.frame(w);
                did.extend(w.screen.iter().cloned());
            }
            did
        }
    }

    fn menu(rf: &Romfs) -> (Menu, Ainb, Vec<u8>) {
        let raw: Vec<u8> = rf.read(FILE).unwrap();
        let before: Ainb = Ainb::parse(&raw).unwrap();
        let out: Vec<u8> = patch(&raw).unwrap();
        let a: Ainb = Ainb::parse(&out).unwrap();
        let trig: usize = a.find_els(TRIG)[0];
        let (sw, at) = idle_branch(&before, trig).unwrap();
        let root: usize = a.params[sw].plugs[at].a as usize;
        (Menu { a, root }, before, out)
    }

    const CUCCO: usize = 1;
    const SAW: usize = 5;
    const CHECK: usize = 10;

    fn slot(k: usize) -> &'static str {
        ORDER[k].slot
    }

    #[test]
    fn the_patched_menu_is_a_sound_graph_that_keeps_the_game_s_own_paths() {
        let Some(rf) = real() else {
            return;
        };
        let (m, before, out) = menu(&rf);
        let a: &Ainb = &m.a;
        assert!(audit(a).is_empty());
        assert_eq!(a.write().unwrap(), out, "the patched graph rewrites byte for byte");
        assert!(patch(&out).is_err(), "a second patch is refused");
        let trig: usize = a.find_els(TRIG)[0];
        let (sw, _) = idle_branch(&before, trig).unwrap();
        assert_eq!(a.params[sw].pl, before.params[sw].pl, "same plug layout");
        assert!(m.root >= before.els.len(), "the idle frame runs the new graph");
        for (j, p) in before.params[sw].group(2).iter().enumerate() {
            if j + 1 < before.params[sw].group(2).len() {
                assert_eq!(a.params[sw].group(2)[j].a, p.a, "cursor jumps keep their transitions");
            }
        }
        for i in before.els.len()..a.els.len() {
            if a.el_name(i) == COMPARE {
                assert_eq!(a.params[i].pl, [[0, 0], [0, 0], [3, 0], [0, 3], [0, 3], [1, 3], [0, 4], [0, 4], [0, 4], [0, 4]], "laid out like the game's own compare");
                assert_eq!(kid(a, i, "Bigger"), kid(a, i, "Smaller"), "counts are only tested for an exact value");
            }
        }
        // Nothing new hangs off the opening sequence or any node the game runs once.
        for i in 0..before.els.len() {
            if i == sw {
                continue;
            }
            let (st, out_at) = leave(a).unwrap();
            for (j, p) in before.params[i].group(2).iter().enumerate() {
                if i == st && before.params[i].pl[2][1] as usize + j == out_at {
                    continue;
                }
                assert_eq!(a.params[i].group(2)[j].a, p.a, "element {i} keeps its children");
            }
            assert_eq!(a.params[i].plugs.len(), before.params[i].plugs.len(), "element {i} keeps its plugs");
        }
        let (st, out_at) = leave(a).unwrap();
        let bye: Vec<usize> = kids(a, a.params[st].plugs[out_at].a as usize);
        assert_eq!(bye[0], before.params[st].plugs[out_at].a as usize, "leaving still runs the game's own leave");
        assert_eq!((a.el_name(bye[1]), a.in_str(bye[1], "GameDataFullName"), f32_in(a, bye[1])), (SET_FLOAT, Some(MEMO), 0.0), "and hands the counter back at zero");
        assert_eq!(bye.len(), 2, "leaving parks or shows nothing");

        let vanilla: Ainb = Ainb::parse(&rf.read("Sequence/ShortCut.module.ainb").unwrap()).unwrap();
        let w: usize = a.find_els(SET_ENUM)[0];
        let v: usize = vanilla.find_els(SET_ENUM)[0];
        let class = |x: &Ainb, i: usize| -> String {
            let g: usize = x.find_in(i, PTR, "Value").unwrap();
            let r: &[u8] = &x.ins[PTR][g * PTR_IN..];
            x.s(u32::from_le_bytes([r[4], r[5], r[6], r[7]])).to_string()
        };
        assert_eq!(class(a, w), class(&vanilla, v), "the button write reads its value like the game's own enum write");
        assert!(idle(ORDER.len()) > pend(0) + GAP && REDRAW + GAP < pend(ORDER.len()) - GAP && REOPEN + GAP < REDRAW - GAP, "notes, waits and the redraw never overlap");
        assert!(CLOSE_AFTER < GAP && OPEN_AFTER < GAP);
    }

    #[test]
    fn every_frame_only_the_button_s_trade_item_is_out() {
        let Some(rf) = real() else {
            return;
        };
        let (m, _, _) = menu(&rf);
        // An old save with three trade items out: the button's stays, the others are parked.
        let mut w: World = world(slot(SAW), &[(CUCCO, 1), (SAW, 1), (CHECK, 1)]);
        m.frame(&mut w);
        assert_eq!((w.counts[CUCCO], w.counts[SAW], w.counts[CHECK]), (PARKED, 1, PARKED));
        assert!(w.screen.is_empty(), "the redraw waits for the counts to be in");
        assert_eq!(m.settle(&mut w), [CLOSE, OPEN, HIT_TEST, SET_CATEGORY], "the page was built with all three out: it is redrawn");
        let settled: World = w.clone();
        for _ in 0..3 {
            m.frame(&mut w);
            assert!(w.screen.is_empty(), "and then nothing changes and nothing is redrawn");
        }
        assert_eq!((w.counts.clone(), w.button.clone()), (settled.counts, settled.button));
        // On every tab, not only the item tab.
        let mut w: World = world(slot(SAW), &[(CUCCO, 1), (SAW, 1)]);
        w.category = 3;
        m.frame(&mut w);
        assert_eq!((w.counts[CUCCO], w.counts[SAW]), (PARKED, 1));
        assert!(w.screen.is_empty(), "the redraw waits for the item tab");
        w.category = ITEMS;
        assert_eq!(m.settle(&mut w).len(), 4);
        // The button's own item is parked (it was just put there): it comes out.
        let mut w: World = world(slot(CHECK), &[(CUCCO, 1), (CHECK, PARKED)]);
        m.frame(&mut w);
        assert_eq!((w.counts[CUCCO], w.counts[CHECK]), (PARKED, 1));
        // Another item on the button: what is out stays out, nothing parked comes out.
        let mut w: World = world("Bow", &[(CUCCO, PARKED), (SAW, 1), (CHECK, PARKED)]);
        m.frames(&mut w, 2);
        assert_eq!((w.counts[CUCCO], w.counts[SAW], w.counts[CHECK]), (PARKED, 1, PARKED));
        // None out and another item on the button: the earliest parked one comes out, once.
        let mut w: World = world("Bow", &[(CUCCO, PARKED), (CHECK, PARKED)]);
        m.settle(&mut w);
        assert_eq!((w.counts[CUCCO], w.counts[SAW], w.counts[CHECK], w.button.as_str()), (1, 0, PARKED, "Bow"));
        // The button names a trade item that was handed in: the nearest parked one before it
        // (wrapping) comes out and takes the button over.
        for (gone, want) in [(SAW, CUCCO), (CUCCO, CHECK)] {
            let held: Vec<(usize, i32)> = [CUCCO, SAW, CHECK].into_iter().filter(|&k| k != gone).map(|k| (k, PARKED)).collect();
            let mut w: World = world(slot(gone), &held);
            m.settle(&mut w);
            assert_eq!(w.button, slot(want), "after handing in {}", slot(gone));
            for &(k, _) in &held {
                assert_eq!(w.counts[k], if k == want { 1 } else { PARKED }, "{} after handing in {}", slot(k), slot(gone));
            }
        }
        // One is out already: it takes the button over and nothing else changes.
        let mut w: World = world(slot(SAW), &[(CUCCO, PARKED), (CHECK, 1)]);
        m.frame(&mut w);
        assert_eq!((w.button.as_str(), w.counts[CUCCO], w.counts[CHECK]), (slot(CHECK), PARKED, 1));
        assert!(w.screen.is_empty());
        // No trade items at all, and a child: nothing happens.
        let mut w: World = world("Bow", &[]);
        m.frames(&mut w, 2);
        assert!(w.counts.iter().all(|&c| c == 0));
        let mut w: World = world(slot(SAW), &[(CUCCO, 1), (SAW, 1)]);
        w.child = true;
        w.press_a = true;
        m.frames(&mut w, 12);
        assert_eq!((w.counts[CUCCO], w.counts[SAW], w.button.as_str(), w.memo), (1, 1, slot(SAW), 0.0));
    }

    #[test]
    fn a_second_a_on_the_trade_slot_brings_the_next_held_item_out_and_redraws() {
        let Some(rf) = real() else {
            return;
        };
        let (m, _, _) = menu(&rf);
        let mut w: World = world(slot(CUCCO), &[(CUCCO, 1), (SAW, PARKED), (CHECK, PARKED)]);
        m.frame(&mut w);
        assert_eq!(w.memo, idle(CUCCO), "the button's trade item is noted");
        let redraw: Vec<String> = [CLOSE, OPEN, HIT_TEST, SET_CATEGORY].iter().map(|s| s.to_string()).collect();
        for want in [SAW, CHECK, CUCCO, SAW] {
            let before: usize = ORDER.iter().position(|t| t.slot == w.button).unwrap();
            let did: Vec<String> = m.press(&mut w);
            assert_eq!(w.button, slot(want), "after {} comes {}", slot(before), slot(want));
            for k in [CUCCO, SAW, CHECK] {
                assert_eq!(w.counts[k], if k == want { 1 } else { PARKED }, "{} after switching to {}", slot(k), slot(want));
            }
            assert_eq!(did, redraw, "the screen is closed and opened again once, after the counts swapped");
            assert_eq!(w.memo, idle(want), "and the new item is noted");
        }
        // The swap is done before the screen opens again.
        let mut w: World = world(slot(CUCCO), &[(CUCCO, 1), (SAW, PARKED)]);
        m.frame(&mut w);
        w.press_a = true;
        let mut swapped: Option<usize> = None;
        let mut frames: Vec<(usize, String)> = Vec::new();
        for i in 0..LONG {
            m.frame(&mut w);
            if swapped.is_none() && (w.counts[CUCCO], w.counts[SAW], w.button.as_str()) == (PARKED, 1, slot(SAW)) {
                swapped = Some(i);
            }
            frames.extend(w.screen.iter().map(|s| (i, s.clone())));
        }
        let at = |what: &str| frames.iter().find(|(_, s)| s == what).map(|(i, _)| *i).unwrap();
        let swapped: usize = swapped.unwrap();
        assert_eq!(at(CLOSE), swapped + CLOSE_AFTER as usize, "the close comes a few frames after the counts swapped");
        assert_eq!(at(OPEN), at(CLOSE) + OPEN_AFTER as usize + 1, "and the screen stays closed for a few frames");
        assert_eq!(frames.len(), 4);
        // A press that picked another item (the menu put the bow on the button) switches nothing.
        let mut w: World = world(slot(CUCCO), &[(CUCCO, 1), (SAW, PARKED)]);
        m.frame(&mut w);
        w.press_a = true;
        m.frame(&mut w);
        w.button = "Bow".to_string();
        let did: Vec<String> = m.settle(&mut w);
        assert_eq!((w.button.as_str(), w.counts[CUCCO], w.counts[SAW]), ("Bow", 1, PARKED));
        assert!(did.is_empty(), "and nothing is redrawn");
        assert_eq!(w.memo, idle(ORDER.len()));
        // A press with another item on the button is the menu's own (it puts the shown trade item there).
        let mut w: World = world("Bow", &[(CUCCO, 1), (SAW, PARKED)]);
        m.frame(&mut w);
        assert!(m.press(&mut w).is_empty());
        assert_eq!((w.button.as_str(), w.counts[CUCCO], w.counts[SAW]), ("Bow", 1, PARKED));
        // One trade item only: nothing to switch to, nothing redrawn.
        let mut w: World = world(slot(SAW), &[(SAW, 1)]);
        m.frame(&mut w);
        assert!(m.press(&mut w).is_empty());
        assert_eq!((w.button.as_str(), w.counts[SAW], w.memo), (slot(SAW), 1, idle(SAW)));
        // Off the item tab a press does nothing.
        let mut w: World = world(slot(CUCCO), &[(CUCCO, 1), (SAW, PARKED)]);
        w.category = 3;
        m.frame(&mut w);
        assert!(m.press(&mut w).is_empty());
        assert_eq!((w.button.as_str(), w.memo), (slot(CUCCO), 0.0));
    }

    #[test]
    fn the_hashed_slots_are_the_enum_values() {
        let Some(mut rf) = real() else {
            return;
        };
        let y = rf.read_byml(crate::chest_slot::LIST).unwrap();
        let rows = y.root.get("Data").and_then(|d| d.get("Enum")).and_then(|e| e.as_arr()).unwrap();
        let h: u32 = murmur3_32(BUTTON.as_bytes(), 0);
        let row = rows.iter().find(|r| matches!(r.get("Hash"), Some(crate::byml::Node::U32(x)) if *x == h)).unwrap();
        let vals = row.get("Values").and_then(|v| v.as_arr()).unwrap();
        let raw = row.get("RawValues").and_then(|v| v.as_arr()).unwrap();
        for t in ORDER {
            let i: usize = raw.iter().position(|r| matches!(r, crate::byml::Node::Str(s) if s == t.slot)).unwrap();
            assert_eq!(vals[i], crate::byml::Node::U64(murmur3_32(t.slot.as_bytes(), 0) as u64), "{}", t.slot);
        }
    }

    #[test]
    fn each_trade_count_is_a_saved_int() {
        let Some(mut rf) = real() else {
            return;
        };
        let y = rf.read_byml(crate::chest_slot::LIST).unwrap();
        let ints = y.root.get("Data").and_then(|d| d.get("Int")).and_then(|e| e.as_arr()).unwrap();
        for t in ORDER {
            let h: u32 = murmur3_32(count_name(t.actor).as_bytes(), 0);
            assert!(ints.iter().any(|r| matches!(r.get("Hash"), Some(crate::byml::Node::U32(x)) if *x == h)), "{}", t.actor);
        }
    }
}
