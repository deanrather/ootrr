use std::collections::BTreeSet as BS;
use std::path::Path as P;

use crate::bfevfl::{Bf, Case, Cont, Event, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::entr::build::{actor, pick_like};
use crate::entr::data::{load, Cat, Door};
use crate::entr::layout::Layout;
use crate::entr::pools::{find, DUNGEONS};
use crate::error::{Error as E, Res};
use crate::romfs::Romfs;
use crate::settings::seed::SeedFile;
use crate::staging::Out;
use crate::start_state::{query_slot, slot};

pub const FLOW: &str = "Get_StoneOfWater";
pub const EAT: &str = "StoneOfWater_JabuJabuEat";
pub const MEET: &str = "StoneOfWater_MeetRuto";
pub const JABU: &str = "Jabu Jabus Belly";
pub const JABU_MAP: &str = "InsideJabuJabuBelly";
pub const MET: &str = "Meet_Ruto_First";
pub const FED: &str = "GaveFishToJabJab";
pub const KING_MOVED: &str = "HavePlayedEvent.Dm_Zora_0010";
pub const ARRIVED: &str = "Step09";
pub const PREFIX: &str = "OOTRREntrJabu";

const SYSTEM: &str = "GameSystemActor";
const CHANGE: &str = "EventReserveBancChange";
const DATA: &str = "GameDataActor";
const SET_BOOL: &str = "EventTriggerSetGameDataBool";
const IS_BOOL: &str = "EventQueryIsGameDataBool";
const CHALLENGE: &str = "ChallengeSystemActor";
const PROGRESS: &str = "EventExecuteProgressChallengeStep";
const TRUE: u32 = 0x8000_0001;
const NONE: u16 = 0xFFFF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dest {
    pub banc: String,
    pub start: String,
    pub boot: String,
    pub first: Option<(String, String)>,
}

pub fn dest(rf: &mut Romfs, cat: &Cat, l: &Layout) -> Res<Option<Dest>> {
    l.check()?;
    let j: usize = find(JABU).ok_or(E::Format("entrance story: Jabu Jabu's Belly is not in the dungeon pool"))?;
    let d: usize = l.at[j];
    if d == j {
        return Ok(None);
    }
    let mouth: u64 = *DUNGEONS[j]
        .doors
        .iter()
        .find(|h| cat.door(**h).is_ok_and(|x| x.age == "child"))
        .ok_or(E::Format("entrance story: Jabu Jabu's mouth has no child entrance"))?;
    let src: u64 = pick_like(cat, mouth, DUNGEONS[d].doors)?;
    let door: &Door = cat.door(src)?;
    let y: Byml = Byml::parse(&rf.read_zs(&door.file)?)?;
    let n: &Node = actor(&y, src).ok_or_else(|| E::Msg(format!("entrance {src} was not found in {}, the game files differ", door.file)))?;
    let dy: &Node = n.get("Dynamic").ok_or(E::Format("entrance story: the entrance actor has no Dynamic map"))?;
    let s = |k: &str| -> Option<String> { dy.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(|v| v.to_string()) };
    let banc: String = s("NextBancParamNameChild").ok_or_else(|| E::Msg(format!("entrance {src} has no child destination")))?;
    let start: String = s("NextStartPosName").ok_or_else(|| E::Msg(format!("entrance {src} has no start position")))?;
    let first: Option<(String, String)> = match (s("InitEventName"), s("InitEventFlag")) {
        (Some(a), Some(b)) => Some((a, b)),
        (None, _) => None,
        (Some(a), None) => return Err(E::Msg(format!("entrance {src} plays {a} without a flag, the game files differ"))),
    };
    Ok(Some(Dest { banc, start, boot: s("NextEventName").unwrap_or_default(), first }))
}

fn idx(n: usize) -> Res<u16> {
    if n >= NONE as usize {
        return Err(E::Format("entrance story: too many events in the flow"));
    }
    Ok(n as u16)
}

fn push(f: &mut Flow, e: Event) -> Res<u16> {
    let at: u16 = idx(f.events.len())?;
    f.events.push(e);
    Ok(at)
}

fn nexts(k: &Kind) -> Vec<u16> {
    match k {
        Kind::Action { next, .. } | Kind::Join { next } | Kind::Sub { next, .. } => vec![*next],
        Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter().map(|c| c.ev).collect(),
        Kind::Fork { join, forks } => forks.iter().copied().chain([*join]).collect(),
        Kind::Label { .. } => Vec::new(),
    }
}

fn reach(f: &Flow, entry: &str) -> Res<Vec<usize>> {
    let e = f.entries.iter().find(|e| e.name == entry).ok_or_else(|| E::Msg(format!("entrance story: {} has no entry {entry}, the game files differ", f.name)))?;
    let mut seen: BS<usize> = BS::new();
    let mut todo: Vec<u16> = [e.main].into_iter().chain(e.subs.iter().copied()).collect();
    while let Some(i) = todo.pop() {
        let i: usize = i as usize;
        if i >= f.events.len() || !seen.insert(i) {
            continue;
        }
        todo.extend(nexts(&f.events[i].kind));
    }
    Ok(seen.into_iter().collect())
}

fn is(f: &Flow, i: usize, who: &str, what: &str) -> bool {
    match &f.events[i].kind {
        Kind::Action { actor, act, .. } => f.actors.get(*actor as usize).is_some_and(|a| a.name == who && a.actions.get(*act as usize).is_some_and(|x| x == what)),
        _ => false,
    }
}

fn text<'a>(e: &'a Event, k: &str) -> Option<&'a str> {
    match e.kind.params()?.get(k)? {
        Val::Str(s) => Some(s.as_str()),
        _ => None,
    }
}

