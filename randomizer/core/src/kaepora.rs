use std::collections::BTreeMap as BM;

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Case, Cont, Event, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::difficulty::ambush::{actor, euler, fresh, list, text};
use crate::error::{Error, Res};
use crate::msbt::{self, Msbt};
use crate::npc_hints::{item_phrase, layout, lead_codes, place, where_is, HI, LO};
use crate::placement::hash_of;
use crate::romfs::Romfs;
use crate::sarc;
use crate::settings::seed::SeedFile;
use crate::slice::data::Data;
use crate::spoiler::Spoiler;
use crate::staging::edit_staged;
use crate::world::{Placement, World};

pub const SETTING: &str = "kaepora_hint";
pub const NUM: u16 = 152;
pub const OWL: &str = "Npc_Kaepora_KokiriForest";
pub const BASE: &str = "Npc_Kaepora_DeathMountainTrail";
pub const MAP_FILE: &str = "Banc/KokiriForest/KokiriForest_Child_Dynamic.bcett.byml.zs";
pub const MSG_FILE: &str = "EventFlowMsg/Npc_Kaepora_DeathMountainTrail.msbt";
pub const MESSAGE_OWNER: &str = BASE;
pub const SEED: &str = "ootrr kaepora hint";
pub const SPOT: [f32; 3] = [-16.35, 119.23, -36.2];
pub const FACING: f32 = -2.7558;
pub const MAX_LINKS: usize = 24;
pub const LABEL: &str = "OOTRRHint_";
pub const DONE: &str = "OOTRRHint_Done";
pub const TALK: &str = "Talk";
pub const TALK_TEMPLATE: &str = "Event29";
pub const FLAG_TEMPLATE: &str = "Event4";
pub const OWNED: u32 = 0;
pub const LACKS: u32 = 1;
pub const GAME_DATA: &str = "GameDataName";
pub const MESSAGE: &str = "MessageId";
pub const SAMPLE: &str = "Talk_03";

