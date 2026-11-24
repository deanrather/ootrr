use std::collections::{BTreeMap as BM, BTreeSet as BS, HashMap as HM};

use crate::catalog::{Catalog, Conf, Seen};

pub const PIECE: [&str; 2] = ["Obj_Heart_Piece", "Obj_GerudoValley_Heart_Piece"];

pub const HEART: [&str; 2] = ["Obj_Heart", "Obj_Heart_InWater"];

pub const RUPEE: [&str; 4] = ["Obj_Rupee_Green", "Obj_Rupee_Blue", "Obj_Rupee_Red", "Obj_Rupee_Purple"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum St {
    Pinned,
    Pool,
    Gap,
    Unlisted,
}

pub fn class_of(s: &Seen) -> Option<&'static str> {
    let g: &str = s.gyaml.as_str();
    if PIECE.contains(&g) {
        return Some("heart piece");
    }
    if HEART.contains(&g) {
        return Some("recovery heart");
    }
    if RUPEE.contains(&g) {
        return Some("rupee");
    }
    if s.drop.as_deref().is_some_and(|d| PIECE.contains(&d)) {
        return Some("heart piece chest");
    }
    None
}

pub fn folder_of(file: &str) -> &str {
    file.split('/').nth(1).unwrap_or("")
}

fn states(c: &Catalog) -> HM<(&str, u64), St> {
    let mut m: HM<(&str, u64), St> = HM::new();
    for l in &c.locs {
        let st: St = if l.conf == Conf::Pool { St::Pool } else { St::Pinned };
        for r in &l.refs {
            m.insert((l.folder.as_str(), r.hash), st);
        }
    }
    for g in &c.gap_rem {
        for r in &g.refs {
            m.entry((g.folder.as_str(), r.hash)).or_insert(St::Gap);
        }
    }
    m
}

