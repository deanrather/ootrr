use crate::bfevfl::{Flow, Kind, Val};
use crate::error::{Error as E, Res};
use crate::full_wallet::fill::{asked, fill, named, ours, Show};

pub const WALLETS: [&str; 2] = ["Item_Wallet_Medium", "Item_Wallet_Large"];
pub const FAMILY: &str = "Wallet";
pub const POUCH: &str = "PouchSystemActor";
pub const ADD: &str = "EventTriggerAddOrRemovePouchContent";
pub const GUEST: &str = "PouchContentGuestPerformer";
pub const ADD_OWN: &str = "EventTriggerAddOwnToPouch";

pub struct Give {
    pub flow: &'static str,
    pub event: &'static str,
    pub show: Show,
}

pub const GIVES: [Give; 4] = [
    Give { flow: "GetPouchContent", event: "Event0", show: Show::Popup },
    Give { flow: "Dm_SY_GetInEvent", event: "Event124", show: Show::Popup },
    Give { flow: "Dm_SY_GetInEvent", event: "Event442", show: Show::Shop },
    Give { flow: "Dm_SY_GetInEvent", event: "Event430", show: Show::Shop },
];
pub const PICKUPS: [(&str, &str); 2] = [("Dm_SY_Get", "Event35"), ("Dm_SY_Get_WithoutPlayer", "Event0")];
pub const SHOPS: [&str; 1] = ["DisplayShop"];

pub fn names() -> Res<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for (t, w) in WALLETS.iter().enumerate() {
        let mut n: Vec<String> = vec![w.to_string(), w.replace("Item_", "Pouch_"), w.replace("Item_", "Obj_")];
        n.extend(crate::pickup_tiers::names(FAMILY, t)?);
        for x in n {
            if !out.contains(&x) {
                out.push(x);
            }
        }
    }
    Ok(out)
}

pub fn path_of(flow: &str) -> String {
    format!("Event/EventFlow/{flow}.bfevfl.zs")
}

pub fn fixed() -> Vec<&'static str> {
    let mut out: Vec<&str> = GIVES.iter().map(|g| g.flow).chain(PICKUPS.iter().map(|p| p.0)).collect();
    out.sort();
    out.dedup();
    out
}

fn is(f: &Flow, i: usize, who: &str, what: &str) -> bool {
    match f.events.get(i).map(|e| &e.kind) {
        Some(Kind::Action { actor, act, .. }) => f.actors.get(usize::from(*actor)).is_some_and(|a| a.name == who && a.actions.get(usize::from(*act)).is_some_and(|x| x == what)),
        _ => false,
    }
}

fn next_of(f: &Flow, i: usize) -> Option<u16> {
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

fn unique(f: &Flow, name: &str) -> Res<usize> {
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| f.events[i].name == name).collect();
    match hits.as_slice() {
        [i] => Ok(*i),
        _ => Err(E::Msg(format!("full wallets: {} has {} events named {name}, expected one, the game files differ", f.name, hits.len()))),
    }
}

fn num(f: &Flow, i: usize) -> Option<i32> {
    match f.events.get(i)?.kind.params()?.get("Num")? {
        Val::Int(n) => Some(*n),
        _ => None,
    }
}

pub fn give(f: &mut Flow, g: &Give) -> Res<bool> {
    let at: usize = unique(f, g.event)?;
    let arg: String = match f.events[at].kind.params().and_then(|c| c.get("Actor")) {
        Some(Val::Arg(a)) if is(f, at, POUCH, ADD) && num(f, at).is_some_and(|n| n > 0) => a.clone(),
        _ => return Err(E::Msg(format!("full wallets: {} {} is not a pouch add of an argument actor, the game files differ", f.name, g.event))),
    };
    let then: u16 = next_of(f, at).ok_or(E::Format("full wallets: the pouch add has no next event"))?;
    if ours(f, then) {
        return Ok(false);
    }
    let hit: u16 = fill(f, g.show, then)?;
    let ask: u16 = asked(f, &names()?, hit, then)?;
    let head: u16 = named(f, &arg, ask)?;
    set_next(f, at, head);
    Ok(true)
}

