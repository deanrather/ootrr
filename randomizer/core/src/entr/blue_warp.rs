use std::collections::BTreeMap as BM;
use std::path::Path as P;

use crate::bfevfl::{Bf, Case, Cont, Event, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::entr::build::actor;
use crate::entr::data::{Cat, Door};
use crate::entr::layout::Layout;
use crate::entr::pools::{find, DUNGEONS};
use crate::error::{Error as E, Res};
use crate::romfs::Romfs;
use crate::staging::Out;
use crate::start_state::{query_slot, slot};
use crate::story_skip::{BOSS_WARPS, ENDING_FLOW};

pub const PREFIX: &str = "OOTRRBlueWarp";

pub const BOSSES: [&str; 8] = ["Deku Tree", "Dodongos Cavern", "Jabu Jabus Belly", "Forest Temple", "Fire Temple", "Water Temple", "Shadow Temple", "Spirit Temple"];

const AGES: [&str; 2] = ["Child", "Adult"];
const SYSTEM: &str = "GameSystemActor";
const CHANGE: &str = "EventReserveBancChange";
const DATA: &str = "GameDataActor";
const SET_BOOL: &str = "EventTriggerSetGameDataBool";
const STEPS: &str = "ChallengeSystemActor";
const PLAYER: &str = "Player_Event";
const IS_CHILD: &str = "EventQueryPlayerIsChild";
const SCENE: &str = "EventSystemActor";
const TRANSITION: &str = "EventTransition";
const TRUE: u32 = 0x8000_0001;
const NONE: u16 = 0xFFFF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Landing {
    pub banc: String,
    pub start: String,
    pub own: bool,
}

fn pad(dungeon: &str) -> Option<(&'static str, &'static str, bool, bool)> {
    match dungeon {
        "Deku Tree" => Some(("KokiriForest", "WarpFromDekuTree", true, false)),
        "Dodongos Cavern" => Some(("DeathMountainTrail", "DodongoCavernFront", true, true)),
        "Jabu Jabus Belly" => Some(("ZoraFountain", "EntrancePos", true, true)),
        "Fire Temple" => Some(("DeathMountainCrater", "FireBolero", true, true)),
        "Water Temple" => Some(("HyruleField", "WaterSerenade", true, true)),
        "Shadow Temple" => Some(("Graveyard", "ShadowNocturne", true, true)),
        "Spirit Temple" => Some(("DesertColossus", "SpiritRequiem", true, true)),
        _ => None,
    }
}

fn made_for(boss: &str, age: &str) -> bool {
    match boss {
        "Dodongos Cavern" => true,
        "Deku Tree" | "Jabu Jabus Belly" => age == "Child",
        "Forest Temple" => false,
        _ => age == "Adult",
    }
}

pub fn has_arrival(cat: &Cat, spot: usize, age: usize) -> Res<bool> {
    let p: bool = pad(DUNGEONS[spot].name).is_some_and(|p| if age == 0 { p.2 } else { p.3 });
    Ok(p || cat.door(DUNGEONS[spot].back)?.ok_for(&AGES[age].to_ascii_lowercase()))
}

fn doorstep(rf: &mut Romfs, cat: &Cat, spot: usize, age: &str) -> Res<(String, String)> {
    let door: &Door = cat.door(DUNGEONS[spot].back)?;
    let y: Byml = Byml::parse(&rf.read_zs(&door.file)?)?;
    let n: &Node = actor(&y, DUNGEONS[spot].back).ok_or_else(|| E::Msg(format!("blue warp: the exit of {} was not found in {}, the game files differ", DUNGEONS[spot].name, door.file)))?;
    let dy: &Node = n.get("Dynamic").ok_or(E::Format("blue warp: the exit actor has no Dynamic map"))?;
    let s = |k: &str| -> Res<String> {
        dy.get(k)
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string())
            .ok_or_else(|| E::Msg(format!("blue warp: the exit of {} has no {k}", DUNGEONS[spot].name)))
    };
    Ok((s(&format!("NextBancParamName{age}"))?, s("NextStartPosName")?))
}

