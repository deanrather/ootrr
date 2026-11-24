//! The adult trade quest runs on its items alone: each hand in takes the previous item and gives
//! the next, whatever the rest of the world is doing, and nothing in the chain is timed.
//!
//! The remake starts each trade from the item shown to the NPC (ShowItemReplaceEventParam), so the
//! chain itself is item driven. What held it back (checked in the dump, 2026-11-21):
//! - Anju only takes the Pocket Cucco once Talon has been woken in Kakariko with it
//!   ([`CROWED`]) and the quest is past Step01_01. Talon only asks for Step02 (and leaves for the
//!   ranch) once Epona is owned (Get_Epona_IsCompleted_Exp), so without the ranch Anju never gave
//!   Cojiro. Both of Anju's checks ([`ANJU_SWITCHES`]) now go straight to her hand over. Talon still
//!   wakes and still asks for his steps, but only while the quest is not past Step02
//!   ([`TALON_GUARD`]): the game moves a quest back when asked for an earlier step, and a Talon
//!   woken first and talked to after the hand over would otherwise pull the quest back to Step02.
//! - The carpenter boss ignores the Poacher's Saw while [`UNREACT`] is true, its new game value,
//!   which only his Spirit quest introduction (SageOfSpirit Ready, Npc_Hylia_012_SageOfSpirit_Ready)
//!   clears. A new game now starts with it false.
//! - The Odd Mushroom (4:00), the Eyeball Frog (2:30) and the Eye Drops (3:30) start a countdown
//!   ([`TIMERS`] entries of the quest flow); its end, a warp, sleeping, the era change or a scene
//!   change with the timer up (all gated on [`COUNTING`]) take the item away and move the quest back.
//!   Each timer entry now ends after its first short wait (no setting, no start, no timer UI), and
//!   [`COUNTING`] is never true, so no warning or rollback runs.
//! - Biggoron makes the sword only 72 hours after the Claim Check ([`WAIT`]); his three checks of
//!   that wait ([`WAIT_SWITCHES`]) now always take the finished branch.
//!
//! Nothing else in the chain reads a world quest: Grog and Fado follow the trade steps, Granny,
//! the carpenter boss, Biggoron and the lake scientist have no presence flag, and King Zora's talk
//! takes the Prescription before and after the Water Temple (he needs thawing, an item rule the
//! logic already has). The egg still hatches on the game's own clock.

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Flow, Kind};
use crate::error::{Error, Res};
use crate::romfs::Romfs;
use crate::staging::Out;
use crate::start_defaults::Edit;

use super::guard::{self, Run};

pub const QUEST: &str = "SubEvent_GetSwordBigGoron";

pub const ANJU: &str = "Npc_Hylia_049_Adult";
pub const CROWED: &str = "KakarikoVillage_CrowedCucco_Npc_Hylia_Talon_Adult";
pub const TALON_WOKEN: &str = "SubEvent_GetSwordBigGoron_IsAfter_Step01_01_Exp";
/// Anju's Pocket Cucco checks, each pinned to the case that hands Cojiro over (case 0 = true).
pub const ANJU_SWITCHES: [(&str, &str); 2] = [("Event59", CROWED), ("Event53", TALON_WOKEN)];
/// Her hand over (sub :SubEvent_GetSwordBigGoron_UntilStep03_ShowItemTalk).
pub const ANJU_GIVE: &str = "Event57";

pub const TALON: &str = "Npc_Hylia_Talon_Adult_Kakariko";
pub const TALON_STEPS: [&str; 3] = ["Step01", "Step01_01", "Step02"];
pub const TALON_AFTER: &str = "SubEvent_GetSwordBigGoron_IsAfter_Step02_Exp";
pub const TALON_GUARD: &str = "OOTRRTradeTalon";

pub const UNREACT: &str = "GetSwordBigGoron_Oyakata_Adult_UnreactCojiro";

/// The countdown entries of the quest flow and the wait each one now ends on.
pub const TIMERS: [(&str, &str); 3] = [("BigGoronTimer_Step04", "Event89"), ("BigGoronTimer_Step09", "Event90"), ("BigGoronTimer_Step10", "Event91")];
pub const TIMER_SET: &str = "EventTriggerMiniGameTimerSetting";
pub const COUNTING: &str = "SubEvent_GetSwordBigGoron_TimerCountingStep";

pub const BIGGORON: &str = "Npc_Goron_Biggoron_Adult";
pub const WAIT: &str = "GetSwordBigGoron_ElapsedTime";
/// The 72 hour checks; case 0 = the wait is over.
pub const WAIT_SWITCHES: [&str; 3] = ["Event71", "Event115", "Event116"];

/// Every flow this file edits.
pub const FLOWS: [&str; 4] = [QUEST, ANJU, TALON, BIGGORON];

/// Trade items never revert (no timers, no rollbacks), so Biggoron needs no fast way up.
pub const LOGIC: [(&str, &str); 1] = [("disable_trade_revert", "true")];