pub fn pickup(f: &mut Flow, event: &str) -> Res<bool> {
    let at: usize = unique(f, event)?;
    if !is(f, at, GUEST, ADD_OWN) {
        return Err(E::Msg(format!("full wallets: {} {event} is not the pickup's pouch add, the game files differ", f.name)));
    }
    let then: u16 = next_of(f, at).ok_or(E::Format("full wallets: the pickup's pouch add has no next event"))?;
    if ours(f, then) {
        return Ok(false);
    }
    let tails: Vec<usize> = (0..f.events.len()).filter(|&i| i == at || (f.events[i].name.starts_with(crate::pickup_tiers::PREFIX) && next_of(f, i) == Some(then))).collect();
    let hit: u16 = fill(f, Show::Popup, then)?;
    let head: u16 = asked(f, &names()?, hit, then)?;
    for i in tails {
        set_next(f, i, head);
    }
    Ok(true)
}

pub fn literal(f: &Flow, i: usize) -> bool {
    let wallet: bool = matches!(f.events[i].kind.params().and_then(|c| c.get("Actor")), Some(Val::Str(s)) if WALLETS.contains(&s.as_str()));
    wallet && is(f, i, POUCH, ADD) && num(f, i).is_some_and(|n| n > 0)
}

pub fn literals(f: &mut Flow, show: Show) -> Res<usize> {
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| literal(f, i)).collect();
    let mut n: usize = 0;
    for i in hits {
        let then: u16 = next_of(f, i).ok_or(E::Format("full wallets: a wallet add has no next event"))?;
        if ours(f, then) {
            continue;
        }
        let hit: u16 = fill(f, show, then)?;
        set_next(f, i, hit);
        n += 1;
    }
    Ok(n)
}

pub fn is_site(flow: &str, event: &str) -> bool {
    GIVES.iter().any(|g| g.flow == flow && g.event == event) || PICKUPS.iter().any(|p| p.0 == flow && p.1 == event)
}

