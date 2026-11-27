//! Skip Zelda Meeting, the gate-only Kakariko gate and the Weird Egg shuffle.
//!
//! Skip Zelda Meeting is the castle story state of crate::start_state::kakariko (the old open
//! Kakariko). The Kakariko gate setting now only sets the guard's flag: no quest moves, so the
//! castle visit and its checks stay.

use crate::bfevfl::{Bf, Flow};
use crate::error::Res;

pub const SKIP: &str = "skip_child_zelda";
pub const SKIP_NUM: u16 = 210;
pub const EGG: &str = "shuffle_weird_egg";
pub const EGG_NUM: u16 = 211;

pub const SKIP_NOTE: &str = "The Zelda meeting is skipped. A new game sets the castle quest complete, the Door of Time quest past Impa's send off, the Fire Stone quest past the gate and the gate guard's flag, so the Kakariko gate is open, the Happy Mask Shop quest has started and Malon and Talon are at Lon Lon Ranch. The items of Zelda's Letter and the Song from Impa are given, and Zelda's Letter and the Weird Egg are taken out of the item pool.";
pub const GATE_NOTE: &str = "The Kakariko gate starts open. Only the gate guard's flag is set: no quest moves on, so Malon's egg, Talon, Zelda and Impa stay where they are and keep their checks.";
pub const EGG_NOTE: &str = "The Weird Egg is shuffled. Malon's castle gift holds a random item, the egg hatches wherever it is carried, and its cucco wakes Talon.";

/// Malon's castle gift, whose vanilla item is the Weird Egg.
pub const EGG_LOC: &str = "HC Malon Egg";
/// The logic names of the child trade items Skip Zelda Meeting takes out of the pool.
pub const GONE_ITEMS: [&str; 3] = ["Zeldas Letter", "Weird Egg", "Chicken"];

/// (file stem, flow, quest, step, prefix): story requests that run only while
/// `<quest>_IsAfter_<step>_Exp` is false, so a story beat played late never moves a quest back.
/// A backward step request applies in the game (quest_order), and with the gate open, the forest
/// open and the egg anywhere, every one of these can come after the quest it asks for moved on:
/// - the Deku Tree exit asks the castle quest for Step01 (an open forest leaves the tree for later);
/// - Kaepora, the market entry, Malon's egg and the hatch ask for the castle steps in any order;
/// - Talon's wake scene asks for Step04 (also in the update's copy of the scene);
/// - Zelda asks the stones quest for Step01, Impa's send off for Step02 and the Fire Stone quest
///   for KakarikoVillageNpc, and the gate guard for KakarikoVillageLetter, after Goron City may
///   already have moved the Fire Stone quest on (the Lost Woods way in, or an open gate).
pub const ORDER: [(&str, &str, &str, &str, &str); 11] = [
    ("Dm_SY_BancChange_EscapeDungeon", "Dm_SY_BancChange_EscapeDungeon", "Goto_HyruleCastle", "Step01", "OOTRRZeldaOrderDeku"),
    ("Goto_HyruleCastle", "Goto_HyruleCastle", "Goto_HyruleCastle", "Step02", "OOTRRZeldaOrderOwl"),
    ("Goto_HyruleCastle", "Goto_HyruleCastle", "Goto_HyruleCastle", "Step02_01", "OOTRRZeldaOrderMarket"),
    ("Goto_HyruleCastle", "Goto_HyruleCastle", "Goto_HyruleCastle", "Step03", "OOTRRZeldaOrderEgg"),
    ("Goto_HyruleCastle", "Goto_HyruleCastle", "Goto_HyruleCastle", "Step03_02", "OOTRRZeldaOrderHatch"),
    ("Goto_HyruleCastle", "Goto_HyruleCastle", "DoorOfTime", "Step01", "OOTRRZeldaOrderStones"),
    ("Dm_HrlCastle_0020", "Dm_HrlCastle_0020", "Goto_HyruleCastle", "Step04", "OOTRRZeldaOrderTalon"),
    ("Dm_HrlCastle_0020.110", "Dm_HrlCastle_0020", "Goto_HyruleCastle", "Step04", "OOTRRZeldaOrderTalon"),
    ("DoorOfTime", "DoorOfTime", "DoorOfTime", "Step02", "OOTRRZeldaOrderSendOff"),
    ("DoorOfTime", "DoorOfTime", "Get_StoneOfFire", "KakarikoVillageNpc", "OOTRRZeldaOrderImpa"),
    ("Get_StoneOfFire", "Get_StoneOfFire", "Get_StoneOfFire", "KakarikoVillageLetter", "OOTRRZeldaOrderLetter"),
];

/// (file stem, flow, prefix): the stones quest's Srep2_1_1 requests (Saria's Song with the Fire
/// Stone, Zelda's talk about the Water Stone) also wait for Impa's send off: from Step01 they would
/// jump the quest past Impa, who only exists at Step01, and her Zelda's Lullaby would be lost.
/// They are in ORDER too (never backwards).
pub const AFTER_SEND_OFF: [(&str, &str, &str); 2] = [
    ("Dm_SY_Ocarina_SariaSong", "Dm_SY_Ocarina_SariaSong", "OOTRRZeldaSentSaria"),
    ("Npc_Hylia_Zelda_Child", "Npc_Hylia_Zelda_Child", "OOTRRZeldaSentZelda"),
];
pub const STONES: &str = "DoorOfTime";
pub const SRES: &str = "Srep2_1_1";
/// The stones quest is past Step01 once Impa's send off asked for Step02.
pub const SENT_OFF: &str = "DoorOfTime_IsAfter_Step01_Exp";

/// The stone count (DoorOfTime:DoorOfTime_CheckStoneNum) runs when the Fire or Water Stone's
/// dungeon is left and asks the stones quest for Step2_1, Step2_2 or Step03 by the stones held.
/// With the gate open both dungeons can come before Zelda: the count then either moved the quest
/// before the castle story (Step03 shuts the castle gate and its steel doors, losing Zelda, Malon's
/// egg and Impa) or was refused and never ran again (the Zelda escape, the Ocarina of Time and the
/// Running Man wait on Step03/Step04). It now only runs after Impa's send off, and the send off runs
/// it once itself.
pub const COUNT_ENTRY: &str = "DoorOfTime_CheckStoneNum";
pub const COUNT_FILE: &str = "Dm_SY_BancChange_EscapeDungeon";
pub const COUNT_PREFIX: &str = "OOTRRZeldaCount";
pub const SEND_OFF_ENTRY: &str = "DoorOfTime_SendOff";
pub const SEND_OFF_PREFIX: &str = "OOTRRZeldaSendOffCount";

/// Applies every ORDER guard of the file `stem`, and the send off and stone count edits that
/// share its files.
pub fn order(b: &mut Bf, stem: &str) -> Res<usize> {
    let mut n: usize = 0;
    if stem == STONES {
        send_off_cut(b)?;
        send_off_count(b)?;
    }
    if stem == COUNT_FILE {
        n += count_after_send_off(b)?;
    }
    for (_, flow, quest, step, prefix) in ORDER.iter().filter(|o| o.0 == stem) {
        n += super::quest_order::order(b, flow, quest, step, prefix)?;
    }
    for (_, flow, prefix) in AFTER_SEND_OFF.iter().filter(|o| o.0 == stem) {
        n += super::guard::guard_steps(b, flow, STONES, &[SRES], SENT_OFF, super::guard::Run::WhenTrue, prefix)?.len();
        n += super::quest_order::order(b, flow, STONES, SRES, &format!("{prefix}Order"))?;
    }
    Ok(n)
}

