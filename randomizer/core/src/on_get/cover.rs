use std::collections::{BTreeMap as BM, BTreeSet as BS};

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Event, Flow, Kind, Val};
use crate::romfs::Romfs;

const POUCH: &str = "PouchSystemActor";
const ADD: &str = "EventTriggerAddOrRemovePouchContent";
const SHOP: &str = "DisplayShop";
const STARTER: &str = "Npc_EventStarter";
const GRANT: &str = "EventTriggerGetItemAtShop";
const PICKUPS: [(&str, &str); 2] = [("Dm_SY_Get", "Event35"), ("Dm_SY_Get_WithoutPlayer", "Event0")];
const SWAPS: [(&str, &str); 2] = [("GetInEventInBottle", "GetInEvent_NotTurn_NotNpc"), ("GetInEventInEquipedBottle", "GetInEvent_NotTurn")];

pub type Patch<'a> = &'a dyn Fn(&mut Bf);
pub type Shelf<'a> = &'a dyn Fn(Vec<u8>) -> Vec<u8>;

#[derive(Default)]
pub struct Report {
    pub checked: BM<String, usize>,
    pub missed: Vec<String>,
}

impl Report {
    fn row(&mut self, path: &str, label: String, reached: bool) {
        *self.checked.entry(path.to_string()).or_default() += 1;
        if !reached {
            self.missed.push(format!("{path}: {label}"));
        }
    }
}

struct Files<'a> {
    rf: &'a mut Romfs,
    item: &'a str,
    shelf: Shelf<'a>,
    patch: Patch<'a>,
    literals: BM<String, BS<usize>>,
    cache: BM<String, Option<(Flow, BS<String>)>>,
    stops: BS<(String, u16)>,
}

impl Files<'_> {
    fn load(&mut self, name: &str) -> Option<(Flow, BS<String>)> {
        if !self.cache.contains_key(name) {
            let raw: Option<Vec<u8>> = self.rf.read_zs(&format!("Event/EventFlow/{name}.bfevfl.zs")).ok();
            let made: Option<(Flow, BS<String>)> = raw.and_then(|r| {
                let r: Vec<u8> = if name == SHOP { (self.shelf)(r) } else { r };
                let was: BS<String> = Bf::parse(&r).ok()?.flows.iter().find(|f| f.name == name)?.events.iter().map(|e| e.name.clone()).collect();
                let mut b: Bf = Bf::parse(&r).ok()?;
                for f in b.flows.iter_mut().filter(|f| f.name == name) {
                    for &i in self.literals.get(name).into_iter().flatten() {
                        if let Some(c) = f.events[i].kind.params_mut() {
                            for (k, v) in c.items.iter_mut() {
                                if k == "Actor" {
                                    *v = Val::Str(self.item.to_string());
                                }
                            }
                        }
                    }
                }
                (self.patch)(&mut b);
                Some((b.flows.into_iter().find(|f| f.name == name)?, was))
            });
            self.cache.insert(name.to_string(), made);
        }
        self.cache.get(name).cloned().flatten()
    }

    fn walk(&mut self, name: &str, from: u16, seen: &mut BS<(String, u16)>) -> bool {
        let Some((f, was)) = self.load(name) else {
            return false;
        };
        let mut todo: Vec<u16> = vec![from];
        let mut added: bool = false;
        while let Some(i) = todo.pop() {
            if i == NONE || self.stops.contains(&(name.to_string(), i)) || !seen.insert((name.to_string(), i)) {
                continue;
            }
            let ev: &Event = &f.events[usize::from(i)];
            added |= !was.contains(&ev.name);
            match &ev.kind {
                Kind::Action { next, .. } | Kind::Join { next } => todo.push(*next),
                Kind::Switch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
                Kind::Fork { join, forks } => {
                    todo.push(*join);
                    todo.extend(forks.iter().copied());
                }
                Kind::Sub { next, flow, entry, .. } => {
                    todo.push(*next);
                    let into: String = if flow.is_empty() { name.to_string() } else { flow.clone() };
                    added |= self.enter(&into, entry, seen);
                }
                Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
                Kind::Label { .. } => {}
            }
        }
        added
    }

    fn enter(&mut self, name: &str, entry: &str, seen: &mut BS<(String, u16)>) -> bool {
        let main: Option<u16> = self.load(name).and_then(|(f, _)| f.entries.iter().find(|e| e.name == entry).map(|e| e.main));
        main.is_some_and(|m| self.walk(name, m, seen))
    }

    fn after(&mut self, name: &str, i: usize) -> bool {
        let next: Option<u16> = self.load(name).and_then(|(f, _)| match &f.events[i].kind {
            Kind::Action { next, .. } | Kind::Sub { next, .. } => Some(*next),
            _ => None,
        });
        next.is_some_and(|n| self.walk(name, n, &mut BS::new()))
    }

    fn around(&mut self, name: &str, i: usize) -> bool {
        if self.after(name, i) {
            return true;
        }
        let new: Vec<u16> = self.load(name).map(|(f, was)| (0..f.events.len()).filter(|&j| !was.contains(&f.events[j].name)).filter_map(|j| u16::try_from(j).ok()).collect()).unwrap_or_default();
        new.into_iter().any(|j| {
            let mut seen: BS<(String, u16)> = BS::new();
            self.walk(name, j, &mut seen);
            seen.iter().any(|(n, k)| n == name && usize::from(*k) == i)
        })
    }
}

