use crate::logic_items::ItemId;
use crate::slice::data::Data;
use crate::slice::support::Plan;
use crate::world::{LocId, Placement, World};

pub struct Spot<'a> {
    pub d: &'a Data,
    pub w: &'a World,
    pub p: &'a Plan,
    pub it: ItemId,
    pub l: LocId,
    pub name: &'a str,
    pub item: &'a str,
}

pub type Rule = fn(&Spot) -> Option<bool>;

/// The rules the settings ask for: they stay when every item may go everywhere.
pub const SETTING_RULES: [(&str, Rule); 7] = [
    ("ganon's boss key on the light arrows cutscene", lacs_key),
    ("dungeon rewards at the end of dungeons", end_of_dungeons),
    ("excluded locations", excluded),
    ("silver rupees", silver),
    ("adult trade items", adult_trade),
    ("skipped locations", skipped),
    ("song spots", songs),
];

pub fn setting_verdict(s: &Spot) -> Option<bool> {
    SETTING_RULES.iter().find_map(|(_, r)| r(s))
}

/// The adult trade chain keeps its items at home: the sequence is not shuffled, unless adult trade
/// shuffle makes each trade item its own item (crate::open_world::trade_shuffle).
fn adult_trade(s: &Spot) -> Option<bool> {
    if s.p.rewards.trade_items {
        return None;
    }
    let child: [&str; 2] = ["Item_WeirdEgg", "Item_ZeldaLetter"];
    s.d.item(s.item)
        .filter(|i| i.actor.as_deref().is_some_and(|a| crate::flow_extra::trade::chain(a) && !child.contains(&a)))
        .is_some_and(|i| !crate::flow_extra::trade::keeps(s.d, s.name, i))
        .then_some(false)
}

fn lacs_key(s: &Spot) -> Option<bool> {
    if s.p.keys.ganon != crate::slice::support::Keep::Lacs {
        return None;
    }
    if s.name == crate::lacs_key::LOC {
        return Some(s.item == crate::lacs_key::ITEM);
    }
    (s.item == crate::lacs_key::ITEM).then_some(false)
}

/// Dungeon Rewards = End of dungeons: a Stone or Medallion only goes to a boss or Rauru reward
/// event, and those events only take a Stone or Medallion. A fitting pair is left to the other rules.
fn end_of_dungeons(s: &Spot) -> Option<bool> {
    if !s.p.rewards.dungeon_own {
        return None;
    }
    let item: bool = s.d.item(s.item).is_some_and(|i| i.kind == "DungeonReward");
    let spot: bool = s.d.flow.by_name(s.name).is_some_and(|l| l.cat == crate::slice::support::DUNGEON_REWARD_CAT);
    (item != spot).then_some(false)
}

fn excluded(s: &Spot) -> Option<bool> {
    (!s.p.excluded.is_empty() && s.p.excluded.iter().any(|e| *e == s.name) && !junk(s.d, s.item)).then_some(false)
}

pub const JUNK: [&str; 17] = [
    "Rupee (1)",
    "Rupees (5)",
    "Rupees (20)",
    "Bomb (1)",
    "Bombs (5)",
    "Bombs (10)",
    "Bombs (20)",
    "Arrows (5)",
    "Arrows (10)",
    "Arrows (30)",
    "Deku Nuts (5)",
    "Deku Nuts (10)",
    "Deku Stick (1)",
    "Deku Sticks (3)",
    "Deku Seeds (30)",
    "Recovery Heart",
    "Magic Jar (Large)",
];

pub fn junk(d: &Data, item: &str) -> bool {
    JUNK.contains(&item) || d.item(item).is_some_and(|i| !i.adv && i.kind == "Shop" && item != "Sold Out")
}

pub const LOW_JUNK: [(&str, u64); 8] = [
    ("Rupee (1)", 35),
    ("Rupees (5)", 35),
    ("Arrows (5)", 6),
    ("Arrows (10)", 4),
    ("Bombs (5)", 6),
    ("Deku Nuts (5)", 5),
    ("Deku Sticks (3)", 5),
    ("Deku Seeds (30)", 4),
];

