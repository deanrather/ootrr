pub mod scrub;
pub mod story;

use std::collections::BTreeMap as BM;

use crate::error::{Error as E, Res};
use crate::settings::seed::SeedFile;
use crate::settings::{Settings, Value};
use crate::slice::data::Data;
use crate::slice::plando::Entry;
use crate::slice::support::plan;
use crate::logic_items::ItemId;
use crate::world::{LocId, Placement, World};

pub const FLAG: &str = "--giver-test";
pub const LINE: &str = "Giver test mode";
pub const SWORD: &str = "kokiri_sword";
pub const NOT_BEATABLE: &str = "Giver test mode: this seed is for testing item givers and is not meant to be beatable: the upgrade tier order is not checked";

pub const GIVERS: [(&str, &str); 7] = [
    ("KF Kokiri Sword Chest", "chest"),
    ("KF Midos Top Left Chest", "chest"),
    ("LW Deku Scrub Near Bridge", "scrub sale"),
    ("KF Sarias House Recovery Heart 1", "ground item"),
    ("KF House of Twins Pot 1", "pot"),
    ("KF Shop Right Side Top Left", "shop shelf"),
    ("LW Gift from Saria", "script add"),
];

pub const SHELVES: [(&str, &str); 7] = [
    ("KF Shop Right Side Top Right", "Ocarina"),
    ("KF Shop Right Side Bottom Left", "Minuet of Forest"),
    ("KF Shop Right Side Bottom Right", "Map (Deku Tree)"),
    ("KF Shop Left Side Top Left", "Progressive Hookshot"),
    ("KF Shop Left Side Top Right", "Progressive Strength Upgrade"),
    ("KF Shop Left Side Bottom Left", "Progressive Scale"),
    ("KF Shop Left Side Bottom Right", "Piece of Heart"),
];

pub const NEEDS: [(&str, &[&str], &str); 14] = [
    ("starting_age", &["child"], "child"),
    ("open_forest", &["open"], "open"),
    ("shuffle_chests", &["overworld", "all"], "all"),
    (crate::slice::pick::SWORD, &["true"], "true"),
    (crate::slice::pick::OCARINAS, &["true"], "true"),
    ("shuffle_rewards", &["supported"], "supported"),
    (crate::scrubs_a::SETTING, &["true"], "true"),
    ("shuffle_freestanding_items", &["overworld", "all"], "all"),
    ("shuffle_pots", &["overworld", "all"], "all"),
    (crate::shops::plan::SETTING, &crate::shops::plan::DEAL_LEVELS, "deals_1"),
    (crate::shops::plan::PRICES, &["affordable"], "affordable"),
    ("shuffle_songs", &["anywhere"], "anywhere"),
    (crate::song_models::SETTING, &["true"], "true"),
    ("shuffle_mapcompass", &["keysanity"], "keysanity"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub item: String,
    pub copies: BM<String, i64>,
}

pub struct Mode {
    pub item: String,
    pub settings: Settings,
    pub notes: Vec<String>,
}

fn shown(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => s.clone(),
        Value::Set(x) => x.join(", "),
        Value::Int(n) => n.to_string(),
    }
}

fn wanted(v: &str) -> Value {
    match v {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        x => Value::s(x),
    }
}

pub fn bare_flag() -> E {
    E::Msg(format!("{FLAG} needs the item to test: {FLAG}=<item>, for example {FLAG}=\"Progressive Wallet\""))
}

pub fn start_opt(item: &str, have: &[String]) -> Option<&'static str> {
    let g: &crate::start_items::table::Group = crate::start_items::table::GROUPS.iter().find(|g| g.logic == item)?;
    let opts: Vec<&crate::start_items::table::Opt> = crate::start_items::table::OPTS.iter().filter(|o| o.group == g.id).collect();
    let top: u32 = opts.iter().filter(|o| have.iter().any(|h| h == o.id)).map(|o| o.tier).max().unwrap_or(0);
    opts.iter().filter(|o| o.tier == top + 1).map(|o| o.id).next()
}

