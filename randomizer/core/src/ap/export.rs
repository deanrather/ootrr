use std::collections::BTreeSet as BS;

use crate::error::{Error as E, Res};
use crate::json::Json;
use crate::logic_items::ItemId;
use crate::logic_val::Val;
use crate::rule_ast::{Bop, Cmp};
use crate::rules::{Node, Rule};
use crate::settings::Settings;
use crate::slice::data::Data;
use crate::slice::generate::{allowed, prepare, Prep};
use crate::slice::pick::scoped;
use crate::slice::support::{Plan, Songs};
use crate::rng::Rng;
use crate::search::{explore, Search};
use crate::world::{LocId, Placement, World};

pub const FORMAT: &str = "ootrr-ap-bundle";
pub const VERSION: i64 = 1;
pub const GAME: &str = "OOTRR";
pub const SEED: &str = "archipelago";

fn int(n: usize) -> Json {
    Json::Int(n as i64)
}

fn ids(v: &[ItemId]) -> Json {
    Json::Arr(v.iter().map(|i| int(*i)).collect())
}

fn val(v: &Val) -> Json {
    match v {
        Val::Bool(b) => Json::Bool(*b),
        Val::Int(n) => Json::Int(*n),
        Val::Float(f) => Json::Num(*f),
        Val::Str(s) => Json::Str(s.clone()),
        Val::List(l) => Json::Arr(l.iter().map(val).collect()),
        Val::Map(m) => Json::Obj(m.iter().map(|(k, x)| (k.clone(), val(x))).collect()),
    }
}

fn tag(t: &str, rest: Vec<Json>) -> Json {
    let mut v: Vec<Json> = vec![Json::Str(t.to_string())];
    v.extend(rest);
    Json::Arr(v)
}

fn cmp(c: Cmp) -> &'static str {
    match c {
        Cmp::Eq => "==",
        Cmp::Ne => "!=",
        Cmp::Lt => "<",
        Cmp::Le => "<=",
        Cmp::Gt => ">",
        Cmp::Ge => ">=",
        Cmp::In => "in",
        Cmp::NotIn => "not in",
    }
}

fn bop(b: Bop) -> &'static str {
    match b {
        Bop::Add => "+",
        Bop::Sub => "-",
        Bop::Mul => "*",
        Bop::Div => "/",
        Bop::Floor => "//",
        Bop::Rem => "%",
    }
}

pub fn node(n: &Node) -> Json {
    match n {
        Node::Const(v) => tag("c", vec![val(v)]),
        Node::Has(i, k) => tag("h", vec![int(*i), Json::Int(*k as i64)]),
        Node::Any(v) => tag("any", vec![ids(v)]),
        Node::All(v) => tag("all", vec![ids(v)]),
        Node::Item(i) => tag("i", vec![int(*i)]),
        Node::Distinct(v) => tag("d", vec![ids(v)]),
        Node::Age => tag("age", vec![]),
        Node::AgeIs(a) => tag("ageis", vec![Json::Str(a.name().to_string())]),
        Node::Tod(f, skip) => tag("tod", vec![Json::Int(*f as i64), node(skip)]),
        Node::Not(x) => tag("not", vec![node(x)]),
        Node::Neg(x) => tag("neg", vec![node(x)]),
        Node::And(v) => tag("and", vec![Json::Arr(v.iter().map(node).collect())]),
        Node::Or(v) => tag("or", vec![Json::Arr(v.iter().map(node).collect())]),
        Node::Cmp(c, a, b) => tag("cmp", vec![Json::Str(cmp(*c).to_string()), node(a), node(b)]),
        Node::Bin(o, a, b) => tag("bin", vec![Json::Str(bop(*o).to_string()), node(a), node(b)]),
        Node::If(k, t, e) => tag("if", vec![node(k), node(t), node(e)]),
        Node::List(v) => tag("list", vec![Json::Arr(v.iter().map(node).collect())]),
    }
}

fn rule(r: &Rule) -> Json {
    if r.always {
        return Json::Bool(true);
    }
    if r.never {
        return Json::Bool(false);
    }
    node(&r.node)
}

fn opt(s: &Option<String>) -> Json {
    match s {
        Some(x) => Json::Str(x.clone()),
        None => Json::Null,
    }
}

