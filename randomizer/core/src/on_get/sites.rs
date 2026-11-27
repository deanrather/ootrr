use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Case, Cont, Event, Flow, Kind, Val};
use crate::error::{Error as E, Res};
use crate::on_get::{At, OnGet, FLOWS};

pub const NAME: &str = "UI_MessageTargetActorName";
pub const ON_SALE: &str = "Shop_SelectItemName";
pub const POUCH: &str = "PouchSystemActor";
pub const ADD: &str = "EventTriggerAddOrRemovePouchContent";
pub const GUEST: &str = "PouchContentGuestPerformer";
pub const ADD_OWN: &str = "EventTriggerAddOwnToPouch";
pub const SHOP: &str = "DisplayShop";
pub const STARTER: &str = "Npc_EventStarter";
pub const GRANT: &str = "EventTriggerGetItemAtShop";
pub const DATA: &str = "GameDataActor";
pub const ASK: &str = "EventQueryIsGameDataString64";
pub const SET: &str = "EventTriggerSetGameDataString64";
pub const WAITER: &str = "EventSystemActor";
pub const WAIT: &str = "EventWait";

pub struct Give {
    pub flow: &'static str,
    pub event: &'static str,
    pub at: At,
}

pub const GIVES: [Give; 4] = [
    Give { flow: "GetPouchContent", event: "Event0", at: At::Gift },
    Give { flow: "Dm_SY_GetInEvent", event: "Event124", at: At::Gift },
    Give { flow: "Dm_SY_GetInEvent", event: "Event442", at: At::Sale },
    Give { flow: "Dm_SY_GetInEvent", event: "Event430", at: At::Sale },
];
pub const PICKUPS: [(&str, &str); 2] = [("Dm_SY_Get", "Event35"), ("Dm_SY_Get_WithoutPlayer", "Event0")];

pub fn path_of(flow: &str) -> String {
    format!("{FLOWS}/{flow}.bfevfl.zs")
}

pub fn fixed() -> Vec<&'static str> {
    let mut out: Vec<&str> = GIVES.iter().map(|g| g.flow).chain(PICKUPS.iter().map(|p| p.0)).chain([SHOP]).collect();
    out.sort();
    out.dedup();
    out
}

pub fn names(items: &[&str]) -> Res<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let mut n: Vec<String> = vec![item.to_string(), item.replace("Item_", "Pouch_"), item.replace("Item_", "Obj_")];
        let tier: Option<(&str, usize)> = crate::progressive::FAMILIES.iter().find_map(|(f, t)| t.iter().position(|x| x == item).map(|i| (*f, i)));
        if let Some((family, t)) = tier {
            n.extend(crate::pickup_tiers::names(family, t)?);
        }
        for x in n {
            if !out.contains(&x) {
                out.push(x);
            }
        }
    }
    Ok(out)
}

pub fn ours(f: &Flow, h: &OnGet, at: u16) -> bool {
    at != NONE && f.events.get(usize::from(at)).is_some_and(|e| e.name.starts_with(h.event_prefix))
}

fn push(f: &mut Flow, h: &OnGet, kind: Kind) -> Res<u16> {
    let at: u16 = u16::try_from(f.events.len()).ok().filter(|i| *i != NONE).ok_or(E::Format("on get: the flow has too many events"))?;
    f.events.push(Event { name: format!("{}{at}", h.event_prefix), kind });
    Ok(at)
}

fn act(f: &mut Flow, h: &OnGet, who: &str, what: &str, items: Vec<(&str, Val)>, next: u16) -> Res<u16> {
    let (a, x): (u16, u16) = crate::start_state::slot(f, who, what)?;
    let c: Cont = Cont { items: items.into_iter().map(|(k, v)| (k.to_string(), v)).collect() };
    push(f, h, Kind::Action { next, actor: a, act: x, params: Some(c) })
}

