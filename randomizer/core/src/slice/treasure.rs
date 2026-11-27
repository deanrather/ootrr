use crate::byml::{Byml, Node};
use crate::bfevfl::{Bf, Flow};
use crate::error::{Error, Res};
use crate::placement::hash_of;
use crate::settings::Settings;
use crate::slice::banc::set_drop_ev;
use crate::slice::data::{Data, Ev};

pub const SETTING: &str = "shuffle_treasure_reward";

pub const LOC: &str = "Market Treasure Chest Game Reward";

pub const FIRST: u64 = 16881481538337649070;

pub const REPEAT: u64 = 6781536951709667179;

pub const VIEW: &str = "DropViewActor";

pub const VANILLA: &str = "Obj_Heart_Piece";

pub const NOTE: &str = "The Treasure Chest Game reward is shuffled. The first win gives the shuffled item once (its chest stays open) and every later win gives the vanilla red rupee. An upgrade there gives the tier fixed for it in the spoiler. Bombs, arrows or seeds there give rupees to a player without their bag, as in any chest.";

pub const REPEAT_OPEN: &str = "IsEnableRepeatOpen";

/// The prize event both reward chests start (GetEventName), its flow and its entry.
pub const EVENT: &str = "MiniGame_ChoiceTreasure_OpenClearTreasureBox";
pub const FLOW: &str = "MiniGame_SequentialChoiceTreasure";
pub const ENTRY: &str = "MiniGame_SequentialChoiceTreasure_OpenClearTreasureBox";

const VIEWS: [(&str, &str); 4] = [
    ("Obj_Heart_Piece", "Obj_Heart_Piece_MiniGame"),
    ("TBox_Rupee_Green", "Obj_OrnamentalRupee_Green_MiniGame"),
    ("TBox_Rupee_Blue", "Obj_OrnamentalRupee_Blue_MiniGame"),
    ("TBox_Rupee_Red", "Obj_OrnamentalRupee_Red_MiniGame"),
];

pub fn mode(s: &Settings, bad: &mut Vec<String>) -> bool {
    s.flag(SETTING).unwrap_or_else(|e| {
        bad.push(e.to_string());
        false
    })
}

pub fn opens(on: bool, name: &str) -> bool {
    on && name == LOC
}

pub fn with(mut d: Data, on: bool) -> Data {
    if on {
        d.events.keep.insert(FIRST);
    }
    d
}

pub fn view_of(actor: &str) -> String {
    match VIEWS.iter().find(|(a, _)| *a == actor) {
        Some((_, v)) => v.to_string(),
        None if actor.starts_with("Item_") => actor.to_string(),
        None => VIEWS[0].1.to_string(),
    }
}

pub fn set_view(y: &mut Byml, hash: u64, actor: &str) -> usize {
    if hash != FIRST {
        return 0;
    }
    let Some(list) = y.root.get_mut("Actors").and_then(|a| a.as_arr_mut()) else {
        return 0;
    };
    let mut n: usize = 0;
    for a in list.iter_mut() {
        if hash_of(a) != Some(hash) {
            continue;
        }
        let Some(d) = a.get_mut("Dynamic") else {
            continue;
        };
        if d.get(VIEW).is_none() {
            continue;
        }
        d.set(VIEW, Node::Str(view_of(actor)));
        d.set(REPEAT_OPEN, Node::Bool(false));
        n += 1;
    }
    n
}

pub fn test_edit(y: &mut Byml, actor: &str, view: bool) -> Res<usize> {
    let n: usize = set_drop_ev(y, FIRST, VANILLA, actor, Some(Ev::Keep))?;
    if view {
        set_view(y, FIRST, actor);
    }
    Ok(n)
}

/// The event the first reward chest starts when it holds ammo that needs a bag: a copy of the
/// prize event whose entry asks for the bag first (crate::progressive::guard_entry). The repeat
/// chest keeps the prize event, so its red rupee is never asked about.
pub fn guard_event(actor: &str) -> String {
    format!("OOTRR_TCG_AmmoGuard_{actor}")
}

fn guard_entry_name(actor: &str) -> String {
    format!("{ENTRY}_AmmoGuard_{actor}")
}