pub const NOTE: &str = "The adult trade quest needs only its items: Anju takes the Pocket Cucco without waking Talon or freeing Lon Lon Ranch, the carpenter boss takes the Poacher's Saw before the Spirit quest starts, the Odd Mushroom, Eyeball Frog and Eye Drops have no timer and never spoil, and Biggoron makes his sword as soon as he sees the Claim Check.";

const AND_OP: i32 = 3;
const NOT_OP: i32 = 5;

/// New game values: the carpenter boss reacts to the saw, and no trade step counts as timed.
pub fn edits() -> Vec<Edit> {
    let x: String = crate::always::ANY_FLAG.to_string();
    vec![
        Edit::Bool(UNREACT.to_string(), false),
        Edit::Exp(COUNTING.to_string(), vec![(0, x.clone()), (0, x), (NOT_OP, String::new()), (AND_OP, String::new())]),
    ]
}

fn event_index(f: &Flow, name: &str) -> Res<u16> {
    let i: usize = f.events.iter().position(|e| e.name == name).ok_or_else(|| Error::Msg(format!("trade: no event {name} in {}", f.name)))?;
    guard::idx(i)
}

/// Anju hands Cojiro over for the Pocket Cucco without Talon.
pub fn anju(b: &mut Bf) -> Res<()> {
    for (name, flag) in ANJU_SWITCHES {
        guard::pin(b, ANJU, &[name], flag, 0)?;
    }
    let f: &Flow = guard::flow_mut(b, ANJU)?;
    let give: u16 = event_index(f, ANJU_GIVE)?;
    if !matches!(&f.events[give as usize].kind, Kind::Sub { entry, .. } if entry == "SubEvent_GetSwordBigGoron_UntilStep03_ShowItemTalk") {
        return Err(Error::Msg(format!("trade: {ANJU_GIVE} in {ANJU} is not Anju's Cojiro hand over")));
    }
    let woken: u16 = event_index(f, ANJU_SWITCHES[1].0)?;
    let crowed: u16 = event_index(f, ANJU_SWITCHES[0].0)?;
    let to = |i: u16| -> Vec<u16> {
        match &f.events[i as usize].kind {
            Kind::Switch { cases, .. } => cases.iter().map(|c| c.ev).collect(),
            _ => Vec::new(),
        }
    };
    if to(crowed) != vec![woken, woken] || to(woken) != vec![give, give] {
        return Err(Error::Msg(format!("trade: Anju's Pocket Cucco checks in {ANJU} do not lead to her hand over")));
    }
    Ok(())
}

/// Talon's step requests run only while the quest is not past Step02.
pub fn talon(b: &mut Bf) -> Res<()> {
    let g: Vec<String> = guard::guard_steps(b, TALON, QUEST, &TALON_STEPS, TALON_AFTER, Run::WhenFalse, TALON_GUARD)?;
    if g.len() != TALON_STEPS.len() {
        return Err(Error::Msg(format!("trade: expected {} step requests in {TALON}, guarded {}", TALON_STEPS.len(), g.len())));
    }
    Ok(())
}

/// The three countdown entries end on their first wait.
pub fn timers(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = guard::flow_mut(b, QUEST)?;
    for (entry, wait) in TIMERS {
        let w: u16 = event_index(f, wait)?;
        let e: usize = f.entries.iter().position(|e| e.name == entry).ok_or_else(|| Error::Msg(format!("trade: no entry {entry} in {QUEST}")))?;
        let set: u16 = f.entries[e].main;
        let Some(Kind::Action { next, actor, act, .. }) = f.events.get(set as usize).map(|e| &e.kind) else {
            return Err(Error::Msg(format!("trade: {entry} in {QUEST} does not start on an action")));
        };
        let name: Option<&str> = f.actors.get(*actor as usize).and_then(|a| a.actions.get(*act as usize)).map(|s| s.as_str());
        if name != Some(TIMER_SET) || *next != w {
            return Err(Error::Msg(format!("trade: {entry} in {QUEST} does not set its timer and then wait on {wait}")));
        }
        let Kind::Action { next, .. } = &mut f.events[w as usize].kind else {
            return Err(Error::Msg(format!("trade: {wait} in {QUEST} is not an action")));
        };
        *next = NONE;
        f.entries[e].main = w;
    }
    Ok(())
}

/// Biggoron's sword is ready as soon as the Claim Check is shown.
pub fn sword(b: &mut Bf) -> Res<()> {
    let n: usize = guard::pin(b, BIGGORON, &WAIT_SWITCHES, WAIT, 0)?;
    if n != WAIT_SWITCHES.len() {
        return Err(Error::Msg(format!("trade: pinned {n} of {} sword waits in {BIGGORON}", WAIT_SWITCHES.len())));
    }
    Ok(())
}

pub fn edit(flow: &str, b: &mut Bf) -> Res<()> {
    match flow {
        QUEST => timers(b),
        ANJU => anju(b),
        TALON => talon(b),
        BIGGORON => sword(b),
        _ => Err(Error::Msg(format!("trade: {flow} is not a trade flow"))),
    }
}

