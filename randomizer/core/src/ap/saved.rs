use std::collections::HashSet;

use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::murmur::murmur3_32;

pub const ROW_BYTES: i64 = 8;
pub const RESET_NEW_GAME: i32 = 8;
pub const SAVED: i32 = 0;
pub const MAX_NAME: usize = 64;

pub fn hash(name: &str) -> u32 {
    murmur3_32(name.as_bytes(), 0)
}

fn hash_of(r: &Node) -> Option<u64> {
    match r.get("Hash")? {
        Node::U32(h) => Some(*h as u64),
        Node::U64(h) => Some(*h),
        Node::I32(h) => Some(*h as u32 as u64),
        _ => None,
    }
}

fn taken(y: &Byml) -> HashSet<u64> {
    let mut out: HashSet<u64> = HashSet::new();
    if let Some(Node::Map(m)) = y.root.get("Data") {
        for (k, v) in m {
            if k == "Bool64bitKey" {
                continue;
            }
            for r in v.as_arr().unwrap_or(&[]) {
                if let Some(h) = hash_of(r) {
                    out.insert(h);
                }
            }
        }
    }
    out
}

fn grow(n: &mut Node, by: i64, what: &str) -> Res<()> {
    match n {
        Node::I32(v) => {
            *v = i32::try_from(*v as i64 + by).map_err(|_| Error::Msg(format!("saved flags: {what} would overflow")))?;
            Ok(())
        }
        Node::U32(v) => {
            *v = u32::try_from(*v as i64 + by).map_err(|_| Error::Msg(format!("saved flags: {what} would overflow")))?;
            Ok(())
        }
        _ => Err(Error::Msg(format!("saved flags: {what} is not a number"))),
    }
}

fn grow_meta(y: &mut Byml, by: i64) -> Res<()> {
    let m: &mut Node = y.root.get_mut("MetaData").ok_or(Error::Format("saved flags: the flag list has no MetaData"))?;
    for k in ["SaveDataSize", "SaveDataOffsetPos"] {
        let first: &mut Node = m
            .get_mut(k)
            .and_then(|a| a.as_arr_mut())
            .and_then(|a| a.first_mut())
            .ok_or_else(|| Error::Msg(format!("saved flags: MetaData has no {k}")))?;
        grow(first, by, k)?;
    }
    for k in ["AllDataSaveSize", "AllDataSaveOffset"] {
        let n: &mut Node = m.get_mut(k).ok_or_else(|| Error::Msg(format!("saved flags: MetaData has no {k}")))?;
        grow(n, by, k)?;
    }
    Ok(())
}