pub fn world_json(w: &World) -> Json {
    let items: Vec<Json> = (0..w.items.len())
        .map(|i| Json::obj().with("name", Json::Str(w.items.names[i].clone())).with("adv", Json::Bool(w.items.adv[i])).with("event", Json::Bool(w.items.event[i])))
        .collect();
    let regions: Vec<Json> = w
        .regions
        .iter()
        .map(|r| {
            Json::obj()
                .with("name", Json::Str(r.name.clone()))
                .with("time", Json::Int(r.provides_time as i64))
                .with("dungeon", opt(&r.dungeon))
                .with("scene", opt(&r.scene))
                .with("exits", Json::Arr(r.exits.iter().map(|x| int(*x)).collect()))
                .with("locs", Json::Arr(r.locations.iter().map(|x| int(*x)).collect()))
        })
        .collect();
    let exits: Vec<Json> = w.exits.iter().map(|x| Json::obj().with("name", Json::Str(x.name.clone())).with("from", int(x.from)).with("to", int(x.to)).with("rule", rule(&x.rule))).collect();
    let locs: Vec<Json> = w
        .locs
        .iter()
        .map(|l| {
            Json::obj()
                .with("name", Json::Str(l.name.clone()))
                .with("region", int(l.region))
                .with("rule", rule(&l.rule))
                .with("event", l.event.map(int).unwrap_or(Json::Null))
        })
        .collect();
    let tod: BS<u32> = w.tod_users.iter().copied().collect();
    Json::obj()
        .with("items", Json::Arr(items))
        .with("regions", Json::Arr(regions))
        .with("exits", Json::Arr(exits))
        .with("locs", Json::Arr(locs))
        .with("goal", rule(&w.goal))
        .with("start", Json::Arr(w.start.iter().map(|(i, n)| Json::Arr(vec![int(*i), Json::Int(*n as i64)])).collect()))
        .with("root", int(w.root))
        .with("real", Json::Arr(w.real.iter().map(|l| int(*l)).collect()))
        .with("tod_users", Json::Arr(tod.iter().map(|s| Json::Int(*s as i64)).collect()))
}

/// Where a dungeon item may go, by its kind's setting.
fn keep(p: &Plan, kind: &str) -> Option<crate::slice::support::Keep> {
    match kind {
        "SmallKey" => Some(p.keys.small),
        k => crate::slice::dungeon_items::keep_of(p, k),
    }
}

/// True for an item that stays in its own world. A dungeon item may go to other worlds only when
/// its setting is Anywhere and the mailbox can hand it over (crate::ap::mailbox::dungeon_item).
fn local_only(d: &Data, p: &Plan, name: &str) -> bool {
    let Some(i) = d.item(name) else { return false };
    if super::mailbox::LOCAL_KINDS.contains(&i.kind.as_str()) {
        return !(keep(p, &i.kind) == Some(crate::slice::support::Keep::Anywhere) && super::mailbox::dungeon_item(d, name).is_some());
    }
    if scoped(p, &i.kind) || crate::slice::dungeon_items::handles(p, &i.kind) {
        return true;
    }
    if p.keys_roam() && i.kind == "SmallKey" {
        return true;
    }
    p.songs == Songs::Own && i.actor.as_deref().is_some_and(crate::slice::generate::is_song)
}

const TARGETS: &str = include_str!("../../../../data/tracker_targets.json");
pub const CHEST_FLAG: &str = "IsTreasureBoxOpen.";
pub const SKULL_FLAG: &str = "IsGoldSkullDead.IsGoldSkullDead_";
/// Set whenever an item's get plays, wherever it came from, so it tells nothing about one
/// location once items are shuffled (a received Song of Time set Song from Ocarina of Time's).
pub const ITEM_GET_FLAG: &str = "HavePlayedGetEvent.";
/// Set by the Gerudo jail key actors themselves (Obj_Dungeon_SmallKey_GerudoFortress_01 to 04):
/// the hideout's logic graphs only read them. A jail spot holding another item never sets it.
pub const JAIL_KEY_FLAG: &str = "IsGet_SmallKey_GerudoFortress.";

/// False for a flag that belongs to an item rather than a place, so it cannot tell whether one
/// location was collected once items are shuffled.
pub fn location_flag(f: &str) -> bool {
    !f.starts_with(ITEM_GET_FLAG) && !f.starts_with(JAIL_KEY_FLAG)
}

pub struct Detect {
    by: std::collections::HashMap<String, Vec<Json>>,
}

