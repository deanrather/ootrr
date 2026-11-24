
use std::collections::BTreeSet as BS;
use std::path::Path;

use crate::bfevfl::{Bf, Case, Cont, Event, Flow, Kind as Ev, Val};
use crate::dungeon_credit::{action_slot, add_int, flag, set_bool, DUNGEONS, HOLDER, SET};
use crate::dungeon_grant::struct_of;
use crate::dungeon_text::{compose, is_english, name as text_of, place_label, NAMES, PLACES};
use crate::error::{Error as E, Res};
use crate::hints::languages;
use crate::msbt::{self, Msbt, Tok};
use crate::romfs::Romfs;
use crate::sarc;
use crate::settings::seed::SeedFile;
use crate::slice::data::{Data, Item};
use crate::slice::key_spots::Pick;
use crate::staging::Out;

pub const GETS: [(&str, usize, &str); 2] = [("Dm_SY_Get", 7, "Event35"), ("Dm_SY_Get_WithoutPlayer", 0, "Event0")];
pub const PICKED: &str = "UI_MessageTargetActorName";
pub const ON_SALE: &str = "Shop_SelectItemName";
pub const SHOP_FLOW: &str = "Event/EventFlow/DisplayShop.bfevfl.zs";
pub const SHOP_GRANTS: usize = 3;
const SHOP_ACTOR: &str = "Npc_EventStarter";
const SHOP_GRANT: &str = "EventTriggerGetItemAtShop";
const ASK: &str = "EventQueryIsGameDataString64";
const ADD_INT: &str = "EventTriggerAddOrSubGameDataInt";
const PREFIX: &str = "OOTRRDungeonPack";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Small,
    Boss,
    Map,
    Compass,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Small, Kind::Boss, Kind::Map, Kind::Compass];

    pub fn of(kind: &str) -> Option<Kind> {
        match kind {
            "SmallKey" => Some(Kind::Small),
            "BossKey" | "GanonBossKey" => Some(Kind::Boss),
            "Map" => Some(Kind::Map),
            "Compass" => Some(Kind::Compass),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Kind::Small => "SmallKey",
            Kind::Boss => "BossKey",
            Kind::Map => "Map",
            Kind::Compass => "Compass",
        }
    }

    pub fn vanilla(self) -> &'static str {
        match self {
            Kind::Small => "Item_Dungeon_SmallKey",
            Kind::Boss => "Item_Dungeon_BossKey",
            Kind::Map => "Item_Dungeon_Map",
            Kind::Compass => "Item_Dungeon_Compass",
        }
    }

    pub fn template(self) -> &'static str {
        match self {
            Kind::Map => Kind::Compass.vanilla(),
            k => k.vanilla(),
        }
    }

    pub fn look(self) -> Option<&'static str> {
        (self == Kind::Map).then_some(Kind::Map.vanilla())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pack {
    pub kind: Kind,
    pub dungeon: &'static str,
}

impl Pack {
    pub fn name(&self) -> String {
        format!("Obj_{}_{}", self.kind.tag(), self.dungeon)
    }

    pub fn pick(&self) -> Pick {
        let mut args: Vec<String> = vec!["--name".to_string(), self.name(), "--own".to_string()];
        if let Some(m) = self.kind.look() {
            args.push("--model-from".to_string());
            args.push(m.to_string());
        }
        Pick { content: self.kind.template().to_string(), pack: self.name(), args }
    }
}

pub fn of(d: &Data, item: &str) -> Option<Pack> {
    of_item(d.item(item)?)
}

pub fn of_item(i: &Item) -> Option<Pack> {
    let kind: Kind = Kind::of(&i.kind)?;
    let dungeon: &'static str = i.dungeon.as_deref().and_then(struct_of)?;
    Some(Pack { kind, dungeon })
}

