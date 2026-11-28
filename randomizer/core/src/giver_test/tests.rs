use std::collections::BTreeSet as BS;
use std::path::{Path as PP, PathBuf as PB};

use super::*;
use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Flow, Kind, Val};
use crate::romfs::Romfs;
use crate::slice::generate::{generate_data, GenOpts, Generated};

fn wallets() -> Settings {
    Settings::default()
}

fn on(item: &str) -> GenOpts {
    GenOpts { giver: Some(item.to_string()), ..GenOpts::default() }
}

fn real() -> Option<PB> {
    let p: PB = ootrr_game::test_romfs::path();
    if p.join("Pack").join("ZsDic.pack.zs").is_file() {
        Some(p)
    } else {
        eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
        None
    }
}

fn flow_of(rf: &mut Romfs, root: Option<&PP>, name: &str) -> Option<Flow> {
    let path: String = format!("Event/EventFlow/{name}.bfevfl.zs");
    let own: Option<PB> = root.map(|r| path.split('/').fold(r.to_path_buf(), |a, s| a.join(s))).filter(|p| p.is_file());
    let raw: Vec<u8> = match own {
        Some(p) => rf.zs.decompress(&std::fs::read(p).ok()?).ok()?,
        None => rf.read_zs(&path).ok()?,
    };
    Bf::parse(&raw).ok()?.flows.into_iter().find(|f| f.name == name)
}

fn reach(rf: &mut Romfs, root: Option<&PP>, name: &str, entry: &str) -> BS<(String, String)> {
    walk(rf, root, name, Some(entry), 0)
}

fn walk(rf: &mut Romfs, root: Option<&PP>, name: &str, entry: Option<&str>, at: u16) -> BS<(String, String)> {
    let mut out: BS<(String, String)> = BS::new();
    let mut seen: BS<(String, u16)> = BS::new();
    let mut calls: Vec<(String, Option<String>)> = vec![(name.to_string(), entry.map(|e| e.to_string()))];
    let mut done: BS<(String, Option<String>)> = BS::new();
    while let Some((fl, en)) = calls.pop() {
        if !done.insert((fl.clone(), en.clone())) {
            continue;
        }
        let Some(f) = flow_of(rf, root, &fl) else { continue };
        let m: u16 = match &en {
            Some(e) => match f.entries.iter().find(|x| &x.name == e).map(|x| x.main) {
                Some(m) => m,
                None => continue,
            },
            None => at,
        };
        let mut todo: Vec<u16> = vec![m];
        while let Some(i) = todo.pop() {
            if i == NONE || !seen.insert((fl.clone(), i)) {
                continue;
            }
            let e: &crate::bfevfl::Event = &f.events[usize::from(i)];
            out.insert((fl.clone(), e.name.clone()));
            match &e.kind {
                Kind::Action { next, .. } | Kind::Join { next } => todo.push(*next),
                Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
                Kind::Fork { join, forks } => {
                    todo.push(*join);
                    todo.extend(forks.iter().copied());
                }
                Kind::Sub { next, flow, entry, .. } => {
                    todo.push(*next);
                    calls.push((if flow.is_empty() { fl.clone() } else { flow.clone() }, Some(entry.clone())));
                }
                Kind::Label { .. } => {}
            }
        }
    }
    out
}

#[test]
fn the_flag_with_no_item_or_an_unknown_item_is_refused() {
    let d: Data = Data::load().unwrap();
    let e: String = bare_flag().to_string();
    assert!(e.contains("needs the item to test"), "{e}");
    let e: String = generate_data(&d, &Settings::default(), "giver test", &on("Not An Item")).err().unwrap().to_string();
    assert!(e.contains("no item is named"), "{e}");
}

#[test]
fn the_test_item_maps_to_its_starting_item_choice() {
    assert_eq!(start_opt("Progressive Wallet", &[]), Some("adult_wallet"));
    assert_eq!(start_opt("Progressive Wallet", &["adult_wallet".to_string()]), Some("giant_wallet"));
    assert_eq!(start_opt("Progressive Wallet", &["giant_wallet".to_string()]), None);
    assert_eq!(start_opt("Kokiri Sword", &[]), Some(SWORD));
}