pub fn route_guard(y: &mut Byml, actor: &str) -> Res<usize> {
    let want: String = guard_event(actor);
    let Some(list) = y.root.get_mut("Actors").and_then(|a| a.as_arr_mut()) else {
        return Ok(0);
    };
    let mut n: usize = 0;
    for a in list.iter_mut() {
        if hash_of(a) != Some(FIRST) {
            continue;
        }
        let Some(d) = a.get_mut("Dynamic") else {
            continue;
        };
        let have: String = d.get("GetEventName").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if have != EVENT && have != want {
            return Err(Error::Msg(format!("the Treasure Chest Game reward chest starts {have:?} and not {EVENT}, the game files differ")));
        }
        d.set("GetEventName", Node::Str(want.clone()));
        n += 1;
    }
    Ok(n)
}

pub fn guard_flow(b: &mut Bf, actor: &str) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == FLOW).ok_or(Error::Format("treasure: the prize flow is missing"))?;
    crate::progressive::guard_entry(f, &guard_entry_name(actor), ENTRY, actor)
}

pub fn guard_rows(y: &mut Byml, actor: &str, analyzed: bool) -> Res<usize> {
    let entry: Option<String> = if analyzed { None } else { Some(guard_entry_name(actor)) };
    crate::progressive::add_rows(y, EVENT, &[(guard_event(actor), entry)])
}