pub const FLAGS: &[(&str, &[&str])] = &[
    ("Progressive Hookshot", &["Item_Hookshot", "Item_Longshot"]),
    ("Progressive Strength Upgrade", &["Armor_Child_Gauntlet_Goron", "Armor_Adult_Gauntlet_Silver", "Armor_Adult_Gauntlet_Gold"]),
    ("Bomb Bag", &["Item_BombBag_Small", "Item_BombBag_Medium", "Item_BombBag_Large"]),
    ("Bow", &["Weapon_Range_Bow", "Item_Quiver_Medium", "Item_Quiver_Large"]),
    ("Slingshot", &["Weapon_Range_SlingShot", "Item_BulletBag_Medium", "Item_BulletBag_Large"]),
    ("Progressive Wallet", &["Item_Wallet_Medium", "Item_Wallet_Large"]),
    ("Progressive Scale", &["Item_Scale_Silver", "Item_Scale_Gold"]),
    ("Magic Meter", &["Item_FairyBless_MP", "Item_FairyBless_MPDouble"]),
    ("Ocarina", &["Item_Ocarina_Fairy", "Item_Ocarina_Time"]),
    ("Boomerang", &["Item_Boomerang"]),
    ("Megaton Hammer", &["Item_Hammer"]),
    ("Lens of Truth", &["Armor_GlassTruth"]),
    ("Fire Arrows", &["Item_Arrow_Fire"]),
    ("Ice Arrows", &["Item_Arrow_Ice"]),
    ("Light Arrows", &["Item_Arrow_Light"]),
    ("Dins Fire", &["Item_DinFire"]),
    ("Farores Wind", &["Item_FaroreWind"]),
    ("Nayrus Love", &["Item_NayruLove"]),
    ("Kokiri Sword", &["Weapon_Sword_Kokiri"]),
    ("Giants Knife", &["Weapon_Sword_Giant"]),
    ("Biggoron Sword", &["Weapon_Sword_Biggoron"]),
    ("Mirror Shield", &["Weapon_Shield_Mirror"]),
    ("Goron Tunic", &["Armor_Adult_Goron"]),
    ("Zora Tunic", &["Armor_Adult_Zora"]),
    ("Iron Boots", &["Armor_Adult_HeavyBoot"]),
    ("Hover Boots", &["Armor_Adult_HoverBoot"]),
    ("Stone of Agony", &["Item_AgonyStone"]),
    ("Gerudo Membership Card", &["Item_Token_Gerudo"]),
    ("Zeldas Letter", &["Item_ZeldaLetter"]),
    ("Rutos Letter", &["Item_Bottle_Letter"]),
    ("Bottle", &["Item_Bottle"]),
    ("Magic Bean", &["Item_MagicBean"]),
    ("Weird Egg", &["Item_WeirdEgg"]),
    ("Bombchus", &["Item_Bombchu"]),
    ("Bombchus (5)", &["Item_Bombchu"]),
    ("Bombchus (10)", &["Item_Bombchu"]),
    ("Bombchus (20)", &["Item_Bombchu"]),
    ("Keaton Mask", &["Armor_Child_Mask_Keaton"]),
    ("Skull Mask", &["Armor_Child_Mask_Skull"]),
    ("Spooky Mask", &["Armor_Child_Mask_Spooky"]),
    ("Bunny Hood", &["Armor_Child_Mask_Bunny"]),
    ("Mask of Truth", &["Armor_Child_Mask_Truth"]),
    ("Minuet of Forest", &["Item_Melody_ForestMinuet"]),
    ("Bolero of Fire", &["Item_Melody_FireBolero"]),
    ("Serenade of Water", &["Item_Melody_WaterSerenade"]),
    ("Requiem of Spirit", &["Item_Melody_SpiritRequiem"]),
    ("Nocturne of Shadow", &["Item_Melody_ShadowNocturne"]),
    ("Prelude of Light", &["Item_Melody_LightPrelude"]),
    ("Zeldas Lullaby", &["Item_Melody_ZeldaLullaby"]),
    ("Eponas Song", &["Item_Melody_EponaSong"]),
    ("Sarias Song", &["Item_Melody_SariaSong"]),
    ("Suns Song", &["Item_Melody_SunSong"]),
    ("Song of Time", &["Item_Melody_TimeSong"]),
    ("Song of Storms", &["Item_Melody_StormSong"]),
    ("Kokiri Emerald", &["Item_Stone_Forest"]),
    ("Goron Ruby", &["Item_Stone_Fire"]),
    ("Zora Sapphire", &["Item_Stone_Water"]),
    ("Light Medallion", &["Item_Medal_Light"]),
    ("Forest Medallion", &["Item_Medal_Forest"]),
    ("Fire Medallion", &["Item_Medal_Fire"]),
    ("Water Medallion", &["Item_Medal_Water"]),
    ("Shadow Medallion", &["Item_Medal_Shadow"]),
    ("Spirit Medallion", &["Item_Medal_Spirit"]),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub item: String,
    pub place: String,
    pub flag: String,
    pub text: String,
}

pub fn read(s: &crate::settings::Settings, bad: &mut Vec<String>) -> bool {
    crate::slice::support::toggle(s, SETTING, bad)
}

pub fn flags_of(item: &str) -> Option<&'static [&'static str]> {
    FLAGS.iter().find(|(n, _)| *n == item).map(|(_, f)| *f)
}

fn hi(s: &str) -> String {
    format!("{HI}{s}{LO}")
}

pub fn told(item: &str, place: &str) -> String {
    format!("Hoo hoo! Heed my words, Link!|Know this: {} awaits you in {}.|You will need it on your journey. Hoo hoo!", hi(&item_phrase(item)), hi(place))
}

