use std::path::Path as P;

use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::romfs::Romfs;
use crate::staging::Out;

pub const ADULT: &str = "Banc/HyruleField/HyruleField_Adult_Dynamic.bcett.byml.zs";
pub const CHILD: &str = "Banc/HyruleField/HyruleField_Child_Dynamic.bcett.byml.zs";
pub const ADULT_WALL: u64 = 9829790505452123995;
pub const CHILD_WALL: u64 = 6295005719083773433;
pub const WALL_GYAML: &str = "FldObj_AirWall_Hookshot";
pub const CHEST: &str = "WaterTemple_Room07_00";
pub const NOTE: &str = "The Gold Skulltula in the tree at Lake Hylia can be hooked as soon as you have the Longshot, wherever it was found. An invisible wall (FldObj_AirWall_Hookshot) in front of it stayed until the Longshot chest in the Water Temple was opened. Its presence now reads Get_StoneOfFire_IsAfter_Ready_Exp (always true) negated, so it never appears.";

pub fn patch(y: &mut Byml, hash: u64) -> Res<()> {
    let actors: &mut Vec<Node> = y.root.get_mut("Actors").and_then(|n| n.as_arr_mut()).ok_or(Error::Format("lake tree wall: the Hyrule Field map has no actors"))?;
    let a: &mut Node = actors
        .iter_mut()
        .find(|a| matches!(a.get("Hash"), Some(Node::U64(h)) if *h == hash))
        .ok_or(Error::Format("lake tree wall: the Hyrule Field map has no Lake Hylia tree wall"))?;
    if a.get("Gyaml").and_then(|g| g.as_str()) != Some(WALL_GYAML) {
        return Err(Error::Format("lake tree wall: the tree wall is not a hookshot air wall"));
    }
    let p: &mut Node = a.get_mut("Presence").ok_or(Error::Format("lake tree wall: the tree wall has no presence"))?;
    if p.get("IsNegation") != Some(&Node::Bool(true)) {
        return Err(Error::Format("lake tree wall: the tree wall's presence is not negated"));
    }
    match p.get_mut("FlagName") {
        Some(n) if n.as_str() == Some(CHEST) => *n = Node::Str(crate::always::NAME.to_string()),
        _ => return Err(Error::Format("lake tree wall: the tree wall does not wait for the Longshot chest")),
    }
    Ok(())
}

pub fn bytes(raw: &[u8], hash: u64) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    patch(&mut y, hash)?;
    let out: Vec<u8> = y.write()?;
    if Byml::parse(&out)?.root != y.root {
        return Err(Error::Format("lake tree wall: the patched map does not read back"));
    }
    Ok(out)
}

pub fn stage(rf: &mut Romfs, more: &mut Vec<Out>, work: &P) -> Res<()> {
    crate::staging::edit_staged(rf, more, work, ADULT, true, |r| bytes(r, ADULT_WALL))?;
    crate::staging::edit_staged(rf, more, work, CHILD, true, |r| bytes(r, CHILD_WALL))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(out: &[u8], hash: u64) -> Option<String> {
        let y: Byml = Byml::parse(out).unwrap();
        let acts: &[Node] = y.root.get("Actors").and_then(|n| n.as_arr()).unwrap();
        let w: &Node = acts.iter().find(|a| a.get("Hash") == Some(&Node::U64(hash)))?;
        w.get("Presence").and_then(|p| p.get("FlagName")).and_then(|n| n.as_str()).map(|s| s.to_string())
    }

    #[test]
    fn the_tree_wall_never_appears() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        for (file, hash) in [(ADULT, ADULT_WALL), (CHILD, CHILD_WALL)] {
            let raw: Vec<u8> = rf.read_zs(file).unwrap();
            assert_eq!(flag(&raw, hash).as_deref(), Some(CHEST));
            let out: Vec<u8> = bytes(&raw, hash).unwrap();
            assert_eq!(flag(&out, hash).as_deref(), Some(crate::always::NAME));
            assert!(bytes(&out, hash).is_err());
            let a: Byml = Byml::parse(&raw).unwrap();
            let b: Byml = Byml::parse(&out).unwrap();
            let n: usize = a.root.get("Actors").and_then(|n| n.as_arr()).unwrap().len();
            assert_eq!(b.root.get("Actors").and_then(|n| n.as_arr()).unwrap().len(), n);
        }
    }

    #[test]
    fn only_the_tree_wall_changes() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        for (file, hash) in [(ADULT, ADULT_WALL), (CHILD, CHILD_WALL)] {
            let raw: Vec<u8> = rf.read_zs(file).unwrap();
            let out: Vec<u8> = bytes(&raw, hash).unwrap();
            let a: Byml = Byml::parse(&raw).unwrap();
            let b: Byml = Byml::parse(&out).unwrap();
            let before: &[Node] = a.root.get("Actors").and_then(|n| n.as_arr()).unwrap();
            let after: &[Node] = b.root.get("Actors").and_then(|n| n.as_arr()).unwrap();
            assert_eq!(before.len(), after.len());
            let mut changed: usize = 0;
            for (x, y) in before.iter().zip(after.iter()) {
                if x != y {
                    changed += 1;
                    assert_eq!(x.get("Hash"), Some(&Node::U64(hash)));
                }
            }
            assert_eq!(changed, 1);
            let walls: usize = before
                .iter()
                .filter(|n| n.get("Gyaml").and_then(|g| g.as_str()) == Some(WALL_GYAML) && n.get("Presence").is_some())
                .count();
            assert_eq!(walls, 1);
        }
    }

    #[test]
    fn staged_edits_keep_the_water_gate_edit() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let work: std::path::PathBuf = std::env::temp_dir().join("ootrr-lake-tree-wall-test");
        let mut more: Vec<Out> = Vec::new();
        crate::water_gate::stage(&mut rf, &mut more, &work).unwrap();
        stage(&mut rf, &mut more, &work).unwrap();
        assert_eq!(more.iter().filter(|o| o.path == ADULT).count(), 1);
        let z: Vec<u8> = more.iter().find(|o| o.path == ADULT).unwrap().z.clone();
        let raw: Vec<u8> = rf.zs.decompress(&z).unwrap();
        assert_eq!(flag(&raw, ADULT_WALL).as_deref(), Some(crate::always::NAME));
        assert_eq!(flag(&raw, crate::water_gate::WALL).as_deref(), Some(crate::always::NAME));
        let z: Vec<u8> = more.iter().find(|o| o.path == CHILD).unwrap().z.clone();
        let raw: Vec<u8> = rf.zs.decompress(&z).unwrap();
        assert_eq!(flag(&raw, CHILD_WALL).as_deref(), Some(crate::always::NAME));
    }

    #[test]
    fn a_default_build_stages_the_tree_wall() {
        let found: bool = crate::slice::stages::STAGES.iter().any(|(n, _)| *n == "lake hylia tree wall");
        assert!(found);
    }
}
