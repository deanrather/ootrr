use std::collections::{BTreeMap as BM, BTreeSet as BS};

use crate::byml::{Byml, Node};
use crate::catalog::Loc;
use crate::error::{Error as E, Res};
use crate::romfs::Romfs;
use crate::slice::banc_edit::Edited;
use crate::slice::data::Data;

pub const REPEAT: [&str; 18] = [
    "Recovery Heart",
    "Fairy Drop",
    "Rupee (1)",
    "Rupees (5)",
    "Rupees (20)",
    "Rupees (50)",
    "Rupees (200)",
    "Deku Nuts (5)",
    "Deku Nuts (10)",
    "Deku Stick (1)",
    "Deku Seeds (30)",
    "Bomb (1)",
    "Bombs (5)",
    "Bombs (10)",
    "Bombs (20)",
    "Arrows (5)",
    "Arrows (10)",
    "Arrows (30)",
];

pub const TWICE: [&str; 23] = [
    "Boomerang",
    "Lens of Truth",
    "Megaton Hammer",
    "Kokiri Sword",
    "Biggoron Sword",
    "Deku Shield",
    "Hylian Shield",
    "Mirror Shield",
    "Goron Tunic",
    "Zora Tunic",
    "Iron Boots",
    "Hover Boots",
    "Stone of Agony",
    "Gerudo Membership Card",
    "Fire Arrows",
    "Ice Arrows",
    "Light Arrows",
    "Dins Fire",
    "Farores Wind",
    "Nayrus Love",
    "Bombchus (5)",
    "Bombchus (10)",
    "Bombchus (20)",
];

pub fn twice(item: &str) -> bool {
    REPEAT.contains(&item) || TWICE.contains(&item)
}

pub fn hashes(l: &Loc) -> usize {
    l.refs.iter().map(|r| r.hash).collect::<BS<u64>>().len()
}

pub fn copies(d: &Data, name: &str) -> usize {
    let l: Option<&Loc> = d.pots.get(name).or_else(|| d.cat.by_name(name));
    l.map_or(0, hashes)
}

pub fn fits(d: &Data, name: &str, item: &str) -> bool {
    copies(d, name) < 2 || twice(item) || d.vanilla_of(name) == Some(item)
}

pub const FLAGGED: [&str; 2] = ["scrub", "cow"];

pub fn scene(file: &str) -> &str {
    let stem: &str = file.rsplit('/').next().unwrap_or(file);
    let stem: &str = stem.split('.').next().unwrap_or(stem);
    stem.strip_suffix("_Static").or_else(|| stem.strip_suffix("_Dynamic")).unwrap_or(stem)
}

pub fn plan(l: &Loc) -> Option<(u64, Vec<(String, u64)>)> {
    if FLAGGED.contains(&l.kind.as_str()) {
        return None;
    }
    let mut by: BM<&str, BS<u64>> = BM::new();
    for r in &l.refs {
        for f in &r.files {
            by.entry(scene(f)).or_default().insert(r.hash);
        }
    }
    if by.len() < 2 {
        return None;
    }
    let multi: Vec<&BS<u64>> = by.values().filter(|h| h.len() > 1).collect();
    let primary: u64 = match multi.as_slice() {
        [] => l.refs.first()?.hash,
        [one] => *one.iter().next()?,
        _ => return None,
    };
    let mut out: Vec<(String, u64)> = Vec::new();
    for r in l.refs.iter().filter(|r| r.hash != primary) {
        for f in &r.files {
            if by.get(scene(f)).is_some_and(|h| h.len() == 1) {
                out.push((f.clone(), r.hash));
            }
        }
    }
    out.sort();
    out.dedup();
    (!out.is_empty()).then_some((primary, out))
}

pub fn shared(d: &Data, placements: &BM<String, String>) -> BM<(String, u64), u64> {
    let mut out: BM<(String, u64), u64> = BM::new();
    for (name, item) in placements {
        if fits(d, name, item) {
            continue;
        }
        let Some(l) = d.pots.get(name).or_else(|| d.cat.by_name(name)) else {
            continue;
        };
        if l.refs.iter().any(|r| crate::special::of(r.hash).is_some()) {
            continue;
        }
        if let Some((primary, copies)) = plan(l) {
            for (f, h) in copies {
                out.insert((f, h), primary);
            }
        }
    }
    out
}

