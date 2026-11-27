//! Open Forest Kokiri: the village keeps one story state of its own, whatever the castle story or
//! the later quests do.
//!
//! Every child Kokiri but Mido and Saria behaves as just after the Deku Tree (Gohma beaten), from a
//! new game on. Their schedules (Component/AIScheduleParam, the last matching condition wins) take
//! the first post-Deku condition as their default and drop the pre-Deku ones before it and the
//! Princess Zelda quest's; their talk flows read the Forest Stone quest as done and the Princess
//! Zelda quest (and the Fire Stone's Saria's Song step) as not reached.
//!
//! Mido keeps his "bring a sword" talk until Gohma is beaten, so his Kokiri Sword hint can be
//! heard: his schedule loses the two Deku Tree quest conditions (he would stand aside and talk his
//! post-Mido lines) and the Princess Zelda one, while his own default and his blocker-trio
//! condition take the standing-aside place of the dropped Deku Tree condition, so he never blocks
//! the path. His talk no longer requests Deku Tree quest steps (the open forest completes that
//! quest, a request would move it back) and no longer runs the blocker actions: his blocker is off
//! from a new game, and the blocker talk opening waits for a blocker action to end.
//!
//! Saria keeps her schedule: it reads no later quest, and her bridge gift does not depend on her.
//! Leaving the forest is untouched: Kokiri 007's exit talk reads the exit wall flag, which the open
//! forest clears.

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::romfs::Romfs;
use crate::staging::Out;

use super::guard;

pub const NOTE: &str = "The Kokiri act as just after the Deku Tree from a new game on, whatever the Zelda meeting or later quests do. Mido keeps his talk from before the Deku Tree (and its hint) until Gohma is beaten, but stands aside.";

/// Child Kokiri that take the post-Deku state.
pub const NPCS: [&str; 14] = [
    "Npc_Kokiri_001_Shop_Child",
    "Npc_Kokiri_002_Child",
    "Npc_Kokiri_003_Child",
    "Npc_Kokiri_004_Child",
    "Npc_Kokiri_005_Child",
    "Npc_Kokiri_006_Child",
    "Npc_Kokiri_007_Child",
    "Npc_Kokiri_008_Child",
    "Npc_Kokiri_009_Child",
    "Npc_Kokiri_010_Child",
    "Npc_Kokiri_011_Child",
    "Npc_Kokiri_012_Child",
    "Npc_Kokiri_013_Child",
    "Npc_Kokiri_Fado_Child",
];
pub const MIDO: &str = "Npc_Kokiri_Mido_Child";

/// Schedule conditions that mean "the Deku Tree is done" (Gohma beaten, or the Forest Stone quest
/// complete).
pub const DONE: [&str; 2] = ["Get_StoneOfForest_IsAfter_DefeatBoss_Exp", "Get_StoneOfForest_IsCompleted_Exp"];
/// Schedule conditions of the castle story, which the open forest ignores.
pub const LATER: &str = "Goto_HyruleCastle_IsCompleted_Exp";
/// Mido's two Deku Tree quest conditions (both stand him aside with his post-Mido talk).
pub const ASIDE: &str = "Meet_DekuTree_IsAfter_Step02_Exp";
pub const ASIDE_DONE: &str = "Meet_DekuTree_IsCompleted_Exp";
/// Mido's blocker-trio condition (set by the areas near the Deku Tree path).
pub const TRIO: &str = "KokiriForest_Npc_Kokiri_Blocker";

/// Talk flow switches pinned to one case: 0 = true, 1 = false.
pub const PINS: [(&str, u32); 4] = [
    ("Get_StoneOfForest_IsAfter_DefeatBoss_Exp", 0),
    ("Get_StoneOfForest_IsCompleted_Exp", 0),
    ("Goto_HyruleCastle_IsCompleted_Exp", 1),
    ("Get_StoneOfFire_IsAfter_SariaMelodyPlay_Exp", 1),
];
const BOOL_QUERIES: [&str; 2] = ["EventQueryIsGameDataBoolExp", "EventQueryIsGameDataBool"];

