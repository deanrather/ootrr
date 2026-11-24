// Ice Cavern checked against its remake actors and the Trophy Tom walkthrough:
// docs/dungeon-logic/ice-cavern.md.
#[cfg(test)]
mod tests {
    use crate::slice::child_logic::tests::{build, reach, region_of};
    use crate::slice::data::Data;
    use crate::world::World;

    const BLUE_FIRE: [&str; 2] = ["Blue Fire", "Buy Blue Fire"];
    const PUSH_BLOCK: [&str; 2] = ["Silver Rupee (Ice Cavern Push Block)", "Silver Rupee Pouch (Ice Cavern Push Block)"];

    #[test]
    fn ice_cavern_pots_sit_in_the_room_of_their_actor() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let rows: [(&str, &str); 4] = [
            // room 1, frozen in red ice before the Freezard gate
            ("Ice Cavern Spinning Scythe Room Remake Pot 2", "Ice Cavern Beginning"),
            // room 11, beside the last blue fire
            ("Ice Cavern Spinning Scythe Room Remake Pot 1", "Ice Cavern End"),
            ("Ice Cavern Behind Ice Walls Remake Pot 1", "Ice Cavern Behind Ice Walls"),
            ("Ice Cavern Behind Ice Walls Remake Pot 2", "Ice Cavern Behind Ice Walls"),
        ];
        for (loc, region) in rows {
            assert_eq!(region_of(&w, loc), region, "{loc}");
        }
    }

    #[test]
    fn the_ice_cavern_red_ice_wants_blue_fire() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let frozen: [&str; 4] = [
            "Ice Cavern Spinning Scythe Room Remake Pot 2",
            "Ice Cavern Behind Ice Walls Remake Pot 1",
            "Ice Cavern Map Chest",
            "Ice Cavern Push Block Silver Rupee Red Ice",
        ];
        assert!(reach(&w, &d, &BLUE_FIRE, "Ice Cavern Beginning"));
        assert!(reach(&w, &d, &BLUE_FIRE, "Ice Cavern Hall Pot 1"));
        for loc in frozen {
            assert!(reach(&w, &d, &[], loc), "{loc}");
            assert!(!reach(&w, &d, &BLUE_FIRE, loc), "{loc}");
        }
    }

    #[test]
    fn the_last_ice_cavern_pot_waits_past_the_push_block_rupees() {
        let d: Data = Data::load().unwrap();
        let w: World = build(&d);
        let pot: &str = "Ice Cavern Spinning Scythe Room Remake Pot 1";
        assert!(reach(&w, &d, &PUSH_BLOCK, "Ice Cavern Compass Chest"));
        assert!(reach(&w, &d, &[], pot));
        assert!(!reach(&w, &d, &PUSH_BLOCK, pot));
        assert!(!reach(&w, &d, &PUSH_BLOCK, "Ice Cavern Iron Boots Chest"));
    }
}
