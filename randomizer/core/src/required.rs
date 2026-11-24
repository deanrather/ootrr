use std::collections::BTreeSet as BS;
use std::path::Path as P;

use crate::ainb::start;
use crate::bomb_lift;
use crate::fast;
use crate::flags_build::LIST_PATH;
use crate::flow_locations::path_of;
use crate::hints;
use crate::intro_skip;
use crate::progressive::{ANALYZED_TABLE, BIG_FLOW, SETTING_TABLE, SMALL_FLOW};
use crate::rsizetable::TABLE_PATH;
use crate::slice::data::Data;
use crate::song_models;
use crate::songs;
use crate::special::SPOTS;

pub const LANGUAGE: &str = "Mals/USen.Product.110.sarc.zs";

pub fn list(d: &Data) -> Vec<String> {
    let mut out: BS<String> = BS::new();
    out.extend(d.scan.files.iter().cloned());
    out.extend(d.pots.files.iter().cloned());
    out.extend(d.pots.rock_files.iter().cloned());
    for l in &d.flow.locs {
        for e in &l.edits {
            if crate::cow_clone::of_file(&e.file).is_none() {
                out.insert(path_of(&e.file));
            }
            out.extend(e.also.iter().map(|a| path_of(a)));
        }
    }
    for c in crate::cow_clone::all() {
        out.extend(c.spots.iter().map(|s| s.file.clone()));
    }
    out.extend(crate::cow_clone::stage::sources());
    for s in [BIG_FLOW, SMALL_FLOW, SETTING_TABLE, ANALYZED_TABLE, TABLE_PATH, LIST_PATH, start::MODULE, songs::FLOW, hints::FLOW, LANGUAGE] {
        out.insert(s.to_string());
    }
    out.extend(fast::FILES.iter().map(|s| s.to_string()));
    out.insert(intro_skip::FILE.to_string());
    out.extend(crate::qol::files());
    out.extend(SPOTS.iter().map(|s| s.flow.to_string()));
    out.extend(song_models::SONGS.iter().map(|s| song_models::pack_of(s)));
    out.extend(bomb_lift::PACKS.iter().map(|s| bomb_lift::pack_of(s)));
    out.insert(crate::dungeon_credit::POUCH_TABLE.to_string());
    out.insert("Pack/ZsDic.pack.zs".to_string());
    out.into_iter().collect()
}

/// Folders a build reads whole: it lists them, or picks files in them by names that come
/// from the seed or the data files (actor packs, event flows, map and language files).
pub const TREES: [&str; 12] = [
    "Banc",
    "Event/EventFlow",
    "GameData",
    "Logic",
    "Mals",
    "Pack",
    "RSDB",
    "Sequence",
    "SLink2",
    "Sound/Bgm",
    "System/CombinationDataTableData",
    "System/Resource",
];

pub const DIFFICULTY_FILES: [&str; 2] = [crate::difficulty::lottery::FILE, crate::difficulty::elink::FILE];
pub const DIFFICULTY_TREES: [&str; 3] = [crate::difficulty::extras::MORE_SCENES, crate::difficulty::ground::NAV_DIR, crate::difficulty::ground::STITCHED_DIR];

/// Pickup actors take the pose file of their model when the game has one: `Model/Item_*_Animation.anim.bfres.zs`
/// (75 files, about 1 MB).
pub const POSE_HEAD: &str = "Model/Item_";
pub const POSE_TAIL: &str = "_Animation.anim.bfres.zs";

/// Music and sound effect shuffle list and size every sound bank and stream.
pub const SOUND: &str = "Sound/Resource";

/// The part of the game a build reads: the files in `files` and everything under the
/// folders in `trees`. Used to unpack only that part of a .nup or .nx (about 300 MB, or
/// about 2.3 GB with music or sound effect shuffle) instead of the whole 25 GB game.
pub struct Unpack {
    pub files: BS<String>,
    pub trees: BS<String>,
    pub shaped: Vec<(String, String)>,
}

impl Unpack {
    pub fn wants(&self, r: &str) -> bool {
        self.files.contains(r) || self.trees.iter().any(|t| r.len() > t.len() && r.starts_with(t.as_str()) && r.as_bytes()[t.len()] == b'/') || self.shaped.iter().any(|(a, z)| r.starts_with(a.as_str()) && r.ends_with(z.as_str()))
    }
}

pub fn unpack(d: &Data, sound: bool) -> Unpack {
    let mut files: BS<String> = list(d).into_iter().collect();
    files.extend(crate::colors::files());
    files.extend(crate::gerudo_card::MEMBERS.iter().map(|n| crate::gerudo_card::schedule_path(n)));
    files.insert(crate::gerudo_card::schedule_path(crate::open_world::rolling_goron::OWN));
    files.extend(crate::open_world::fortress::CARPENTERS.iter().map(|n| crate::open_world::fortress::own_schedule(n)));
    if let Ok(p) = crate::title_logo::patch() {
        files.extend(crate::title_logo::touched(&p));
    }
    files.extend(DIFFICULTY_FILES.iter().map(|s| s.to_string()));
    let mut trees: BS<String> = TREES.iter().chain(DIFFICULTY_TREES.iter()).map(|s| s.to_string()).collect();
    if sound {
        trees.insert(SOUND.to_string());
    }
    Unpack { files, trees, shaped: vec![(POSE_HEAD.to_string(), POSE_TAIL.to_string())] }
}

