pub mod fill;
pub mod sites;

use std::fs;
use std::path::{Path as P, PathBuf as PB};

use crate::bfevfl::Bf;
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::modpack::{Host, Pack};
use crate::pickup_pack::write::start;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst, TABLE_PATH};
use crate::slice::edit::Built;
use crate::staging::MARGIN;
use crate::start_items::{Kit, Set};
use crate::zs;
use crate::TITLE_ID;

pub const SETTING: &str = "full_wallets";
pub const NUM: u16 = 610;
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

fn flow_files(base: &P) -> Res<Vec<String>> {
    let mut out: Vec<String> = sites::fixed().iter().map(|f| sites::path_of(f)).collect();
    let dir: PB = FLOWS.split('/').fold(base.to_path_buf(), |a, s| a.join(s));
    if dir.is_dir() {
        for e in fs::read_dir(&dir)? {
            let name: String = e?.file_name().to_string_lossy().to_string();
            if name.ends_with(".bfevfl.zs") {
                out.push(format!("{FLOWS}/{name}"));
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn local(base: &P, path: &str) -> PB {
    path.split('/').fold(base.to_path_buf(), |a, s| a.join(s))
}

pub fn after(on: bool, romfs: &P, load: &P, mod_name: &str, host: Host, mut b: Built) -> Res<Built> {
    if !on {
        return Ok(b);
    }
    let mut rf: Romfs = Romfs::open(romfs)?;
    let base: PB = b.root.join("romfs");
    let own: PB = base.join(TABLE_PATH);
    let mut rst: Rst = if own.is_file() { Rst::parse(&rf.zs.decompress(&fs::read(&own)?)?)? } else { rsizetable::load(&mut rf)? };
    let pk: Pack = Pack::create_for(load, TITLE_ID, mod_name, host)?;
    if pk.root != b.root {
        return Err(E::Msg(format!("the wallet flows went to {} and not to the seed mod {}", pk.root.display(), b.root.display())));
    }
    let mut lines: Vec<String> = Vec::new();
    for path in flow_files(&base)? {
        let modded: bool = local(&base, &path).is_file();
        let first: Vec<u8> = start(&mut rf, Some(&base), &path)?;
        let mut bf: Bf = Bf::parse(&first)?;
        let mut n: usize = 0;
        for f in bf.flows.iter_mut() {
            n += sites::patch(f)?;
        }
        if n == 0 {
            continue;
        }
        let raw: Vec<u8> = bf.write()?;
        let old: usize = if modded { first.len() } else { rf.read_zs(&path)?.len() };
        pk.write_romfs(&path, &zs::compress_raw(&raw, dict_id(&rf.read(&path)?))?)?;
        if raw.len() > old {
            rst.grow(&path, old as u64, raw.len() as u64, MARGIN)?;
        }
        lines.push(format!("{path} ({n})"));
        b.counts.files += usize::from(!modded);
    }
    pk.write_romfs(TABLE_PATH, &rst.to_zs()?)?;
    b.report.push_str(&format!("Full Wallets: wallet fills added at {} give points: {}\n", lines.len(), lines.join(", ")));
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(b.report.contains("Full Wallets: wallet fills added at"), "{}", b.report);
        assert!(!local(&b.root.join("romfs"), navi).exists());
        let _ = fs::remove_dir_all(&load);
    }

    #[test]
    fn the_fill_tops_up_every_wallet_and_the_caps_match_the_game_wallet_table() {
        assert!(CAPS.iter().all(|(_, n)| *n <= fill::FILL));
        assert_eq!(sites::WALLETS.to_vec(), CAPS.iter().map(|c| c.0).collect::<Vec<&str>>());
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