fn asked(f: &mut Flow, h: &OnGet, data: &str, hit: u16, miss: u16) -> Res<u16> {
    let (a, q): (u16, u16) = crate::progressive::ensure(f, DATA, ASK)?;
    let mut at: u16 = miss;
    for n in names(h.items)?.into_iter().rev() {
        let c: Cont = Cont { items: vec![("GameDataName".to_string(), Val::Str(data.to_string())), ("Index".to_string(), Val::Int(-1)), ("Value".to_string(), Val::Str(n))] };
        at = push(f, h, Kind::Switch { actor: a, query: q, cases: vec![Case { val: 0, ev: hit }, Case { val: 1, ev: at }], params: Some(c) })?;
    }
    Ok(at)
}

fn named(f: &mut Flow, h: &OnGet, arg: &str, next: u16) -> Res<u16> {
    let settle: u16 = act(f, h, WAITER, WAIT, vec![("IsWaitEnd", Val::Bool(1)), ("Frames", Val::Int(1))], next)?;
    act(f, h, DATA, SET, vec![("GameDataName", Val::Str(NAME.to_string())), ("Value", Val::Arg(arg.to_string())), ("Index", Val::Int(-1))], settle)
}

pub fn is(f: &Flow, i: usize, who: &str, what: &str) -> bool {
    match f.events.get(i).map(|e| &e.kind) {
        Some(Kind::Action { actor, act, .. }) => f.actors.get(usize::from(*actor)).is_some_and(|a| a.name == who && a.actions.get(usize::from(*act)).is_some_and(|x| x == what)),
        _ => false,
    }
}

pub fn next_of(f: &Flow, i: usize) -> Option<u16> {
    match f.events.get(i).map(|e| &e.kind) {
        Some(Kind::Action { next, .. }) | Some(Kind::Sub { next, .. }) => Some(*next),
        _ => None,
    }
}

fn set_next(f: &mut Flow, i: usize, to: u16) {
    if let Some(Kind::Action { next, .. }) | Some(Kind::Sub { next, .. }) = f.events.get_mut(i).map(|e| &mut e.kind) {
        *next = to;
    }
}

fn unique(f: &Flow, h: &OnGet, name: &str) -> Res<usize> {
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| f.events[i].name == name).collect();
    match hits.as_slice() {
        [i] => Ok(*i),
        _ => Err(E::Msg(format!("{}: {} has {} events named {name}, expected one, the game files differ", h.name, f.name, hits.len()))),
    }
}

fn num(f: &Flow, i: usize) -> Option<i32> {
    match f.events.get(i)?.kind.params()?.get("Num")? {
        Val::Int(n) => Some(*n),
        _ => None,
    }
}

fn then_of(f: &Flow, h: &OnGet, i: usize) -> Res<u16> {
    next_of(f, i).ok_or_else(|| E::Msg(format!("{}: {} event {i} has no next event, the game files differ", h.name, f.name)))
}

pub fn give(f: &mut Flow, h: &OnGet, g: &Give) -> Res<bool> {
    let at: usize = unique(f, h, g.event)?;
    let arg: String = match f.events[at].kind.params().and_then(|c| c.get("Actor")) {
        Some(Val::Arg(a)) if is(f, at, POUCH, ADD) && num(f, at).is_some_and(|n| n > 0) => a.clone(),
        _ => return Err(E::Msg(format!("{}: {} {} is not a pouch add of an argument actor, the game files differ", h.name, f.name, g.event))),
    };
    let then: u16 = then_of(f, h, at)?;
    if ours(f, h, then) {
        return Ok(false);
    }
    let hit: u16 = (h.add_events)(f, g.at, then)?;
    let ask: u16 = asked(f, h, NAME, hit, then)?;
    let head: u16 = named(f, h, &arg, ask)?;
    set_next(f, at, head);
    Ok(true)
}

pub fn pickup(f: &mut Flow, h: &OnGet, event: &str) -> Res<bool> {
    let at: usize = unique(f, h, event)?;
    if !is(f, at, GUEST, ADD_OWN) {
        return Err(E::Msg(format!("{}: {} {event} is not the pickup's pouch add, the game files differ", h.name, f.name)));
    }
    let then: u16 = then_of(f, h, at)?;
    if ours(f, h, then) {
        return Ok(false);
    }
    let tails: Vec<usize> = (0..f.events.len()).filter(|&i| i == at || (f.events[i].name.starts_with(crate::pickup_tiers::PREFIX) && next_of(f, i) == Some(then))).collect();
    let hit: u16 = (h.add_events)(f, At::Gift, then)?;
    let head: u16 = asked(f, h, NAME, hit, then)?;
    for i in tails {
        set_next(f, i, head);
    }
    Ok(true)
}