#[test]
fn the_flag_changes_only_the_settings_it_lists_and_says_which() {
    let d: Data = Data::load().unwrap();
    let m: Mode = Mode::new(&d, &wallets(), "Progressive Wallet").unwrap();
    for (id, ok, to) in NEEDS {
        let v: String = shown(m.settings.get(id).unwrap());
        assert!(ok.contains(&v.as_str()), "{id} is {v}, want {to}");
    }
    let kit: &[String] = m.settings.items("starting_items").unwrap();
    assert!(kit.iter().any(|k| k == SWORD) && kit.iter().any(|k| k == "adult_wallet"), "{kit:?}");
    let base: Settings = wallets();
    for (id, v) in m.settings.values().iter() {
        let listed: bool = NEEDS.iter().any(|n| n.0 == id.as_str()) || id.as_str() == "starting_items";
        if !listed {
            assert_eq!(base.get(id), Some(v), "{id} changed but is not listed");
        }
    }
    assert!(m.notes[0].starts_with(LINE) && m.notes[0].contains("shuffle_pots off to all"), "{}", m.notes[0]);
}

#[test]
fn the_flag_places_the_named_item_at_the_givers_and_the_custom_items_on_the_kokiri_shelves() {
    let d: Data = Data::load().unwrap();
    let g: Generated = generate_data(&d, &wallets(), "giver test", &on("Progressive Wallet")).unwrap();
    println!("{}", g.report);
    let m: &Mark = g.seed_file.giver_test.as_ref().unwrap();
    assert_eq!(m.item, "Progressive Wallet");
    let placed: Vec<&str> = GIVERS.iter().map(|g| g.0).filter(|l| g.report.contains(&format!("{l} Progressive Wallet"))).collect();
    for l in &placed {
        assert_eq!(g.seed_file.placements.get(*l).map(|s| s.as_str()), Some("Progressive Wallet"), "{l}");
    }
    assert_eq!(placed.len(), GIVERS.len(), "placed {placed:?}");
    assert!(g.report.contains(NOT_BEATABLE));
    for (loc, item) in SHELVES {
        assert_eq!(g.seed_file.placements.get(loc).map(|s| s.as_str()), Some(item), "{loc}");
    }
    let same: Settings = Mode::new(&d, &wallets(), "Progressive Wallet").unwrap().settings;
    let mut shelves: Vec<Entry> = SHELVES.iter().map(|(l, i)| Entry { location: l.to_string(), item: i.to_string() }).collect();
    shelves.push(Entry { location: GIVERS[5].0.to_string(), item: "Progressive Wallet".to_string() });
    let plain: Generated = generate_data(&d, &same, "giver test", &GenOpts { plando: shelves, ..GenOpts::default() }).unwrap();
    let count = |p: &BM<String, String>| -> BM<String, i64> {
        let mut out: BM<String, i64> = BM::new();
        for v in p.values() {
            *out.entry(v.clone()).or_insert(0) += 1;
        }
        out
    };
    let (with, without): (BM<String, i64>, BM<String, i64>) = (count(&g.seed_file.placements), count(&plain.seed_file.placements));
    let names: BS<&String> = with.keys().chain(without.keys()).collect();
    for n in names {
        let moved: i64 = with.get(n).copied().unwrap_or(0) - without.get(n).copied().unwrap_or(0);
        assert_eq!(moved, m.copies.get(n).copied().unwrap_or(0), "{n}: the flag changed its count by {moved}, the seed file says {:?}", m.copies.get(n));
    }
    assert!(m.copies.get("Progressive Wallet").copied().unwrap_or(0) > 0, "{:?}", m.copies);
    println!("copies {:?}", m.copies);
    let back: SeedFile = SeedFile::parse(&g.seed_file.to_text()).unwrap();
    assert_eq!(back.giver_test.as_ref(), Some(m));
    let again: Generated = generate_data(&d, &wallets(), "giver test", &on("Progressive Wallet")).unwrap();
    assert_eq!(again.seed_file.to_text(), g.seed_file.to_text());
}

#[test]
fn without_the_flag_the_seed_file_has_no_giver_field_and_the_same_checksum() {
    let d: Data = Data::load().unwrap();
    let g: Generated = generate_data(&d, &wallets(), "giver test off", &GenOpts::default()).unwrap();
    assert!(g.seed_file.giver_test.is_none());
    let t: String = g.seed_file.to_text();
    assert!(!t.contains("giver_test"));
    let mut marked: SeedFile = g.seed_file.clone();
    marked.giver_test = Some(Mark { item: "Progressive Wallet".to_string(), copies: BM::new() });
    assert_ne!(marked.checksum(), g.seed_file.checksum());
    marked.giver_test = None;
    assert_eq!(marked.to_text(), t);
    assert_eq!(counted(&g.seed_file), g.seed_file);
}