pub fn landings(rf: &mut Romfs, cat: &Cat, l: &Layout) -> Res<Vec<[Landing; 2]>> {
    l.check()?;
    let mut out: Vec<[Landing; 2]> = Vec::new();
    for boss in BOSSES {
        let j: usize = find(boss).ok_or(E::Format("blue warp: a boss dungeon is not in the pool"))?;
        let spot: usize = l.spot_of(j);
        let mut two: Vec<Landing> = Vec::new();
        for n in 0..AGES.len() {
            let n: usize = if has_arrival(cat, spot, n)? { n } else { 1 - n };
            let age: &str = AGES[n];
            let at: &str = DUNGEONS[spot].name;
            let own: bool = spot == j && made_for(boss, age);
            let (banc, start): (String, String) = match pad(at).filter(|p| if n == 0 { p.2 } else { p.3 }) {
                Some(p) => (format!("{}_{age}", p.0), p.1.to_string()),
                None => doorstep(rf, cat, spot, age)?,
            };
            two.push(Landing { banc, start, own });
        }
        let [child, adult]: [Landing; 2] = two.try_into().map_err(|_| E::Format("blue warp: two ages"))?;
        out.push([child, adult]);
    }
    Ok(out)
}

fn idx(n: usize) -> Res<u16> {
    if n >= NONE as usize {
        return Err(E::Format("blue warp: too many events in the flow"));
    }
    Ok(n as u16)
}

fn push(f: &mut Flow, kind: Kind) -> Res<u16> {
    let at: u16 = idx(f.events.len())?;
    f.events.push(Event { name: format!("{PREFIX}{at}"), kind });
    Ok(at)
}

fn who(f: &Flow, i: usize) -> Option<(&str, &str)> {
    match &f.events.get(i)?.kind {
        Kind::Action { actor, act, .. } => {
            let a = f.actors.get(*actor as usize)?;
            Some((a.name.as_str(), a.actions.get(*act as usize)?.as_str()))
        }
        Kind::Switch { actor, query, .. } => {
            let a = f.actors.get(*actor as usize)?;
            Some((a.name.as_str(), a.queries.get(*query as usize)?.as_str()))
        }
        _ => None,
    }
}

fn text(f: &Flow, i: usize, k: &str) -> Option<String> {
    match f.events.get(i)?.kind.params()?.get(k)? {
        Val::Str(s) => Some(s.clone()),
        _ => None,
    }
}

fn nexts(k: &Kind) -> Vec<u16> {
    match k {
        Kind::Action { next, .. } | Kind::Join { next } | Kind::Sub { next, .. } => vec![*next],
        Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter().map(|c| c.ev).collect(),
        Kind::Fork { join, forks } => forks.iter().copied().chain([*join]).collect(),
        Kind::Label { .. } => Vec::new(),
    }
}

fn reach(f: &Flow, from: u16) -> Vec<usize> {
    let mut seen: Vec<bool> = vec![false; f.events.len()];
    let mut todo: Vec<u16> = vec![from];
    while let Some(i) = todo.pop() {
        let i: usize = i as usize;
        if i >= seen.len() || seen[i] {
            continue;
        }
        seen[i] = true;
        todo.extend(nexts(&f.events[i].kind));
    }
    (0..seen.len()).filter(|&i| seen[i]).collect()
}

fn entry(f: &Flow, name: &str) -> Option<u16> {
    f.entries.iter().find(|e| e.name == name).map(|e| e.main)
}

fn set_flag(f: &mut Flow, flag: &str, next: u16) -> Res<u16> {
    let (a, x): (u16, u16) = slot(f, DATA, SET_BOOL)?;
    let items: Vec<(String, Val)> = vec![
        ("Index".to_string(), Val::Int(-1)),
        ("GameDataName".to_string(), Val::Str(flag.to_string())),
        ("Value".to_string(), Val::Bool(TRUE)),
    ];
    push(f, Kind::Action { next, actor: a, act: x, params: Some(Cont { items }) })
}