pub const REWARDS: [(&str, &str, &str); 9] = [
    ("Kokiri Emerald", "Queen Gohma", "Queen Gohma"),
    ("Goron Ruby", "King Dodongo", "King Dodongo"),
    ("Zora Sapphire", "Barinade", "Barinade"),
    ("Forest Medallion", "Phantom Ganon", "Phantom Ganon"),
    ("Fire Medallion", "Volvagia", "Volvagia"),
    ("Water Medallion", "Morpha", "Morpha"),
    ("Shadow Medallion", "Bongo Bongo", "Bongo Bongo"),
    ("Spirit Medallion", "Twinrova", "Twinrova"),
    ("Light Medallion", "ToT Reward from Rauru", "Rauru"),
];
pub const STONES: usize = 3;
pub const VANILLA_GOAL: [&str; 2] = ["Shadow Medallion", "Spirit Medallion"];

pub fn guarded(item: &str, place: &str, keeper: &str) -> String {
    format!("Hoo hoo! Heed my words, Link!|Know this: {} awaits you in {}, from {}.|You will need it on your journey. Hoo hoo!", hi(&item_phrase(item)), hi(place), hi(keeper))
}

fn goal(s: &crate::settings::Settings, mode: &str, stones: &str, medallions: &str, rewards: &str) -> Vec<&'static str> {
    let count = |key: &str| -> usize { s.int(key).map(|n| n.max(0) as usize).unwrap_or(0) };
    let stone: Vec<&'static str> = REWARDS[..STONES].iter().map(|r| r.0).collect();
    let medal: Vec<&'static str> = REWARDS[STONES..].iter().map(|r| r.0).collect();
    match s.text(mode) {
        Ok("stones") => stone.into_iter().take(count(stones)).collect(),
        Ok("medallions") => medal.into_iter().take(count(medallions)).collect(),
        Ok("dungeons") => REWARDS.iter().map(|r| r.0).take(count(rewards)).collect(),
        _ => Vec::new(),
    }
}

pub fn needed(s: &crate::settings::Settings) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    let mut add = |v: Vec<&'static str>| {
        for i in v {
            if !out.contains(&i) {
                out.push(i);
            }
        }
    };
    if s.text("bridge").ok() == Some("vanilla") {
        add(VANILLA_GOAL.to_vec());
    } else {
        add(goal(s, "bridge", "bridge_stones", "bridge_medallions", "bridge_rewards"));
    }
    if s.text("shuffle_ganon_bosskey").ok() == Some(crate::lacs_key::VALUE) {
        add(REWARDS.iter().map(|r| r.0).take(s.int(crate::medal_gate::LACS_REWARDS).map(|n| n.max(0) as usize).unwrap_or(0)).collect());
    } else {
        add(goal(s, "shuffle_ganon_bosskey", "ganon_bosskey_stones", "ganon_bosskey_medallions", "ganon_bosskey_rewards"));
    }
    out.sort_by_key(|i| REWARDS.iter().position(|r| r.0 == *i));
    out
}

pub const ALL_DONE: &str = "Hoo hoo! I have nothing more to tell you, Link. Your path lies clear before you!";

pub fn chain(d: &Data, seed: &SeedFile) -> Vec<Link> {
    let Some((w, place_of)): Option<(World, Placement)> = crate::slice::starts::rebuild(d, seed).ok() else {
        return Vec::new();
    };
    let Some(sp): Option<Spoiler> = crate::spoiler::playthrough(&w, &place_of).ok() else {
        return Vec::new();
    };
    let mut seen: BM<String, usize> = BM::new();
    let mut out: Vec<Link> = Vec::new();
    'spheres: for sphere in &sp.spheres {
        for (l, i) in sphere {
            let name: &str = &w.items.names[*i];
            let Some(flags) = flags_of(name) else {
                continue;
            };
            let held: usize = w.start.iter().filter(|(id, _)| *id == *i).map(|(_, n)| *n as usize).sum();
            let k: usize = *seen.entry(name.to_string()).and_modify(|n| *n += 1).or_insert(0);
            let Some(flag) = flags.get(held + k) else {
                continue;
            };
            let Some(at) = place(d, &w.locs[*l].name) else {
                continue;
            };
            out.push(Link { item: name.to_string(), place: at.clone(), flag: format!("IsGet.{flag}"), text: told(name, &at) });
            if out.len() == MAX_LINKS {
                break 'spheres;
            }
        }
    }
    if let Ok(loaded) = seed.settings() {
        for item in needed(&loaded.settings) {
            if out.iter().any(|l| l.item == item) || w.item(item).is_some_and(|i| w.start.iter().any(|(id, n)| *id == i && *n > 0)) {
                continue;
            }
            let Some((_, event, keeper)) = REWARDS.iter().find(|r| r.0 == item) else {
                continue;
            };
            let Some(flag) = flags_of(item).and_then(|f| f.first()) else {
                continue;
            };
            let loc: String = where_is(d, seed, item).unwrap_or_else(|| event.to_string());
            let Some(at) = place(d, &loc) else {
                continue;
            };
            let text: String = if loc == *event { guarded(item, &at, keeper) } else { told(item, &at) };
            out.push(Link { item: item.to_string(), place: at, flag: format!("IsGet.{flag}"), text });
        }
    }
    out
}