const CONDS: &str = "ConditionalSchedule";
const DEFAULT: &str = "DefaultSchedule";
const FLAG: &str = "ConditionFlag";
const REVERSE: &str = "IsReverseCondition";
const ACTION: &str = "ActionSchedule";

const BLOCKER_INIT: &str = "InitTalk_Blocker";
const PLAIN_INIT: &str = "InitTalk_KokiriOnly";
const INIT_FLOW: &str = "InitTalk";
const QUEST: &str = "Meet_DekuTree";

pub fn schedule_path(npc: &str) -> String {
    crate::gerudo_card::schedule_path(npc)
}

pub fn flow_path(npc: &str) -> String {
    format!("Event/EventFlow/{npc}.bfevfl.zs")
}

fn flag_of(n: &Node) -> Option<&str> {
    n.get(FLAG).and_then(|f| f.as_str())
}

fn reversed(n: &Node) -> bool {
    n.get(REVERSE) == Some(&Node::Bool(true))
}

fn conds(y: &Byml, who: &str) -> Res<Vec<Node>> {
    y.root.get(CONDS).and_then(|c| c.as_arr()).map(|c| c.to_vec()).ok_or_else(|| Error::Msg(format!("kokiri: {who}'s schedule has no conditions, the game files differ")))
}

/// A Kokiri schedule in the post-Deku state. None when the schedule reads neither the Deku Tree
/// being done nor the castle story (nothing to change).
pub fn post_deku(raw: &[u8], who: &str) -> Res<Option<Vec<u8>>> {
    let mut y: Byml = Byml::parse(raw)?;
    // Kokiri 002 has no condition list at all.
    if y.root.get(CONDS).is_none() {
        return Ok(None);
    }
    let mut list: Vec<Node> = conds(&y, who)?;
    let before: usize = list.len();
    let mut changed: bool = false;
    if let Some(p) = list.iter().position(|n| !reversed(n) && flag_of(n).is_some_and(|f| DONE.contains(&f))) {
        let mut d: Node = list[p].clone();
        d.remove(FLAG);
        d.remove(REVERSE);
        y.root.set(DEFAULT, d);
        list.drain(..=p);
        changed = true;
    }
    list.retain(|n| flag_of(n) != Some(LATER));
    if !changed && list.len() == before {
        return Ok(None);
    }
    y.root.set(CONDS, Node::Arr(list));
    Ok(Some(y.write()?))
}

/// Mido's schedule: his own default and his blocker-trio condition (both with his pre-Deku talk)
/// stand him where the Deku Tree quest stood him aside; the two quest conditions and the castle
/// story one are gone, so he keeps that talk until Gohma is beaten.
pub fn mido_schedule(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    let list: Vec<Node> = conds(&y, MIDO)?;
    let one = |flag: &str| -> Res<usize> {
        let hits: Vec<usize> = (0..list.len()).filter(|&i| flag_of(&list[i]) == Some(flag) && !reversed(&list[i])).collect();
        match hits.as_slice() {
            [i] => Ok(*i),
            _ => Err(Error::Msg(format!("kokiri: Mido's schedule reads {flag} {} times, the game files differ", hits.len()))),
        }
    };
    let aside: Node = list[one(ASIDE)?].get(ACTION).cloned().ok_or(Error::Format("kokiri: Mido's standing-aside condition has no action"))?;
    let trio: usize = one(TRIO)?;
    for f in [ASIDE_DONE, LATER] {
        one(f)?;
    }
    let mut out: Vec<Node> = Vec::new();
    for (i, n) in list.iter().enumerate() {
        match flag_of(n) {
            Some(ASIDE) | Some(ASIDE_DONE) | Some(LATER) => {}
            _ if i == trio => {
                let mut t: Node = n.clone();
                t.set(ACTION, aside.clone());
                out.push(t);
            }
            _ => out.push(n.clone()),
        }
    }
    let d: &mut Node = y.root.get_mut(DEFAULT).ok_or(Error::Format("kokiri: Mido's schedule has no default"))?;
    d.set(ACTION, aside);
    y.root.set(CONDS, Node::Arr(out));
    y.write()
}