pub fn by_name(name: &str) -> Option<Pack> {
    let rest: &str = name.strip_prefix("Obj_")?;
    Kind::ALL.iter().find_map(|k| {
        let d: &str = rest.strip_prefix(k.tag())?.strip_prefix('_')?;
        DUNGEONS.iter().find(|x| **x == d).map(|x| Pack { kind: *k, dungeon: x })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    Ground,
    Holder,
    Shelf,
}

pub fn spot(d: &Data, loc: &str) -> Option<Spot> {
    if crate::shops::data::is_loc(loc) {
        Some(Spot::Shelf)
    } else if crate::slice::skulls::is_skull(d, loc) {
        None
    } else if d.pots.has(loc) {
        Some(Spot::Holder)
    } else if d.cat.by_name(loc).is_some_and(crate::slice::pickups::on_ground) {
        Some(Spot::Ground)
    } else {
        None
    }
}

pub fn placed(d: &Data, seed: &SeedFile) -> BS<Pack> {
    seed.placements
        .iter()
        .filter(|(loc, item)| d.vanilla_of(loc) != Some(item.as_str()) && spot(d, loc).is_some())
        .filter_map(|(_, item)| of(d, item))
        .collect()
}

pub fn picked_up(d: &Data, seed: &SeedFile) -> BS<Pack> {
    seed.placements
        .iter()
        .filter(|(loc, item)| d.vanilla_of(loc) != Some(item.as_str()) && matches!(spot(d, loc), Some(Spot::Ground | Spot::Holder)))
        .filter_map(|(_, item)| of(d, item))
        .collect()
}

pub fn handed(d: &Data, loc: &str, item: &str) -> bool {
    d.vanilla_of(loc) != Some(item) && spot(d, loc).is_some() && of(d, item).is_some()
}

fn idx(n: usize) -> Res<u16> {
    u16::try_from(n).map_err(|_| E::Format("dungeon packs: index does not fit in 16 bits"))
}

fn push(f: &mut Flow, kind: Ev) -> Res<u16> {
    let i: u16 = idx(f.events.len())?;
    f.events.push(Event { name: format!("{PREFIX}{i}"), kind });
    Ok(i)
}

pub fn credit(f: &mut Flow, p: Pack, end: u16) -> Res<u16> {
    let (a, set): (u16, u16) = action_slot(f, HOLDER, SET)?;
    let d: &str = p.dungeon;
    match p.kind {
        Kind::Small => {
            let (_, add): (u16, u16) = action_slot(f, HOLDER, ADD_INT)?;
            let once: u16 = push(f, Ev::Action { next: end, actor: a, act: set, params: Some(set_bool(flag(d, "IsGetSmallKeyOnce"))) })?;
            let total: u16 = push(f, Ev::Action { next: once, actor: a, act: add, params: Some(add_int(flag(d, "SmallKeyTotalNum"))) })?;
            push(f, Ev::Action { next: total, actor: a, act: add, params: Some(add_int(flag(d, "SmallKeyNum"))) })
        }
        k => {
            let g: crate::dungeon_grant::Kind = match k {
                Kind::Boss => crate::dungeon_grant::Kind::Boss,
                Kind::Map => crate::dungeon_grant::Kind::Map,
                _ => crate::dungeon_grant::Kind::Compass,
            };
            push(f, Ev::Action { next: end, actor: a, act: set, params: Some(set_bool(flag(d, g.member()))) })
        }
    }
}

fn ask(f: &mut Flow, data: &str, value: &str, hit: u16, miss: u16) -> Res<u16> {
    let (a, q): (u16, u16) = crate::progressive::ensure(f, HOLDER, ASK)?;
    let c: Cont = Cont { items: vec![("GameDataName".to_string(), Val::Str(data.to_string())), ("Index".to_string(), Val::Int(-1)), ("Value".to_string(), Val::Str(value.to_string()))] };
    push(f, Ev::Switch { actor: a, query: q, cases: vec![Case { val: 0, ev: hit }, Case { val: 1, ev: miss }], params: Some(c) })
}

pub fn chain(f: &mut Flow, data: &str, packs: &BS<Pack>, then: u16) -> Res<u16> {
    let mut miss: u16 = then;
    for p in packs.iter().rev() {
        let hit: u16 = credit(f, *p, then)?;
        miss = ask(f, data, &p.name(), hit, miss)?;
    }
    Ok(miss)
}

fn next_of(k: &Ev) -> Option<u16> {
    match k {
        Ev::Action { next, .. } | Ev::Sub { next, .. } => Some(*next),
        _ => None,
    }
}

fn set_next(k: &mut Ev, to: u16) {
    if let Ev::Action { next, .. } | Ev::Sub { next, .. } = k {
        *next = to;
    }
}

fn hooked(f: &Flow) -> bool {
    f.events.iter().any(|e| e.name.starts_with(PREFIX))
}

pub fn patch_get(f: &mut Flow, i: usize, name: &str, packs: &BS<Pack>) -> Res<()> {
    let then: u16 = match f.events.get(i) {
        Some(e) if e.name == name => next_of(&e.kind).ok_or_else(|| E::Msg(format!("dungeon packs: {name} of {} is not an action", f.name)))?,
        _ => return Err(E::Msg(format!("dungeon packs: event {i} of {} is not the pouch add {name}, the game files differ", f.name))),
    };
    if packs.is_empty() || hooked(f) {
        return Ok(());
    }
    let first: u16 = chain(f, PICKED, packs, then)?;
    set_next(&mut f.events[i].kind, first);
    Ok(())
}

fn is_shop_grant(f: &Flow, e: &Event) -> bool {
    match &e.kind {
        Ev::Action { actor, act, .. } => f.actors.get(usize::from(*actor)).is_some_and(|a| a.name == SHOP_ACTOR && a.actions.get(usize::from(*act)).is_some_and(|x| x == SHOP_GRANT)),
        _ => false,
    }
}

pub fn patch_shop(f: &mut Flow, packs: &BS<Pack>) -> Res<()> {
    let grants: Vec<usize> = (0..f.events.len()).filter(|&i| is_shop_grant(f, &f.events[i])).collect();
    if grants.len() != SHOP_GRANTS {
        return Err(E::Msg(format!("dungeon packs: the shop flow grants {} times and not {SHOP_GRANTS}, the game files differ", grants.len())));
    }
    if packs.is_empty() || hooked(f) {
        return Ok(());
    }
    for g in grants {
        let then: u16 = next_of(&f.events[g].kind).ok_or(E::Format("dungeon packs: a shop grant has no next"))?;
        let first: u16 = chain(f, ON_SALE, packs, then)?;
        set_next(&mut f.events[g].kind, first);
    }
    Ok(())
}

fn flow_mut<'a>(b: &'a mut Bf, name: &str) -> Res<&'a mut Flow> {
    b.flows.iter_mut().find(|f| f.name == name).ok_or_else(|| E::Msg(format!("dungeon packs: no {name} flow")))
}