/// An excluded spot `l` trades its junk for a low junk item (LOW_JUNK, picked by weight) that the
/// fill put at one of the `others`. A trade keeps the seed's item counts, which the build matches
/// against the chests the game ships (slice::chest_targets): a newly rolled item would add a copy
/// the game files do not hold ("the seed places N copies of X but its chests hold N-1"). With no
/// low junk to trade for, the spot keeps the fill's junk, which the excluded rule already made junk.
pub fn swap_low_junk(w: &World, got: &mut Placement, l: LocId, others: &[LocId], ok: &dyn Fn(ItemId, LocId) -> bool, rng: &mut crate::rng::Rng) {
    let low = |i: ItemId| LOW_JUNK.iter().find(|(n, _)| w.item(n) == Some(i)).map(|(_, wt)| *wt);
    let Some(mine) = got.get(l) else {
        return;
    };
    if low(mine).is_some() {
        return;
    }
    let cands: Vec<(LocId, ItemId, u64)> = others
        .iter()
        .copied()
        .filter(|m| *m != l)
        .filter_map(|m| {
            let i: ItemId = got.get(m)?;
            let wt: u64 = low(i)?;
            (ok(i, l) && ok(mine, m)).then_some((m, i, wt))
        })
        .collect();
    let total: u64 = cands.iter().map(|c| c.2).sum();
    if total == 0 {
        return;
    }
    let mut r: u64 = rng.below(total);
    for (m, i, wt) in cands {
        if r < wt {
            got.put(l, i);
            got.put(m, mine);
            return;
        }
        r -= wt;
    }
}

fn silver(s: &Spot) -> Option<bool> {
    crate::slice::silver::at(s.d, s.w, s.p, s.it, s.l)
}

fn skipped(s: &Spot) -> Option<bool> {
    (crate::slice::edit::skipped_locations(s.p).contains(&s.name) && !s.d.item(s.item).is_some_and(crate::slice::edit::given_at_start)).then_some(false)
}

fn songs(s: &Spot) -> Option<bool> {
    crate::slice::song_spots::gate(s.d, s.p, s.name, s.item)
}

#[cfg(test)]
mod tests {
    use crate::settings::{Settings, Value};
    use crate::slice::data::{Data, Item};
    use crate::slice::generate::{allowed, generate_data, GenOpts, Generated};
    use crate::slice::support::Plan;
    use crate::start_state::{Ops, KAKARIKO_LOCATIONS};
    use crate::world::World;

    fn with(v: &[(&str, &str)]) -> Settings {
        let mut s: Settings = Settings::default();
        for (k, x) in v {
            // The new game checks come from skipping the Zelda meeting, a toggle.
            match *x {
                "true" | "false" => s.set(k, Value::Bool(*x == "true")).unwrap(),
                _ => s.set(k, Value::s(x)).unwrap(),
            }
        }
        s.finish().unwrap()
    }

    fn open_keys() -> Settings {
        with(&[("open_forest", "open"), ("skip_child_zelda", "true"), ("shuffle_smallkeys", "keysanity")])
    }

    fn every_kind() -> Settings {
        with(&[
            ("open_forest", "open"),
            ("skip_child_zelda", "true"),
            ("shuffle_smallkeys", "keysanity"),
            ("shuffle_bosskeys", "keysanity"),
            ("shuffle_mapcompass", "keysanity"),
            ("shuffle_ganon_bosskey", "keysanity"),
            ("shuffle_songs", "anywhere"),
        ])
    }

    fn dungeon_items_at_new_game(d: &Data, s: &Settings, seeds: std::ops::Range<usize>) -> usize {
        let mut bad: usize = 0;
        for n in seeds {
            let g: Generated = generate_data(d, s, &format!("r-{n}"), &GenOpts::default()).unwrap();
            for l in KAKARIKO_LOCATIONS {
                bad += usize::from(g.seed_file.placements.get(l).and_then(|x| d.item(x)).is_some_and(|i| i.dungeon.is_some()));
            }
            crate::slice::edit::skipped_items(d, &g.seed_file, &KAKARIKO_LOCATIONS).unwrap();
        }
        bad
    }

    fn small_keys_at_new_game(start: usize) {
        let d: Data = Data::load().unwrap();
        let n: usize = dungeon_items_at_new_game(&d, &open_keys(), start..start + 50);
        println!("seeds r-{start} to r-{}: {n} dungeon items at the checks given at new game (nightly dda8087 before the fix: 40 of the 200 seeds r-0 to r-199)", start + 49);
        assert_eq!(n, 0);
    }

    parts!(small_keys_at_new_game, open_gate_seeds_0_to_49 = 0, open_gate_seeds_50_to_99 = 50, open_gate_seeds_100_to_149 = 100, open_gate_seeds_150_to_199 = 150);

