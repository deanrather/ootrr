use std::fs;
use std::path::{Path as P, PathBuf as PB};

use crate::byml::{Byml, Node};
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::modpack::{Host, Pack};
use crate::pickup_pack::ammo::{KIND, MAX};
use crate::pickup_pack::rows::{self, POUCH_INFO};
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
pub const FILL: &str = "IsItemStockMaxIfGet";
pub const RUPEE: &str = "Rupee";
pub const COUNT: &str = "PlayerStatus.CurrentRupee";
pub const FIRST: (&str, i32) = ("Item_Wallet_Small", 200);
pub const CAPS: [(&str, i32); 2] = [("Item_Wallet_Medium", 500), ("Item_Wallet_Large", 1000)];
pub const NOTE: &str = "Full Wallets: a new game starts with as many rupees as its best wallet holds, and every Adult's or Giant's Wallet row of the pouch table (with its pickup, shop and reward copies) carries IsItemStockMaxIfGet, so the wallet should come filled. Not yet seen in the game.";

fn num(n: &Node) -> Option<i64> {
    match n {
        Node::I32(v) => Some(i64::from(*v)),
        Node::U32(v) => Some(i64::from(*v)),
        Node::I64(v) => Some(*v),
        _ => None,
    }
}

fn fake(id: &str) -> bool {
    id.starts_with("OOTRR_") && id.ends_with("_Cold")
}

pub fn fills(r: &Node) -> bool {
    r.get(KIND).and_then(|v| v.as_str()) == Some(RUPEE) && r.get(MAX).and_then(num).is_some_and(|n| n > i64::from(FIRST.1)) && !rows::id(r).is_some_and(fake)
}

pub fn fill_all(y: &mut Byml) -> Res<Vec<String>> {
    let list: &mut Vec<Node> = y.root.as_arr_mut().ok_or(E::Format("actor table: the root is not a row list"))?;
    let mut out: Vec<String> = Vec::new();
    for r in list.iter_mut().filter(|r| fills(r)) {
        if r.get(FILL) != Some(&Node::Bool(true)) {
            r.set(FILL, Node::Bool(true));
        }
        out.push(rows::id(r).unwrap_or("").to_string());
    }
    Ok(out)
}

pub fn start_cap(k: &Kit) -> i32 {
    CAPS.iter().filter(|(a, _)| k.pushes.contains(a)).map(|(_, n)| *n).max().unwrap_or(FIRST.1)
}

pub fn start_sets(k: &Kit) -> Vec<Set> {
    vec![Set::Int(COUNT.to_string(), start_cap(k))]
}

