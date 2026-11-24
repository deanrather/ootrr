use crate::error::{Error as E, Res};
use crate::logic_json::Json as Lj;
use crate::pot_slice::rocks::{apply_with, Spot};

pub const SETTING: &str = "shuffle_potion_shop";
pub const KEY: &str = "potion_shop_checks";
pub const ON: &str = "on";
pub const REGION: &str = "Market Potion Shop";
pub const BOTTLE: &str = "Market Potion Shop Free Bottle";
pub const POTION: &str = "Market Potion Shop Red Potion";
pub const LOCS: [&str; 2] = [BOTTLE, POTION];
pub const RULE: &str = "'Zelda Met' and has_bottle";
pub const SHELF: &str = "Market Potion Shop ";

pub fn shelf(name: &str) -> bool {
    name.strip_prefix(SHELF).is_some_and(|r| r.starts_with("Left Side ") || r.starts_with("Right Side "))
}
pub const NEED: &str = "has_bottle";

pub const NOTE: &str = "The Market Potion Shop keeper's free bottle and his Red Potion for the escaped bug are shuffled. A child who has met Zelda gets both by day. While the free bottle holds another item he only starts the bug tutorial for a child who already carries an empty bottle, and the shop shelves open after the tutorial as before, so the logic asks for a bottle for both checks and for the shelves.";

pub fn is(name: &str) -> bool {
    LOCS.contains(&name)
}

pub fn wanted(over: &[(&str, &str)]) -> bool {
    over.iter().any(|(k, v)| *k == KEY && *v == ON)
}

pub fn spots() -> Vec<Spot> {
    LOCS.iter().map(|n| Spot { name: n.to_string(), region: REGION.to_string(), rule: RULE.to_string() }).collect()
}

fn shelves(files: &mut [Lj]) -> Res<usize> {
    let mut n: usize = 0;
    for f in files.iter_mut() {
        let Lj::Arr(regions) = f else {
            continue;
        };
        for reg in regions.iter_mut() {
            let Lj::Obj(fields) = reg else {
                continue;
            };
            if !fields.iter().any(|(k, v)| k == "region_name" && v.as_str() == Some(REGION)) {
                continue;
            }
            let Some((_, Lj::Obj(list))) = fields.iter_mut().find(|(k, _)| k == "locations") else {
                return Err(E::Msg(format!("potion shop: {REGION} has no locations")));
            };
            for (name, rule) in list.iter_mut() {
                if !shelf(name) {
                    continue;
                }
                let Lj::Str(old) = rule else {
                    return Err(E::Msg(format!("potion shop: the rule of {name} is not text")));
                };
                *old = format!("({}) and {NEED}", old.trim());
                n += 1;
            }
        }
    }
    Ok(n)
}