pub fn shop_file(raw: &[u8], packs: &BS<Pack>) -> Res<Vec<u8>> {
    if packs.is_empty() {
        return Ok(raw.to_vec());
    }
    let mut b: Bf = Bf::parse(raw)?;
    let f: &mut Flow = b.flows.first_mut().ok_or(E::Format("dungeon packs: the shop flow file holds no flow"))?;
    patch_shop(f, packs)?;
    let out: Vec<u8> = b.write()?;
    if Bf::parse(&out)? != b {
        return Err(E::Format("dungeon packs: the written shop flow does not parse back to the edited model"));
    }
    Ok(out)
}

pub fn stage(rf: &mut Romfs, late: &mut Vec<Out>, work: &Path, packs: &BS<Pack>) -> Res<()> {
    if packs.is_empty() {
        return Ok(());
    }
    for (flow, i, name) in GETS {
        crate::story_skip::stage_late(rf, late, work, flow, |b| patch_get(flow_mut(b, flow)?, i, name, packs))?;
    }
    Ok(())
}

pub fn name_label(p: &Pack) -> String {
    format!("{}_Name", p.name())
}

fn lit(s: &str) -> Res<Vec<Tok>> {
    msbt::from_markup(s)
}

pub fn title(item: &[Tok], place: &[Tok]) -> Res<Vec<Tok>> {
    let mut out: Vec<Tok> = item.to_vec();
    out.extend(lit(" (")?);
    out.extend(place.iter().cloned());
    out.extend(lit(")")?);
    Ok(out)
}

