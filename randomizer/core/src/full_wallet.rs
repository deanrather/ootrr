pub mod fill;

use crate::on_get::OnGet;
use crate::start_items::{Kit, Set};

pub const SETTING: &str = "full_wallets";
pub const NUM: u16 = 610;
pub const WALLETS: [&str; 2] = ["Item_Wallet_Medium", "Item_Wallet_Large"];
pub const ON_GET: OnGet = OnGet { name: "Full Wallets", enabled: |p| p.full_wallet, event_prefix: fill::PREFIX, items: &WALLETS, add_events: fill::fill, new_game_writes: start_sets };
pub const COUNT: &str = "PlayerStatus.CurrentRupee";
pub const FIRST: (&str, i32) = ("Item_Wallet_Small", 200);
pub const CAPS: [(&str, i32); 2] = [("Item_Wallet_Medium", 500), ("Item_Wallet_Large", 1000)];
pub const FLOWS: &str = "Event/EventFlow";
pub const NOTE: &str = "Full Wallets: the new-game save sets the rupee count to the best starting wallet's capacity; after each Adult's or Giant's Wallet give (chest, person, reward, shop, pickup) the event flows open the rupee counter and add 1000 rupees, which the game caps at the wallet's capacity. Seen in the game: the chest give. The other gives and the new-game count are not yet seen.";

pub fn cap(actor: &str) -> Option<i32> {
    CAPS.iter().find(|(a, _)| *a == actor).map(|(_, n)| *n)
}

pub fn start_cap(k: &Kit) -> i32 {
    CAPS.iter().filter(|(a, _)| k.pushes.contains(a)).map(|(_, n)| *n).max().unwrap_or(FIRST.1)
}

pub fn start_sets(k: &Kit) -> Vec<Set> {
    vec![Set::Int(COUNT.to_string(), start_cap(k))]
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf as PB;

    use super::*;
    use crate::modpack::Host;
    use crate::romfs::Romfs;
    use crate::slice::edit::Built;

    #[test]
    fn a_new_game_starts_with_its_best_wallet_full() {
        let mut k: Kit = Kit::default();
        assert_eq!(start_sets(&k), vec![Set::Int(COUNT.to_string(), 200)]);
        k.pushes.push("Item_Wallet_Medium");
        assert_eq!(start_cap(&k), 500);
        k.pushes.push("Item_Wallet_Large");
        assert_eq!(start_sets(&k), vec![Set::Int(COUNT.to_string(), 1000)]);
        assert!(NUM >= 600);
        assert_eq!(cap(FIRST.0), None);
    }

    #[test]
    fn a_full_wallets_build_leaves_the_navi_table_alone() {
        let navi: &str = "GameParameter/NaviMgrGlobalParam/GlobalGameParameterTable.game__navi__NaviMgrGlobalParam.bgyml";
        let d: crate::slice::data::Data = crate::slice::data::Data::load().unwrap();
        assert!(!crate::required::unpack(&d, false).wants(navi));
        let p: PB = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return;
        }
        let mut s: crate::settings::Settings = crate::settings::Settings::default();
        s.set(SETTING, crate::settings::Value::Bool(true)).unwrap();
        let g: crate::slice::generate::Generated = crate::slice::generate::generate_data(&d, &s, "full wallet navi", &crate::slice::generate::GenOpts::default()).unwrap();
        let load: PB = std::env::temp_dir().join(format!("ootrr_full_wallet_navi_{}", std::process::id()));
        let _ = fs::remove_dir_all(&load);
        let b: Built = crate::slice::edit::build_for(&g.seed_file, &p, &load, "fullwallet", Host::Eden).unwrap();
        assert!(b.report.contains("Full Wallets: added at the get points of"), "{}", b.report);
        assert!(!navi.split('/').fold(b.root.join("romfs"), |a, s| a.join(s)).exists());
        let _ = fs::remove_dir_all(&load);
    }

    #[test]
    fn the_fill_tops_up_every_wallet_and_the_caps_match_the_game_wallet_table() {
        assert!(CAPS.iter().all(|(_, n)| *n <= fill::FILL));
        assert_eq!(WALLETS.to_vec(), CAPS.iter().map(|c| c.0).collect::<Vec<&str>>());
        assert_eq!(fill::HOLD, 60);
        assert_eq!(crate::progressive::tiers("Wallet").unwrap().to_vec(), WALLETS.to_vec());
        let p: PB = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let y: crate::byml::Byml = crate::byml::Byml::parse(&rf.read_zs(crate::pickup_pack::rows::POUCH_INFO).unwrap()).unwrap();
        for (a, n) in std::iter::once(FIRST).chain(CAPS) {
            for id in [a.to_string(), a.replace("Item_", "Pouch_")] {
                let r: &crate::byml::Node = crate::pickup_pack::rows::find(&y, &id).unwrap();
                assert_eq!(r.get(crate::pickup_pack::ammo::KIND).and_then(|v| v.as_str()), Some("Rupee"), "{id}");
                assert_eq!(r.get(crate::pickup_pack::ammo::MAX), Some(&crate::byml::Node::I32(n)), "{id}");
            }
        }
    }
}