fn writes_only(f: &mut Flow, i: u16, then: u16, done: &mut BM<u16, u16>, depth: usize) -> Res<u16> {
    if i == NONE {
        return Ok(then);
    }
    if depth > 128 {
        return Err(E::Format("blue warp: an arrival event does not end, the game files differ"));
    }
    if let Some(x) = done.get(&i) {
        return Ok(*x);
    }
    let kind: Kind = f.events.get(i as usize).ok_or(E::Format("blue warp: an arrival event leads nowhere"))?.kind.clone();
    let named: Option<(String, String)> = who(f, i as usize).map(|(a, b)| (a.to_string(), b.to_string()));
    let actor_name: &str = named.as_ref().map(|n| n.0.as_str()).unwrap_or("");
    let out: u16 = match kind {
        Kind::Action { next, actor, act, params } => {
            let rest: u16 = writes_only(f, next, then, done, depth + 1)?;
            if actor_name == DATA || actor_name == STEPS {
                push(f, Kind::Action { next: rest, actor, act, params })?
            } else if actor_name == SCENE && named.as_ref().is_some_and(|n| n.1 == TRANSITION) {
                let scene: String = text(f, i as usize, "EventName").ok_or(E::Format("blue warp: a scene has no name"))?;
                set_flag(f, &format!("HavePlayedEvent.{scene}"), rest)?
            } else {
                rest
            }
        }
        Kind::Sub { next, params, flow, entry: name } => {
            let rest: u16 = writes_only(f, next, then, done, depth + 1)?;
            if flow.starts_with("Navi") || name.contains("Navi") {
                rest
            } else {
                push(f, Kind::Sub { next: rest, params, flow, entry: name })?
            }
        }
        Kind::Join { next } => writes_only(f, next, then, done, depth + 1)?,
        Kind::Fork { join, .. } => writes_only(f, join, then, done, depth + 1)?,
        Kind::Switch { actor, query, cases, params } => {
            if actor_name != DATA {
                return Err(E::Msg(format!("blue warp: an arrival event branches on {actor_name}, which is not known")));
            }
            let mut copied: Vec<Case> = Vec::new();
            for c in &cases {
                copied.push(Case { val: c.val, ev: writes_only(f, c.ev, then, done, depth + 1)? });
            }
            for val in [0, 1] {
                if !copied.iter().any(|c| c.val == val) {
                    copied.push(Case { val, ev: then });
                }
            }
            copied.sort_by_key(|c| c.val);
            push(f, Kind::Switch { actor, query, cases: copied, params })?
        }
        Kind::SubSwitch { .. } | Kind::Label { .. } => return Err(E::Format("blue warp: an arrival event has a kind that is not known")),
    };
    done.insert(i, out);
    Ok(out)
}

fn retarget(kind: &mut Kind, to: &Landing) -> Res<()> {
    let p: &mut Cont = kind.params_mut().ok_or(E::Format("blue warp: a map change has no parameters"))?;
    for (k, v) in p.items.iter_mut() {
        match k.as_str() {
            "NextBanc" => *v = Val::Str(to.banc.clone()),
            "NextStartPos" => *v = Val::Str(to.start.clone()),
            "BootEventName" if !to.own => *v = Val::Str(String::new()),
            "NextPosition" | "NextStartRot" => *v = Val::Floats(vec![0.0, 0.0, 0.0]),
            _ => {}
        }
    }
    Ok(())
}

fn branch(f: &mut Flow, change: &Kind, boot: &str, to: &Landing) -> Res<u16> {
    let mut k: Kind = change.clone();
    if to.own {
        let own_map: bool = k.params().and_then(|p| p.get("NextBanc")).is_some_and(|v| matches!(v, Val::Str(s) if s.ends_with("_Child") || s.ends_with("_Adult")));
        if own_map {
            if let Some(p) = k.params_mut() {
                for (key, v) in p.items.iter_mut().filter(|x| x.0 == "NextBanc") {
                    let Val::Str(s) = v else { return Err(E::Format("blue warp: a map name is not text")) };
                    let age: &str = if to.banc.ends_with("_Child") { "_Child" } else { "_Adult" };
                    let stem: &str = s.rsplit_once('_').map(|x| x.0).ok_or_else(|| E::Msg(format!("blue warp: {key} has no age")))?;
                    *s = format!("{stem}{age}");
                }
            }
        }
        return push(f, k);
    }
    retarget(&mut k, to)?;
    let at: u16 = push(f, k)?;
    match entry(f, boot) {
        Some(main) => writes_only(f, main, at, &mut BM::new(), 0),
        None if boot.starts_with("Dm_") => set_flag(f, &format!("HavePlayedEvent.{boot}"), at),
        None => Ok(at),
    }
}