fn msbt_of(es: &[sarc::Entry], name: &str) -> Res<Msbt> {
    let e: &sarc::Entry = sarc::get(es, name).ok_or_else(|| E::Msg(format!("the language archive has no {name}")))?;
    msbt::parse(&e.data)
}

pub fn text_file(raw: &[u8], packs: &BS<Pack>, english: bool) -> Res<Vec<u8>> {
    let mut es: Vec<sarc::Entry> = sarc::parse(raw)?;
    let places: Msbt = msbt_of(&es, PLACES)?;
    let mut m: Msbt = msbt_of(&es, NAMES)?;
    for p in packs {
        let v: &str = p.kind.vanilla();
        let item: Vec<Tok> = text_of(&m, &format!("{v}_Name"))?;
        let place: Vec<Tok> = text_of(&places, place_label(p.dungeon))?;
        let own: String = p.name();
        m.put_tokens(&format!("{own}_Name"), &title(&item, &place)?)?;
        m.put_tokens(&format!("{own}_GetMessage"), &compose(p.dungeon, &item, &place, english, false)?)?;
        let cap: Vec<u8> = m.raw(&format!("{v}_GetCaption")).map(|b| b.to_vec()).ok_or_else(|| E::Msg(format!("{v} has no get caption")))?;
        let label: String = format!("{own}_GetCaption");
        if m.raw(&label).is_none() {
            m.add_raw(&label, cap)?;
        }
    }
    sarc::set(&mut es, NAMES, m.write()?);
    sarc::write(&es)
}

