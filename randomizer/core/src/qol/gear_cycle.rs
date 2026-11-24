//! Holding D-pad up steps the selected head gear to the next mask or Lens of Truth the player
//! owns; holding D-pad down swaps the Iron and Hover Boots. A tap stays the game's own toggle.
//!
//! The shortcut menu can't list gear (the game builds only its item list, in code), so this
//! writes the selection directly. `Sequence/UpdateShortCutOpenHoldFrameCounter.module` runs
//! every frame outside menus and times the item button hold with `Sequence_ShortCutOpenHoldCounter`.
//! Where it resets that counter because neither item button is held, the D-pad takes over:
//!
//! - a press sets the counter to [`START`], far below the range in which the game opens the item
//!   menu (1 to 10000);
//! - while held, a float switch adds the frame step up to -1000, then runs the cycle and sets
//!   [`START`] again, so a long hold keeps stepping;
//! - the cycle reads `PlayerStatus.EquipHeadArmor` (or `EquipBoot`), finds it in the list by name
//!   hash, writes the next owned item (`IsGet.<actor>`) and asks the game to apply it. Once
//!   the mask quest is complete every mask counts as owned; one the player never borrowed is
//!   pushed into the pouch before it is equipped.
//!
//! [`patch_boots_hop`] drops the little hop of a boots change from the player's scheduler (the
//! child and adult player packs): the boots-change action is swapped for the wait action. Not yet
//! confirmed in game that the boots still switch.

use crate::ainb::build::{Spec, QUERY};
use crate::ainb::{audit, Ainb};
use crate::error::{Error, Res};
use crate::qol::{edit_plain, Have};
use crate::romfs::Romfs;
use crate::staging::Out;

pub const COUNTER_FILE: &str = "Sequence/UpdateShortCutOpenHoldFrameCounter.module.ainb";

pub const NOTE: &str = "Hold D-pad up to switch to the next mask or Lens of Truth you own (every mask once the mask quest is complete), and D-pad down to switch between the Iron and Hover Boots. Keep holding to keep switching. A tap still puts the gear on or takes it off as before. Changing boots no longer makes Link hop. Experimental.";

pub const COUNTER: &str = "Sequence_ShortCutOpenHoldCounter";
pub const EQUIP_HEAD: &str = "PlayerStatus.EquipHeadArmor";
pub const EQUIP_BOOT: &str = "PlayerStatus.EquipBoot";
/// The mask quest: at Complete every mask can be borrowed, so every mask counts as owned.
pub const MASK_QUEST: &str = "Step_SubEvent_MaskSalesman";
pub const QUEST_DONE: &str = "Complete";

/// Frames of hold (in game steps) before the first switch.
pub const HOLD_STEPS: f32 = 15.0;
/// Frames of hold between later switches: each switch rebuilds Link's equipment, so a long hold
/// steps at half the speed of the first switch.
pub const REPEAT_STEPS: f32 = 30.0;
/// The counter value a press starts from: the hold counts up to [`CYCLE_AT`].
pub const START: f32 = -1000.0 - HOLD_STEPS;
/// The counter value after each switch.
pub const AGAIN: f32 = -1000.0 - REPEAT_STEPS;
pub const CYCLE_AT: f32 = -1000.0;
/// The lowest and highest values the switch tests; the game's own menu range starts at 1.
const FLOOR: f32 = -5000.0;
const CEIL: f32 = -500.0;

/// One piece of gear: the name written to the equipment game data, the owned flag, every name
/// that counts as "this one is equipped", and whether the completed mask quest makes it owned.
#[derive(Clone, Copy, Debug)]
pub struct Gear {
    pub name: &'static str,
    pub owned: &'static str,
    pub also: &'static [&'static str],
    pub quest: bool,
}

const fn mask(name: &'static str, owned: &'static str) -> Gear {
    Gear { name, owned, also: &[], quest: true }
}

const fn boot(name: &'static str, owned: &'static str) -> Gear {
    Gear { name, owned, also: &[], quest: false }
}