/// Guards every call of the stone count in the dungeon exit flow on the send off.
pub fn count_after_send_off(b: &mut Bf) -> Res<usize> {
    use crate::bfevfl::{Case, Cont, Event, Kind, Val};
    use crate::error::Error;
    let f = super::guard::flow_mut(b, COUNT_FILE)?;
    if f.events.iter().any(|e| e.name.starts_with(COUNT_PREFIX)) {
        return Err(Error::Msg(format!("zelda: {COUNT_FILE} already guards the stone count")));
    }
    let calls: Vec<usize> = (0..f.events.len()).filter(|&i| matches!(&f.events[i].kind, Kind::Sub { flow, entry, .. } if flow == STONES && entry == COUNT_ENTRY)).collect();
    if calls.is_empty() {
        return Err(Error::Msg(format!("zelda: {COUNT_FILE} no longer runs the stone count, the game files differ")));
    }
    let slot: (u16, u16) = super::guard::query_slot(f, super::guard::EXP)?;
    for (n, i) in calls.iter().copied().enumerate() {
        let Kind::Sub { next, .. } = f.events[i].kind else { unreachable!() };
        // A switch never ends a flow in the game: a skipped count that ended its entry ends on a
        // one-frame wait.
        let skip: u16 = if next == crate::bfevfl::flow::NONE {
            let (actor, act): (u16, u16) = crate::start_state::slot(f, crate::start_state::SYSTEM_ACTOR, crate::start_state::WAIT)?;
            let w: u16 = super::guard::idx(f.events.len())?;
            let items: Vec<(String, Val)> = vec![("IsWaitEnd".to_string(), Val::Bool(0x8000_0001)), ("Frames".to_string(), Val::Int(1))];
            f.events.push(Event { name: format!("{COUNT_PREFIX}End{n}"), kind: Kind::Action { next, actor, act, params: Some(Cont { items }) } });
            w
        } else {
            next
        };
        let g: u16 = super::guard::idx(f.events.len())?;
        let at: u16 = super::guard::idx(i)?;
        super::guard::retarget(f, at, g);
        // An entry's sub list names its sub calls (they are preloaded): it keeps naming the count.
        for e in f.entries.iter_mut() {
            for s in e.subs.iter_mut().filter(|s| **s == g) {
                *s = at;
            }
        }
        let params: Cont = Cont { items: vec![("GameDataName".to_string(), Val::Str(SENT_OFF.to_string()))] };
        let cases: Vec<Case> = vec![Case { val: 0, ev: at }, Case { val: 1, ev: skip }];
        f.events.push(Event { name: format!("{COUNT_PREFIX}{n}"), kind: Kind::Switch { actor: slot.0, query: slot.1, cases, params: Some(params) } });
    }
    Ok(calls.len())
}

/// Impa's send off runs the stone count right after its Fire Stone quest request, so stones won
/// before the send off count at once.
pub fn send_off_count(b: &mut Bf) -> Res<()> {
    use crate::bfevfl::{Event, Kind};
    use crate::error::Error;
    let f = super::guard::flow_mut(b, STONES)?;
    if f.events.iter().any(|e| e.name.starts_with(SEND_OFF_PREFIX)) {
        return Err(Error::Msg(format!("zelda: {STONES} already counts the stones at the send off")));
    }
    let fire: Vec<usize> = (0..f.events.len()).filter(|&i| super::guard::step_of(f, &f.events[i], "Get_StoneOfFire") == Some("KakarikoVillageNpc")).collect();
    let [i] = fire[..] else {
        return Err(Error::Msg(format!("zelda: {STONES} asks for KakarikoVillageNpc {} times, the game files differ", fire.len())));
    };
    let Kind::Action { next, .. } = f.events[i].kind else {
        return Err(Error::Msg("zelda: the send off's Fire Stone request is not an action".to_string()));
    };
    let settle: u16 = settle(f, next)?;
    let c: u16 = super::guard::idx(f.events.len())?;
    f.events.push(Event { name: format!("{SEND_OFF_PREFIX}0"), kind: Kind::Sub { next: settle, params: None, flow: String::new(), entry: COUNT_ENTRY.to_string() } });
    if let Kind::Action { next, .. } = &mut f.events[i].kind {
        *next = c;
    }
    for e in f.entries.iter_mut().filter(|e| e.name == SEND_OFF_ENTRY) {
        e.subs.push(c);
    }
    Ok(())
}

/// The door's own story (crate::door_gate: the escape area, the moat Ocarina, the door) runs on
/// flags and the door setting's requirement, so it can be done before Zelda is met while the stones
/// quest still waits on the castle quest. The send off then moves the quest on to where those flags
/// say it is, forward only: the escape played -> Step04 (and the drawbridge it closed at Step03
/// is open again), the Song of Time lesson played after it -> Step05, the door opened once the quest
/// is past Step04 -> Step06 (as door_gate does for an early door).
pub const SETTLE_PREFIX: &str = "OOTRRZeldaSettle";
pub const ESCAPED: &str = crate::door_gate::ESCAPED;
pub const SONG_OF_TIME: &str = "HavePlayedEvent.Dm_MelodyGet_Time";
pub const OPENED: &str = crate::door_gate::OPENED;
pub const BRIDGE: &str = "IsHyruleFieldBridgeClose";

fn push(f: &mut Flow, name: String, kind: crate::bfevfl::Kind) -> Res<u16> {
    let at: u16 = super::guard::idx(f.events.len())?;
    f.events.push(crate::bfevfl::Event { name, kind });
    Ok(at)
}

fn request_of(f: &mut Flow, quest: &str, step: &str, keep_done: bool, next: u16, name: String) -> Res<u16> {
    use crate::bfevfl::{Cont, Kind, Val};
    let (actor, act): (u16, u16) = crate::start_state::slot(f, crate::start_state::CHALLENGE_ACTOR, crate::start_state::PROGRESS)?;
    let items: Vec<(String, Val)> = vec![
        ("IsWaitEnd".to_string(), Val::Bool(0x8000_0001)),
        ("IsIgnoreCompletedChallenge".to_string(), Val::Bool(if keep_done { 0x8000_0001 } else { 0 })),
        ("IsIgnoreChallengeProgressUI".to_string(), Val::Bool(0x8000_0001)),
        ("ChallengeName".to_string(), Val::Str(quest.to_string())),
        ("StepName".to_string(), Val::Str(step.to_string())),
    ];
    push(f, name, Kind::Action { next, actor, act, params: Some(Cont { items }) })
}

/// A request for `step` of `quest` that runs only while the quest is not past it.
fn forward_of(f: &mut Flow, quest: &str, step: &str, next: u16, tag: &str) -> Res<u16> {
    let r: u16 = request_of(f, quest, step, true, next, format!("{SETTLE_PREFIX}{tag}"))?;
    super::quest_order::ask(f, super::quest_order::EXP_QUERY, &super::quest_order::exp_after(quest, step), next, r, format!("{SETTLE_PREFIX}{tag}Ahead"))
}

fn forward(f: &mut Flow, step: &str, next: u16, tag: &str) -> Res<u16> {
    forward_of(f, STONES, step, next, tag)
}

pub fn settle(f: &mut Flow, end: u16) -> Res<u16> {
    use crate::bfevfl::{Cont, Kind, Val};
    use super::quest_order::{ask, exp_after, BOOL_QUERY, EXP_QUERY};
    // A switch never ends a flow in the game: the chain ends on a one-frame wait.
    let (actor, act): (u16, u16) = crate::start_state::slot(f, crate::start_state::SYSTEM_ACTOR, crate::start_state::WAIT)?;
    let wait: Vec<(String, Val)> = vec![("IsWaitEnd".to_string(), Val::Bool(0x8000_0001)), ("Frames".to_string(), Val::Int(1))];
    let done: u16 = push(f, format!("{SETTLE_PREFIX}End"), Kind::Action { next: end, actor, act, params: Some(Cont { items: wait }) })?;
    let door_step: u16 = forward(f, "Step06", done, "Door")?;
    let door: u16 = ask(f, BOOL_QUERY, OPENED, door_step, done, format!("{SETTLE_PREFIX}DoorOpened"))?;
    let door_ready: u16 = ask(f, EXP_QUERY, &exp_after(STONES, "Step04"), door, done, format!("{SETTLE_PREFIX}DoorReady"))?;
    let song_step: u16 = forward(f, "Step05", door_ready, "Song")?;
    let song: u16 = ask(f, BOOL_QUERY, SONG_OF_TIME, song_step, door_ready, format!("{SETTLE_PREFIX}SongPlayed"))?;
    let escape_step: u16 = forward(f, "Step04", song, "Escape")?;
    let (dactor, dact): (u16, u16) = crate::start_state::slot(f, crate::start_state::DATA_ACTOR, crate::start_state::SET_BOOL)?;
    let open: Vec<(String, Val)> = vec![("Index".to_string(), Val::Int(-1)), ("Value".to_string(), Val::Bool(0)), ("GameDataName".to_string(), Val::Str(BRIDGE.to_string()))];
    let bridge: u16 = push(f, format!("{SETTLE_PREFIX}Bridge"), Kind::Action { next: escape_step, actor: dactor, act: dact, params: Some(Cont { items: open }) })?;
    ask(f, BOOL_QUERY, ESCAPED, bridge, door_ready, format!("{SETTLE_PREFIX}Escaped"))
}

