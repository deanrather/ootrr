use super::flows::{action, cont, finish, flow, marked, push, query, redirect};
use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Case, Cont, Flow, Kind, Val};
use crate::error::{Error, Res};

pub const FILE: &str = "Event/EventFlow/DmF_SY_FallDown.bfevfl.zs";
pub const FLOW: &str = "DmF_SY_FallDown";
pub const PLAYER: &str = "Player_Event";
pub const ABYSS: &str = "EventPlayerAbyss";
pub const LIFE: &str = "EventQueryCurrentLife";
pub const HEAL: &str = "EventTriggerHealLife";
pub const NO_DAMAGE: &str = "InvalidDamage";
pub const VALUE: &str = "Value";
pub const MARK: &str = "OOTRRNearDeath";
pub const FLOOR: i32 = 4;
pub const TOP: i32 = 80;
pub const TRUE: u32 = 0x8000_0001;
pub const LESS: u32 = 0;
pub const EQUAL: u32 = 1;
pub const MORE: u32 = 2;
pub const NOTE: &str = "Falling into a pit, lava or quicksand leaves Link at one heart instead of costing one. With one heart or less left, a fall costs a heart as usual, so it can still end the game.";

fn is_abyss(f: &Flow, k: &Kind) -> bool {
    let Kind::Action { actor, act, .. } = k else {
        return false;
    };
    f.actors.get(*actor as usize).is_some_and(|a| a.name == PLAYER && a.actions.get(*act as usize).is_some_and(|s| s == ABYSS))
}

fn life_check(f: &mut Flow, name: String, value: i32, cases: [u16; 3]) -> Res<u16> {
    let (qa, q): (u16, u16) = query(f, PLAYER, LIFE)?;
    let list: Vec<Case> = [LESS, EQUAL, MORE].iter().zip(cases.iter()).filter(|(_, ev)| **ev != NONE).map(|(v, ev)| Case { val: *v, ev: *ev }).collect();
    push(f, name, Kind::Switch { actor: qa, query: q, cases: list, params: cont(vec![(VALUE, Val::Int(value))]) })
}

fn ladder(f: &mut Flow, lo: i32, hi: i32, after: u16) -> Res<u16> {
    if lo > hi {
        return Ok(after);
    }
    let mid: i32 = lo + (hi - lo) / 2;
    let below: u16 = ladder(f, lo, mid - 1, after)?;
    let above: u16 = ladder(f, mid + 1, hi, after)?;
    let (a, n): (u16, u16) = action(f, PLAYER, HEAL)?;
    let take: u16 = push(f, format!("{MARK}Take{mid}"), Kind::Action { next: after, actor: a, act: n, params: cont(vec![(VALUE, Val::Int(FLOOR - mid))]) })?;
    life_check(f, format!("{MARK}Life{mid}"), mid, [below, take, above])
}

fn spare(params: Option<Cont>) -> Cont {
    let mut c: Cont = params.unwrap_or_default();
    match c.items.iter_mut().find(|(k, _)| k == NO_DAMAGE) {
        Some((_, v)) => *v = Val::Bool(TRUE),
        None => c.items.push((NO_DAMAGE.to_string(), Val::Bool(TRUE))),
    }
    c
}