pub fn patch(b: &mut Bf, to: &[[Landing; 2]]) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == ENDING_FLOW).ok_or(E::Format("blue warp: the dungeon ending flow is missing"))?;
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(E::Format("blue warp: the dungeon ending flow was already patched"));
    }
    let child: (u16, u16, Option<Cont>) = {
        let (actor, query): (u16, u16) = query_slot(f, PLAYER, IS_CHILD)?;
        let params: Option<Option<Cont>> = f.events.iter().find_map(|e| match &e.kind {
            Kind::Switch { actor: a, query: q, params, .. } if *a == actor && *q == query => Some(params.clone()),
            _ => None,
        });
        (actor, query, params.ok_or(E::Format("blue warp: the flow never asks Link's age, the game files differ"))?)
    };
    let mut jobs: BM<usize, usize> = BM::new();
    for (boss, (_, name)) in BOSS_WARPS.iter().enumerate() {
        let main: u16 = entry(f, name).ok_or_else(|| E::Msg(format!("blue warp: the ending flow has no {name}")))?;
        let mut found: usize = 0;
        for i in reach(f, main) {
            let map: bool = text(f, i, "NextBanc").is_some_and(|s| s.ends_with("_Child") || s.ends_with("_Adult"));
            if who(f, i) == Some((SYSTEM, CHANGE)) && map {
                if jobs.insert(i, boss).is_some_and(|x| x != boss) {
                    return Err(E::Msg(format!("blue warp: two bosses share a map change at {name}")));
                }
                found += 1;
            }
        }
        if found == 0 {
            return Err(E::Msg(format!("blue warp: {name} changes to no map, the game files differ")));
        }
    }
    for (i, boss) in jobs {
        let landing: &[Landing; 2] = to.get(boss).ok_or(E::Format("blue warp: a boss has no landing"))?;
        if BOSSES[boss] == "Dodongos Cavern" && landing.iter().all(|x| x.own) {
            continue;
        }
        let change: Kind = f.events[i].kind.clone();
        let boot: String = text(f, i, "BootEventName").unwrap_or_default();
        let young: u16 = branch(f, &change, &boot, &landing[0])?;
        let old: u16 = branch(f, &change, &boot, &landing[1])?;
        f.events[i] = Event {
            name: format!("{PREFIX}{i}"),
            kind: Kind::Switch { actor: child.0, query: child.1, cases: vec![Case { val: 0, ev: young }, Case { val: 1, ev: old }], params: child.2.clone() },
        };
    }
    declare_subs(f);
    Ok(())
}

fn declare_subs(f: &mut Flow) {
    for n in 0..f.entries.len() {
        let calls: Vec<u16> = reach(f, f.entries[n].main).into_iter().filter(|&i| matches!(f.events[i].kind, Kind::Sub { .. })).map(|i| i as u16).collect();
        let e = &mut f.entries[n];
        for c in calls {
            if f.events[c as usize].name.starts_with(PREFIX) && !e.subs.contains(&c) {
                e.subs.push(c);
            }
        }
    }
}