/// Stages every always-on edit of this module as a late file, on top of what the location patcher
/// and the earlier stages wrote to the same flows. In the vanilla order every guard passes and
/// Malon's gift reads the same as before.
pub fn stage_late(rf: &mut crate::romfs::Romfs, late: &mut Vec<crate::staging::Out>, work: &std::path::Path) -> Res<()> {
    for stem in order_files() {
        crate::story_skip::stage_late(rf, late, work, stem, |b| {
            order(b, stem)?;
            if stem == CASTLE_FLOW {
                malon_gift(b)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------- the castle chain

/// The castle story flow: Malon's egg gift, the hatch, Talon's wake and the Zelda meeting.
pub const CASTLE_FLOW: &str = "Goto_HyruleCastle";
/// Malon's castle talk picks her gift or her chat on the castle quest step (gift while the quest
/// is not past Step02_01, her gift then asks for Step03).
pub const MALON_ASK: &str = "Event41";
pub const MALON_ASK_EXP: &str = "Goto_HyruleCastle_IsAfter_Step02_01_Exp";
/// Written right after Malon's gift (Event176 -> Event76, reached only from the gift's Step03
/// request), so it marks the gift as given whatever the quest step says.
pub const MALON_DONE: &str = "HaveTalked_Special.Goto_HyruleCastle_Goto_HyruleCastle_AfterMalonAppear_Event38";
/// Malon's Step03 request after her gift: in the game it starts the egg's hatch timer.
pub const MALON_STEP: &str = "Event46";
/// Showing the cucco to the sleeping Talon (his show-item reaction plays this entry).
pub const WAKE_ENTRY: &str = "Goto_HyruleCastle_AfterWakeUpTalon";
pub const WAKE_PREFIX: &str = "OOTRRZeldaWake";
pub const SHOW_FLOW: &str = "ShowItem";
/// The show-item event the game plays when nobody takes the item.
pub const SHOW_NOBODY: &str = "EntryPoint0";
/// The castle grounds area that hatches the egg on walking in (with the quest at Step03 in the
/// game). It runs the same entry as the timer's hatch (EggHatch, then Step03_02).
pub const HATCH_MODULE: &str = "Logic/HyruleCastle_Prefab_ee1b.logic.module.ainb";
pub const QUEST: &str = "Work/Challenge/Goto_HyruleCastle.game__challenge__Challenge.gyml";
pub const HATCH_STEP: &str = "Step03";
/// Set by every get of the egg, wherever it comes from; the hatch removes the egg itself.
pub const EGG_GOT: &str = "IsGet.Item_WeirdEgg";
const BOOL_QUERY: &str = "EventQueryIsGameDataBool";

/// Malon's gift is given once, by her own mark instead of the quest step. With the egg shuffled the
/// hatch moves the step on before or without her gift, and with the gate open anything else can;
/// her gift stays until she has given it (she is at the castle until Talon wakes).
pub fn malon_gift(b: &mut Bf) -> Res<()> {
    use crate::bfevfl::{Kind, Val};
    super::guard::requery(b, CASTLE_FLOW, MALON_ASK, MALON_ASK_EXP, BOOL_QUERY, MALON_DONE)?;
    let f = super::guard::flow_mut(b, CASTLE_FLOW)?;
    let e = f.events.iter_mut().find(|e| e.name == MALON_ASK).ok_or_else(|| crate::error::Error::Msg(format!("zelda: no {MALON_ASK} in {CASTLE_FLOW}")))?;
    if let Kind::Switch { params: Some(c), .. } = &mut e.kind
        && c.get("Index").is_none()
    {
        c.items.insert(0, ("Index".to_string(), Val::Int(-1)));
    }
    Ok(())
}

/// With the egg shuffled, Malon's gift no longer starts the hatch (her gift may be anything), and
/// Talon only wakes once Malon has given her gift: Malon leaves the castle when he wakes, so an egg
/// found elsewhere must not let her check go first. Showing him the cucco before that plays the
/// game's "nobody takes it" event.
pub fn egg_flow(b: &mut Bf) -> Res<()> {
    use crate::bfevfl::{Event, Kind};
    use crate::error::Error;
    let f = super::guard::flow_mut(b, CASTLE_FLOW)?;
    if f.events.iter().any(|e| e.name.starts_with(WAKE_PREFIX)) {
        return Err(Error::Msg(format!("zelda: {CASTLE_FLOW} already has the egg shuffle edits")));
    }
    let i: usize = f.events.iter().position(|e| e.name == MALON_STEP).ok_or_else(|| Error::Msg(format!("zelda: no {MALON_STEP} in {CASTLE_FLOW}")))?;
    if super::guard::step_of(f, &f.events[i], "Goto_HyruleCastle") != Some(HATCH_STEP) {
        return Err(Error::Msg(format!("zelda: {MALON_STEP} no longer asks for {HATCH_STEP}, the game files differ")));
    }
    let Kind::Action { next, .. } = f.events[i].kind else {
        return Err(Error::Msg(format!("zelda: {MALON_STEP} is not an action")));
    };
    super::guard::retarget(f, super::guard::idx(i)?, next);
    let w: usize = f.entries.iter().position(|e| e.name == WAKE_ENTRY).ok_or_else(|| Error::Msg(format!("zelda: no {WAKE_ENTRY} in {CASTLE_FLOW}")))?;
    let old: u16 = f.entries[w].main;
    let nobody: u16 = super::guard::idx(f.events.len())?;
    f.events.push(Event { name: format!("{WAKE_PREFIX}Nobody"), kind: Kind::Sub { next: crate::bfevfl::flow::NONE, params: None, flow: SHOW_FLOW.to_string(), entry: SHOW_NOBODY.to_string() } });
    let top: u16 = super::quest_order::ask(f, BOOL_QUERY, MALON_DONE, old, nobody, format!("{WAKE_PREFIX}Malon"))?;
    f.entries[w].main = top;
    f.entries[w].subs.push(nobody);
    Ok(())
}

/// With the egg shuffled the castle grounds hatch reads "the egg was got" instead of "the quest is at
/// Step03", so the egg hatches on walking in with it wherever it was found, and never without it.
pub fn hatch_module(a: &mut crate::ainb::Ainb) -> Res<usize> {
    use crate::error::Error;
    use crate::gerudo_card::{bool_node, give_queries, qpos, queries, rewire, BOOL, STEP_CHECK};
    let steps: Vec<usize> = a.find_els(STEP_CHECK).into_iter().filter(|&i| a.in_str(i, "ChallengeName") == Some(QUEST) && a.in_str(i, "ChallengeStepName") == Some(HATCH_STEP)).collect();
    let [s] = steps[..] else {
        return Err(Error::Msg(format!("zelda: the hatch area reads {} {HATCH_STEP} checks, the game files differ", steps.len())));
    };
    let read: usize = bool_node(a, EGG_GOT, "ootrr zelda egg got read")?;
    let mut moved: usize = 0;
    for i in 0..a.els.len() {
        if i == read || i == s {
            continue;
        }
        for g in a.input_ids(i, BOOL) {
            let x = a.input(BOOL, g).ok_or(Error::Format("zelda: input out of range"))?;
            if x.src >= 0 {
                if x.src as usize == s {
                    rewire(a, i, g, x.src, x.name, read, 0)?;
                    moved += 1;
                }
                continue;
            }
            if x.src > -100 || x.plug < 1 {
                continue;
            }
            let start: usize = (-100 - i32::from(x.src)) as usize;
            let n: usize = x.plug as usize;
            let mut hit: Vec<usize> = Vec::new();
            for j in start..start + n {
                let (src, _) = crate::door_open::multi_entry(a, j).ok_or(Error::Format("zelda: the multi link table is too short"))?;
                if usize::from(src) == s {
                    let e: u16 = u16::try_from(read).map_err(|_| Error::Format("zelda: element index out of range"))?;
                    a.multi[8 * j..8 * j + 2].copy_from_slice(&e.to_le_bytes());
                    a.multi[8 * j + 2..8 * j + 4].copy_from_slice(&0u16.to_le_bytes());
                    hit.push(j);
                    moved += 1;
                }
            }
            if hit.is_empty() {
                continue;
            }
            let p = &mut a.params[i];
            let (n0, s0): (usize, usize) = (p.pl[0][0] as usize, p.pl[0][1] as usize);
            for k in s0..s0 + n0 {
                if p.plugs[k].b == x.name && p.plugs[k].a == s as u32 {
                    p.plugs[k].a = read as u32;
                }
            }
            let q: u16 = qpos(a, read)?;
            let mut want: Vec<u16> = queries(a, i);
            if want.len() == n {
                for j in &hit {
                    want[j - start] = q;
                }
            } else {
                let old: Option<u16> = a.query_pos(s).and_then(|p| u16::try_from(p).ok());
                want.retain(|w| Some(*w) != old);
                if !want.contains(&q) {
                    want.push(q);
                }
            }
            give_queries(a, i, &want)?;
        }
    }
    if moved == 0 {
        return Err(Error::Msg("zelda: nothing in the hatch area reads its Step03 check, the game files differ".to_string()));
    }
    Ok(moved)
}

pub fn hatch_bytes(raw: &[u8]) -> Res<Vec<u8>> {
    use crate::error::Error;
    let mut a: crate::ainb::Ainb = crate::ainb::Ainb::parse(raw)?;
    hatch_module(&mut a)?;
    let out: Vec<u8> = a.write()?;
    let back: crate::ainb::Ainb = crate::ainb::Ainb::parse(&out)?;
    if back.write()? != out || !crate::ainb::audit(&back).is_empty() {
        return Err(Error::Format("zelda: the hatch module does not read back"));
    }
    Ok(out)
}

/// The Weird Egg shuffle edits (Plan.gates.egg): the castle flow on top of the always-on late
/// edit, and the hatch area's logic module.
pub fn stage_egg(rf: &mut crate::romfs::Romfs, late: &mut Vec<crate::staging::Out>, work: &std::path::Path) -> Res<()> {
    crate::story_skip::stage_late(rf, late, work, CASTLE_FLOW, egg_flow)?;
    for (flow, i, name) in crate::starter_bags::ADDS {
        crate::story_skip::stage_late(rf, late, work, flow, |b| egg_get(b, flow, i, name))?;
    }
    Ok(())
}

/// With the egg shuffled, the castle quest's Step03 (Malon's gift in the game, which starts the
/// egg's hatch timer) comes from getting the egg instead, wherever it is: right after each item
/// get's pouch add (crate::starter_bags::ADDS: chests and gifts, ground pickups, pickups with no
/// Link), while the Weird Egg is held and the quest is not past Step02_01. The game's own timer
/// and Navi's step start then hatch it as they do after Malon's gift. (The castle grounds hatch
/// area, the other trigger, lies past Talon in the courtyard, so it cannot be the way to the cucco.)
pub const EGG_GET_PREFIX: &str = "OOTRRZeldaEggGet";
pub const EGG_COUNT: &str = "PouchData.Item_WeirdEgg.StockNum";

pub fn egg_get(b: &mut Bf, flow: &str, i: usize, name: &str) -> Res<()> {
    use crate::bfevfl::{Case, Cont, Event, Kind, Val};
    use crate::error::Error;
    let f = super::guard::flow_mut(b, flow)?;
    if f.events.iter().any(|e| e.name.starts_with(EGG_GET_PREFIX)) {
        return Err(Error::Msg(format!("zelda: {flow} already starts the egg's hatch")));
    }
    let next: u16 = match f.events.get(i) {
        Some(Event { name: n, kind: Kind::Action { next, .. } }) if n == name => *next,
        _ => return Err(Error::Msg(format!("zelda: event {i} of {flow} is not the pouch add {name}, the game files differ"))),
    };
    let then: u16 = if next == crate::bfevfl::flow::NONE { wait(f, 1, next, format!("{EGG_GET_PREFIX}End"))? } else { next };
    let step: u16 = request_of(f, "Goto_HyruleCastle", HATCH_STEP, true, then, format!("{EGG_GET_PREFIX}Step"))?;
    let early: u16 = super::quest_order::ask(f, super::quest_order::EXP_QUERY, MALON_ASK_EXP, then, step, format!("{EGG_GET_PREFIX}Early"))?;
    let (a, x): (u16, u16) = super::guard::query_slot(f, "EventQueryIsGameDataInt")?;
    // Below 0 is a parked egg (adult trade shuffle's child group): still held.
    let cases: Vec<Case> = vec![Case { val: 0, ev: early }, Case { val: 1, ev: then }, Case { val: 2, ev: early }];
    let params: Cont = Cont { items: vec![("Index".to_string(), Val::Int(-1)), ("GameDataName".to_string(), Val::Str(EGG_COUNT.to_string())), ("Value".to_string(), Val::Int(0))] };
    let held: u16 = push(f, format!("{EGG_GET_PREFIX}Held"), Kind::Switch { actor: a, query: x, cases, params: Some(params) })?;
    if let Kind::Action { next, .. } = &mut f.events[i].kind {
        *next = held;
    }
    Ok(())
}

// ---------------------------------------------------------------- Saria in the meadow

/// After the Zelda meeting Saria waits in the Sacred Forest Meadow with her song. Her meadow copy and
/// the song's trigger area exist once the stones quest is past Step01 (Impa's send off), which waits
/// on the castle quest and has refused requests made right after it; they now read the castle quest
/// complete, the meeting itself, as the logic does (Song from Saria needs Zelda met). In the forest
/// she already leaves on the castle quest's completion.
pub const MEADOW_MODULES: [&str; 2] = ["Logic/SacredForestMeadow_Child_Prefab_3265.logic.module.ainb", "Logic/SacredForestMeadow_Child_Prefab_bbc3.logic.module.ainb"];
pub const MEADOW_FROM: &str = "DoorOfTime_IsAfter_Step01_Exp";
pub const MEADOW_TO: &str = "Goto_HyruleCastle_IsCompleted_Exp";

pub fn meadow(a: &mut crate::ainb::Ainb) -> Res<usize> {
    use crate::error::Error;
    const READ: &str = "HoldGameDataGetBoolExpSimple";
    const NAME: &str = "GameDataNameBoolExp";
    let hits: Vec<usize> = a.find_els(READ).into_iter().filter(|&i| a.in_str(i, NAME) == Some(MEADOW_FROM)).collect();
    if hits.is_empty() {
        return Err(Error::Msg(format!("zelda: the meadow module no longer reads {MEADOW_FROM}, the game files differ")));
    }
    let to: u32 = a.pool.add(MEADOW_TO);
    for i in &hits {
        let g: usize = a.find_in(*i, 3, NAME).ok_or(Error::Format("zelda: the meadow read has no name input"))?;
        a.set_in_val(3, g, &to.to_le_bytes())?;
    }
    Ok(hits.len())
}

pub fn meadow_bytes(raw: &[u8]) -> Res<Vec<u8>> {
    use crate::error::Error;
    let mut a: crate::ainb::Ainb = crate::ainb::Ainb::parse(raw)?;
    meadow(&mut a)?;
    let out: Vec<u8> = a.write()?;
    let back: crate::ainb::Ainb = crate::ainb::Ainb::parse(&out)?;
    if back.write()? != out || !crate::ainb::audit(&back).is_empty() {
        return Err(Error::Format("zelda: the meadow module does not read back"));
    }
    Ok(out)
}

pub fn stage_meadow(rf: &mut crate::romfs::Romfs, late: &mut Vec<crate::staging::Out>, work: &std::path::Path) -> Res<()> {
    let have = |p: &str| -> Option<Vec<u8>> { std::fs::read(p.split('/').fold(work.to_path_buf(), |a, s| a.join(s))).ok() };
    for m in MEADOW_MODULES {
        super::fortress::merge_plain(rf, late, &have, m, meadow_bytes)?;
    }
    Ok(())
}

/// The files ORDER edits, once each.
pub fn order_files() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for s in ORDER.iter().map(|o| o.0).chain(AFTER_SEND_OFF.iter().map(|o| o.0)) {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

// ---------------------------------------------------------------- always-on intro skips

/// Link's house door opening (Dm_Kokiri_0045, which writes the last two) and Saria's first
/// greeting in front of the house (Dm_Kokiri_0050, her stop and her after-talk), marked as played
/// at every new game whatever the settings. The Kakariko state wrote the same set, played in game.
pub const KOKIRI_INTRO: [&str; 7] = [
    "HavePlayedEvent.Dm_Kokiri_0045",
    "HavePlayedEvent.Dm_Kokiri_0050",
    // Saria's first talk, where she meets Navi (played from her KokiriForest_StopInFrontOfLinkHouse
    // talk): its trigger and areas go once it has played, and nothing else waits on it (her bridge
    // gift reads only its own scenes).
    "HavePlayedEvent.Dm_Kokiri_0060",
    "HavePlayedEvent.KokiriForest_StopInFrontOfLinkHouse",
    "HavePlayedEvent.Dm_Kokiri_0050_After_Near",
    "Dm_Kokiri_0050_After_Near_End",
    "KokiriForest_LinkHouseDoor_Open",
];
/// The first Hyrule Field owl (Goto_HyruleCastle_Kaepora_00): its area
/// (HyruleField_Child_Prefab_de5f) only fires while Dm_HrlField_0010 has not played, so the scenes
/// are marked as played and what the owl's event leaves behind is written instead: the Threads of
/// Time menu, the field arrival, the cleared first-visit palette and Navi's field hint. Its castle
/// quest step is not written (nothing gated on it holds a check).
pub const FIELD_OWL: [(&str, bool); 6] = [
    ("HavePlayedEvent.Dm_HrlField_0005", true),
    ("HavePlayedEvent.Dm_HrlField_0010", true),
    ("HavePlayedEvent.HyruleField_Goto_HyruleCastle_Kaepora", true),
    ("UI_IsFinishChallengeMenuOpenEvent", true),
    ("NaviHint_Disp_FieldHint_01", false),
    (crate::start_state::FIELD_FIRST_VISIT, false),
];
/// What Link's first steps into Hyrule Field leave behind in a vanilla save that this build's saves
/// lacked (diff of a frozen-clock save against the eden playthrough saves): the field opening
/// scene (Dm_HrlField_0000, which also marks the field visited, see crate::start_state::visits),
/// the field arrival boot event (Saria's bridge cut keeps Link on the bridge, so it never runs),
/// Kaepora's talk marker and the opening's own load event. The clock is code side; these are the
/// field intro state the game has whenever it runs. Written at every new game and again at the end
/// of Saria's gift (crate::story_skip::patch_saria_give).
pub const FIELD_INTRO: [(&str, bool); 9] = [
    ("HavePlayedEvent.Dm_HrlField_0000", true),
    ("HavePlayedEvent.Dm_HrlField_0005", true),
    ("HavePlayedEvent.Dm_HrlField_0010", true),
    ("IsVisitLocation.HyruleField", true),
    ("HavePlayedEvent.Dm_SY_BancChangeArrive_HyruleField", true),
    ("HaveTalked.Npc_Kaepora", true),
    ("HavePlayedEvent.Opening_LoadingKokiriForest", true),
    (crate::start_state::FIELD_ARRIVED, true),
    (crate::start_state::FIELD_FIRST_VISIT, false),
];

/// The owl also hands out the amiibo pouch.
pub const OWL_ITEM: &str = "Pouch_Amiibo";
/// The world clock runs from the start (always on, no setting): the owl's field arrival
/// (crate::start_state::FIELD_ARRIVED) and Impa's send off (Dm_HrlField_0020) are the two clock
/// gates the clock setting wrote, and a save diff found the clock also waits on Zelda's courtyard
/// talk markers (crate::start_state::ZELDA_TALK), which the Zelda meeting cut no longer writes.
pub const CLOCK: [&str; 2] = [crate::start_state::FIELD_ARRIVED, "HavePlayedEvent.Dm_HrlField_0020"];

/// The castle quest step the first field owl asks for. Before it the clock stood still (the owl
/// was the only thing that moved the quest to Step02, and the game's clock gate is code side); the
/// skipped owl's area is gone once the quest is past Step01. Nothing in the forest reads it (the
/// Kokiri read only the castle quest's completion).
pub const OWL_STEPS: [(&str, &str); 2] = [("Goto_HyruleCastle", "Ready"), ("Goto_HyruleCastle", "Step02")];
/// The clock the field opening sets (Dm_HrlField_0000 eases it to 7:30).
pub const OWL_TIME: (i32, i32) = (7, 30);

/// Every new game: the Kokiri intro, the first field owl and the clock gates, and the in-game music
/// the skipped door scene would start. `castle` is false when another state
/// (crate::start_state::kakariko) already moves the castle quest on and gives the owl's item.
pub fn new_game(castle: bool) -> crate::start_state::Ops {
    let mut flags: Vec<(String, bool)> = KOKIRI_INTRO.iter().map(|n| (n.to_string(), true)).collect();
    flags.extend(FIELD_OWL.iter().map(|(n, v)| (n.to_string(), *v)));
    flags.extend(CLOCK.iter().map(|n| (n.to_string(), true)));
    flags.extend(crate::start_state::ZELDA_TALK.iter().map(|n| (n.to_string(), true)));
    for (n, v) in FIELD_INTRO {
        if !flags.iter().any(|(m, _)| m == n) {
            flags.push((n.to_string(), v));
        }
    }
    let steps: Vec<(String, String)> = if castle { OWL_STEPS.iter().map(|(c, s)| (c.to_string(), s.to_string())).collect() } else { Vec::new() };
    crate::start_state::Ops {
        steps,
        flags,
        items: if castle { vec![OWL_ITEM.to_string()] } else { Vec::new() },
        time: Some(OWL_TIME),
        bgm: true,
        ..crate::start_state::Ops::default()
    }
}

// ---------------------------------------------------------------- the Zelda meeting

/// The meeting (Goto_HyruleCastle_MeetZelda, started by the courtyard area at castle quest Step04)
/// keeps its start, which takes Link into the event, and then runs none of its scenes: the screen
/// fades back in, Zelda's Letter's check and Impa's Zelda's Lullaby check are given as item gets,
/// the scenes are marked as played, the castle quest completes, the stones quest is asked for Step01,
/// and Link goes to Hyrule Field the way Impa's send off takes him (map change booting
/// HyruleField_DoorOfTime_SendOff, whose ride scene is cut, see [`SEND_OFF_SCENE`]). He lands in
/// control in front of the castle bridge.
pub const MEET_ENTRY: &str = "Goto_HyruleCastle_MeetZelda";
/// The last of the meeting's start steps (it clears projectiles; Zelda joins after it).
pub const MEET_FROM: (&str, &str) = ("Event206", "EventRemoveProjectiles");
pub const MEET_PREFIX: &str = "OOTRRZeldaMeet";
pub const MEET_SCENES: [&str; 7] = [
    "HavePlayedEvent.Dm_HrlCastle_0030",
    "HavePlayedEvent.Dm_HrlCastle_0035",
    "HavePlayedEvent.Dm_HrlCastle_0045",
    "HavePlayedEvent.Dm_HrlCastle_0050",
    "HavePlayedEvent.Dm_HrlCastle_0060",
    "HavePlayedEvent.Dm_MelodyGet_Zelda",
    super::MEET_ZELDA,
];
pub const LETTER_LOC: &str = "HC Zeldas Letter";
pub const SONG_LOC: &str = "Song from Impa";
/// The forest exit's map change in the same flow, whose parameters the meeting's copies.
pub const FIELD_CHANGE: &str = "Event42";
pub const SEND_OFF_POS: &str = "DoorOfTime_MeetImpa_EndPos";
pub const SEND_OFF_BOOT: &str = "HyruleField_DoorOfTime_SendOff";
/// Impa's ride out of the castle at the start of the send off.
pub const SEND_OFF_SCENE: (&str, &str) = ("Event10", "Dm_HrlField_0020");
pub const SEND_OFF_CUT: &str = "OOTRRZeldaSendOffCut";
const SETTLE: i32 = 30;

fn set_flag(f: &mut Flow, name: &str, v: bool, next: u16, label: String) -> Res<u16> {
    use crate::bfevfl::{Cont, Kind, Val};
    let (actor, act): (u16, u16) = crate::start_state::slot(f, crate::start_state::DATA_ACTOR, crate::start_state::SET_BOOL)?;
    let items: Vec<(String, Val)> = vec![("Index".to_string(), Val::Int(-1)), ("Value".to_string(), Val::Bool(if v { 0x8000_0001 } else { 0 })), ("GameDataName".to_string(), Val::Str(name.to_string()))];
    push(f, label, Kind::Action { next, actor, act, params: Some(Cont { items }) })
}

fn wait(f: &mut Flow, frames: i32, next: u16, label: String) -> Res<u16> {
    use crate::bfevfl::{Cont, Kind, Val};
    let (actor, act): (u16, u16) = crate::start_state::slot(f, crate::start_state::SYSTEM_ACTOR, crate::start_state::WAIT)?;
    let items: Vec<(String, Val)> = vec![("IsWaitEnd".to_string(), Val::Bool(0x8000_0001)), ("Frames".to_string(), Val::Int(frames))];
    push(f, label, Kind::Action { next, actor, act, params: Some(Cont { items }) })
}

pub fn meeting(b: &mut Bf, letter: &crate::story_skip::SceneGive, song: &crate::story_skip::SceneGive) -> Res<()> {
    use crate::bfevfl::{Cont, Kind, Val};
    use crate::error::Error;
    let f = super::guard::flow_mut(b, CASTLE_FLOW)?;
    if f.events.iter().any(|e| e.name.starts_with(MEET_PREFIX)) {
        return Err(Error::Msg(format!("zelda: {CASTLE_FLOW} already has the meeting cut")));
    }
    let from: usize = f.events.iter().position(|e| e.name == MEET_FROM.0).ok_or_else(|| Error::Msg(format!("zelda: no {} in {CASTLE_FLOW}", MEET_FROM.0)))?;
    let is_from: bool = matches!(&f.events[from].kind, Kind::Action { actor, act, .. } if f.actors.get(*actor as usize).and_then(|a| a.actions.get(*act as usize)).map(|s| s.as_str()) == Some(MEET_FROM.1));
    if !is_from {
        return Err(Error::Msg(format!("zelda: {} is no longer the meeting's {}, the game files differ", MEET_FROM.0, MEET_FROM.1)));
    }
    let change: usize = f.events.iter().position(|e| e.name == FIELD_CHANGE).ok_or_else(|| Error::Msg(format!("zelda: no {FIELD_CHANGE} in {CASTLE_FLOW}")))?;
    let Kind::Action { actor: ca, act: cx, params: Some(cp), .. } = f.events[change].kind.clone() else {
        return Err(Error::Msg(format!("zelda: {FIELD_CHANGE} is not the forest exit's map change")));
    };
    if cp.get("NextBanc") != Some(&Val::Str("HyruleField_Child".to_string())) {
        return Err(Error::Msg(format!("zelda: {FIELD_CHANGE} no longer goes to Hyrule Field, the game files differ")));
    }
    let mut items: Vec<(String, Val)> = cp.items.clone();
    for (k, v) in items.iter_mut() {
        match k.as_str() {
            "NextStartPos" => *v = Val::Str(SEND_OFF_POS.to_string()),
            "BootEventName" => *v = Val::Str(SEND_OFF_BOOT.to_string()),
            "NextStartRot" => *v = Val::Floats(vec![0.0, 180.0, 0.0]),
            "FadePattern" => *v = Val::Int(0),
            _ => {}
        }
    }
    let go: u16 = push(f, format!("{MEET_PREFIX}Field"), Kind::Action { next: crate::bfevfl::flow::NONE, actor: ca, act: cx, params: Some(Cont { items }) })?;
    // The stones quest waits on the castle quest: a short wait after Complete (it refused a request
    // made in the same instant at new game), then Step01, never backwards.
    let stones: u16 = forward_of(f, STONES, "Step01", go, "MeetStones")?;
    let settle: u16 = wait(f, SETTLE, stones, format!("{MEET_PREFIX}Settle"))?;
    let done: u16 = request_of(f, "Goto_HyruleCastle", "Complete", false, settle, format!("{MEET_PREFIX}Complete"))?;
    let mut next: u16 = done;
    for (n, flag) in MEET_SCENES.iter().enumerate().rev() {
        next = set_flag(f, flag, true, next, format!("{MEET_PREFIX}Played{n}"))?;
    }
    let lullaby: u16 = crate::story_skip::give_item(f, &format!("{MEET_PREFIX}Song"), song, next, &[MEET_ENTRY])?;
    let note: u16 = crate::story_skip::give_item(f, &format!("{MEET_PREFIX}Letter"), letter, lullaby, &[MEET_ENTRY])?;
    let (fa, fx): (u16, u16) = crate::start_state::slot(f, "UISystemActor", "EventEventFadeIn")?;
    let fade: Vec<(String, Val)> = vec![("Color".to_string(), Val::Int(0)), ("IsNoPlayAnime".to_string(), Val::Bool(0)), ("Length".to_string(), Val::Int(1)), ("IsWaitEnd".to_string(), Val::Bool(0x8000_0001))];
    let first: u16 = push(f, format!("{MEET_PREFIX}FadeIn"), Kind::Action { next: note, actor: fa, act: fx, params: Some(Cont { items: fade }) })?;
    if let Kind::Action { next, .. } = &mut f.events[from].kind {
        *next = first;
    }
    Ok(())
}

/// The send off starts with Impa's ride out of the castle; it becomes its played flag, so the map
/// change from the meeting lands Link in control.
pub fn send_off_cut(b: &mut Bf) -> Res<()> {
    let f = super::guard::flow_mut(b, STONES)?;
    crate::story_skip::cut_scene(f, SEND_OFF_CUT, SEND_OFF_SCENE.0, SEND_OFF_SCENE.1).map(|_| ())
}

/// Stages the meeting cut (the castle flow) on top of every earlier edit. The send off's cut is
/// part of the always-on stones edits ([`order`]).
pub fn stage_meeting(rf: &mut crate::romfs::Romfs, late: &mut Vec<crate::staging::Out>, work: &std::path::Path, letter: &crate::story_skip::SceneGive, song: &crate::story_skip::SceneGive) -> Res<()> {
    crate::story_skip::stage_late(rf, late, work, CASTLE_FLOW, |b| meeting(b, letter, song))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfevfl::{Flow, Kind, Val};
    use crate::romfs::Romfs;
    use std::path::PathBuf as PB;

    fn real() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(p)
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn load(rf: &mut Romfs, stem: &str) -> Bf {
        Bf::parse(&rf.read_zs(&format!("Event/EventFlow/{stem}.bfevfl.zs")).unwrap()).unwrap()
    }

    /// Every request ORDER names is reached only through a switch on its IsAfter expression, whose
    /// false case runs it and whose true case skips to what came after it.
    #[test]
    fn every_late_story_request_is_guarded_against_moving_a_quest_back() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        for stem in order_files() {
            let mut b: Bf = load(&mut rf, stem);
            let n: usize = order(&mut b, stem).unwrap();
            let want: Vec<&(&str, &str, &str, &str, &str)> = ORDER.iter().filter(|o| o.0 == stem).collect();
            assert!(n >= want.len(), "{stem}: {n} guards for {} entries", want.len());
            let back: Bf = Bf::parse(&b.write().unwrap()).unwrap();
            for (_, flow, quest, step, prefix) in want {
                let f: &Flow = back.flows.iter().find(|f| f.name == *flow).unwrap();
                let exp: String = super::super::quest_order::exp_after(quest, step);
                for (i, e) in f.events.iter().enumerate() {
                    if super::super::guard::step_of(f, e, quest) != Some(step) {
                        continue;
                    }
                    let callers: Vec<&str> = f
                        .events
                        .iter()
                        .filter(|g| match &g.kind {
                            Kind::Action { next, .. } | Kind::Sub { next, .. } | Kind::Join { next } => *next as usize == i,
                            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter().any(|c| c.ev as usize == i),
                            Kind::Fork { forks, .. } => forks.iter().any(|x| *x as usize == i),
                            Kind::Label { .. } => false,
                        })
                        .map(|g| g.name.as_str())
                        .collect();
                    assert!(!callers.is_empty() && callers.iter().all(|c| c.starts_with(prefix)), "{stem}: {quest} {step} is reached from {callers:?}");
                    let g = f.events.iter().find(|g| g.name.starts_with(prefix) && matches!(&g.kind, Kind::Switch { cases, .. } if cases.iter().any(|c| c.val == 1 && c.ev as usize == i))).unwrap();
                    assert_eq!(g.kind.params().and_then(|c| c.get("GameDataName")), Some(&Val::Str(exp.clone())), "{stem}");
                    assert!(f.entries.iter().all(|en| en.main as usize != i), "{stem}: an entry still starts at the request");
                }
            }
        }
    }

    #[test]
    fn the_skip_and_the_gate_are_separate_settings() {
        let s: &crate::settings::Schema = crate::settings::schema();
        let skip = s.get(SKIP).unwrap();
        let egg = s.get(EGG).unwrap();
        assert_eq!((skip.num, egg.num), (SKIP_NUM, EGG_NUM));
        assert!(s.settings.iter().filter(|x| x.num == SKIP_NUM || x.num == EGG_NUM).count() == 2);
        assert!(egg.deps.iter().any(|d| d.on == SKIP));
    }

    fn named<'a>(f: &'a Flow, n: &str) -> &'a crate::bfevfl::Event {
        f.events.iter().find(|e| e.name == n).unwrap_or_else(|| panic!("no {n}"))
    }

    #[test]
    fn the_stone_count_waits_for_the_send_off_and_the_send_off_counts() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let mut b: Bf = load(&mut rf, COUNT_FILE);
        assert_eq!(count_after_send_off(&mut b).unwrap(), 2);
        let f: &Flow = b.flows.iter().find(|f| f.name == COUNT_FILE).unwrap();
        for e in f.events.iter().filter(|e| e.name.starts_with(COUNT_PREFIX) && !e.name.contains("End")) {
            assert_eq!(e.kind.params().and_then(|c| c.get("GameDataName")), Some(&Val::Str(SENT_OFF.to_string())));
            let Kind::Switch { cases, .. } = &e.kind else { panic!("{} is not a switch", e.name) };
            let yes = &f.events[cases.iter().find(|c| c.val == 0).unwrap().ev as usize];
            assert!(matches!(&yes.kind, Kind::Sub { flow, entry, .. } if flow == STONES && entry == COUNT_ENTRY));
            assert!(cases.iter().all(|c| c.ev != crate::bfevfl::flow::NONE));
        }
        Bf::parse(&b.write().unwrap()).unwrap();
        let mut d: Bf = load(&mut rf, STONES);
        send_off_count(&mut d).unwrap();
        order(&mut load(&mut rf, STONES), STONES).unwrap();
        let f: &Flow = d.flows.iter().find(|f| f.name == STONES).unwrap();
        let fire = named(f, "Event62");
        let Kind::Action { next, .. } = fire.kind else { panic!() };
        assert!(matches!(&f.events[next as usize].kind, Kind::Sub { entry, .. } if entry == COUNT_ENTRY));
        assert!(f.entries.iter().find(|e| e.name == SEND_OFF_ENTRY).unwrap().subs.contains(&next));
        // After the count, the quest is settled to the door flags, forward only.
        let Kind::Sub { next: top, .. } = f.events[next as usize].kind else { panic!() };
        assert_eq!(f.events[top as usize].kind.params().and_then(|c| c.get("GameDataName")), Some(&Val::Str(ESCAPED.to_string())));
        let asked: Vec<String> = f.events.iter().filter(|e| e.name.starts_with(SETTLE_PREFIX)).filter_map(|e| match e.kind.params().and_then(|c| c.get("StepName")) {
            Some(Val::Str(s)) => Some(s.clone()),
            _ => None,
        }).collect();
        assert_eq!(asked, vec!["Step06", "Step05", "Step04"]);
        Bf::parse(&d.write().unwrap()).unwrap();
    }

    #[test]
    fn the_srep_requests_wait_for_the_send_off_and_never_go_back() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        for (stem, flow, prefix) in AFTER_SEND_OFF {
            let mut b: Bf = load(&mut rf, stem);
            assert!(order(&mut b, stem).unwrap() >= 2, "{stem}");
            let f: &Flow = b.flows.iter().find(|f| f.name == flow).unwrap();
            let reads: Vec<String> = f.events.iter().filter(|e| e.name.starts_with(prefix)).filter_map(|e| match e.kind.params().and_then(|c| c.get("GameDataName")) {
                Some(Val::Str(s)) => Some(s.clone()),
                _ => None,
            }).collect();
            assert!(reads.contains(&SENT_OFF.to_string()) && reads.contains(&super::super::quest_order::exp_after(STONES, SRES)), "{stem}: {reads:?}");
        }
    }

    #[test]
    fn the_meeting_gives_both_checks_completes_the_castle_and_leaves_for_the_field() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let mut b: Bf = load(&mut rf, CASTLE_FLOW);
        order(&mut b, CASTLE_FLOW).unwrap();
        malon_gift(&mut b).unwrap();
        let g = |a: &str| crate::story_skip::SceneGive { actor: a.to_string(), family: None, key: None, line: None };
        meeting(&mut b, &g("Item_Boomerang"), &g("Item_Melody_ZeldaLullaby")).unwrap();
        assert!(meeting(&mut b, &g("Item_Boomerang"), &g("Item_Melody_ZeldaLullaby")).is_err(), "a second cut is refused");
        let back: Bf = Bf::parse(&b.write().unwrap()).unwrap();
        let f: &Flow = back.flows.iter().find(|f| f.name == CASTLE_FLOW).unwrap();
        // Walk the entry from its start: Zelda never joins, every scene is gone, the chain ends on
        // the map change to the send off.
        let main: u16 = f.entries.iter().find(|e| e.name == MEET_ENTRY).unwrap().main;
        let mut seen: Vec<usize> = Vec::new();
        let mut todo: Vec<u16> = vec![main];
        while let Some(i) = todo.pop() {
            if i == crate::bfevfl::flow::NONE || seen.contains(&(i as usize)) {
                continue;
            }
            seen.push(i as usize);
            match &f.events[i as usize].kind {
                Kind::Action { next, .. } | Kind::Sub { next, .. } | Kind::Join { next } => todo.push(*next),
                Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
                Kind::Fork { join, forks } => {
                    todo.push(*join);
                    todo.extend(forks.iter().copied());
                }
                Kind::Label { .. } => {}
            }
        }
        let names: Vec<&str> = seen.iter().map(|&i| f.events[i].name.as_str()).collect();
        assert!(!names.contains(&"Event27") && !names.contains(&"Event216"), "Zelda's part still plays: {names:?}");
        let params = |k: &str| -> Vec<String> {
            seen.iter().filter_map(|&i| match f.events[i].kind.params().and_then(|c| c.get(k)) {
                Some(Val::Str(s)) => Some(s.clone()),
                _ => None,
            }).collect()
        };
        assert!(params("EventName").is_empty(), "no scene plays: {:?}", params("EventName"));
        for s in MEET_SCENES {
            assert!(params("GameDataName").contains(&s.to_string()), "{s}");
        }
        assert!(params("StepName").contains(&"Complete".to_string()) && params("StepName").contains(&"Step01".to_string()));
        assert_eq!(params("BootEventName"), vec![SEND_OFF_BOOT.to_string()]);
        assert_eq!(params("NextStartPos"), vec![SEND_OFF_POS.to_string()]);
        let gets: usize = seen.iter().filter(|&&i| f.events[i].name.starts_with(MEET_PREFIX) && matches!(&f.events[i].kind, Kind::Sub { .. })).count();
        assert!(gets >= 2, "two item gets, found {gets}");
        // The send off's ride is cut to its played flag.
        let mut d: Bf = load(&mut rf, STONES);
        order(&mut d, STONES).unwrap();
        let s: &Flow = d.flows.iter().find(|f| f.name == STONES).unwrap();
        assert!(!s.events.iter().any(|e| e.kind.params().and_then(|c| c.get("EventName")) == Some(&Val::Str(SEND_OFF_SCENE.1.to_string()))));
    }

    #[test]
    fn the_meadow_saria_waits_for_the_zelda_meeting() {
        let Some(root) = real() else { return };
        let rf: Romfs = Romfs::open(&root).unwrap();
        for m in MEADOW_MODULES {
            let out: Vec<u8> = meadow_bytes(&rf.read(m).unwrap()).unwrap();
            let mut a = crate::ainb::Ainb::parse(&out).unwrap();
            assert!(meadow(&mut a).is_err(), "{m}: the stones quest is still read");
            assert!(a.find_els("HoldGameDataGetBoolExpSimple").into_iter().any(|i| a.in_str(i, "GameDataNameBoolExp") == Some(MEADOW_TO)), "{m}");
        }
    }

    #[test]
    fn every_new_game_skips_the_kokiri_intro_and_the_field_owl_and_runs_the_clock() {
        let o = new_game(true);
        for n in KOKIRI_INTRO.iter().chain(CLOCK.iter()).chain(crate::start_state::ZELDA_TALK.iter()) {
            assert!(o.flags.contains(&(n.to_string(), true)), "{n}");
        }
        assert!(o.flags.contains(&("HavePlayedEvent.Dm_HrlField_0010".to_string(), true)));
        assert!(o.flags.contains(&(crate::start_state::FIELD_FIRST_VISIT.to_string(), false)));
        assert_eq!(o.items, vec![OWL_ITEM.to_string()]);
        assert!(o.bgm && o.time == Some(OWL_TIME) && o.steps.len() == OWL_STEPS.len());
        assert!(new_game(false).items.is_empty() && new_game(false).steps.is_empty() && new_game(false).bgm);
        let p = crate::slice::support::plan(&crate::settings::Settings::default()).unwrap();
        assert!(p.open.time, "the clock is always on");
        let mut s = crate::settings::Settings::default();
        s.set(super::super::TIME, crate::settings::Value::Bool(false)).unwrap();
        assert!(crate::slice::support::plan(&s).unwrap().open.time, "even when a string says off");
    }

    #[test]
    fn malon_gives_once_by_her_own_mark() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let mut b: Bf = load(&mut rf, CASTLE_FLOW);
        order(&mut b, CASTLE_FLOW).unwrap();
        malon_gift(&mut b).unwrap();
        let f: &Flow = b.flows.iter().find(|f| f.name == CASTLE_FLOW).unwrap();
        let ask = named(f, MALON_ASK);
        assert_eq!(ask.kind.params().and_then(|c| c.get("GameDataName")), Some(&Val::Str(MALON_DONE.to_string())));
        assert_eq!(ask.kind.params().and_then(|c| c.get("Index")), Some(&Val::Int(-1)));
        // The mark is written on the gift's way out, after the Step03 request.
        let mark = f.events.iter().position(|e| e.kind.params().and_then(|c| c.get("GameDataName")) == Some(&Val::Str(MALON_DONE.to_string())) && matches!(e.kind, Kind::Action { .. })).unwrap();
        assert!(f.events.iter().any(|e| matches!(e.kind, Kind::Action { next, .. } if next as usize == mark)));
        Bf::parse(&b.write().unwrap()).unwrap();
    }

    #[test]
    fn a_shuffled_egg_hatches_from_the_egg_and_talon_waits_for_malon() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let mut b: Bf = load(&mut rf, CASTLE_FLOW);
        order(&mut b, CASTLE_FLOW).unwrap();
        malon_gift(&mut b).unwrap();
        egg_flow(&mut b).unwrap();
        let f: &Flow = b.flows.iter().find(|f| f.name == CASTLE_FLOW).unwrap();
        let step: usize = f.events.iter().position(|e| e.name == MALON_STEP).unwrap();
        let reach = |g: &crate::bfevfl::Event| match &g.kind {
            Kind::Action { next, .. } | Kind::Sub { next, .. } | Kind::Join { next } => *next as usize == step,
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter().any(|c| c.ev as usize == step),
            Kind::Fork { forks, .. } => forks.iter().any(|x| *x as usize == step),
            Kind::Label { .. } => false,
        };
        assert!(!f.events.iter().any(reach), "Malon's gift still starts the hatch");
        let w = f.entries.iter().find(|e| e.name == WAKE_ENTRY).unwrap();
        let top = &f.events[w.main as usize];
        assert_eq!(top.kind.params().and_then(|c| c.get("GameDataName")), Some(&Val::Str(MALON_DONE.to_string())));
        let Kind::Switch { cases, .. } = &top.kind else { panic!() };
        let no = &f.events[cases.iter().find(|c| c.val == 1).unwrap().ev as usize];
        assert!(matches!(&no.kind, Kind::Sub { flow, entry, .. } if flow == SHOW_FLOW && entry == SHOW_NOBODY));
        Bf::parse(&b.write().unwrap()).unwrap();
        // Getting the egg anywhere asks for Step03 (the hatch timer), only while it is held and the
        // quest has not passed Step02_01.
        for (flow, i, name) in crate::starter_bags::ADDS {
            let mut g: Bf = load(&mut rf, flow);
            egg_get(&mut g, flow, i, name).unwrap();
            assert!(egg_get(&mut g, flow, i, name).is_err(), "{flow}: a second hook is refused");
            let back: Bf = Bf::parse(&g.write().unwrap()).unwrap();
            let f: &Flow = back.flows.iter().find(|f| f.name == flow).unwrap();
            let Kind::Action { next, .. } = f.events[i].kind else { panic!() };
            let held = &f.events[next as usize];
            assert_eq!(held.kind.params().and_then(|c| c.get("GameDataName")), Some(&Val::Str(EGG_COUNT.to_string())), "{flow}");
            assert!(f.events.iter().any(|e| e.name.starts_with(EGG_GET_PREFIX) && e.kind.params().and_then(|c| c.get("StepName")) == Some(&Val::Str(HATCH_STEP.to_string()))));
        }
    }
}