pub fn stage_guard(rf: &mut crate::romfs::Romfs, late: &mut Vec<crate::staging::Out>, work: &std::path::Path, actor: &str) -> Res<()> {
    crate::story_skip::stage_late(rf, late, work, FLOW, |b| guard_flow(b, actor))?;
    for (table, analyzed) in [(crate::progressive::SETTING_TABLE, false), (crate::progressive::ANALYZED_TABLE, true)] {
        crate::story_skip::stage_late_file(rf, late, work, table, |raw: &[u8]| {
            let mut y: Byml = Byml::parse(raw)?;
            guard_rows(&mut y, actor, analyzed)?;
            y.write()
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap as BM;
    use std::path::PathBuf as PB;

    use super::*;
    use crate::json::{self, Json};
    use crate::romfs::Romfs;
    use crate::search::explore;
    use crate::settings::Value;
    use crate::settings::seed::SeedFile;
    use crate::slice::data::{State, EVENTS_JSON};
    use crate::slice::edit::{edit_file, Edited, Target};
    use crate::slice::fill::vanilla_place;
    use crate::slice::generate::{allowed, generate_data, GenOpts, Generated};
    use crate::slice::logic::world;
    use crate::slice::pick::pick;
    use crate::slice::support::{plan, Plan};
    use crate::world::{LocId, Placement, World};

    const FILE: &str = "Banc/Market/Market_Child_Static.bcett.byml.zs";

    fn on() -> Settings {
        let mut s: Settings = Settings::default();
        s.set(SETTING, Value::Bool(true)).unwrap();
        s
    }

    fn real() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(p)
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn dynamic(y: &Byml, hash: u64) -> Node {
        let list: &[Node] = y.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]);
        list.iter().find(|a| hash_of(a) == Some(hash)).and_then(|a| a.get("Dynamic")).cloned().unwrap()
    }

    fn text(n: &Node, k: &str) -> String {
        n.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
    }

    fn target(new: &str) -> Target {
        Target {
            name: LOC.to_string(),
            hash: FIRST,
            files: vec![FILE.to_string()],
            old_item: "Piece of Heart (Treasure Chest Game)".to_string(),
            new_item: "Megaton Hammer".to_string(),
            old_actor: "Obj_Heart_Piece".to_string(),
            new_actor: new.to_string(),
            ev: Some(Ev::Keep),
            family: None,
            key: None,
            heart_chest: false,
        }
    }

    #[test]
    fn the_event_data_matches_the_two_stacked_reward_chests() {
        let j: Json = json::parse(EVENTS_JSON).unwrap();
        let rows: &[Json] = j.get("chests").and_then(|v| v.as_arr()).unwrap();
        let row = |h: u64| rows.iter().find(|r| r.get("hash").and_then(|v| v.as_str()) == Some(h.to_string().as_str())).unwrap();
        let a: &Json = row(FIRST);
        let b: &Json = row(REPEAT);
        assert_eq!(a.get("location").and_then(|v| v.as_str()), Some(LOC));
        assert_eq!(a.get("drop").and_then(|v| v.as_str()), Some("Obj_Heart_Piece"));
        assert_eq!(b.get("drop").and_then(|v| v.as_str()), Some("TBox_Rupee_Red"));
        assert_eq!(a.get("translate"), b.get("translate"));
        for r in [a, b] {
            assert_eq!(r.get("event").and_then(|v| v.as_str()), Some("MiniGame_ChoiceTreasure_OpenClearTreasureBox"));
            assert_eq!(r.get("verdict").and_then(|v| v.as_str()), Some("keep_vanilla"));
        }
        assert_eq!(a.get("dynamic_extra").and_then(|v| v.get(VIEW)).and_then(|v| v.as_str()), Some("Obj_Heart_Piece_MiniGame"));
        let d: Data = Data::load().unwrap();
        assert_eq!(d.cat.by_name(LOC).map(|l| l.refs.iter().map(|r| r.hash).collect::<Vec<u64>>()), Some(vec![FIRST]));
        assert_eq!(d.state_of(LOC), Some(State::Event));
        assert!(!d.event_ok(LOC));
        assert!(with(Data::load().unwrap(), true).event_ok(LOC));
        assert!(!with(Data::load().unwrap(), false).event_ok(LOC));
    }

    #[test]
    fn the_preview_is_a_minigame_model_or_an_item_actor() {
        assert_eq!(view_of("Obj_Heart_Piece"), "Obj_Heart_Piece_MiniGame");
        assert_eq!(view_of("TBox_Rupee_Blue"), "Obj_OrnamentalRupee_Blue_MiniGame");
        assert_eq!(view_of("Item_Hammer"), "Item_Hammer");
        assert_eq!(view_of("Obj_Heart_Container"), "Obj_Heart_Piece_MiniGame");
        assert_eq!(view_of("TBox_ArrowBundle_Large"), "Obj_Heart_Piece_MiniGame");
    }

    #[test]
    fn the_logic_needs_the_lens_and_the_vanilla_world_reaches_the_reward() {
        let d: Data = Data::load().unwrap();
        let w: World = world(&d).unwrap();
        let l: LocId = w.loc(LOC).unwrap();
        let mut p: Placement = vanilla_place(&w, &d).unwrap();
        assert!(explore(&w, &p, &[]).loc_access(l));
        let lens: LocId = (0..w.locs.len()).find(|x| p.get(*x).is_some_and(|i| w.items.names[i] == "Lens of Truth")).unwrap();
        p.put(lens, w.item("Rupees (20)").unwrap());
        assert!(!explore(&w, &p, &[]).loc_access(l));
    }

    #[test]
    fn off_keeps_the_reward_vanilla_and_on_frees_only_the_first_win_chest() {
        let d: Data = Data::load().unwrap();
        let w: World = world(&d).unwrap();
        let off: Plan = plan(&Settings::default()).unwrap();
        assert!(!off.treasure);
        assert!(!pick(&d, &w, &off).free.contains(&w.loc(LOC).unwrap()));
        let p: Plan = plan(&on()).unwrap();
        assert!(p.treasure);
        assert!(p.notes.iter().any(|n| n == NOTE));
        let free: Vec<LocId> = pick(&d, &w, &p).free;
        assert!(free.contains(&w.loc(LOC).unwrap()));
        let market: Vec<&str> = free.iter().map(|l| w.locs[*l].name.as_str()).filter(|n| n.starts_with("Market Treasure Chest Game")).collect();
        assert_eq!(market, vec![LOC]);
        let mut s: Settings = on();
        s.set("shuffle_chests", Value::s("dungeons")).unwrap();
        assert!(!pick(&d, &w, &plan(&s).unwrap()).free.contains(&w.loc(LOC).unwrap()));
    }

    #[test]
    fn seeds_with_the_setting_put_any_item_on_the_reward() {
        let d: Data = Data::load().unwrap();
        let w: World = world(&d).unwrap();
        let s: Settings = on();
        let p: Plan = plan(&s).unwrap();
        let mut seen: BM<String, usize> = BM::new();
        for n in 0..24 {
            let g: Generated = generate_data(&d, &s, &format!("treasure {n}"), &GenOpts::default()).unwrap();
            let it: &str = &g.seed_file.placements[LOC];
            assert!(allowed(&d, &w, &p, w.item(it).unwrap(), w.loc(LOC).unwrap()));
            *seen.entry(it.to_string()).or_insert(0) += 1;
            assert!(g.report.contains(NOTE));
        }
        assert!(seen.len() > 1, "{seen:?}");
        let l: LocId = w.loc(LOC).unwrap();
        for any in ["Progressive Hookshot", "Bottle", "Rupees (200)", "Bomb Bag", "Deku Nuts (5)", "Gold Skulltula Token"] {
            assert!(allowed(&d, &w, &p, w.item(any).unwrap(), l), "{any}");
        }
        let off: Generated = generate_data(&d, &Settings::default(), "treasure off", &GenOpts::default()).unwrap();
        assert!(!off.seed_file.placements.contains_key(LOC));
    }

    #[test]
    fn an_upgrade_on_the_reward_drops_its_fixed_tier_and_keeps_the_clear_event() {
        const WATER: &str = "Water Temple Longshot Chest";
        let d: Data = Data::load().unwrap();
        let go = |x: &str| GenOpts { plando: vec![crate::slice::plando::parse(x).unwrap()], ..GenOpts::default() };
        let prize: &str = d.vanilla_of(LOC).unwrap();
        let swap = GenOpts { plando: vec![crate::slice::plando::parse(&format!("{LOC}=Progressive Hookshot")).unwrap(), crate::slice::plando::parse(&format!("{WATER}={prize}")).unwrap()], ..GenOpts::default() };
        let g: Generated = generate_data(&d, &on(), "p", &swap).unwrap();
        assert_eq!(g.seed_file.placements[LOC], "Progressive Hookshot");
        assert!(crate::slice::tier_order::fixed_spot(&d, LOC));
        let order: crate::slice::tier_order::Verdict = crate::slice::tier_order::fixed_actors(&d, &g.seed_file).unwrap();
        let tier: &String = order.assign.get(LOC).unwrap();
        assert!(["Item_Hookshot", "Item_Longshot"].contains(&tier.as_str()), "{tier}");
        let built: Data = with(Data::load().unwrap(), true);
        let mut only: crate::settings::seed::SeedFile = g.seed_file.clone();
        only.placements.retain(|l, _| l == LOC || l == WATER);
        let (ts, _, _) = crate::slice::chest_targets::targets_with(&built, &only, false, &order.assign, &crate::pickup_persist::Extra::default()).unwrap();
        let t: &Target = ts.iter().find(|t| t.name == LOC).unwrap_or_else(|| panic!("{:?}", ts.iter().map(|t| &t.name).collect::<Vec<_>>()));
        assert_eq!(&t.new_actor, tier);
        assert!(t.family.is_none(), "the chain would replace the clear event");
        let h: Generated = generate_data(&d, &on(), "p", &go(&format!("{LOC}=Megaton Hammer"))).unwrap();
        assert_eq!(h.seed_file.placements[LOC], "Megaton Hammer");
    }

    #[test]
    fn the_dump_edit_changes_only_the_first_win_drop_and_its_preview() {
        let Some(root) = real() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let raw: Vec<u8> = rf.read_zs(FILE).unwrap();
        let before: Byml = Byml::parse(&raw).unwrap();
        let first: Node = dynamic(&before, FIRST);
        let repeat: Node = dynamic(&before, REPEAT);
        assert_eq!(text(&first, "Drop__DropActor"), "Obj_Heart_Piece");
        assert_eq!(text(&first, VIEW), "Obj_Heart_Piece_MiniGame");
        assert_eq!(text(&repeat, "Drop__DropActor"), "TBox_Rupee_Red");
        assert_eq!(text(&repeat, VIEW), "Obj_OrnamentalRupee_Red_MiniGame");
        assert!(edit_file(FILE, &raw, &[Target { ev: None, ..target("Item_Hammer") }]).is_err());
        let e: Edited = edit_file(FILE, &raw, &[target("Item_Hammer")]).unwrap().unwrap();
        assert_eq!(e.hits.len(), 1);
        let after: Byml = Byml::parse(&e.bytes).unwrap();
        let a: Node = dynamic(&after, FIRST);
        assert_eq!(text(&a, "Drop__DropActor"), "Item_Hammer");
        assert_eq!(text(&a, VIEW), "Item_Hammer");
        assert_eq!(text(&a, "GetEventName"), "MiniGame_ChoiceTreasure_OpenClearTreasureBox");
        assert_eq!(first.get(REPEAT_OPEN), Some(&Node::Bool(true)));
        assert_eq!(a.get(REPEAT_OPEN), Some(&Node::Bool(false)));
        assert_eq!(dynamic(&after, REPEAT), repeat);
        let old: &[Node] = before.root.get("Actors").and_then(|x| x.as_arr()).unwrap();
        let new: &[Node] = after.root.get("Actors").and_then(|x| x.as_arr()).unwrap();
        assert_eq!(old.len(), new.len());
        let diff: Vec<Option<u64>> = old.iter().zip(new).filter(|(x, y)| x != y).map(|(x, _)| hash_of(x)).collect();
        assert_eq!(diff, vec![Some(FIRST)]);
        let mut keys: Vec<&str> = match (&first, &a) {
            (Node::Map(x), Node::Map(y)) => x.iter().zip(y).filter(|(p, q)| p != q).map(|(p, _)| p.0.as_str()).collect(),
            _ => Vec::new(),
        };
        keys.sort();
        assert_eq!(keys, vec![VIEW, "Drop__DropActor", REPEAT_OPEN]);
        let mut c: Byml = before.clone();
        c.root.set("Actors", Node::Arr(new.to_vec()));
        assert_eq!(c, after);
        let mut back: Byml = after.clone();
        assert_eq!(set_view(&mut back, REPEAT, "Item_Hammer"), 0);
        assert_eq!(set_view(&mut back, FIRST, "Obj_Heart_Piece"), 1);
        assert_eq!(text(&dynamic(&back, FIRST), VIEW), "Obj_Heart_Piece_MiniGame");
        let mut m: Byml = before.clone();
        assert_eq!(test_edit(&mut m, "Item_Hammer", true).unwrap(), 1);
        assert_eq!(m, after);
        let mut k: Byml = before.clone();
        assert_eq!(test_edit(&mut k, "TBox_Rupee_Gold", false).unwrap(), 1);
        assert_eq!(text(&dynamic(&k, FIRST), "Drop__DropActor"), "TBox_Rupee_Gold");
        assert_eq!(text(&dynamic(&k, FIRST), VIEW), "Obj_Heart_Piece_MiniGame");
        assert_eq!(dynamic(&k, REPEAT), repeat);
    }

    #[test]
    fn a_generated_seed_builds_with_the_hammer_in_the_first_win_chest() {
        let Some(root) = real() else { return };
        let d: Data = Data::load().unwrap();
        let o: GenOpts = GenOpts { plando: vec![crate::slice::plando::parse(&format!("{LOC}=Megaton Hammer")).unwrap()], ..GenOpts::default() };
        let g: Generated = generate_data(&d, &on(), "treasure build", &o).unwrap();
        let seed: SeedFile = g.seed_file;
        let out: PB = std::env::temp_dir().join(format!("ootrr_treasure_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let b: crate::slice::edit::Built = crate::slice::edit::build_for(&seed, &root, &out, "treasure", crate::modpack::Host::Eden).unwrap();
        assert!(b.chests.iter().any(|c| c.name == LOC && c.new_actor == "Item_Hammer"), "{}", b.report);
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let f: PB = b.root.join("romfs").join(FILE.replace('/', std::path::MAIN_SEPARATOR_STR));
        let z: Vec<u8> = std::fs::read(&f).unwrap();
        let y: Byml = Byml::parse(&rf.zs.decompress(&z).unwrap()).unwrap();
        let a: Node = dynamic(&y, FIRST);
        assert_eq!(text(&a, "Drop__DropActor"), "Item_Hammer");
        assert_eq!(text(&a, VIEW), "Item_Hammer");
        assert_eq!(text(&dynamic(&y, REPEAT), "Drop__DropActor"), "TBox_Rupee_Red");
        assert_eq!(text(&dynamic(&y, REPEAT), VIEW), "Obj_OrnamentalRupee_Red_MiniGame");
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn bombs_in_the_first_win_chest_ask_for_the_bag_and_the_repeat_chest_keeps_its_event() {
        use crate::bfevfl::{Kind, Val};
        let Some(root) = real() else { return };
        let d: Data = Data::load().unwrap();
        let o: GenOpts = GenOpts { plando: vec![crate::slice::plando::parse(&format!("{LOC}=Bombs (20)")).unwrap()], ..GenOpts::default() };
        let g: Generated = generate_data(&d, &on(), "treasure bombs", &o).unwrap();
        let out: PB = std::env::temp_dir().join(format!("ootrr_treasure_bombs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let b: crate::slice::edit::Built = crate::slice::edit::build_for(&g.seed_file, &root, &out, "treasure", crate::modpack::Host::Eden).unwrap();
        let bombs: &str = "Item_BombBundle_Small";
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let read = |rf: &mut Romfs, rel: &str| -> Vec<u8> {
            let z: Vec<u8> = std::fs::read(b.root.join("romfs").join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))).unwrap();
            rf.zs.decompress(&z).unwrap()
        };
        let y: Byml = Byml::parse(&read(&mut rf, FILE)).unwrap();
        assert_eq!(text(&dynamic(&y, FIRST), "Drop__DropActor"), bombs);
        assert_eq!(text(&dynamic(&y, FIRST), "GetEventName"), guard_event(bombs));
        assert_eq!(text(&dynamic(&y, REPEAT), "GetEventName"), EVENT);
        let bf: Bf = Bf::parse(&read(&mut rf, &format!("Event/EventFlow/{FLOW}.bfevfl.zs"))).unwrap();
        let f: &Flow = bf.flows.iter().find(|f| f.name == FLOW).unwrap();
        let e = f.entries.iter().find(|e| e.name == guard_entry_name(bombs)).unwrap();
        let mut at: usize = usize::from(e.main);
        let mut asked: Vec<String> = Vec::new();
        let none: usize = loop {
            let Kind::Switch { cases, params, .. } = &f.events[at].kind else { break at };
            if let Some(Val::Str(s)) = params.as_ref().and_then(|c| c.get("ActorName")) {
                asked.push(s.clone());
            }
            let yes: usize = cases.iter().find(|c| c.val == 0).unwrap().ev as usize;
            at = cases.iter().find(|c| c.val == 1).unwrap().ev as usize;
            assert!(matches!(f.events[yes].kind, Kind::Sub { .. }));
            if !matches!(f.events[at].kind, Kind::Switch { .. }) {
                let Kind::Sub { entry, params, .. } = &f.events[yes].kind else { unreachable!() };
                assert_eq!(entry, ENTRY);
                assert_eq!(params.as_ref().and_then(|c| c.get("ContentName")), Some(&Val::Arg("ContentName".to_string())));
            }
        };
        asked.sort();
        assert_eq!(asked, vec!["Item_BombBag_Large", "Item_BombBag_Medium", "Item_BombBag_Small"]);
        let Kind::Sub { entry, params, next, .. } = &f.events[none].kind else { panic!("the no-bag branch is not a call") };
        assert_eq!(entry, ENTRY);
        assert_eq!(params.as_ref().and_then(|c| c.get("ContentName")), Some(&Val::Str(crate::progressive::NO_BAG.to_string())));
        assert_eq!(f.events[usize::from(*next)].kind.params().and_then(|c| c.get("Value")), Some(&Val::Int(crate::progressive::NO_BAG_EXTRA)));
        let row = |rf: &mut Romfs, t: &str, id: &str| -> Node {
            let y: Byml = Byml::parse(&read(rf, t)).unwrap();
            y.root.as_arr().unwrap().iter().find(|r| text(r, "__RowId") == id).cloned().unwrap()
        };
        let ours: Node = row(&mut rf, crate::progressive::SETTING_TABLE, &guard_event(bombs));
        let theirs: Node = row(&mut rf, crate::progressive::SETTING_TABLE, EVENT);
        assert_eq!(text(&ours, "EntryPointName"), guard_entry_name(bombs));
        assert_eq!(ours.get("FlowchartFileName"), theirs.get("FlowchartFileName"));
        let ours: Node = row(&mut rf, crate::progressive::ANALYZED_TABLE, &guard_event(bombs));
        let theirs: Node = row(&mut rf, crate::progressive::ANALYZED_TABLE, EVENT);
        assert_eq!(ours.get("SubFlowList"), theirs.get("SubFlowList"));
        let _ = std::fs::remove_dir_all(&out);
    }
}