impl Mode {
    pub fn new(d: &Data, s: &Settings, ask: &str) -> Res<Mode> {
        let item: String = crate::slice::plando::item_of(&crate::slice::logic::world_with(d, &plan(s)?.logic())?, ask).map(|_| ask.to_string())?;
        let item: String = d.items.iter().find(|i| i.name.eq_ignore_ascii_case(&item)).map(|i| i.name.clone()).unwrap_or(item);
        let mut out: Settings = s.clone();
        let mut changed: Vec<String> = Vec::new();
        for (id, ok, to) in NEEDS {
            let now: String = out.get(id).map(shown).unwrap_or_default();
            if ok.contains(&now.as_str()) {
                continue;
            }
            out.set(id, wanted(to))?;
            changed.push(format!("{id} {now} to {to}"));
        }
        let mut kit: Vec<String> = out.items("starting_items").map(|v| v.to_vec()).unwrap_or_default();
        let mut added: Vec<&str> = Vec::new();
        for want in [Some(SWORD).filter(|s| !kit.iter().any(|k| k == s)), start_opt(&item, &kit)].into_iter().flatten() {
            kit.push(want.to_string());
            added.push(want);
        }
        if !added.is_empty() {
            let refs: Vec<&str> = kit.iter().map(|k| k.as_str()).collect();
            out.set("starting_items", Value::set(&refs))?;
            changed.push(format!("starting_items adds {}", added.join(", ")));
        }
        let out: Settings = out.finish()?;
        let mut notes: Vec<String> = vec![format!("{LINE}: test item {item}; settings changed: {}", if changed.is_empty() { "none".to_string() } else { changed.join("; ") }), NOT_BEATABLE.to_string()];
        if start_opt(&item, &[]).is_none() {
            notes.push(format!("{LINE}: {item} is not a starting item choice, so the new-game giver is left out"));
        }
        Ok(Mode { item, settings: out, notes })
    }

    pub fn entries(&self) -> Vec<Entry> {
        let mut out: Vec<Entry> = GIVERS.iter().map(|(l, _)| Entry { location: l.to_string(), item: self.item.clone() }).collect();
        out.extend(SHELVES.iter().map(|(l, i)| Entry { location: l.to_string(), item: i.to_string() }));
        out
    }

    pub fn keep(&self, d: &Data, w: &World, free: &[LocId], ok: &dyn Fn(ItemId, LocId) -> bool) -> Res<(Vec<(LocId, ItemId)>, Vec<String>)> {
        let mut out: Vec<(LocId, ItemId)> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        for e in self.entries() {
            let l: LocId = crate::slice::plando::loc_of(w, &e.location)?;
            let i: ItemId = crate::slice::plando::item_of(w, &e.item)?;
            if !free.contains(&l) {
                notes.push(format!("{LINE}: {} is not shuffled with these settings, left out", e.location));
            } else if !ok(i, l) {
                notes.push(format!("{LINE}: the placement rules refuse {} at {}, left out", e.item, e.location));
            } else {
                if d.item(&e.item).is_some_and(|x| !crate::slice::flow_rules::allows(d, &e.location, x)) {
                    notes.push(format!("{LINE}: {} hands out one fixed tier of {}, not the next tier", e.location, e.item));
                }
                out.push((l, i));
            }
        }
        let at: Vec<String> = out.iter().map(|(l, i)| format!("{} {}", w.locs[*l].name, w.items.names[*i])).collect();
        notes.push(format!("{LINE}: placed {}", at.join(", ")));
        Ok((out, notes))
    }
}

pub fn fill_pool(d: &Data, w: &World, free: &[LocId], keep: &[(LocId, ItemId)], base: &mut Placement) -> Res<BM<String, i64>> {
    let mut net: BM<String, i64> = BM::new();
    let mut items: Vec<ItemId> = keep.iter().map(|k| k.1).collect();
    items.sort_unstable();
    items.dedup();
    let held: Vec<LocId> = keep.iter().map(|k| k.0).collect();
    for i in items {
        let want: usize = keep.iter().filter(|k| k.1 == i).count();
        let have: usize = free.iter().filter(|l| base.get(**l) == Some(i)).count();
        for _ in have..want {
            let l: LocId = free
                .iter()
                .copied()
                .find(|l| !held.contains(l) && base.get(*l).is_some_and(|j| crate::slice::place_rules::junk(d, &w.items.names[j])))
                .ok_or_else(|| E::Msg(format!("{LINE}: the pool has no junk left to make a copy of {}", w.items.names[i])))?;
            let was: ItemId = base.get(l).ok_or(E::Format("giver test: a junk spot lost its item"))?;
            *net.entry(w.items.names[was].clone()).or_insert(0) -= 1;
            *net.entry(w.items.names[i].clone()).or_insert(0) += 1;
            base.put(l, i);
        }
    }
    net.retain(|_, v| *v != 0);
    Ok(net)
}

pub fn counted(seed: &SeedFile) -> SeedFile {
    let mut out: SeedFile = seed.clone();
    if let Some(m) = &seed.giver_test {
        for (k, v) in &m.copies {
            *out.copies.entry(k.clone()).or_insert(0) += v;
        }
    }
    out
}

#[cfg(test)]
mod tests;