/// Head gear in N64 subscreen order, then the Lens. The Lens's equipment name is a guess: the
/// pouch item is Armor_GlassTruth and each age has its own actor, so all three count as current.
pub const HEAD: [Gear; 9] = [
    mask("Armor_Child_Mask_Keaton", "IsGet.Armor_Child_Mask_Keaton"),
    mask("Armor_Child_Mask_Skull", "IsGet.Armor_Child_Mask_Skull"),
    mask("Armor_Child_Mask_Spooky", "IsGet.Armor_Child_Mask_Spooky"),
    mask("Armor_Child_Mask_Bunny", "IsGet.Armor_Child_Mask_Bunny"),
    mask("Armor_Child_Mask_Goron", "IsGet.Armor_Child_Mask_Goron"),
    mask("Armor_Child_Mask_Zora", "IsGet.Armor_Child_Mask_Zora"),
    mask("Armor_Child_Mask_Gerudo", "IsGet.Armor_Child_Mask_Gerudo"),
    mask("Armor_Child_Mask_Truth", "IsGet.Armor_Child_Mask_Truth"),
    Gear { name: "Armor_GlassTruth", owned: "IsGet.Armor_GlassTruth", also: &["Armor_Child_GlassTruth", "Armor_Adult_GlassTruth"], quest: false },
];

pub const BOOTS: [Gear; 2] = [boot("Armor_Adult_HeavyBoot", "IsGet.Armor_Adult_HeavyBoot"), boot("Armor_Adult_HoverBoot", "IsGet.Armor_Adult_HoverBoot")];

const HOLD_CHECK: &str = "SeqQueryControllerHoldMaskChecker";
const TRIG_CHECK: &str = "SeqQueryControllerTrigMaskChecker";
const GET_BOOL: &str = "SeqQueryGetNonStructGameDataBool";
const GET_ENUM: &str = "SeqQueryGetNonStructGameDataEnum";
const EITHER: &str = "SeqQueryLogicalDisjunction";
const PUSH: &str = "SeqOneShotPushToPouch";
const GET_FLOAT: &str = "SeqQueryGetNonStructGameDataFloat";
const SET_FLOAT: &str = "SeqOneShotSetNonStructGameDataFloat";
const ADD_FLOAT: &str = "SeqExecuteAddNonStructGameDataFloat";
const DUMMY: &str = "SeqExecuteGenericDummyAction";
const IS_EQUAL: &str = "SeqQueryIsEqualU32";
const HASH: &str = "SeqQueryCalcStringHashMurMur3";
const U32: &str = "const u32";
/// The default plug label of a switch, as the game's own switches name it.
const OTHER: &str = "その他";

/// Bool output positions of the hold and trigger mask checkers (outputs are in name order).
pub const DOWN: i16 = 2;
pub const R: i16 = 8;
pub const UP: i16 = 11;

const S32: usize = 0;
const BOOL: usize = 1;
const F32: usize = 2;
const STR: usize = 3;
const BOOL_SELECTOR: u16 = 7;
const PARALLEL: u16 = 3;
const F32_SWITCH: u16 = 4;
const LIVE: u32 = 0x8000_0000;

fn yes(b: bool) -> Vec<u8> {
    vec![b as u8, 0, 0, 0]
}

