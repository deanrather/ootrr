use std::fs;
use std::path::{Path as P, PathBuf as PB};

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Cont, Event, Flow, Kind, Val};
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::modpack::{Host, Pack};
use crate::pickup_pack::write::start;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst, TABLE_PATH};
use crate::slice::edit::Built;
use crate::staging::MARGIN;
use crate::start_items::{Kit, Set};
use crate::zs;
use crate::TITLE_ID;

pub const SETTING: &str = "full_wallets";
pub const NUM: u16 = 610;
pub const COUNT: &str = "PlayerStatus.CurrentRupee";
pub const FIRST: (&str, i32) = ("Item_Wallet_Small", 200);
pub const CAPS: [(&str, i32); 2] = [("Item_Wallet_Medium", 500), ("Item_Wallet_Large", 1000)];
pub const POUCH: &str = "PouchSystemActor";
pub const ADD: &str = "EventTriggerIncreaseRupee";
pub const GIVE_KEYS: [&str; 6] = ["ContentName", "ActorName", "ReveiveItem", "ReceiveItem", "TakenRewardActor", "Actor"];
pub const FLOWS: &str = "Event/EventFlow";
pub const VANILLA: [&str; 1] = ["Event/EventFlow/GatheringTokenGold.bfevfl.zs"];
const PREFIX: &str = "OOTRRFullWallet";
pub const NOTE: &str = "Full Wallets: a new game starts with as many rupees as its best wallet holds, and every flow that gives Adult's or Giant's Wallet adds that wallet's capacity in rupees right after the give, which the game should cap at the new capacity. Wallets on the ground are not covered. Not yet seen in the game.";

pub fn cap(actor: &str) -> Option<i32> {
    CAPS.iter().find(|(a, _)| *a == actor).map(|(_, n)| *n)
}

pub fn wallet_of(k: &Kind) -> Option<i32> {
    let c: &Cont = match k {
        Kind::Action { params: Some(c), .. } | Kind::Sub { params: Some(c), .. } => c,
        _ => return None,
    };
    c.items.iter().filter(|(key, _)| GIVE_KEYS.contains(&key.as_str())).find_map(|(_, v)| match v {
        Val::Str(s) => cap(s),
        _ => None,
    })
}

fn next_of(k: &Kind) -> Option<u16> {
    match k {
        Kind::Action { next, .. } | Kind::Sub { next, .. } => Some(*next),
        _ => None,
    }
}

fn set_next(k: &mut Kind, to: u16) {
    if let Kind::Action { next, .. } | Kind::Sub { next, .. } = k {
        *next = to;
    }
}

pub fn ours(f: &Flow, at: u16) -> bool {
    at != NONE && f.events.get(usize::from(at)).is_some_and(|e| e.name.starts_with(PREFIX))
}

pub fn splice(f: &mut Flow) -> Res<usize> {
    let hits: Vec<(usize, i32)> = f.events.iter().enumerate().filter_map(|(i, e)| wallet_of(&e.kind).map(|n| (i, n))).collect();
    let mut n: usize = 0;
    for (i, amount) in hits {
        let then: u16 = next_of(&f.events[i].kind).ok_or(E::Format("full wallets: a wallet give has no next event"))?;
        if ours(f, then) {
            continue;
        }
        let (a, x): (u16, u16) = crate::start_state::slot(f, POUCH, ADD)?;
        let at: u16 = u16::try_from(f.events.len()).map_err(|_| E::Format("full wallets: the flow has too many events"))?;
        let c: Cont = Cont { items: vec![("Value".to_string(), Val::Int(amount))] };
        f.events.push(Event { name: format!("{PREFIX}{at}"), kind: Kind::Action { next: then, actor: a, act: x, params: Some(c) } });
        set_next(&mut f.events[i].kind, at);
        n += 1;
    }
    Ok(n)
}

pub fn start_cap(k: &Kit) -> i32 {
    CAPS.iter().filter(|(a, _)| k.pushes.contains(a)).map(|(_, n)| *n).max().unwrap_or(FIRST.1)
}

pub fn start_sets(k: &Kit) -> Vec<Set> {
    vec![Set::Int(COUNT.to_string(), start_cap(k))]
}