pub fn shelf_end(f: &Flow, grant: usize) -> usize {
    let Some(stop) = crate::shelf::hook::GRANTS.iter().find(|g| g.0 == grant).map(|g| g.2) else {
        return grant;
    };
    let mut seen: Vec<u16> = Vec::new();
    let mut todo: Vec<u16> = next_of(f, grant).into_iter().collect();
    while let Some(i) = todo.pop() {
        let Some(e) = f.events.get(usize::from(i)) else {
            continue;
        };
        if i == stop || seen.contains(&i) {
            continue;
        }
        seen.push(i);
        match &e.kind {
            Kind::Sub { entry, .. } if entry == crate::shelf::hook::GIVE => return usize::from(i),
            Kind::Action { next, .. } | Kind::Sub { next, .. } | Kind::Join { next } => todo.push(*next),
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
            Kind::Fork { join, forks } => todo.extend(forks.iter().copied().chain([*join])),
            Kind::Label { .. } => {}
        }
    }
    grant
}

fn shelves(f: &mut Flow, h: &OnGet) -> Res<usize> {
    let grants: Vec<usize> = (0..f.events.len()).filter(|&i| is(f, i, STARTER, GRANT)).collect();
    if grants.is_empty() {
        return Err(E::Msg(format!("{}: the shop flow has no shelf purchase, the game files differ", h.name)));
    }
    let mut n: usize = 0;
    for g in grants {
        let at: usize = shelf_end(f, g);
        let then: u16 = then_of(f, h, at)?;
        if ours(f, h, then) {
            continue;
        }
        let hit: u16 = (h.add_events)(f, At::Sale, then)?;
        let head: u16 = asked(f, h, ON_SALE, hit, then)?;
        set_next(f, at, head);
        n += 1;
    }
    Ok(n)
}

pub fn literal(f: &Flow, h: &OnGet, i: usize) -> bool {
    let listed: bool = matches!(f.events[i].kind.params().and_then(|c| c.get("Actor")), Some(Val::Str(s)) if h.items.contains(&s.as_str()));
    listed && is(f, i, POUCH, ADD) && num(f, i).is_some_and(|n| n > 0)
}

fn literals(f: &mut Flow, h: &OnGet) -> Res<usize> {
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| literal(f, h, i)).collect();
    let mut n: usize = 0;
    for i in hits {
        let then: u16 = then_of(f, h, i)?;
        if ours(f, h, then) {
            continue;
        }
        let hit: u16 = (h.add_events)(f, At::Gift, then)?;
        set_next(f, i, hit);
        n += 1;
    }
    Ok(n)
}

