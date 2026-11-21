use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Case, Flow, Kind, Val};
use crate::difficulty::flows::{action, cont, finish, flow, marked, push, query, FALSE};
use crate::error::{Error, Res};
use crate::qol::{edit_zs, Have};
use crate::romfs::Romfs;
use crate::staging::Out;

pub const FILES: [&str; 2] = ["Event/EventFlow/Npc_Hylia_Dampe_001.bfevfl.zs", "Event/EventFlow/Npc_Hylia_Dampe_001.110.bfevfl.zs"];
pub const FLOW: &str = "Npc_Hylia_Dampe_001";
pub const ACTOR: &str = "Npc_Hylia_Dampe_001";
pub const ADVENT: &str = "EventMiniGameDiggingHoleAdventActor";
pub const SYSTEM: &str = "GameSystemActor";
pub const COIN: &str = "EventQueryRandomChoice2";
pub const PRIZE: &str = "Obj_Rupee_";
pub const MARK: &str = "OOTRRDampe";
pub const MONSTERS: [&str; 4] = ["Enemy_Poe", "Enemy_Floormaster", "Enemy_ReDead", "Enemy_DeadHand"];
pub const NOTE: &str = "Each time Dampe digs up a rupee on his grave digging tour, a monster comes out of the hole with it: a Poe, a Floormaster, a ReDead or a Dead Hand, one in four each. The Piece of Heart dig at the end is left alone.";

fn is_prize(f: &Flow, kind: &Kind) -> bool {
    let Kind::Action { actor, act, params: Some(c), .. } = kind else {
        return false;
    };
    let named: bool = f.actors.get(*actor as usize).is_some_and(|a| a.name == ACTOR && a.actions.get(*act as usize).is_some_and(|s| s == ADVENT));
    named && matches!(c.get("ActorName"), Some(Val::Str(s)) if s.starts_with(PRIZE))
}

fn monster(f: &mut Flow, i: usize, next: u16) -> Res<u16> {
    let (a, n): (u16, u16) = action(f, ACTOR, ADVENT)?;
    push(f, format!("{MARK}Spawn{i}"), Kind::Action { next, actor: a, act: n, params: cont(vec![("ActorName", Val::Str(MONSTERS[i % MONSTERS.len()].to_string())), ("IsWaitEnd", Val::Bool(FALSE))]) })
}

fn coin(f: &mut Flow, name: String, heads: u16, tails: u16) -> Res<u16> {
    let (a, q): (u16, u16) = query(f, SYSTEM, COIN)?;
    push(f, name, Kind::Switch { actor: a, query: q, cases: vec![Case { val: 0, ev: heads }, Case { val: 1, ev: tails }], params: None })
}

pub fn chain(f: &mut Flow, hole: usize, next: u16) -> Res<u16> {
    let spawns: Vec<u16> = (0..MONSTERS.len()).map(|i| monster(f, i, next)).collect::<Res<Vec<u16>>>()?;
    let low: u16 = coin(f, format!("{MARK}Low{hole}"), spawns[0], spawns[1])?;
    let high: u16 = coin(f, format!("{MARK}High{hole}"), spawns[2], spawns[3])?;
    coin(f, format!("{MARK}Pick{hole}"), low, high)
}

pub fn monsters(raw: &[u8]) -> Res<Vec<u8>> {
    let mut b: Bf = Bf::parse(raw)?;
    let f: &mut Flow = flow(&mut b, FLOW)?;
    if marked(f, MARK) {
        return Err(Error::Format("dampe: the flow is already edited"));
    }
    let prizes: Vec<usize> = (0..f.events.len()).filter(|&i| is_prize(f, &f.events[i].kind)).collect();
    if prizes.len() != 5 {
        return Err(Error::Msg(format!("dampe: expected 5 rupee digs, found {}", prizes.len())));
    }
    for (hole, i) in prizes.iter().enumerate() {
        let old: u16 = match &f.events[*i].kind {
            Kind::Action { next, .. } => *next,
            _ => NONE,
        };
        let first: u16 = chain(f, hole, old)?;
        if let Kind::Action { next, .. } = &mut f.events[*i].kind {
            *next = first;
        }
    }
    finish(&b)
}

