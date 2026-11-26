use std::collections::{BTreeMap as BM, HashMap as HM};
use std::path::Path as P;

use crate::bfevfl::{Actor, Bf, Case, Cont, Event, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::entr::build::actor;
use crate::entr::data::{Cat, Door};
use crate::entr::layout::Layout;
use crate::entr::pools::{find, DUNGEONS};
use crate::event_chests::resolve::{table, Row};
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
const CLOCK: &str = "EventWorldManagerController";
const PRELOAD: &str = "EventTriggerBancRequestBeforehandLoad";
pub const SCENE_WARPS: [(&str, &str, usize); 1] = [("Dm_InsideJabu_Jabu_0050", "Get_StoneOfWater_Complete", 2)];
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
            if actor_name == DATA || actor_name == STEPS || actor_name == CLOCK {
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
        Kind::Fork { join, forks } => {
            let mut head: u16 = writes_only(f, join, then, done, depth + 1)?;
            for x in forks.iter().rev() {
                let mut seen: BM<u16, u16> = done.clone();
                seen.insert(join, head);
                head = writes_only(f, *x, head, &mut seen, depth + 1)?;
            }
            head
        }
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

fn arrival_writes(f: &mut Flow, rows: &HM<String, Row>, boot: &str, then: u16) -> Res<u16> {
    if boot.is_empty() {
        return Ok(then);
    }
    let row: &Row = rows.get(boot).ok_or_else(|| E::Msg(format!("blue warp: the arrival event {boot} is not in the event table")))?;
    if row.stem == ENDING_FLOW {
        let main: u16 = entry(f, &row.entry).ok_or_else(|| E::Msg(format!("blue warp: the ending flow has no {}", row.entry)))?;
        return writes_only(f, main, then, &mut BM::new(), 0);
    }
    if row.stem.starts_with("Dm_") {
        return set_flag(f, &format!("HavePlayedEvent.{boot}"), then);
    }
    Err(E::Msg(format!("blue warp: the arrival event {boot} is in {}, which is not known", row.stem)))
}

fn own_change(change: &Kind, to: &Landing) -> Res<Kind> {
    let mut k: Kind = change.clone();
    if let Some(p) = k.params_mut() {
        for (key, v) in p.items.iter_mut().filter(|x| x.0 == "NextBanc") {
            let Val::Str(s) = v else { return Err(E::Format("blue warp: a map name is not text")) };
            let age: &str = if to.banc.ends_with("_Child") { "_Child" } else { "_Adult" };
            let stem: &str = s.rsplit_once('_').map(|x| x.0).ok_or_else(|| E::Msg(format!("blue warp: {key} has no age")))?;
            *s = format!("{stem}{age}");
        }
    }
    Ok(k)
}

fn preload_path(banc: &str) -> Res<String> {
    let dir: &str = banc.rsplit_once('_').map(|x| x.0).ok_or(E::Format("blue warp: a map has no age"))?;
    Ok(format!("Work/Banc/Scene/{dir}/{banc}.bcett.json"))
}

fn change_for(f: &mut Flow, change: &Kind, preload: Option<&Kind>, to: &Landing) -> Res<u16> {
    let mut k: Kind = if to.own { own_change(change, to)? } else { change.clone() };
    if !to.own {
        retarget(&mut k, to)?;
    }
    let banc: Option<String> = k.params().and_then(|p| p.get("NextBanc")).and_then(|v| match v {
        Val::Str(s) => Some(s.clone()),
        _ => None,
    });
    let banc: String = banc.ok_or(E::Format("blue warp: a map change names no map"))?;
    let at: u16 = push(f, k)?;
    let Some(p) = preload else {
        return Ok(at);
    };
    let mut load: Kind = p.clone();
    let path: String = preload_path(&banc)?;
    let items: &mut Cont = load.params_mut().ok_or(E::Format("blue warp: a preload has no parameters"))?;
    let x: &mut (String, Val) = items.items.iter_mut().find(|x| x.0 == "BancPath").ok_or(E::Format("blue warp: a preload names no map"))?;
    x.1 = Val::Str(path);
    let Kind::Action { next, .. } = &mut load else {
        return Err(E::Format("blue warp: a preload is not an action"));
    };
    *next = at;
    push(f, load)
}

fn repoint(f: &mut Flow, from: u16, to: u16) {
    for e in f.events.iter_mut() {
        let hit = |x: &mut u16| {
            if *x == from {
                *x = to;
            }
        };
        match &mut e.kind {
            Kind::Action { next, .. } | Kind::Join { next } | Kind::Sub { next, .. } => hit(next),
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter_mut().for_each(|c| hit(&mut c.ev)),
            Kind::Fork { join, forks } => {
                forks.iter_mut().for_each(hit);
                hit(join);
            }
            Kind::Label { .. } => {}
        }
    }
    for e in f.entries.iter_mut().filter(|e| e.main == from) {
        e.main = to;
    }
}

fn take_preload(f: &mut Flow, change: usize) -> Res<Option<Kind>> {
    let at: u16 = idx(change)?;
    let found: Vec<usize> = (0..f.events.len())
        .filter(|&i| who(f, i) == Some((SYSTEM, PRELOAD)) && matches!(f.events[i].kind, Kind::Action { next, .. } if next == at))
        .collect();
    let mut kept: Option<Kind> = None;
    for i in found {
        kept = Some(f.events[i].kind.clone());
        repoint(f, idx(i)?, at);
    }
    Ok(kept)
}

#[derive(Clone)]
pub struct Ask {
    actor: Actor,
    params: Option<Cont>,
}

pub fn ask(b: &Bf) -> Res<Ask> {
    let f: &Flow = b.flows.iter().find(|f| f.name == ENDING_FLOW).ok_or(E::Format("blue warp: the dungeon ending flow is missing"))?;
    for e in &f.events {
        if let Kind::Switch { actor, query, params, .. } = &e.kind {
            let a: &Actor = f.actors.get(*actor as usize).ok_or(E::Format("blue warp: a switch has no actor"))?;
            if a.name == PLAYER && a.queries.get(*query as usize).is_some_and(|q| q == IS_CHILD) {
                return Ok(Ask { actor: a.clone(), params: params.clone() });
            }
        }
    }
    Err(E::Format("blue warp: the flow never asks Link's age, the game files differ"))
}

fn age_switch(f: &mut Flow, how: &Ask, young: u16, old: u16) -> Res<Kind> {
    if !f.actors.iter().any(|a| a.name == PLAYER && a.sub.is_empty()) {
        let mut a: Actor = how.actor.clone();
        a.actions.clear();
        a.queries.clear();
        f.actors.push(a);
    }
    let (actor, query): (u16, u16) = query_slot(f, PLAYER, IS_CHILD)?;
    Ok(Kind::Switch { actor, query, cases: vec![Case { val: 0, ev: young }, Case { val: 1, ev: old }], params: how.params.clone() })
}

fn fixed_changes(f: &Flow, among: impl IntoIterator<Item = usize>) -> Vec<usize> {
    among
        .into_iter()
        .filter(|&i| who(f, i) == Some((SYSTEM, CHANGE)) && text(f, i, "NextBanc").is_some_and(|s| s.ends_with("_Child") || s.ends_with("_Adult")))
        .collect()
}

fn untouched(f: &Flow) -> Res<()> {
    if f.events.iter().any(|e| e.name.starts_with(PREFIX)) {
        return Err(E::Msg(format!("blue warp: {} was already patched", f.name)));
    }
    Ok(())
}

pub fn patch(b: &mut Bf, to: &[[Landing; 2]], rows: &HM<String, Row>) -> Res<()> {
    let how: Ask = ask(b)?;
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == ENDING_FLOW).ok_or(E::Format("blue warp: the dungeon ending flow is missing"))?;
    untouched(f)?;
    let mut jobs: BM<usize, usize> = BM::new();
    for (boss, (_, name)) in BOSS_WARPS.iter().enumerate() {
        let main: u16 = entry(f, name).ok_or_else(|| E::Msg(format!("blue warp: the ending flow has no {name}")))?;
        let found: Vec<usize> = fixed_changes(f, reach(f, main));
        if found.is_empty() {
            return Err(E::Msg(format!("blue warp: {name} changes to no map, the game files differ")));
        }
        for i in found {
            if jobs.insert(i, boss).is_some_and(|x| x != boss) {
                return Err(E::Msg(format!("blue warp: two bosses share a map change at {name}")));
            }
        }
    }
    for (scene, boot, boss) in SCENE_WARPS {
        let landing: &[Landing; 2] = to.get(boss).ok_or(E::Format("blue warp: a boss has no landing"))?;
        let hits: Vec<usize> = (0..f.events.len())
            .filter(|&i| who(f, i) == Some((SCENE, TRANSITION)) && text(f, i, "EventName").as_deref() == Some(scene))
            .collect();
        if hits.is_empty() {
            return Err(E::Msg(format!("blue warp: no ending plays {scene}, the game files differ")));
        }
        for i in hits {
            let moved: u16 = push(f, f.events[i].kind.clone())?;
            let mut heads: [u16; 2] = [moved, moved];
            for (head, land) in heads.iter_mut().zip(landing.iter()) {
                if !land.own {
                    *head = arrival_writes(f, rows, boot, moved)?;
                }
            }
            let kind: Kind = age_switch(f, &how, heads[0], heads[1])?;
            f.events[i] = Event { name: format!("{PREFIX}{i}"), kind };
        }
    }
    for (i, boss) in jobs {
        let landing: &[Landing; 2] = to.get(boss).ok_or(E::Format("blue warp: a boss has no landing"))?;
        if BOSSES[boss] == "Dodongos Cavern" && landing.iter().all(|x| x.own) {
            continue;
        }
        let preload: Option<Kind> = take_preload(f, i)?;
        let change: Kind = f.events[i].kind.clone();
        let boot: String = text(f, i, "BootEventName").unwrap_or_default();
        let mut heads: [u16; 2] = [0, 0];
        for (head, land) in heads.iter_mut().zip(landing.iter()) {
            let at: u16 = change_for(f, &change, preload.as_ref(), land)?;
            *head = if land.own { at } else { arrival_writes(f, rows, &boot, at)? };
        }
        let kind: Kind = age_switch(f, &how, heads[0], heads[1])?;
        f.events[i] = Event { name: format!("{PREFIX}{i}"), kind };
    }
    declare_subs(f);
    Ok(())
}

pub fn patch_scene(b: &mut Bf, scene: &str, to: &[Landing; 2], how: &Ask) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == scene).ok_or_else(|| E::Msg(format!("blue warp: no flow {scene}")))?;
    untouched(f)?;
    let found: Vec<usize> = fixed_changes(f, 0..f.events.len());
    if found.is_empty() {
        return Err(E::Msg(format!("blue warp: {scene} changes to no map, the game files differ")));
    }
    for i in found {
        let preload: Option<Kind> = take_preload(f, i)?;
        let change: Kind = f.events[i].kind.clone();
        let young: u16 = change_for(f, &change, preload.as_ref(), &to[0])?;
        let old: u16 = change_for(f, &change, preload.as_ref(), &to[1])?;
        let kind: Kind = age_switch(f, how, young, old)?;
        f.events[i] = Event { name: format!("{PREFIX}{i}"), kind };
    }
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
    let rows: HM<String, Row> = table(rf)?;
    let mut how: Option<Ask> = None;
    crate::story_skip::stage_late(rf, late, work, ENDING_FLOW, |b| {
        how = Some(ask(b)?);
        patch(b, &to, &rows)
    })?;
    let how: Ask = how.ok_or(E::Format("blue warp: the ending flow was not staged"))?;
    for (scene, _, boss) in SCENE_WARPS {
        crate::story_skip::stage_late(rf, late, work, scene, |b| patch_scene(b, scene, &to[boss], &how))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entr::data::load;
    use crate::story_skip::ENDING_FILE;
    use std::collections::BTreeSet as BS;
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

    fn read(rf: &mut Romfs, name: &str) -> Bf {
        Bf::parse(&rf.read_zs(&format!("Event/EventFlow/{name}.bfevfl.zs")).unwrap()).unwrap()
    }

    fn steps_done(f: &Flow, name: &str, age: u32) -> BS<String> {
        let mut seen: Vec<bool> = vec![false; f.events.len()];
        let mut todo: Vec<u16> = vec![entry(f, name).unwrap()];
        let mut out: BS<String> = BS::new();
        while let Some(i) = todo.pop() {
            let i: usize = i as usize;
            if i >= seen.len() || seen[i] {
                continue;
            }
            seen[i] = true;
            if text(f, i, "StepName").as_deref() == Some("Complete") {
                out.insert(text(f, i, "ChallengeName").unwrap());
            }
            match (&f.events[i].kind, who(f, i)) {
                (Kind::Switch { cases, .. }, Some((PLAYER, IS_CHILD))) => todo.extend(cases.iter().filter(|c| c.val == age).map(|c| c.ev)),
                (Kind::Sub { next, flow, entry: sub, .. }, _) => {
                    todo.push(*next);
                    if flow.is_empty() {
                        todo.extend(entry(f, sub));
                    }
                }
                (k, _) => todo.extend(nexts(k)),
            }
        }
        out
    }

    #[test]
    fn every_dungeon_still_counts_as_done_when_its_arrival_scene_is_left_out() {
        let Some(mut rf) = real() else {
            return;
        };
        let cat: Cat = load().unwrap();
        let rows: HM<String, Row> = table(&mut rf).unwrap();
        let quests: [&str; 8] = ["Get_StoneOfForest", "Get_StoneOfFire", "Get_StoneOfWater", "SageOfForest", "SageOfFire", "SageOfWater", "SageOfShadow", "SageOfSpirit"];
        let l: Layout = Layout::vanilla();
        let mut to: Vec<[Landing; 2]> = landings(&mut rf, &cat, &l).unwrap();
        for x in to.iter_mut().flatten() {
            x.own = false;
        }
        let mut b: Bf = vanilla(&mut rf);
        patch(&mut b, &to, &rows).unwrap();
        for (n, (_, name)) in BOSS_WARPS.iter().enumerate() {
            for a in 0..2 {
                let got: BS<String> = steps_done(flow(&b), name, a);
                assert!(got.contains(quests[n]), "{name} {a}: {got:?}");
            }
        }
    }

    #[test]
    fn no_other_scene_a_blue_warp_plays_holds_a_map_change_of_its_own() {
        let Some(mut rf) = real() else {
            return;
        };
        let b: Bf = vanilla(&mut rf);
        let f: &Flow = flow(&b);
        let mut scenes: BS<String> = BS::new();
        for (_, name) in BOSS_WARPS {
            for i in reach(f, entry(f, name).unwrap()) {
                if who(f, i) == Some((SCENE, TRANSITION)) {
                    scenes.insert(text(f, i, "EventName").unwrap());
                }
            }
        }
        let mut holding: BS<String> = BS::new();
        for scene in &scenes {
            let Ok(raw) = rf.read_zs(&format!("Event/EventFlow/{scene}.bfevfl.zs")) else { continue };
            let s: Bf = Bf::parse(&raw).unwrap();
            if s.flows.iter().any(|g| !fixed_changes(g, 0..g.events.len()).is_empty()) {
                holding.insert(scene.clone());
            }
        }
        let known: BS<String> = SCENE_WARPS.iter().map(|x| x.0.to_string()).collect();
        assert_eq!(holding, known);
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
        let rows: HM<String, Row> = table(rf).unwrap();
        patch(&mut b, &to, &rows).unwrap();
        assert_eq!(Bf::parse(&b.write().unwrap()).unwrap(), b);
        let how: Ask = ask(&b).unwrap();
        for (scene, _, boss) in SCENE_WARPS {
            let mut s: Bf = read(rf, scene);
            patch_scene(&mut s, scene, &to[boss], &how).unwrap();
            assert_eq!(Bf::parse(&s.write().unwrap()).unwrap(), s);
            let g: &Flow = s.flows.iter().find(|f| f.name == scene).unwrap();
            assert!(patch_scene(&mut s.clone(), scene, &to[boss], &how).is_err());
            for i in fixed_changes(g, 0..g.events.len()) {
                let map: String = text(g, i, "NextBanc").unwrap();
                assert!(to[boss].iter().any(|x| x.banc == map), "{scene}: {map}");
            }
        }
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
        assert!(patch(&mut b, &to, &rows).is_err());
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