fn action(f: &Flow, i: usize, who: &str, what: &str) -> bool {
    matches!(&f.events[i].kind, Kind::Action { actor, act, .. } if f.actors[usize::from(*actor)].name == who && f.actors[usize::from(*actor)].actions[usize::from(*act)] == what)
}

pub fn run(rf: &mut Romfs, item: &str, shelf: Shelf, patch: Patch) -> Report {
    let d: crate::flow_locations::Data = crate::flow_locations::Data::load().unwrap();
    let mut out: Report = Report::default();
    let mut grants: Vec<(String, String, String, usize)> = Vec::new();
    let mut literals: BM<String, BS<usize>> = BM::new();
    for l in d.locs.iter().filter(|l| l.supported || l.gated) {
        for e in l.edits.iter().filter(|e| e.role == "grant") {
            let file: String = e.file.trim_end_matches(".bfevfl").to_string();
            let own: Option<Flow> = rf.read_zs(&format!("Event/EventFlow/{file}.bfevfl.zs")).ok().and_then(|r| Bf::parse(&r).ok()).and_then(|b| b.flows.into_iter().find(|f| f.name == file));
            let Some(f) = own else {
                out.row("not checked (the build makes this flow)", l.name.clone(), true);
                continue;
            };
            if action(&f, e.index, POUCH, ADD) {
                literals.entry(file.clone()).or_default().insert(e.index);
            }
            grants.push((l.name.clone(), l.cat.to_string(), file, e.index));
        }
    }
    let mut files: Files = Files { rf, item, shelf, patch, literals, cache: BM::new(), stops: BS::new() };
    for (name, cat, file, i) in grants {
        let Some((f, _)) = files.load(&file) else {
            out.row("flow location", name, false);
            continue;
        };
        let ev: Event = f.events[i].clone();
        let (path, reached): (&str, bool) = match &ev.kind {
            Kind::Sub { flow, entry, .. } => {
                let into: String = if flow.is_empty() { file.clone() } else { flow.clone() };
                let entry: &str = SWAPS.iter().find(|s| into == "Dm_SY_GetInEvent" && s.0 == entry).map(|s| s.1).unwrap_or(entry);
                ("flow location, give routine", files.enter(&into, entry, &mut BS::new()))
            }
            _ if action(&f, i, POUCH, ADD) => ("flow location, own script add", files.around(&file, i)),
            _ => ("flow location, other", false),
        };
        out.row(path, format!("{name} ({cat})"), reached);
    }
    for path in [crate::progressive::SMALL_FLOW, crate::progressive::BIG_FLOW] {
        let name: &str = path.trim_start_matches("Event/EventFlow/").trim_end_matches(".bfevfl.zs");
        let reached: bool = files.enter(name, "Success_", &mut BS::new());
        out.row("chest", name.to_string(), reached);
    }
    let reached: bool = files.enter(crate::ap::mailbox::GIVE_FLOW, crate::ap::mailbox::GIVE_ENTRY, &mut BS::new());
    out.row("Archipelago delivery", crate::ap::mailbox::GIVE_ENTRY.to_string(), reached);
    for (flow, event) in PICKUPS {
        let i: usize = files.load(flow).unwrap().0.events.iter().position(|e| e.name == event).unwrap();
        let reached: bool = files.around(flow, i);
        out.row("ground pickup", format!("{flow}:{event}"), reached);
    }
    let shop: Flow = files.load(SHOP).unwrap().0;
    let grants: Vec<usize> = (0..shop.events.len()).filter(|&i| action(&shop, i, STARTER, GRANT)).collect();
    let again: Vec<usize> = (0..shop.events.len()).filter(|&i| i == crate::shelf::hook::CHECK.0 || matches!(&shop.events[i].kind, Kind::Sub { entry, .. } if entry == crate::shelf::hook::PICK)).collect();
    for &i in &grants {
        files.stops = grants.iter().chain(&again).filter(|&&j| j != i).filter_map(|&j| u16::try_from(j).ok()).map(|j| (SHOP.to_string(), j)).collect();
        let reached: bool = files.after(SHOP, i);
        out.row("shop shelf", shop.events[i].name.clone(), reached);
    }
    out
}

