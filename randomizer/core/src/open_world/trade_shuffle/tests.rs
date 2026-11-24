use std::collections::HashMap as HM;
use std::path::PathBuf as PB;

use super::*;
use crate::flow_extra::sim::{granted, run, run_args, sim, Scene};
use crate::flow_sim::run::{lines, Fin, Sim};
use crate::flow_sim::state::V;
use crate::settings::{Settings, Value};
use crate::slice::generate::{generate_data, GenOpts, Generated};

fn root() -> Option<PB> {
    let p: PB = ootrr_game::test_romfs::path();
    if p.join("Event/EventFlow").is_dir() {
        Some(p)
    } else {
        eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
        None
    }
}

fn one(rf: &mut Romfs, out: &mut HM<String, Flow>, name: &str, edit: &dyn Fn(&mut Bf) -> Res<()>) {
    let raw: Vec<u8> = rf.read_zs(&format!("Event/EventFlow/{name}.bfevfl.zs")).unwrap();
    let mut b: Bf = Bf::parse(&raw).unwrap();
    edit(&mut b).unwrap();
    let mut back: Bf = Bf::parse(&b.write().unwrap()).unwrap();
    assert_eq!(back, b, "{name}");
    out.insert(name.to_string(), back.flows.remove(0));
}

/// What the seed build makes of the trade flows: every trade spot gives  (the location
/// edits with Opts::trades), then this module's edits.
const NEW: &str = "Item_Longshot";

fn patched(rf: &mut Romfs) -> HM<String, Flow> {
    let cs: Vec<crate::flow_locations::Choice> = crate::flow_extra::trade::LOCS.iter().map(|l| crate::flow_locations::Choice { location: l.to_string(), item: NEW.to_string() }).collect();
    let o: crate::flow_locations::Opts = crate::flow_locations::Opts { untested: false, songs: false, trades: true };
    let mut out: HM<String, Flow> = crate::flow_extra::sim::edited_with(rf, &cs, &o).unwrap();
    for n in NPCS {
        match out.get_mut(n) {
            Some(f) => {
                npc(f).unwrap();
            }
            None => one(rf, &mut out, n, &|b: &mut Bf| npc_bf(b, n).map(|_| ())),
        }
    }
    one(rf, &mut out, SHOW_FLOW, &show);
    one(rf, &mut out, GET_FLOW, &get);
    one(rf, &mut out, QUEST, &egg);
    out
}

fn step(s: &Sim, f: &Fin) -> String {
    let h: u32 = crate::flags::hash_name(STEP);
    match s.db.get(&f.m.st, h) {
        Some(V::E(e)) => s.db.enum_name(h, e),
        other => format!("{other:?}"),
    }
}

fn scene(flow: &str, entry: &str, set: &[(&str, &str)], own: &[&str]) -> Scene {
    Scene {
        flow: flow.to_string(),
        entry: entry.to_string(),
        set: set.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        own: own.iter().map(|a| (a.to_string(), 1)).collect(),
    }
}

/// The hand ins that take a trade item: flow, entry, the item shown.
const HANDS: [(&str, &str, &str); 9] = [
    ("Npc_Hylia_049_Adult", "SubEvent_GetSwordBigGoron_UntilStep03_ShowItemTalk", "Item_PocketCucco"),
    ("Npc_Hylia_017_Adult", "SubEvent_GetSwordBigGoron_Step03", "Item_CojiroCucco"),
    ("Npc_Hylia_053_Adult", "SubEvent_GetSwordBigGoron_Step04", "Item_OddMushroom"),
    ("Npc_Kokiri_Fado_Adult", "SubEvent_GetSwordBigGoron_Step05", "Item_OddPowder"),
    ("Npc_Hylia_012_Adult", "SubEvent_GetSwordBigGoron_Step06", "Item_PoacherSaw"),
    ("Npc_Goron_Biggoron_Adult", "SubEvent_GetSwordBigGoron_Step07", "Item_BrokenSword"),
    ("Npc_Zora_KingZora_adult", "SubEvent_GetSwordBigGoron_Step08", "Item_Prescription"),
    ("Npc_Hylia_045_Adult", "SubEvent_GetSwordBigGoron_Step09", "Item_EyeballFrog"),
    ("Dm_DeathMtTrail_0050", "C08", "Item_EyeDrop"),
];

