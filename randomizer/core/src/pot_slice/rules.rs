use std::collections::{BTreeMap as BM, HashMap as HM};

use crate::logic_items::ItemId;
use crate::pot_slice::pickup;
use crate::slice::data::Data;
use crate::world::{LocId, Placement, World};

pub fn at(d: &Data, name: &str, item: &str) -> Option<bool> {
    if d.pots.has(name) {
        let known: bool = pickup::of(item).is_some() || crate::slice::key_spots::placeable(d, item, crate::slice::key_spots::Spot::Pot);
        let kind: bool = d.pots.kind_of(name).is_some();
        return Some(known && kind);
    }
    None
}

pub fn verdict(d: &Data, w: &World, it: ItemId, l: LocId) -> Option<bool> {
    if d.pots.is_empty() {
        return None;
    }
    let item: &str = &w.items.names[it];
    if let Some(v) = at(d, &w.locs[l].name, item) {
        return Some(v);
    }
    if d.pots.only.contains(item) {
        return Some(false);
    }
    None
}

pub fn caps(d: &Data, w: &World, free: &[LocId], vanilla: &Placement, ok: &dyn Fn(ItemId, LocId) -> bool) -> HM<LocId, usize> {
    let mut out: HM<LocId, usize> = HM::new();
    if !free.iter().any(|l| d.pots.has(&w.locs[*l].name)) {
        return out;
    }
    let pool: Vec<ItemId> = free.iter().filter_map(|l| vanilla.get(*l)).collect();
    let mut n: BM<ItemId, usize> = BM::new();
    for i in pool.iter().filter(|i| !w.items.adv[**i]) {
        *n.entry(*i).or_insert(0) += 1;
    }
    let filler: Option<ItemId> = n.iter().max_by_key(|(i, c)| (**c, std::cmp::Reverse(**i))).map(|(i, _)| *i);
    for l in free {
        out.insert(*l, usize::from(filler.is_some_and(|i| ok(i, *l))));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::explore;
    use crate::settings::{Settings, Value};
    use crate::slice::fill::vanilla_place;
    use crate::slice::generate::{generate_data, GenOpts, Generated};
    use crate::slice::logic::{world, world_with};
    use crate::slice::support::{plan, Plan};
    use crate::world::Placement;

    fn probe(d: &Data, w: &World, item: &str, loc: &str) -> Option<bool> {
        verdict(d, w, w.item(item).unwrap(), w.loc(loc).unwrap())
    }

    #[test]
    fn pots_take_any_item_with_a_pickup() {
        let d: Data = Data::load().unwrap();
        let w: World = world(&d).unwrap();
        let pot: &str = "KF House of Twins Pot 1";
        let chest: &str = "KF Midos Top Left Chest";
        assert_eq!(probe(&d, &w, "Rupees (20)", pot), Some(true));
        assert_eq!(probe(&d, &w, "Fire Arrows", pot), Some(true));
        assert_eq!(probe(&d, &w, "Fire Arrows", "Dodongos Cavern Right Side Pot 1"), Some(true));
        assert_eq!(probe(&d, &w, "Megaton Hammer", "Dodongos Cavern Right Side Pot 1"), Some(true));
        assert_eq!(probe(&d, &w, "Gold Skulltula Token", "Dodongos Cavern Right Side Pot 1"), Some(true));
        assert_eq!(probe(&d, &w, "Heart Container", "Dodongos Cavern Right Side Pot 1"), Some(true));
        assert_eq!(probe(&d, &w, "Recovery Heart", pot), Some(true));
        assert_eq!(probe(&d, &w, "Progressive Strength Upgrade", pot), Some(true));
        assert_eq!(probe(&d, &w, "Piece of Heart", pot), Some(true));
        assert_eq!(probe(&d, &w, "Ocarina", pot), Some(true));
        assert_eq!(probe(&d, &w, "Small Key (Forest Temple)", pot), Some(true));
        assert_eq!(probe(&d, &w, "Deku Nuts (5)", chest), None);
        assert_eq!(probe(&d, &w, "Fairy Drop", chest), Some(false));
        assert_eq!(probe(&d, &w, "Progressive Hookshot", chest), None);
        assert_eq!(probe(&d, &w, "Rupees (5)", chest), None);
    }

    fn with(pairs: &[(&str, &str)]) -> Settings {
        let mut s: Settings = Settings::default();
        for (k, v) in pairs {
            s.set(k, Value::s(v)).unwrap();
        }
        s
    }

    fn beatable(d: &Data, s: &Settings, g: &Generated) -> bool {
        let p: Plan = plan(s).unwrap();
        let w: World = world_with(d, &p.logic()).unwrap();
        let mut pl: Placement = vanilla_place(&w, d).unwrap();
        for (loc, item) in &g.seed_file.placements {
            pl.put(w.loc(loc).unwrap(), w.item(item).unwrap());
        }
        explore(&w, &pl, &[]).won()
    }

    #[test]
    fn pot_seeds_are_beatable_and_obey_the_pickup_rules() {
        let d: Data = Data::load().unwrap();
        let mut moved: usize = 0;
        for (chests, pots) in [("all", "all"), ("off", "all"), ("all", "dungeons")] {
            let s: Settings = with(&[("shuffle_chests", chests), ("shuffle_pots", pots), ("shuffle_crates", "all")]);
            let p: Plan = plan(&s).unwrap();
            let w: World = world_with(&d, &p.logic()).unwrap();
            for n in 0..3 {
                let g: Generated = generate_data(&d, &s, &format!("pots {chests} {pots} {n}"), &GenOpts::default()).unwrap();
                assert!(beatable(&d, &s, &g), "{chests} {pots} {n}");
                let mut here: usize = 0;
                for (loc, item) in &g.seed_file.placements {
                    if d.pots.has(loc) {
                        here += 1;
                        let ok: bool = pickup::of(item).is_some() || crate::slice::key_spots::placeable(&d, item, crate::slice::key_spots::Spot::Pot);
                        assert!(ok, "{loc} holds {item}");
                        assert!(d.pots.kind_of(loc).is_some(), "{loc} holds {item} but is no holder kind this build knows");
                        if d.vanilla_of(loc) != Some(item.as_str()) {
                            moved += 1;
                        }
                    } else if d.pots.only.contains(item) {
                        // Pot junk may leave the pots now (any item may go to any shuffled spot), but only
                        // to a spot the build has a form for it at.
                        assert!(crate::slice::generate::buildable(&d, &w, &p, w.item(item).unwrap(), w.loc(loc).unwrap()), "{loc} holds {item}");
                    }
                }
                assert!(here > 20, "{here}");
            }
        }
        assert!(moved > 10, "{moved}");
    }

    #[test]
    fn pots_together_with_rewards_hearts_and_roaming_keys_still_generate() {
        let d: Data = Data::load().unwrap();
        let mut s: Settings = with(&[("shuffle_pots", "all"), ("shuffle_crates", "all"), ("shuffle_rewards", "supported"), ("shuffle_smallkeys", "any_dungeon")]);
        s.set("shuffle_boss_hearts", Value::Bool(true)).unwrap();
        for n in 0..2 {
            let g: Generated = generate_data(&d, &s, &format!("busy {n}"), &GenOpts::default()).unwrap();
            assert!(beatable(&d, &s, &g), "{n}");
            assert!(g.seed_file.placements.keys().any(|k| d.pots.has(k)));
        }
    }

    #[test]
    fn the_default_settings_never_touch_a_pot() {
        let d: Data = Data::load().unwrap();
        let g: Generated = generate_data(&d, &Settings::default(), "default pots", &GenOpts::default()).unwrap();
        assert!(g.seed_file.placements.keys().all(|k| !d.pots.has(k)));
    }

    #[test]
    fn a_dungeon_only_setting_keeps_pots_inside_dungeons() {
        let d: Data = Data::load().unwrap();
        let s: Settings = with(&[("shuffle_pots", "dungeons")]);
        let p: Plan = plan(&s).unwrap();
        let w: World = world_with(&d, &p.logic()).unwrap();
        let g: Generated = generate_data(&d, &s, "dungeon pots", &GenOpts::default()).unwrap();
        for k in g.seed_file.placements.keys().filter(|k| d.pots.has(k)) {
            assert!(w.regions[w.locs[w.loc(k).unwrap()].region].dungeon.is_some(), "{k}");
        }
    }

    #[test]
    fn a_data_set_without_pots_never_has_an_opinion() {
        let d: Data = Data::load().unwrap();
        let w: World = world(&d).unwrap();
        let mut e: Data = d.clone();
        e.pots = Default::default();
        assert_eq!(probe(&e, &w, "Deku Nuts (5)", "KF Midos Top Left Chest"), None);
    }

    #[test]
    fn a_beehive_takes_any_item_with_a_pickup() {
        let d: Data = Data::load().unwrap();
        let w: World = world(&d).unwrap();
        let hive: &str = "Kak Hole Beehive 3";
        assert_eq!(crate::slice::copies::copies(&d, hive), 3);
        assert_eq!(probe(&d, &w, "Rupees (20)", hive), Some(true));
        assert_eq!(probe(&d, &w, "Recovery Heart", hive), Some(true));
        assert_eq!(probe(&d, &w, "Rupees (5)", hive), Some(true));
        assert_eq!(probe(&d, &w, "Arrows (10)", hive), Some(true));
        assert_eq!(probe(&d, &w, "Fire Arrows", hive), Some(true));
        assert_eq!(probe(&d, &w, "Megaton Hammer", hive), Some(true));
        assert_eq!(probe(&d, &w, "Gold Skulltula Token", hive), Some(true));
        assert_eq!(probe(&d, &w, "Piece of Heart", hive), Some(true));
        assert_eq!(probe(&d, &w, "Heart Container", hive), Some(true));
        assert_eq!(probe(&d, &w, "Small Key (Forest Temple)", hive), Some(true));
        assert_eq!(probe(&d, &w, "Progressive Hookshot", hive), Some(true));
    }

    #[test]
    fn beehive_seeds_are_beatable_and_hold_what_a_hive_of_three_copies_can_hold() {
        let d: Data = Data::load().unwrap();
        let mut moved: usize = 0;
        for chests in ["all", "off"] {
            let mut s: Settings = with(&[("shuffle_chests", chests), ("shuffle_pots", "all"), ("shuffle_crates", "all")]);
            s.set(crate::pot_slice::plan::HIVES, Value::Bool(true)).unwrap();
            for n in 0..3 {
                let g: Generated = generate_data(&d, &s, &format!("hives {chests} {n}"), &GenOpts::default()).unwrap();
                assert!(beatable(&d, &s, &g), "{chests} {n}");
                let hives: Vec<(&String, &String)> = g.seed_file.placements.iter().filter(|(k, _)| d.pots.kind_of(k) == Some(crate::pot_slice::data::Kind::Hive)).collect();
                assert_eq!(hives.len(), 7, "{chests} {n}");
                for (loc, item) in hives {
                    assert!(at(&d, loc, item) == Some(true) || d.vanilla_of(loc) == Some(item.as_str()), "{loc} holds {item}");
                    if d.vanilla_of(loc) != Some(item.as_str()) {
                        moved += 1;
                    }
                }
            }
        }
        assert!(moved > 0, "{moved}");
    }

    #[test]
    fn beehives_alone_never_move_a_pot_or_a_crate() {
        let d: Data = Data::load().unwrap();
        let mut s: Settings = Settings::default();
        s.set(crate::pot_slice::plan::HIVES, Value::Bool(true)).unwrap();
        let g: Generated = generate_data(&d, &s, "hives only", &GenOpts::default()).unwrap();
        assert!(beatable(&d, &s, &g));
        assert!(g.seed_file.placements.keys().filter(|k| d.pots.has(k)).all(|k| d.pots.kind_of(k) == Some(crate::pot_slice::data::Kind::Hive)));
        assert!(g.seed_file.placements.keys().any(|k| d.pots.has(k)));
    }

    #[test]
    fn the_default_settings_never_touch_a_beehive() {
        let d: Data = Data::load().unwrap();
        let g: Generated = generate_data(&d, &Settings::default(), "default hives", &GenOpts::default()).unwrap();
        assert!(g.seed_file.placements.keys().all(|k| !k.starts_with("Kak Hole Beehive")));
        let s: Settings = with(&[("shuffle_pots", "all"), ("shuffle_crates", "all")]);
        let g: Generated = generate_data(&d, &s, "pots no hives", &GenOpts::default()).unwrap();
        assert!(g.seed_file.placements.keys().all(|k| !k.starts_with("Kak Hole Beehive")));
    }

    #[test]
    fn ground_spots_get_rupees_as_often_as_pots_when_both_are_shuffled() {
        let d: Data = Data::load().unwrap();
        let s: Settings = with(&[("shuffle_freestanding_items", "all"), ("shuffle_pots", "all"), ("shuffle_crates", "all")]);
        let mut n: [[usize; 3]; 2] = [[0; 3]; 2];
        for seed in ["ground a", "ground b", "ground c"] {
            let g: Generated = generate_data(&d, &s, seed, &GenOpts::default()).unwrap();
            for (loc, item) in &g.seed_file.placements {
                let at: usize = if d.cat.by_name(loc).is_some_and(crate::slice::pickups::is_world) {
                    0
                } else if d.pots.has(loc) {
                    1
                } else {
                    continue;
                };
                if d.item(item).is_none_or(|x| x.adv) {
                    continue;
                }
                n[at][0] += 1;
                if item.starts_with("Rupee") {
                    n[at][1] += 1;
                }
                if item == "Recovery Heart" {
                    n[at][2] += 1;
                }
            }
        }
        let share = |at: usize, k: usize| n[at][k] as f64 / n[at][0].max(1) as f64;
        assert!(n[0][0] > 100 && n[1][0] > 100, "{n:?}");
        assert!(share(0, 1) > share(1, 1) * 0.6, "ground rupees {n:?}");
        assert!(share(0, 2) < share(1, 2) * 1.8 + 0.05, "ground hearts {n:?}");
    }
}