pub fn print(title: &str, r: &Report) {
    println!("== {title}");
    for (k, n) in &r.checked {
        let missed: usize = r.missed.iter().filter(|m| m.starts_with(&format!("{k}: "))).count();
        if k.starts_with("not checked") {
            println!("(these locations were not walked)");
            println!("{k}: {n}");
        } else {
            println!("{k}: {} of {n} reached", n - missed);
        }
    }
    for m in &r.missed {
        println!("MISSED {m}");
    }
    println!("not checked: bottled items, silent repeat pickups, adds made by the game's code");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_hook_reaches_every_get_path() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return;
        }
        let sold: Vec<crate::shelf::hook::Sold> = vec![crate::shelf::hook::Sold { family: "Wallet", counters: vec!["W.0".to_string()], copies: 1 }];
        let plain = |r: Vec<u8>| r;
        let tiered = |r: Vec<u8>| crate::shelf::hook::edit(&r, &sold, &[], false).unwrap();
        for h in crate::on_get::ALL {
            let patch = |b: &mut Bf| {
                for f in b.flows.iter_mut() {
                    crate::on_get::sites::patch(f, h).unwrap();
                }
            };
            let shelves: [(&str, Shelf); 2] = [("plain shelf", &plain), ("tiered shelf", &tiered)];
            for (kind, shelf) in shelves {
                let mut rf: Romfs = Romfs::open(&p).unwrap();
                let r: Report = run(&mut rf, h.items[0], shelf, &patch);
                print(&format!("{} ({kind})", h.title), &r);
                assert_eq!(r.missed, Vec::<String>::new());
                assert_eq!((r.checked["shop shelf"], r.checked["ground pickup"], r.checked["chest"]), (3, 2, 2));
                assert!(r.checked["flow location, give routine"] > 90 && r.checked["flow location, own script add"] == 2);
            }
        }
    }

    #[test]
    fn the_check_reports_every_path_when_nothing_is_hooked() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            return;
        }
        let sold: Vec<crate::shelf::hook::Sold> = vec![crate::shelf::hook::Sold { family: "Wallet", counters: vec!["W.0".to_string()], copies: 1 }];
        let plain = |r: Vec<u8>| r;
        let tiered = |r: Vec<u8>| crate::shelf::hook::edit(&r, &sold, &[], false).unwrap();
        let shelves: [Shelf; 2] = [&plain, &tiered];
        for shelf in shelves {
            let mut rf: Romfs = Romfs::open(&p).unwrap();
            let r: Report = run(&mut rf, "Item_Wallet_Medium", shelf, &|_| {});
            let all: usize = r.checked.iter().filter(|(k, _)| !k.starts_with("not checked")).map(|(_, n)| *n).sum();
            assert_eq!(r.missed.len(), all, "{:?}", r.missed);
        }
    }

    #[test]
    fn hooking_one_shelf_purchase_leaves_the_other_two_reported() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            return;
        }
        let sold: Vec<crate::shelf::hook::Sold> = vec![crate::shelf::hook::Sold { family: "Wallet", counters: vec!["W.0".to_string()], copies: 1 }];
        let plain = |r: Vec<u8>| r;
        let tiered = |r: Vec<u8>| crate::shelf::hook::edit(&r, &sold, &[], false).unwrap();
        let shelves: [Shelf; 2] = [&plain, &tiered];
        for shelf in shelves {
            for one in ["Event123", "Event281", "Event543"] {
                let patch = |b: &mut Bf| {
                    let f: &mut Flow = &mut b.flows[0];
                    if f.name != SHOP {
                        return;
                    }
                    let i: usize = f.events.iter().position(|e| e.name == one).unwrap();
                    let at: u16 = u16::try_from(f.events.len()).unwrap();
                    let Kind::Action { next: then, .. } = f.events[i].kind.clone() else { panic!("{one}") };
                    let other: usize = (0..f.events.len()).find(|&j| matches!(f.events[j].kind, Kind::Action { .. }) && !action(f, j, STARTER, GRANT)).unwrap();
                    let mut added: Event = f.events[other].clone();
                    added.name = "Added".to_string();
                    if let Kind::Action { next, .. } = &mut added.kind {
                        *next = then;
                    }
                    f.events.push(added);
                    if let Kind::Action { next, .. } = &mut f.events[i].kind {
                        *next = at;
                    }
                };
                let mut rf: Romfs = Romfs::open(&p).unwrap();
                let r: Report = run(&mut rf, "Item_Wallet_Medium", shelf, &patch);
                let missed: Vec<&String> = r.missed.iter().filter(|m| m.starts_with("shop shelf")).collect();
                assert_eq!(missed.len(), 2, "{one}: {missed:?}");
                assert!(!missed.iter().any(|m| m.ends_with(one)), "{one}: {missed:?}");
            }
        }
    }
}