fn label(i: usize) -> String {
    format!("{LABEL}{i:02}")
}

fn message(label: &str) -> String {
    format!("EventFlowMsg/{MESSAGE_OWNER}:{label}")
}

fn say(t: &Event, label: &str) -> Res<Kind> {
    let Kind::Action { next: _, actor, act, params } = &t.kind else {
        return Err(Error::Format("kaepora: the talk template is not an action"));
    };
    let mut p: Cont = params.clone().ok_or(Error::Format("kaepora: the talk template has no parameters"))?;
    let mut set: bool = false;
    for (k, v) in p.items.iter_mut() {
        if k == MESSAGE {
            *v = Val::Str(message(label));
            set = true;
        }
    }
    if !set {
        return Err(Error::Format("kaepora: the talk template has no message"));
    }
    Ok(Kind::Action { next: NONE, actor: *actor, act: *act, params: Some(p) })
}

fn push(f: &mut Flow, name: String, kind: Kind) -> Res<u16> {
    if f.events.len() >= NONE as usize {
        return Err(Error::Format("kaepora: too many events in a flow"));
    }
    f.events.push(Event { name, kind });
    Ok((f.events.len() - 1) as u16)
}

fn find<'a>(f: &'a Flow, name: &str) -> Res<&'a Event> {
    f.events.iter().find(|e| e.name == name).ok_or_else(|| Error::Msg(format!("kaepora: the owl flow has no {name}, the game files differ")))
}

pub fn flow_bytes(raw: &[u8], links: &[Link]) -> Res<Vec<u8>> {
    let mut b: Bf = Bf::parse(raw)?;
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == BASE).ok_or(Error::Format("kaepora: the owl flow is not in its file"))?;
    if f.events.iter().any(|e| e.name.starts_with(LABEL)) {
        return Err(Error::Format("kaepora: the owl flow is already edited"));
    }
    let talk: Event = find(f, TALK_TEMPLATE)?.clone();
    let check: Event = find(f, FLAG_TEMPLATE)?.clone();
    let Kind::Switch { actor: ga, query: gq, params: gp, .. } = &check.kind else {
        return Err(Error::Format("kaepora: the flag template is not a switch"));
    };
    let (ga, gq): (u16, u16) = (*ga, *gq);
    let entry: usize = f.entries.iter().position(|e| e.name == TALK).ok_or(Error::Format("kaepora: the owl flow has no Talk entry"))?;
    let mut next: u16 = push(f, DONE.to_string(), say(&talk, DONE)?)?;
    for (i, l) in links.iter().enumerate().rev() {
        let said: u16 = push(f, label(i), say(&talk, &label(i))?)?;
        let mut p: Cont = gp.clone().ok_or(Error::Format("kaepora: the flag template has no parameters"))?;
        let mut set: bool = false;
        for (k, v) in p.items.iter_mut() {
            if k == GAME_DATA {
                *v = Val::Str(l.flag.clone());
                set = true;
            }
        }
        if !set {
            return Err(Error::Format("kaepora: the flag template has no flag name"));
        }
        next = push(f, format!("{LABEL}Has{i:02}"), Kind::Switch { actor: ga, query: gq, cases: vec![Case { val: OWNED, ev: next }, Case { val: LACKS, ev: said }], params: Some(p) })?;
    }
    f.entries[entry].main = next;
    let out: Vec<u8> = b.write()?;
    if Bf::parse(&out)? != b {
        return Err(Error::Format("kaepora: the edited owl flow does not read back"));
    }
    Ok(out)
}

