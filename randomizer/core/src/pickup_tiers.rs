use std::collections::BTreeSet as BS;
use std::path::Path;

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Case, Cont, Event, Flow, Kind, Val};
use crate::error::{Error as E, Res};
use crate::progressive::{owners, tiers};
use crate::romfs::Romfs;
use crate::settings::seed::SeedFile;
use crate::slice::data::Data;
use crate::staging::Out;

pub const GETS: [(&str, usize, &str); 2] = crate::dungeon_packs::GETS;
pub const NAME: &str = "UI_MessageTargetActorName";
pub const PREFIX: &str = "OOTRR_PickTier";
pub const SKIPPED: [&str; 1] = ["Ocarina"];
const DATA: &str = "GameDataActor";
const ASK: &str = "EventQueryIsGameDataString64";
const SET_TEXT: &str = "EventTriggerSetGameDataString64";
const ARG: &str = "ActorName";
const WAITER: &str = "EventSystemActor";
const WAIT: &str = "EventWait";
pub const SETTLE: i32 = 5;
const OWNS: &str = "EventQueryCheckIsGet";
const POUCH: &str = "PouchSystemActor";
const ADD: &str = "EventTriggerAddOrRemovePouchContent";
const GUEST: &str = "PouchContentGuestPerformer";
const ADD_OWN: &str = "EventTriggerAddOwnToPouch";
const SET_NAME: &str = "EventTriggerSetOwnNameToMessage";
const EXPAND: &str = "EventTriggerGetFirstExpansionMaxNumActor";
const WINDOW: (&str, &str) = ("GetPouchContent", "OpenGetWindow");

pub fn runtime(d: &Data, loc: &str, family: &str) -> bool {
    crate::slice::tier_order::fixed_spot(d, loc) && !crate::shops::is_loc(loc) && loc != crate::slice::treasure::LOC && !SKIPPED.contains(&family)
}

/// Whether a pack of `item` must play its get event every time. The tier hook lives in that
/// event, and the game skips the event once the pack's identity has played it before, so a
/// second upgrade from the ground would be added silently as the tier the player already has.
pub fn replays(item: &str) -> bool {
    crate::progressive::FAMILIES
        .iter()
        .filter(|(f, _)| !SKIPPED.contains(f))
        .any(|(f, t)| t.contains(&item) || owners(f).is_ok_and(|o| o.contains(&item)))
}

pub fn base(family: &str) -> Res<&'static str> {
    let t: &[&str] = tiers(family)?;
    let at: Option<&&str> = if crate::slice::key_spots::TOP_ONLY.contains(&family) { t.last() } else { t.first() };
    at.copied().ok_or_else(|| E::Msg(format!("pickup tiers: {family} has no tier")))
}

pub fn families(d: &Data, seed: &SeedFile) -> BS<&'static str> {
    seed.placements.iter().filter_map(|(loc, item)| d.family(item).filter(|f| runtime(d, loc, f))).collect()
}

pub fn names(family: &str, tier: usize) -> Res<Vec<String>> {
    let t: &str = tiers(family)?.get(tier).copied().ok_or_else(|| E::Msg(format!("pickup tiers: {family} has no tier {tier}")))?;
    let mut out: BS<String> = BS::new();
    out.insert(t.to_string());
    if let Some(o) = owners(family)?.get(tier) {
        out.insert((*o).to_string());
    }
    if let Some(p) = crate::slice::key_spots::pick(t) {
        out.insert(p.kit());
        out.insert(p.hang());
        out.insert(p.pack);
    }
    Ok(out.into_iter().collect())
}

fn idx(n: usize) -> Res<u16> {
    u16::try_from(n).ok().filter(|i| *i != NONE).ok_or(E::Format("pickup tiers: the flow has too many events"))
}

fn push(f: &mut Flow, kind: Kind) -> Res<u16> {
    let i: u16 = idx(f.events.len())?;
    f.events.push(Event { name: format!("{PREFIX}{i}"), kind });
    Ok(i)
}

