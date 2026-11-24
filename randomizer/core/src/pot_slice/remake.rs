use std::collections::HashSet as HS;
use std::path::Path as P;

use crate::catalog::{Catalog, Loc};
use crate::error::{Error as E, Res};
use crate::json::{self, Json};
use crate::logic_json::Json as Lj;
use crate::pot_slice::data::Kind;
use crate::pot_slice::rocks::{apply_with, Spot};
use crate::pot_slice::scan::{finish, judge_all, PotScan};
use crate::romfs::Romfs;
use crate::slice::data::Data;

pub const CATALOG_JSON: &str = include_str!("../../../../data/pots_remake.json");
pub const SCAN_JSON: &str = include_str!("../../../../data/pots_remake_scan.json");
pub const KEY: &str = "remake_pot_spots";
pub const ON: &str = "on";
pub const MARK: &str = "remake";

pub fn locs() -> Res<Vec<Loc>> {
    let c: Catalog = Catalog::parse(CATALOG_JSON)?;
    for l in &c.locs {
        if !matches!(Kind::of(&l.kind), Some(Kind::Pot | Kind::Crate | Kind::Flying)) {
            return Err(E::Msg(format!("remake pot catalog: {} has the kind {}, which is not pot, crate or flying pot", l.name, l.kind)));
        }
        if l.refs.len() != 1 || l.refs.iter().any(|r| r.files.len() != 1) {
            return Err(E::Msg(format!("remake pot catalog: {} must own exactly one actor in one file", l.name)));
        }
        if !l.ev.iter().any(|e| e == MARK) {
            return Err(E::Msg(format!("remake pot catalog: {} is not marked {MARK}", l.name)));
        }
    }
    Ok(c.locs)
}

pub fn names() -> Res<HS<String>> {
    Ok(locs()?.into_iter().map(|l| l.name).collect())
}

fn text<'a>(j: &'a Json, k: &str, at: &str) -> Res<&'a str> {
    j.get(k).and_then(|v| v.as_str()).ok_or_else(|| E::Msg(format!("remake pot catalog: {at} has no {k}")))
}

pub fn spots_of(t: &str) -> Res<Vec<Spot>> {
    let j: Json = json::parse(t)?;
    let rows: &[Json] = j.get("locations").and_then(|v| v.as_arr()).ok_or(E::Format("remake pot catalog: no locations"))?;
    let mut out: Vec<Spot> = Vec::with_capacity(rows.len());
    for r in rows {
        let name: &str = text(r, "name", "a row")?;
        out.push(Spot { name: name.to_string(), region: text(r, "region", name)?.to_string(), rule: text(r, "rule", name)?.to_string() });
    }
    Ok(out)
}

pub fn spots() -> Res<Vec<Spot>> {
    spots_of(CATALOG_JSON)
}

pub fn wanted(over: &[(&str, &str)]) -> bool {
    over.iter().any(|(k, v)| *k == KEY && *v == ON)
}

pub fn apply(files: &mut [Lj]) -> Res<usize> {
    apply_with(files, &spots()?)
}