pub fn stage_text(rf: &mut Romfs, more: &mut Vec<Out>, packs: &BS<Pack>) -> Res<()> {
    if packs.is_empty() {
        return Ok(());
    }
    for l in languages(rf)? {
        let english: bool = is_english(&l);
        crate::dungeon_grant::merge(rf, more, &l, |raw: &[u8]| text_file(raw, packs, english))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfevfl::flow::NONE;
    use crate::bfevfl::Actor;

    fn d() -> Data {
        Data::load().unwrap()
    }

    #[test]
    fn every_dungeon_item_has_a_pack_named_for_its_kind_and_dungeon() {
        let d: Data = d();
        let fire: Pack = of(&d, "Small Key (Fire Temple)").unwrap();
        assert_eq!(fire, Pack { kind: Kind::Small, dungeon: "FireTemple" });
        assert_eq!(fire.name(), "Obj_SmallKey_FireTemple");
        assert_eq!(of(&d, "Boss Key (Ganons Castle)").unwrap().name(), "Obj_BossKey_InsideGanonCastle");
        assert_eq!(of(&d, "Map (Deku Tree)").unwrap().name(), "Obj_Map_InsideTheDekuTree");
        assert_eq!(of(&d, "Compass (Ice Cavern)").unwrap().name(), "Obj_Compass_IceCavern");
        assert!(of(&d, "Progressive Hookshot").is_none());
        assert!(of(&d, "Small Key (Thieves Hideout)").is_none());
        let mut n: usize = 0;
        for i in &d.items {
            let Some(p) = of_item(i) else { continue };
            n += 1;
            assert_eq!(by_name(&p.name()), Some(p), "{}", i.name);
            assert!(crate::pickup_pack::is_actor_name(&p.name()), "{}", i.name);
        }
        assert!(n >= 30, "{n}");
        assert_eq!(by_name("Obj_SmallKey_Nowhere"), None);
        assert_eq!(by_name("Obj_Hookshot"), None);
    }

    #[test]
    fn the_pick_builds_its_own_identity_and_the_map_wears_the_map_model() {
        let p: Pick = Pack { kind: Kind::Map, dungeon: "FireTemple" }.pick();
        let (s, l) = p.spec().unwrap();
        assert_eq!(s.item, "Item_Dungeon_Compass");
        assert_eq!(s.name, "Obj_Map_FireTemple");
        assert!(l.own);
        assert_eq!(l.model.as_deref(), Some("Item_Dungeon_Map"));
        let (s, l) = Pack { kind: Kind::Small, dungeon: "WaterTemple" }.pick().spec().unwrap();
        assert_eq!((s.item.as_str(), s.name.as_str(), l.own, l.model), ("Item_Dungeon_SmallKey", "Obj_SmallKey_WaterTemple", true, None));
    }

    #[test]
    fn skulltula_drops_keep_the_sweep_and_the_rest_take_packs() {
        let d: Data = d();
        let skull: &str = d.cat.locs.iter().find(|l| l.kind == "gs").map(|l| l.name.as_str()).unwrap();
        assert_eq!(spot(&d, skull), None);
        assert_eq!(spot(&d, "KF Shop Right Side Top Left"), Some(Spot::Shelf));
        let heart: &str = d.cat.locs.iter().find(|l| l.kind == "bossheart").map(|l| l.name.as_str()).unwrap();
        assert_eq!(spot(&d, heart), Some(Spot::Ground));
    }

    fn actor(name: &str, acts: &[&str], queries: &[&str]) -> Actor {
        Actor { name: name.to_string(), sub: String::new(), arg: String::new(), actions: acts.iter().map(|s| s.to_string()).collect(), queries: queries.iter().map(|s| s.to_string()).collect(), params: None, ep: 0xFFFF, cut: 1 }
    }

    fn get_flow() -> Flow {
        Flow {
            name: "Dm_SY_Get".to_string(),
            actors: vec![actor("PouchContentGuestPerformer", &["EventTriggerAddOwnToPouch"], &[]), actor(HOLDER, &[], &["EventQueryIsGameDataBool"])],
            events: vec![
                Event { name: "Event35".to_string(), kind: Ev::Action { next: 1, actor: 0, act: 0, params: None } },
                Event { name: "Event36".to_string(), kind: Ev::Action { next: NONE, actor: 0, act: 0, params: None } },
            ],
            entries: Vec::new(),
        }
    }

    fn text(c: &Option<Cont>, k: &str) -> Option<String> {
        c.as_ref()?.items.iter().find(|(x, _)| x == k).and_then(|(_, v)| match v {
            Val::Str(s) => Some(s.clone()),
            _ => None,
        })
    }

    fn walk(f: &Flow, from: u16, picked: &str) -> (Vec<String>, u16) {
        let mut at: u16 = from;
        let mut out: Vec<String> = Vec::new();
        for _ in 0..64 {
            let e: &Event = &f.events[usize::from(at)];
            if !e.name.starts_with(PREFIX) {
                return (out, at);
            }
            match &e.kind {
                Ev::Switch { cases, params, .. } => {
                    let yes: bool = text(params, "Value").as_deref() == Some(picked);
                    at = cases.iter().find(|c| c.val == if yes { 0 } else { 1 }).unwrap().ev;
                }
                Ev::Action { next, params, .. } => {
                    out.push(text(params, "GameDataName").unwrap());
                    at = *next;
                }
                _ => panic!("unexpected event"),
            }
        }
        panic!("loop")
    }

    #[test]
    fn the_pickup_get_credits_only_the_pack_it_names_then_goes_on() {
        let mut f: Flow = get_flow();
        let packs: BS<Pack> = [Pack { kind: Kind::Small, dungeon: "FireTemple" }, Pack { kind: Kind::Boss, dungeon: "WaterTemple" }].into_iter().collect();
        patch_get(&mut f, 0, "Event35", &packs).unwrap();
        let Ev::Action { next, .. } = f.events[0].kind else { panic!() };
        let (w, end) = walk(&f, next, "Obj_SmallKey_FireTemple");
        assert_eq!(w, vec!["DungeonInfo.FireTemple.SmallKeyNum", "DungeonInfo.FireTemple.SmallKeyTotalNum", "DungeonInfo.FireTemple.IsGetSmallKeyOnce"]);
        assert_eq!(end, 1);
        assert_eq!(walk(&f, next, "Obj_BossKey_WaterTemple"), (vec!["DungeonInfo.WaterTemple.IsGetBossKey".to_string()], 1));
        assert_eq!(walk(&f, next, "Item_Hookshot"), (Vec::new(), 1));
        let n: usize = f.events.len();
        patch_get(&mut f, 0, "Event35", &packs).unwrap();
        assert_eq!(f.events.len(), n, "a second patch changes nothing");
        assert!(patch_get(&mut get_flow(), 1, "Event35", &packs).is_err());
    }

    #[test]
    fn the_title_wraps_the_place_in_brackets() {
        let t: Vec<Tok> = title(&lit("Small Key").unwrap(), &lit("Fire Temple").unwrap()).unwrap();
        assert_eq!(msbt::to_markup(&t), "Small Key (Fire Temple)");
    }

    fn real() -> Option<Romfs> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return None;
        }
        Romfs::open(&p).ok()
    }

    #[test]
    fn the_real_flows_take_both_credits() {
        let Some(mut rf) = real() else { return };
        let packs: BS<Pack> = [Pack { kind: Kind::Small, dungeon: "FireTemple" }, Pack { kind: Kind::Map, dungeon: "IceCavern" }].into_iter().collect();
        for (flow, i, name) in GETS {
            let raw: Vec<u8> = rf.read_zs(&format!("Event/EventFlow/{flow}.bfevfl.zs")).unwrap();
            let mut b: Bf = Bf::parse(&raw).unwrap();
            let f: &mut Flow = flow_mut(&mut b, flow).unwrap();
            let then: u16 = next_of(&f.events[i].kind).unwrap();
            patch_get(f, i, name, &packs).unwrap();
            let first: u16 = next_of(&f.events[i].kind).unwrap();
            assert_eq!(walk(f, first, "Obj_Map_IceCavern"), (vec!["DungeonInfo.IceCavern.IsGetMap".to_string()], then), "{flow}");
            let out: Vec<u8> = b.write().unwrap();
            assert_eq!(Bf::parse(&out).unwrap(), b);
        }
        let raw: Vec<u8> = rf.read_zs(SHOP_FLOW).unwrap();
        let shop: Vec<u8> = shop_file(&raw, &packs).unwrap();
        let b: Bf = Bf::parse(&shop).unwrap();
        let f: &Flow = &b.flows[0];
        let grants: Vec<usize> = (0..f.events.len()).filter(|&i| is_shop_grant(f, &f.events[i])).collect();
        assert_eq!(grants.len(), SHOP_GRANTS);
        let vanilla: Bf = Bf::parse(&raw).unwrap();
        for g in grants {
            let then: u16 = next_of(&vanilla.flows[0].events[g].kind).unwrap();
            let first: u16 = next_of(&f.events[g].kind).unwrap();
            let (w, end) = walk(f, first, "Obj_SmallKey_FireTemple");
            assert_eq!(end, then);
            assert_eq!(w.len(), 3);
        }
        assert_eq!(shop_file(&raw, &BS::new()).unwrap(), raw);
    }

    #[test]
    fn every_language_names_the_dungeon_of_the_pack() {
        let Some(mut rf) = real() else { return };
        let packs: BS<Pack> = Kind::ALL.iter().map(|k| Pack { kind: *k, dungeon: "FireTemple" }).chain([Pack { kind: Kind::Boss, dungeon: "InsideGanonCastle" }, Pack { kind: Kind::Small, dungeon: "GerudoTrainingGround" }, Pack { kind: Kind::Map, dungeon: "InsideJabuJabuBelly" }]).collect();
        for l in languages(&mut rf).unwrap() {
            let raw: Vec<u8> = rf.read_zs(&l).unwrap();
            let out: Vec<u8> = text_file(&raw, &packs, is_english(&l)).unwrap();
            let es: Vec<sarc::Entry> = sarc::parse(&out).unwrap();
            let m: Msbt = msbt_of(&es, NAMES).unwrap();
            for p in &packs {
                for sfx in ["_Name", "_GetMessage", "_GetCaption"] {
                    assert!(m.raw(&format!("{}{sfx}", p.name())).is_some(), "{l} {} {sfx}", p.name());
                }
            }
            if is_english(&l) && l.contains("USen") {
                assert_eq!(msbt::to_markup(&m.text("Obj_SmallKey_FireTemple_Name").unwrap()), "Small Key (Fire Temple)");
                assert_eq!(msbt::to_markup(&m.text("Obj_Map_FireTemple_Name").unwrap()), "Dungeon Map (Fire Temple)");
            }
        }
    }
}