pub fn stage_late(rf: &mut Romfs, late: &mut Vec<Out>, work: &std::path::Path) -> Res<()> {
    for flow in FLOWS {
        crate::story_skip::stage_late(rf, late, work, flow, |b| edit(flow, b))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_extra::sim::{granted, run, sim, Scene};
    use crate::flow_sim::run::Fin;
    use std::collections::HashMap as HM;
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

    fn load(rf: &mut Romfs, flow: &str) -> Bf {
        Bf::parse(&rf.read_zs(&format!("Event/EventFlow/{flow}.bfevfl.zs")).unwrap()).unwrap()
    }

    fn edited(rf: &mut Romfs, flow: &str) -> Bf {
        let mut b: Bf = load(rf, flow);
        edit(flow, &mut b).unwrap();
        let back: Bf = Bf::parse(&b.write().unwrap()).unwrap();
        assert_eq!(back, b, "{flow}");
        back
    }

    fn cases(f: &Flow, name: &str) -> Vec<(u32, String)> {
        let e = f.events.iter().find(|e| e.name == name).unwrap();
        match &e.kind {
            Kind::Switch { cases, .. } => cases.iter().map(|c| (c.val, f.events[c.ev as usize].name.clone())).collect(),
            _ => panic!("{name} is not a switch"),
        }
    }

    #[test]
    fn the_new_game_values_name_real_flags_and_the_timer_formula_is_never_true() {
        let e: Vec<Edit> = edits();
        assert_eq!(e[0], Edit::Bool(UNREACT.to_string(), false));
        let Edit::Exp(n, t) = &e[1] else { panic!() };
        assert_eq!(n, COUNTING);
        assert_eq!(t[0], t[1]);
        assert_eq!(t[2].0, NOT_OP);
        assert_eq!(t[3].0, AND_OP);
        let Some(r) = real() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let mut more: Vec<Out> = Vec::new();
        crate::start_defaults::stage(&mut rf, &mut more, &e).unwrap();
        assert_eq!(more.len(), 1);
    }

    #[test]
    fn every_edit_lands_and_a_second_run_is_refused() {
        let Some(r) = real() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();

        let a: Bf = edited(&mut rf, ANJU);
        let f: &Flow = &a.flows[0];
        assert_eq!(cases(f, "Event59"), vec![(0, "Event53".to_string()), (1, "Event53".to_string())]);
        assert_eq!(cases(f, "Event53"), vec![(0, ANJU_GIVE.to_string()), (1, ANJU_GIVE.to_string())]);

        let t: Bf = edited(&mut rf, TALON);
        let f: &Flow = &t.flows[0];
        assert_eq!(f.events.iter().filter(|e| e.name.starts_with(TALON_GUARD)).count(), 3);
        let mut again: Bf = t.clone();
        assert!(talon(&mut again).is_err());

        let q: Bf = edited(&mut rf, QUEST);
        let f: &Flow = &q.flows[0];
        for (entry, wait) in TIMERS {
            let e = f.entries.iter().find(|e| e.name == entry).unwrap();
            assert_eq!(f.events[e.main as usize].name, wait);
            assert!(matches!(f.events[e.main as usize].kind, Kind::Action { next: NONE, .. }));
        }
        let mut again: Bf = q.clone();
        assert!(timers(&mut again).is_err());

        let g: Bf = edited(&mut rf, BIGGORON);
        let f: &Flow = &g.flows[0];
        for w in WAIT_SWITCHES {
            let c: Vec<(u32, String)> = cases(f, w);
            assert!(c.len() == 2 && c[0].1 == c[1].1, "{w} {c:?}");
        }
    }

    #[test]
    fn anju_trades_the_pocket_cucco_for_cojiro_with_talon_asleep_and_no_epona() {
        let Some(r) = real() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let sc: Scene = Scene {
            flow: ANJU.to_string(),
            entry: "Npc_Hylia_049_Adult_ShowPocketCucco".to_string(),
            set: vec![("Step_SubEvent_GetSwordBigGoron".to_string(), "EggHatched".to_string())],
            own: vec![("Item_PocketCucco".to_string(), 1)],
        };
        let mut vanilla = sim(&r, HM::new()).unwrap();
        let before: Vec<Fin> = run(&mut vanilla, &sc).unwrap();
        assert!(before.iter().all(|f| granted(f, "Item_CojiroCucco") == 0), "vanilla Anju waits for Talon");
        let mut fl: HM<String, Flow> = HM::new();
        fl.insert(ANJU.to_string(), edited(&mut rf, ANJU).flows.remove(0));
        let mut s = sim(&r, fl).unwrap();
        let after: Vec<Fin> = run(&mut s, &sc).unwrap();
        assert!(after.iter().any(|f| granted(f, "Item_CojiroCucco") == 1 && granted(f, "Item_PocketCucco") == -1), "Anju hands Cojiro over");
        assert!(after.iter().filter(|f| granted(f, "Item_CojiroCucco") == 1).all(|f| f.m.reqs.iter().any(|q| q.starts_with("SubEvent_GetSwordBigGoron -> Step03"))));
    }
}