impl Detect {
    pub fn load() -> Res<Detect> {
        let j: Json = crate::json::parse(TARGETS)?;
        let mut by: std::collections::HashMap<String, Vec<Json>> = std::collections::HashMap::new();
        for t in j.get("targets").and_then(|v| v.as_arr()).unwrap_or(&[]) {
            if let Some(n) = t.get("ootr").and_then(|v| v.as_str()) {
                by.entry(n.to_string()).or_default().push(t.clone());
            }
        }
        Ok(Detect { by })
    }

    /// Flags and keys that more than one location would claim. The client counts a location as
    /// found when any of its flags or keys is set, so a shared one would send every location
    /// that names it (the KF House of Twins and Know it All House pots share map hashes, and
    /// neighbouring Barinade pots share theirs). Every location the catalog or the tracker
    /// knows is counted, shuffled or not, because an unshuffled pot still sets its key.
    pub fn shared(&self, d: &Data) -> BS<String> {
        let mut names: BS<&str> = d.cat.locs.iter().map(|l| l.name.as_str()).collect();
        names.extend(self.by.keys().map(|k| k.as_str()));
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for n in names {
            let (f, k) = self.of(d, n);
            for x in f.into_iter().chain(k) {
                *seen.entry(x).or_default() += 1;
            }
        }
        seen.into_iter().filter(|(_, c)| *c > 1).map(|(x, _)| x).collect()
    }