fn action_is(f: &Flow, i: usize, who: &str, what: &str) -> bool {
    match f.events.get(i).map(|e| &e.kind) {
        Some(Kind::Action { actor, act, .. }) => f.actors.get(usize::from(*actor)).is_some_and(|a| a.name == who && a.actions.get(usize::from(*act)).is_some_and(|x| x == what)),
        _ => false,
    }
}

fn next_of(k: &Kind) -> Option<u16> {
    match k {
        Kind::Action { next, .. } | Kind::Sub { next, .. } => Some(*next),
        _ => None,
    }
}

fn with_next(k: &Kind, to: u16) -> Kind {
    let mut k: Kind = k.clone();
    if let Kind::Action { next, .. } | Kind::Sub { next, .. } = &mut k {
        *next = to;
    }
    k
}

fn targets(k: &Kind) -> Vec<u16> {
    match k {
        Kind::Action { next, .. } | Kind::Sub { next, .. } | Kind::Join { next } => vec![*next],
        Kind::Switch { cases, .. } => cases.iter().map(|c| c.ev).collect(),
        Kind::Fork { join, forks } => forks.iter().copied().chain(std::iter::once(*join)).collect(),
        _ => Vec::new(),
    }
}

fn reach(events: &[Event], from: u16) -> BS<u16> {
    let mut seen: BS<u16> = BS::new();
    let mut todo: Vec<u16> = vec![from];
    while let Some(i) = todo.pop() {
        if i == NONE || usize::from(i) >= events.len() || !seen.insert(i) {
            continue;
        }
        todo.extend(targets(&events[usize::from(i)].kind));
    }
    seen
}

fn pouch(f: &mut Flow, tier: &str, num: i32, named: bool, next: u16) -> Res<u16> {
    let (a, x): (u16, u16) = crate::start_state::slot(f, POUCH, ADD)?;
    let c: Cont = Cont {
        items: vec![
            ("Num".to_string(), Val::Int(num)),
            ("IsEquipDIrect".to_string(), Val::Bool(0)),
            ("IsSetMessageTargetActorName".to_string(), Val::Bool(u32::from(named))),
            ("Actor".to_string(), Val::Str(tier.to_string())),
            ("IsGetEvent".to_string(), Val::Bool(0)),
        ],
    };
    push(f, Kind::Action { next, actor: a, act: x, params: Some(c) })
}

fn settle(f: &mut Flow, next: u16) -> Res<u16> {
    let (wa, wx): (u16, u16) = crate::start_state::slot(f, WAITER, WAIT)?;
    let c: Cont = Cont { items: vec![("IsWaitEnd".to_string(), Val::Bool(1)), ("Frames".to_string(), Val::Int(SETTLE))] };
    push(f, Kind::Action { next, actor: wa, act: wx, params: Some(c) })
}

fn ask(f: &mut Flow, value: &str, hit: u16, miss: u16) -> Res<u16> {
    let (a, q): (u16, u16) = crate::progressive::ensure(f, DATA, ASK)?;
    let c: Cont = Cont { items: vec![("GameDataName".to_string(), Val::Str(NAME.to_string())), ("Index".to_string(), Val::Int(-1)), ("Value".to_string(), Val::Str(value.to_string()))] };
    push(f, Kind::Switch { actor: a, query: q, cases: vec![Case { val: 0, ev: hit }, Case { val: 1, ev: miss }], params: Some(c) })
}

struct Tail {
    window: Kind,
    expand: Option<Kind>,
    own: Kind,
    then: u16,
}

fn path(f: &mut Flow, t: &Tail, family: &str, picked: usize, given: usize) -> Res<u16> {
    let ts: &[&str] = tiers(family)?;
    let mut next: u16 = t.then;
    if picked != given {
        next = pouch(f, ts[given], 1, false, next)?;
    }
    if picked > given {
        next = pouch(f, ts[picked], -1, false, next)?;
    }
    if picked != given {
        next = settle(f, next)?;
    }
    next = push(f, with_next(&t.own, next))?;
    if let Some(x) = &t.expand {
        next = push(f, with_next(x, next))?;
    }
    next = push(f, with_next(&t.window, next))?;
    pouch(f, ts[given], 1, true, next)
}