fn set_text(e: &mut Event, k: &str, v: &str) -> Res<()> {
    let p: &mut Cont = e.kind.params_mut().ok_or(E::Format("entrance story: the map change has no parameters"))?;
    let x: &mut (String, Val) = p.items.iter_mut().find(|x| x.0 == k).ok_or_else(|| E::Msg(format!("entrance story: the map change has no {k}")))?;
    x.1 = Val::Str(v.to_string());
    Ok(())
}

fn repoint(f: &mut Flow, from: u16, to: u16) -> Res<usize> {
    if f.entries.iter().any(|e| e.main == from || e.subs.contains(&from)) {
        return Err(E::Format("entrance story: an entry starts at the map change"));
    }
    let mut n: usize = 0;
    for e in f.events.iter_mut().filter(|e| !e.name.starts_with(PREFIX)) {
        let mut hit = |x: &mut u16| {
            if *x == from {
                *x = to;
                n += 1;
            }
        };
        match &mut e.kind {
            Kind::Action { next, .. } | Kind::Join { next } | Kind::Sub { next, .. } => hit(next),
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter_mut().for_each(|c| hit(&mut c.ev)),
            Kind::Fork { join, forks } => {
                forks.iter_mut().for_each(&mut hit);
                hit(join);
            }
            Kind::Label { .. } => {}
        }
    }
    Ok(n)
}

fn set_bool(f: &mut Flow, name: &str, flag: &str, next: u16) -> Res<u16> {
    let (actor, act): (u16, u16) = slot(f, DATA, SET_BOOL)?;
    let items: Vec<(String, Val)> = vec![
        ("Index".to_string(), Val::Int(-1)),
        ("Value".to_string(), Val::Bool(TRUE)),
        ("GameDataName".to_string(), Val::Str(flag.to_string())),
    ];
    push(f, Event { name: name.to_string(), kind: Kind::Action { next, actor, act, params: Some(Cont { items }) } })
}