pub fn patch(f: &mut Flow) -> Res<usize> {
    let mut n: usize = 0;
    let name: String = f.name.clone();
    for g in GIVES.iter().filter(|g| g.flow == name) {
        n += usize::from(give(f, g)?);
    }
    for (_, e) in PICKUPS.iter().filter(|p| p.0 == name) {
        n += usize::from(pickup(f, e)?);
    }
    if !PICKUPS.iter().any(|p| p.0 == name) {
        let show: Show = if SHOPS.contains(&name.as_str()) { Show::Shop } else { Show::Popup };
        n += literals(f, show)?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap as BM, BTreeSet as BS};
    use std::path::PathBuf as PB;

    use super::*;
    use crate::bfevfl::flow::NONE;
    use crate::bfevfl::{Bf, Cont, Event};
    use crate::full_wallet::fill::{self, CLOSE, FILL, HOLD, NAME, OPEN, STABLE};
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
            assert!(ours(f, at), "event {at} of {} is not ours", f.name);
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
        [format!("{}.{OPEN}", fill::UI), "PouchSystemActor.EventTriggerIncreaseRupee".to_string(), format!("{}.{STABLE}", fill::UI), "EventSystemActor.EventWait".to_string(), format!("{}.{CLOSE}", fill::UI)].to_vec()
    }

    fn shop() -> Vec<String> {
        ["PouchSystemActor.EventTriggerIncreaseRupee".to_string(), format!("{}.{STABLE}", fill::UI)].to_vec()
    }

    fn check_fill(f: &Flow, hit: u16, then: u16, show: Show) {
        assert_eq!(line(f, hit, then), if show == Show::Popup { popup() } else { shop() });
        let add: u16 = if show == Show::Popup { match &f.events[usize::from(hit)].kind { Kind::Action { next, .. } => *next, _ => NONE } } else { hit };
        assert_eq!(param(f, add, "Value"), Some(Val::Int(FILL)));
    }

    fn asks(f: &Flow, head: u16, then: u16) -> u16 {
        let mut at: u16 = head;
        let mut seen: Vec<String> = Vec::new();
        let mut hit: Option<u16> = None;
        while at != then {
            let Kind::Switch { cases, .. } = &f.events[usize::from(at)].kind else { panic!("not a question") };
            assert_eq!(what(f, at), "GameDataActor.EventQueryIsGameDataString64");
            assert_eq!(param(f, at, "GameDataName"), Some(Val::Str(NAME.to_string())));
            let Some(Val::Str(n)) = param(f, at, "Value") else { panic!("no name") };
            seen.push(n);
            assert_eq!(cases.len(), 2);
            assert!(hit.is_none_or(|h| h == cases[0].ev));
            hit = Some(cases[0].ev);
            at = cases[1].ev;
        }
        assert_eq!(seen, names().unwrap());
        hit.unwrap()
    }

    #[test]
    fn each_give_routine_add_fills_only_after_a_wallet_and_keeps_its_path() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        for g in GIVES.iter() {
            let was: Flow = flow(&mut rf, g.flow);
            let mut f: Flow = was.clone();
            let at: usize = unique(&f, g.event).unwrap();
            let then: u16 = next_of(&f, at).unwrap();
            let Some(Val::Arg(arg)) = param(&f, at as u16, "Actor") else { panic!("{} {}", g.flow, g.event) };
            assert!(give(&mut f, g).unwrap());
            let head: u16 = next_of(&f, at).unwrap();
            assert_eq!(what(&f, head), format!("{}.{}", fill::DATA, fill::SET));
            assert_eq!(param(&f, head, "Value"), Some(Val::Arg(arg.clone())));
            assert_eq!(param(&f, head, "GameDataName"), Some(Val::Str(NAME.to_string())));
            let settle: u16 = match &f.events[usize::from(head)].kind { Kind::Action { next, .. } => *next, _ => NONE };
            assert_eq!(what(&f, settle), "EventSystemActor.EventWait");
            assert_eq!(param(&f, settle, "Frames"), Some(Val::Int(1)));
            let first: u16 = match &f.events[usize::from(settle)].kind { Kind::Action { next, .. } => *next, _ => NONE };
            let hit: u16 = asks(&f, first, then);
            check_fill(&f, hit, then, g.show);
            for (i, e) in was.events.iter().enumerate() {
                if i != at {
                    assert_eq!(&f.events[i], e, "{} event {i}", g.flow);
                }
            }
            let again: Flow = f.clone();
            assert!(!give(&mut f, g).unwrap());
            assert_eq!(f, again);
        }
    }

    #[test]
    fn a_ground_pickup_fills_after_its_pouch_add_with_and_without_pickup_tiers() {
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
                let at: usize = unique(&f, event).unwrap();
                let then: u16 = next_of(&f, at).unwrap();
                let before: usize = (0..f.events.len()).filter(|&i| next_of(&f, i) == Some(then)).count();
                assert_eq!(before, if tiers { 5 } else { 1 }, "{name} tiers {tiers}");
                assert!(pickup(&mut f, event).unwrap());
                let head: u16 = next_of(&f, at).unwrap();
                let into: usize = (0..f.events.len()).filter(|&i| next_of(&f, i) == Some(head)).count();
                assert_eq!(into, before, "{name} tiers {tiers}");
                let stray: Vec<usize> = (0..f.events.len()).filter(|&i| next_of(&f, i) == Some(then) && !ours(&f, i as u16)).collect();
                assert!(stray.is_empty(), "{name} tiers {tiers}: {stray:?}");
                let hit: u16 = asks(&f, head, then);
                check_fill(&f, hit, then, Show::Popup);
                assert!(!pickup(&mut f, event).unwrap());
                let mut bf: Bf = Bf::parse(&rf.read_zs(&path_of(name)).unwrap()).unwrap();
                let k: usize = bf.flows.iter().position(|x| x.name == *name).unwrap();
                bf.flows[k] = f.clone();
                assert_eq!(Bf::parse(&bf.write().unwrap()).unwrap().flows[k], f);
            }
        }
    }

    #[test]
    fn a_literal_wallet_add_fills_and_the_shop_keeps_its_own_counter() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        for (name, show) in [("DisplayShop", Show::Shop), ("Npc_Hylia_001_Adult", Show::Popup)] {
            let mut f: Flow = flow(&mut rf, name);
            assert_eq!(patch(&mut f.clone()).unwrap(), 0, "{name} gives no wallet in the game's own files");
            let (a, x): (u16, u16) = crate::start_state::slot(&mut f, POUCH, ADD).unwrap();
            let mut adds: Vec<u16> = Vec::new();
            for (actor, n) in [("Item_Wallet_Large", 1), ("Item_Wallet_Medium", -1), ("Item_Wallet_Small", 1), ("Item_Wallet_Medium", 1)] {
                let c: Cont = Cont { items: vec![("Num".to_string(), Val::Int(n)), ("Actor".to_string(), Val::Str(actor.to_string()))] };
                adds.push(f.events.len() as u16);
                f.events.push(Event { name: format!("T{}", f.events.len()), kind: Kind::Action { next: 0, actor: a, act: x, params: Some(c) } });
            }
            assert_eq!(patch(&mut f).unwrap(), 2, "{name}");
            for (k, at) in adds.iter().enumerate() {
                let to: u16 = next_of(&f, usize::from(*at)).unwrap();
                assert_eq!(ours(&f, to), k == 0 || k == 3, "{name} add {k}");
                if ours(&f, to) {
                    check_fill(&f, to, 0, show);
                }
            }
            assert_eq!(patch(&mut f).unwrap(), 0);
        }
    }

    fn reach(rf: &mut Romfs, cache: &mut BM<String, Option<Flow>>, flow: &str, entry: &str, seen: &mut BS<(String, String)>, hits: &mut BS<String>) {
        if !seen.insert((flow.to_string(), entry.to_string())) {
            return;
        }
        if !cache.contains_key(flow) {
            let f: Option<Flow> = rf.read_zs(&path_of(flow)).ok().and_then(|r| Bf::parse(&r).ok()).and_then(|b| b.flows.into_iter().find(|f| f.name == flow));
            cache.insert(flow.to_string(), f);
        }
        let Some(f) = cache.get(flow).cloned().flatten() else {
            hits.insert(format!("missing {flow}"));
            return;
        };
        let Some(e) = f.entries.iter().find(|e| e.name == entry) else {
            hits.insert(format!("missing {flow}:{entry}"));
            return;
        };
        let mut todo: Vec<u16> = vec![e.main];
        let mut done: BS<u16> = BS::new();
        while let Some(i) = todo.pop() {
            if i == NONE || !done.insert(i) {
                continue;
            }
            let ev: &Event = &f.events[usize::from(i)];
            if is_site(flow, &ev.name) {
                hits.insert(format!("{flow}:{}", ev.name));
            }
            match &ev.kind {
                Kind::Action { next, .. } => todo.push(*next),
                Kind::Join { next } => todo.push(*next),
                Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
                Kind::Fork { join, forks } => {
                    todo.push(*join);
                    todo.extend(forks.iter().copied());
                }
                Kind::Sub { .. } | Kind::Label { .. } => {}
            }
            if let Kind::Sub { next, flow: to, entry: en, .. } = &ev.kind {
                todo.push(*next);
                let into: String = if to.is_empty() { flow.to_string() } else { to.clone() };
                reach(rf, cache, &into, en, seen, hits);
            }
        }
    }

    const SWAPS: [(&str, &str); 2] = [("GetInEventInBottle", "GetInEvent_NotTurn_NotNpc"), ("GetInEventInEquipedBottle", "GetInEvent_NotTurn")];

    #[test]
    fn every_flow_location_grant_and_both_chests_reach_a_wallet_fill() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let d: crate::flow_locations::Data = crate::flow_locations::Data::load().unwrap();
        let mut cache: BM<String, Option<Flow>> = BM::new();
        let mut rows: BM<String, BS<String>> = BM::new();
        let mut open: Vec<String> = Vec::new();
        let mut made: Vec<String> = Vec::new();
        for l in d.locs.iter().filter(|l| l.supported || l.gated) {
            for e in l.edits.iter().filter(|e| e.role == "grant") {
                let file: String = e.file.trim_end_matches(".bfevfl").to_string();
                let Ok(raw) = rf.read_zs(&path_of(&file)) else {
                    made.push(format!("{} ({file})", l.name));
                    continue;
                };
                let Some(f) = Bf::parse(&raw).unwrap().flows.into_iter().find(|x| x.name == file) else {
                    made.push(format!("{} ({file})", l.name));
                    continue;
                };
                let ev: &Event = &f.events[e.index];
                let mut hits: BS<String> = BS::new();
                let via: String = match &ev.kind {
                    Kind::Sub { flow: to, entry, .. } => {
                        let into: String = if to.is_empty() { file.clone() } else { to.clone() };
                        let entry: &str = SWAPS.iter().find(|s| into == "Dm_SY_GetInEvent" && s.0 == entry).map(|s| s.1).unwrap_or(entry);
                        reach(&mut rf, &mut cache, &into, entry, &mut BS::new(), &mut hits);
                        format!("{into}:{entry}")
                    }
                    Kind::Action { .. } if is(&f, e.index, POUCH, ADD) => {
                        hits.insert("literal add".to_string());
                        format!("{POUCH}.{ADD}")
                    }
                    k => format!("{k:?}"),
                };
                let ok: bool = hits.iter().any(|h| !h.starts_with("missing"));
                let key: String = format!("{} | {via}", l.cat);
                rows.entry(key.clone()).or_default().extend(hits.into_iter());
                if !ok {
                    open.push(format!("{} ({key})", l.name));
                }
            }
        }
        for path in [crate::progressive::SMALL_FLOW, crate::progressive::BIG_FLOW] {
            let name: &str = path.trim_start_matches("Event/EventFlow/").trim_end_matches(".bfevfl.zs");
            let mut hits: BS<String> = BS::new();
            reach(&mut rf, &mut cache, name, "Success_", &mut BS::new(), &mut hits);
            assert!(hits.contains("GetPouchContent:Event0"), "{name}: {hits:?}");
            rows.insert(format!("chest | {name}:Success_"), hits);
        }
        let mut ap: BS<String> = BS::new();
        reach(&mut rf, &mut cache, crate::ap::mailbox::GIVE_FLOW, crate::ap::mailbox::GIVE_ENTRY, &mut BS::new(), &mut ap);
        assert!(ap.contains("GetPouchContent:Event0"), "{ap:?}");
        for (k, v) in &rows {
            eprintln!("{k} -> {v:?}");
        }
        eprintln!("not reached: {open:?}");
        eprintln!("flows the build makes (not in the game files): {made:?}");
        assert!(made.iter().all(|m| m.contains("Cow")), "{made:?}");
        assert_eq!(open, Vec::<String>::new());
    }

    #[test]
    fn the_game_files_give_no_wallet_outside_the_give_routines() {
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
                here += patch(f).unwrap();
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
        assert_eq!(sites, GIVES.len() + PICKUPS.len());
    }

    #[test]
    fn the_hold_is_a_second_and_the_names_cover_each_wallet() {
        assert_eq!(HOLD, 60);
        let all: Vec<String> = names().unwrap();
        for w in WALLETS {
            for x in [w.to_string(), w.replace("Item_", "Pouch_"), w.replace("Item_", "Obj_")] {
                assert!(all.contains(&x), "{x}");
            }
        }
        assert_eq!(crate::progressive::tiers(FAMILY).unwrap().to_vec(), WALLETS.to_vec());
        for t in 0..WALLETS.len() {
            for n in crate::pickup_tiers::names(FAMILY, t).unwrap() {
                assert!(all.contains(&n), "{n}");
            }
        }
        assert!(all.iter().all(|n| n.contains("Wallet_Medium") || n.contains("Wallet_Large")), "{all:?}");
    }
}
