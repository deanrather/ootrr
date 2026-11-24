// The three child dungeons checked against their remake actors (rooms and puzzle events
// of the Banc files): docs/dungeon-logic/deku-tree.md, dodongos-cavern.md, jabu-jabus-belly.md.
#[cfg(test)]
pub(crate) mod tests {
    use crate::logic_items::ItemId;
    use crate::search::{explore, Search};
    use crate::slice::data::Data;
    use crate::slice::fill::vanilla_place;
    use crate::slice::logic::world_with;
    use crate::world::{Placement, World};

    const BLAST: [&str; 6] = ["Bomb Bag", "Bombchus", "Bombchus (5)", "Bombchus (10)", "Bombchus (20)", "Megaton Hammer"];
    const FIRE: [&str; 5] = ["Deku Stick Drop", "Buy Deku Stick (1)", "Dins Fire", "Fire Arrows", "Bow"];

    pub(crate) fn build(d: &Data) -> World {
        let on: [(&str, &str); 4] = [
            ("open_door_of_time", "open"),
            (crate::pot_slice::remake::KEY, crate::pot_slice::remake::ON),
            (crate::pot_slice::rocks::KEY, crate::pot_slice::rocks::ON),
            (crate::spots::free::KEY, crate::spots::free::ON),
        ];
        world_with(d, &on).unwrap()
    }

    // Every advancement item three times except the ones named in `skip`, which also leave
    // the vanilla placement and every logic event that hands them out (the Deku Babas give
    // sticks). Event items are not handed out, they come from their events. Farore's Wind is
    // always left out, since its warp reaches any spot set earlier.
    pub(crate) fn reach(w: &World, d: &Data, skip: &[&str], at: &str) -> bool {
        let mut names: Vec<&str> = skip.to_vec();
        names.push("Farores Wind");
        let gone: Vec<ItemId> = names.iter().map(|n| w.item(n).unwrap()).collect();
        let mut w: World = w.clone();
        for l in w.locs.iter_mut() {
            if l.event.is_some_and(|i| gone.contains(&i)) {
                l.event = None;
            }
        }
        let w: &World = &w;
        let mut p: Placement = vanilla_place(w, d).unwrap();
        for l in &w.real {
            if p.get(*l).is_some_and(|i| gone.contains(&i)) {
                p.item[*l] = None;
            }
        }
        let mut extra: Vec<ItemId> = Vec::new();
        for i in 0..w.items.names.len() {
            if w.items.adv[i] && !w.items.event[i] && !gone.contains(&i) {
                extra.extend([i; 3]);
            }
        }
        let s: Search = explore(w, &p, &extra);
        match w.loc(at) {
            Some(l) => s.loc_access(l),
            None => s.region_reached(w.region(at).unwrap(), None),
        }
    }