pub fn text_bytes(raw: &[u8], links: &[Link]) -> Res<Vec<u8>> {
    let mut files: Vec<sarc::Entry> = sarc::parse(raw)?;
    let entry: &sarc::Entry = sarc::get(&files, MSG_FILE).ok_or(Error::Format("kaepora: the language archive has no owl text"))?;
    let mut m: Msbt = msbt::parse(&entry.data)?;
    let lead: String = lead_codes(&msbt::to_markup(&m.text(SAMPLE).map_err(|_| Error::Format("kaepora: the owl text has no sample line"))?));
    for (i, l) in links.iter().enumerate() {
        m.put_markup(&label(i), &format!("{lead}{}", layout(&l.text, "")))?;
    }
    m.put_markup(DONE, &format!("{lead}{}", layout(ALL_DONE, "")))?;
    sarc::set(&mut files, MSG_FILE, m.write()?);
    sarc::write(&files)
}

pub fn map_bytes(raw: &[u8], raws: &[&[u8]], at: [f32; 3], yaw: f32) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    let h: u64 = fresh(raws, SEED);
    let acts: &mut Vec<Node> = list(&mut y, "Actors")?;
    if acts.iter().any(|a| text(a, "Gyaml") == OWL || hash_of(a) == Some(h)) {
        return Err(Error::Format("kaepora: the Kokiri Forest map already holds an owl"));
    }
    acts.push(actor(OWL, h, at, euler(0.0, yaw)));
    y.write()
}

pub fn stage(rf: &mut Romfs, more: &mut Vec<crate::staging::Out>, work: &std::path::PathBuf, d: &Data, seed: &SeedFile, on: bool) -> Res<()> {
    if !on {
        return Ok(());
    }
    let links: Vec<Link> = chain(d, seed);
    let map: Vec<u8> = rf.read_zs(MAP_FILE)?;
    let statics: Vec<u8> = rf.read_zs(&MAP_FILE.replace("_Dynamic", "_Static"))?;
    edit_staged(rf, more, work, MAP_FILE, true, |raw| map_bytes(raw, &[&map, &statics], SPOT, FACING))?;
    for f in crate::hints::languages(rf)? {
        edit_staged(rf, more, work, &f, true, |raw| text_bytes(raw, &links))?;
    }
    Ok(())
}