fn give(f: &mut Flow, t: &Tail, family: &str, picked: usize) -> Res<u16> {
    let mut gives: Vec<u16> = Vec::new();
    for given in 0..tiers(family)?.len() {
        gives.push(path(f, t, family, picked, given)?);
    }
    let (a, q): (u16, u16) = crate::progressive::ensure(f, DATA, OWNS)?;
    let root: Kind = crate::progressive::chain(f, a, q, family, owners(family)?, &gives)?;
    push(f, root)
}

pub fn patch(f: &mut Flow, at: usize, name: &str, fams: &BS<&str>) -> Res<bool> {
    if fams.is_empty() || f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Ok(false);
    }
    if f.events.get(at).map(|e| e.name.as_str()) != Some(name) || !action_is(f, at, GUEST, ADD_OWN) {
        return Err(E::Msg(format!("pickup tiers: event {at} of {} is not the pouch add {name}, the game files differ", f.name)));
    }
    let then: u16 = next_of(&f.events[at].kind).ok_or(E::Format("pickup tiers: the pouch add has no next"))?;
    let sets: Vec<usize> = (0..f.events.len()).filter(|&i| action_is(f, i, GUEST, SET_NAME)).collect();
    let [set] = sets.as_slice() else {
        return Err(E::Msg(format!("pickup tiers: {} names the picked item {} times, expected once", f.name, sets.len())));
    };
    let set: usize = *set;
    let open: u16 = next_of(&f.events[set].kind).ok_or(E::Format("pickup tiers: the name event has no next"))?;
    let window: Kind = match f.events.get(usize::from(open)).map(|e| &e.kind) {
        Some(k @ Kind::Sub { flow, entry, .. }) if flow == WINDOW.0 && entry == WINDOW.1 => k.clone(),
        _ => return Err(E::Msg(format!("pickup tiers: {} does not open the get window right after naming the item", f.name))),
    };
    let own_at: u16 = idx(at)?;
    if !reach(&f.events, open).contains(&own_at) {
        return Err(E::Msg(format!("pickup tiers: the get window of {} does not lead to the pouch add", f.name)));
    }
    let expand: Option<Kind> = (0..f.events.len()).find(|&i| action_is(f, i, GUEST, EXPAND) && next_of(&f.events[i].kind) == Some(own_at)).map(|i| f.events[i].kind.clone());
    let t: Tail = Tail { window, expand, own: f.events[at].kind.clone(), then };
    let n: usize = f.events.len();
    let mut hits: Vec<(Vec<String>, u16)> = Vec::new();
    for fam in fams.iter() {
        for picked in 0..tiers(fam)?.len() {
            let hit: u16 = give(f, &t, fam, picked)?;
            hits.push((names(fam, picked)?, hit));
        }
    }
    let rename: u16 = push(f, with_next(&f.events[set].kind.clone(), open))?;
    let mut by_arg: u16 = rename;
    for (ns, hit) in hits.iter().rev() {
        for nm in ns.iter().rev() {
            by_arg = ask(f, nm, *hit, by_arg)?;
        }
    }
    let (da, dx): (u16, u16) = crate::start_state::slot(f, DATA, SET_TEXT)?;
    let copy: Cont = Cont { items: vec![("GameDataName".to_string(), Val::Str(NAME.to_string())), ("Value".to_string(), Val::Arg(ARG.to_string())), ("Index".to_string(), Val::Int(-1))] };
    let from_arg: u16 = push(f, Kind::Action { next: by_arg, actor: da, act: dx, params: Some(copy) })?;
    let mut miss: u16 = from_arg;
    for (ns, hit) in hits.iter().rev() {
        for nm in ns.iter().rev() {
            miss = ask(f, nm, *hit, miss)?;
        }
    }
    let first: u16 = settle(f, miss)?;
    if let Kind::Action { next, .. } = &mut f.events[set].kind {
        *next = first;
    }
    let calls: Vec<u16> = (n..f.events.len()).filter(|&i| matches!(f.events[i].kind, Kind::Sub { .. })).filter_map(|i| idx(i).ok()).collect();
    for k in 0..f.entries.len() {
        let r: BS<u16> = reach(&f.events, f.entries[k].main);
        for c in &calls {
            if r.contains(c) && !f.entries[k].subs.contains(c) {
                f.entries[k].subs.push(*c);
            }
        }
    }
    Ok(true)
}

