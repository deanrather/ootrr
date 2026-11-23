use super::flows::{action, cont, finish, flow, marked, push, query, redirect};
use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Case, Cont, Entry, Flow, Kind, Val};
use crate::error::{Error, Res};

pub const FILE: &str = "Event/EventFlow/DmF_SY_FallDown.bfevfl.zs";
pub const FLOW: &str = "DmF_SY_FallDown";
pub const PLAYER: &str = "Player_Event";
pub const ABYSS: &str = "EventPlayerAbyss";
pub const LIFE: &str = "EventQueryCurrentLife";
pub const HEAL: &str = "EventTriggerHealLife";
pub const WAITER: &str = "EventSystemActor";
pub const WAIT: &str = "EventWait";
pub const NO_DAMAGE: &str = "InvalidDamage";
pub const VALUE: &str = "Value";
pub const MARK: &str = "OOTRRPitPenalty";
pub const ORIGINAL: &str = "OOTRRPitOriginal";
pub const LIFT: i32 = 4;
pub const BIG_LIFT: i32 = 8;
pub const WEAK: i32 = 2;
pub const SETTLE: i32 = 4;
pub const HEART: i32 = 4;
pub const LAST: i32 = 1;
pub const TOP: i32 = 80;
pub const TRUE: u32 = 0x8000_0001;
pub const LESS: u32 = 0;
pub const EQUAL: u32 = 1;
pub const MORE: u32 = 2;

pub const FREE: i64 = 0;
pub const VANILLA: i64 = 1;
pub const ONE_HEART: i64 = 2;
pub const ZERO: i64 = 3;
pub const RANGE: (i64, i64) = (FREE, ZERO);

pub fn note(level: i64) -> String {
    match level {
        FREE => "Falling into a pit, lava or quicksand costs nothing: Link comes back at the edge with the hearts he had.".to_string(),
        ONE_HEART => "Falling into a pit, lava or quicksand leaves Link at one heart. With one heart or less left, a fall costs nothing.".to_string(),
        ZERO => "Falling into a pit, lava or quicksand takes every heart: a bottled fairy saves Link, otherwise it is game over.".to_string(),
        _ => String::new(),
    }
}

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

fn ladder(f: &mut Flow, tag: &str, lo: i32, hi: i32, target: i32, after: u16) -> Res<u16> {
    if lo > hi {
        return Ok(after);
    }
    let mid: i32 = lo + (hi - lo) / 2;
    let below: u16 = ladder(f, tag, lo, mid - 1, target, after)?;
    let above: u16 = ladder(f, tag, mid + 1, hi, target, after)?;
    let (a, n): (u16, u16) = action(f, PLAYER, HEAL)?;
    let take: u16 = push(f, format!("{MARK}{tag}Take{mid}"), Kind::Action { next: after, actor: a, act: n, params: cont(vec![(VALUE, Val::Int(target - mid))]) })?;
    life_check(f, format!("{MARK}{tag}Life{mid}"), mid, [below, take, above])
}

fn spare(params: Option<Cont>) -> Cont {
    let mut c: Cont = params.unwrap_or_default();
    match c.items.iter_mut().find(|(k, _)| k == NO_DAMAGE) {
        Some((_, v)) => *v = Val::Bool(TRUE),
        None => c.items.push((NO_DAMAGE.to_string(), Val::Bool(TRUE))),
    }
    c
}

fn to_last(f: &mut Flow, hi: i32, at: u16) -> Res<u16> {
    let (a, n): (u16, u16) = action(f, WAITER, WAIT)?;
    let wait: u16 = push(f, format!("{MARK}Wait"), Kind::Action { next: at, actor: a, act: n, params: cont(vec![("IsWaitEnd", Val::Bool(TRUE)), ("Frames", Val::Int(1))]) })?;
    ladder(f, "Last", LAST + 1, hi, LAST, wait)
}