pub fn note() -> String {
    "Kaepora hint: an owl stands in the child Kokiri Forest and names where the next item you need lies. Experimental.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, Value};
    use crate::slice::generate::{generate_data, GenOpts, Generated};

    fn dump() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from)?;
        if p.join("Pack").join("ZsDic.pack.zs").is_file() { Some(p) } else { None }
    }

    fn seed(n: u32) -> SeedFile {
        let d: Data = Data::load().unwrap();
        let g: Generated = generate_data(&d, &Settings::default(), &format!("kaepora {n}"), &GenOpts::default()).unwrap();
        g.seed_file
    }

    #[test]
    fn the_spot_is_within_one_and_a_half_units_of_the_navmesh_floor() {
        let Some(root) = dump() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let g = crate::difficulty::ground::load(&mut rf, "KokiriForest_Child").unwrap().unwrap();
        let near: bool = (-3..=3).any(|a| (-3..=3).any(|b| g.floor_near(SPOT[0] + a as f32 * 0.5, SPOT[1], SPOT[2] + b as f32 * 0.5, 0.6, 0.6).is_some()));
        assert!(near, "{:?}", g.floors(SPOT[0], SPOT[2]));
    }

    #[test]
    fn the_setting_is_off_by_default_and_read() {
        let mut bad: Vec<String> = Vec::new();
        assert!(!read(&Settings::default(), &mut bad));
        let mut s: Settings = Settings::default();
        s.set(SETTING, Value::Bool(true)).unwrap();
        assert!(read(&s, &mut bad) && bad.is_empty());
        let mut t: Settings = Settings::default();
        t.set(SETTING, Value::Bool(false)).unwrap();
        assert_eq!(Settings::default().encode().unwrap(), t.encode().unwrap());
    }

    #[test]
    fn every_item_of_the_table_is_a_real_item_with_real_game_flags() {
        let d: Data = Data::load().unwrap();
        let flags: String = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/flags.json")).unwrap();
        let mut names: Vec<&str> = FLAGS.iter().map(|f| f.0).collect();
        let n: usize = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n);
        for (item, fl) in FLAGS {
            assert!(d.item(item).is_some(), "{item} is not an item");
            assert!(!fl.is_empty() && fl.len() <= 3, "{item}");
            if let Some(f) = d.family(item) {
                assert_eq!(fl.len(), crate::progressive::tiers(f).unwrap().len(), "{item} needs one flag per tier");
            }
            for f in fl.iter() {
                assert!(flags.contains(&format!("\"IsGet.{f}\"")), "{f} is not a game flag");
            }
        }
    }

    #[test]
    fn the_chain_names_required_items_with_a_flag_each_in_order() {
        let d: Data = Data::load().unwrap();
        for n in 0..3 {
            let s: SeedFile = seed(n);
            let c: Vec<Link> = chain(&d, &s);
            assert!(!c.is_empty() && c.len() <= MAX_LINKS + REWARDS.len(), "{n}: {}", c.len());
            let mut flags: Vec<&str> = c.iter().map(|l| l.flag.as_str()).collect();
            let k: usize = flags.len();
            flags.sort();
            flags.dedup();
            assert_eq!(flags.len(), k, "{n}: a flag is asked twice");
            for l in &c {
                assert!(l.flag.starts_with("IsGet.") && l.text.contains(&l.place), "{l:?}");
                let laid: String = layout(&l.text, "");
                assert!(msbt::from_markup(&laid).is_ok(), "{laid}");
                for page in laid.split(crate::npc_hints::PAGE) {
                    assert!(page.split('\n').count() <= crate::npc_hints::LINES, "{page}");
                }
            }
            assert_eq!(chain(&d, &s), c, "{n}: the chain is not stable");
        }
    }

    #[test]
    fn the_hint_names_where_the_item_was_placed() {
        let d: Data = Data::load().unwrap();
        let s: SeedFile = seed(7);
        for l in &chain(&d, &s) {
            let at: Vec<String> = s.placements.iter().filter(|(_, i)| **i == l.item).filter_map(|(loc, _)| place(&d, loc)).collect();
            let vanilla: Vec<String> = d.vanilla.iter().filter(|(_, i)| **i == l.item).filter_map(|(loc, _)| place(&d, loc)).collect();
            assert!(at.contains(&l.place) || vanilla.contains(&l.place), "{} is not in {} ({:?})", l.item, l.place, at);
        }
    }

    fn with(pairs: &[(&str, Value)]) -> Settings {
        let mut s: Settings = Settings::default();
        for (k, v) in pairs {
            s.set(k, v.clone()).unwrap();
        }
        s
    }

    #[test]
    fn the_needed_rewards_follow_the_bridge_and_the_boss_key_in_logic_order() {
        assert_eq!(needed(&Settings::default()), ["Shadow Medallion", "Spirit Medallion"]);
        assert!(needed(&with(&[("bridge", Value::s("open"))])).is_empty());
        assert_eq!(needed(&with(&[("bridge", Value::s("stones")), ("bridge_stones", Value::Int(2))])), ["Kokiri Emerald", "Goron Ruby"]);
        assert_eq!(needed(&with(&[("bridge", Value::s("medallions")), ("bridge_medallions", Value::Int(3))])), ["Forest Medallion", "Fire Medallion", "Water Medallion"]);
        assert_eq!(needed(&with(&[("bridge", Value::s("dungeons")), ("bridge_rewards", Value::Int(4))])), ["Kokiri Emerald", "Goron Ruby", "Zora Sapphire", "Forest Medallion"]);
        assert_eq!(
            needed(&with(&[("bridge", Value::s("stones")), ("bridge_stones", Value::Int(1)), ("shuffle_ganon_bosskey", Value::s("medallions")), ("ganon_bosskey_medallions", Value::Int(2))])),
            ["Kokiri Emerald", "Forest Medallion", "Fire Medallion"]
        );
        assert_eq!(needed(&with(&[("bridge", Value::s("tokens"))])), Vec::<&str>::new());
        assert_eq!(REWARDS.len(), 9);
        for r in REWARDS {
            assert!(flags_of(r.0).is_some() && crate::npc_hints::place(&Data::load().unwrap(), r.1).is_some(), "{r:?}");
        }
    }

    #[test]
    fn with_everything_but_the_rewards_held_the_chain_still_asks_for_the_medallions_and_says_where() {
        let d: Data = Data::load().unwrap();
        let kit: Vec<&str> = vec!["hookshot", "longshot", "megaton_hammer", "lens_of_truth", "magic_meter", "dins_fire", "farores_wind", "fire_arrows", "ice_arrows", "light_arrows", "iron_boots", "hover_boots", "goron_tunic", "zora_tunic", "mirror_shield", "ocarina_of_time", "bow", "boomerang", "golden_gauntlets"];
        let s: Settings = with(&[("starting_items", Value::set(&kit))]);
        let g = generate_data(&d, &s, "kaepora kit", &GenOpts::default()).unwrap();
        let c: Vec<Link> = chain(&d, &g.seed_file);
        let last: Vec<&Link> = c.iter().filter(|l| l.item.ends_with("Medallion")).collect();
        assert!(last.iter().any(|l| l.item == "Shadow Medallion") && last.iter().any(|l| l.item == "Spirit Medallion"), "{:?}", c.iter().map(|l| &l.item).collect::<Vec<_>>());
        for l in last {
            assert!(l.flag.starts_with("IsGet.Item_Medal_") && l.text.contains(&l.place), "{l:?}");
        }
        assert!(c.len() <= MAX_LINKS + REWARDS.len());
    }

    #[test]
    fn a_reward_held_at_its_boss_is_named_with_the_dungeon_and_the_boss() {
        let t: String = guarded("Shadow Medallion", "the Shadow Temple", "Bongo Bongo");
        assert!(t.contains("Shadow Medallion") && t.contains("the Shadow Temple") && t.contains("Bongo Bongo"), "{t}");
        let laid: String = layout(&t, "");
        assert!(msbt::from_markup(&laid).is_ok());
        for page in laid.split(crate::npc_hints::PAGE) {
            assert!(page.split('\n').count() <= crate::npc_hints::LINES, "{page}");
        }
    }

    fn vanilla_flow() -> Option<Vec<u8>> {
        let root: std::path::PathBuf = dump()?;
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        Some(rf.read_zs(&crate::kaepora_pack::flow_file(BASE)).unwrap())
    }

    fn fake(n: usize) -> Vec<Link> {
        (0..n).map(|i| Link { item: "Boomerang".into(), place: "Zora's Domain".into(), flag: format!("IsGet.Item_Boomerang{i}"), text: told("Boomerang", "Zora's Domain") }).collect()
    }

    #[test]
    fn every_path_of_the_edited_talk_ends_the_way_the_game_ends_its_talks() {
        let Some(raw) = vanilla_flow() else { return };
        for n in [0, 1, 5, MAX_LINKS] {
            let out: Vec<u8> = flow_bytes(&raw, &fake(n)).unwrap();
            let was: Bf = Bf::parse(&raw).unwrap();
            let now: Bf = Bf::parse(&out).unwrap();
            let (a, b): (&Flow, &Flow) = (&was.flows[0], &now.flows[0]);
            assert_eq!(b.events[..a.events.len()], a.events[..]);
            assert_eq!(b.events.len(), a.events.len() + 2 * n + 1);
            let e: usize = b.entries.iter().position(|e| e.name == TALK).unwrap();
            let mut stack: Vec<u16> = vec![b.entries[e].main];
            let mut said: usize = 0;
            let mut seen: Vec<u16> = Vec::new();
            while let Some(i) = stack.pop() {
                if seen.contains(&i) {
                    continue;
                }
                seen.push(i);
                match &b.events[i as usize].kind {
                    Kind::Action { next, .. } => {
                        assert_eq!(*next, NONE, "{}", b.events[i as usize].name);
                        said += 1;
                    }
                    Kind::Switch { cases, .. } => stack.extend(cases.iter().map(|c| c.ev)),
                    k => panic!("{k:?}"),
                }
            }
            assert_eq!(said, n + 1, "one line per link and the closing line");
            for ev in b.events.iter().skip(a.events.len()) {
                if let Kind::Action { .. } = &ev.kind {
                    let Some(Val::Str(m)) = ev.kind.params().and_then(|p| p.get(MESSAGE)) else { panic!("{}", ev.name) };
                    assert!(m.starts_with(&format!("EventFlowMsg/{MESSAGE_OWNER}:{LABEL}")), "{m}");
                }
            }
            assert!(flow_bytes(&out, &fake(n)).is_err());
        }
    }

    #[test]
    fn the_language_archive_gains_the_lines_and_keeps_the_others() {
        let Some(root) = dump() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let langs: Vec<String> = crate::hints::languages(&rf).unwrap();
        let raw: Vec<u8> = rf.read_zs(&langs[0]).unwrap();
        let out: Vec<u8> = text_bytes(&raw, &fake(3)).unwrap();
        let (a, b): (Vec<sarc::Entry>, Vec<sarc::Entry>) = (sarc::parse(&raw).unwrap(), sarc::parse(&out).unwrap());
        let (m0, m1): (Msbt, Msbt) = (msbt::parse(&sarc::get(&a, MSG_FILE).unwrap().data).unwrap(), msbt::parse(&sarc::get(&b, MSG_FILE).unwrap().data).unwrap());
        for l in ["Talk_00", "Talk_07", "Near_00"] {
            assert_eq!(m0.raw(l), m1.raw(l), "{l}");
        }
        for i in 0..3 {
            assert!(m1.raw(&label(i)).is_some(), "{i}");
        }
        assert!(m1.raw(DONE).is_some());
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn the_owl_is_added_to_the_kokiri_forest_map_once() {
        let Some(root) = dump() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let raw: Vec<u8> = rf.read_zs(MAP_FILE).unwrap();
        let out: Vec<u8> = map_bytes(&raw, &[&raw], [1.0, 2.0, 3.0], 0.5).unwrap();
        let was: Byml = Byml::parse(&raw).unwrap();
        let now: Byml = Byml::parse(&out).unwrap();
        let (a, b): (&[Node], &[Node]) = (was.root.get("Actors").unwrap().as_arr().unwrap(), now.root.get("Actors").unwrap().as_arr().unwrap());
        assert_eq!(b.len(), a.len() + 1);
        assert_eq!(&b[..a.len()], a);
        assert_eq!(text(&b[a.len()], "Gyaml"), OWL);
        assert!(map_bytes(&out, &[&out], SPOT, FACING).is_err());
    }

    #[test]
    fn a_build_stages_the_map_and_every_language() {
        let Some(root) = dump() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let d: Data = Data::load().unwrap();
        let s: SeedFile = seed(1);
        let work: std::path::PathBuf = std::env::temp_dir().join(format!("ootrr_kaepora_{}", std::process::id()));
        let mut more: Vec<crate::staging::Out> = Vec::new();
        stage(&mut rf, &mut more, &work, &d, &s, false).unwrap();
        assert!(more.is_empty());
        stage(&mut rf, &mut more, &work, &d, &s, true).unwrap();
        let langs: Vec<String> = crate::hints::languages(&rf).unwrap();
        assert_eq!(more.len(), 1 + langs.len());
        assert!(more.iter().any(|o| o.path == MAP_FILE));
        for l in &langs {
            assert!(more.iter().any(|o| &o.path == l), "{l}");
        }
    }
}