fn flow_mut<'a>(b: &'a mut Bf, name: &str) -> Res<&'a mut Flow> {
    b.flows.iter_mut().find(|f| f.name == name).ok_or_else(|| E::Msg(format!("pickup tiers: no {name} flow")))
}

pub fn stage(rf: &mut Romfs, late: &mut Vec<Out>, work: &Path, fams: &BS<&str>) -> Res<()> {
    if fams.is_empty() {
        return Ok(());
    }
    for (flow, i, name) in GETS {
        crate::story_skip::stage_late(rf, late, work, flow, |b| patch(flow_mut(b, flow)?, i, name, fams).map(|_| ()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, Value};
    use crate::slice::generate::{generate_data, GenOpts, Generated};

    fn dump() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.is_dir() {
            Some(p)
        } else {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
            None
        }
    }

    fn walk(f: &Flow, from: u16, own_yes: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut at: u16 = from;
        let mut steps: usize = 0;
        while at != NONE && steps < 64 {
            steps += 1;
            let e: &Event = &f.events[usize::from(at)];
            at = match &e.kind {
                Kind::Switch { actor, query, cases, params } => {
                    let a = &f.actors[usize::from(*actor)];
                    let q: &str = &a.queries[usize::from(*query)];
                    let yes: bool = match params.as_ref().and_then(|c| c.get("ActorName").or_else(|| c.get("Value"))) {
                        Some(Val::Str(s)) if q == OWNS => own_yes.contains(&s.as_str()),
                        _ => false,
                    };
                    let v: u32 = if yes { 0 } else { 1 };
                    cases.iter().find(|c| c.val == v).map(|c| c.ev).unwrap_or(NONE)
                }
                Kind::Action { next, actor, act, params } => {
                    let a = &f.actors[usize::from(*actor)];
                    let what: &str = &a.actions[usize::from(*act)];
                    let actor_name: String = match params.as_ref().and_then(|c| c.get("Actor")) {
                        Some(Val::Str(s)) => s.clone(),
                        _ => String::new(),
                    };
                    let num: String = match params.as_ref().and_then(|c| c.get("Num")) {
                        Some(Val::Int(n)) => n.to_string(),
                        _ => String::new(),
                    };
                    out.push(format!("{what} {actor_name} {num}").trim().to_string());
                    *next
                }
                Kind::Sub { next, flow, entry, .. } => {
                    out.push(format!("{flow}:{entry}"));
                    *next
                }
                _ => NONE,
            };
        }
        out
    }

    fn entry_of(f: &Flow, family: &str, picked: usize) -> u16 {
        let n: String = crate::slice::key_spots::pick(tiers(family).unwrap()[picked]).unwrap().pack;
        let hit = f.events.iter().find_map(|e| match &e.kind {
            Kind::Switch { cases, params: Some(c), .. } if c.get("Value") == Some(&Val::Str(n.clone())) && c.get("GameDataName") == Some(&Val::Str(NAME.to_string())) => cases.iter().find(|c| c.val == 0).map(|c| c.ev),
            _ => None,
        });
        hit.unwrap()
    }

    #[test]
    fn a_picked_strength_pack_gives_the_next_tier_the_player_lacks() {
        let Some(root) = dump() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let fams: BS<&str> = ["Strength", "StickBag"].into_iter().collect();
        for (flow, i, name) in GETS {
            let mut b: Bf = Bf::parse(&rf.read_zs(&format!("Event/EventFlow/{flow}.bfevfl.zs")).unwrap()).unwrap();
            assert!(patch(flow_mut(&mut b, flow).unwrap(), i, name, &fams).unwrap());
            let b: Bf = Bf::parse(&b.write().unwrap()).unwrap();
            let f: &Flow = b.flows.iter().find(|x| x.name == flow).unwrap();
            let silver: Vec<String> = walk(f, entry_of(f, "Strength", 1), &[]);
            assert_eq!(silver.first().map(|s| s.as_str()), Some("EventTriggerAddOrRemovePouchContent Item_Gauntlet_Goron 1"), "{flow} {silver:?}");
            assert!(silver.contains(&"GetPouchContent:OpenGetWindow".to_string()), "{silver:?}");
            let own: usize = silver.iter().position(|s| s == ADD_OWN).unwrap();
            assert_eq!(&silver[own + 1..own + 4], [WAIT, "EventTriggerAddOrRemovePouchContent Item_Gauntlet_Silver -1", "EventTriggerAddOrRemovePouchContent Item_Gauntlet_Goron 1"], "{silver:?}");
            let again: Vec<String> = walk(f, entry_of(f, "Strength", 0), &["Armor_Child_Gauntlet_Goron"]);
            assert_eq!(again.first().map(|s| s.as_str()), Some("EventTriggerAddOrRemovePouchContent Item_Gauntlet_Silver 1"), "{again:?}");
            let own: usize = again.iter().position(|s| s == ADD_OWN).unwrap();
            assert_eq!(&again[own + 1..own + 3], [WAIT, "EventTriggerAddOrRemovePouchContent Item_Gauntlet_Silver 1"], "{again:?}");
            let stick: Vec<String> = walk(f, entry_of(f, "StickBag", 1), &[]);
            assert_eq!(stick.first().map(|s| s.as_str()), Some("EventTriggerAddOrRemovePouchContent Item_StickBag_Medium 1"), "{stick:?}");
            let own: usize = stick.iter().position(|s| s == ADD_OWN).unwrap();
            assert_eq!(&stick[own + 1..own + 4], [WAIT, "EventTriggerAddOrRemovePouchContent Item_StickBag_Large -1", "EventTriggerAddOrRemovePouchContent Item_StickBag_Medium 1"], "{stick:?}");
            let set: &Event = f.events.iter().find(|e| matches!(&e.kind, Kind::Action { actor, act, .. } if f.actors[usize::from(*actor)].actions[usize::from(*act)] == SET_NAME) && !e.name.starts_with(PREFIX)).unwrap();
            let other: Vec<String> = walk(f, next_of(&set.kind).unwrap(), &[]);
            assert_eq!(other.first().map(|s| s.as_str()), Some(WAIT), "{other:?}");
            let k: usize = other.iter().position(|s| s == SET_TEXT).unwrap();
            assert_eq!(&other[k + 1..k + 3], [SET_NAME, "GetPouchContent:OpenGetWindow"], "{other:?}");
            let mut twice: Bf = b.clone();
            assert!(!patch(flow_mut(&mut twice, flow).unwrap(), i, name, &fams).unwrap());
        }
    }

    #[test]
    fn upgrade_packs_replay_their_get_event_and_others_do_not() {
        for item in ["Item_Gauntlet_Goron", "Item_Gauntlet_Silver", "Item_Hookshot", "Item_BombBag_Small", "Item_StickBag_Large", "Item_Range_Bow", "Weapon_Range_Bow", "Item_Quiver_Medium", "Item_FairyBless_MP"] {
            assert!(replays(item), "{item}");
        }
        for item in ["Item_Ocarina_Fairy", "Item_Ocarina_Time", "Item_Heart_Piece", "Obj_Rupee_Green", "Item_Arrow_Ice", "Weapon_Sword_Kokiri"] {
            assert!(!replays(item), "{item}");
        }
    }

    #[test]
    fn families_follow_the_upgrades_at_fixed_spots() {
        let d: Data = Data::load().unwrap();
        let mut s: Settings = Settings::default();
        s.set(crate::slice::pickups::SETTING, Value::s("all")).unwrap();
        let mut seen: BS<&str> = BS::new();
        for n in 0..8 {
            let g: Generated = generate_data(&d, &s, &format!("pickup tiers {n}"), &GenOpts::default()).unwrap();
            let fams: BS<&str> = families(&d, &g.seed_file);
            assert!(!fams.contains("Ocarina"));
            for (loc, item) in &g.seed_file.placements {
                if let Some(f) = d.family(item).filter(|f| runtime(&d, loc, f)) {
                    assert!(fams.contains(f), "{loc} {item}");
                }
            }
            seen.extend(fams);
        }
        assert!(!seen.is_empty());
    }
}