    pub(crate) fn region_of<'a>(w: &'a World, loc: &str) -> &'a str {
        let l = w.loc(loc).unwrap_or_else(|| panic!("{loc}"));
        w.regions[w.locs[l].region].name.as_str()
    }

    #[test]
    fn remake_spots_sit_in_the_room_of_their_actor() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let rows: [(&str, &str); 22] = [
            ("Deku Tree Before Boss Remake Heart 2", "Deku Tree Compass Room"),
            ("Deku Tree Before Boss Remake Heart 4", "Deku Tree Compass Room"),
            ("Deku Tree Before Boss Remake Heart 5", "Deku Tree Compass Room"),
            ("Deku Tree Before Boss Remake Heart 1", "Deku Tree Before Boss"),
            ("Deku Tree Grass 3", "Deku Tree Compass Room"),
            ("Deku Tree Grass 4", "Deku Tree Compass Room"),
            ("Deku Tree Grass 6", "Deku Tree Slingshot Room"),
            ("Deku Tree Grass 10", "Deku Tree Slingshot Room"),
            ("Deku Tree Grass 11", "Deku Tree Slingshot Room"),
            ("Deku Tree Grass 5", "Deku Tree Basement Water Room"),
            ("Deku Tree Grass 12", "Deku Tree Basement Water Room"),
            ("Deku Tree Grass 13", "Deku Tree Basement Water Room"),
            ("Dodongos Cavern Last Block Pot 4", "Dodongos Cavern Lower Right Side"),
            ("Dodongos Cavern Before Boss Remake Pot 1", "Dodongos Cavern Lower Right Side"),
            ("Dodongos Cavern Before Boss Remake Pot 3", "Dodongos Cavern Lower Right Side"),
            ("Dodongos Cavern Before Boss Remake Pot 4", "Dodongos Cavern Torch Room"),
            ("Dodongos Cavern Before Boss Remake Pot 2", "Dodongos Cavern Upper Lizalfos"),
            ("Dodongos Cavern Lower Right Side Remake Pot 1", "Dodongos Cavern Before Upper Lizalfos"),
            ("Dodongos Cavern Climb Remake Pot 3", "Dodongos Cavern Before Boss"),
            ("King Dodongo Boss Room Remake Pot 1", "Dodongos Cavern Before Boss"),
            ("Jabu Jabus Belly Main Remake Crate 1", "Jabu Jabus Belly Before Boss"),
            ("Jabu Jabus Belly Before Boss Remake Crate 1", "Jabu Jabus Belly Before Boss"),
        ];
        for (loc, region) in rows {
            assert_eq!(region_of(&w, loc), region, "{loc}");
        }
    }

    // The eye switch of the Slingshot Room drops the ladder to the third floor.
    #[test]
    fn the_deku_third_floor_wants_the_slingshot_or_the_boomerang() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let shots: [&str; 2] = ["Slingshot", "Boomerang"];
        for at in ["Deku Tree Compass Chest", "Deku Tree Before Boss Remake Heart 2", "Deku Tree Grass 3"] {
            assert!(reach(&w, &d, &[], at), "{at}");
            assert!(!reach(&w, &d, &shots, at), "{at} without a shot");
        }
        assert!(reach(&w, &d, &shots, "Deku Tree Map Chest"));
    }

    // Bombs open the first web, but the candle room door wants its three timed candles lit.
    #[test]
    fn the_deku_back_room_wants_fire_for_the_candles_even_with_bombs() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let back: &str = "Deku Tree Basement Back Room";
        assert!(reach(&w, &d, &FIRE[2..], back));
        assert!(!reach(&w, &d, &FIRE, back));
        assert!(reach(&w, &d, &FIRE, "Deku Tree Basement Water Room"));
        assert!(reach(&w, &d, &FIRE, "Deku Tree Grass 5"));
        assert!(!reach(&w, &d, &FIRE, "Deku Tree Grass 1"));
    }

    #[test]
    fn the_dodongo_block_room_pots_wait_past_the_mouth() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        assert!(reach(&w, &d, &BLAST, "Dodongos Cavern Climb"));
        assert!(reach(&w, &d, &BLAST, "Dodongos Cavern Last Block Pot 4"));
        for at in ["Dodongos Cavern Climb Remake Pot 3", "King Dodongo Boss Room Remake Pot 1"] {
            assert!(reach(&w, &d, &[], at), "{at}");
            assert!(!reach(&w, &d, &BLAST, at), "{at} without explosives or the Hammer");
        }
    }

    // Room 18 sits behind a cracked wall at the bridge's end; room 20 behind the block room wall.
    #[test]
    fn the_dodongo_bridge_skulltula_wants_the_wall_broken() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let alcove: &str = "Dodongos Cavern GS Alcove Above Stairs";
        assert_eq!(region_of(&w, alcove), "Dodongos Cavern Far Bridge");
        assert!(reach(&w, &d, &BLAST, "Dodongos Cavern Far Bridge"));
        assert!(!reach(&w, &d, &BLAST, alcove));
        assert!(reach(&w, &d, &["Progressive Hookshot", "Boomerang"], alcove));
        let names = crate::slice::skull_names::names().unwrap();
        let name = |h: u64| names.spots.iter().find(|s| s.hash == h).map(|s| s.name.clone()).unwrap();
        assert_eq!(name(4565265801822094851), alcove);
        assert_eq!(name(13690401647626643497), "Dodongos Cavern GS Back Room");
    }

    #[test]
    fn the_jabu_switch_crate_waits_past_big_octo() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let crate_: &str = "Jabu Jabus Belly Main Remake Crate 1";
        assert!(reach(&w, &d, &["Boomerang"], "Jabu Jabus Belly Map Chest"));
        assert!(reach(&w, &d, &[], crate_));
        assert!(!reach(&w, &d, &["Boomerang"], crate_));
    }

    // No N64 loop back to the first room: past Big Octo the route runs straight on to the
    // boss door, so the only dungeon way into Before Boss is from Past Big Octo.
    #[test]
    fn the_jabu_boss_door_is_reached_only_past_big_octo() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let before: &str = "Jabu Jabus Belly Before Boss";
        let into: Vec<&str> = w
            .exits
            .iter()
            .filter(|e| w.regions[e.to].name == before)
            .map(|e| w.regions[e.from].name.as_str())
            .filter(|f| f.starts_with("Jabu"))
            .collect();
        assert_eq!(into, vec!["Jabu Jabus Belly Past Big Octo"]);
        assert!(reach(&w, &d, &[], "Jabu Jabus Belly GS Near Boss"));
        assert!(!reach(&w, &d, &["Boomerang"], "Jabu Jabus Belly GS Near Boss"));
    }

    // The remake entrance wants a strength upgrade at any age; a Bomb Bag alone does not open it.
    #[test]
    fn the_dodongo_entrance_wants_a_strength_upgrade() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let door: &str = "Dodongos Cavern Beginning";
        assert!(reach(&w, &d, &[], door));
        assert!(!reach(&w, &d, &["Progressive Strength Upgrade"], door));
        assert!(reach(&w, &d, &["Progressive Strength Upgrade"], "Death Mountain"));
    }

    // An Armos is hurt only by explosives or the Hammer, and the child's first Spirit room
    // opens its doors only when its Armos falls.
    #[test]
    fn the_spirit_child_side_wants_explosives_for_the_armos() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        // The locked door room is also reached back down the climb with all five keys.
        let bridges: &str = "Child Spirit Temple Beyond Metal Bridges";
        assert!(reach(&w, &d, &[], bridges));
        assert!(!reach(&w, &d, &BLAST, bridges));
        assert!(!reach(&w, &d, &BLAST, "Spirit Temple Child Bridge Chest"));
        assert!(reach(&w, &d, &BLAST, "Spirit Temple Lobby"));
    }
}