fn redirect(f: &mut Flow, d: &Dest) -> Res<()> {
    let hits: Vec<usize> = reach(f, EAT)?.into_iter().filter(|&i| is(f, i, SYSTEM, CHANGE) && text(&f.events[i], "NextBanc") == Some(JABU_MAP)).collect();
    let [change] = hits[..] else {
        return Err(E::Msg(format!("entrance story: {EAT} changes to {JABU_MAP} {} times, expected once", hits.len())));
    };
    let old: u16 = idx(change)?;
    for (k, v) in [("NextBanc", &d.banc), ("NextStartPos", &d.start), ("EventStarterName", &d.start), ("BootEventName", &d.boot)] {
        set_text(&mut f.events[change], k, v)?;
    }
    let mut head: u16 = old;
    if let Some((boot, flag)) = &d.first {
        let mut e: Event = f.events[change].clone();
        e.name = format!("{PREFIX}First");
        set_text(&mut e, "BootEventName", boot)?;
        let first: u16 = push(f, e)?;
        let (actor, query): (u16, u16) = query_slot(f, DATA, IS_BOOL)?;
        let items: Vec<(String, Val)> = vec![("Index".to_string(), Val::Int(-1)), ("GameDataName".to_string(), Val::Str(flag.clone()))];
        let cases: Vec<Case> = vec![Case { val: 0, ev: old }, Case { val: 1, ev: first }];
        head = push(f, Event { name: format!("{PREFIX}Seen"), kind: Kind::Switch { actor, query, cases, params: Some(Cont { items }) } })?;
    }
    let (actor, act): (u16, u16) = slot(f, CHALLENGE, PROGRESS)?;
    let items: Vec<(String, Val)> = vec![
        ("ChallengeName".to_string(), Val::Str(FLOW.to_string())),
        ("IsWaitEnd".to_string(), Val::Bool(TRUE)),
        ("StepName".to_string(), Val::Str(ARRIVED.to_string())),
        ("IsIgnoreChallengeProgressUI".to_string(), Val::Bool(TRUE)),
        ("IsIgnoreCompletedChallenge".to_string(), Val::Bool(TRUE)),
    ];
    let step: u16 = push(f, Event { name: format!("{PREFIX}Arrived"), kind: Kind::Action { next: head, actor, act, params: Some(Cont { items }) } })?;
    if repoint(f, old, step)? == 0 {
        return Err(E::Format("entrance story: nothing leads to the swallow's map change"));
    }
    Ok(())
}

fn mark(f: &mut Flow) -> Res<()> {
    let hits: Vec<usize> = reach(f, MEET)?.into_iter().filter(|&i| is(f, i, DATA, SET_BOOL) && text(&f.events[i], "GameDataName") == Some(MET)).collect();
    let [met] = hits[..] else {
        return Err(E::Msg(format!("entrance story: {MEET} sets {MET} {} times, expected once", hits.len())));
    };
    let Kind::Action { next, .. } = f.events[met].kind else {
        return Err(E::Format("entrance story: the Ruto meeting flag is not an action"));
    };
    let king: u16 = set_bool(f, &format!("{PREFIX}King"), KING_MOVED, next)?;
    let fed: u16 = set_bool(f, &format!("{PREFIX}Fed"), FED, king)?;
    if let Kind::Action { next, .. } = &mut f.events[met].kind {
        *next = fed;
    }
    Ok(())
}

pub fn patch(b: &mut Bf, d: &Dest) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == FLOW).ok_or(E::Format("entrance story: Get_StoneOfWater has no flow of its name"))?;
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(E::Format("entrance story: Get_StoneOfWater already follows the shuffle"));
    }
    redirect(f, d)?;
    mark(f)
}