pub fn near_death(raw: &[u8]) -> Res<Vec<u8>> {
    let mut b: Bf = Bf::parse(raw)?;
    let f: &mut Flow = flow(&mut b, FLOW)?;
    if marked(f, MARK) {
        return Err(Error::Format("difficulty: the fall flow is already edited"));
    }
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| is_abyss(f, &f.events[i].kind)).collect();
    if hits.len() != 1 {
        return Err(Error::Msg(format!("difficulty: the fall flow runs {ABYSS} {} times, expected once", hits.len())));
    }
    let at: u16 = hits[0] as u16;
    let (next, actor, act, params): (u16, u16, u16, Option<Cont>) = match &f.events[at as usize].kind {
        Kind::Action { next, actor, act, params } => (*next, *actor, *act, params.clone()),
        _ => return Err(Error::Format("difficulty: the fall flow's abyss event is not an action")),
    };
    let old: usize = f.events.len();
    let first: u16 = ladder(f, FLOOR + 1, TOP, next)?;
    let spared: u16 = push(f, format!("{MARK}Abyss"), Kind::Action { next: first, actor, act, params: Some(spare(params)) })?;
    let gate: u16 = life_check(f, format!("{MARK}Gate"), FLOOR, [at, at, spared])?;
    redirect(f, old, at, gate);
    finish(&b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfevfl::{Actor, Entry, Event};

    fn player() -> Actor {
        Actor {
            name: PLAYER.to_string(),
            sub: String::new(),
            arg: String::new(),
            actions: vec![ABYSS.to_string(), "EventPlayerPostAbyss".to_string()],
            queries: vec!["EventQueryCheckPlayerAbyssType".to_string()],
            params: None,
            ep: 0xFFFF,
            cut: 1,
        }
    }

    fn fall_file() -> Vec<u8> {
        let gate: Event = Event { name: "IsGameOver".to_string(), kind: Kind::Switch { actor: 0, query: 0, cases: vec![Case { val: 1, ev: 1 }], params: None } };
        let abyss: Event = Event { name: "Abyss".to_string(), kind: Kind::Action { next: NONE, actor: 0, act: 0, params: cont(vec![("IsWaitEnd", Val::Bool(TRUE)), (NO_DAMAGE, Val::Bool(0))]) } };
        let post: Event = Event { name: "Post".to_string(), kind: Kind::Action { next: NONE, actor: 0, act: 1, params: None } };
        let entries: Vec<Entry> = vec![Entry { name: "Main".to_string(), subs: Vec::new(), vars: Vec::new(), main: 0, pad: 0 }, Entry { name: "Return".to_string(), subs: Vec::new(), vars: Vec::new(), main: 2, pad: 0 }];
        Bf { ver: [0, 4, 0, 0], bom: 0xFEFF, shift: 3, pad: 0, name: FLOW.to_string(), flows: vec![Flow { name: FLOW.to_string(), actors: vec![player()], events: vec![gate, abyss, post], entries }], tls: Vec::new() }.write().unwrap()
    }

    fn walk(f: &Flow, from: u16, life: i32) -> (Vec<u16>, Option<i32>) {
        let mut seen: Vec<u16> = Vec::new();
        let mut heal: Option<i32> = None;
        let mut at: u16 = from;
        while at != NONE {
            seen.push(at);
            assert!(seen.len() < 40, "the flow loops");
            at = match &f.events[at as usize].kind {
                Kind::Switch { actor, query, cases, params } => {
                    let q: &str = &f.actors[*actor as usize].queries[*query as usize];
                    let v: u32 = if q == LIFE {
                        let Some(Val::Int(n)) = params.as_ref().and_then(|c| c.get(VALUE)) else { panic!() };
                        if life < *n { LESS } else if life == *n { EQUAL } else { MORE }
                    } else {
                        1
                    };
                    cases.iter().find(|c| c.val == v).map(|c| c.ev).unwrap_or(NONE)
                }
                Kind::Action { next, actor, act, params } => {
                    if f.actors[*actor as usize].actions[*act as usize] == HEAL {
                        assert!(heal.is_none(), "healed twice");
                        let Some(Val::Int(n)) = params.as_ref().and_then(|c| c.get(VALUE)) else { panic!() };
                        heal = Some(*n);
                    }
                    *next
                }
                _ => panic!(),
            };
        }
        (seen, heal)
    }

    fn abyss_params(f: &Flow, seen: &[u16]) -> Cont {
        let hit: Vec<&u16> = seen.iter().filter(|&&e| is_abyss(f, &f.events[e as usize].kind)).collect();
        assert_eq!(hit.len(), 1);
        f.events[*hit[0] as usize].kind.params().unwrap().clone()
    }

    #[test]
    fn a_fall_with_more_than_one_heart_is_spared_the_damage_and_left_at_one_heart() {
        let mut b: Bf = Bf::parse(&near_death(&fall_file()).unwrap()).unwrap();
        let f: &mut Flow = flow(&mut b, FLOW).unwrap();
        for life in FLOOR + 1..=TOP {
            let (seen, heal): (Vec<u16>, Option<i32>) = walk(f, f.entries[0].main, life);
            assert_eq!(heal, Some(FLOOR - life), "life {life}");
            assert_eq!(life + heal.unwrap(), FLOOR);
            let p: Cont = abyss_params(f, &seen);
            assert_eq!(p.get(NO_DAMAGE), Some(&Val::Bool(TRUE)));
            assert_eq!(p.get("IsWaitEnd"), Some(&Val::Bool(TRUE)));
            assert!(seen.len() <= 12, "life {life} took {} steps", seen.len());
        }
    }

    #[test]
    fn a_fall_at_one_heart_or_less_runs_the_vanilla_event_with_its_damage() {
        let mut b: Bf = Bf::parse(&near_death(&fall_file()).unwrap()).unwrap();
        let f: &mut Flow = flow(&mut b, FLOW).unwrap();
        for life in 0..=FLOOR {
            let (seen, heal): (Vec<u16>, Option<i32>) = walk(f, f.entries[0].main, life);
            assert_eq!(heal, None);
            assert!(seen.contains(&1));
            assert_eq!(abyss_params(f, &seen).get(NO_DAMAGE), Some(&Val::Bool(0)));
        }
    }

    #[test]
    fn the_other_entries_and_the_life_above_the_top_are_left_alone() {
        let mut b: Bf = Bf::parse(&near_death(&fall_file()).unwrap()).unwrap();
        let f: &mut Flow = flow(&mut b, FLOW).unwrap();
        assert_eq!(f.entries[1].main, 2);
        assert_eq!(walk(f, f.entries[1].main, 40), (vec![2], None));
        assert_eq!(walk(f, f.entries[0].main, TOP + 1).1, None);
        let Kind::Action { params, .. } = &f.events[1].kind else { panic!() };
        assert_eq!(params.as_ref().unwrap().get(NO_DAMAGE), Some(&Val::Bool(0)));
        assert_eq!(f.actors.len(), 1);
        assert!(f.actors[0].actions.contains(&HEAL.to_string()));
        assert!(f.actors[0].queries.contains(&LIFE.to_string()));
    }

    #[test]
    fn a_second_edit_and_a_flow_without_the_abyss_event_are_refused() {
        let once: Vec<u8> = near_death(&fall_file()).unwrap();
        assert!(near_death(&once).is_err());
        let mut b: Bf = Bf::parse(&fall_file()).unwrap();
        b.flows[0].actors[0].actions[0] = "EventWait".to_string();
        assert!(near_death(&b.write().unwrap()).is_err());
    }

    #[test]
    fn the_game_fall_flow_is_edited_and_reads_back() {
        let Some(root) = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from).filter(|p| p.is_dir()) else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: crate::romfs::Romfs = crate::romfs::Romfs::open(&root).unwrap();
        let raw: Vec<u8> = rf.read_zs(FILE).unwrap();
        let out: Vec<u8> = near_death(&raw).unwrap();
        let mut b: Bf = Bf::parse(&out).unwrap();
        let f: &mut Flow = flow(&mut b, FLOW).unwrap();
        let spared: Vec<&Event> = f.events.iter().filter(|e| e.name == format!("{MARK}Abyss")).collect();
        assert_eq!(spared.len(), 1);
        assert_eq!(spared[0].kind.params().unwrap().get(NO_DAMAGE), Some(&Val::Bool(TRUE)));
        let gate: u16 = f.events.iter().position(|e| e.name == format!("{MARK}Gate")).unwrap() as u16;
        let into: usize = f.events.iter().filter(|e| matches!(&e.kind, Kind::Switch { cases, .. } if cases.iter().any(|c| c.ev == gate))).count();
        assert_eq!(into, 1);
        assert_eq!(f.events.iter().filter(|e| is_abyss(f, &e.kind)).count(), 2);
    }
}