#[test]
fn a_seed_file_whose_giver_copies_add_items_from_nothing_is_refused() {
    let d: Data = Data::load().unwrap();
    let g: Generated = generate_data(&d, &wallets(), "giver test copies", &on("Progressive Wallet")).unwrap();
    let good: String = g.seed_file.to_text();
    assert!(SeedFile::parse(&good).is_ok());
    let bad_marks: [BM<String, i64>; 2] = [
        [("Progressive Wallet".to_string(), 1)].into_iter().collect(),
        [("Progressive Wallet".to_string(), 1), ("Bombchus (10)".to_string(), 1), ("Rupee (1)".to_string(), -2)].into_iter().collect(),
    ];
    for c in bad_marks {
        let mut s: SeedFile = g.seed_file.clone();
        s.giver_test = Some(Mark { item: "Progressive Wallet".to_string(), copies: c.clone() });
        let e: String = SeedFile::parse(&s.to_text()).err().map(|x| x.to_string()).unwrap_or_default();
        assert!(e.contains("giver_test.copies"), "{c:?}: {e}");
    }
    let shelf: BM<String, i64> = [("Progressive Wallet".to_string(), 1), ("Minuet of Forest".to_string(), 1), ("Rupee (1)".to_string(), -2)].into_iter().collect();
    let mut s: SeedFile = g.seed_file.clone();
    s.giver_test = Some(Mark { item: "Progressive Wallet".to_string(), copies: shelf });
    assert!(SeedFile::parse(&s.to_text()).is_ok(), "a copy of a shelf item is refused");
}

#[test]
fn the_kokiri_talk_calls_the_story_give_in_every_entry_that_reaches_the_greeting() {
    let Some(p) = real() else {
        return;
    };
    let d: Data = Data::load().unwrap();
    let mut rf: Romfs = Romfs::open(&p).unwrap();
    let mut f: Flow = flow_of(&mut rf, None, story::NPC).unwrap();
    let was: Flow = f.clone();
    let (actor, family): (String, Option<&str>) = story::actor_of(&d, "Progressive Wallet").unwrap();
    assert_eq!((actor.as_str(), family), ("Item_Wallet_Medium", Some("Wallet")));
    story::edit(&mut f, &actor, family).unwrap();
    assert!(story::edit(&mut f.clone(), &actor, family).is_err(), "a second edit is refused");
    let gives: Vec<usize> = (0..f.events.len()).filter(|&i| matches!(&f.events[i].kind, Kind::Sub { flow, entry, .. } if flow == story::GIVE.0 && entry == story::GIVE.1)).collect();
    assert_eq!(gives.len(), 2, "one give per wallet tier");
    let greet: usize = was.events.iter().position(|e| e.name == story::AFTER).unwrap();
    let mut callers: Vec<String> = Vec::new();
    for e in f.entries.iter() {
        let from_greet: bool = story::reaches(&was, e.main, greet);
        for &i in &gives {
            assert_eq!(story::reaches(&f, e.main, i), from_greet, "entry {} and give {i}", e.name);
            assert_eq!(e.subs.contains(&(i as u16)), from_greet, "entry {} lists give {i}: {}", e.name, e.subs.contains(&(i as u16)));
        }
        if from_greet {
            callers.push(e.name.clone());
        }
    }
    println!("entries reaching the greeting: {callers:?}");
    assert!(callers.contains(&story::ENTRY.to_string()));
    let mut actors: Vec<String> = Vec::new();
    for &i in &gives {
        if let Some(Val::Str(a)) = f.events[i].kind.params().and_then(|c| c.get(story::PARAM)) {
            actors.push(a.clone());
        }
    }
    assert_eq!(actors, ["Item_Wallet_Medium", "Item_Wallet_Large"]);
    for (i, e) in was.events.iter().enumerate() {
        if e.name != story::AFTER {
            assert_eq!(&f.events[i], e, "event {i}");
        }
    }
    let bf: Bf = Bf::parse(&rf.read_zs(story::PATH).unwrap()).unwrap();
    let mut out: Bf = bf.clone();
    let k: usize = out.flows.iter().position(|x| x.name == story::NPC).unwrap();
    out.flows[k] = f.clone();
    assert_eq!(Bf::parse(&out.write().unwrap()).unwrap().flows[k], f);
    let into: BS<(String, String)> = reach(&mut rf, None, story::GIVE.0, story::GIVE.1);
    assert!(into.contains(&(story::GIVE.0.to_string(), "Event124".to_string())), "GetInEvent_Goron does not reach Event124");
}