pub fn stage(rf: &mut Romfs, seed: &SeedFile, late: &mut Vec<Out>, work: &P) -> Res<()> {
    if seed.entrances.is_empty() {
        return Ok(());
    }
    let l: Layout = Layout::from_map(&seed.entrances)?;
    let cat: Cat = load()?;
    let Some(d) = dest(rf, &cat, &l)? else {
        return Ok(());
    };
    crate::story_skip::stage_late(rf, late, work, FLOW, |b| patch(b, &d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entr::build::{moves, vals, Move};
    use crate::entr::layout::ages_fit;
    use crate::entr::mode;
    use crate::settings::{Settings, Value};
    use crate::slice::data::Data;
    use crate::slice::generate::{generate_data, GenOpts, Generated};
    use std::path::PathBuf as PB;

    fn real() -> Option<Romfs> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(Romfs::open(&p).unwrap())
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn water(rf: &mut Romfs) -> Bf {
        Bf::parse(&rf.read_zs(&format!("Event/EventFlow/{FLOW}.bfevfl.zs")).unwrap()).unwrap()
    }

    fn flow(b: &Bf) -> &Flow {
        b.flows.iter().find(|f| f.name == FLOW).unwrap()
    }

    fn changes(f: &Flow, entry: &str) -> Vec<(String, String, String)> {
        let t = |e: &Event, k: &str| text(e, k).unwrap_or("").to_string();
        reach(f, entry)
            .unwrap()
            .into_iter()
            .filter(|&i| is(f, i, SYSTEM, CHANGE))
            .map(|i| (t(&f.events[i], "NextBanc"), t(&f.events[i], "NextStartPos"), t(&f.events[i], "BootEventName")))
            .collect()
    }

    fn writes(f: &Flow, entry: &str) -> Vec<String> {
        reach(f, entry).unwrap().into_iter().filter(|&i| is(f, i, DATA, SET_BOOL)).filter_map(|i| text(&f.events[i], "GameDataName").map(|s| s.to_string())).collect()
    }

    fn swapped(d: usize) -> Layout {
        let mut l: Layout = Layout::vanilla();
        l.at.swap(find(JABU).unwrap(), d);
        l
    }

    #[test]
    fn a_layout_that_keeps_jabu_in_place_needs_no_story_edit() {
        let Some(mut rf) = real() else {
            return;
        };
        let c: Cat = load().unwrap();
        assert_eq!(dest(&mut rf, &c, &Layout::vanilla()).unwrap(), None);
        let mut l: Layout = Layout::vanilla();
        l.at.swap(find("Fire Temple").unwrap(), find("Water Temple").unwrap());
        assert_eq!(dest(&mut rf, &c, &l).unwrap(), None);
    }

    #[test]
    fn the_swallow_leads_where_the_mouth_leads_for_every_dungeon_that_fits() {
        let Some(mut rf) = real() else {
            return;
        };
        let c: Cat = load().unwrap();
        let j: usize = find(JABU).unwrap();
        let mouth: u64 = DUNGEONS[j].doors[0];
        let vanilla: Bf = water(&mut rf);
        let first: Vec<(String, String, String)> = changes(flow(&vanilla), EAT);
        assert_eq!(first, vec![(JABU_MAP.to_string(), "InsideJabuJabuBelly_FromZoraFountain".to_string(), "Get_StoneOfWater_ArriveJabuJabuBelly".to_string())]);
        let mut tried: usize = 0;
        for d in (0..DUNGEONS.len()).filter(|d| *d != j) {
            let l: Layout = swapped(d);
            if !ages_fit(&c, &l).unwrap().is_empty() {
                continue;
            }
            tried += 1;
            let x: Dest = dest(&mut rf, &c, &l).unwrap().unwrap();
            let mv: Vec<Move> = moves(&c, &l).unwrap();
            let src: u64 = mv.iter().find(|m| m.0 == mouth).unwrap().1;
            let file: String = c.door(src).unwrap().file.clone();
            let y: Byml = Byml::parse(&rf.read_zs(&file).unwrap()).unwrap();
            let v = vals(actor(&y, src).unwrap());
            let got = |k: &str| v.iter().find(|x| x.0 == k).and_then(|x| x.1.as_ref()).and_then(|n| n.as_str()).unwrap_or("").to_string();
            assert_eq!((x.banc.clone(), x.start.clone(), x.boot.clone()), (got("NextBancParamNameChild"), got("NextStartPosName"), got("NextEventName")), "{}", DUNGEONS[d].name);
            assert_eq!(x.banc, DUNGEONS[d].map);
            assert!(mv.contains(&(DUNGEONS[d].back, DUNGEONS[j].back)), "{} leads back to the fountain", DUNGEONS[d].name);
            let mut b: Bf = vanilla.clone();
            patch(&mut b, &x).unwrap();
            let back: Bf = Bf::parse(&b.write().unwrap()).unwrap();
            assert_eq!(back, b);
            let f: &Flow = flow(&b);
            let all: Vec<(String, String, String)> = changes(f, EAT);
            assert!(all.iter().all(|(m, s, _)| *m == x.banc && *s == x.start), "{all:?}");
            let boots: BS<String> = all.iter().map(|a| a.2.clone()).collect();
            let want: BS<String> = [Some(x.boot.clone()), x.first.as_ref().map(|a| a.0.clone())].into_iter().flatten().collect();
            assert_eq!(boots, want, "{}", DUNGEONS[d].name);
            assert_eq!(all.len(), 1 + usize::from(x.first.is_some()));
            let steps: Vec<usize> = reach(f, EAT).unwrap().into_iter().filter(|&i| is(f, i, CHALLENGE, PROGRESS) && text(&f.events[i], "StepName") == Some(ARRIVED)).collect();
            assert_eq!(steps.len(), 1);
            assert!(writes(f, EAT).contains(&FED.to_string()));
            let met: Vec<String> = writes(f, MEET);
            assert!(met.contains(&FED.to_string()) && met.contains(&KING_MOVED.to_string()), "{met:?}");
            assert!(patch(&mut b, &x).is_err());
        }
        assert!(tried >= 2, "{tried}");
    }

    #[test]
    fn the_deku_tree_behind_the_mouth_plays_its_first_arrival_until_it_has_played() {
        let Some(mut rf) = real() else {
            return;
        };
        let c: Cat = load().unwrap();
        let x: Dest = dest(&mut rf, &c, &swapped(find("Deku Tree").unwrap())).unwrap().unwrap();
        assert_eq!(x.first, Some(("InsideTheDekuTree_Arrive".to_string(), "HavePlayedEvent.Dm_Deku_0000".to_string())));
        let mut b: Bf = water(&mut rf);
        patch(&mut b, &x).unwrap();
        let f: &Flow = flow(&b);
        let seen: &Event = f.events.iter().find(|e| e.name == format!("{PREFIX}Seen")).unwrap();
        assert_eq!(text(seen, "GameDataName"), Some("HavePlayedEvent.Dm_Deku_0000"));
        let Kind::Switch { cases, .. } = &seen.kind else { panic!("not a switch") };
        let boot = |ev: u16| text(&f.events[ev as usize], "BootEventName").unwrap().to_string();
        let got: Vec<(u32, String)> = cases.iter().map(|k| (k.val, boot(k.ev))).collect();
        assert_eq!(got, vec![(0, x.boot.clone()), (1, "InsideTheDekuTree_Arrive".to_string())]);
    }

    fn seed_with(pairs: &[(&str, &str)], text: &str) -> Generated {
        let mut s: Settings = Settings::default();
        for (k, v) in pairs {
            s.set(k, Value::s(v)).unwrap();
        }
        generate_data(&Data::load().unwrap(), &s, text, &GenOpts::default()).unwrap()
    }

    #[test]
    fn the_build_step_stages_nothing_with_the_shuffle_off_and_the_swallow_with_it_on() {
        let Some(mut rf) = real() else {
            return;
        };
        let work: &P = P::new("no such work folder");
        let off: Generated = seed_with(&[], "story off");
        let mut late: Vec<Out> = Vec::new();
        stage(&mut rf, &off.seed_file, &mut late, work).unwrap();
        assert!(late.is_empty());
        let j: usize = find(JABU).unwrap();
        let on: Generated = (1..12)
            .map(|k| seed_with(&[(mode::SETTING, "dungeons")], &format!("story{k}")))
            .find(|g| Layout::from_map(&g.seed_file.entrances).is_ok_and(|l| l.at[j] != j))
            .unwrap();
        stage(&mut rf, &on.seed_file, &mut late, work).unwrap();
        let paths: Vec<&str> = late.iter().map(|o| o.path.as_str()).collect();
        assert_eq!(paths, vec![format!("Event/EventFlow/{FLOW}.bfevfl.zs").as_str()]);
        let b: Bf = Bf::parse(&rf.zs.decompress(&late[0].z).unwrap()).unwrap();
        let l: Layout = Layout::from_map(&on.seed_file.entrances).unwrap();
        let want: &str = DUNGEONS[l.at[j]].map;
        assert!(changes(flow(&b), EAT).iter().all(|(m, ..)| m == want));
    }
}
