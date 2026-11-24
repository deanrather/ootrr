use crate::error::{Error as E, Res};
use crate::logic_json::Json;
use crate::shops::data::{table, Table};
use crate::shops::layout::{Layout, Place};
use crate::shops::price;
use crate::slice::data::{Data, Item};
use crate::slice::logic::{spec, world_from};
use crate::world::{Spec, World};

pub const BOMBS: &str = "'Defeat King Dodongo'";
pub const BOTTLE: &str = "has_bottle";

pub fn data_with(d: &Data, l: &Layout) -> Res<Data> {
    let t: &Table = table()?;
    let mut out: Data = d.clone();
    for n in &t.items {
        if out.item(&n.name).is_some() {
            continue;
        }
        out.item_ix.insert(n.name.clone(), out.items.len());
        out.items.push(Item {
            name: n.name.clone(),
            kind: "Shop".to_string(),
            adv: false,
            actor: Some(n.actor.clone()),
            conf: "exact".to_string(),
            dungeon: None,
            tiers: Vec::new(),
            world: Vec::new(),
        });
    }
    for p in &l.places {
        let v: &str = l.logic_item(&p.loc).unwrap_or("");
        if out.item(v).is_none() {
            return Err(E::Msg(format!("shops: {v} at {} is not a known item", p.loc)));
        }
        out.remake.insert(p.loc.clone(), v.to_string());
    }
    Ok(out)
}

pub fn extra(t: &Table, p: &Place) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    if let Some(w) = price::rule(p.price) {
        out.push(w);
    }
    if !p.deal && t.bottle.contains(&p.item) {
        out.push(BOTTLE);
    }
    if !p.deal && t.bombs.contains(&p.item) {
        out.push(BOMBS);
    }
    out
}

pub fn rule_of(old: &str, more: &[&str]) -> String {
    if more.is_empty() {
        return old.to_string();
    }
    format!("({}) and {}", old.trim(), more.join(" and "))
}

pub fn patch(s: &mut Spec, l: &Layout) -> Res<usize> {
    let t: &Table = table()?;
    let mut n: usize = 0;
    for f in s.regions.iter_mut() {
        let Json::Arr(regions) = f else {
            continue;
        };
        for r in regions.iter_mut() {
            let Json::Obj(fields) = r else {
                continue;
            };
            for (k, v) in fields.iter_mut() {
                if k != "locations" {
                    continue;
                }
                let Json::Obj(locs) = v else {
                    continue;
                };
                for (name, rule) in locs.iter_mut() {
                    let Some(p) = l.get(name) else {
                        continue;
                    };
                    let Json::Str(old) = rule else {
                        return Err(E::Msg(format!("shops: the rule of {name} is not text")));
                    };
                    *old = rule_of(old, &extra(t, p));
                    n += 1;
                }
            }
        }
    }
    if n != l.places.len() {
        return Err(E::Msg(format!("shops: {n} of {} shop locations found in the logic", l.places.len())));
    }
    Ok(n)
}

pub fn world(d: &Data, over: &[(&str, &str)], l: &Layout) -> Res<World> {
    let mut s: Spec = spec(d, over)?;
    patch(&mut s, l)?;
    world_from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic_items::ItemId;
    use crate::search::{explore, Search};
    use crate::shops::layout::{home, make};
    use crate::shops::plan::{Level, Opts, Prices};
    use crate::slice::fill::vanilla_place;
    use crate::slice::logic::world;
    use crate::world::Placement;

    fn opts(l: Level) -> Opts {
        Opts { level: l, prices: Prices::Weighted, min: 10, max: 300, kinds: crate::shops::plan::Kinds::default() }
    }

    #[test]
    fn rules_gain_the_wallet_and_the_buyer_needs() {
        let t: &Table = table().unwrap();
        let p = |item: &str, price: i32, deal: bool| Place { loc: "x".into(), item: item.into(), actor: String::new(), price, deal, pool: String::new() };
        assert!(extra(t, &p("Buy Heart", 5, false)).is_empty());
        assert_eq!(extra(t, &p("Buy Goron Tunic", 600, false)), vec!["(Progressive_Wallet, 2)"]);
        assert_eq!(extra(t, &p("Buy Fish", 50, false)), vec![BOTTLE]);
        assert_eq!(extra(t, &p("Buy Bombchu (20)", 240, false)), vec!["Progressive_Wallet", BOMBS]);
        assert_eq!(extra(t, &p("Buy Fish", 250, true)), vec!["Progressive_Wallet"]);
        assert_eq!(rule_of("True", &[]), "True");
        assert_eq!(rule_of("is_adult", &["has_bottle"]), "(is_adult) and has_bottle");
    }

    #[test]
    fn the_remake_shelves_as_vanilla_still_win_and_deals_hold_their_pool_item() {
        let d: Data = Data::load().unwrap();
        let plain: World = world(&d).unwrap();
        let l: Layout = home().unwrap();
        let sd: Data = data_with(&d, &l).unwrap();
        let w: World = super::world(&sd, &[], &l).unwrap();
        assert_eq!(w.locs.len(), plain.locs.len());
        let v: Placement = vanilla_place(&w, &sd).unwrap();
        assert!(explore(&w, &v, &[]).won());
        for s in &table().unwrap().slots {
            assert_eq!(sd.vanilla_of(&s.name), Some(s.item.as_str()), "{}", s.name);
        }
        assert!(table().unwrap().slots.iter().any(|s| s.name == "KF Shop Right Side Top Right" && s.item.contains("Deku")));
        assert_eq!(d.vanilla_of("KF Shop Right Side Top Right"), Some("Buy Arrows (30)"));
        let m: Layout = make(&opts(Level::Deals(2)), "logic", crate::shops::layout::Room::any(64)).unwrap();
        let md: Data = data_with(&d, &m).unwrap();
        let mw: World = super::world(&md, &[], &m).unwrap();
        let mv: Placement = vanilla_place(&mw, &md).unwrap();
        for p in &m.places {
            let i: ItemId = mv.get(mw.loc(&p.loc).unwrap()).unwrap();
            assert_eq!(mw.items.names[i], if p.deal { p.pool.as_str() } else { p.item.as_str() });
        }
    }

    #[test]
    fn a_six_hundred_rupee_tunic_needs_both_wallets() {
        let d: Data = Data::load().unwrap();
        let l: Layout = home().unwrap();
        let tunic: &str = "GC Shop Left Side Bottom Right";
        assert_eq!(l.get(tunic).map(|p| (p.item.as_str(), p.price)), Some(("Buy Goron Tunic", 600)));
        let sd: Data = data_with(&d, &l).unwrap();
        let w: World = super::world(&sd, &[], &l).unwrap();
        let wallet: ItemId = w.item("Progressive Wallet").unwrap();
        let mut v: Placement = vanilla_place(&w, &sd).unwrap();
        let junk: ItemId = w.item("Rupees (5)").unwrap();
        for x in &w.real {
            if v.get(*x) == Some(wallet) {
                v.put(*x, junk);
            }
        }
        let at = |extra: &[ItemId]| -> bool {
            let s: Search = explore(&w, &v, extra);
            s.loc_access(w.loc(tunic).unwrap())
        };
        assert!(!at(&[]));
        assert!(!at(&[wallet]));
        assert!(at(&[wallet, wallet]));
    }
}