#[test]
fn the_order_matches_the_logic_items_and_the_button_enum() {
    let d: Data = Data::load().unwrap();
    for (k, x) in ORDER.iter().enumerate() {
        assert_eq!(d.item(x.name).and_then(|i| i.actor.as_deref()), Some(x.actor), "{}", x.name);
        assert!(crate::flow_extra::trade::chain(x.actor), "{}", x.actor);
        // EventPlayerSetSelectedItem counts the button enum without Empty: the egg hatch puts the
        // Pocket Cucco (enum value 36) on the button with Item=35.
        assert_eq!(x.index, 34 + k as i32, "{}", x.name);
    }
    for t in crate::flow_extra::trade::LOCS {
        let v: &str = d.flow.by_name(t).map(|l| l.vanilla.as_str()).unwrap();
        assert!(by_name(v).is_some(), "{t} gives {v}");
    }
}

#[test]
fn the_pouch_menu_lists_the_adult_items_earliest_first() {
    let Some(r) = root() else { return };
    let rf: Romfs = Romfs::open(&r).unwrap();
    let raw: Vec<u8> = rf.read(POUCH_UI).unwrap();
    let out: Vec<u8> = pouch_order(&raw).unwrap();
    let (a, b): (Byml, Byml) = (Byml::parse(&raw).unwrap(), Byml::parse(&out).unwrap());
    let list = |y: &Byml| -> Vec<String> {
        let items: &[Node] = y.root.get("ItemArray").and_then(Node::as_arr).unwrap();
        let e: &Node = items.iter().find(|e| e.get("BtnName").and_then(Node::as_str) == Some(TRADE_BUTTON)).unwrap();
        e.get("Actor").and_then(Node::as_arr).unwrap().iter().map(|n| n.get("ActorShort").and_then(Node::as_str).unwrap().to_string()).collect()
    };
    let (old, new): (Vec<String>, Vec<String>) = (list(&a), list(&b));
    assert_eq!(old.len(), 14);
    assert_eq!(old[0], "Item_ClaimCheck");
    assert_eq!(new[..11], ORDER.iter().map(|x| x.actor.to_string()).collect::<Vec<String>>()[..]);
    assert_eq!(new[11..], old[11..]);
    let count = |y: &Byml| y.root.get("ItemArray").and_then(Node::as_arr).unwrap().len();
    assert_eq!(count(&a), count(&b));
    assert_eq!(pouch_order(&out).unwrap(), out);
}

#[test]
fn every_hand_in_moves_the_quest_to_the_earliest_item_left() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let mut s: Sim = sim(&r, fl).unwrap();
    for (flow, entry, shown) in HANDS {
        let later: (&str, &str) = if shown == "Item_CojiroCucco" { ("Item_OddMushroom", "Step04") } else { ("Item_CojiroCucco", "Step03") };
        let cases: [(&str, Vec<&str>, &str); 3] = [
            ("Step07", vec![shown, "Item_ClaimCheck"], "Step11"),
            ("Step10", vec![shown, later.0, "Item_EyeDrop"], later.1),
            ("Step02", vec![shown], "Step02"),
        ];
        for (start, own, want) in cases {
            let fins: Vec<Fin> = run(&mut s, &scene(flow, entry, &[(STEP, start)], &own)).unwrap();
            let took: Vec<&Fin> = fins.iter().filter(|f| granted(f, shown) == -1).collect();
            assert!(!took.is_empty(), "{flow} {entry} never takes {shown}");
            assert!(took.iter().any(|f| granted(f, NEW) == 1), "{flow} {entry} never gives the shuffled item");
            for f in took {
                assert!(ORDER.iter().all(|x| x.actor == shown || granted(f, x.actor) == 0), "{flow} {entry} still gives a trade item: {:?}", f.m.gr);
                // Holding nothing else, the quest stays put, or where the gift's own settle left it
                // while the shown item was still held.
                let own_step: &str = ORDER.iter().find(|x| x.actor == shown).and_then(|x| x.step).unwrap_or(start);
                let ok: bool = step(&s, f) == want || (own.len() == 1 && step(&s, f) == own_step);
                assert!(ok, "{flow} {entry} from {start} holding {own:?}: {} {:?}", step(&s, f), f.m.reqs);
            }
        }
    }
}