    pub fn of(&self, d: &Data, name: &str) -> (BS<String>, BS<String>) {
        let mut flags: BS<String> = BS::new();
        let mut keys: BS<String> = BS::new();
        if crate::slice::skulls::is_skull(d, name) {
            for h in crate::slice::skull_names::hashes(d, name) {
                keys.insert(h.to_string());
                flags.insert(format!("{SKULL_FLAG}{h}"));
            }
        } else if let Some(c) = d.cat.by_name(name) {
            for r in &c.refs {
                if c.kind == "chest" {
                    flags.insert(format!("{CHEST_FLAG}{}", r.hash));
                } else {
                    keys.insert(r.hash.to_string());
                }
            }
        }
        let strs = |t: &Json, k: &str| -> Vec<String> { t.get(k).and_then(|v| v.as_arr()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default() };
        for t in self.by.get(name).map(|v| v.as_slice()).unwrap_or(&[]) {
            let skull: bool = t.get("kind").and_then(|v| v.as_str()) == Some("skulltula");
            match t.get("how").and_then(|v| v.as_str()) {
                Some("flag") if !t.get("weak").and_then(|v| v.as_bool()).unwrap_or(false) => flags.extend(strs(t, "flags").into_iter().filter(|f| location_flag(f))),
                Some("key") => {
                    for k in strs(t, "keys").into_iter().chain(strs(t, "keys2")) {
                        if skull {
                            flags.insert(format!("{SKULL_FLAG}{k}"));
                        }
                        keys.insert(k);
                    }
                }
                _ => {}
            }
        }
        (flags, keys)
    }
}

pub fn foreign_ok(d: &Data, name: &str) -> bool {
    d.cat.by_name(name).is_some_and(|c| c.kind == "chest") || d.flow.by_name(name).is_some()
}

fn kind_of(d: &Data, name: &str) -> String {
    if let Some(c) = d.cat.by_name(name) {
        return c.kind.clone();
    }
    if d.flow.by_name(name).is_some() {
        return "npc".to_string();
    }
    if crate::shops::is_loc(name) {
        return "shop".to_string();
    }
    "other".to_string()
}

pub fn export(d0: &Data, s: &Settings, preset: &str) -> Res<Json> {
    Ok(export_with(d0, s, preset, SEED, 0)?.0)
}

pub fn names(d: &Data) -> Res<Json> {
    let mut over: Vec<(&str, &str)> = crate::spots::FULL.to_vec();
    over.push((crate::flow_extra::potion::KEY, crate::flow_extra::potion::ON));
    over.push((crate::slice::skull_rules::KEY, crate::slice::skull_rules::ON));
    let w: World = crate::slice::logic::world_with(d, &over)?;
    let mut locs: BS<String> = w.real.iter().map(|l| w.locs[*l].name.clone()).collect();
    locs.extend(crate::shops::data::table()?.names.iter().cloned());
    let mut items: BS<String> = d.items.iter().map(|i| i.name.clone()).collect();
    items.extend(w.items.names.iter().cloned());
    Ok(Json::obj()
        .with("format", Json::Str("ootrr-ap-names".to_string()))
        .with("version", Json::Int(VERSION))
        .with("locations", Json::strs(locs.iter().map(|x| x.as_str())))
        .with("items", Json::strs(items.iter().map(|x| x.as_str()))))
}

fn roll_value(rng: &mut Rng, s: &crate::settings::model::Setting) -> Option<crate::settings::model::Value> {
    use crate::settings::model::{Kind, Value};
    match s.kind {
        Kind::Toggle => Some(Value::Bool(rng.below(2) == 1)),
        Kind::Choice if !s.choices.is_empty() => Some(Value::s(&s.choices[rng.below(s.choices.len() as u64) as usize].id)),
        Kind::Number if s.max >= s.min => {
            let step: i64 = s.step.max(1);
            let n: i64 = (s.max - s.min) / step;
            Some(Value::Int(s.min + step * rng.below(n as u64 + 1) as i64))
        }
        Kind::Multi if !s.choices.is_empty() => Some(Value::Set(s.choices.iter().filter(|_| rng.below(2) == 1).map(|c| c.id.clone()).collect())),
        _ => None,
    }
}

pub fn roll(rng: &mut Rng, base: &Settings, k: usize) -> Option<Settings> {
    let all: Vec<&crate::settings::model::Setting> = crate::settings::schema().settings.iter().filter(|s| !s.legacy).collect();
    let mut s: Settings = base.clone();
    for _ in 0..k {
        let one = all[rng.below(all.len() as u64) as usize];
        if let Some(v) = roll_value(rng, one) {
            let _ = s.set(&one.id, v);
        }
    }
    s.finish().ok()
}

fn probes(w: &World, free: &[LocId], fixed: &Placement, pool: &[ItemId], n: usize) -> Json {
    let mut rng: Rng = Rng::from_text("ootrr archipelago probes");
    let mut out: Vec<Json> = Vec::with_capacity(n);
    for k in 0..n {
        let keep: u64 = if k + 1 == n { 100 } else { (k as u64 * 100) / n.max(1) as u64 };
        let held: Vec<ItemId> = pool.iter().copied().filter(|_| rng.below(100) < keep).collect();
        let s: Search = explore(w, fixed, &held);
        let reach: Vec<Json> = free.iter().filter(|l| s.loc_access(**l)).map(|l| int(*l)).collect();
        out.push(Json::obj().with("held", ids(&held)).with("reach", Json::Arr(reach)).with("won", Json::Bool(s.won())));
    }
    Json::Arr(out)
}

pub fn export_with(d0: &Data, s: &Settings, preset: &str, seed: &str, n: usize) -> Res<(Json, Json)> {
    let Prep { p, lay: _, owned, w, vanilla, pk, gates, base } = prepare(d0, s, seed, &[])?;
    let d: &Data = owned.as_ref().unwrap_or(d0);
    if gates.is_some() {
        return Err(E::Format("archipelago: entrance shuffle is not supported yet"));
    }
    let warnings: Vec<String> = Vec::new();
    let lacs: Vec<(LocId, ItemId)> = crate::lacs_key::forced(&w, &p, &pk.free, &base, &[])?;
    let free: Vec<LocId> = pk.free.clone();
    let mut pool: Vec<ItemId> = Vec::with_capacity(free.len());
    for l in &free {
        pool.push(base.get(*l).ok_or_else(|| E::Msg(format!("archipelago: the shuffled location {} holds no pool item", w.locs[*l].name)))?);
    }
    let kinds: BS<ItemId> = pool.iter().copied().collect();
    let kinds: Vec<ItemId> = kinds.into_iter().collect();
    let mut rows: Vec<Json> = Vec::with_capacity(free.len());
    let mut loose: usize = 0;
    let det: Detect = Detect::load()?;
    let shared: BS<String> = det.shared(d);
    let placed: std::collections::BTreeMap<String, String> = free.iter().map(|l| (w.locs[*l].name.clone(), String::new())).collect();
    let mut blind: Vec<String> = Vec::new();
    for l in &free {
        let name: &str = &w.locs[*l].name;
        let fixed = |i: ItemId| {
            let item: &str = &w.items.names[i];
            d.family(item).is_some_and(|f| crate::slice::tier_order::give_of(d, &placed, name, item, f) != crate::slice::tier_order::Give::Flow)
        };
        let deny: Vec<ItemId> = kinds.iter().copied().filter(|i| !allowed(d, &w, &p, *i, *l) || fixed(*i)).collect();
        if deny.is_empty() {
            loose += 1;
        }
        let here: Option<&String> = w.regions[w.locs[*l].region].dungeon.as_ref();
        let (mut flags, mut keys): (BS<String>, BS<String>) = det.of(d, name);
        flags.retain(|x| !shared.contains(x));
        keys.retain(|x| !shared.contains(x));
        if flags.is_empty() && keys.is_empty() {
            blind.push(name.to_string());
        }
        rows.push(
            Json::obj()
                .with("loc", int(*l))
                .with("kind", Json::Str(kind_of(d, name)))
                .with("dungeon", opt(&here.cloned()))
                .with("vanilla", vanilla.get(*l).map(|i| Json::Str(w.items.names[i].clone())).unwrap_or(Json::Null))
                .with("foreign", Json::Bool(foreign_ok(d, name)))
                .with("deny", ids(&deny))
                .with("flags", Json::strs(flags.iter().map(|x| x.as_str())))
                .with("keys", Json::strs(keys.iter().map(|x| x.as_str()))),
        );
    }
    let mut fixed_place: Placement = Placement::new(w.locs.len());
    for l in w.real.iter().filter(|l| !free.contains(l)) {
        if let Some(i) = vanilla.get(*l) {
            fixed_place.put(*l, i);
        }
    }
    let checks: Json = probes(&w, &free, &fixed_place, &pool, n);
    let fixed: Vec<Json> = w
        .real
        .iter()
        .filter(|l| !free.contains(l))
        .filter_map(|l| vanilla.get(*l).map(|i| Json::Arr(vec![int(*l), int(i)])))
        .collect();
    let items: Vec<Json> = kinds
        .iter()
        .map(|i| {
            let name: &str = &w.items.names[*i];
            let it = d.item(name);
            Json::obj()
                .with("id", int(*i))
                .with("kind", Json::Str(it.map(|x| x.kind.clone()).unwrap_or_default()))
                .with("dungeon", opt(&it.and_then(|x| x.dungeon.clone())))
                .with("junk", Json::Bool(crate::slice::place_rules::junk(d, name)))
                .with("local", Json::Bool(local_only(d, &p, name)))
        })
        .collect();
    Ok((Json::obj()
        .with("format", Json::Str(FORMAT.to_string()))
        .with("version", Json::Int(VERSION))
        .with("game", Json::Str(GAME.to_string()))
        .with("preset", Json::Str(preset.to_string()))
        .with("settings", Json::Str(s.encode()?))
        .with("seed", Json::Str(seed.to_string()))
        .with("world", world_json(&w))
        .with("free", Json::Arr(rows))
        .with("pool", ids(&pool))
        .with("pool_items", Json::Arr(items))
        .with("fixed", Json::Arr(fixed))
        .with("forced", Json::Arr(lacs.iter().map(|(l, i)| Json::Arr(vec![int(*l), int(*i)])).collect()))
        .with("loose", int(loose))
        .with("blind", Json::strs(blind.iter().map(|x| x.as_str())))
        .with("warnings", Json::strs(warnings.iter().map(|x| x.as_str())))
        .with("notes", Json::strs(p.notes.iter().map(|x| x.as_str()))), checks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slice::generate::{generate_data, GenOpts};

    fn arr<'a>(j: &'a Json, k: &str) -> &'a [Json] {
        j.get(k).and_then(|v| v.as_arr()).unwrap()
    }

    #[test]
    fn the_default_bundle_matches_what_the_generator_shuffles() {
        let d: Data = Data::load().unwrap();
        let s: Settings = Settings::default();
        let b: Json = export(&d, &s, "default").unwrap();
        let w: &Json = b.get("world").unwrap();
        let locs: &[Json] = arr(w, "locs");
        let names: Vec<String> = arr(&b, "free").iter().map(|r| locs[r.get("loc").unwrap().as_i64().unwrap() as usize].get("name").unwrap().as_str().unwrap().to_string()).collect();
        let g = generate_data(&d, &s, SEED, &GenOpts::default()).unwrap();
        let mut want: Vec<String> = g.seed_file.placements.keys().cloned().collect();
        let mut got: Vec<String> = names.clone();
        want.sort();
        got.sort();
        assert_eq!(got, want);
        let items: &[Json] = arr(w, "items");
        let mut pool: Vec<String> = arr(&b, "pool").iter().map(|i| items[i.as_i64().unwrap() as usize].get("name").unwrap().as_str().unwrap().to_string()).collect();
        let mut placed: Vec<String> = g.seed_file.placements.values().cloned().collect();
        pool.sort();
        placed.sort();
        assert_eq!(pool, placed);
    }

    fn row<'a>(b: &'a Json, name: &str) -> &'a Json {
        let locs: &[Json] = arr(b.get("world").unwrap(), "locs");
        arr(b, "free").iter().find(|r| locs[r.get("loc").unwrap().as_i64().unwrap() as usize].get("name").and_then(|v| v.as_str()) == Some(name)).unwrap()
    }

    fn local(b: &Json, name: &str) -> bool {
        let items: &[Json] = arr(b.get("world").unwrap(), "items");
        let id: i64 = items.iter().position(|i| i.get("name").and_then(|v| v.as_str()) == Some(name)).unwrap() as i64;
        arr(b, "pool_items").iter().find(|m| m.get("id").and_then(|v| v.as_i64()) == Some(id)).and_then(|m| m.get("local")).and_then(|v| v.as_bool()).unwrap()
    }

    fn preset(id: &str) -> Settings {
        crate::settings::presets::builtin().unwrap().into_iter().find(|p| p.id == id).unwrap().apply(&Settings::default()).unwrap()
    }

    /// A song received from another world set its own song spot's get flag, so the client sent
    /// that spot's check without it being done. Item get flags are not used to see a location.
    #[test]
    fn no_location_is_seen_through_an_item_get_flag() {
        let d: Data = Data::load().unwrap();
        let s: Settings = preset("full_shuffle");
        let b: Json = export(&d, &s, "full_shuffle").unwrap();
        for r in arr(&b, "free") {
            for f in arr(r, "flags") {
                assert!(location_flag(f.as_str().unwrap()), "{f:?}");
            }
        }
        let blind: Vec<&str> = arr(&b, "blind").iter().filter_map(|x| x.as_str()).collect();
        assert!(blind.contains(&"HC Malon Egg"));
        assert_eq!(row(&b, "KF Midos Top Left Chest").get("flags").and_then(|v| v.as_arr()).map(|v| v.len()), Some(1));
    }

    #[test]
    fn jail_keys_are_not_seen_through_their_own_actor_flag() {
        let d: Data = Data::load().unwrap();
        let det: Detect = Detect::load().unwrap();
        for n in ["Hideout 1 Torch Jail Gerudo Key", "Hideout 4 Torches Jail Gerudo Key"] {
            assert!(det.of(&d, n).0.is_empty(), "{n}");
        }
        assert_eq!(det.of(&d, "Bottom of the Well Freestanding Key").0.into_iter().collect::<Vec<_>>(), vec!["BottomOfTheWell_Level_Room_03_Get_SmallKey".to_string()]);
    }

    /// The catalog had 24 Gold Skulltulas' hashes swapped in cycles (Fire Temple GS Boulder Maze
    /// held Scarecrow Top's), so killing one sent another's check. A Skulltula is now seen through
    /// the hash the build uses (crate::slice::skull_names), and every Skulltula keeps its signals.
    #[test]
    fn every_skulltula_is_seen_through_the_hash_the_build_uses() {
        let d: Data = Data::load().unwrap();
        let det: Detect = Detect::load().unwrap();
        let shared: BS<String> = det.shared(&d);
        let skulls: Vec<&str> = d.cat.locs.iter().map(|l| l.name.as_str()).filter(|n| crate::slice::skulls::is_skull(&d, n)).collect();
        assert!(skulls.len() >= 90, "{}", skulls.len());
        for n in skulls {
            let want: Vec<String> = crate::slice::skull_names::hashes(&d, n).iter().map(|h| h.to_string()).collect();
            let (f, k) = det.of(&d, n);
            for h in &want {
                assert!(k.contains(h) && !shared.contains(h), "{n}: {h} missing or shared");
                assert!(f.contains(&format!("{SKULL_FLAG}{h}")), "{n}: no flag for {h}");
            }
        }
        let (_, k) = det.of(&d, "Fire Temple GS Boulder Maze");
        assert!(!k.contains("9172014785497046138"), "Boulder Maze still watches Scarecrow Top's hash");
    }

    /// Breaking one Barinade pot sent its neighbours' checks too: they shared map hashes.
    #[test]
    fn no_two_locations_share_a_flag_or_key() {
        let d: Data = Data::load().unwrap();
        for id in ["default", "full_shuffle"] {
            let b: Json = export(&d, &preset(id), id).unwrap();
            let locs: &[Json] = arr(b.get("world").unwrap(), "locs");
            let mut owner: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            for r in arr(&b, "free") {
                let name: &str = locs[r.get("loc").unwrap().as_i64().unwrap() as usize].get("name").unwrap().as_str().unwrap();
                for x in arr(r, "flags").iter().chain(arr(r, "keys")) {
                    let x: String = x.as_str().unwrap().to_string();
                    if let Some(o) = owner.insert(x.clone(), name.to_string()) {
                        panic!("{id}: {o} and {name} both use {x}");
                    }
                }
            }
        }
        let b: Json = export(&d, &preset("full_shuffle"), "full_shuffle").unwrap();
        let blind: Vec<&str> = arr(&b, "blind").iter().filter_map(|x| x.as_str()).collect();
        assert!(blind.contains(&"Jabu Jabus Belly Barinade Pot 1"));
        assert!(!blind.contains(&"Jabu Jabus Belly Barinade Pot 6"));
    }

    #[test]
    fn keys_set_to_anywhere_may_go_to_other_worlds() {
        let d: Data = Data::load().unwrap();
        let ks: Json = export(&d, &preset("keysanity"), "keysanity").unwrap();
        assert!(!local(&ks, "Small Key (Forest Temple)"));
        let def: Json = export(&d, &preset("default"), "default").unwrap();
        assert!(local(&def, "Small Key (Forest Temple)"));
    }

    #[test]
    fn rolled_settings_only_shuffle_named_locations_and_items() {
        let d: Data = Data::load().unwrap();
        let n: Json = names(&d).unwrap();
        let locs: BS<&str> = arr(&n, "locations").iter().filter_map(|x| x.as_str()).collect();
        let items: BS<&str> = arr(&n, "items").iter().filter_map(|x| x.as_str()).collect();
        let presets = crate::settings::presets::builtin().unwrap();
        let mut rng: Rng = Rng::from_text("ootrr archipelago names");
        let mut done: usize = 0;
        for k in 0..40 {
            let base: Settings = presets[k % presets.len()].apply(&Settings::default()).unwrap();
            let Some(s) = roll(&mut rng, &base, 12) else { continue };
            let Ok(b) = export(&d, &s, "custom") else { continue };
            let w: &Json = b.get("world").unwrap();
            let wl: &[Json] = arr(w, "locs");
            let wi: &[Json] = arr(w, "items");
            for r in arr(&b, "free") {
                let name: &str = wl[r.get("loc").unwrap().as_i64().unwrap() as usize].get("name").unwrap().as_str().unwrap();
                assert!(locs.contains(name), "{name} is not in the names list");
            }
            for i in arr(&b, "pool") {
                let name: &str = wi[i.as_i64().unwrap() as usize].get("name").unwrap().as_str().unwrap();
                assert!(items.contains(name), "{name} is not in the names list");
            }
            done += 1;
        }
        assert!(done >= 10, "only {done} rolled settings exported");
    }

    /// Song from Royal Familys Tomb is a sale spot without the tier wrapper, so the build gives
    /// an upgrade there one fixed tier and refused the slot when the order did not hold. The
    /// export now keeps upgrades off every spot the build would give a fixed tier.
    #[test]
    fn upgrades_stay_off_every_spot_that_gives_a_fixed_tier() {
        let d: Data = Data::load().unwrap();
        let mut s: Settings = preset("default");
        s.set("shuffle_songs", crate::settings::model::Value::s("anywhere")).unwrap();
        let b: Json = export(&d, &s.finish().unwrap(), "custom").unwrap();
        let items: &[Json] = arr(b.get("world").unwrap(), "items");
        let deny: Vec<&str> = arr(row(&b, "Song from Royal Familys Tomb"), "deny").iter().map(|i| items[i.as_i64().unwrap() as usize].get("name").unwrap().as_str().unwrap()).collect();
        assert!(deny.contains(&"Bomb Bag"), "{deny:?}");
        let chest: Vec<&str> = arr(row(&b, "KF Midos Top Left Chest"), "deny").iter().map(|i| items[i.as_i64().unwrap() as usize].get("name").unwrap().as_str().unwrap()).collect();
        assert!(!chest.contains(&"Bomb Bag"));
    }
}