fn swap_hash(n: &mut Node, old: u64, new: u64) -> usize {
    match n {
        Node::U64(v) if *v == old => {
            *v = new;
            1
        }
        Node::Arr(a) => a.iter_mut().map(|x| swap_hash(x, old, new)).sum(),
        Node::Map(m) => m.iter_mut().map(|(_, x)| swap_hash(x, old, new)).sum(),
        _ => 0,
    }
}

fn holds(n: &Node, h: u64) -> bool {
    match n {
        Node::U64(v) => *v == h,
        Node::Arr(a) => a.iter().any(|x| holds(x, h)),
        Node::Map(m) => m.iter().any(|(_, x)| holds(x, h)),
        _ => false,
    }
}

pub fn share_file(y: &mut Byml, file: &str, swaps: &[(u64, u64)]) -> Res<usize> {
    let mut n: usize = 0;
    for (old, new) in swaps {
        if holds(&y.root, *new) {
            return Err(E::Msg(format!("{file} already holds hash {new}, so the copy {old} cannot take it")));
        }
        let k: usize = swap_hash(&mut y.root, *old, *new);
        if k == 0 {
            return Err(E::Msg(format!("the copy {old} was not found in {file}, the game files differ")));
        }
        n += k;
    }
    Ok(n)
}

pub fn share(rf: &mut Romfs, d: &Data, placements: &BM<String, String>, edits: &mut Vec<Edited>) -> Res<usize> {
    let mut by: BM<String, Vec<(u64, u64)>> = BM::new();
    for ((f, old), new) in shared(d, placements) {
        by.entry(f).or_default().push((old, new));
    }
    let mut n: usize = 0;
    for (f, swaps) in by {
        let i: usize = match edits.iter().position(|e| e.file == f) {
            Some(i) => i,
            None => {
                let raw: Vec<u8> = rf.read_zs(&f)?;
                edits.push(Edited { file: f.clone(), old_len: raw.len(), bytes: raw, hits: Vec::new(), used: Vec::new(), hearts: Vec::new(), wraps: Vec::new(), touched: Vec::new() });
                edits.len() - 1
            }
        };
        let mut y: Byml = Byml::parse(&edits[i].bytes)?;
        n += share_file(&mut y, &f, &swaps)?;
        edits[i].bytes = y.write()?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Ref;
    use crate::pot_slice::rules;
    use crate::settings::{Settings, Value};
    use crate::slice::generate::{generate_data, GenOpts, Generated};
    use crate::slice::pickups;

    #[test]
    fn a_hash_counts_once_even_when_it_sits_in_several_files() {
        let d: Data = Data::load().unwrap();
        let kak: &Loc = d.cat.by_name("Kak Impas House Freestanding PoH").unwrap();
        assert!(kak.refs.iter().map(|r| r.files.len()).sum::<usize>() >= 3);
        assert_eq!(hashes(kak), 1);
        let kf: &Loc = d.cat.by_name("KF Sarias House Recovery Heart 1").unwrap();
        assert_eq!(hashes(kf), 2);
        let mut twin: Loc = kf.clone();
        twin.refs = vec![Ref { hash: 7, files: vec!["a".to_string()] }, Ref { hash: 7, files: vec!["b".to_string()] }];
        assert_eq!(hashes(&twin), 1);
        assert_eq!(copies(&d, "no such place"), 0);
    }

    #[test]
    fn a_spot_with_copies_takes_any_item_and_shares_one_hash_when_it_counts() {
        let d: Data = Data::load().unwrap();
        let kf: &str = "KF Top of Sarias Recovery Heart 1";
        assert_eq!(copies(&d, kf), 2);
        assert!(fits(&d, kf, "Recovery Heart"));
        assert!(!fits(&d, kf, "Piece of Heart"));
        assert_eq!(pickups::at(&d, kf, "Piece of Heart"), Some(true));
        let pot: &str = "ZF Hidden Cave Pot 2";
        assert_eq!(rules::at(&d, pot, "Heart Container"), Some(true));
        let mut placed: BM<String, String> = BM::new();
        placed.insert(kf.to_string(), "Piece of Heart".to_string());
        placed.insert("LLR Freestanding PoH".to_string(), "Progressive Hookshot".to_string());
        placed.insert("ZF Hidden Cave Pot 3".to_string(), "Rupees (20)".to_string());
        let s: BM<(String, u64), u64> = shared(&d, &placed);
        assert_eq!(s.len(), 1, "{s:?}");
        let ((f, old), new) = s.iter().next().unwrap();
        assert!(f.ends_with("KokiriForest_Child_Dynamic.bcett.byml.zs"), "{f}");
        assert_eq!((*old, *new), (9778104484775444655, 12317452644253046635));
    }

    #[test]
    fn only_variants_with_one_hash_take_the_primary() {
        let d: Data = Data::load().unwrap();
        let ledge: &Loc = d.cat.by_name("KF West Ledge Chest").unwrap();
        let (primary, moved) = plan(ledge).unwrap();
        assert!(moved.iter().all(|(f, h)| f.contains("LostWood") && *h != primary), "{moved:?}");
        assert!(plan(d.cat.by_name("Hideout 1 Torch Jail Gerudo Key").unwrap()).is_none());
        assert!(plan(d.cat.by_name("LW Deku Scrub Near Bridge").unwrap()).is_none());
        assert!(plan(d.cat.by_name("DMT Cow Grotto Cow").unwrap()).is_none());
        assert_eq!(scene("Banc/ShadowTemple/ShadowTemple_Static.bcett.byml.zs"), scene("Banc/ShadowTemple/ShadowTemple_Dynamic.bcett.byml.zs"));
        assert_ne!(scene("Banc/KakarikoVillage/KakarikoVillage_Adult_Dynamic.bcett.byml.zs"), scene("Banc/KakarikoVillage/KakarikoVillage_Adult_Alternative_Dynamic.bcett.byml.zs"));
        let mut twin: Loc = ledge.clone();
        twin.refs = vec![Ref { hash: 1, files: vec!["Banc/A/A_Child_Dynamic.bcett.byml.zs".to_string()] }, Ref { hash: 2, files: vec!["Banc/A/A_Child_Dynamic.bcett.byml.zs".to_string()] }, Ref { hash: 3, files: vec!["Banc/A/A_Adult_Dynamic.bcett.byml.zs".to_string()] }];
        assert_eq!(plan(&twin), Some((1, vec![("Banc/A/A_Adult_Dynamic.bcett.byml.zs".to_string(), 3)])));
    }

    #[test]
    fn every_copy_in_the_dump_takes_its_primary_hash_without_a_clash() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {}", p.display());
            return;
        }
        let d: Data = Data::load().unwrap();
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let mut by: BM<String, BS<(u64, u64)>> = BM::new();
        let mut spots: usize = 0;
        for l in d.cat.locs.iter().chain(d.pots.locs.iter()) {
            if let Some((primary, moved)) = plan(l) {
                spots += 1;
                for (f, h) in moved {
                    by.entry(f).or_default().insert((h, primary));
                }
            }
        }
        assert!(spots > 30, "{spots}");
        for (f, swaps) in &by {
            let mut y: Byml = Byml::parse(&rf.read_zs(f).unwrap()).unwrap();
            let swaps: Vec<(u64, u64)> = swaps.iter().copied().collect();
            let n: usize = share_file(&mut y, f, &swaps).unwrap();
            assert!(n >= 2 * swaps.len(), "{f}: {n}");
            let hashes: Vec<u64> = y.root.get("Actors").and_then(|a| a.as_arr()).unwrap().iter().filter_map(|a| match a.get("Hash") {
                Some(Node::U64(h)) => Some(*h),
                _ => None,
            }).collect();
            let unique: BS<u64> = hashes.iter().copied().collect();
            assert_eq!(unique.len(), hashes.len(), "{f} holds one hash twice");
        }
    }

    #[test]
    fn seeds_share_the_hash_of_every_counted_item_on_a_spot_with_copies() {
        let d: Data = Data::load().unwrap();
        let mut s: Settings = Settings::default();
        s.set("shuffle_rewards", Value::s("off")).unwrap();
        s.set(pickups::SETTING, Value::s("all")).unwrap();
        s.set("shuffle_pots", Value::s("all")).unwrap();
        let mut counted: usize = 0;
        for n in 0..6 {
            let g: Generated = generate_data(&d, &s, &format!("copies {n}"), &GenOpts::default()).unwrap();
            let s: BM<(String, u64), u64> = shared(&d, &g.seed_file.placements);
            for (loc, item) in &g.seed_file.placements {
                let Some(l) = d.pots.get(loc).or_else(|| d.cat.by_name(loc)) else { continue };
                if fits(&d, loc, item) || plan(l).is_none() {
                    continue;
                }
                counted += 1;
                let (primary, moved) = plan(l).unwrap();
                assert!(moved.iter().all(|(f, h)| s.get(&(f.clone(), *h)) == Some(&primary)), "{item} at {loc}");
            }
        }
        assert!(counted > 0);
    }
}
