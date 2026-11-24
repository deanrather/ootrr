#[cfg(test)]
mod tests {
    use crate::logic_items::ItemId;
    use crate::search::{explore, Search};
    use crate::slice::data::Data;
    use crate::slice::fill::vanilla_place;
    use crate::slice::logic::world_with;
    use crate::world::{Placement, World};

    const KEY: &str = "Small Key (Ganons Castle)";

    // Every advancement item three times except the ones named in `skip` and the castle key,
    // then `keys` castle keys. Skipped items and the vanilla key leave the placement, and
    // Farore's Wind is always left out, since its warp reaches any spot set earlier.
    fn reach(d: &Data, skip: &[&str], keys: usize, at: &str) -> bool {
        let w: World = world_with(d, &[("open_door_of_time", "open")]).unwrap();
        let k: ItemId = w.item(KEY).unwrap();
        let mut names: Vec<&str> = skip.to_vec();
        names.push("Farores Wind");
        let gone: Vec<ItemId> = names.iter().map(|n| w.item(n).unwrap()).collect();
        let mut p: Placement = vanilla_place(&w, d).unwrap();
        for l in &w.real {
            if p.get(*l).is_some_and(|i| i == k || gone.contains(&i)) {
                p.item[*l] = None;
            }
        }
        let mut extra: Vec<ItemId> = Vec::new();
        for i in 0..w.items.names.len() {
            if w.items.adv[i] && i != k && !gone.contains(&i) {
                extra.extend([i; 3]);
            }
        }
        extra.extend(vec![k; keys]);
        let s: Search = explore(&w, &p, &extra);
        match w.loc(at) {
            Some(l) => s.loc_access(l),
            None => s.region_reached(w.region(at).unwrap(), None),
        }
    }

    #[test]
    fn the_fire_and_light_trials_want_the_golden_gauntlets() {
        let d: Data = Data::load().unwrap();
        let gold: [&str; 1] = ["Progressive Strength Upgrade"];
        // Without any strength upgrade the castle itself is still open.
        assert!(reach(&d, &gold, 1, "Ganons Castle Water Trial Ending"));
        for at in ["Ganons Castle Fire Trial Ending", "Ganons Castle Light Trial"] {
            assert!(reach(&d, &[], 1, at), "{at}");
            assert!(!reach(&d, &gold, 1, at), "{at} without the gauntlets");
        }
    }

    // The Lullaby chest and the invisible key chest share the room before the locked door.
    #[test]
    fn the_lullaby_chest_needs_no_key_and_the_boulder_room_needs_one() {
        let d: Data = Data::load().unwrap();
        assert!(reach(&d, &[], 0, "Ganons Castle Light Trial Lullaby Chest"));
        assert!(!reach(&d, &[], 0, "Ganons Castle Light Trial Boulder Room"));
        assert!(reach(&d, &[], 1, "Ganons Castle Light Trial Boulder Room"));
    }

    #[test]
    fn the_forest_trial_rupee_on_the_block_wants_the_song_of_time() {
        let d: Data = Data::load().unwrap();
        assert!(reach(&d, &[], 0, "Ganons Castle Forest Trial Silver Rupee Front Left"));
        assert!(!reach(&d, &["Song of Time"], 0, "Ganons Castle Forest Trial Silver Rupee Front Left"));
    }
}
