#[cfg(test)]
mod tests {
    use crate::logic_items::ItemId;
    use crate::search::{explore, Search};
    use crate::slice::data::Data;
    use crate::slice::fill::vanilla_place;
    use crate::slice::logic::world_with;
    use crate::world::{Placement, World};

    // Every advancement item three times except the ones named in `skip`; the skipped items
    // also leave the vanilla placement. Farore's Wind is always left out, since its warp
    // reaches any dungeon spot set earlier.
    fn reach(d: &Data, skip: &[&str], at: &str) -> bool {
        let w: World = world_with(d, &[("open_door_of_time", "open")]).unwrap();
        let mut names: Vec<&str> = skip.to_vec();
        names.push("Farores Wind");
        let gone: Vec<ItemId> = names.iter().map(|n| w.item(n).unwrap()).collect();
        let mut p: Placement = vanilla_place(&w, d).unwrap();
        for l in &w.real {
            if p.get(*l).is_some_and(|i| gone.contains(&i)) {
                p.item[*l] = None;
            }
        }
        let mut extra: Vec<ItemId> = Vec::new();
        for i in 0..w.items.names.len() {
            if w.items.adv[i] && !gone.contains(&i) {
                extra.extend([i; 3]);
            }
        }
        let s: Search = explore(&w, &p, &extra);
        match w.loc(at) {
            Some(l) => s.loc_access(l),
            None => s.region_reached(w.region(at).unwrap(), None),
        }
    }

    // Fake doors the Hammer breaks guard the Boss Key.
    #[test]
    fn the_boss_key_wants_the_hammer() {
        let d: Data = Data::load().unwrap();
        assert!(reach(&d, &[], "Spirit Temple Boss Key Chest"));
        assert!(!reach(&d, &["Megaton Hammer"], "Spirit Temple Boss Key Chest"));
        assert!(reach(&d, &["Megaton Hammer"], "Spirit Temple Big Mirror Room"));
    }

    // The statue room skulltula is reached from Pierre, not by Hookshot or Hover Boots.
    #[test]
    fn the_lobby_skulltula_wants_pierre() {
        let d: Data = Data::load().unwrap();
        assert!(reach(&d, &[], "Spirit Temple GS Lobby"));
        assert!(!reach(&d, &["Scarecrow Song"], "Spirit Temple GS Lobby"));
    }

    // In the three Anubis room the Lullaby raises the platform before the pit door.
    #[test]
    fn the_mirror_shield_side_wants_the_lullaby() {
        let d: Data = Data::load().unwrap();
        assert!(reach(&d, &[], "Spirit Temple Mirror Shield Chest"));
        assert!(!reach(&d, &["Zeldas Lullaby"], "Spirit Temple Beyond Anubis Room"));
    }
}
