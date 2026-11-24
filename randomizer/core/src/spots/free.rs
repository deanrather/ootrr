use crate::catalog::{Catalog, Loc, Via};
use crate::error::{Error as E, Res};
use crate::json::{self, Json};
use crate::pot_slice::rocks::Spot;
use crate::slice::data::Data;

pub const CATALOG_JSON: &str = include_str!("../../../../data/free_remake.json");
pub const KEY: &str = "free_remake_spots";
pub const ON: &str = "on";
pub const MARK: &str = "remake";

pub fn locs() -> Res<Vec<Loc>> {
    let c: Catalog = Catalog::parse(CATALOG_JSON)?;
    for l in &c.locs {
        if l.kind != "freestanding" || l.via != Via::Actor {
            return Err(E::Msg(format!("remake freestanding catalog: {} is not a freestanding actor", l.name)));
        }
        if l.refs.len() != 1 || l.refs.iter().any(|r| r.files.is_empty()) {
            return Err(E::Msg(format!("remake freestanding catalog: {} must own exactly one actor hash", l.name)));
        }
        if !l.ev.iter().any(|e| e == MARK) {
            return Err(E::Msg(format!("remake freestanding catalog: {} is not marked {MARK}", l.name)));
        }
    }
    Ok(c.locs)
}

fn text<'a>(j: &'a Json, k: &str, at: &str) -> Res<&'a str> {
    j.get(k).and_then(|v| v.as_str()).ok_or_else(|| E::Msg(format!("remake freestanding catalog: {at} has no {k}")))
}

pub fn spots() -> Res<Vec<Spot>> {
    let j: Json = json::parse(CATALOG_JSON)?;
    let rows: &[Json] = j.get("locations").and_then(|v| v.as_arr()).ok_or(E::Format("remake freestanding catalog: no locations"))?;
    let mut out: Vec<Spot> = Vec::with_capacity(rows.len());
    for r in rows {
        let name: &str = text(r, "name", "a row")?;
        out.push(Spot { name: name.to_string(), region: text(r, "region", name)?.to_string(), rule: text(r, "rule", name)?.to_string() });
    }
    Ok(out)
}

pub fn wanted(over: &[(&str, &str)]) -> bool {
    over.iter().any(|(k, v)| *k == KEY && *v == ON)
}

pub fn is(d: &Data, name: &str) -> bool {
    d.cat.by_name(name).is_some_and(|l| l.kind == "freestanding" && l.ev.iter().any(|e| e == MARK))
}

impl Data {
    pub fn with_free(mut self) -> Res<Data> {
        for l in locs()? {
            if self.cat.by_name(&l.name).is_some() || self.pots.has(&l.name) {
                return Err(E::Msg(format!("remake freestanding catalog: {} is already a location", l.name)));
            }
            self.remake.insert(l.name.clone(), l.vanilla.clone());
            self.cat.push(l);
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap as BM, HashSet as HS};

    use crate::search::{explore, Search};
    use crate::settings::{Settings, Value};
    use crate::slice::fill::vanilla_place;
    use crate::slice::logic::{world, world_with};
    use crate::slice::pickups;
    use crate::slice::support::plan;
    use crate::world::{LocId, Placement, World};

    fn with(v: &str) -> Settings {
        let mut s: Settings = Settings::default();
        s.set(pickups::SETTING, Value::s(v)).unwrap();
        s
    }

    #[test]
    fn every_row_owns_a_fresh_hash_and_a_ground_pickup() {
        let d: Data = Data::load().unwrap();
        let ls: Vec<Loc> = locs().unwrap();
        assert_eq!(ls.len(), 98);
        let mine: HS<u64> = ls.iter().map(|l| l.refs[0].hash).collect();
        assert_eq!(mine.len(), ls.len());
        for l in d.cat.locs.iter().filter(|l| !is(&d, &l.name)).chain(d.pots.locs.iter()) {
            for r in &l.refs {
                assert!(!mine.contains(&r.hash), "{} already holds hash {}", l.name, r.hash);
            }
        }
        for l in &ls {
            assert!(is(&d, &l.name), "{}", l.name);
            assert!(pickups::is_world(d.cat.by_name(&l.name).unwrap()), "{}", l.name);
            assert_eq!(d.vanilla_of(&l.name), Some(l.vanilla.as_str()), "{}", l.name);
            assert!(pickups::world_actor(&l.vanilla).is_some(), "{} {}", l.name, l.vanilla);
            assert_eq!(crate::slice::copies::copies(&d, &l.name), 1, "{}", l.name);
        }
    }

    #[test]
    fn the_world_has_no_remake_freestanding_spot_until_every_freestanding_item_is_shuffled() {
        let d: Data = Data::load().unwrap();
        let off: World = world(&d).unwrap();
        let on: World = world_with(&d, &[(KEY, ON)]).unwrap();
        let sp: Vec<Spot> = spots().unwrap();
        assert!(sp.iter().all(|s| off.loc(&s.name).is_none()));
        assert_eq!(on.real.len(), off.real.len() + sp.len());
        for s in &sp {
            let l: LocId = on.loc(&s.name).unwrap_or_else(|| panic!("{}", s.name));
            assert_eq!(on.regions[on.locs[l].region].name, s.region, "{}", s.name);
        }
        assert!(!wanted(&plan(&Settings::default()).unwrap().logic()));
        assert!(!wanted(&plan(&with("pieces")).unwrap().logic()));
        assert!(wanted(&plan(&with("all")).unwrap().logic()));
    }

    #[test]
    fn the_vanilla_world_reaches_every_remake_freestanding_spot() {
        let d: Data = Data::load().unwrap();
        let w: World = world_with(&d, &[(KEY, ON)]).unwrap();
        let v: Placement = vanilla_place(&w, &d).unwrap();
        let s: Search = explore(&w, &v, &[]);
        assert!(s.won());
        for sp in spots().unwrap() {
            assert!(s.loc_access(w.loc(&sp.name).unwrap()), "{}", sp.name);
            if sp.name.contains(" Adult ") {
                assert!(sp.rule.contains("is_adult"), "{}", sp.name);
            }
            if sp.name.contains(" Child ") {
                assert!(sp.rule.contains("is_child"), "{}", sp.name);
            }
        }
    }

    #[test]
    fn the_catalog_counts_are_pinned() {
        let mut n: BM<String, usize> = BM::new();
        for l in locs().unwrap() {
            *n.entry(l.vanilla.clone()).or_insert(0) += 1;
        }
        assert_eq!(n.values().sum::<usize>(), locs().unwrap().len());
        assert!(n.contains_key("Rupee (1)") && n.contains_key("Rupees (5)") && n.contains_key("Recovery Heart"));
    }
}