/// Points every bool switch on a [`PINS`] flag at its pinned case. Returns how many switches were
/// pinned (switches already pinned count too).
pub fn pin_flow(b: &mut Bf, flow: &str) -> Res<usize> {
    let f: &mut Flow = guard::flow_mut(b, flow)?;
    let bools: Vec<Vec<bool>> = f.actors.iter().map(|a| a.queries.iter().map(|q| BOOL_QUERIES.contains(&q.as_str())).collect()).collect();
    let mut n: usize = 0;
    for i in 0..f.events.len() {
        let Kind::Switch { actor, query, cases, params } = &mut f.events[i].kind else { continue };
        let Some(Val::Str(name)) = params.as_ref().and_then(|p| p.get("GameDataName")) else { continue };
        let Some(&(_, val)) = PINS.iter().find(|(p, _)| p == name) else { continue };
        if !bools.get(*actor as usize).and_then(|a| a.get(*query as usize)).copied().unwrap_or(false) {
            return Err(Error::Msg(format!("kokiri: {flow} event {i} reads {name} with a query that is not a bool")));
        }
        match cases.iter().find(|c| c.val == val).map(|c| c.ev) {
            Some(to) => {
                for c in cases.iter_mut() {
                    c.ev = to;
                }
            }
            // A missing case ends the flow (Kokiri 010's near balloon has no post-Deku line), so
            // the pinned switch keeps no case at all.
            None => cases.clear(),
        }
        n += 1;
    }
    Ok(n)
}

fn action_name<'a>(f: &'a Flow, k: &Kind) -> Option<&'a str> {
    let Kind::Action { actor, act, .. } = k else { return None };
    f.actors.get(*actor as usize).and_then(|a| a.actions.get(*act as usize)).map(|s| s.as_str())
}

/// Sends everything that led to event `i` (an action) on to that action's next event.
fn bypass(f: &mut Flow, i: usize) -> Res<()> {
    let Kind::Action { next, .. } = f.events[i].kind else {
        return Err(Error::Msg(format!("kokiri: {} event {i} is not an action", f.name)));
    };
    let from: u16 = guard::idx(i)?;
    if next == NONE {
        // Only an action, a join or a sub may end the flow in place of the bypassed event.
        let bad: bool = f.events.iter().any(|e| match &e.kind {
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter().any(|c| c.ev == from),
            Kind::Fork { join, forks } => *join == from || forks.contains(&from),
            _ => false,
        }) || f.entries.iter().any(|e| e.main == from || e.subs.contains(&from));
        if bad {
            return Err(Error::Msg(format!("kokiri: {} event {i} ends the flow but a switch, fork or entry leads to it", f.name)));
        }
    }
    guard::retarget(f, from, next);
    Ok(())
}

/// Mido's talk for the open forest: no Deku Tree quest step requests, no blocker actions, and the
/// plain Kokiri talk opening in place of the blocker one. Returns (step requests, blocker actions,
/// openings) changed.
pub fn mido_flow(b: &mut Bf) -> Res<(usize, usize, usize)> {
    let f: &mut Flow = guard::flow_mut(b, MIDO)?;
    let plain: Kind = f
        .events
        .iter()
        .find(|e| matches!(&e.kind, Kind::Sub { flow, entry, .. } if flow == INIT_FLOW && entry == PLAIN_INIT))
        .map(|e| e.kind.clone())
        .ok_or_else(|| Error::Msg(format!("kokiri: {MIDO} has no {INIT_FLOW}:{PLAIN_INIT} call to copy, the game files differ")))?;
    let mut opens: usize = 0;
    for e in f.events.iter_mut() {
        let Kind::Sub { next, flow, entry, .. } = &e.kind else { continue };
        if flow != INIT_FLOW || entry != BLOCKER_INIT {
            continue;
        }
        let keep: u16 = *next;
        let mut k: Kind = plain.clone();
        if let Kind::Sub { next, .. } = &mut k {
            *next = keep;
        }
        e.kind = k;
        opens += 1;
    }
    let steps: Vec<usize> = (0..f.events.len()).filter(|&i| guard::step_of(f, &f.events[i], QUEST).is_some()).collect();
    let blockers: Vec<usize> = (0..f.events.len())
        .filter(|&i| action_name(f, &f.events[i].kind).is_some_and(|a| a.contains("Blocker")) && matches!(&f.events[i].kind, Kind::Action { actor, .. } if f.actors.get(*actor as usize).is_some_and(|a| a.name == MIDO)))
        .collect();
    if steps.is_empty() || blockers.is_empty() || opens == 0 {
        return Err(Error::Msg(format!("kokiri: {MIDO} has {} step requests, {} blocker actions and {opens} blocker openings, the game files differ or the edit already ran", steps.len(), blockers.len())));
    }
    for &i in blockers.iter().chain(steps.iter()) {
        bypass(f, i)?;
    }
    Ok((steps.len(), blockers.len(), opens))
}