pub fn after(on: bool, romfs: &P, load: &P, mod_name: &str, host: Host, mut b: Built) -> Res<Built> {
    if !on {
        return Ok(b);
    }
    let mut rf: Romfs = Romfs::open(romfs)?;
    let base: PB = b.root.join("romfs");
    let own: PB = base.join(TABLE_PATH);
    let mut rst: Rst = if own.is_file() { Rst::parse(&rf.zs.decompress(&fs::read(&own)?)?)? } else { rsizetable::load(&mut rf)? };
    let modded: bool = POUCH_INFO.split('/').fold(base.clone(), |a, s| a.join(s)).is_file();
    let first: Vec<u8> = start(&mut rf, Some(&base), POUCH_INFO)?;
    let old: usize = if modded { first.len() } else { rf.read_zs(POUCH_INFO)?.len() };
    let mut y: Byml = Byml::parse(&first)?;
    let filled: Vec<String> = fill_all(&mut y)?;
    if filled.is_empty() {
        return Err(E::Msg(format!("full wallets: {POUCH_INFO} has no wallet row above {} rupees", FIRST.1)));
    }
    let raw: Vec<u8> = y.write()?;
    let pk: Pack = Pack::create_for(load, TITLE_ID, mod_name, host)?;
    if pk.root != b.root {
        return Err(E::Msg(format!("the wallet table went to {} and not to the seed mod {}", pk.root.display(), b.root.display())));
    }
    pk.write_romfs(POUCH_INFO, &zs::compress_raw(&raw, dict_id(&rf.read(POUCH_INFO)?))?)?;
    if raw.len() > old {
        rst.grow(POUCH_INFO, old as u64, raw.len() as u64, MARGIN)?;
        pk.write_romfs(TABLE_PATH, &rst.to_zs()?)?;
    }
    b.counts.files += 1;
    b.report.push_str(&format!("Full Wallets: {} wallet rows fill on get ({})\n", filled.len(), filled.join(", ")));
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pickup_pack::rows::find;
    use crate::pickup_pack::rows::tests::{row, table};

    fn s(x: &str) -> Node {
        Node::Str(x.to_string())
    }

    fn wallet(id: &str, n: i32) -> Node {
        row(id, &[(KIND, s(RUPEE)), (MAX, Node::I32(n))])
    }

    fn sample() -> Byml {
        table(vec![
            row("Item_BombBag_Small", &[(KIND, s("Bomb")), (MAX, Node::I32(20)), (FILL, Node::Bool(true))]),
            wallet("Item_Wallet_Large", 1000),
            wallet("Item_Wallet_Medium", 500),
            wallet("Item_Wallet_Small", 200),
            wallet("OOTRR_Item_Wallet_Large_Cold", 1000),
            wallet("Obj_Wallet_Medium", 500),
            row("Obj_Rupee_Green", &[("BundleActorNum", Node::I32(1))]),
            wallet("Pouch_Wallet_Small", 200),
        ])
    }

    #[test]
    fn only_wallets_above_the_first_one_fill_and_nothing_else_changes() {
        let was: Byml = sample();
        let mut y: Byml = was.clone();
        let got: Vec<String> = fill_all(&mut y).unwrap();
        assert_eq!(got, vec!["Item_Wallet_Large", "Item_Wallet_Medium", "Obj_Wallet_Medium"]);
        for id in &got {
            let mut want: Node = find(&was, id).unwrap().clone();
            want.set(FILL, Node::Bool(true));
            assert_eq!(find(&y, id), Some(&want), "{id}");
        }
        for id in ["Item_BombBag_Small", "Item_Wallet_Small", "OOTRR_Item_Wallet_Large_Cold", "Obj_Rupee_Green", "Pouch_Wallet_Small"] {
            assert_eq!(find(&y, id), find(&was, id), "{id}");
        }
        let again: Byml = y.clone();
        assert_eq!(fill_all(&mut y).unwrap(), got);
        assert_eq!(y, again);
    }

    #[test]
    fn a_new_game_starts_with_its_best_wallet_full() {
        let mut k: Kit = Kit::default();
        assert_eq!(start_sets(&k), vec![Set::Int(COUNT.to_string(), 200)]);
        k.pushes.push("Item_Wallet_Medium");
        assert_eq!(start_cap(&k), 500);
        k.pushes.push("Item_Wallet_Large");
        assert_eq!(start_sets(&k), vec![Set::Int(COUNT.to_string(), 1000)]);
        assert!(NUM >= 600);
    }

    fn real() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(p)
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    #[test]
    fn the_game_wallets_match_the_caps_and_fill_like_the_bags() {
        let Some(p) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let was: Byml = Byml::parse(&rf.read_zs(POUCH_INFO).unwrap()).unwrap();
        for (a, n) in std::iter::once(FIRST).chain(CAPS) {
            for id in [a.to_string(), a.replace("Item_", "Pouch_")] {
                let r: &Node = find(&was, &id).unwrap();
                assert_eq!(r.get(KIND).and_then(|v| v.as_str()), Some(RUPEE), "{id}");
                assert_eq!(r.get(MAX).and_then(num), Some(i64::from(n)), "{id}");
                assert_eq!(r.get(FILL), None, "{id}");
            }
        }
        assert_eq!(find(&was, "Item_BombBag_Small").unwrap().get(FILL), Some(&Node::Bool(true)));
        let mut y: Byml = was.clone();
        let got: Vec<String> = fill_all(&mut y).unwrap();
        assert_eq!(got, vec!["Item_Wallet_Large", "Item_Wallet_Medium", "Pouch_Wallet_Large", "Pouch_Wallet_Medium"]);
        let back: Byml = Byml::parse(&y.write().unwrap()).unwrap();
        assert_eq!(back, y);
        let all: &[Node] = was.root.as_arr().unwrap();
        let now: &[Node] = back.root.as_arr().unwrap();
        assert_eq!(all.len(), now.len());
        let changed: usize = all.iter().zip(now).filter(|(a, b)| a != b).count();
        assert_eq!(changed, 4);
    }
}