#[test]
fn anjus_first_gift_still_starts_the_quest_then_settles() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let mut s: Sim = sim(&r, fl).unwrap();
    let fins: Vec<Fin> = run(&mut s, &scene("Npc_Hylia_049_Adult", "SubEvent_GetSwordBigGoron_Start", &[(STEP, "Ready")], &["Item_OddPowder"])).unwrap();
    let gave: Vec<&Fin> = fins.iter().filter(|f| granted(f, NEW) == 1).collect();
    assert!(!gave.is_empty());
    for f in gave {
        assert!(f.m.reqs.iter().any(|q| q.contains("-> StartSubEvent: accepted")), "{:?}", f.m.reqs);
        assert_eq!(step(&s, f), "Step05");
    }
    // Before Anju's gift (quest at Ready) nothing settles, so her gift stays on offer.
    let fins: Vec<Fin> = run(&mut s, &scene("Npc_Hylia_017_Adult", "SubEvent_GetSwordBigGoron_Step03", &[(STEP, "Ready")], &["Item_CojiroCucco", "Item_ClaimCheck"])).unwrap();
    let took: Vec<&Fin> = fins.iter().filter(|f| granted(f, "Item_CojiroCucco") == -1).collect();
    assert!(!took.is_empty());
    assert!(took.iter().all(|f| step(&s, f) == "Ready"));
}

/// The `Item` each path put on the button, read from the verbose trace (the simulator does not
/// model the button).
fn picks(fins: &[Fin], fl: &Flow) -> Vec<Option<i32>> {
    fins.iter()
        .map(|f| {
            lines(&f.m.log).iter().find_map(|(_, t)| {
                if !t.contains("EventPlayerSetSelectedItem") {
                    return None;
                }
                let name: &str = t.split(' ').next()?;
                let e: &Event = fl.events.iter().find(|e| e.name == name)?;
                match e.kind.params()?.get("Item")? {
                    Val::Int(n) => Some(*n),
                    _ => None,
                }
            })
        })
        .collect()
}

#[test]
fn showing_a_trade_item_to_nobody_selects_the_next_held_one() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let show_flow: Flow = fl[SHOW_FLOW].clone();
    let mut s: Sim = sim(&r, fl).unwrap();
    s.opts.verbose = true;
    let child: &str = crate::flow_sim::ask::CHILD;
    let three: Vec<&str> = vec!["Item_CojiroCucco", "Item_Prescription", "Item_PocketEgg"];
    let cases: [(&str, Vec<&str>, Option<i32>, &str); 5] = [
        ("CojiroCucco", three.clone(), Some(41), "Step03"),
        ("Prescription", three.clone(), Some(34), "Step03"),
        ("PocketEgg", three, Some(36), "Step03"),
        ("ClaimCheck", vec!["Item_ClaimCheck"], None, "Step11"),
        ("Bomb", vec!["Item_ClaimCheck", "Item_EyeDrop"], None, "Step10"),
    ];
    for (button, own, want, quest) in cases {
        let sc: Scene = scene(SHOW_FLOW, SHOW_ENTRY, &[(child, "false"), (BUTTON, button), (STEP, "Step07")], &own);
        let fins: Vec<Fin> = run(&mut s, &sc).unwrap();
        assert!(!fins.is_empty());
        for (f, p) in fins.iter().zip(picks(&fins, &show_flow)) {
            assert_eq!(p, want, "button {button} holding {own:?}");
            assert_eq!(step(&s, f), quest, "button {button} holding {own:?}");
        }
    }
    let sc: Scene = scene(SHOW_FLOW, SHOW_ENTRY, &[(child, "true"), (BUTTON, "CojiroCucco"), (STEP, "Step07")], &["Item_CojiroCucco", "Item_Prescription"]);
    let fins: Vec<Fin> = run(&mut s, &sc).unwrap();
    assert!(!fins.is_empty());
    assert!(picks(&fins, &show_flow).iter().all(Option::is_none));
    assert!(fins.iter().all(|f| step(&s, f) == "Step07"));
}

#[test]
fn the_egg_hatches_only_from_a_held_egg_and_no_held_cucco() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let mut s: Sim = sim(&r, fl).unwrap();
    let cases: [(Vec<&str>, bool); 4] = [(vec![], false), (vec!["Item_PocketEgg"], true), (vec!["Item_PocketEgg", "Item_PocketCucco"], false), (vec!["Item_OddPowder"], false)];
    for (own, hatch) in cases {
        let fins: Vec<Fin> = run(&mut s, &scene(QUEST, EGG_ENTRY, &[(STEP, "WaitEggHatch")], &own)).unwrap();
        assert!(!fins.is_empty());
        for f in &fins {
            assert_eq!(granted(f, "Item_PocketCucco") == 1, hatch, "{own:?} {:?}", f.m.gr);
            assert_eq!(granted(f, "Item_PocketEgg") == -1, hatch, "{own:?}");
        }
        if own == vec!["Item_OddPowder"] {
            assert!(fins.iter().all(|f| step(&s, f) == "Step05"));
        }
    }
}