pub fn missing(root: &P, d: &Data) -> Vec<String> {
    list(d)
        .into_iter()
        .filter(|r| !r.split('/').fold(root.to_path_buf(), |a, s| a.join(s)).is_file())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Res;
    use crate::settings::{Kind, Settings, Value};
    use std::path::PathBuf as PB;

    #[test]
    fn the_list_names_the_files_a_default_build_reads() {
        let d: Data = Data::load().unwrap();
        let l: Vec<String> = list(&d);
        assert!(l.iter().any(|x| x == TABLE_PATH));
        assert!(l.iter().any(|x| x.starts_with("Banc/")));
        assert!(l.iter().any(|x| x.contains("Enemy_Scrub_Business_Common")));
        assert!(l.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn the_unpack_set_holds_the_build_list_the_colour_files_and_its_trees() {
        let d: Data = Data::load().unwrap();
        let u: Unpack = unpack(&d, false);
        assert!(list(&d).iter().all(|f| u.wants(f)));
        assert!(crate::colors::files().iter().all(|f| u.wants(f)));
        assert!(u.wants("Pack/Actor/Anything.pack.zs") && u.wants("Banc/X/y.zs"));
        assert!(!u.wants("Packed/x") && !u.wants("Pack") && !u.wants("Sound/Resource/Stream/SE_A.bwav"));
        assert!(unpack(&d, true).wants("Sound/Resource/Stream/SE_A.bwav"));
    }

    #[test]
    fn an_empty_folder_misses_everything() {
        let d: Data = Data::load().unwrap();
        let dir = std::env::temp_dir().join(format!("ootrr_required_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(missing(&dir, &d).len(), list(&d).len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_package_build_unpacks_the_files_the_difficulty_settings_read() {
        let d: Data = Data::load().unwrap();
        let u: Unpack = unpack(&d, false);
        for f in [crate::difficulty::lottery::FILE, crate::difficulty::elink::FILE, "AoC1/Banc/BancBasedResource/StartPos.byml.zs", "Phive/NavMesh/DodongoCavern.Nin_Ounce_NVN2.bphnm.zs", "Phive/StitchedNavMesh/HyruleField_Child/X0_Z0.Nin_Ounce_NVN2.bphnm.zs"] {
            assert!(u.wants(f), "{f}");
        }
        assert!(u.wants(&crate::gerudo_card::schedule_path(crate::open_world::rolling_goron::OWN)));
        assert!(u.wants("Model/Item_Ocarina_Fairy_Animation.anim.bfres.zs") && !u.wants("Model/Item_Ocarina_Fairy.fmdb.bfres.zs") && !u.wants("Model/Animal_Cow_Animation.anim.bfres.zs"));
        assert!(!u.wants("Phive/Shape/x") && !u.wants("ELink2/other.zs"));
    }

    const OURS: [&str; 4] = [crate::first_pickups::SETTING, crate::area_intros::SETTING, crate::qol::blocks::SETTING, crate::qol::owl::SETTING];

    fn hard() -> Settings {
        let mut s: Settings = Settings::default();
        for x in crate::settings::schema().settings.iter().filter(|x| x.group == "difficulty" || OURS.contains(&x.id.as_str())) {
            let v: Value = match x.kind {
                Kind::Toggle => Value::Bool(true),
                Kind::Number => Value::Int(x.max),
                Kind::Choice => Value::s(&x.choices[x.choices.len() - 1].id),
                Kind::Multi | Kind::List => continue,
            };
            s.set(&x.id, v).unwrap();
        }
        s
    }

    fn files(root: &P) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut stack: Vec<PB> = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p: PB = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push(p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        out.sort();
        out
    }

    fn at(root: &P, r: &str) -> PB {
        r.split('/').fold(root.to_path_buf(), |a, s| a.join(s))
    }

    fn part(root: &P, u: &Unpack, to: &P) -> usize {
        let mut n: usize = 0;
        for r in files(root).into_iter().filter(|r| u.wants(r)) {
            let (a, b): (PB, PB) = (at(root, &r), at(to, &r));
            std::fs::create_dir_all(b.parent().unwrap()).unwrap();
            if std::fs::hard_link(&a, &b).is_err() {
                std::fs::copy(&a, &b).unwrap();
            }
            n += 1;
        }
        n
    }

    #[test]
    fn every_difficulty_setting_builds_the_same_mod_from_the_unpack_set_alone() {
        let root: PB = ootrr_game::test_romfs::path();
        if !root.is_dir() {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", root.display());
            return;
        }
        let d: Data = Data::load().unwrap();
        let u: Unpack = unpack(&d, false);
        let tmp: PB = std::env::temp_dir().join(format!("ootrr_unpack_set_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let cut: PB = tmp.join("romfs");
        assert!(part(&root, &u, &cut) > 0);
        let s: Settings = hard();
        let o: crate::difficulty::Opts = crate::difficulty::read(&s, &mut Vec::new());
        assert!(o.regen && o.fairy_helps && o.mega_like && o.extra > crate::difficulty::horde::FROM);
        let g: crate::slice::generate::Generated = crate::slice::generate::generate_data(&d, &s, "unpack set", &crate::slice::generate::GenOpts::default()).unwrap();
        let full: PB = crate::slice::edit::build(&g.seed_file, &root, &tmp.join("full"), "unpack set").unwrap().root;
        let some: Res<crate::slice::edit::Built> = crate::slice::edit::build(&g.seed_file, &cut, &tmp.join("part"), "unpack set");
        let some: PB = some.unwrap_or_else(|e| panic!("a build from the unpack set alone failed: {e}")).root;
        let (a, b): (Vec<String>, Vec<String>) = (files(&full), files(&some));
        assert_eq!(a, b);
        let differ: Vec<&String> = a.iter().filter(|r| std::fs::read(at(&full, r)).unwrap() != std::fs::read(at(&some, r)).unwrap()).collect();
        assert!(differ.is_empty(), "differ from the folder build: {differ:?}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