fn valid(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

pub fn row(name: &str) -> Node {
    Node::Map(vec![
        ("DefaultValue".to_string(), Node::Bool(false)),
        ("Hash".to_string(), Node::U32(hash(name))),
        ("ResetTypeValue".to_string(), Node::I32(RESET_NEW_GAME)),
        ("SaveFileIndex".to_string(), Node::I32(SAVED)),
    ])
}

pub fn add_bools(y: &mut Byml, names: &[String]) -> Res<usize> {
    let mut seen: HashSet<u64> = taken(y);
    for n in names {
        if !valid(n) {
            return Err(Error::Msg(format!("saved flags: {n:?} is not a usable flag name")));
        }
        if !seen.insert(hash(n) as u64) {
            return Err(Error::Msg(format!("saved flags: the hash of {n} is already used by another flag")));
        }
    }
    if names.is_empty() {
        return Ok(0);
    }
    let bools: &mut Vec<Node> = y
        .root
        .get_mut("Data")
        .and_then(|d| d.get_mut("Bool"))
        .and_then(|b| b.as_arr_mut())
        .ok_or(Error::Format("saved flags: the flag list has no Bool table"))?;
    bools.extend(names.iter().map(|n| row(n)));
    grow_meta(y, ROW_BYTES * names.len() as i64)?;
    Ok(names.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chest_slot::LIST;
    use crate::romfs::Romfs;

    fn meta(size: i32, pos: i32, all: i32, off: i32) -> Node {
        Node::Map(vec![
            ("AllDataSaveOffset".to_string(), Node::I32(off)),
            ("AllDataSaveSize".to_string(), Node::I32(all)),
            ("SaveDataOffsetPos".to_string(), Node::Arr(vec![Node::I32(pos), Node::I32(672)])),
            ("SaveDataSize".to_string(), Node::Arr(vec![Node::I32(size), Node::I32(44660)])),
        ])
    }

    fn sample() -> Byml {
        let bools: Vec<Node> = vec![row("Old.A"), row("Old.B")];
        let ints: Vec<Node> = vec![Node::Map(vec![("DefaultValue".to_string(), Node::I32(0)), ("Hash".to_string(), Node::U32(hash("Old.Int")))])];
        let data: Node = Node::Map(vec![("Bool".to_string(), Node::Arr(bools)), ("Int".to_string(), Node::Arr(ints))]);
        Byml { root: Node::Map(vec![("Data".to_string(), data), ("MetaData".to_string(), meta(1000, 100, 2000, 200))]), version: 2, big: false }
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn num(y: &Byml, k: &str) -> i32 {
        match y.root.get("MetaData").unwrap().get(k).unwrap() {
            Node::I32(v) => *v,
            Node::Arr(a) => match a[0] {
                Node::I32(v) => v,
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    #[test]
    fn new_flags_are_saved_rows_and_the_save_grows_by_eight_bytes_each() {
        let mut y: Byml = sample();
        assert_eq!(add_bools(&mut y, &names(&["RandoAP.Recv.1", "RandoAP.Got.1", "RandoSeed.x"])).unwrap(), 3);
        let b: &[Node] = y.root.get("Data").unwrap().get("Bool").unwrap().as_arr().unwrap();
        assert_eq!(b.len(), 5);
        assert_eq!(b[2], row("RandoAP.Recv.1"));
        assert_eq!(b[4].get("SaveFileIndex"), Some(&Node::I32(0)));
        assert_eq!(b[4].get("ResetTypeValue"), Some(&Node::I32(8)));
        assert_eq!((num(&y, "SaveDataSize"), num(&y, "SaveDataOffsetPos"), num(&y, "AllDataSaveSize"), num(&y, "AllDataSaveOffset")), (1024, 124, 2024, 224));
        let other: &[Node] = y.root.get("MetaData").unwrap().get("SaveDataSize").unwrap().as_arr().unwrap();
        assert_eq!(other[1], Node::I32(44660));
    }

    #[test]
    fn taken_hashes_duplicates_and_bad_names_are_refused_without_changes() {
        for bad in [names(&["Old.A"]), names(&["Old.Int"]), names(&["X", "X"]), names(&["has space"]), names(&[""])] {
            let mut y: Byml = sample();
            let before: Byml = y.clone();
            assert!(add_bools(&mut y, &bad).is_err(), "{bad:?}");
            assert_eq!(y.root, before.root);
        }
        let mut y: Byml = sample();
        assert_eq!(add_bools(&mut y, &[]).unwrap(), 0);
        assert_eq!(num(&y, "SaveDataSize"), 1000);
    }

    #[test]
    fn the_real_list_takes_new_flags_and_reads_back() {
        let p = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let mut y: Byml = rf.read_byml(LIST).unwrap();
        let size: i32 = num(&y, "SaveDataSize");
        let pos: i32 = num(&y, "SaveDataOffsetPos");
        let n: usize = y.root.get("Data").unwrap().get("Bool").unwrap().as_arr().unwrap().len();
        let new: Vec<String> = (0..300).map(|i| format!("RandoAP.Got.{i:012x}")).collect();
        assert_eq!(add_bools(&mut y, &new).unwrap(), 300);
        let back: Byml = Byml::parse(&y.write().unwrap()).unwrap();
        assert_eq!(back.root.get("Data").unwrap().get("Bool").unwrap().as_arr().unwrap().len(), n + 300);
        assert_eq!(num(&back, "SaveDataSize"), size + 2400);
        assert_eq!(num(&back, "SaveDataOffsetPos"), pos + 2400);
        let l = crate::flags::FlagList::from_byml(&back).unwrap();
        assert!(l.by_hash(hash("RandoAP.Got.00000000012a") as u64).is_some());
    }
}