#[test]
fn an_item_get_settles_the_quest() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let mut s: Sim = sim(&r, fl).unwrap();
    let sc: Scene = scene(GET_FLOW, GET_FLOW, &[(STEP, "Step07")], &["Item_ClaimCheck"]);
    let fins: Vec<Fin> = run_args(&mut s, &sc, &[("ActorName", "Item_CojiroCucco")]).unwrap();
    let got: Vec<&Fin> = fins.iter().filter(|f| granted(f, "Item_CojiroCucco") == 1).collect();
    assert!(!got.is_empty(), "{:?}", fins.iter().map(|f| format!("{:?}", f.end)).collect::<Vec<_>>());
    assert!(got.iter().all(|f| step(&s, f) == "Step03"));
}

fn with(items: bool) -> Settings {
    let mut s: Settings = Settings::default();
    s.set("shuffle_rewards", Value::s("supported")).unwrap();
    s.set(crate::flow_extra::trade::SETTING, Value::Bool(true)).unwrap();
    s.set(SETTING, Value::Bool(items)).unwrap();
    s
}

#[test]
fn the_setting_is_off_by_default_and_needs_the_trade_checks() {
    use crate::slice::support::plan;
    assert!(!plan(&Settings::default()).unwrap().rewards.trade_items);
    assert!(!plan(&with(false)).unwrap().rewards.trade_items);
    assert!(plan(&with(true)).unwrap().rewards.trade_items);
    let mut s: Settings = with(true);
    s.set(crate::flow_extra::trade::SETTING, Value::Bool(false)).unwrap();
    assert!(!plan(&s).unwrap().rewards.trade_items);
    assert!(plan(&with(true)).unwrap().logic().contains(&LOGIC));
    assert!(!plan(&with(false)).unwrap().logic().contains(&LOGIC));
    let mut d: Settings = Settings::default();
    d.set(SETTING, Value::Bool(false)).unwrap();
    assert_eq!(Settings::default().encode().unwrap(), d.encode().unwrap());
}

#[test]
fn seeds_place_every_trade_item_once_and_the_spots_replace_their_gifts() {
    let d: Data = Data::load().unwrap();
    let mut moved: usize = 0;
    for n in 0..4 {
        let g: Generated = generate_data(&d, &with(true), &format!("trade shuffle {n}"), &GenOpts::default()).unwrap();
        for x in ORDER.iter() {
            let at: Vec<&String> = g.seed_file.placements.iter().filter(|(_, i)| i.as_str() == x.name).map(|(l, _)| l).collect();
            assert_eq!(at.len(), 1, "seed {n}: {} placed at {at:?}", x.name);
            if d.flow.by_name(at[0]).map(|l| l.vanilla.as_str()) != Some(x.name) {
                moved += 1;
            }
        }
        let cs: Vec<crate::flow_locations::Choice> = crate::flow_extra::trade::LOCS
            .iter()
            .filter_map(|l| {
                let item: String = d.item(g.seed_file.placements.get(*l)?).and_then(|i| i.actor.clone())?;
                Some(crate::flow_locations::Choice { location: l.to_string(), item })
            })
            .collect();
        let o: crate::flow_locations::Opts = crate::flow_locations::Opts { untested: true, songs: false, trades: true };
        let (ch, _, _) = crate::flow_locations::plan(&d.flow, &cs, &o).unwrap();
        assert!(ch.iter().all(|c| c.role != crate::flow_extra::trade::ROLE), "seed {n}");
    }
    assert!(moved > 20, "only {moved} trade items left their vanilla spot");
    let o: GenOpts = GenOpts { plando: vec![crate::slice::plando::parse("LW Trade Cojiro=Odd Mushroom").unwrap()], ..GenOpts::default() };
    assert!(generate_data(&d, &with(true), "trade shuffle plando", &o).is_ok());
    let o: GenOpts = GenOpts { plando: vec![crate::slice::plando::parse("LW Trade Odd Potion=Claim Check").unwrap()], ..GenOpts::default() };
    assert!(generate_data(&d, &with(true), "trade shuffle plando 2", &o).is_ok());
}

