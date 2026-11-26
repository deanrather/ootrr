use std::collections::{BTreeMap as BM, BTreeSet as BS};
use std::fs;
use std::io::Read;
use std::path::{Path as P, PathBuf as PB};

use crate::byml::{Byml, Node};
use crate::collapse;
use crate::colors::{self, Colors};
use crate::error::{Error as E, Res};
use crate::rng::Rng;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst};
use crate::sfx::{self, Sfx};
use crate::silence;
use crate::slink::{Link, SLINK};
use crate::zs;

pub const BGM_FILE: &str = "Sound/Bgm/bgm.Product.111.byml.zs";
const STREAM: &str = "Sound/Resource/Stream";
const MARGIN: u32 = 0x400;
const UNSHUFFLED: [&str; 2] = ["BGM_DungeonBoss_Ganon", "BGM_EscapeFromGanonTower"];
const JINGLES: [&str; 8] = [
    "BGM_Appear",
    "BGM_FinaleJingle",
    "BGM_GameOver",
    "BGM_ItemGet_FromZelda",
    "BGM_MedalGet",
    "BGM_SpiritStoneGet",
    "BGM_SleepInBed",
    "BGM_SleepInBedToEnemyMuseum",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Music {
    Off,
    Grouped,
    Anywhere,
    Silent,
}

impl Music {
    pub fn parse(s: &str) -> Option<Music> {
        match s {
            "off" => Some(Music::Off),
            "grouped" => Some(Music::Grouped),
            "anywhere" => Some(Music::Anywhere),
            "none" => Some(Music::Silent),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Music::Off => "off",
            Music::Grouped => "grouped",
            Music::Anywhere => "anywhere",
            Music::Silent => "none",
        }
    }

    pub fn shuffles(self) -> bool {
        matches!(self, Music::Grouped | Music::Anywhere)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cosmetics {
    pub music: Music,
    pub sfx: Sfx,
    pub colors: Colors,
    pub seed: String,
    pub title: bool,
    /// Colour picks for single parts, by part id. A part not listed follows `colors`.
    pub parts: colors::Picks,
}

impl Default for Cosmetics {
    fn default() -> Cosmetics {
        Cosmetics { music: Music::Off, sfx: Sfx::Off, colors: Colors::Off, seed: String::new(), title: true, parts: colors::Picks::new() }
    }
}

impl Cosmetics {
    pub fn any(&self) -> bool {
        self.music != Music::Off || self.sfx != Sfx::Off || !colors::plan(self.colors, "", &self.parts).is_empty()
    }

    /// Whether anything in these cosmetics is drawn at random, so the cosmetic seed matters.
    pub fn rolls(&self) -> bool {
        let drawn = |i: &colors::Item| match self.parts.get(i.id) {
            None => matches!(self.colors, Colors::Random | Colors::Shared),
            Some(p) => *p == colors::Pick::Random && self.colors != Colors::Off,
        };
        self.music.shuffles() || self.sfx != Sfx::Off || colors::ITEMS.iter().any(drawn)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Theme,
    Boss,
    Battle,
    Jingle,
}

pub fn is_jingle(name: &str) -> bool {
    JINGLES.contains(&name) || ["Jingle", "ItemGet", "HeartGet", "ReadRiddle"].iter().any(|w| name.contains(w))
}

fn kind(name: &str) -> Option<Kind> {
    if name.starts_with("BGM_Dm_") || UNSHUFFLED.contains(&name) {
        return None;
    }
    if is_jingle(name) {
        return Some(Kind::Jingle);
    }
    if name.starts_with("BGM_DungeonBoss") || name == "BGM_MiddleBoss" {
        return Some(Kind::Boss);
    }
    if name == "BGM_EnemyBattle" || name == "BGM_FishingBattle" {
        return Some(Kind::Battle);
    }
    Some(Kind::Theme)
}

fn asset_key(e: &Node, have: &dyn Fn(&str) -> bool) -> Option<String> {
    let nodes: &[Node] = e.get("Node")?.as_arr()?;
    let mut keys: Vec<&str> = Vec::new();
    for n in nodes {
        match n.get("Type").and_then(|t| t.as_str()) {
            Some("Fork") => return None,
            Some("Asset") => keys.push(n.get("Key")?.as_str()?),
            _ => {}
        }
    }
    if keys.len() == 1 {
        return Some(keys[0].to_string());
    }
    keys.iter().find(|k| !side(k) && have(k)).map(|k| k.to_string())
}

fn side(k: &str) -> bool {
    ["Intro", "Outro", "Pad", "Roar", "Demo"].iter().any(|w| k.contains(w))
}

type Event = (String, String, Kind, bool);

fn events(y: &Byml, have: &dyn Fn(&str) -> bool, loops: &dyn Fn(&str) -> bool) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::new();
    let Some(Node::Map(all)) = y.root.get("BGM") else {
        return out;
    };
    let one: BM<String, String> = one_track(all, have);
    for (name, e) in all {
        let area: Option<(&str, bool)> = collapse::of(name).map(|(a, ambience)| (a.key, ambience));
        let (Some(k), Some(key)) = (kind(name), area.map(|a| a.0.to_string()).or_else(|| one.get(name).cloned()).or_else(|| asset_key(e, have))) else {
            continue;
        };
        if have(&key) {
            let gives: bool = !area.is_some_and(|a| a.1) && (k == Kind::Jingle || loops(&key));
            out.push((name.clone(), key, k, gives));
        }
    }
    out
}

fn one_track(all: &[(String, Node)], have: &dyn Fn(&str) -> bool) -> BM<String, String> {
    let mut by: BM<&str, Vec<(&str, &Node)>> = BM::new();
    for (name, e) in all.iter().filter(|(name, _)| kind(name).is_some()) {
        by.entry(user(name, e)).or_default().push((name, e));
    }
    let mut out: BM<String, String> = BM::new();
    for (shared, events) in by {
        if events.len() < 2 || events.iter().any(|x| x.0 == shared) {
            continue;
        }
        let Some(key) = events.iter().flat_map(|x| node_keys(x.1)).filter(|k| !side(k) && have(k)).min() else {
            continue;
        };
        out.extend(events.iter().map(|x| (x.0.to_string(), key.to_string())));
    }
    out
}

fn stems(e: &Node, have: &dyn Fn(&str) -> bool) -> Option<Vec<String>> {
    let nodes: &[Node] = e.get("Node")?.as_arr()?;
    let mut fork: bool = false;
    let mut keys: BS<String> = BS::new();
    for n in nodes {
        match n.get("Type").and_then(|t| t.as_str()) {
            Some("Fork") => fork = true,
            Some("Asset") => {
                keys.insert(n.get("Key")?.as_str()?.to_string());
            }
            _ => {}
        }
    }
    (fork && keys.len() > 1 && keys.iter().all(|k| have(k))).then(|| keys.into_iter().collect())
}

fn layered(y: &Byml, have: &dyn Fn(&str) -> bool) -> Vec<(String, Vec<String>, Kind)> {
    let mut out: Vec<(String, Vec<String>, Kind)> = Vec::new();
    let Some(Node::Map(all)) = y.root.get("BGM") else {
        return out;
    };
    let one: BM<String, String> = one_track(all, have);
    for (name, e) in all.iter().filter(|(name, _)| !one.contains_key(name)) {
        if let (Some(k), Some(set)) = (kind(name), stems(e, have)) {
            out.push((name.clone(), set, k));
        }
    }
    out
}

fn slot(m: Music, k: Kind) -> u8 {
    match (m, k) {
        (Music::Anywhere, Kind::Jingle) => 9,
        (Music::Anywhere, _) => 0,
        (_, k) => k as u8,
    }
}

fn deal<T: Clone + Ord>(from: &BS<T>, give: &BS<T>, rng: &mut Rng) -> BM<T, T> {
    let from: Vec<T> = from.iter().cloned().collect();
    let give: Vec<T> = give.iter().cloned().collect();
    let mut to: Vec<T> = Vec::with_capacity(from.len());
    while !give.is_empty() && to.len() < from.len() {
        let mut round: Vec<T> = give.clone();
        rng.shuffle(&mut round);
        to.extend(round.into_iter().take(from.len() - to.len()));
    }
    rng.shuffle(&mut to);
    for i in 0..to.len() {
        if to[i] != from[i] {
            continue;
        }
        if let Some(j) = (0..to.len()).find(|&j| to[j] != from[i] && to[i] != from[j]) {
            to.swap(i, j);
        } else if let Some(other) = give.iter().find(|g| **g != from[i]) {
            to[i] = other.clone();
        }
    }
    from.into_iter().zip(to).collect()
}

fn plan_layers(ev: &[(String, Vec<String>, Kind)], m: Music, rng: &mut Rng, loops: &dyn Fn(&str) -> bool) -> BM<String, BM<String, String>> {
    let mut by: BM<u8, (BS<Vec<String>>, BS<Vec<String>>)> = BM::new();
    for (name, set, k) in ev {
        let pool: &mut (BS<Vec<String>>, BS<Vec<String>>) = by.entry(slot(m, *k)).or_default();
        if collapse::of(name).is_none() {
            pool.0.insert(set.clone());
        }
        if set.iter().all(|key| loops(key)) {
            pool.1.insert(set.clone());
        }
    }
    let mut swap: BM<Vec<String>, Vec<String>> = BM::new();
    for (from, give) in by.into_values() {
        swap.extend(deal(&from, &give, rng));
    }
    let mut out: BM<String, BM<String, String>> = BM::new();
    for (name, set, _) in ev.iter().filter(|x| collapse::of(&x.0).is_none()) {
        let Some(new) = swap.get(set).filter(|n| *n != set) else {
            continue;
        };
        let map: BM<String, String> = set.iter().enumerate().map(|(i, k)| (k.clone(), new[i % new.len()].clone())).collect();
        out.insert(name.clone(), map);
    }
    out
}

fn rewrite_layers(y: &mut Byml, to: &BM<String, BM<String, String>>) -> usize {
    let mut n: usize = 0;
    let Some(Node::Map(all)) = y.root.get_mut("BGM") else {
        return n;
    };
    for (name, e) in all.iter_mut() {
        let Some(map) = to.get(name) else {
            continue;
        };
        let Some(nodes) = e.get_mut("Node").and_then(|x| x.as_arr_mut()) else {
            continue;
        };
        for node in nodes.iter_mut() {
            if node.get("Type").and_then(|t| t.as_str()) != Some("Asset") {
                continue;
            }
            let Some(new) = node.get("Key").and_then(|k| k.as_str()).and_then(|k| map.get(k)).cloned() else {
                continue;
            };
            if node.set("Key", Node::Str(new)) {
                n += 1;
            }
        }
    }
    n
}

fn plan(ev: &[Event], m: Music, rng: &mut Rng) -> BM<String, (String, String)> {
    let mut by: BM<u8, (BS<String>, BS<String>)> = BM::new();
    for (_, key, k, gives) in ev {
        let pool: &mut (BS<String>, BS<String>) = by.entry(slot(m, *k)).or_default();
        pool.0.insert(key.clone());
        if *gives {
            pool.1.insert(key.clone());
        }
    }
    let mut swap: BM<String, String> = BM::new();
    for (from, give) in by.into_values() {
        swap.extend(deal(&from, &give, rng));
    }
    let mut out: BM<String, (String, String)> = BM::new();
    for (name, key, _, _) in ev {
        if let Some(n) = swap.get(key).filter(|n| *n != key) {
            out.insert(name.clone(), (key.clone(), n.clone()));
        }
    }
    out
}

fn rewrite(y: &mut Byml, to: &BM<String, (String, String)>) -> usize {
    let mut n: usize = 0;
    let Some(Node::Map(all)) = y.root.get_mut("BGM") else {
        return n;
    };
    for (name, e) in all.iter_mut() {
        let Some((old, new)) = to.get(name) else {
            continue;
        };
        let area: bool = collapse::of(name).is_some();
        let Some(nodes) = e.get_mut("Node").and_then(|x| x.as_arr_mut()) else {
            continue;
        };
        for node in nodes.iter_mut() {
            if node.get("Type").and_then(|t| t.as_str()) != Some("Asset") {
                continue;
            }
            let Some(k) = node.get("Key").and_then(|k| k.as_str()) else {
                continue;
            };
            if (k == old || (!area && !side(k))) && node.set("Key", Node::Str(new.clone())) {
                n += 1;
            }
        }
    }
    n
}

pub fn shuffle_music(y: &mut Byml, m: Music, seed: &str, have: &dyn Fn(&str) -> bool, loops: &dyn Fn(&str) -> bool) -> usize {
    if !m.shuffles() {
        return 0;
    }
    let ev: Vec<Event> = events(y, have, loops);
    let mut rng: Rng = Rng::from_text(&format!("{seed} music {}", m.id()));
    let to: BM<String, (String, String)> = plan(&ev, m, &mut rng);
    let lay: Vec<(String, Vec<String>, Kind)> = layered(y, have);
    let sets: BM<String, BM<String, String>> = plan_layers(&lay, m, &mut rng, loops);
    rewrite(y, &to) + rewrite_layers(y, &sets)
}

const SEED_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// A new cosmetic seed, different on every call: eight letters and digits.
pub fn fresh_seed() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(if cfg!(target_os = "wasi") { 0 } else { std::process::id() });
    let mut rng: Rng = Rng::from_seed(h.finish());
    (0..8).map(|_| SEED_CHARS[rng.below(SEED_CHARS.len() as u64) as usize] as char).collect()
}

/// Applies the cosmetics. An empty cosmetic seed draws a new one for this build, so each
/// build gets new music and colours; the notes name it so it can be typed in to repeat them.
pub fn apply(mod_root: &P, romfs: &P, c: &Cosmetics) -> Res<Vec<String>> {
    let given: &str = c.seed.trim();
    let text: String = if given.is_empty() { fresh_seed() } else { given.to_string() };
    let mut notes: Vec<String> = apply_music(mod_root, romfs, c, &text)?;
    notes.extend(sfx::apply(mod_root, romfs, c.sfx, &text)?);
    notes.extend(colors::apply(mod_root, romfs, c.colors, &c.parts, &text)?);
    if c.rolls() && given.is_empty() {
        notes.push(format!("Cosmetic seed: {text}. Type it in the Cosmetic seed box to build these music and colours again."));
    }
    Ok(notes)
}

fn user<'a>(name: &'a str, e: &'a Node) -> &'a str {
    e.get("SLinkUserName").and_then(|u| u.as_str()).unwrap_or(name)
}

fn node_keys(e: &Node) -> impl Iterator<Item = &str> {
    e.get("Node").and_then(|n| n.as_arr()).unwrap_or(&[]).iter().filter_map(|n| n.get("Key").and_then(|k| k.as_str()))
}

pub fn moves(old: &Byml, new: &Byml) -> BM<(String, String), String> {
    let mut out: BM<(String, String), String> = BM::new();
    let (Some(Node::Map(a)), Some(Node::Map(b))) = (old.root.get("BGM"), new.root.get("BGM")) else {
        return out;
    };
    let owned: BS<(&str, &str)> = a.iter().filter(|(name, e)| user(name, e) == name).flat_map(|(name, e)| node_keys(e).map(move |k| (name.as_str(), k))).collect();
    for ((name, e), (_, f)) in a.iter().zip(b.iter()) {
        let by: &str = user(name, e);
        let (Some(x), Some(y)) = (e.get("Node").and_then(|n| n.as_arr()), f.get("Node").and_then(|n| n.as_arr())) else {
            continue;
        };
        for (p, q) in x.iter().zip(y.iter()) {
            let (Some(k), Some(n)) = (p.get("Key").and_then(|v| v.as_str()), q.get("Key").and_then(|v| v.as_str())) else {
                continue;
            };
            if k != n && (by == name || !owned.contains(&(by, k))) {
                out.insert((by.to_string(), k.to_string()), n.to_string());
            }
        }
    }
    out
}

fn looping(file: &P) -> bool {
    let mut head: [u8; 0x50] = [0; 0x50];
    fs::File::open(file).and_then(|mut f| f.read_exact(&mut head)).is_ok() && head.starts_with(b"BWAV") && head[0x4C..] != [0xFF; 4]
}

fn put(mod_root: &P, rel: &str, data: &[u8]) -> Res<()> {
    let out: PB = mod_root.join("romfs").join(rel);
    if let Some(d) = out.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(&out, data)?;
    Ok(())
}

fn one_node(link: &Link, raw: &[u8], old: &Byml, loops: &dyn Fn(&str) -> bool) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for (a, _) in collapse::areas() {
        for &event in a.events {
            let Some(e) = old.root.get("BGM").and_then(|b| b.get(event)) else {
                continue;
            };
            let heard: Option<String> = link.calls_of(user(event, e)).into_iter().find(|c| c.key == a.key).and_then(|c| link.asset(raw, c));
            if heard.is_some_and(|h| h != a.key && loops(&h)) {
                out.push(event);
            }
        }
    }
    out
}

fn write_table(mod_root: &P, rf: &mut Romfs, events: &[&str]) -> Res<()> {
    let z: Vec<u8> = rf.read(BGM_FILE)?;
    let old: Vec<u8> = rf.zs.decompress(&z)?;
    let mut y: Byml = Byml::parse(&old)?;
    for event in events {
        collapse::collapse(&mut y, event)?;
    }
    let new: Vec<u8> = y.write()?;
    put(mod_root, BGM_FILE, &zs::compress_raw(&new, zs::dict_of(&z))?)?;
    let at: PB = mod_root.join("romfs").join(rsizetable::TABLE_PATH);
    let mut t: Rst = match fs::read(&at) {
        Ok(b) if !b.is_empty() => Rst::parse(&rf.zs.decompress(&b)?)?,
        _ => rsizetable::load(rf)?,
    };
    if t.grow(BGM_FILE, old.len() as u64, new.len() as u64, MARGIN)?.is_none() {
        return Err(E::Format("music: the size table has no entry for the music table"));
    }
    put(mod_root, rsizetable::TABLE_PATH, &t.to_zs()?)
}

fn apply_music(mod_root: &P, romfs: &P, c: &Cosmetics, text: &str) -> Res<Vec<String>> {
    let mut notes: Vec<String> = Vec::new();
    if c.music == Music::Off {
        return Ok(notes);
    }
    let mut rf: Romfs = Romfs::open(romfs)?;
    if c.music == Music::Silent {
        return silence::apply(mod_root, &mut rf);
    }
    let old: Byml = rf.read_byml(BGM_FILE)?;
    let mut y: Byml = old.clone();
    let top: PB = rf.root.join(STREAM);
    let have = |k: &str| top.join(format!("{k}.bwav")).is_file();
    let loops = |k: &str| looping(&top.join(format!("{k}.bwav")));
    shuffle_music(&mut y, c.music, text, &have, &loops);
    let want: BM<(String, String), String> = moves(&old, &y);
    let z: Vec<u8> = rf.read(SLINK)?;
    let mut raw: Vec<u8> = rf.zs.decompress(&z)?;
    let link: Link = Link::read(&raw)?;
    let n: usize = link.retarget(&mut raw, &want)?;
    if n == 0 {
        return Err(E::Format("the music table has no track this shuffle can move"));
    }
    put(mod_root, SLINK, &zs::compress_raw(&raw, zs::dict_of(&z))?)?;
    let steady: Vec<&str> = one_node(&link, &raw, &old, &loops);
    write_table(mod_root, &mut rf, &steady)?;
    notes.push(format!(
        "Music is shuffled {}. {n} music tracks moved. Cutscene music keeps its own tracks. {} area themes that were built from several parts now play one looping track without restarting; those that are quiet at night stay quiet.",
        if c.music == Music::Grouped { "within each kind of track" } else { "across all tracks" },
        steady.len()
    ));
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(name: &str, key: &str) -> (String, Node) {
        let asset: Node = Node::Map(vec![("Key".to_string(), Node::Str(key.to_string())), ("Type".to_string(), Node::Str("Asset".to_string()))]);
        let start: Node = Node::Map(vec![("Type".to_string(), Node::Str("Start".to_string()))]);
        (name.to_string(), Node::Map(vec![("Node".to_string(), Node::Arr(vec![start, asset]))]))
    }

    fn table(rows: &[(&str, &str)]) -> Byml {
        let all: Vec<(String, Node)> = rows.iter().map(|r| ev(r.0, r.1)).collect();
        Byml { root: Node::Map(vec![("BGM".to_string(), Node::Map(all))]), version: 7, big: false }
    }

    fn keys(y: &Byml) -> BM<String, String> {
        let mut out: BM<String, String> = BM::new();
        if let Some(Node::Map(all)) = y.root.get("BGM") {
            for (n, e) in all {
                out.insert(n.clone(), asset_key(e, &|_| true).unwrap());
            }
        }
        out
    }

    const ROWS: [(&str, &str); 8] = [
        ("BGM_ForestTemple", "BGM_ForestTemple"),
        ("BGM_ShadowTemple", "BGM_ShadowTemple"),
        ("BGM_WaterTemple", "BGM_WaterTemple"),
        ("BGM_GoronCity", "BGM_GoronCity"),
        ("BGM_EnemyBattle", "BGM_EnemyBattle"),
        ("BGM_MiddleBoss", "BGM_MiddleBoss"),
        ("BGM_MedalGet", "BGM_MedalGet"),
        ("BGM_GameOver", "BGM_GameOver"),
    ];

    #[test]
    fn grouped_music_only_trades_tracks_of_the_same_kind() {
        let mut y: Byml = table(&ROWS);
        let n: usize = (0..40).map(|i| shuffle_music(&mut y, Music::Grouped, &format!("s{i}"), &|_| true, &|_| true)).find(|n| *n > 0).unwrap_or(0);
        assert!(n > 0);
        let k: BM<String, String> = keys(&y);
        for jingle in ["BGM_MedalGet", "BGM_GameOver"] {
            assert!(k[jingle] == "BGM_MedalGet" || k[jingle] == "BGM_GameOver", "{jingle}");
        }
        assert_eq!(k["BGM_EnemyBattle"], "BGM_EnemyBattle");
        assert_eq!(k["BGM_MiddleBoss"], "BGM_MiddleBoss");
        let mut themes: Vec<&String> = ["BGM_ForestTemple", "BGM_ShadowTemple", "BGM_WaterTemple", "BGM_GoronCity"].iter().map(|n| &k[*n]).collect();
        themes.sort();
        assert_eq!(themes, vec!["BGM_ForestTemple", "BGM_GoronCity", "BGM_ShadowTemple", "BGM_WaterTemple"]);
    }

    #[test]
    fn anywhere_music_mixes_boss_battle_and_themes_but_not_jingles() {
        let mut moved: bool = false;
        for s in 0..20 {
            let mut y: Byml = table(&ROWS);
            shuffle_music(&mut y, Music::Anywhere, &format!("seed {s}"), &|_| true, &|_| true);
            let k: BM<String, String> = keys(&y);
            assert!(k["BGM_MedalGet"] == "BGM_MedalGet" || k["BGM_MedalGet"] == "BGM_GameOver");
            moved |= k["BGM_EnemyBattle"] != "BGM_EnemyBattle";
        }
        assert!(moved);
    }

    #[test]
    fn the_same_seed_gives_the_same_music_and_off_changes_nothing() {
        let mut a: Byml = table(&ROWS);
        let mut b: Byml = table(&ROWS);
        shuffle_music(&mut a, Music::Grouped, "same", &|_| true, &|_| true);
        shuffle_music(&mut b, Music::Grouped, "same", &|_| true, &|_| true);
        assert_eq!(a, b);
        let mut c: Byml = table(&ROWS);
        assert_eq!(shuffle_music(&mut c, Music::Off, "same", &|_| true, &|_| true), 0);
        assert_eq!(c, table(&ROWS));
        assert_eq!(shuffle_music(&mut c, Music::Silent, "same", &|_| true, &|_| true), 0);
        assert_eq!(c, table(&ROWS));
    }

    #[test]
    fn cutscene_events_and_tracks_without_a_stream_file_are_left_alone() {
        let rows: [(&str, &str); 4] = [("BGM_Dm_HrlField_0030", "BGM_Kaepora"), ("BGM_ForestTemple", "BGM_ForestTemple"), ("BGM_ShadowTemple", "BGM_ShadowTemple"), ("BGM_WaterTemple", "BGM_WaterTemple")];
        let mut y: Byml = table(&rows);
        shuffle_music(&mut y, Music::Grouped, "s", &|k| k != "BGM_WaterTemple", &|_| true);
        let k: BM<String, String> = keys(&y);
        assert_eq!(k["BGM_Dm_HrlField_0030"], "BGM_Kaepora");
        assert_eq!(k["BGM_WaterTemple"], "BGM_WaterTemple");
    }

    #[test]
    fn events_that_share_a_track_keep_sharing_it() {
        let rows: [(&str, &str); 4] = [("BGM_DungeonBoss", "BGM_DungeonBoss"), ("BGM_DungeonBoss_KotakeKoume", "BGM_DungeonBoss"), ("BGM_MiddleBoss", "BGM_MiddleBoss"), ("BGM_DungeonBoss_Dinasour", "BGM_DungeonBoss_Dinasour")];
        for s in 0..10 {
            let mut y: Byml = table(&rows);
            shuffle_music(&mut y, Music::Grouped, &format!("s{s}"), &|_| true, &|_| true);
            let k: BM<String, String> = keys(&y);
            assert_eq!(k["BGM_DungeonBoss"], k["BGM_DungeonBoss_KotakeKoume"]);
        }
    }

    #[test]
    fn fanfares_are_jingles_and_a_shared_intro_is_never_moved() {
        for n in ["BGM_ItemGet_Small", "BGM_ItemGet_Large", "BGM_HeartGet", "BGM_ReadRiddle", "BGM_ReadRiddle_Small", "BGM_GameOver"] {
            assert_eq!(kind(n), Some(Kind::Jingle), "{n}");
        }
        let fan = |name: &str, key: &str| -> (String, Node) {
            let (n, mut e): (String, Node) = ev(name, key);
            let intro: Node = Node::Map(vec![("Key".to_string(), Node::Str("BGM_ItemGet_Intro".to_string())), ("Type".to_string(), Node::Str("Asset".to_string()))]);
            if let Some(Node::Arr(v)) = e.get_mut("Node") {
                v.push(intro);
            }
            (n, e)
        };
        for s in 0..20 {
            let all: Vec<(String, Node)> = vec![fan("BGM_HeartGet", "BGM_HeartGet"), fan("BGM_ItemGet_Large", "BGM_ItemGet_Large"), fan("BGM_ItemGet_Medium", "BGM_ItemGet_Medium"), ev("BGM_ForestTemple", "BGM_ForestTemple")];
            let mut y: Byml = Byml { root: Node::Map(vec![("BGM".to_string(), Node::Map(all))]), version: 7, big: false };
            shuffle_music(&mut y, Music::Grouped, &format!("f{s}"), &|_| true, &|_| true);
            let Some(Node::Map(all)) = y.root.get("BGM") else {
                panic!();
            };
            for (name, e) in all.iter().take(3) {
                let nodes: &[Node] = e.get("Node").unwrap().as_arr().unwrap();
                assert_eq!(nodes[2].get("Key").unwrap().as_str(), Some("BGM_ItemGet_Intro"), "{name}");
                assert_ne!(nodes[1].get("Key").unwrap().as_str(), Some("BGM_ForestTemple"));
            }
        }
    }

    #[test]
    fn the_modes_parse_and_round_trip_their_ids() {
        for m in [Music::Off, Music::Grouped, Music::Anywhere, Music::Silent] {
            assert_eq!(Music::parse(m.id()), Some(m));
        }
        assert_eq!(Music::parse("loud"), None);
        let quiet: Cosmetics = Cosmetics { music: Music::Silent, ..Cosmetics::default() };
        assert!(quiet.any() && !quiet.rolls());
        assert!(!Cosmetics::default().any());
        let mut c: Cosmetics = Cosmetics::default();
        c.parts.insert("boots".into(), colors::Pick::Rgb([1, 2, 3]));
        assert!(!c.any() && !c.rolls(), "picks do nothing outside the custom mode");
        c.colors = Colors::Custom;
        assert!(c.any() && !c.rolls());
        c.parts.insert("boots".into(), colors::Pick::Random);
        assert!(c.any() && c.rolls());
    }

    #[test]
    fn a_fresh_cosmetic_seed_is_new_each_time_and_easy_to_type() {
        let a: String = fresh_seed();
        let b: String = fresh_seed();
        assert_ne!(a, b);
        assert_eq!(a.len(), 8);
        assert!(a.bytes().all(|x| SEED_CHARS.contains(&x)));
    }

    fn real() -> Option<PB> {
        let p: PB = std::env::var("OOTRR_ROMFS").map(PB::from).ok()?;
        p.join("Pack").join("ZsDic.pack.zs").is_file().then_some(p)
    }

    #[test]
    fn the_real_link_plays_the_shuffled_tracks_and_two_seeds_differ() {
        let Some(root) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let old: Byml = rf.read_byml(BGM_FILE).unwrap();
        let vanilla: Vec<u8> = rf.read_zs(SLINK).unwrap();
        let bars: Vec<u8> = rf.read_zs("Sound/Resource/BGM_Static.bars.zs").unwrap();
        let in_bank: BS<u32> = sfx::entries(&bars).unwrap().into_iter().map(|e| e.0).collect();
        let top: PB = root.join(STREAM);
        let mut outs: Vec<Vec<u8>> = Vec::new();
        for text in ["first", "second"] {
            let out: PB = std::env::temp_dir().join(format!("ootrr_music_{}_{text}", std::process::id()));
            let _ = fs::remove_dir_all(&out);
            let c: Cosmetics = Cosmetics { music: Music::Grouped, sfx: Sfx::Off, colors: Colors::Off, seed: text.to_string(), title: true, parts: colors::Picks::new() };
            let notes: Vec<String> = apply(&out, &root, &c).unwrap();
            assert!(notes[0].contains("music tracks moved"), "{notes:?}");
            assert!(out.join("romfs").join(BGM_FILE).is_file());
            assert!(out.join("romfs").join(crate::rsizetable::TABLE_PATH).is_file());
            let raw: Vec<u8> = rf.zs.decompress(&fs::read(out.join("romfs").join(SLINK)).unwrap()).unwrap();
            assert_eq!(raw.len(), vanilla.len());
            let mut y: Byml = old.clone();
            shuffle_music(&mut y, Music::Grouped, text, &|k| top.join(format!("{k}.bwav")).is_file(), &|k| looping(&top.join(format!("{k}.bwav"))));
            let want: BM<(String, String), String> = moves(&old, &y);
            let link: Link = Link::read(&raw).unwrap();
            let before: Link = Link::read(&vanilla).unwrap();
            let (mut moved, mut kept): (usize, usize) = (0, 0);
            let bgm: BS<u32> = match old.root.get("BGM") {
                Some(Node::Map(m)) => m.iter().map(|e| crate::rsizetable::hash(&e.0)).collect(),
                _ => BS::new(),
            };
            for (c, d) in link.calls.iter().zip(before.calls.iter()).filter(|x| bgm.contains(&x.0.user)) {
                let (now, was): (String, String) = (link.asset(&raw, c).unwrap_or_default(), before.asset(&vanilla, d).unwrap_or_default());
                if now != was {
                    moved += 1;
                    assert!(in_bank.contains(&crate::rsizetable::hash(&now)), "{now}");
                    let sharing: BS<u32> = link.calls.iter().filter(|x| x.block == c.block).map(|x| x.user).collect();
                    let users: Vec<&String> = want.iter().filter(|((u, k), n)| sharing.contains(&crate::rsizetable::hash(u)) && *k == c.key && **n == now).map(|x| &x.0 .0).collect();
                    assert!(!users.is_empty(), "{} {now}", c.key);
                } else if want.keys().any(|(u, k)| crate::rsizetable::hash(u) == c.user && *k == c.key) {
                    kept += 1;
                }
            }
            println!("{text}: {moved} calls play another track, {kept} wanted moves left out because a shared call disagrees");
            assert!(moved >= 70, "{moved}");
            let diff: usize = raw.iter().zip(vanilla.iter()).filter(|(a, b)| a != b).count();
            assert!(diff <= moved * 8);
            let kokiri: Vec<String> = link.calls_of("BGM_KokiriForest").iter().filter(|c| c.key == "BGM_Kokiri_Daytime_01").map(|c| link.asset(&raw, c).unwrap()).collect();
            println!("{text}: Kokiri Forest day plays {kokiri:?}");
            outs.push(raw);
            let _ = fs::remove_dir_all(&out);
        }
        assert_ne!(outs[0], outs[1]);
    }

    #[test]
    fn every_real_track_can_move_except_the_listed_ones() {
        let Some(root) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let old: Byml = rf.read_byml(BGM_FILE).unwrap();
        let vanilla: Vec<u8> = rf.read_zs(SLINK).unwrap();
        let link: Link = Link::read(&vanilla).unwrap();
        let top: PB = root.join(STREAM);
        let have = |k: &str| top.join(format!("{k}.bwav")).is_file();
        let Some(Node::Map(all)) = old.root.get("BGM") else {
            panic!();
        };
        let mut calls: BM<(u32, String), (String, BS<String>)> = BM::new();
        for (name, e) in all {
            if kind(name).is_none() {
                continue;
            }
            let user: &str = e.get("SLinkUserName").and_then(|u| u.as_str()).unwrap();
            for node in e.get("Node").and_then(|n| n.as_arr()).unwrap_or(&[]) {
                let Some(k) = node.get("Key").and_then(|k| k.as_str()).filter(|k| have(k) && (!side(k) || collapse::of(name).is_some())) else {
                    continue;
                };
                let at: (u32, String) = (crate::rsizetable::hash(user), k.to_string());
                calls.entry(at).or_insert_with(|| (user.to_string(), BS::new())).1.insert(name.clone());
            }
        }
        calls.retain(|at, (_, events)| events.iter().any(|e| collapse::of(e).is_none_or(|(a, _)| a.key == at.1)));
        let ranch: u32 = crate::rsizetable::hash("BGM_LonLonRanch");
        for m in [Music::Grouped, Music::Anywhere] {
            let mut moved: BS<(u32, String)> = BS::new();
            for i in 0..60 {
                let mut y: Byml = old.clone();
                shuffle_music(&mut y, m, &format!("move {i}"), &have, &|k| looping(&top.join(format!("{k}.bwav"))));
                let mut raw: Vec<u8> = vanilla.clone();
                link.retarget(&mut raw, &moves(&old, &y)).unwrap();
                let heard: BS<String> = link.calls.iter().filter(|c| c.user == ranch && calls.contains_key(&(c.user, c.key.clone()))).map(|c| {
                    let now: String = link.asset(&raw, c).unwrap();
                    if now == c.key { "its own".to_string() } else { now }
                }).collect();
                assert_eq!(heard.len(), 1, "{} seed {i}: {heard:?}", m.id());
                assert!(!heard.contains("its own"), "{} seed {i}", m.id());
                for c in &link.calls {
                    if link.asset(&raw, c) != link.asset(&vanilla, c) {
                        moved.insert((c.user, c.key.clone()));
                    }
                }
            }
            let stuck: Vec<String> = calls.iter().filter(|(at, _)| !moved.contains(*at)).map(|(at, (user, events))| format!("{user}/{} {:?}", at.1, events)).collect();
            println!("{}: {} of {} calls never move", m.id(), stuck.len(), calls.len());
            for s in &stuck {
                println!("STUCK {} {s}", m.id());
            }
            assert!(stuck.is_empty(), "{} {stuck:?}", m.id());
        }
    }

    #[test]
    fn events_that_share_a_sound_user_move_its_calls_and_the_owner_wins() {
        let aliased = |name: &str, user: &str, key: &str| -> (String, Node) {
            let (n, mut e): (String, Node) = ev(name, key);
            if let Node::Map(m) = &mut e {
                m.push(("SLinkUserName".to_string(), Node::Str(user.to_string())));
            }
            (n, e)
        };
        let table_of = |keys: [&str; 4]| -> Byml {
            let all: Vec<(String, Node)> = vec![aliased("BGM_Ranch_Child", "BGM_Ranch", keys[0]), aliased("BGM_Ranch_Adult", "BGM_Ranch", keys[1]), aliased("BGM_Pond", "BGM_Town", keys[2]), aliased("BGM_Town", "BGM_Town", keys[3])];
            Byml { root: Node::Map(vec![("BGM".to_string(), Node::Map(all))]), version: 7, big: false }
        };
        let want: BM<(String, String), String> = moves(&table_of(["day", "night", "town", "town"]), &table_of(["x", "y", "pond", "town"]));
        let expect: BM<(String, String), String> = [("day", "x"), ("night", "y")].iter().map(|(k, n)| (("BGM_Ranch".to_string(), k.to_string()), n.to_string())).collect();
        assert_eq!(want, expect);
    }

    #[test]
    fn events_with_a_user_and_no_owner_trade_as_one_track() {
        let aliased = |name: &str, nodes: (String, Node)| -> (String, Node) {
            let mut e: Node = nodes.1;
            if let Node::Map(m) = &mut e {
                m.push(("SLinkUserName".to_string(), Node::Str("BGM_Ranch".to_string())));
            }
            (name.to_string(), e)
        };
        let all: Vec<(String, Node)> = vec![aliased("BGM_Ranch_Adult", ev("a", "adult_1")), aliased("BGM_Ranch_Child", layered_ev("c", &["child_1", "child_2"]))];
        let y: Byml = Byml { root: Node::Map(vec![("BGM".to_string(), Node::Map(all))]), version: 7, big: false };
        let ev: Vec<Event> = events(&y, &|_| true, &|_| true);
        assert_eq!(ev.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(), vec!["adult_1", "adult_1"]);
        assert!(layered(&y, &|_| true).is_empty());
    }

    fn layered_ev(name: &str, keys: &[&str]) -> (String, Node) {
        let mut nodes: Vec<Node> = vec![Node::Map(vec![("Type".to_string(), Node::Str("Fork".to_string()))])];
        for k in keys {
            nodes.push(Node::Map(vec![("Key".to_string(), Node::Str(k.to_string())), ("Type".to_string(), Node::Str("Asset".to_string()))]));
        }
        (name.to_string(), Node::Map(vec![("Node".to_string(), Node::Arr(nodes))]))
    }

    #[test]
    fn layered_themes_trade_whole_stem_sets() {
        let fresh = || {
            let all: Vec<(String, Node)> = vec![layered_ev("BGM_A", &["a1", "a2", "a3"]), layered_ev("BGM_B", &["b1", "b2"])];
            Byml { root: Node::Map(vec![("BGM".to_string(), Node::Map(all))]), version: 7, big: false }
        };
        let moved = (0..40).find_map(|i| {
            let mut y: Byml = fresh();
            (shuffle_music(&mut y, Music::Grouped, &format!("s{i}"), &|_| true, &|_| true) > 0).then_some(y)
        });
        let y: Byml = moved.unwrap();
        let a: Vec<String> = stems(y.root.get("BGM").unwrap().get("BGM_A").unwrap(), &|_| true).unwrap();
        assert!(a.iter().all(|k| k.starts_with('b')));
    }

    #[test]
    fn a_track_that_plays_once_is_given_a_new_one_and_never_given_away() {
        let rows: [(&str, &str); 4] = [("BGM_ForestTemple", "loop_a"), ("BGM_ShadowTemple", "loop_b"), ("BGM_WaterTemple", "once"), ("BGM_GoronCity", "loop_c")];
        for s in 0..40 {
            let mut y: Byml = table(&rows);
            shuffle_music(&mut y, Music::Grouped, &format!("s{s}"), &|_| true, &|k| k != "once");
            let k: BM<String, String> = keys(&y);
            for (name, own) in rows {
                assert!(k[name] != own && k[name] != "once", "s{s} {name} {}", k[name]);
            }
        }
    }

    struct Built {
        old: Byml,
        new: Byml,
        vanilla: Vec<u8>,
        link: Vec<u8>,
        sizes: (u64, u64, u32, u32),
        note: String,
    }

    fn built(root: &P, m: Music, seed: &str) -> Built {
        let mut rf: Romfs = Romfs::open(root).unwrap();
        let out: PB = std::env::temp_dir().join(format!("ootrr_areas_{}_{}_{seed}", std::process::id(), m.id()));
        let _ = fs::remove_dir_all(&out);
        let c: Cosmetics = Cosmetics { music: m, seed: seed.to_string(), ..Cosmetics::default() };
        let note: String = apply(&out, root, &c).unwrap().remove(0);
        let mut read = |rel: &str| -> Vec<u8> { rf.zs.decompress(&fs::read(out.join("romfs").join(rel)).unwrap()).unwrap() };
        let table: Vec<u8> = read(BGM_FILE);
        let link: Vec<u8> = read(SLINK);
        let ours: Rst = Rst::parse(&read(rsizetable::TABLE_PATH)).unwrap();
        let game: Rst = rsizetable::load(&mut rf).unwrap();
        let was: Vec<u8> = rf.read_zs(BGM_FILE).unwrap();
        let sizes: (u64, u64, u32, u32) = (was.len() as u64, table.len() as u64, game.get(BGM_FILE).unwrap(), ours.get(BGM_FILE).unwrap());
        let _ = fs::remove_dir_all(&out);
        Built { old: Byml::parse(&was).unwrap(), new: Byml::parse(&table).unwrap(), vanilla: rf.read_zs(SLINK).unwrap(), link, sizes, note }
    }

    fn nodes<'a>(y: &'a Byml, event: &str) -> &'a [Node] {
        y.root.get("BGM").unwrap().get(event).unwrap().get("Node").unwrap().as_arr().unwrap()
    }

    fn of_type<'a>(all: &'a [Node], t: &str) -> Vec<&'a Node> {
        all.iter().filter(|n| n.get("Type").and_then(|x| x.as_str()) == Some(t)).collect()
    }

    fn conditions(n: &Node) -> Vec<Option<&Node>> {
        n.get("Transition").unwrap().as_arr().unwrap().iter().map(|t| t.get("ConditionBin")).collect()
    }

    #[test]
    fn every_moved_area_theme_is_one_looping_track_that_nothing_restarts() {
        let Some(root) = real() else {
            return;
        };
        let top: PB = root.join(STREAM);
        let loops = |k: &str| looping(&top.join(format!("{k}.bwav")));
        for (m, seed) in [(Music::Anywhere, "listen-1"), (Music::Anywhere, "two"), (Music::Grouped, "three"), (Music::Grouped, "four")] {
            let b: Built = built(&root, m, seed);
            let link: Link = Link::read(&b.link).unwrap();
            let mut done: usize = 0;
            let (mut timed, mut parted): (usize, usize) = (0, 0);
            for (a, _) in collapse::areas() {
                for &event in a.events {
                    let was: &[Node] = nodes(&b.old, event);
                    let now: &[Node] = nodes(&b.new, event);
                    timed += usize::from(was.iter().any(|n| n.get("IsDurationAuto") == Some(&Node::Bool(false))));
                    parted += usize::from(of_type(was, "Asset").len() > 1);
                    let assets: Vec<&Node> = of_type(now, "Asset");
                    assert_eq!(assets.len(), 1, "{event}");
                    assert_eq!(assets[0].get("Key").unwrap().as_str(), Some(a.key), "{event}");
                    assert_eq!(assets[0].get("IsDurationAuto"), Some(&Node::Bool(true)), "{event}");
                    assert!(now.iter().all(|n| n.get("Duration").is_none()), "{event}");
                    assert_eq!(now.len(), if a.quiet_at_night { 4 } else { 2 }, "{event}");
                    let entry: &Node = b.new.root.get("BGM").unwrap().get(event).unwrap();
                    assert_eq!(entry.get("TransitionParam"), Some(&Node::Arr(Vec::new())), "{event}");
                    assert_eq!(conditions(of_type(was, "Start")[0]), conditions(of_type(now, "Start")[0]), "{event}");
                    let to: &Node = of_type(now, "Start")[0].get("Transition").unwrap().as_arr().unwrap()[0].get("DestinationNode").unwrap();
                    if a.quiet_at_night {
                        let (switch, wait): (&Node, &Node) = (of_type(now, "Switch")[0], of_type(now, "Wait")[0]);
                        assert_eq!(Some(to), switch.get("Name"), "{event}");
                        assert_eq!(wait.get("IsInfinity"), Some(&Node::Bool(true)), "{event}");
                        assert_eq!(conditions(assets[0]).len(), 1, "{event}");
                        assert_eq!(conditions(assets[0])[0], conditions(switch)[0], "{event}");
                        assert_ne!(conditions(wait)[0], conditions(switch)[0], "{event}");
                    } else {
                        assert_eq!(Some(to), assets[0].get("Name"), "{event}");
                        assert!(conditions(assets[0]).is_empty(), "{event}");
                    }
                    let names: BS<&str> = now.iter().filter_map(|n| n.get("Name").and_then(|x| x.as_str())).collect();
                    assert_eq!(names.len(), now.len(), "{event}");
                    let user: &str = entry.get("SLinkUserName").unwrap().as_str().unwrap();
                    let calls: Vec<_> = link.calls_of(user).into_iter().filter(|c| c.key == a.key).collect();
                    assert!(!calls.is_empty(), "{event}: no sound link call for {}", a.key);
                    for c in calls {
                        let heard: String = link.asset(&b.link, c).unwrap();
                        assert!(heard != a.key && loops(&heard), "{seed} {event}: {heard}");
                    }
                    done += 1;
                }
            }
            let (Some(Node::Map(was)), Some(Node::Map(now))) = (b.old.root.get("BGM"), b.new.root.get("BGM")) else {
                panic!();
            };
            assert_eq!(was.len(), now.len());
            for (x, y) in was.iter().zip(now) {
                assert_eq!(x.0, y.0);
                assert!(collapse::of(&x.0).is_some() || x.1 == y.1, "{} changed", x.0);
            }
            let (old_len, new_len, game, ours): (u64, u64, u32, u32) = b.sizes;
            assert!(u64::from(ours) >= u64::from(game) + new_len.saturating_sub(old_len));
            assert!(b.note.contains(&format!("{done} area themes")), "{}", b.note);
            println!("{} {seed}: {done} events rewritten ({parted} had several parts, {timed} had fixed lengths), table {old_len} -> {new_len} bytes", m.id());
            assert_eq!(done, collapse::areas().map(|a| a.0.events.len()).sum::<usize>(), "{seed}");
            assert!(timed >= 15 && parted >= 20, "{timed} {parted}");
        }
    }

    #[test]
    fn every_track_handed_out_loops_and_ganon_and_the_escape_keep_their_own() {
        let Some(root) = real() else {
            return;
        };
        let top: PB = root.join(STREAM);
        for (m, seed) in [(Music::Anywhere, "listen-1"), (Music::Anywhere, "five"), (Music::Grouped, "six")] {
            let b: Built = built(&root, m, seed);
            let before: Link = Link::read(&b.vanilla).unwrap();
            let after: Link = Link::read(&b.link).unwrap();
            let kept: BS<u32> = UNSHUFFLED.iter().map(|u| crate::rsizetable::hash(u)).collect();
            let mut handed: usize = 0;
            for (c, d) in after.calls.iter().zip(before.calls.iter()) {
                let (now, was): (String, String) = (after.asset(&b.link, c).unwrap_or_default(), before.asset(&b.vanilla, d).unwrap_or_default());
                assert!(!kept.contains(&c.user) || now == was, "{seed} {was} -> {now}");
                if now != was && !is_jingle(&was) {
                    assert!(!UNSHUFFLED.iter().any(|u| now.starts_with(u) && !now.starts_with("BGM_DungeonBoss_Ganondorf")), "{seed} {was} -> {now}");
                    assert!(looping(&top.join(format!("{now}.bwav"))), "{seed} {was} -> {now}");
                    handed += 1;
                }
            }
            assert!(handed >= 40, "{handed}");
        }
    }

    #[test]
    fn writing_the_music_table_back_unchanged_keeps_every_value() {
        let Some(root) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let raw: Vec<u8> = rf.read_zs(BGM_FILE).unwrap();
        let old: Byml = Byml::parse(&raw).unwrap();
        let new: Vec<u8> = old.write().unwrap();
        println!("music table written back: {} -> {} bytes, byte-identical {}", raw.len(), new.len(), raw == new);
        assert_eq!(Byml::parse(&new).unwrap(), old);
    }
}