fn flow_files(base: &P) -> Res<Vec<String>> {
    let mut out: Vec<String> = VANILLA.iter().map(|s| s.to_string()).collect();
    let dir: PB = FLOWS.split('/').fold(base.to_path_buf(), |a, s| a.join(s));
    if dir.is_dir() {
        for e in fs::read_dir(&dir)? {
            let name: String = e?.file_name().to_string_lossy().to_string();
            if name.ends_with(".bfevfl.zs") {
                out.push(format!("{FLOWS}/{name}"));
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

pub fn after(on: bool, romfs: &P, load: &P, mod_name: &str, host: Host, mut b: Built) -> Res<Built> {
    if !on {
        return Ok(b);
    }
    let mut rf: Romfs = Romfs::open(romfs)?;
    let base: PB = b.root.join("romfs");
    let own: PB = base.join(TABLE_PATH);
    let mut rst: Rst = if own.is_file() { Rst::parse(&rf.zs.decompress(&fs::read(&own)?)?)? } else { rsizetable::load(&mut rf)? };
    let pk: Pack = Pack::create_for(load, TITLE_ID, mod_name, host)?;
    if pk.root != b.root {
        return Err(E::Msg(format!("the wallet flows went to {} and not to the seed mod {}", pk.root.display(), b.root.display())));
    }
    let mut lines: Vec<String> = Vec::new();
    for path in flow_files(&base)? {
        let modded: bool = path.split('/').fold(base.clone(), |a, s| a.join(s)).is_file();
        let first: Vec<u8> = start(&mut rf, Some(&base), &path)?;
        let mut bf: Bf = Bf::parse(&first)?;
        let mut n: usize = 0;
        for f in bf.flows.iter_mut() {
            n += splice(f)?;
        }
        if n == 0 {
            continue;
        }
        let raw: Vec<u8> = bf.write()?;
        let old: usize = if modded { first.len() } else { rf.read_zs(&path)?.len() };
        pk.write_romfs(&path, &zs::compress_raw(&raw, dict_id(&rf.read(&path)?))?)?;
        if raw.len() > old {
            rst.grow(&path, old as u64, raw.len() as u64, MARGIN)?;
        }
        lines.push(format!("{path} ({n})"));
        b.counts.files += usize::from(!modded);
    }
    pk.write_romfs(TABLE_PATH, &rst.to_zs()?)?;
    b.report.push_str(&format!("Full Wallets: rupees added after {} wallet gives: {}\n", lines.len(), lines.join(", ")));
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(p: &P) -> Flow {
        let raw: Vec<u8> = Romfs::open(p).unwrap().read_zs(VANILLA[0]).unwrap();
        Bf::parse(&raw).unwrap().flows.remove(0)
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

    fn sub(key: &str, v: &str, next: u16) -> Kind {
        Kind::Sub { next, params: Some(Cont { items: vec![(key.to_string(), Val::Str(v.to_string()))] }), flow: String::new(), entry: "Success_".to_string() }
    }

    #[test]
    fn only_upgrade_wallet_gives_count() {
        assert_eq!(wallet_of(&sub("ContentName", "Item_Wallet_Medium", NONE)), Some(500));
        assert_eq!(wallet_of(&sub("TakenRewardActor", "Item_Wallet_Large", 3)), Some(1000));
        assert_eq!(wallet_of(&sub("ContentName", "Item_Wallet_Small", NONE)), None);
        assert_eq!(wallet_of(&sub("DisplayItemActor", "Item_Wallet_Large", NONE)), None);
        assert_eq!(wallet_of(&sub("ContentName", "Item_Hookshot", NONE)), None);
        let q: Kind = Kind::Switch { actor: 0, query: 0, cases: Vec::new(), params: Some(Cont { items: vec![("ActorName".to_string(), Val::Str("Item_Wallet_Medium".to_string()))] }) };
        assert_eq!(wallet_of(&q), None);
        assert_eq!(cap(FIRST.0), None);
    }

    #[test]
    fn a_new_game_starts_with_its_best_wallet_full() {
        let mut k: Kit = Kit::default();
        assert_eq!(start_sets(&k), vec![Set::Int(COUNT.to_string(), 200)]);
        k.pushes.push("Item_Wallet_Medium");
        assert_eq!(start_cap(&k), 500);
        k.pushes.push("Item_Wallet_Large");
        assert_eq!(start_sets(&k), vec![Set::Int(COUNT.to_string(), 1000)]);
        assert!(NUM >= 600);
    }

    #[test]
    fn each_gold_skulltula_wallet_reward_adds_its_capacity_once_and_keeps_its_path() {
        let Some(p) = real() else {
            return;
        };
        let was: Flow = flow(&p);
        let mut f: Flow = was.clone();
        let gives: Vec<(usize, i32, u16)> = was.events.iter().enumerate().filter_map(|(i, e)| wallet_of(&e.kind).map(|n| (i, n, next_of(&e.kind).unwrap()))).collect();
        assert_eq!(gives.iter().map(|g| g.1).collect::<Vec<i32>>(), vec![500, 1000], "{gives:?}");
        assert_eq!(splice(&mut f).unwrap(), 2);
        assert_eq!(f.events.len(), was.events.len() + 2);
        for (i, n, then) in &gives {
            let to: u16 = next_of(&f.events[*i].kind).unwrap();
            assert!(ours(&f, to));
            let Kind::Action { next, actor, act, params } = &f.events[usize::from(to)].kind else {
                panic!("not an action");
            };
            assert_eq!(*next, *then);
            assert_eq!(f.actors[usize::from(*actor)].name, POUCH);
            assert_eq!(f.actors[usize::from(*actor)].actions[usize::from(*act)], ADD);
            assert_eq!(params.as_ref().unwrap().items, vec![("Value".to_string(), Val::Int(*n))]);
        }
        for (i, e) in was.events.iter().enumerate() {
            if !gives.iter().any(|g| g.0 == i) {
                assert_eq!(&f.events[i], e, "event {i}");
            }
        }
        let again: Flow = f.clone();
        assert_eq!(splice(&mut f).unwrap(), 0);
        assert_eq!(f, again);
        let mut bf: Bf = Bf::parse(&Romfs::open(&p).unwrap().read_zs(VANILLA[0]).unwrap()).unwrap();
        bf.flows[0] = f.clone();
        assert_eq!(Bf::parse(&bf.write().unwrap()).unwrap().flows[0], f);
    }

    #[test]
    fn the_chest_flows_give_each_tier_and_get_their_rupees() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        for path in [crate::progressive::SMALL_FLOW, crate::progressive::BIG_FLOW] {
            let mut bf: Bf = Bf::parse(&rf.read_zs(path).unwrap()).unwrap();
            let f: &mut Flow = bf.flows.first_mut().unwrap();
            assert_eq!(splice(f).unwrap(), 0, "{path} gives no wallet before the progressive wrap");
            crate::progressive::wrap(f, "Wallet").unwrap();
            assert_eq!(splice(f).unwrap(), 2, "{path}");
            let back: Bf = Bf::parse(&bf.write().unwrap()).unwrap();
            assert_eq!(back.flows[0].events.iter().filter(|e| e.name.starts_with(PREFIX)).count(), 2);
        }
    }
}