fn counted(flow: &str, entry: &str, set: &[(&str, &str)], own: &[(&str, i64)]) -> Scene {
    Scene {
        flow: flow.to_string(),
        entry: entry.to_string(),
        set: set.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        own: own.iter().map(|(a, n)| (a.to_string(), *n)).collect(),
    }
}

fn count(s: &mut Sim, f: &Fin, actor: &str) -> i64 {
    let h: u32 = s.db.reg(&count_name(actor));
    s.db.int(&f.m.st, h).unwrap_or(0)
}

const P: i64 = PARKED as i64;

#[test]
fn the_show_switch_brings_the_next_item_out_and_parks_the_shown_one() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let show_flow: Flow = fl[SHOW_FLOW].clone();
    let mut s: Sim = sim(&r, fl).unwrap();
    s.opts.verbose = true;
    let child: &str = crate::flow_sim::ask::CHILD;
    let own: [(&str, i64); 3] = [("Item_CojiroCucco", 1), ("Item_Prescription", P), ("Item_PocketEgg", P)];
    let sc: Scene = counted(SHOW_FLOW, SHOW_ENTRY, &[(child, "false"), (BUTTON, "CojiroCucco"), (STEP, "Step07")], &own);
    let fins: Vec<Fin> = run(&mut s, &sc).unwrap();
    assert!(!fins.is_empty());
    for (f, p) in fins.iter().zip(picks(&fins, &show_flow)) {
        assert_eq!(p, Some(41), "a parked item counts as held: the Prescription is next");
        assert_eq!((count(&mut s, f, "Item_CojiroCucco"), count(&mut s, f, "Item_Prescription"), count(&mut s, f, "Item_PocketEgg")), (P, 1, P));
        assert_eq!(step(&s, f), "Step03", "the parked Cojiro still sets the quest step");
    }
    // The pouch add comes before the button: the game may not select an item the pouch lacks.
    let order: Vec<&str> = show_flow.events.iter().filter(|e| e.name.starts_with("OOTRRTradeShuffle")).filter_map(|e| match &e.kind {
        Kind::Action { next, .. } if *next != crate::bfevfl::flow::NONE => match &show_flow.events[*next as usize].kind {
            Kind::Action { params: Some(c), .. } if c.get("Item").is_some() => e.kind.params().and_then(|c| c.get("Value")).map(|_| "out then button"),
            _ => None,
        },
        _ => None,
    }).collect();
    assert_eq!(order.len(), ORDER.len() * (ORDER.len() - 1));
    // Only one held: nothing changes.
    let sc: Scene = counted(SHOW_FLOW, SHOW_ENTRY, &[(child, "false"), (BUTTON, "ClaimCheck"), (STEP, "Step07")], &[("Item_ClaimCheck", 1)]);
    let fins: Vec<Fin> = run(&mut s, &sc).unwrap();
    assert!(fins.iter().all(|f| count(&mut s, f, "Item_ClaimCheck") == 1));
}

/// The `Item` each path last put on the button.
fn last_pick(f: &Fin, fl: &Flow) -> Option<i32> {
    lines(&f.m.log).iter().rev().find_map(|(_, t)| {
        if !t.contains("EventPlayerSetSelectedItem") {
            return None;
        }
        let name: &str = t.split(' ').next()?;
        let e: &Event = fl.events.iter().find(|e| e.name == name)?;
        match e.kind.params()?.get("Item")? {
            Val::Int(n) => Some(*n),
            _ => None,
        }
    })
}