pub fn apply(files: &mut [Lj]) -> Res<usize> {
    let n: usize = shelves(files)?;
    if n != 8 {
        return Err(E::Msg(format!("potion shop: found {n} shelves in {REGION} instead of 8")));
    }
    apply_with(files, &spots())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap as HM, BTreeSet as BS};
    use std::path::{Path as P, PathBuf as PB};

    use super::*;
    use crate::bfevfl::{Flow, Kind, Val};
    use crate::flow_extra::sim::{beyond_always, broken, called, edited, granted, run, sim, Scene};
    use crate::flow_locations::{Change, Data as FlowData, Loc};
    use crate::flow_sim::run::{End, Fin, Sim};
    use crate::logic_items::ItemId;
    use crate::romfs::Romfs;
    use crate::search::{explore, Search};
    use crate::settings::{Settings, Value};
    use crate::slice::data::Data;
    use crate::slice::fill::vanilla_place;
    use crate::slice::generate::{generate_data, GenOpts, Generated};
    use crate::slice::logic::{world, world_with};
    use crate::slice::support::plan as plan_of;
    use crate::world::{Placement, World};

    const NEW: &str = "Item_Longshot";
    const FLOW: &str = "Npc_Hylia_020_Shop_Child";
    const TALK: &str = "Item_Bottle_TutorialEvent_Talk";
    const BUG: &str = "Item_Bottle_TutorialEvent_Animal_Insect_GetEvent";
    const STARTED: &str = "Market_Item_Bottle_TutorialEvent_Started";
    const CLEARED: &str = "Market_Item_Bottle_TutorialEvent_Cleared";
    const EMPTY: &str = "EventQueryIsAnyBottleEmpty";
    const BOTTLE_SUB: &str = "Dm_SY_GetInEvent:GetInEventInEquipedBottle";
    const WAIT: &str = "RestrictedDoor_Market_12";

    fn root() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Event/EventFlow").is_dir() {
            Some(p)
        } else {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
            None
        }
    }

    fn scene(entry: &str, set: &[(&str, &str)]) -> Scene {
        Scene { flow: FLOW.to_string(), entry: entry.to_string(), set: set.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), own: Vec::new() }
    }

    fn sims(r: &P, loc: &str) -> (Sim, Sim) {
        let mut rf: Romfs = Romfs::open(r).unwrap();
        let fl: HM<String, Flow> = edited(&mut rf, loc, NEW).unwrap();
        assert_eq!(beyond_always(&fl), vec![FLOW.to_string()], "{loc}");
        (sim(r, HM::new()).unwrap(), sim(r, fl).unwrap())
    }

    fn answer(s: &mut Sim, empty: Option<bool>) {
        if let Some(b) = empty {
            s.opts.answers.insert(EMPTY.to_string(), u32::from(!b));
        }
    }

    fn talks(f: &Fin, id: &str) -> bool {
        crate::flow_sim::run::lines(&f.m.log).iter().any(|(_, t)| t.contains(id))
    }

    fn sets(f: &Fin, flag: &str) -> bool {
        f.m.writes.get(flag).map(|s| s.as_str()) == Some("true")
    }

    fn with(on: bool) -> Settings {
        let mut s: Settings = Settings::default();
        s.set("shuffle_rewards", Value::s("supported")).unwrap();
        s.set(SETTING, Value::Bool(on)).unwrap();
        s
    }

    #[test]
    fn the_free_bottle_offer_waits_for_an_empty_bottle_and_gives_the_item_once() {
        let Some(r) = root() else { return };
        let sc: Scene = scene(TALK, &[(STARTED, "false")]);
        let (mut a, mut b) = sims(&r, BOTTLE);
        let van: Vec<Fin> = run(&mut a, &sc).unwrap();
        assert!(van.iter().any(|f| granted(f, "Item_Bottle") == 1 && sets(f, STARTED)));
        assert!(van.iter().all(|f| granted(f, NEW) == 0));
        answer(&mut b, Some(false));
        let none: Vec<Fin> = run(&mut b, &sc).unwrap();
        assert!(!none.is_empty());
        for f in &none {
            assert_eq!(granted(f, NEW) + granted(f, "Item_Bottle"), 0);
            assert!(!sets(f, STARTED));
            assert!(talks(f, WAIT));
        }
        assert_eq!(broken(&none), 0);
        let (_, mut c) = sims(&r, BOTTLE);
        answer(&mut c, Some(true));
        let have: Vec<Fin> = run(&mut c, &sc).unwrap();
        assert!(have.iter().any(|f| granted(f, NEW) == 1 && sets(f, STARTED)));
        for f in &have {
            assert!(granted(f, NEW) <= 1 && granted(f, "Item_Bottle") == 0);
            assert_eq!(granted(f, NEW) == 1, sets(f, STARTED));
            assert!(!talks(f, WAIT));
        }
        assert_eq!(broken(&have), broken(&van));
        let (_, mut d) = sims(&r, BOTTLE);
        answer(&mut d, Some(true));
        let again: Vec<Fin> = run(&mut d, &scene(TALK, &[(STARTED, "true")])).unwrap();
        assert!(again.iter().all(|f| granted(f, NEW) == 0));
    }

    #[test]
    fn the_bug_reward_hands_over_the_item_once_without_pouring_into_the_bottle() {
        let Some(r) = root() else { return };
        let sc: Scene = scene(BUG, &[(STARTED, "true"), (CLEARED, "false")]);
        let (mut a, mut b) = sims(&r, POTION);
        let van: Vec<Fin> = run(&mut a, &sc).unwrap();
        assert!(van.iter().any(|f| called(f, BOTTLE_SUB) && sets(f, CLEARED)));
        let out: Vec<Fin> = run(&mut b, &sc).unwrap();
        assert!(out.iter().any(|f| granted(f, NEW) == 1 && sets(f, CLEARED)));
        for f in &out {
            assert!(!called(f, BOTTLE_SUB));
            assert!(granted(f, NEW) <= 1 && granted(f, "Item_Bottle_RedPotion") == 0);
            if f.end == End::Done {
                assert_eq!(granted(f, NEW) == 1, sets(f, CLEARED));
            }
        }
        assert_eq!(broken(&out), broken(&van));
    }

    #[test]
    fn both_rows_are_gated_flow_locations_with_their_own_edits() {
        let d: FlowData = FlowData::load().unwrap();
        let x: crate::flow_extra::Extra = crate::flow_extra::Extra::load().unwrap();
        for (n, ev, item) in [(BOTTLE, 162, "Item_Bottle"), (POTION, 138, "Item_Bottle_RedPotion")] {
            let l: &Loc = d.by_name(n).unwrap();
            assert!(l.gated && !l.supported, "{n}");
            assert_eq!(l.edits.len(), 1, "{n}");
            assert_eq!((l.edits[0].file.as_str(), l.edits[0].index), ("Npc_Hylia_020_Shop_Child.bfevfl", ev), "{n}");
            assert_eq!(l.item.as_deref(), Some(item), "{n}");
            let ch: Vec<Change> = crate::flow_locations::plan(&d, &[crate::flow_locations::Choice { location: n.to_string(), item: NEW.to_string() }], &Default::default()).unwrap().0;
            assert_eq!(ch.len(), 1, "{n}");
            assert!(x.by_name(n).is_some(), "{n}");
            assert!(!d.loose.iter().any(|o| o.file == l.edits[0].file && o.index == ev), "{n}");
        }
        let kinds: Vec<&str> = x.by_name(BOTTLE).unwrap().ops.iter().map(|o| o.kind()).collect();
        assert_eq!(kinds, vec!["query", "retarget"]);
        let kinds: Vec<&str> = x.by_name(POTION).unwrap().ops.iter().map(|o| o.kind()).collect();
        assert_eq!(kinds, vec!["swap_sub"]);
    }

    #[test]
    fn the_toggle_is_off_by_default_needs_supported_rewards_and_keeps_the_string() {
        assert!(!plan_of(&Settings::default()).unwrap().rewards.potion);
        assert!(!plan_of(&with(false)).unwrap().rewards.potion);
        let on = plan_of(&with(true)).unwrap();
        assert!(on.rewards.potion && on.logic().contains(&(KEY, ON)));
        assert!(!plan_of(&Settings::default()).unwrap().logic().contains(&(KEY, ON)));
        let mut s: Settings = Settings::default();
        s.set("shuffle_rewards", Value::s("off")).unwrap();
        s.set(SETTING, Value::Bool(true)).unwrap();
        assert!(!plan_of(&s).unwrap().rewards.potion);
        let mut d: Settings = Settings::default();
        d.set(SETTING, Value::Bool(false)).unwrap();
        assert_eq!(Settings::default().encode().unwrap(), d.encode().unwrap());
    }

    #[test]
    fn the_rows_are_flow_ok_closed_until_the_toggle_and_bind_no_item() {
        let d: Data = Data::load().unwrap();
        let off = plan_of(&Settings::default()).unwrap();
        let on = plan_of(&with(true)).unwrap();
        for n in LOCS {
            let l: &Loc = d.flow.by_name(n).unwrap();
            assert!(d.flow_ok(n), "{n}");
            assert!(!crate::flow_extra::open(l, &off) && crate::flow_extra::open(l, &on), "{n}");
            assert!(crate::slice::edit::is_sale(&d, n), "{n}");
        }
        assert_eq!(d.vanilla_of(BOTTLE), Some("Bottle"));
        assert_eq!(d.vanilla_of(POTION), Some("Rupees (5)"));
        assert!(!crate::flow_extra::bound(&d, "Bottle") && !crate::flow_extra::bound(&d, "Rupees (5)"));
    }

    fn seen(s: &Search, w: &World, n: &str) -> bool {
        s.loc_access(w.loc(n).unwrap())
    }

    fn without(w: &World, d: &Data, gone: &[&str]) -> Placement {
        let mut p: Placement = vanilla_place(w, d).unwrap();
        let ids: BS<ItemId> = gone.iter().filter_map(|g| w.item(g)).collect();
        let f: ItemId = w.item("Rupees (5)").unwrap();
        for l in &w.real {
            if p.get(*l).is_some_and(|i| ids.contains(&i)) {
                p.put(*l, f);
            }
        }
        p
    }

    #[test]
    fn the_logic_adds_both_checks_and_the_bottle_rule_only_when_on() {
        let d: Data = Data::load().unwrap();
        let off: World = world(&d).unwrap();
        for n in LOCS {
            assert!(off.loc(n).is_none(), "{n}");
        }
        let w: World = world_with(&d, &[(KEY, ON)]).unwrap();
        let bottles: Vec<&str> = ["Bottle", "Bottle with Milk", "Rutos Letter", "Bottle with Red Potion", "Bottle with Green Potion", "Bottle with Blue Potion", "Bottle with Fairy", "Bottle with Fish", "Bottle with Blue Fire", "Bottle with Bugs", "Bottle with Poe", "Bottle with Big Poe", "Blue Potion"].into_iter().filter(|b| w.item(b).is_some()).collect();
        let all: Search = explore(&w, &vanilla_place(&w, &d).unwrap(), &[]);
        let dry: Search = explore(&w, &without(&w, &d, &bottles), &[]);
        let plain: Search = explore(&off, &without(&off, &d, &bottles), &[]);
        let mut shown: usize = 0;
        let sides = ["Left Side", "Right Side"].iter().flat_map(|s| ["Top Left", "Top Right", "Bottom Left", "Bottom Right"].iter().map(move |p| format!("{SHELF}{s} {p}")));
        for n in LOCS.iter().map(|s| s.to_string()).chain(sides) {
            assert!(seen(&all, &w, &n), "{n}");
            assert!(!seen(&dry, &w, &n), "{n}");
            if shelf(&n) && seen(&plain, &off, &n) {
                shown += 1;
            }
        }
        assert_eq!(shown, 8);
        assert!(all.won());
    }

    #[test]
    fn apply_refuses_logic_files_without_the_eight_shelves() {
        let mut empty: Vec<Lj> = vec![Lj::Arr(Vec::new())];
        assert!(apply(&mut empty).is_err());
    }

    #[test]
    fn toggle_off_places_neither_check_and_on_adds_both_with_items_the_flows_hand_out() {
        let d: Data = Data::load().unwrap();
        let off: Generated = generate_data(&d, &with(false), "potion", &GenOpts::default()).unwrap();
        let base: Generated = generate_data(&d, &Settings::default(), "potion", &GenOpts::default()).unwrap();
        assert_eq!(off.seed_file.placements, base.seed_file.placements);
        for l in LOCS {
            assert!(!off.seed_file.placements.contains_key(l), "{l}");
        }
        // Every item may go to the potion shop keeper: the build turns both hand overs into the
        // plain GetInEvent_NotTurn give and a dungeon item there credits its dungeon
        // (crate::debug_names), so only crate::slice::generate::allowed judges it.
        let p: crate::slice::support::Plan = plan_of(&with(true)).unwrap();
        let w: World = world_with(&d, &p.logic()).unwrap();
        for n in 0..3 {
            let g: Generated = generate_data(&d, &with(true), &format!("potion {n}"), &GenOpts::default()).unwrap();
            assert_eq!(g.seed_file.placements.len(), off.seed_file.placements.len() + LOCS.len());
            for l in LOCS {
                let it: &str = &g.seed_file.placements[l];
                assert!(crate::slice::generate::allowed(&d, &w, &p, w.item(it).unwrap(), w.loc(l).unwrap()), "{l} holds {it}");
            }
            assert!(g.seed_file.placements.values().any(|i| i == "Bottle"));
        }
    }

    #[test]
    fn a_built_mod_carries_the_guard_and_the_plain_hand_over() {
        let Some(r) = root() else { return };
        let d: Data = Data::load().unwrap();
        let o: GenOpts = GenOpts { plando: vec![crate::slice::plando::parse("Market Potion Shop Free Bottle=Hover Boots").unwrap(), crate::slice::plando::parse("Market Potion Shop Red Potion=Piece of Heart").unwrap()], ..GenOpts::default() };
        let g: Generated = generate_data(&d, &with(true), "potion build", &o).unwrap();
        assert_eq!(g.seed_file.placements.get(BOTTLE).map(|s| s.as_str()), Some("Hover Boots"));
        let load: PB = PB::from(env!("CARGO_MANIFEST_DIR")).join("../../target/potion_mods");
        let _ = std::fs::remove_dir_all(&load);
        let b: crate::slice::edit::Built = crate::slice::edit::build_for(&g.seed_file, &r, &load, "ootrr-potion", crate::modpack::Host::Eden).unwrap();
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let p: String = crate::flow_locations::path_of("Npc_Hylia_020_Shop_Child.bfevfl");
        let out: crate::bfevfl::Bf = crate::bfevfl::Bf::parse(&rf.zs.decompress(&std::fs::read(b.root.join("romfs").join(&p)).unwrap()).unwrap()).unwrap();
        let f: &Flow = &out.flows[0];
        let Kind::Switch { actor, query, params, .. } = &f.events[16].kind else { panic!("switch") };
        assert_eq!(f.actors[*actor as usize].name, "PouchSystemActor");
        assert_eq!(f.actors[*actor as usize].queries[*query as usize], EMPTY);
        assert!(params.is_none());
        let Kind::Sub { flow, entry, params, .. } = &f.events[138].kind else { panic!("sub") };
        assert_eq!((flow.as_str(), entry.as_str()), ("Dm_SY_GetInEvent", "GetInEvent_NotTurn"));
        let given: Option<&Val> = params.as_ref().and_then(|c| c.get("ActorName"));
        assert!(given.is_some_and(|v| *v != Val::Str("Item_Bottle_RedPotion".to_string())), "{given:?}");
        assert!(params.as_ref().and_then(|c| c.get("BottleItem")).is_none());
        let Kind::Sub { params, .. } = &f.events[162].kind else { panic!("sub") };
        assert_ne!(params.as_ref().and_then(|c| c.get("ActorName")), Some(&Val::Str("Item_Bottle".to_string())));
        assert!(crate::flow_extra::analyzed::worked(&mut rf, &b.root.join("romfs")).unwrap().is_none(), "an edited potion shop event calls a flow its row does not load");
        let _ = std::fs::remove_dir_all(&load);
    }
}