pub fn scan(root: &P) -> Res<PotScan> {
    let d: Data = Data::load()?;
    let ls: Vec<Loc> = locs()?;
    let mut rf: Romfs = Romfs::open(root)?;
    let rows: Vec<(String, crate::slice::data::State)> = judge_all(&d, &mut rf, &ls)?;
    finish(&rf, &ls, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap as BM;

    use crate::pot_slice::data::PotData;
    use crate::search::{explore, Search};
    use crate::settings::{Settings, Value};
    use crate::slice::data::State;
    use crate::slice::fill::vanilla_place;
    use crate::slice::generate::{allowed, generate_data, GenOpts, Generated};
    use crate::slice::logic::{world, world_with};
    use crate::slice::support::{plan, Mode, Plan};
    use crate::world::{LocId, Placement, World};

    fn with(pairs: &[(&str, &str)]) -> Settings {
        let mut s: Settings = Settings::default();
        for (k, v) in pairs {
            s.set(k, Value::s(v)).unwrap();
        }
        s
    }

    fn on_world(d: &Data) -> World {
        world_with(d, &[(KEY, ON)]).unwrap()
    }

    #[test]
    fn the_catalog_counts_are_pinned_and_no_hash_is_held_twice() {
        let d: Data = Data::load().unwrap();
        let ls: Vec<Loc> = locs().unwrap();
        assert_eq!(ls.len(), 213);
        assert!(ls.iter().all(|l| l.conf == crate::catalog::Conf::Forced));
        let mut by: BM<&str, usize> = BM::new();
        let mut kinds: BM<&str, usize> = BM::new();
        for l in &ls {
            *by.entry(l.area.as_str()).or_insert(0) += 1;
            *kinds.entry(l.kind.as_str()).or_insert(0) += 1;
        }
        assert_eq!((kinds["pot"], kinds["crate"], kinds["flying_pot"]), (173, 30, 10));
        assert_eq!(by.get("Gerudo Valley"), Some(&4));
        assert_eq!(by.get("Fire Temple"), Some(&10));
        assert_eq!(by.get("Shadow Temple"), Some(&18));
        assert_eq!(by.get("Kakariko Village"), Some(&31));
        assert_eq!(ls[..19].iter().filter(|l| l.kind == "pot").count(), 19);
        let mine: HS<u64> = ls.iter().map(|l| l.refs[0].hash).collect();
        assert_eq!(mine.len(), ls.len());
        for l in d.cat.locs.iter().chain(d.pots.locs.iter().filter(|l| !d.pots.is_remake(&l.name))) {
            for r in &l.refs {
                assert!(!mine.contains(&r.hash), "{} already holds hash {}", l.name, r.hash);
            }
        }
        for l in &ls {
            assert!(d.cat.by_name(&l.name).is_none(), "{}", l.name);
            assert!(crate::pot_slice::pickup::of(&l.vanilla).is_some_and(crate::pot_slice::pickup::refill), "{}", l.name);
        }
    }

    #[test]
    fn every_remake_pot_is_scanned_eligible() {
        let d: Data = Data::load().unwrap();
        let p: &PotData = &d.pots;
        for l in locs().unwrap() {
            assert!(p.is_remake(&l.name));
            assert_eq!(p.state_of(&l.name), Some(State::Eligible), "{}", l.name);
            assert_eq!(d.vanilla_of(&l.name), Some(l.vanilla.as_str()));
        }
        assert!(p.remake_game.starts_with("oot-remake:"));
        assert!(!p.remake_files.is_empty());
    }

    #[test]
    fn the_world_has_no_remake_pot_until_the_key_is_set() {
        let d: Data = Data::load().unwrap();
        let off: World = world(&d).unwrap();
        let on: World = on_world(&d);
        let sp: Vec<Spot> = spots().unwrap();
        assert!(sp.iter().all(|s| off.loc(&s.name).is_none()));
        assert_eq!(on.real.len(), off.real.len() + sp.len());
        for s in &sp {
            let l: LocId = on.loc(&s.name).unwrap_or_else(|| panic!("{}", s.name));
            assert_eq!(on.regions[on.locs[l].region].name, s.region, "{}", s.name);
        }
        assert!(!wanted(&plan(&Settings::default()).unwrap().logic()));
        assert!(wanted(&plan(&with(&[("shuffle_pots", "all")])).unwrap().logic()));
        assert!(wanted(&plan(&with(&[("shuffle_crates", "dungeons")])).unwrap().logic()));
    }

    #[test]
    fn the_vanilla_world_reaches_every_remake_pot_and_the_age_rules_bind() {
        let d: Data = Data::load().unwrap();
        let w: World = on_world(&d);
        let v: Placement = vanilla_place(&w, &d).unwrap();
        let s: Search = explore(&w, &v, &[]);
        assert!(s.won());
        for sp in spots().unwrap() {
            assert!(s.loc_access(w.loc(&sp.name).unwrap()), "{}", sp.name);
        }
        let sp: Vec<Spot> = spots().unwrap();
        assert!(sp.iter().filter(|s| s.name.contains(" Child ")).all(|s| s.rule.starts_with("is_child")));
        assert!(sp.iter().filter(|s| s.name.contains(" Adult ")).all(|s| s.rule.starts_with("is_adult")));
        assert!(sp.iter().filter(|s| s.name.contains(" Remake Crate ")).all(|s| s.rule.contains("can_break_crate")));
    }

    #[test]
    fn remake_spots_take_what_a_matched_spot_of_their_kind_takes() {
        let d: Data = Data::load().unwrap();
        let w: World = world_with(&d, &crate::spots::FULL).unwrap();
        let p: Plan = plan(&with(&[("shuffle_pots", "all"), ("shuffle_crates", "all")])).unwrap();
        let ok = |item: &str, loc: &str| allowed(&d, &w, &p, w.item(item).unwrap(), w.loc(loc).unwrap());
        let items: [&str; 8] = ["Rupees (20)", "Recovery Heart", "Progressive Hookshot", "Megaton Hammer", "Fire Arrows", "Gold Skulltula Token", "Heart Container", "Small Key (Fire Temple)"];
        // Dungeon items can stand in any pot of their own dungeon now, so the Fire Temple remake pot
        // is matched with a Fire Temple pot.
        for (mine, theirs) in [
            ("Fire Temple Near Boss Remake Pot 1", "Fire Temple Near Boss Pot 1"),
            ("GV Storms Grotto Adult Remake Pot 1", "Dodongos Cavern Right Side Pot 1"),
            ("Kakariko Village Adult Remake Pot 1", "Dodongos Cavern Right Side Pot 1"),
            ("Gerudo Fortress Adult Remake Crate 1", "GC Boulder Maze Crate"),
        ] {
            for i in items {
                assert_eq!(ok(i, mine), ok(i, theirs), "{i} at {mine}");
            }
        }
        assert!(ok("Megaton Hammer", "Fire Temple Near Boss Remake Pot 1"));
        assert!(ok("Small Key (Fire Temple)", "Fire Temple Near Boss Remake Pot 1"));
        assert!(!ok("Small Key (Fire Temple)", "Dodongos Cavern Right Side Pot 1"));
        assert!(ok("Rupees (20)", "Hideout Break Room Adult Remake Flying Pot 1"));
    }

    #[test]
    fn remake_spots_join_in_with_pots_or_crates() {
        let mut bad: Vec<String> = Vec::new();
        let d: crate::pot_slice::Drops = crate::pot_slice::plan::read(&with(&[("shuffle_pots", "dungeons")]), &mut bad);
        assert!(bad.is_empty());
        assert_eq!((d.remake, d.pots, d.crates), (Mode::All, Mode::Dungeons, Mode::Off));
        let c: crate::pot_slice::Drops = crate::pot_slice::plan::read(&with(&[("shuffle_crates", "overworld")]), &mut bad);
        assert_eq!(c.remake, Mode::All);
        assert_eq!(crate::pot_slice::plan::read(&Settings::default(), &mut bad).remake, Mode::Off);
    }

    fn beatable(d: &Data, s: &Settings, g: &Generated) -> bool {
        let p: Plan = plan(s).unwrap();
        let w: World = world_with(d, &p.logic()).unwrap();
        let mut pl: Placement = vanilla_place(&w, d).unwrap();
        for (loc, item) in &g.seed_file.placements {
            pl.put(w.loc(loc).unwrap(), w.item(item).unwrap());
        }
        explore(&w, &pl, &[]).won()
    }

    #[test]
    fn remake_pot_seeds_are_beatable_and_keep_to_the_rules() {
        let d: Data = Data::load().unwrap();
        let mut moved: usize = 0;
        let mut keys: usize = 0;
        for remake in ["all", "dungeons", "overworld"] {
            let pots: &str = remake;
            let s: Settings = with(&[("shuffle_pots", remake), ("shuffle_crates", remake)]);
            let p: Plan = plan(&s).unwrap();
            let w: World = world_with(&d, &p.logic()).unwrap();
            for n in 0..2 {
                let g: Generated = generate_data(&d, &s, &format!("remake pots {remake} {pots} {n}"), &GenOpts::default()).unwrap();
                assert!(beatable(&d, &s, &g), "{remake} {pots} {n}");
                let mut here: usize = 0;
                for (loc, item) in &g.seed_file.placements {
                    if !d.pots.is_remake(loc) {
                        continue;
                    }
                    here += 1;
                    assert!(crate::pot_slice::rules::at(&d, loc, item) == Some(true) || d.vanilla_of(loc) == Some(item.as_str()), "{loc} holds {item}");
                    if d.item(item).is_some_and(|i| i.adv) {
                        keys += 1;
                    }
                    let dungeon: bool = w.regions[w.locs[w.loc(loc).unwrap()].region].dungeon.is_some();
                    assert!(remake == "all" || (remake == "dungeons") == dungeon, "{loc} under {remake}");
                    if d.vanilla_of(loc) != Some(item.as_str()) {
                        moved += 1;
                    }
                }
                let want: usize = match remake {
                    "all" => 213,
                    "dungeons" => 114,
                    _ => 99,
                };
                assert_eq!(here, want, "{remake} {pots} {n}");
            }
        }
        assert!(moved > 50, "{moved}");
        assert!(keys > 0, "{keys}");
    }

    #[test]
    fn the_defaults_never_touch_a_remake_pot_and_each_kind_follows_its_own_setting() {
        let d: Data = Data::load().unwrap();
        let g: Generated = generate_data(&d, &Settings::default(), "no remake pots", &GenOpts::default()).unwrap();
        assert!(g.seed_file.placements.keys().all(|k| !d.pots.is_remake(k)));
        let g: Generated = generate_data(&d, &with(&[("shuffle_pots", "all")]), "pots only", &GenOpts::default()).unwrap();
        let remade: Vec<&String> = g.seed_file.placements.keys().filter(|k| d.pots.is_remake(k)).collect();
        assert!(!remade.is_empty());
        assert!(remade.iter().all(|k| d.pots.kind_of(k) != Some(crate::pot_slice::data::Kind::Crate)), "{remade:?}");
    }

    #[test]
    fn a_remake_pot_drops_the_plain_pickup_like_any_pot() {
        use crate::settings::seed::SeedFile;
        let d: Data = Data::load().unwrap();
        let one = |item: &str| {
            let m: BM<String, String> = [("Fire Temple Near Boss Remake Pot 1".to_string(), item.to_string())].into_iter().collect();
            crate::pot_slice::targets::targets(&d, &SeedFile::new("s", &Settings::default(), "g", m).unwrap())
        };
        let t = one("Rupees (50)").unwrap();
        assert_eq!(t.len(), 1);
        assert!(t[0].actor == "Obj_Rupee_Purple");
        let k = one("Megaton Hammer").unwrap();
        assert!(k.len() == 1 && k[0].actor == "Obj_Hammer", "{k:?}");
    }

    fn real() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(p)
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    #[test]
    fn the_shipped_scan_matches_the_dump() {
        let Some(root) = real() else {
            return;
        };
        let s: PotScan = scan(&root).unwrap();
        let (game, files, rows): (String, Vec<String>, Vec<(String, State)>) = crate::pot_slice::data::parse_scan(SCAN_JSON).unwrap();
        assert_eq!((s.game, s.files, s.rows), (game, files, rows));
    }
}