fn tally(c: &Catalog, seen: &[Seen]) -> BM<(String, &'static str), [BS<u64>; 4]> {
    let st: HM<(&str, u64), St> = states(c);
    let mut out: BM<(String, &'static str), [BS<u64>; 4]> = BM::new();
    for s in seen {
        let Some(k) = class_of(s) else {
            continue;
        };
        let f: &str = folder_of(&s.file);
        let v: St = st.get(&(f, s.hash)).copied().unwrap_or(St::Unlisted);
        let e: &mut [BS<u64>; 4] = out.entry((f.to_string(), k)).or_default();
        e[v as usize].insert(s.hash);
    }
    out
}

pub fn census(c: &Catalog, seen: &[Seen]) -> String {
    let mut s: String = String::from("scene / class: pinned, pooled, unmatched, unlisted (hashes)\n");
    let mut t: BM<&str, [usize; 4]> = BM::new();
    for ((f, k), v) in tally(c, seen) {
        s.push_str(&format!("{f} / {k}: {}, {}, {}, {}\n", v[0].len(), v[1].len(), v[2].len(), v[3].len()));
        let e: &mut [usize; 4] = t.entry(k).or_insert([0; 4]);
        for i in 0..4 {
            e[i] += v[i].len();
        }
    }
    for (k, v) in &t {
        s.push_str(&format!("total {k}: {}, {}, {}, {}\n", v[0], v[1], v[2], v[3]));
    }
    s
}

pub fn unlisted(c: &Catalog, seen: &[Seen]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for ((f, k), v) in tally(c, seen) {
        for h in &v[St::Unlisted as usize] {
            out.push(format!("{f}: {k} hash {h} is in neither the matched nor the unmatched list"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slice::data::Data;

    const FIX: &str = "{\"format\":1,\"note\":\"x\",\"locations\":[\
{\"name\":\"A Piece\",\"area\":\"Area A\",\"kind\":\"heart\",\"vanilla\":\"Piece of Heart\",\"item\":\"Obj_Heart_Piece\",\"conf\":\"forced\",\"folder\":\"FolderA\",\"via\":\"actor\",\"age\":\"both\",\"refs\":[{\"hash\":\"1\",\"files\":[\"Banc/FolderA/a.zs\"]}]},\
{\"name\":\"B Heart\",\"area\":\"Area A\",\"kind\":\"freestanding\",\"vanilla\":\"Recovery Heart\",\"item\":\"Obj_Heart\",\"conf\":\"pool\",\"folder\":\"FolderA\",\"via\":\"actor\",\"age\":\"both\",\"pool\":\"p\",\"refs\":[{\"hash\":\"2\",\"files\":[\"Banc/FolderA/a.zs\"]}]}\
],\"unmatched_oot\":[],\"unmatched_remake\":[{\"folder\":\"FolderA\",\"kind\":\"freestanding\",\"item\":\"Obj_Rupee_Blue\",\"via\":\"actor\",\"age\":\"both\",\"reason\":\"x\",\"refs\":[{\"hash\":\"3\",\"files\":[\"Banc/FolderA/a.zs\"]}],\"cands\":[]}]}";

    fn seen(f: &str, h: u64, g: &str, d: Option<&str>) -> Seen {
        Seen { file: f.to_string(), hash: h, gyaml: g.to_string(), drop: d.map(|s| s.to_string()) }
    }

    fn dump() -> Vec<Seen> {
        vec![
            seen("Banc/FolderA/a.zs", 1, "Obj_Heart_Piece", None),
            seen("Banc/FolderA/b.zs", 1, "Obj_Heart_Piece", None),
            seen("Banc/FolderA/a.zs", 2, "Obj_Heart", None),
            seen("Banc/FolderA/a.zs", 3, "Obj_Rupee_Blue", None),
            seen("Banc/FolderA/a.zs", 4, "Obj_TBox_A_02", Some("Obj_Heart_Piece")),
            seen("Banc/FolderA/a.zs", 5, "Obj_Pot_A_01", None),
        ]
    }

    #[test]
    fn classes_cover_actors_and_heart_piece_chests_only() {
        let d: Vec<Seen> = dump();
        let k: Vec<Option<&str>> = d.iter().map(class_of).collect();
        assert_eq!(k, vec![Some("heart piece"), Some("heart piece"), Some("recovery heart"), Some("rupee"), Some("heart piece chest"), None]);
        assert_eq!(folder_of("Banc/FolderA/a.zs"), "FolderA");
    }

    #[test]
    fn a_hash_in_two_files_counts_once_and_unlisted_ones_are_reported() {
        let c: Catalog = Catalog::parse(FIX).unwrap();
        let s: String = census(&c, &dump());
        assert!(s.contains("FolderA / heart piece: 1, 0, 0, 0"));
        assert!(s.contains("FolderA / recovery heart: 0, 1, 0, 0"));
        assert!(s.contains("FolderA / rupee: 0, 0, 1, 0"));
        assert!(s.contains("FolderA / heart piece chest: 0, 0, 0, 1"));
        let u: Vec<String> = unlisted(&c, &dump());
        assert_eq!(u.len(), 1);
        assert!(u[0].contains("hash 4"));
    }

    #[test]
    fn the_committed_catalog_lists_every_freestanding_age_by_its_files() {
        let d: Data = Data::load().unwrap();
        let mut n: usize = 0;
        for l in d.cat.locs.iter().filter(|l| matches!(l.kind.as_str(), "heart" | "freestanding") && !crate::spots::free::is(&d, &l.name)) {
            let fs: Vec<&str> = l.refs.iter().flat_map(|r| r.files.iter().map(|f| f.as_str())).collect();
            let a: bool = fs.iter().any(|f| f.contains("_Adult"));
            let c: bool = fs.iter().any(|f| f.contains("_Child"));
            let want: &str = match (a, c) {
                (true, true) => "both",
                (true, false) => "adult",
                _ => "child",
            };
            assert_eq!(l.age, want, "{}", l.name);
            n += 1;
        }
        assert_eq!(n, 14 + 18);
    }

    #[test]
    fn the_freestanding_catalog_counts_are_pinned() {
        let d: Data = Data::load().unwrap();
        let by = |k: &str, c: Conf| d.cat.locs.iter().filter(|l| l.kind == k && l.conf == c && !crate::spots::free::is(&d, &l.name)).count();
        assert_eq!(d.cat.locs.iter().filter(|l| crate::spots::free::is(&d, &l.name)).count(), 98);
        assert_eq!((by("heart", Conf::Item), by("heart", Conf::Forced), by("heart", Conf::Pool)), (7, 8, 0));
        assert_eq!((by("freestanding", Conf::Item), by("freestanding", Conf::Forced), by("freestanding", Conf::Pool)), (0, 2, 15));
        let gap: Vec<&crate::catalog::GapRem> = d.cat.gap_rem.iter().filter(|g| g.kind == "freestanding").collect();
        assert_eq!(gap.len(), 114);
        assert!(gap.iter().all(|g| g.item != "Obj_Heart_Piece" && g.item != "Obj_GerudoValley_Heart_Piece"));
        assert!(d.cat.gap_rem.iter().all(|g| g.kind != "heart" && !PIECE.contains(&g.item.as_str())));
        let pools: BS<&str> = d.cat.locs.iter().filter(|l| l.kind == "freestanding").filter_map(|l| l.pool.as_deref()).collect();
        assert_eq!(pools.len(), 3);
    }
}