    #[test]
    fn with_every_dungeon_item_anywhere_none_lands_at_the_new_game_checks() {
        let d: Data = Data::load().unwrap();
        let n: usize = dungeon_items_at_new_game(&d, &every_kind(), 0..40);
        println!("every dungeon item anywhere, seeds r-0 to r-39: {n} at the checks given at new game");
        assert_eq!(n, 0);
    }

    #[test]
    fn nightly_seed_r_10_key_at_zeldas_letter_is_refused() {
        let d: Data = Data::load().unwrap();
        let s: Settings = open_keys();
        let p: Plan = crate::slice::support::plan(&s).unwrap();
        let w: World = crate::slice::logic::world_with(&d, &p.logic()).unwrap();
        let (loc, key): (&str, &str) = ("HC Zeldas Letter", "Small Key (Forest Temple)");
        let i: &Item = d.item(key).unwrap();
        assert!(i.actor.is_some() || !i.tiers.is_empty());
        assert!(!crate::slice::edit::given_at_start(i));
        let l: crate::world::LocId = w.loc(loc).unwrap();
        assert!(!allowed(&d, &w, &p, w.item(key).unwrap(), l));
        assert!(allowed(&d, &w, &p, w.item("Gold Skulltula Token").unwrap(), l));
        let mut g: Generated = generate_data(&d, &s, "r-10", &GenOpts::default()).unwrap();
        g.seed_file.placements.insert(loc.to_string(), key.to_string());
        let e: String = crate::slice::edit::skipped_items(&d, &g.seed_file, &KAKARIKO_LOCATIONS).unwrap_err().to_string();
        assert!(e.contains("HC Zeldas Letter holds Small Key (Forest Temple), which cannot be given when a new game starts"), "{e}");
        let actor: String = i.actor.clone().unwrap();
        let ops: Ops = crate::start_state::kakariko(&[actor.clone()]);
        assert!(ops.items.contains(&actor));
        let credit: usize = ops.flags.iter().map(|(n, _)| n).chain(ops.ints.iter().map(|(n, _)| n)).filter(|n| n.starts_with("DungeonInfo.")).count();
        assert_eq!(credit, 0);
    }

    #[test]
    fn keys_in_their_own_dungeon_never_reach_the_new_game_checks() {
        let d: Data = Data::load().unwrap();
        let s: Settings = with(&[("open_forest", "open"), ("skip_child_zelda", "true")]);
        assert_eq!(dungeon_items_at_new_game(&d, &s, 0..20), 0);
    }

    #[test]
    fn a_dungeon_item_at_a_new_game_check_stops_the_build() {
        let d: Data = Data::load().unwrap();
        let s: Settings = open_keys();
        let p: Plan = crate::slice::support::plan(&s).unwrap();
        let mut g: Generated = generate_data(&d, &s, "r-key", &GenOpts::default()).unwrap();
        for name in ["Small Key (Shadow Temple)", "Boss Key (Fire Temple)", "Map (Deku Tree)", "Compass (Water Temple)", "Boss Key (Ganons Castle)"] {
            g.seed_file.placements.insert("HC Zeldas Letter".to_string(), name.to_string());
            let e: String = crate::slice::edit::start_state(&d, &g.seed_file, &p).unwrap_err().to_string();
            assert!(e.contains(&format!("HC Zeldas Letter holds {name}, which cannot be given when a new game starts")), "{e}");
        }
        g.seed_file.placements.insert("HC Zeldas Letter".to_string(), "Gold Skulltula Token".to_string());
        let ok: Vec<String> = crate::slice::edit::skipped_items(&d, &g.seed_file, &KAKARIKO_LOCATIONS).unwrap();
        assert_eq!(ok.first().map(|s| s.as_str()), Some("Item_Token_Gold"));
    }

    #[test]
    fn a_plando_key_at_a_new_game_check_is_refused() {
        let d: Data = Data::load().unwrap();
        let o: GenOpts = GenOpts { plando: vec![crate::slice::plando::parse("HC Zeldas Letter=Small Key (Forest Temple)").unwrap()], ..GenOpts::default() };
        let e: String = generate_data(&d, &open_keys(), "plando key", &o).map(|_| ()).unwrap_err().to_string();
        assert!(e.contains("cannot be placed at HC Zeldas Letter"), "{e}");
        let closed: Settings = with(&[("shuffle_smallkeys", "keysanity")]);
        assert!(generate_data(&d, &closed, "plando key", &o).is_ok());
    }
}