pub fn patch(f: &mut Flow, h: &OnGet) -> Res<usize> {
    let name: String = f.name.clone();
    if name == SHOP {
        return shelves(f, h);
    }
    let mut n: usize = 0;
    for g in GIVES.iter().filter(|g| g.flow == name) {
        n += usize::from(give(f, h, g)?);
    }
    for (_, e) in PICKUPS.iter().filter(|p| p.0 == name) {
        n += usize::from(pickup(f, h, e)?);
    }
    if !PICKUPS.iter().any(|p| p.0 == name) {
        n += literals(f, h)?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet as BS;
    use std::path::PathBuf as PB;

    use super::*;
    use crate::bfevfl::Bf;
    use crate::full_wallet::fill;
    use crate::full_wallet::ON_GET as W;
    use crate::romfs::Romfs;

    fn real() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(p)
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn flow(rf: &mut Romfs, name: &str) -> Flow {
        let b: Bf = Bf::parse(&rf.read_zs(&path_of(name)).unwrap()).unwrap();
        b.flows.into_iter().find(|f| f.name == name).unwrap()
    }

    fn what(f: &Flow, i: u16) -> String {
        match &f.events[usize::from(i)].kind {
            Kind::Action { actor, act, .. } => format!("{}.{}", f.actors[usize::from(*actor)].name, f.actors[usize::from(*actor)].actions[usize::from(*act)]),
            Kind::Switch { actor, query, .. } => format!("{}.{}", f.actors[usize::from(*actor)].name, f.actors[usize::from(*actor)].queries[usize::from(*query)]),
            k => format!("{k:?}"),
        }
    }

    fn line(f: &Flow, from: u16, stop: u16) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut at: u16 = from;
        while at != stop {
            assert!(ours(f, &W, at), "event {at} of {} is not ours", f.name);
            out.push(what(f, at));
            at = match &f.events[usize::from(at)].kind {
                Kind::Action { next, .. } => *next,
                k => panic!("not an action: {k:?}"),
            };
        }
        out
    }

    fn param(f: &Flow, i: u16, k: &str) -> Option<Val> {
        f.events[usize::from(i)].kind.params().and_then(|c| c.get(k)).cloned()
    }

    fn popup() -> Vec<String> {
        [format!("{}.{}", fill::UI, fill::OPEN), "PouchSystemActor.EventTriggerIncreaseRupee".to_string(), format!("{}.{}", fill::UI, fill::STABLE), "EventSystemActor.EventWait".to_string(), format!("{}.{}", fill::UI, fill::CLOSE)].to_vec()
    }

    fn sale() -> Vec<String> {
        ["PouchSystemActor.EventTriggerIncreaseRupee".to_string(), format!("{}.{}", fill::UI, fill::STABLE)].to_vec()
    }

    fn check_fill(f: &Flow, hit: u16, then: u16, at: At) {
        assert_eq!(line(f, hit, then), if at == At::Gift { popup() } else { sale() });
        let add: u16 = if at == At::Gift { next_of(f, usize::from(hit)).unwrap() } else { hit };
        assert_eq!(param(f, add, "Value"), Some(Val::Int(fill::FILL)));
    }

    fn asks(f: &Flow, head: u16, then: u16, data: &str) -> u16 {
        let mut at: u16 = head;
        let mut seen: Vec<String> = Vec::new();
        let mut hit: Option<u16> = None;
        while at != then {
            let Kind::Switch { cases, .. } = &f.events[usize::from(at)].kind else { panic!("not a question") };
            assert_eq!(what(f, at), "GameDataActor.EventQueryIsGameDataString64");
            assert_eq!(param(f, at, "GameDataName"), Some(Val::Str(data.to_string())));
            let Some(Val::Str(n)) = param(f, at, "Value") else { panic!("no name") };
            seen.push(n);
            assert_eq!(cases.len(), 2);
            assert!(hit.is_none_or(|h| h == cases[0].ev));
            hit = Some(cases[0].ev);
            at = cases[1].ev;
        }
        assert_eq!(seen, names(W.items).unwrap());
        hit.unwrap()
    }

    #[test]
    fn each_give_routine_add_runs_the_hook_only_for_its_items_and_keeps_its_path() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        for g in GIVES.iter() {
            let was: Flow = flow(&mut rf, g.flow);
            let mut f: Flow = was.clone();
            let at: usize = unique(&f, &W, g.event).unwrap();
            let then: u16 = next_of(&f, at).unwrap();
            let Some(Val::Arg(arg)) = param(&f, at as u16, "Actor") else { panic!("{} {}", g.flow, g.event) };
            assert!(give(&mut f, &W, g).unwrap());
            let head: u16 = next_of(&f, at).unwrap();
            assert_eq!(what(&f, head), format!("{DATA}.{SET}"));
            assert_eq!(param(&f, head, "Value"), Some(Val::Arg(arg.clone())));
            assert_eq!(param(&f, head, "GameDataName"), Some(Val::Str(NAME.to_string())));
            let settle: u16 = next_of(&f, usize::from(head)).unwrap();
            assert_eq!(what(&f, settle), "EventSystemActor.EventWait");
            assert_eq!(param(&f, settle, "Frames"), Some(Val::Int(1)));
            let first: u16 = next_of(&f, usize::from(settle)).unwrap();
            let hit: u16 = asks(&f, first, then, NAME);
            check_fill(&f, hit, then, g.at);
            for (i, e) in was.events.iter().enumerate() {
                if i != at {
                    assert_eq!(&f.events[i], e, "{} event {i}", g.flow);
                }
            }
            let again: Flow = f.clone();
            assert!(!give(&mut f, &W, g).unwrap());
            assert_eq!(f, again);
        }
    }

    #[test]
    fn a_ground_pickup_runs_the_hook_after_its_pouch_add_with_and_without_pickup_tiers() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let fams: BS<&str> = ["Wallet"].into_iter().collect();
        for ((name, event), (gf, gi, gn)) in PICKUPS.iter().zip(crate::pickup_tiers::GETS) {
            assert_eq!((*name, *event), (gf, gn));
            for tiers in [false, true] {
                let mut f: Flow = flow(&mut rf, name);
                if tiers {
                    assert!(crate::pickup_tiers::patch(&mut f, gi, gn, &fams).unwrap());
                }
                let at: usize = unique(&f, &W, event).unwrap();
                let then: u16 = next_of(&f, at).unwrap();
                let before: usize = (0..f.events.len()).filter(|&i| next_of(&f, i) == Some(then)).count();
                assert_eq!(before, if tiers { 5 } else { 1 }, "{name} tiers {tiers}");
                assert!(pickup(&mut f, &W, event).unwrap());
                let head: u16 = next_of(&f, at).unwrap();
                let into: usize = (0..f.events.len()).filter(|&i| next_of(&f, i) == Some(head)).count();
                assert_eq!(into, before, "{name} tiers {tiers}");
                let stray: Vec<usize> = (0..f.events.len()).filter(|&i| next_of(&f, i) == Some(then) && !ours(&f, &W, i as u16)).collect();
                assert!(stray.is_empty(), "{name} tiers {tiers}: {stray:?}");
                let hit: u16 = asks(&f, head, then, NAME);
                check_fill(&f, hit, then, At::Gift);
                assert!(!pickup(&mut f, &W, event).unwrap());
                let mut bf: Bf = Bf::parse(&rf.read_zs(&path_of(name)).unwrap()).unwrap();
                let k: usize = bf.flows.iter().position(|x| x.name == *name).unwrap();
                bf.flows[k] = f.clone();
                assert_eq!(Bf::parse(&bf.write().unwrap()).unwrap().flows[k], f);
            }
        }
    }

    #[test]
    fn every_shelf_purchase_runs_the_hook_after_the_shelf_adds_the_tier() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let sold: Vec<crate::shelf::hook::Sold> = vec![crate::shelf::hook::Sold { family: "Wallet", counters: vec!["W.0".to_string()], copies: 1 }];
        for tiered in [false, true] {
            let raw: Vec<u8> = rf.read_zs(&path_of(SHOP)).unwrap();
            let raw: Vec<u8> = if tiered { crate::shelf::hook::edit(&raw, &sold, &[], false).unwrap() } else { raw };
            let mut b: Bf = Bf::parse(&raw).unwrap();
            let f: &mut Flow = &mut b.flows[0];
            let grants: Vec<usize> = (0..f.events.len()).filter(|&i| is(f, i, STARTER, GRANT)).collect();
            assert_eq!(grants.len(), 3);
            let ends: Vec<usize> = grants.iter().map(|&g| shelf_end(f, g)).collect();
            let gives: usize = ends.iter().filter(|&&e| matches!(&f.events[e].kind, Kind::Sub { entry, .. } if entry == crate::shelf::hook::GIVE)).count();
            assert_eq!(gives, if tiered { 2 } else { 0 });
            let thens: Vec<u16> = ends.iter().map(|&e| next_of(f, e).unwrap()).collect();
            assert_eq!(patch(f, &W).unwrap(), 3);
            for (&end, &then) in ends.iter().zip(&thens) {
                let head: u16 = next_of(f, end).unwrap();
                let hit: u16 = asks(f, head, then, ON_SALE);
                check_fill(f, hit, then, At::Sale);
            }
            assert_eq!(patch(f, &W).unwrap(), 0);
            let out: Vec<u8> = b.write().unwrap();
            assert_eq!(Bf::parse(&out).unwrap(), b);
        }
    }

    #[test]
    fn a_shelf_trap_in_the_same_shop_does_not_move_the_hook_before_the_tier_add() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let sold: Vec<crate::shelf::hook::Sold> = vec![crate::shelf::hook::Sold { family: "Wallet", counters: vec!["W.0".to_string()], copies: 1 }];
        let raw: Vec<u8> = crate::shelf::hook::edit(&rf.read_zs(&path_of(SHOP)).unwrap(), &sold, &[], false).unwrap();
        let mut b: Bf = Bf::parse(&raw).unwrap();
        crate::frost::trigger::hook_shop(&mut b, &["Item_Rupee_Gold".to_string()]).unwrap();
        let f: &mut Flow = &mut b.flows[0];
        for (grant, _, _) in crate::shelf::hook::GRANTS {
            assert!(!matches!(&f.events[usize::from(next_of(f, grant).unwrap())].kind, Kind::Sub { .. }), "the trap's questions come first");
            let end: usize = shelf_end(f, grant);
            assert!(matches!(&f.events[end].kind, Kind::Sub { entry, .. } if entry == crate::shelf::hook::GIVE), "grant {grant} ends at {end}");
        }
        assert_eq!(patch(f, &W).unwrap(), 3);
        for (grant, _, _) in crate::shelf::hook::GRANTS {
            let end: usize = shelf_end(f, grant);
            assert!(ours(f, &W, next_of(f, end).unwrap()), "grant {grant}");
            assert!(!ours(f, &W, next_of(f, grant).unwrap()), "grant {grant}");
        }
        assert_eq!(patch(f, &W).unwrap(), 0);
    }

    #[test]
    fn a_literal_add_of_a_listed_item_runs_the_hook() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let mut f: Flow = flow(&mut rf, "Npc_Hylia_001_Adult");
        assert_eq!(patch(&mut f.clone(), &W).unwrap(), 0, "the game's own file gives no wallet");
        let (a, x): (u16, u16) = crate::start_state::slot(&mut f, POUCH, ADD).unwrap();
        let mut adds: Vec<u16> = Vec::new();
        for (actor, n) in [("Item_Wallet_Large", 1), ("Item_Wallet_Medium", -1), ("Item_Wallet_Small", 1), ("Item_Wallet_Medium", 1)] {
            let c: Cont = Cont { items: vec![("Num".to_string(), Val::Int(n)), ("Actor".to_string(), Val::Str(actor.to_string()))] };
            adds.push(f.events.len() as u16);
            f.events.push(Event { name: format!("T{}", f.events.len()), kind: Kind::Action { next: 0, actor: a, act: x, params: Some(c) } });
        }
        assert_eq!(patch(&mut f, &W).unwrap(), 2);
        for (k, at) in adds.iter().enumerate() {
            let to: u16 = next_of(&f, usize::from(*at)).unwrap();
            assert_eq!(ours(&f, &W, to), k == 0 || k == 3, "add {k}");
            if ours(&f, &W, to) {
                check_fill(&f, to, 0, At::Gift);
            }
        }
        assert_eq!(patch(&mut f, &W).unwrap(), 0);
    }

    #[test]
    fn the_game_files_give_no_wallet_outside_the_hooked_flows() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let dir: PB = p.join("Event").join("EventFlow");
        let mut sites: usize = 0;
        let mut files: usize = 0;
        for e in std::fs::read_dir(&dir).unwrap() {
            let n: String = e.unwrap().file_name().to_string_lossy().to_string();
            let Some(stem) = n.strip_suffix(".bfevfl.zs") else {
                continue;
            };
            let raw: Vec<u8> = rf.read_zs(&path_of(stem)).unwrap();
            let mut b: Bf = Bf::parse(&raw).unwrap();
            let mut here: usize = 0;
            for f in b.flows.iter_mut() {
                here += patch(f, &W).unwrap();
            }
            assert_eq!(here > 0, fixed().contains(&stem), "{stem}");
            if here > 0 {
                let back: Bf = Bf::parse(&b.write().unwrap()).unwrap();
                assert_eq!(back.flows, b.flows, "{stem}");
            }
            sites += here;
            files += 1;
        }
        assert!(files > 900, "{files}");
        assert_eq!(sites, GIVES.len() + PICKUPS.len() + 3);
    }
}