fn local_flow(f: &Flow) -> String {
    f.events
        .iter()
        .find_map(|e| match &e.kind {
            Kind::Sub { flow, entry, .. } if f.entries.iter().any(|x| &x.name == entry) => Some(flow.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn lifted(f: &mut Flow, by: i32, flow: &str, calls: &mut Vec<u16>) -> Res<u16> {
    let (ha, hn): (u16, u16) = action(f, PLAYER, HEAL)?;
    let back: u16 = push(f, format!("{MARK}Down{by}"), Kind::Action { next: NONE, actor: ha, act: hn, params: cont(vec![(VALUE, Val::Int(-by))]) })?;
    let call: u16 = push(f, format!("{MARK}Call{by}"), Kind::Sub { next: back, params: None, flow: flow.to_string(), entry: ORIGINAL.to_string() })?;
    calls.push(call);
    let (a, n): (u16, u16) = action(f, WAITER, WAIT)?;
    let wait: u16 = push(f, format!("{MARK}Settle{by}"), Kind::Action { next: call, actor: a, act: n, params: cont(vec![("IsWaitEnd", Val::Bool(TRUE)), ("Frames", Val::Int(SETTLE))]) })?;
    push(f, format!("{MARK}Up{by}"), Kind::Action { next: wait, actor: ha, act: hn, params: cont(vec![(VALUE, Val::Int(by))]) })
}

fn wrap(f: &mut Flow, level: i64) -> Res<()> {
    let at: usize = f.entries.iter().position(|e| e.name == FLOW).ok_or(Error::Format("difficulty: the fall flow has no entry of its own name"))?;
    let orig: Entry = f.entries[at].clone();
    f.entries.push(Entry { name: ORIGINAL.to_string(), ..orig });
    let flow: String = local_flow(f);
    let mut calls: Vec<u16> = Vec::new();
    let big: u16 = lifted(f, BIG_LIFT, &flow, &mut calls)?;
    let small: u16 = lifted(f, LIFT, &flow, &mut calls)?;
    let above: u16 = if level == ONE_HEART {
        ladder(f, "One", HEART + 1, TOP, HEART, small)?
    } else {
        let call: u16 = push(f, format!("{MARK}Call"), Kind::Sub { next: NONE, params: None, flow: flow.clone(), entry: ORIGINAL.to_string() })?;
        calls.push(call);
        call
    };
    let mid: u16 = life_check(f, format!("{MARK}Gate"), HEART, [small, small, above])?;
    let first: u16 = life_check(f, format!("{MARK}Weak"), WEAK, [big, big, mid])?;
    f.entries[at].main = first;
    f.entries[at].subs = calls;
    Ok(())
}

pub fn penalty(raw: &[u8], level: i64) -> Res<Vec<u8>> {
    if !(RANGE.0..=RANGE.1).contains(&level) || level == VANILLA {
        return Err(Error::Msg(format!("difficulty: fall penalty {level} has no edit")));
    }
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
    if level == ZERO {
        let old: usize = f.events.len();
        let start: u16 = to_last(f, TOP, at)?;
        redirect(f, old, at, start);
    } else {
        f.events[at as usize].kind = Kind::Action { next, actor, act, params: Some(spare(params)) };
        wrap(f, level)?;
    }
    finish(&b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfevfl::{Actor, Entry, Event};

    fn actor(name: &str, actions: &[&str], queries: &[&str]) -> Actor {
        Actor {
            name: name.to_string(),
            sub: String::new(),
            arg: String::new(),
            actions: actions.iter().map(|s| s.to_string()).collect(),
            queries: queries.iter().map(|s| s.to_string()).collect(),
            params: None,
            ep: 0xFFFF,
            cut: 1,
        }
    }

    fn fall_file() -> Vec<u8> {
        let gate: Event = Event { name: "IsGameOver".to_string(), kind: Kind::Switch { actor: 0, query: 0, cases: vec![Case { val: 1, ev: 1 }], params: None } };
        let abyss: Event = Event { name: "Abyss".to_string(), kind: Kind::Action { next: NONE, actor: 0, act: 0, params: cont(vec![("IsWaitEnd", Val::Bool(TRUE)), (NO_DAMAGE, Val::Bool(0))]) } };
        let post: Event = Event { name: "Post".to_string(), kind: Kind::Action { next: NONE, actor: 0, act: 1, params: None } };
        let entries: Vec<Entry> = vec![Entry { name: FLOW.to_string(), subs: Vec::new(), vars: Vec::new(), main: 0, pad: 0 }, Entry { name: "Return".to_string(), subs: Vec::new(), vars: Vec::new(), main: 2, pad: 0 }];
        let actors: Vec<Actor> = vec![actor(PLAYER, &[ABYSS, "EventPlayerPostAbyss"], &["EventQueryCheckPlayerAbyssType"]), actor(WAITER, &[WAIT], &[])];
        Bf { ver: [0, 4, 0, 0], bom: 0xFEFF, shift: 3, pad: 0, name: FLOW.to_string(), flows: vec![Flow { name: FLOW.to_string(), actors, events: vec![gate, abyss, post], entries }], tls: Vec::new() }.write().unwrap()
    }

    #[derive(Debug, PartialEq)]
    struct Run {
        life: i32,
        damage: Option<bool>,
        heal_before: bool,
        waited: bool,
        steps: usize,
        heals: usize,
        at_abyss: i32,
    }

    fn walk(f: &Flow, from: u16, start: i32) -> Run {
        let mut life: i32 = start;
        let mut damage: Option<bool> = None;
        let mut heal_before: bool = false;
        let mut waited: bool = false;
        let mut steps: usize = 0;
        let mut at: u16 = from;
        let mut heals: usize = 0;
        let mut at_abyss: i32 = 0;
        let mut stack: Vec<u16> = Vec::new();
        loop {
            while at == NONE && !stack.is_empty() {
                at = stack.pop().unwrap();
            }
            if at == NONE {
                break;
            }
            steps += 1;
            assert!(steps < 80, "the flow loops");
            at = match &f.events[at as usize].kind {
                Kind::Sub { next, entry, .. } => {
                    stack.push(*next);
                    f.entries.iter().find(|e| &e.name == entry).unwrap().main
                }
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
                    let name: &str = &f.actors[*actor as usize].actions[*act as usize];
                    if name == HEAL {
                        heals += 1;
                        assert!(heals <= 3, "healed four times");
                        let Some(Val::Int(n)) = params.as_ref().and_then(|c| c.get(VALUE)) else { panic!() };
                        life += *n;
                        if damage.is_none() {
                            heal_before = true;
                        }
                    }
                    if name == WAIT {
                        waited = true;
                    }
                    if name == ABYSS {
                        assert!(damage.is_none(), "two abyss events");
                        let off: bool = params.as_ref().and_then(|c| c.get(NO_DAMAGE)) == Some(&Val::Bool(TRUE));
                        damage = Some(!off);
                        at_abyss = life;
                    }
                    *next
                }
                _ => panic!(),
            };
        }
        Run { life, damage, heal_before, waited, steps, heals, at_abyss }
    }

    fn edited(level: i64) -> Bf {
        Bf::parse(&penalty(&fall_file(), level).unwrap()).unwrap()
    }

    fn lift_for(life: i32) -> i32 {
        if life <= WEAK { BIG_LIFT } else { LIFT }
    }

    #[test]
    fn no_penalty_leaves_every_life_alone_and_lifts_a_low_one_before_anything_runs() {
        let mut b: Bf = edited(FREE);
        let f: &mut Flow = flow(&mut b, FLOW).unwrap();
        for life in 1..=TOP {
            let r: Run = walk(f, f.entries[0].main, life);
            assert_eq!((r.life, r.damage), (life, Some(false)), "life {life}");
            if life > HEART {
                assert_eq!((r.heals, r.at_abyss), (0, life), "life {life}");
            } else {
                assert_eq!((r.heals, r.at_abyss, r.waited), (2, life + lift_for(life), true), "life {life}");
            }
        }
    }

    #[test]
    fn one_heart_leaves_one_heart_above_it_and_leaves_a_heart_or_less_alone() {
        let mut b: Bf = edited(ONE_HEART);
        let f: &mut Flow = flow(&mut b, FLOW).unwrap();
        for life in 1..=TOP {
            let r: Run = walk(f, f.entries[0].main, life);
            let want: i32 = life.min(HEART);
            assert_eq!((r.life, r.damage, r.waited), (want, Some(false), true), "life {life}");
            if life > HEART {
                assert_eq!((r.heals, r.at_abyss), (3, HEART + LIFT), "life {life}");
            } else {
                assert_eq!((r.heals, r.at_abyss), (2, life + lift_for(life)), "life {life}");
            }
        }
    }

    #[test]
    fn zero_brings_every_life_down_to_a_quarter_heart_before_the_vanilla_damage() {
        let mut b: Bf = edited(ZERO);
        let f: &mut Flow = flow(&mut b, FLOW).unwrap();
        for life in 1..=TOP {
            let r: Run = walk(f, f.entries[0].main, life);
            assert_eq!((r.life, r.damage, r.waited), (LAST, Some(true), true), "life {life}");
            assert!(r.heals <= 1);
            assert!(r.steps <= 12, "life {life} took {} steps", r.steps);
        }
    }

    #[test]
    fn the_other_entries_and_the_original_event_are_left_alone() {
        for level in [FREE, ONE_HEART, ZERO] {
            let mut b: Bf = edited(level);
            let f: &mut Flow = flow(&mut b, FLOW).unwrap();
            assert_eq!(f.entries[1].main, 2);
            assert_eq!(walk(f, f.entries[1].main, 40).steps, 1);
            let Kind::Action { params, next, .. } = &f.events[1].kind else { panic!() };
            let off: Val = Val::Bool(if level == ZERO { 0 } else { TRUE });
            assert_eq!(params.as_ref().unwrap().get(NO_DAMAGE), Some(&off));
            assert_eq!(*next, NONE);
            assert_eq!(f.actors.len(), 2);
        }
    }

    #[test]
    fn vanilla_a_second_edit_and_a_flow_without_the_abyss_event_are_refused() {
        assert!(penalty(&fall_file(), VANILLA).is_err());
        assert!(penalty(&fall_file(), 4).is_err());
        let once: Vec<u8> = penalty(&fall_file(), ONE_HEART).unwrap();
        assert!(penalty(&once, ZERO).is_err());
        let mut b: Bf = Bf::parse(&fall_file()).unwrap();
        b.flows[0].actors[0].actions[0] = "EventWait".to_string();
        assert!(penalty(&b.write().unwrap(), FREE).is_err());
    }

    #[test]
    fn the_game_fall_flow_takes_every_level_and_reads_back() {
        let Some(root) = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from).filter(|p| p.is_dir()) else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: crate::romfs::Romfs = crate::romfs::Romfs::open(&root).unwrap();
        let raw: Vec<u8> = rf.read_zs(FILE).unwrap();
        for level in [FREE, ONE_HEART, ZERO] {
            let out: Vec<u8> = penalty(&raw, level).unwrap();
            let mut b: Bf = Bf::parse(&out).unwrap();
            let f: &mut Flow = flow(&mut b, FLOW).unwrap();
            let main: u16 = f.entries[0].main;
            let new: Vec<u16> = (0..f.events.len() as u16).filter(|&i| f.events[i as usize].name.starts_with(MARK)).collect();
            if level == ZERO {
                let first: u16 = *new.last().unwrap();
                let into: usize = f.events.iter().filter(|e| !e.name.starts_with(MARK) && matches!(&e.kind, Kind::Switch { cases, .. } if cases.iter().any(|c| c.ev == first))).count();
                assert_eq!(into, 1, "level {level}");
                assert!(!new.contains(&main));
            } else {
                assert!(new.contains(&main), "level {level}");
                let o: &Entry = f.entries.iter().find(|e| e.name == ORIGINAL).unwrap();
                assert_eq!(o.main, 45);
                assert!(f.entries.iter().filter(|e| e.name == FLOW).count() == 1);
            }
            let wait: usize = f.actors.iter().position(|a| a.name == WAITER && a.sub.is_empty()).unwrap();
            assert!(f.actors[wait].actions.contains(&WAIT.to_string()));
        }
    }
}
