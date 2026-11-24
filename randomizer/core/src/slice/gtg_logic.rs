// Gerudo Training Ground checked against its remake actors and the Trophy Tom walkthrough:
// docs/dungeon-logic/gerudo-training-ground.md.
#[cfg(test)]
mod tests {
    use crate::slice::child_logic::tests::{build, reach, region_of};
    use crate::slice::data::Data;
    use crate::world::World;

    const BLAST: [&str; 5] = ["Bomb Bag", "Bombchus", "Bombchus (5)", "Bombchus (10)", "Bombchus (20)"];

    // The Like Like room opens from the Wolfos room: silver block, then an invisible eye switch.
    #[test]
    fn the_like_like_room_wants_the_silver_block_and_the_lens() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let room: &str = "Gerudo Training Ground Like Like Room";
        assert!(reach(&w, &d, &[], room));
        assert!(!reach(&w, &d, &["Progressive Strength Upgrade"], room));
        assert!(!reach(&w, &d, &["Lens of Truth"], room));
        // The Lens is not needed in game, but logic asks for it unless the trick is on.
        assert!(!reach(&w, &d, &["Lens of Truth"], "Gerudo Training Ground Eye Statue Upper"));
        assert!(reach(&w, &d, &["Lens of Truth"], "Gerudo Training Ground Before Heavy Block Chest"));
    }

    #[test]
    fn gtg_eye_switches_take_the_hookshot() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let no_arrows: [&str; 2] = ["Bow", "Slingshot"];
        assert!(reach(&w, &d, &no_arrows, "Gerudo Training Ground Lobby Right Chest"));
        assert!(!reach(&w, &d, &["Bow", "Slingshot", "Progressive Hookshot"], "Gerudo Training Ground Lobby Right Chest"));
    }

    #[test]
    fn the_gtg_beamos_falls_to_the_hammer() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let chest: &str = "Gerudo Training Ground Beamos Chest";
        assert!(reach(&w, &d, &BLAST, chest));
        let mut none: Vec<&str> = BLAST.to_vec();
        none.push("Megaton Hammer");
        assert!(!reach(&w, &d, &none, chest));
    }

    // Two remake chests without a drop actor are paired by their in-game default
    // (banc::DEFAULT_DROPS); the trap chest of the Like Like room stays out.
    #[test]
    fn the_invisible_lobby_chest_and_the_hammer_room_chest_are_checks() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let left: &str = "Gerudo Training Ground Lobby Left Chest";
        let clear: &str = "Gerudo Training Ground Hammer Room Clear Chest";
        for (loc, hash) in [(left, 11182647876438515984u64), (clear, 4961588221597286817)] {
            let l = w.loc(loc).unwrap();
            assert!(w.real.contains(&l), "{loc}");
            assert!(crate::slice::banc::default_drop(hash).is_some(), "{loc}");
        }
        assert_eq!(region_of(&w, left), "Gerudo Training Ground Lobby");
        assert_eq!(region_of(&w, clear), "Gerudo Training Ground Hammer Room");
        assert!(reach(&w, &d, &[], left));
        assert!(!reach(&w, &d, &["Lens of Truth"], left));
        assert!(!reach(&w, &d, &["Megaton Hammer"], clear));
    }
}