pub fn stage(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    FILES.iter().map(|p| edit_zs(rf, have, p, monsters)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfevfl::{Actor, Entry, Event};

    fn actor(name: &str, actions: &[&str], queries: &[&str]) -> Actor {
        Actor { name: name.to_string(), sub: String::new(), arg: String::new(), actions: actions.iter().map(|s| s.to_string()).collect(), queries: queries.iter().map(|s| s.to_string()).collect(), params: None, ep: 0xFFFF, cut: 1 }
    }

    fn dig(name: &str, prize: &str, next: u16) -> Event {
        Event { name: name.to_string(), kind: Kind::Action { next, actor: 0, act: 0, params: cont(vec![("ActorName", Val::Str(prize.to_string())), ("IsWaitEnd", Val::Bool(crate::difficulty::flows::TRUE))]) } }
    }

    fn tour(prizes: &[&str]) -> Vec<u8> {
        let mut events: Vec<Event> = prizes.iter().enumerate().map(|(i, p)| dig(&format!("Dig{i}"), p, NONE)).collect();
        events.push(Event { name: "Other".to_string(), kind: Kind::Action { next: NONE, actor: 0, act: 0, params: cont(vec![("ActorName", Val::Str("Dm_Obj_Graveyard_Rock_A_01".to_string()))]) } });
        let entries: Vec<Entry> = vec![Entry { name: "Main".to_string(), subs: Vec::new(), vars: Vec::new(), main: 0, pad: 0 }];
        Bf { ver: [0, 4, 0, 0], bom: 0xFEFF, shift: 3, pad: 0, name: FLOW.to_string(), flows: vec![Flow { name: FLOW.to_string(), actors: vec![actor(ACTOR, &[ADVENT], &[]), actor(SYSTEM, &[], &[COIN])], events, entries }], tls: Vec::new() }.write().unwrap()
    }

    fn walk(f: &Flow, from: u16) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        let mut at: Vec<u16> = vec![from];
        while let Some(e) = at.pop() {
            if e == NONE {
                continue;
            }
            match &f.events[e as usize].kind {
                Kind::Action { next, params, .. } => {
                    if let Some(Val::Str(s)) = params.as_ref().and_then(|c| c.get("ActorName")) {
                        names.push(s.clone());
                    }
                    if f.events[e as usize].name.starts_with(MARK) {
                        at.push(*next);
                    }
                }
                Kind::Switch { cases, .. } => at.extend(cases.iter().map(|c| c.ev)),
                _ => {}
            }
        }
        names.sort();
        names
    }

    #[test]
    fn a_tour_that_is_not_the_expected_one_is_refused() {
        assert!(monsters(b"not a flow").is_err());
        assert!(monsters(&tour(&["Obj_Rupee_Green"])).is_err());
    }

    #[test]
    fn every_rupee_dig_leads_to_one_of_the_four_monsters_one_in_four_each() {
        let raw: Vec<u8> = tour(&["Obj_Rupee_Green", "Obj_Rupee_Blue", "Obj_Rupee_Red", "Obj_Rupee_Purple", "Obj_Rupee_Gold"]);
        let out: Vec<u8> = monsters(&raw).unwrap();
        let b: Bf = Bf::parse(&out).unwrap();
        let f: &Flow = &b.flows[0];
        let mut want: Vec<String> = MONSTERS.iter().map(|s| s.to_string()).collect();
        want.sort();
        for i in 0..5 {
            let Kind::Action { next, .. } = &f.events[i].kind else { panic!() };
            assert_ne!(*next, NONE);
            assert_eq!(walk(f, *next), want);
        }
        let Kind::Action { next, .. } = &f.events[5].kind else { panic!() };
        assert_eq!(*next, NONE);
        let coins: Vec<&Event> = f.events.iter().filter(|e| matches!(&e.kind, Kind::Switch { cases, .. } if cases.len() == 2)).collect();
        assert_eq!(coins.len(), 15);
        assert_eq!(f.events.len(), 6 + 5 * 7);
        assert!(monsters(&out).is_err());
    }

    #[test]
    fn the_game_flow_gets_five_chains_and_keeps_its_prizes() {
        let Some(p) = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from) else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let Ok(mut rf) = Romfs::open(&p) else {
            return;
        };
        for file in FILES {
            let raw: Vec<u8> = rf.read_zs(file).unwrap();
            let was: Bf = Bf::parse(&raw).unwrap();
            let out: Vec<u8> = monsters(&raw).unwrap();
            let now: Bf = Bf::parse(&out).unwrap();
            let (a, b): (&Flow, &Flow) = (&was.flows[0], &now.flows[0]);
            assert_eq!(b.events.len(), a.events.len() + 35);
            assert_eq!(b.entries, a.entries);
            let prizes: Vec<usize> = (0..b.events.len()).filter(|&i| is_prize(b, &b.events[i].kind)).collect();
            assert_eq!(prizes.len(), 5);
            for i in prizes {
                let Kind::Action { params, .. } = &b.events[i].kind else { panic!() };
                let Kind::Action { params: old, .. } = &a.events[i].kind else { panic!() };
                assert_eq!(params, old);
            }
        }
    }
}