fn stage_plain(rf: &mut Romfs, late: &mut Vec<Out>, work: &std::path::Path, path: &str, edit: impl FnOnce(&[u8]) -> Res<Vec<u8>>) -> Res<()> {
    if let Some(o) = late.iter_mut().find(|o| o.path == path) {
        let b: Vec<u8> = edit(&o.z)?;
        o.new = b.len();
        o.z = b;
        return Ok(());
    }
    let have = |p: &str| -> Option<Vec<u8>> { std::fs::read(p.split('/').fold(work.to_path_buf(), |a, s| a.join(s))).ok() };
    late.push(crate::qol::edit_plain(rf, &have, path, edit)?);
    Ok(())
}

/// The flows that read a pinned flag (the others need no edit).
pub fn pinned_flows(rf: &mut Romfs) -> Res<Vec<&'static str>> {
    let mut out: Vec<&'static str> = Vec::new();
    for npc in NPCS {
        let mut b: Bf = Bf::parse(&rf.read_zs(&flow_path(npc))?)?;
        if pin_flow(&mut b, npc)? > 0 {
            out.push(npc);
        }
    }
    Ok(out)
}

/// Stages the open forest Kokiri as late files, on top of what earlier stages wrote.
pub fn stage_late(rf: &mut Romfs, late: &mut Vec<Out>, work: &std::path::Path) -> Res<()> {
    for npc in NPCS {
        let raw: Vec<u8> = rf.read(&schedule_path(npc))?;
        if post_deku(&raw, npc)?.is_some() {
            stage_plain(rf, late, work, &schedule_path(npc), |r| post_deku(r, npc)?.ok_or_else(|| Error::Msg(format!("kokiri: {npc}'s schedule has nothing left to change"))))?;
        }
    }
    stage_plain(rf, late, work, &schedule_path(MIDO), mido_schedule)?;
    for npc in pinned_flows(rf)? {
        crate::story_skip::stage_late(rf, late, work, npc, |b| pin_flow(b, npc).map(|_| ()))?;
    }
    crate::story_skip::stage_late(rf, late, work, MIDO, |b| mido_flow(b).map(|_| ()))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn flags(y: &Byml) -> Vec<String> {
        y.root.get(CONDS).and_then(|c| c.as_arr()).unwrap_or(&[]).iter().map(|n| flag_of(n).unwrap_or("").to_string()).collect()
    }

    fn events(n: &Node) -> Vec<String> {
        n.get("EventEntry").and_then(|e| e.as_arr()).map(|a| a.iter().filter_map(|e| e.get("EventName").and_then(|s| s.as_str()).map(str::to_string)).collect()).unwrap_or_default()
    }

    fn reached(f: &Flow) -> Vec<bool> {
        let mut seen: Vec<bool> = vec![false; f.events.len()];
        let mut todo: Vec<u16> = f.entries.iter().flat_map(|e| std::iter::once(e.main).chain(e.subs.iter().copied())).collect();
        while let Some(i) = todo.pop() {
            if i == NONE || seen[i as usize] {
                continue;
            }
            seen[i as usize] = true;
            match &f.events[i as usize].kind {
                Kind::Action { next, .. } | Kind::Join { next } | Kind::Sub { next, .. } => todo.push(*next),
                Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
                Kind::Fork { join, forks } => {
                    todo.push(*join);
                    todo.extend(forks.iter().copied());
                }
                Kind::Label { .. } => {}
            }
        }
        seen
    }

    #[test]
    fn every_kokiri_schedule_starts_post_deku_and_ignores_the_castle_story() {
        let Some(root) = real() else { return };
        let rf: Romfs = Romfs::open(&root).unwrap();
        let mut changed: usize = 0;
        for npc in NPCS {
            let raw: Vec<u8> = rf.read(&schedule_path(npc)).unwrap();
            let old: Byml = Byml::parse(&raw).unwrap();
            let Some(out) = post_deku(&raw, npc).unwrap() else {
                assert!(!flags(&old).iter().any(|f| DONE.contains(&f.as_str()) || f == LATER), "{npc}");
                continue;
            };
            changed += 1;
            assert!(out.len() <= raw.len(), "{npc} grew");
            let y: Byml = Byml::parse(&out).unwrap();
            let now: Vec<String> = flags(&y);
            assert!(!now.iter().any(|f| f == LATER || DONE.contains(&f.as_str())), "{npc}: {now:?}");
            let was: Vec<String> = flags(&old);
            let list: &[Node] = old.root.get(CONDS).unwrap().as_arr().unwrap();
            match was.iter().position(|f| DONE.contains(&f.as_str())) {
                Some(p) => {
                    let d: &Node = y.root.get(DEFAULT).unwrap();
                    assert_eq!(d.get(ACTION), list[p].get(ACTION), "{npc}");
                    assert_eq!(events(d), events(&list[p]), "{npc}");
                    assert!(d.get(FLAG).is_none(), "{npc}");
                    let kept: Vec<String> = was[p + 1..].iter().filter(|f| *f != LATER).cloned().collect();
                    assert_eq!(now, kept, "{npc}");
                }
                None => {
                    assert_eq!(y.root.get(DEFAULT), old.root.get(DEFAULT), "{npc}");
                    assert_eq!(now, was.iter().filter(|f| *f != LATER).cloned().collect::<Vec<_>>(), "{npc}");
                }
            }
            assert!(post_deku(&out, npc).unwrap().is_none(), "{npc} changes twice");
        }
        // Every child Kokiri but the shopkeeper (shop talk only) and 002 (no conditions).
        assert_eq!(changed, 12);
    }

    #[test]
    fn the_exit_kokiri_keeps_reading_the_exit_wall() {
        let Some(root) = real() else { return };
        let rf: Romfs = Romfs::open(&root).unwrap();
        let npc: &str = "Npc_Kokiri_007_Child";
        let y: Byml = Byml::parse(&post_deku(&rf.read(&schedule_path(npc)).unwrap(), npc).unwrap().unwrap()).unwrap();
        let list: &[Node] = y.root.get(CONDS).unwrap().as_arr().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(flag_of(&list[0]), Some(crate::slice::support::EXIT_WALL));
        assert!(reversed(&list[0]));
    }

    #[test]
    fn mido_talks_before_the_deku_tree_until_gohma_and_stands_aside() {
        let Some(root) = real() else { return };
        let rf: Romfs = Romfs::open(&root).unwrap();
        let raw: Vec<u8> = rf.read(&schedule_path(MIDO)).unwrap();
        let old: Byml = Byml::parse(&raw).unwrap();
        let out: Vec<u8> = mido_schedule(&raw).unwrap();
        assert!(out.len() <= raw.len());
        let y: Byml = Byml::parse(&out).unwrap();
        assert_eq!(flags(&y), vec![TRIO, "Get_StoneOfForest_IsAfter_DefeatBoss_Exp", "Get_StoneOfForest_Complete_Mido_TalkEnd"]);
        let list: &[Node] = old.root.get(CONDS).unwrap().as_arr().unwrap();
        let aside: &Node = list.iter().find(|n| flag_of(n) == Some(ASIDE)).unwrap().get(ACTION).unwrap();
        let d: &Node = y.root.get(DEFAULT).unwrap();
        assert_eq!(d.get(ACTION), Some(aside));
        assert_eq!(events(d), events(old.root.get(DEFAULT).unwrap()));
        assert!(events(d).iter().any(|e| e.contains("Npc_Kokiri_Mido_Child_DC0_TT")));
        let t: &Node = &y.root.get(CONDS).unwrap().as_arr().unwrap()[0];
        assert_eq!(t.get(ACTION), Some(aside));
        assert_eq!(events(t), events(&list[0]));
        assert!(mido_schedule(&out).is_err());
    }

    #[test]
    fn mido_still_says_his_hint_lines_and_requests_no_step() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let mut b: Bf = Bf::parse(&rf.read_zs(&flow_path(MIDO)).unwrap()).unwrap();
        let (steps, blockers, opens): (usize, usize, usize) = mido_flow(&mut b).unwrap();
        assert_eq!((steps, blockers, opens), (3, 6, 1));
        let out: Vec<u8> = b.write().unwrap();
        let back: Bf = Bf::parse(&out).unwrap();
        assert_eq!(back, b);
        let f: &Flow = back.flows.iter().find(|f| f.name == MIDO).unwrap();
        let seen: Vec<bool> = reached(f);
        for (i, e) in f.events.iter().enumerate() {
            if !seen[i] {
                continue;
            }
            assert!(guard::step_of(f, e, QUEST).is_none(), "event {i} still requests a step");
            assert!(!action_name(f, &e.kind).is_some_and(|a| a.contains("Blocker")), "event {i} still runs {:?}", action_name(f, &e.kind));
            assert!(!matches!(&e.kind, Kind::Sub { entry, .. } if entry == BLOCKER_INIT), "event {i}");
        }
        let talk: u16 = f.entries.iter().find(|e| e.name == "Talk").unwrap().main;
        let mut from_talk: Vec<bool> = vec![false; f.events.len()];
        let mut todo: Vec<u16> = vec![talk];
        while let Some(i) = todo.pop() {
            if i == NONE || from_talk[i as usize] {
                continue;
            }
            from_talk[i as usize] = true;
            match &f.events[i as usize].kind {
                Kind::Action { next, .. } | Kind::Join { next } | Kind::Sub { next, .. } => todo.push(*next),
                Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
                Kind::Fork { join, forks } => {
                    todo.push(*join);
                    todo.extend(forks.iter().copied());
                }
                Kind::Label { .. } => {}
            }
        }
        let said: Vec<String> = (0..f.events.len())
            .filter(|&i| from_talk[i])
            .filter_map(|i| match f.events[i].kind.params().and_then(|p| p.get("MessageId")) {
                Some(Val::Str(s)) => s.rsplit(':').next().map(str::to_string),
                _ => None,
            })
            .collect();
        for l in crate::npc_hints::MIDO_TALKS {
            assert!(said.iter().any(|s| s == l), "{l} is no longer reached from Mido's talk: {said:?}");
        }
        assert!(mido_flow(&mut b).is_err());
    }

    #[test]
    fn the_talk_flows_read_the_forest_stone_done_and_the_castle_story_not_reached() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let flows: Vec<&str> = pinned_flows(&mut rf).unwrap();
        assert!(flows.len() >= 10, "{flows:?}");
        let mut total: usize = 0;
        for npc in &flows {
            let mut b: Bf = Bf::parse(&rf.read_zs(&flow_path(npc)).unwrap()).unwrap();
            total += pin_flow(&mut b, npc).unwrap();
            assert_eq!(Bf::parse(&b.write().unwrap()).unwrap(), b, "{npc}");
            let f: &Flow = b.flows.iter().find(|f| f.name == *npc).unwrap();
            for e in &f.events {
                let Kind::Switch { cases, params, .. } = &e.kind else { continue };
                let Some(Val::Str(name)) = params.as_ref().and_then(|p| p.get("GameDataName")) else { continue };
                if PINS.iter().any(|(p, _)| p == name) {
                    assert!(cases.windows(2).all(|w| w[0].ev == w[1].ev), "{npc} {}", e.name);
                }
            }
        }
        assert!(total >= 20, "{total}");
    }
}