#[test]
fn a_hand_in_puts_the_trade_item_before_it_on_the_button() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let flows: HM<String, Flow> = fl.clone();
    let mut s: Sim = sim(&r, fl).unwrap();
    s.opts.verbose = true;
    let at = |actor: &str| ORDER.iter().position(|x| x.actor == actor).unwrap();
    for (flow, entry, shown) in HANDS {
        let (early, late): (&str, &str) = if shown == "Item_OddMushroom" || shown == "Item_ClaimCheck" { ("Item_OddPowder", "Item_EyeDrop") } else { ("Item_OddMushroom", "Item_ClaimCheck") };
        // The nearest held one before the shown item, wrapping round.
        let n: usize = ORDER.len();
        let back = |a: &str| (at(shown) + n - at(a)) % n;
        let (near, far): (&str, &str) = if back(early) < back(late) { (early, late) } else { (late, early) };
        let fins: Vec<Fin> = run(&mut s, &counted(flow, entry, &[(STEP, "Step07")], &[(shown, 1), (late, P), (early, P)])).unwrap();
        let took: Vec<&Fin> = fins.iter().filter(|f| granted(f, shown) == -1).collect();
        assert!(!took.is_empty(), "{flow} {entry}");
        for f in took {
            assert_eq!((count(&mut s, f, shown), count(&mut s, f, near), count(&mut s, f, far)), (0, 1, P), "{flow} {entry}");
            assert_eq!(last_pick(f, &flows[flow]), Some(ORDER[at(near)].index), "{flow} {entry}: {near} goes on the button");
            let want: &str = ORDER.iter().find(|x| x.actor == early).and_then(|x| x.step).unwrap();
            assert_eq!(step(&s, f), want, "{flow} {entry}: the quest follows the earliest item left, parked or not");
        }
        // Another one is out already: it goes on the button and nothing parked comes out.
        let fins: Vec<Fin> = run(&mut s, &counted(flow, entry, &[(STEP, "Step07")], &[(shown, 1), (late, 1), (early, P)])).unwrap();
        for f in fins.iter().filter(|f| granted(f, shown) == -1) {
            assert_eq!((count(&mut s, f, early), count(&mut s, f, late)), (P, 1), "{flow} {entry}");
            assert_eq!(last_pick(f, &flows[flow]), Some(ORDER[at(late)].index), "{flow} {entry}");
        }
        // Nothing else held: the button is left alone.
        let fins: Vec<Fin> = run(&mut s, &counted(flow, entry, &[(STEP, "Step07")], &[(shown, 1)])).unwrap();
        for f in fins.iter().filter(|f| granted(f, shown) == -1) {
            assert_eq!(last_pick(f, &flows[flow]), None, "{flow} {entry}");
        }
    }
}

#[test]
fn a_new_trade_item_is_parked_while_the_button_s_own_is_out() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let mut s: Sim = sim(&r, fl).unwrap();
    // Button, the Claim Check's count before, then the new Cojiro's and the Claim Check's after.
    let cases: [(&str, i64, i64, i64); 4] = [("ClaimCheck", 1, P, 1), ("Bomb", 1, 1, 1), ("ClaimCheck", 0, 1, 0), ("CojiroCucco", 1, 1, P)];
    for (button, check, want, left) in cases {
        let sc: Scene = counted(GET_FLOW, GET_FLOW, &[(BUTTON, button), (STEP, "Step07")], &[("Item_ClaimCheck", check)]);
        let fins: Vec<Fin> = run_args(&mut s, &sc, &[("ActorName", "Item_CojiroCucco")]).unwrap();
        let got: Vec<&Fin> = fins.iter().filter(|f| granted(f, "Item_CojiroCucco") == 1).collect();
        assert!(!got.is_empty());
        for f in got {
            assert_eq!(count(&mut s, f, "Item_CojiroCucco"), want, "button {button}, Claim Check count {check}");
            assert_eq!(count(&mut s, f, "Item_ClaimCheck"), left, "button {button}: only the button's own trade item stays out");
            assert_eq!(step(&s, f), "Step03", "parked or not, the new item settles the quest");
        }
    }
}

#[test]
fn a_parked_egg_still_hatches() {
    let Some(r) = root() else { return };
    let mut rf: Romfs = Romfs::open(&r).unwrap();
    let fl: HM<String, Flow> = patched(&mut rf);
    let mut s: Sim = sim(&r, fl).unwrap();
    let fins: Vec<Fin> = run(&mut s, &counted(QUEST, EGG_ENTRY, &[(STEP, "WaitEggHatch")], &[("Item_PocketEgg", P), ("Item_PoacherSaw", 1)])).unwrap();
    assert!(!fins.is_empty());
    for f in &fins {
        assert_eq!((granted(f, "Item_PocketEgg"), granted(f, "Item_PocketCucco")), (-1, 1));
        assert_eq!((count(&mut s, f, "Item_PocketEgg"), count(&mut s, f, "Item_PocketCucco")), (0, 1), "the egg came out before it was taken");
    }
    // A parked Pocket Cucco blocks the hatch like one that is out.
    let fins: Vec<Fin> = run(&mut s, &counted(QUEST, EGG_ENTRY, &[(STEP, "WaitEggHatch")], &[("Item_PocketEgg", 1), ("Item_PocketCucco", P)])).unwrap();
    assert!(fins.iter().all(|f| granted(f, "Item_PocketCucco") == 0));
}