fn int(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn text(a: &mut Ainb, s: &str) -> Vec<u8> {
    a.pool.add(s).to_le_bytes().to_vec()
}

fn one(a: &Ainb, name: &str, what: &'static str) -> Res<usize> {
    match a.find_els(name).as_slice() {
        [i] => Ok(*i),
        _ => Err(Error::Format(what)),
    }
}

/// The one element named `name` whose string input `input` is `v`.
fn one_with(a: &Ainb, name: &str, input: &str, v: &str, what: &'static str) -> Res<usize> {
    match a.find_els(name).into_iter().filter(|&i| a.in_str(i, input) == Some(v)).collect::<Vec<usize>>().as_slice() {
        [i] => Ok(*i),
        _ => Err(Error::Format(what)),
    }
}

/// The bool selectors whose input reads output `at` of `src`.
pub fn readers(a: &Ainb, src: usize, at: i16) -> Vec<usize> {
    (0..a.els.len())
        .filter(|&i| a.el_type(i) == BOOL_SELECTOR)
        .filter(|&i| a.find_in(i, BOOL, "Input").and_then(|g| a.input(BOOL, g)).is_some_and(|x| x.src == src as i16 && x.plug == at))
        .collect()
}

/// The position in `plugs` of the child plug named `name`.
pub fn child(a: &Ainb, i: usize, name: &str) -> Res<usize> {
    let p = &a.params[i];
    let b: usize = p.pl[2][1] as usize;
    p.group(2).iter().position(|x| a.s(x.b) == name).map(|j| b + j).ok_or(Error::Format("gear cycle: a selector lacks its True or False plug"))
}

/// Flags bool output `at` of `src` as read, as the game does for every output a node links to.
fn mark_read(a: &mut Ainb, src: usize, at: i16) -> Res<()> {
    let p = &a.params[src];
    if at < 0 || at as u32 >= p.ioe[BOOL][3] {
        return Err(Error::Format("gear cycle: no such output"));
    }
    let o: usize = (p.ioe[BOOL][2] as usize + at as usize) * 4;
    let r: &mut [u8] = a.outs[BOOL].get_mut(o..o + 4).ok_or(Error::Format("gear cycle: output out of range"))?;
    let v: u32 = u32::from_le_bytes([r[0], r[1], r[2], r[3]]) | LIVE;
    r.copy_from_slice(&v.to_le_bytes());
    Ok(())
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

fn const_hash(a: &mut Ainb, seed: &str, s: &str) -> Res<usize> {
    let v: Vec<u8> = text(a, s);
    let mut n: Spec = Spec::node(HASH, seed).prop(BOOL, "IsCalcHashOnInit", yes(true)).prop(BOOL, "OutputOnlyOnEnter", yes(false)).constant(STR, "String", v).out_ptr("MurMur3Hash", U32);
    n.role = QUERY;
    a.add_node(&n)
}

fn set_counter(a: &mut Ainb, seed: &str, v: f32) -> Res<usize> {
    let name: Vec<u8> = text(a, COUNTER);
    a.add_node(
        &Spec::node(SET_FLOAT, seed)
            .prop(BOOL, "ExecuteEveryFrame", yes(false))
            .idle(S32, "GameDataIndex", int(-1))
            .constant(F32, "Value", v.to_le_bytes().to_vec())
            .constant(STR, "GameDataFullName", name),
    )
}

/// A float switch over `src` (output 0): each case runs its child while the value is inside
/// [min, max]; otherwise `other` runs. Plug extras as in the game's own switch on this counter.
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

/// The cycle for one equipment slot: the next owned item after the current one, or the first
/// owned one when the current one is not in the list. Does nothing when no other item is owned.
fn cycle(a: &mut Ainb, tag: &str, data: &str, list: &[Gear], dummy: usize) -> Res<usize> {
    let data_v: Vec<u8> = text(a, data);
    let seed_cur: String = format!("{tag}#current");
    let cur: usize = a.add_node(
        &Spec::query("SeqQueryGetNonStructGameDataString64", &seed_cur)
            .constant(S32, "GameDataIndex", int(-1))
            .idle(BOOL, "IsDirect", yes(false))
            .constant(STR, "GameDataFullName", data_v.clone())
            .out(STR, "Value"),
    )?;
    let seed_h: String = format!("{tag}#current_hash");
    let mut h: Spec = Spec::node(HASH, &seed_h).prop(BOOL, "IsCalcHashOnInit", yes(false)).prop(BOOL, "OutputOnlyOnEnter", yes(false)).linked(STR, "String", cur, 0).out_ptr("MurMur3Hash", U32);
    h.role = QUERY;
    let cur_hash: usize = a.add_node(&h)?;
    let quest_done: Option<usize> = if list.iter().any(|g| g.quest) { Some(quest_done(a, tag)?) } else { None };
    let mut owned: Vec<usize> = Vec::new();
    let mut sets: Vec<usize> = Vec::new();
    for (j, g) in list.iter().enumerate() {
        let flag: Vec<u8> = text(a, g.owned);
        let seed_o: String = format!("{tag}#owned{j}");
        let have: usize = a.add_node(&Spec::query(GET_BOOL, &seed_o).idle(S32, "GameDataIndex", int(-1)).idle(BOOL, "IsDirect", yes(false)).constant(STR, "GameDataFullName", flag).out(BOOL, "Value"))?;
        let quest: Option<usize> = if g.quest { quest_done } else { None };
        owned.push(match quest {
            Some(q) => {
                let seed_e: String = format!("{tag}#available{j}");
                a.add_node(&Spec::query(EITHER, &seed_e).linked(BOOL, "ValueA", have, 0).linked(BOOL, "ValueB", q, 0).out(BOOL, "Result"))?
            }
            None => have,
        });
        let name: Vec<u8> = text(a, g.name);
        let seed_w: String = format!("{tag}#write{j}");
        let write: usize = a.add_node(
            &Spec::node("SeqOneShotSetNonStructGameDataString64", &seed_w)
                .prop(BOOL, "ExecuteEveryFrame", yes(false))
                .constant(S32, "GameDataIndex", int(-1))
                .constant(STR, "GameDataFullName", data_v.clone())
                .constant(STR, "Value", name),
        )?;
        let seed_s: String = format!("{tag}#apply{j}");
        let apply: usize = a.add_node(&Spec::node("SeqSyncPlayerEquipmentAndGameData", &seed_s).prop(BOOL, "ExecuteEveryFrame", yes(false)).constant(BOOL, "IsOnlyPouchPlayer", yes(false)))?;
        let seed_i: String = format!("{tag}#index{j}");
        let index: usize = a.add_node(&Spec::node("SeqOneShotPouchTargetSyncEquipmentIndex", &seed_i).prop(BOOL, "ExecuteEveryFrame", yes(false)))?;
        let plain: usize = parallel(a, &format!("{tag}#set{j}"), &[write, apply, index])?;
        sets.push(match quest {
            Some(_) => {
                let path: Vec<u8> = text(a, &format!("Work/Actor/{}.engine__actor__ActorParam.gyml", g.name));
                let seed_p: String = format!("{tag}#push{j}");
                let push: usize = a.add_node(&Spec::node(PUSH, &seed_p).prop(BOOL, "ExecuteEveryFrame", yes(false)).idle(S32, "Num", int(1)).constant(STR, "ActorName", path))?;
                let lend: usize = parallel(a, &format!("{tag}#lend{j}"), &[push, write, apply, index])?;
                selector(a, &format!("{tag}#have{j}"), have, 0, plain, lend)?
            }
            None => plain,
        });
    }
    let n: usize = list.len();
    let mut first: usize = dummy;
    for j in (0..n).rev() {
        first = selector(a, &format!("{tag}#first{j}"), owned[j], 0, sets[j], first)?;
    }
    let mut after: Vec<usize> = Vec::new();
    for i in 0..n {
        let mut next: usize = dummy;
        for k in (1..n).rev() {
            let j: usize = (i + k) % n;
            next = selector(a, &format!("{tag}#after{i}_{k}"), owned[j], 0, sets[j], next)?;
        }
        after.push(next);
    }
    let mut names: Vec<(usize, &str)> = Vec::new();
    for (i, g) in list.iter().enumerate() {
        names.push((i, g.name));
        names.extend(g.also.iter().map(|s| (i, *s)));
    }
    let mut pick: usize = first;
    for (e, (i, name)) in names.iter().enumerate().rev() {
        let c: usize = const_hash(a, &format!("{tag}#name{e}"), name)?;
        let seed_q: String = format!("{tag}#is{e}");
        let same: usize = a.add_node(&Spec::query(IS_EQUAL, &seed_q).linked_ptr("ValueA", U32, cur_hash).linked_ptr("ValueB", U32, c).out(BOOL, "IsEqual"))?;
        pick = selector(a, &format!("{tag}#current{e}"), same, 0, after[*i], pick)?;
    }
    Ok(pick)
}

/// "The mask quest is complete", read once per cycle.
fn quest_done(a: &mut Ainb, tag: &str) -> Res<usize> {
    let name: Vec<u8> = text(a, MASK_QUEST);
    let seed_q: String = format!("{tag}#quest");
    let step: usize = a.add_node(
        &Spec::query(GET_ENUM, &seed_q).idle(S32, "GameDataIndex", int(-1)).constant(BOOL, "IsDirect", yes(true)).constant(STR, "GameDataFullName", name).out_ptr("Value", U32),
    )?;
    let done: usize = const_hash(a, &format!("{tag}#quest_done"), QUEST_DONE)?;
    let seed_e: String = format!("{tag}#quest_is_done");
    a.add_node(&Spec::query(IS_EQUAL, &seed_e).linked_ptr("ValueA", U32, step).linked_ptr("ValueB", U32, done).out(BOOL, "IsEqual"))
}

struct Hook {
    hold: usize,
    trig: usize,
    read: usize,
    add: usize,
    dummy: usize,
}

/// One D-pad direction: a press starts the hold count, a hold steps it and cycles at the end,
/// anything else falls through to `next`.
fn direction(a: &mut Ainb, h: &Hook, at: i16, data: &str, list: &[Gear], next: usize) -> Res<usize> {
    let tag: String = format!("OOTRRGearCycle{at}");
    let start: usize = set_counter(a, &format!("{tag}#start"), START)?;
    let again: usize = set_counter(a, &format!("{tag}#again"), AGAIN)?;
    let step: usize = cycle(a, &tag, data, list, h.dummy)?;
    let fire: usize = parallel(a, &format!("{tag}#fire"), &[step, again])?;
    let timer: usize = float_switch(a, &format!("{tag}#timer"), h.read, &[(FLOOR, CYCLE_AT, h.add), (CYCLE_AT, CEIL, fire)], h.dummy)?;
    let held: usize = selector(a, &format!("{tag}#held"), h.hold, at, timer, next)?;
    mark_read(a, h.hold, at)?;
    mark_read(a, h.trig, at)?;
    selector(a, &format!("{tag}#pressed"), h.trig, at, start, held)
}

fn starts_hold(a: &Ainb, i: usize) -> bool {
    a.find_in(i, F32, "Value").and_then(|g| a.input(F32, g)).is_some_and(|x| x.val == START.to_le_bytes())
}

/// Adds the D-pad up and down branches where the counter graph resets the count because no
/// item button is held.
pub fn patch_counter(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    if a.find_els(SET_FLOAT).into_iter().any(|i| starts_hold(&a, i)) {
        return Err(Error::Format("gear cycle: the counter graph is already patched"));
    }
    let hold: usize = one(&a, HOLD_CHECK, "gear cycle: the counter graph has no single hold checker")?;
    let trig: usize = one(&a, TRIG_CHECK, "gear cycle: the counter graph has no single trigger checker")?;
    let read: usize = one_with(&a, GET_FLOAT, "GameDataFullName", COUNTER, "gear cycle: the counter graph does not read the counter once")?;
    let add: usize = one(&a, ADD_FLOAT, "gear cycle: the counter graph has no single counter step")?;
    let dummy: usize = one(&a, DUMMY, "gear cycle: the counter graph has no single do-nothing node")?;
    let last: usize = match readers(&a, hold, R).as_slice() {
        [i] => *i,
        _ => return Err(Error::Format("gear cycle: the counter graph does not test the held R once")),
    };
    let plug: usize = child(&a, last, "False")?;
    let reset: usize = a.params[last].plugs[plug].a as usize;
    if a.el_name(reset) != SET_FLOAT || a.in_str(reset, "GameDataFullName") != Some(COUNTER) {
        return Err(Error::Format("gear cycle: releasing both item buttons no longer resets the counter"));
    }
    let h: Hook = Hook { hold, trig, read, add, dummy };
    let down: usize = direction(&mut a, &h, DOWN, EQUIP_BOOT, &BOOTS, reset)?;
    let up: usize = direction(&mut a, &h, UP, EQUIP_HEAD, &HEAD, down)?;
    a.params[last].plugs[plug].a = up as u32;
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(Error::Msg(format!("gear cycle: counter graph audit: {}", bad.join("; "))));
    }
    a.write()
}

pub const SCHEDULER: &str = "AI/Player.scheduler.bootschange.module.ainb";

/// The boots-change scheduler picks Link's wait action where it picked the boots-change action
/// (the hop). Whether the boots still switch without that action is not yet confirmed in game.
pub fn patch_boots_hop(pack: &[u8]) -> Res<Vec<u8>> {
    let mut es: Vec<crate::sarc::Entry> = crate::sarc::parse(pack)?;
    let raw: Vec<u8> = crate::sarc::get(&es, SCHEDULER).ok_or(Error::Format("gear cycle: the player pack has no boots-change scheduler"))?.data.clone();
    let mut a: Ainb = Ainb::parse(&raw)?;
    let sel: usize = one(&a, "SelectorPlayerSchedulerBootsChange", "gear cycle: the scheduler has no single boots-change selector")?;
    let wait: usize = a.params[sel].plugs[child(&a, sel, "Wait")?].a as usize;
    let hop: usize = child(&a, sel, "BootsChange")?;
    if a.params[sel].plugs[hop].a as usize == wait {
        return Err(Error::Format("gear cycle: the boots-change hop is already dropped"));
    }
    a.params[sel].plugs[hop].a = wait as u32;
    crate::sarc::set(&mut es, SCHEDULER, a.write()?);
    crate::sarc::write(&es)
}

pub fn stage(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    Ok(vec![edit_plain(rf, have, COUNTER_FILE, patch_counter)?])
}

/// Writes the probe mod files under `out/K2-gear-cycle/romfs`: the patched counter graph, and
/// for each given player pack (the seed's own copies) that pack without the boots-change hop.
pub fn probe(rf: &mut Romfs, out: &std::path::Path, packs: &[std::path::PathBuf]) -> Res<Vec<std::path::PathBuf>> {
    let base: std::path::PathBuf = out.join("K2-gear-cycle").join("romfs");
    let d: std::path::PathBuf = base.join("Sequence");
    std::fs::create_dir_all(&d)?;
    let f: std::path::PathBuf = d.join("UpdateShortCutOpenHoldFrameCounter.module.ainb");
    std::fs::write(&f, patch_counter(&rf.read(COUNTER_FILE)?)?)?;
    let mut done: Vec<std::path::PathBuf> = vec![f];
    for p in packs {
        let z: Vec<u8> = std::fs::read(p)?;
        let id: Option<u32> = crate::flow_locations::dict_id(&z);
        let raw: Vec<u8> = patch_boots_hop(&rf.zs.decompress(&z)?)?;
        let name = p.file_name().ok_or(Error::Format("gear probe: a pack path has no file name"))?;
        let dir: std::path::PathBuf = base.join("Pack").join("Actor");
        std::fs::create_dir_all(&dir)?;
        let f: std::path::PathBuf = dir.join(name);
        std::fs::write(&f, crate::zs::compress_raw(&raw, id)?)?;
        done.push(f);
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ainb::graft::root;

    fn real() -> Option<Romfs> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.join("Sequence").join("UpdateShortCutOpenHoldFrameCounter.module.ainb").is_file() {
            Romfs::open(&p).ok()
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn kid(a: &Ainb, i: usize, name: &str) -> usize {
        a.params[i].plugs[child(a, i, name).unwrap()].a as usize
    }

    fn kids(a: &Ainb, i: usize) -> Vec<usize> {
        a.params[i].group(2).iter().map(|p| p.a as usize).collect()
    }

    fn reads(a: &Ainb, i: usize, k: usize) -> (usize, i16) {
        let x = a.input(k, a.find_in(i, k, "Input").unwrap()).unwrap();
        (x.src as usize, x.plug)
    }

    fn f32_in(a: &Ainb, i: usize) -> f32 {
        let v = a.input(F32, a.find_in(i, F32, "Value").unwrap()).unwrap().val;
        f32::from_le_bytes([v[0], v[1], v[2], v[3]])
    }

    /// The plain set of a chain step: masks pick it when owned, the push-first set otherwise.
    fn plain(a: &Ainb, set: usize) -> usize {
        if a.el_type(set) == BOOL_SELECTOR {
            kid(a, set, "True")
        } else {
            set
        }
    }

    /// The written names reachable from a cycle selector, in the order the chain tries them.
    fn tried(a: &Ainb, mut i: usize, dummy: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        while i != dummy {
            let set: usize = plain(a, kid(a, i, "True"));
            let write: usize = kids(a, set)[0];
            out.push(a.in_str(write, "Value").unwrap().to_string());
            i = kid(a, i, "False");
        }
        out
    }

    #[test]
    fn the_counter_graph_cycles_owned_gear_on_a_held_dpad() {
        let Some(rf) = real() else {
            return;
        };
        let raw: Vec<u8> = rf.read(COUNTER_FILE).unwrap();
        let before: Ainb = Ainb::parse(&raw).unwrap();
        let out: Vec<u8> = patch_counter(&raw).unwrap();
        let a: Ainb = Ainb::parse(&out).unwrap();
        assert!(audit(&a).is_empty());
        assert_eq!(root(&a), root(&before));
        assert_eq!(a.write().unwrap(), out, "the patched graph rewrites byte for byte");
        assert!(patch_counter(&out).is_err(), "a second patch is refused");
        assert_eq!(u32::from_le_bytes([out[0x14], out[0x15], out[0x16], out[0x17]]) as usize, a.els.iter().filter(|e| e[6] & QUERY != 0).count());

        let hold: usize = one(&a, HOLD_CHECK, "").unwrap();
        let trig: usize = one(&a, TRIG_CHECK, "").unwrap();
        let read: usize = one_with(&a, GET_FLOAT, "GameDataFullName", COUNTER, "").unwrap();
        let add: usize = one(&a, ADD_FLOAT, "").unwrap();
        let dummy: usize = one(&a, DUMMY, "").unwrap();
        let last: usize = readers(&a, hold, R)[0];
        let reset: usize = kid(&before, last, "False");

        let up: usize = kid(&a, last, "False");
        assert_eq!(reads(&a, up, BOOL), (trig, UP));
        let start: usize = kid(&a, up, "True");
        assert_eq!((a.el_name(start), a.in_str(start, "GameDataFullName"), f32_in(&a, start)), (SET_FLOAT, Some(COUNTER), START));
        let held: usize = kid(&a, up, "False");
        assert_eq!(reads(&a, held, BOOL), (hold, UP));
        let timer: usize = kid(&a, held, "True");
        assert_eq!(a.el_type(timer), F32_SWITCH);
        assert_eq!(reads(&a, timer, F32), (read, 0));
        let plugs = a.params[timer].group(2).to_vec();
        assert_eq!(plugs.len(), 3);
        let range = |e: &[u8]| (f32::from_le_bytes([e[4], e[5], e[6], e[7]]), f32::from_le_bytes([e[12], e[13], e[14], e[15]]));
        assert_eq!((plugs[0].a as usize, range(&plugs[0].extra)), (add, (FLOOR, CYCLE_AT)));
        assert_eq!(range(&plugs[1].extra), (CYCLE_AT, CEIL));
        assert!(CEIL < 1.0 && START < CYCLE_AT, "the hold never reaches the game's menu range");
        assert_eq!(plugs[2].a as usize, dummy);
        assert_eq!(a.s(u32::from_le_bytes([plugs[2].extra[4], plugs[2].extra[5], plugs[2].extra[6], plugs[2].extra[7]])), OTHER);
        let fire: Vec<usize> = kids(&a, plugs[1].a as usize);
        assert_eq!((fire.len(), f32_in(&a, fire[1])), (2, AGAIN), "each switch restarts the hold count, slower than the first");

        let mut pick: usize = fire[0];
        let mut found: Vec<(String, Vec<String>)> = Vec::new();
        while pick != dummy && a.el_type(pick) == BOOL_SELECTOR && a.el_name(reads(&a, pick, BOOL).0) == IS_EQUAL {
            let same: usize = reads(&a, pick, BOOL).0;
            let b = a.input(5, a.find_in(same, 5, "ValueB").unwrap()).unwrap();
            let name: String = a.in_str(b.src as usize, "String").unwrap().to_string();
            found.push((name, tried(&a, kid(&a, pick, "True"), dummy)));
            pick = kid(&a, pick, "False");
        }
        assert_eq!(found.len(), HEAD.len() + 2, "every mask, the Lens and its two age actors are recognised");
        let names: Vec<&str> = HEAD.iter().map(|g| g.name).collect();
        assert_eq!(found[0].0, "Armor_Child_Mask_Keaton");
        assert_eq!(found[0].1, names[1..].to_vec(), "after Keaton comes every other head item, Lens last");
        let bunny = found.iter().find(|f| f.0 == "Armor_Child_Mask_Bunny").unwrap();
        assert_eq!(bunny.1, [&names[4..], &names[..3]].concat());
        let child_lens = found.iter().find(|f| f.0 == "Armor_Child_GlassTruth").unwrap();
        assert_eq!(child_lens.1, names[..8].to_vec(), "the Lens wraps round to the masks");
        assert_eq!(tried(&a, pick, dummy), names, "an unknown current item starts from the first owned");
        let first_set: usize = kid(&a, pick, "True");
        assert_eq!(a.el_type(first_set), BOOL_SELECTOR);
        let have: usize = reads(&a, first_set, BOOL).0;
        assert_eq!(a.in_str(have, "GameDataFullName"), Some("IsGet.Armor_Child_Mask_Keaton"));
        let set = kids(&a, kid(&a, first_set, "True"));
        assert_eq!(set.iter().map(|&i| a.el_name(i)).collect::<Vec<&str>>(), ["SeqOneShotSetNonStructGameDataString64", "SeqSyncPlayerEquipmentAndGameData", "SeqOneShotPouchTargetSyncEquipmentIndex"]);
        assert_eq!(a.in_str(set[0], "GameDataFullName"), Some(EQUIP_HEAD));
        let lend = kids(&a, kid(&a, first_set, "False"));
        assert_eq!(a.el_name(lend[0]), PUSH, "a mask never borrowed goes into the pouch first");
        assert_eq!(a.in_str(lend[0], "ActorName"), Some("Work/Actor/Armor_Child_Mask_Keaton.engine__actor__ActorParam.gyml"));
        assert_eq!(&lend[1..], &set[..], "then it is written and applied like an owned one");
        let either: usize = reads(&a, pick, BOOL).0;
        assert_eq!(a.el_name(either), EITHER, "a mask is available when owned or when the mask quest is done");
        let ab = |n: &str| a.input(BOOL, a.find_in(either, BOOL, n).unwrap()).unwrap().src as usize;
        assert_eq!(ab("ValueA"), have);
        let done: usize = ab("ValueB");
        let step = a.input(5, a.find_in(done, 5, "ValueA").unwrap()).unwrap().src as usize;
        assert_eq!(a.in_str(step, "GameDataFullName"), Some(MASK_QUEST));
        let value = a.input(5, a.find_in(done, 5, "ValueB").unwrap()).unwrap().src as usize;
        assert_eq!(a.in_str(value, "String"), Some(QUEST_DONE));
        let lens_first: usize = {
            let mut i: usize = pick;
            for _ in 0..8 {
                i = kid(&a, i, "False");
            }
            i
        };
        assert_eq!(a.in_str(reads(&a, lens_first, BOOL).0, "GameDataFullName"), Some("IsGet.Armor_GlassTruth"), "the Lens needs the Lens itself");

        let down: usize = kid(&a, held, "False");
        assert_eq!(reads(&a, down, BOOL), (trig, DOWN));
        let down_held: usize = kid(&a, down, "False");
        assert_eq!(reads(&a, down_held, BOOL), (hold, DOWN));
        assert_eq!(kid(&a, down_held, "False"), reset, "with nothing held the game still resets its counter");
        let boot_fire: Vec<usize> = kids(&a, a.params[kid(&a, down_held, "True")].group(2)[1].a as usize);
        let heavy: usize = boot_fire[0];
        let hover_after = tried(&a, kid(&a, heavy, "True"), dummy);
        assert_eq!(hover_after, ["Armor_Adult_HoverBoot"]);
        assert_eq!(a.in_str(kids(&a, kid(&a, kid(&a, heavy, "True"), "True"))[0], "GameDataFullName"), Some(EQUIP_BOOT));

        let w = |a: &Ainb, src: usize, at: i16| {
            let p = &a.params[src];
            let o: usize = (p.ioe[BOOL][2] as usize + at as usize) * 4;
            u32::from_le_bytes([a.outs[BOOL][o], a.outs[BOOL][o + 1], a.outs[BOOL][o + 2], a.outs[BOOL][o + 3]])
        };
        for src in [hold, trig] {
            assert!(w(&a, src, UP) & LIVE != 0 && w(&a, src, DOWN) & LIVE != 0);
            assert!(w(&before, src, UP) & LIVE == 0);
        }
        let boot_have = a.find_els(EITHER).len();
        assert_eq!(boot_have, 8, "only the eight masks read the mask quest");
    }

    #[test]
    fn the_hop_probe_sends_the_boots_change_to_the_wait_action() {
        let Some(mut rf) = real() else {
            return;
        };
        for pack in ["Pack/Actor/Player_Child.pack.zs", "Pack/Actor/Player_Adult.pack.zs"] {
            let raw: Vec<u8> = rf.read_zs(pack).unwrap();
            let out: Vec<u8> = patch_boots_hop(&raw).unwrap();
            assert!(patch_boots_hop(&out).is_err(), "a second patch is refused");
            let es = crate::sarc::parse(&out).unwrap();
            let before = crate::sarc::parse(&raw).unwrap();
            assert_eq!(es.len(), before.len());
            let a: Ainb = Ainb::parse(&crate::sarc::get(&es, SCHEDULER).unwrap().data).unwrap();
            let sel: usize = one(&a, "SelectorPlayerSchedulerBootsChange", "").unwrap();
            assert_eq!(kid(&a, sel, "BootsChange"), kid(&a, sel, "Wait"));
            assert_eq!(crate::sarc::get(&es, SCHEDULER).unwrap().data.len(), crate::sarc::get(&before, SCHEDULER).unwrap().data.len(), "the graph keeps its size");
            for e in &before {
                if e.name != SCHEDULER {
                    assert_eq!(crate::sarc::get(&es, &e.name).unwrap().data, e.data, "{}", e.name);
                }
            }
        }
    }
}
