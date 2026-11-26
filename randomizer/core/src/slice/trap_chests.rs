use std::collections::BTreeMap as BM;
use std::path::Path as P;

use crate::byml::{Byml, Node};
use crate::chest_slot::{self, LIST};
use crate::error::{Error as E, Res};
use crate::placement::{drops, hash_of};
use crate::romfs::Romfs;
use crate::staging::{edit_staged, Out};

pub const ACTOR: &str = "Obj_TBox_A_04";
pub const CHEST: &str = "Obj_TBox_A_01";
pub const LOGIC: &str = "Gimmick_TrapTreasureBox_Set";
pub const OFF: &str = "it is a trap chest in the game";

pub const NAMES: [&str; 6] = [
    "Spirit Temple First Mirror Left Chest",
    "Gerudo Training Ground Heavy Block Fourth Chest",
    "Ganons Castle Water Trial Left Chest",
    "Ganons Castle Light Trial First Right Chest",
    "Ganons Castle Light Trial Second Left Chest",
    "Ganons Castle Light Trial Third Right Chest",
];

pub fn gated(name: &str) -> bool {
    NAMES.contains(&name)
}

fn reference(n: &Node) -> Option<u64> {
    match n.get("Reference") {
        Some(Node::U64(x)) => Some(*x),
        Some(Node::I64(x)) => Some(*x as u64),
        _ => None,
    }
}

fn text<'a>(n: &'a Node, k: &str) -> &'a str {
    n.get(k).and_then(|v| v.as_str()).unwrap_or("")
}

fn unhook(g: &mut Node, hash: u64) -> usize {
    let whole: bool = text(g, "Logic").contains(LOGIC);
    let Some(refs) = g.get_mut("References").and_then(|r| r.as_arr_mut()) else {
        return 0;
    };
    let subs: Vec<String> = refs
        .iter()
        .filter(|r| text(r, "Id") == "AiGroup" && text(r, "Logic").contains(LOGIC))
        .filter_map(|r| text(r, "Path").strip_suffix("AiGroup").map(|p| p.to_string()))
        .collect();
    let before: usize = refs.len();
    refs.retain(|r| reference(r) != Some(hash) || !(whole || subs.iter().any(|p| text(r, "Path").starts_with(p.as_str()))));
    before - refs.len()
}

pub fn open(y: &mut Byml, hash: u64, drop: &str) -> Res<usize> {
    let mut n: usize = 0;
    if let Some(list) = y.root.get_mut("Actors").and_then(|a| a.as_arr_mut()) {
        for a in list.iter_mut() {
            if hash_of(a) != Some(hash) || a.get("Gyaml").and_then(|v| v.as_str()) != Some(ACTOR) {
                continue;
            }
            a.set("Gyaml", Node::Str(CHEST.to_string()));
            let set: bool = match a.get_mut("Dynamic") {
                Some(d) => d.set("Drop__DropActor", Node::Str(drop.to_string())),
                None => a.set("Dynamic", Node::Map(vec![("Drop__DropActor".to_string(), Node::Str(drop.to_string()))])),
            };
            if !set {
                return Err(E::Msg(format!("trap chest {hash} could not take an item")));
            }
            n += 1;
        }
    }
    if n == 0 {
        return Ok(0);
    }
    let mut cut: usize = 0;
    if let Some(gs) = y.root.get_mut("AiGroups").and_then(|g| g.as_arr_mut()) {
        for g in gs.iter_mut() {
            cut += unhook(g, hash);
        }
    }
    if cut == 0 {
        return Err(E::Msg(format!("trap chest {hash} has no trap logic to unhook, the game files differ")));
    }
    Ok(n)
}

pub fn stage(rf: &mut Romfs, more: &mut Vec<Out>, work: &P, hashes: &[u64]) -> Res<()> {
    if hashes.is_empty() {
        return Ok(());
    }
    let mut want: Vec<u64> = hashes.to_vec();
    want.sort_unstable();
    want.dedup();
    let names: BM<u32, String> = chest_slot::names_of(drops(rf)?.into_iter().map(|x| x.hash));
    edit_staged(rf, more, work, LIST, true, |raw| {
        let mut y: Byml = Byml::parse(raw)?;
        let free: Vec<u32> = chest_slot::spares(&y);
        let mut i: usize = 0;
        for h in &want {
            if chest_slot::has(&y, *h) {
                continue;
            }
            let s: u32 = *free.get(i).ok_or(E::Format("no spare flag left for a trap chest"))?;
            chest_slot::add(&mut y, *h, s, &names)?;
            i += 1;
        }
        y.write()
    })
}