#[test]
fn a_build_with_the_flag_puts_the_item_at_each_placed_giver_and_loads_its_flows() {
    let Some(p) = real() else {
        return;
    };
    let d: Data = Data::load().unwrap();
    let g: Generated = generate_data(&d, &wallets(), "giver test build", &on("Progressive Wallet")).unwrap();
    let load: PB = PB::from(env!("CARGO_MANIFEST_DIR")).join("../../target/giver_test_mods");
    let _ = std::fs::remove_dir_all(&load);
    let b: crate::slice::edit::Built = crate::slice::edit::build_for(&g.seed_file, &p, &load, "ootrr-giver-test", crate::modpack::Host::Eden).unwrap();
    println!("{}", b.report);
    let root: PB = b.root.join("romfs");
    let mut rf: Romfs = Romfs::open(&p).unwrap();
    let kid: Flow = flow_of(&mut rf, Some(&root), story::NPC).unwrap();
    let castle: Vec<&crate::bfevfl::Event> = kid.events.iter().filter(|e| matches!(e.kind.params().and_then(|c| c.get("GameDataName")), Some(Val::Str(s)) if s == "Goto_HyruleCastle_IsCompleted_Exp")).collect();
    assert!(!castle.is_empty());
    for e in castle {
        let Kind::Switch { cases, .. } = &e.kind else { panic!("{} is not a switch", e.name) };
        assert!(cases.windows(2).all(|w| w[0].ev == w[1].ev), "{}: the castle story still picks another talk: {cases:?}", e.name);
    }
    let story_give: bool = reach(&mut rf, Some(&root), story::NPC, story::ENTRY).contains(&(story::GIVE.0.to_string(), "Event124".to_string()));
    assert!(story_give, "the Kokiri talk does not reach the story give");
    assert!(crate::flow_extra::analyzed::worked(&mut rf, &root).unwrap().is_none(), "an edited flow calls a flow its event row does not load");
    for c in b.chests.iter().filter(|c| GIVERS.iter().any(|g| g.0 == c.name)) {
        println!("chest {}: {} {}", c.name, c.new_item, c.new_actor);
        assert_eq!(c.new_item, "Progressive Wallet", "{}", c.name);
    }
    let mut lines: Vec<&str> = b.chests.iter().filter(|c| GIVERS.iter().any(|g| g.0 == c.name)).map(|c| c.name.as_str()).collect();
    lines.sort_unstable();
    assert_eq!(lines, ["KF Kokiri Sword Chest", "KF Midos Top Left Chest", "KF Sarias House Recovery Heart 1"]);
    assert!(b.report.contains(&format!("{LINE}: story give")), "{}", b.report);
    assert!(b.report.contains(NOT_BEATABLE), "{}", b.report);
    assert!(b.report.contains(&format!("{LINE}: a Business Scrub")), "{}", b.report);
    let village: crate::byml::Byml = crate::byml::Byml::parse(&rf.zs.decompress(&std::fs::read(root.join(scrub::MAP)).unwrap()).unwrap()).unwrap();
    let scrubs: usize = village.root.get("Actors").unwrap().as_arr().unwrap().iter().filter(|a| a.get("Gyaml").and_then(|v| v.as_str()) == Some(scrub::BODY)).count();
    assert_eq!(scrubs, 2, "the village map holds its own scrub and the test mode's");
    let plain: crate::byml::Byml = rf.read_byml(scrub::MAP).unwrap();
    assert_eq!(plain.root.get("Actors").unwrap().as_arr().unwrap().iter().filter(|a| a.get("Gyaml").and_then(|v| v.as_str()) == Some(scrub::BODY)).count(), 1);
    let dropped: bool = b.report.lines().any(|l| l.contains("Obj_Wallet_Medium") && l.contains("KF House of Twins Pot 1") && l.contains("KF Sarias House Recovery Heart 1"));
    assert!(dropped, "the pot and the heart do not hold the wallet pickup");
    assert!(b.report.lines().any(|l| l.contains("KF Shop Right Side Top Left | Progressive Wallet")), "the wallet shelf is missing");
    let scrub: Flow = flow_of(&mut rf, Some(&root), "Enemy_Scrub_Business_Common").unwrap();
    let sales: Vec<usize> = (0..scrub.events.len()).filter(|&i| matches!(scrub.events[i].kind.params().and_then(|c| c.get("ActorName")), Some(Val::Str(s)) if s.starts_with("Item_Wallet_"))).collect();
    println!("scrub sale events giving a wallet: {:?}", sales.iter().map(|&i| scrub.events[i].name.clone()).collect::<Vec<String>>());
    assert!(!sales.is_empty(), "no scrub sale gives the wallet");
    for i in sales {
        let got: BS<(String, String)> = walk(&mut rf, Some(&root), "Enemy_Scrub_Business_Common", None, i as u16);
        assert!(got.contains(&("Dm_SY_GetInEvent".to_string(), "Event430".to_string())), "sale {i} does not reach Event430");
    }
}