pub fn stage(rf: &mut Romfs, cat: &Cat, l: &Layout, late: &mut Vec<Out>, work: &P) -> Res<()> {
    let to: Vec<[Landing; 2]> = landings(rf, cat, l)?;
    crate::story_skip::stage_late(rf, late, work, ENDING_FLOW, |b| patch(b, &to))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entr::data::load;
    use crate::story_skip::ENDING_FILE;
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

    fn vanilla(rf: &mut Romfs) -> Bf {
        let mut b: Bf = Bf::parse(&rf.read_zs(ENDING_FILE).unwrap()).unwrap();
        crate::story_skip::patch_endings(&mut b).unwrap();
        crate::story_skip::patch_fire_ending(&mut b).unwrap();
        let gives: Vec<(&str, &[&str], crate::story_skip::SceneGive)> = crate::story_skip::REWARD_SCENES
            .iter()
            .map(|(_, scene, extra)| (*scene, *extra, crate::story_skip::SceneGive { actor: "Item_Bottle".to_string(), family: None, key: None, line: None }))
            .collect();
        crate::story_skip::patch_reward_gives(&mut b, &gives).unwrap();
        b
    }

    fn flow(b: &Bf) -> &Flow {
        b.flows.iter().find(|f| f.name == ENDING_FLOW).unwrap()
    }

    fn lands(f: &Flow, name: &str, age: u32) -> Vec<(String, String)> {
        let mut seen: Vec<bool> = vec![false; f.events.len()];
        let mut todo: Vec<u16> = vec![entry(f, name).unwrap()];
        let mut out: Vec<(String, String)> = Vec::new();
        while let Some(i) = todo.pop() {
            let i: usize = i as usize;
            if i >= seen.len() || seen[i] {
                continue;
            }
            seen[i] = true;
            if who(f, i) == Some((SYSTEM, CHANGE)) {
                let map: String = text(f, i, "NextBanc").unwrap();
                if map != "SageRoom" {
                    out.push((map, text(f, i, "NextStartPos").unwrap()));
                }
            }
            match (&f.events[i].kind, who(f, i)) {
                (Kind::Switch { cases, .. }, Some((PLAYER, IS_CHILD))) => todo.extend(cases.iter().filter(|c| c.val == age).map(|c| c.ev)),
                (k, _) => todo.extend(nexts(k)),
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn start_exists(rf: &mut Romfs, map: &str, start: &str) -> bool {
        fn named(n: &Node, want: &str) -> bool {
            n.get("Name").and_then(|v| v.as_str()) == Some(want)
        }
        let dir: &str = map.rsplit_once('_').unwrap().0;
        ["Static", "Dynamic"].iter().any(|kind| {
            let file: String = format!("Banc/{dir}/{map}_{kind}.bcett.byml.zs");
            let Ok(raw) = rf.read_zs(&file) else { return false };
            let y: Byml = Byml::parse(&raw).unwrap();
            y.root.get("Actors").and_then(|a| a.as_arr()).is_some_and(|l| l.iter().any(|a| named(a, start)))
        })
    }

    fn check(rf: &mut Romfs, l: &Layout) -> Bf {
        let cat: Cat = load().unwrap();
        let to: Vec<[Landing; 2]> = landings(rf, &cat, l).unwrap();
        let mut b: Bf = vanilla(rf);
        patch(&mut b, &to).unwrap();
        assert_eq!(Bf::parse(&b.write().unwrap()).unwrap(), b);
        for (n, (_, name)) in BOSS_WARPS.iter().enumerate() {
            for (a, age) in AGES.iter().enumerate() {
                let got: Vec<(String, String)> = lands(flow(&b), name, a as u32);
                assert!(!got.is_empty(), "{name} {age}");
                let spot: usize = l.spot_of(find(BOSSES[n]).unwrap());
                if has_arrival(&cat, spot, a).unwrap() {
                    for (map, start) in &got {
                        assert!(map.ends_with(&format!("_{age}")), "{name} {age} lands in {map} {l:?}");
                        assert!(start_exists(rf, map, start), "{name} {age}: {map} has no start {start}");
                    }
                }
                if !(BOSSES[n] == "Deku Tree" && to[n][a].own) {
                    assert!(got.iter().all(|x| x.0 == to[n][a].banc), "{name} {age}: {got:?}");
                }
            }
        }
        assert!(patch(&mut b, &to).is_err());
        b
    }

    #[test]
    fn every_blue_warp_keeps_links_age_and_lands_on_a_real_start_with_the_dungeons_in_place() {
        let Some(mut rf) = real() else {
            return;
        };
        check(&mut rf, &Layout::vanilla());
    }

    #[test]
    fn every_blue_warp_keeps_links_age_wherever_its_dungeon_sits() {
        let Some(mut rf) = real() else {
            return;
        };
        let cat: Cat = load().unwrap();
        let mut tried: usize = 0;
        for a in 0..DUNGEONS.len() {
            for b in a + 1..DUNGEONS.len() {
                let mut l: Layout = Layout::vanilla();
                l.at.swap(a, b);
                if crate::entr::layout::ages_fit(&cat, &l).unwrap().is_empty() {
                    check(&mut rf, &l);
                    tried += 1;
                }
            }
        }
        assert!(tried > 20, "{tried}");
    }

    #[test]
    fn a_boss_behind_another_door_lands_outside_that_door() {
        let Some(mut rf) = real() else {
            return;
        };
        let cat: Cat = load().unwrap();
        let mut l: Layout = Layout::vanilla();
        l.at.swap(find("Deku Tree").unwrap(), find("Forest Temple").unwrap());
        let to: Vec<[Landing; 2]> = landings(&mut rf, &cat, &l).unwrap();
        assert_eq!(to[0][1].banc, "SacredForestMeadow_Adult");
        assert_eq!(to[3][1].banc, "KokiriForest_Adult");
        assert_eq!(to[3][0], Landing { banc: "KokiriForest_Child".to_string(), start: "WarpFromDekuTree".to_string(), own: false });
        let b: Bf = check(&mut rf, &l);
        let f: &Flow = flow(&b);
        assert!(f.events.iter().any(|e| e.name.starts_with(PREFIX) && matches!(&e.kind, Kind::Action { params: Some(p), .. } if matches!(p.get("ChallengeName"), Some(Val::Str(s)) if s == "SageOfForest"))));
    }
}
