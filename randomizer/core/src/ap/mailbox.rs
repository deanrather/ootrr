use std::collections::BTreeSet as BS;

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Case, Cont, Entry, Event, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::dungeon_credit::{action_slot, set_bool};
use crate::error::{Error, Res};
use crate::flow_extra::analyzed::{Need, KIDS, SUBS};
use crate::slice::data::Data;

use super::markers::{HOLDER, SET};

pub const GIVE_FILE: &str = crate::difficulty::dupes::GIVE_FILE;
pub const GIVE_FLOW: &str = crate::difficulty::dupes::GIVE_FLOW;
pub const GIVE_ENTRY: &str = "GetInEvent_NotTurn_NotNpc";
pub const WINDOW_FLOW: &str = "GetPouchContent";
pub const WINDOW_ENTRY: &str = "DisplayGetByActorName";
pub const FLOWCHART: &str = "Work/Event/EventFlow/Dm_SY_GetInEvent.evfc";
pub const SETTING_TEMPLATE: &str = "CrumblingGanonCastle_Arrive";
pub const ANALYZED_TEMPLATE: &str = "NaviLockOnHint";
/// The longest actor name a record hands out. The game's own Dm_SY_GetInEvent passes
/// Item_FairyBless_DefenseDouble (29 characters) as ActorName at the Great Fairy of Courage, and
/// seeds give Item_DekuStickBundle_Small and Item_Melody_ShadowNocturne (26) through
/// GetInEvent_NotTurn_NotNpc, so names of that length are known to work. Actors the mod adds
/// keep the shorter limit of [`crate::ap::item::MAX_ACTOR`].
pub const MAX_ACTOR: usize = 29;
pub const PENDING: &str = "OOTRR.AP.Pending";
pub const DISPATCH: &str = "OOTRR_AP_Dispatch";
const ASK_BOOL: &str = "EventQueryIsGameDataBool";
const YES: u32 = 0;
const NO: u32 = 1;
pub const LOCAL_KINDS: [&str; 10] = ["SmallKey", "BossKey", "GanonBossKey", "Map", "Compass", "HideoutSmallKey", "SmallKeyRing", "HideoutSmallKeyRing", "TCGSmallKey", "TCGSmallKeyRing"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Style {
    Get(String),
    Tiers(String, String),
    Window(String),
    Dungeon(String, crate::gift_keys::Item, &'static str),
}

/// A dungeon item the mailbox can hand over: shown with the get window, then credited to its
/// dungeon the way a credited chest does (crate::gift_keys::credit).
pub fn dungeon_item(d: &Data, item: &str) -> Option<(String, crate::gift_keys::Item, &'static str)> {
    let i = d.item(item)?;
    let k: crate::gift_keys::Item = crate::gift_keys::Item::of(&i.kind)?;
    let dg: &'static str = i.dungeon.as_deref().and_then(crate::dungeon_grant::struct_of)?;
    let actor: String = i.actor.clone()?;
    actor_ok(&actor).ok()?;
    Some((actor, k, dg))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub key: String,
    pub item: String,
    pub style: Style,
}

fn key_ok(key: &str) -> Res<()> {
    if key.len() != 12 || !key.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
        return Err(Error::Msg(format!("mailbox: record key {key:?} is not 12 lowercase hex digits")));
    }
    Ok(())
}

fn actor_ok(actor: &str) -> Res<()> {
    if actor.is_empty() || actor.len() > MAX_ACTOR || !actor.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return Err(Error::Msg(format!("mailbox: {actor:?} is not a usable actor name (letters, digits and _, at most {MAX_ACTOR} characters: the longest the game's own item get passes)")));
    }
    Ok(())
}

impl Record {
    pub fn new(key: &str, actor: &str) -> Res<Record> {
        key_ok(key)?;
        actor_ok(actor)?;
        Ok(Record { key: key.to_string(), item: actor.to_string(), style: Style::Get(actor.to_string()) })
    }

    pub fn of(d: &Data, key: &str, item: &str) -> Res<Record> {
        key_ok(key)?;
        let i = d.item(item).ok_or_else(|| Error::Msg(format!("mailbox: {item} is not an OOTRR item")))?;
        if LOCAL_KINDS.contains(&i.kind.as_str()) {
            let (actor, k, dg) = dungeon_item(d, item).ok_or_else(|| Error::Msg(format!("mailbox: {item} is a dungeon item this build cannot hand over from another world")))?;
            return Ok(Record { key: key.to_string(), item: item.to_string(), style: Style::Dungeon(actor, k, dg) });
        }
        let style: Style = match d.family(item) {
            Some(fam) => {
                let first: &str = crate::progressive::tiers(fam)?.first().copied().ok_or_else(|| Error::Msg(format!("mailbox: {fam} has no tiers")))?;
                actor_ok(first)?;
                Style::Tiers(fam.to_string(), first.to_string())
            }
            None => {
                let actor: String = i.actor.clone().ok_or_else(|| Error::Msg(format!("mailbox: {item} has no actor the game can hand out")))?;
                actor_ok(&actor)?;
                if crate::ocarina_hold::no_anim(&actor) { Style::Window(actor) } else { Style::Get(actor) }
            }
        };
        Ok(Record { key: key.to_string(), item: item.to_string(), style })
    }

    pub fn windowed(&self) -> bool {
        match &self.style {
            Style::Window(_) | Style::Dungeon(..) => true,
            Style::Tiers(fam, _) => crate::progressive::tiers(fam).is_ok_and(|t| t.iter().any(|a| crate::ocarina_hold::no_anim(a))),
            Style::Get(_) => false,
        }
    }

    pub fn recv(&self) -> String {
        format!("OOTRR.AP.Recv.{}", self.key)
    }

    pub fn got(&self) -> String {
        format!("OOTRR.AP.Got.{}", self.key)
    }

    pub fn event(&self) -> String {
        format!("OOTRR_AP_{}", self.key)
    }
}

pub fn flags(recs: &[Record]) -> Vec<String> {
    let mut v: Vec<String> = recs.iter().flat_map(|r| [r.recv(), r.got()]).collect();
    if !recs.is_empty() {
        v.push(PENDING.to_string());
    }
    v
}

fn idx(n: usize) -> Res<u16> {
    u16::try_from(n).ok().filter(|&i| i != NONE).ok_or(Error::Format("mailbox: the give flow is full"))
}

fn push(f: &mut Flow, name: String, kind: Kind) -> Res<u16> {
    let i: u16 = idx(f.events.len())?;
    f.events.push(Event { name, kind });
    Ok(i)
}

fn ask(f: &mut Flow, flag: &str, yes: u16, no: u16) -> Res<Kind> {
    let (a, q): (u16, u16) = crate::start_state::query_slot(f, HOLDER, ASK_BOOL)?;
    Ok(Kind::Switch {
        actor: a,
        query: q,
        cases: vec![Case { val: YES, ev: yes }, Case { val: NO, ev: no }],
        params: Some(Cont { items: vec![("GameDataName".to_string(), Val::Str(flag.to_string())), ("Index".to_string(), Val::Int(-1))] }),
    })
}

fn clear_bool(name: String) -> Cont {
    Cont { items: vec![("Index".to_string(), Val::Int(-1)), ("Value".to_string(), Val::Bool(0)), ("GameDataName".to_string(), Val::Str(name))] }
}

fn give_params(actor: &str) -> Cont {
    Cont { items: vec![("ActorName".to_string(), Val::Str(actor.to_string())), ("IsNpcEventStarter".to_string(), Val::Bool(0))] }
}

fn add_one(f: &mut Flow, r: &Record) -> Res<()> {
    let name: String = r.event();
    if f.entries.iter().any(|e| e.name == name) {
        return Err(Error::Msg(format!("mailbox: {GIVE_FLOW} already has an entry {name}")));
    }
    let (a, set): (u16, u16) = action_slot(f, HOLDER, SET)?;
    let got: u16 = push(f, format!("{name}_Got"), Kind::Action { next: NONE, actor: a, act: set, params: Some(set_bool(r.got())) })?;
    let (main, subs): (u16, Vec<u16>) = match &r.style {
        Style::Get(actor) => {
            let give: u16 = push(f, format!("{name}_Give"), Kind::Sub { next: got, params: Some(give_params(actor)), flow: String::new(), entry: GIVE_ENTRY.to_string() })?;
            (give, vec![give])
        }
        Style::Tiers(fam, _) if r.windowed() => {
            let mut gives: Vec<u16> = Vec::new();
            let mut subs: Vec<u16> = Vec::new();
            for (i, t) in crate::progressive::tiers(fam)?.iter().enumerate() {
                let p: Cont = Cont { items: vec![("ActorName".to_string(), Val::Str(t.to_string()))] };
                let window: u16 = push(f, format!("{name}_Show{i}"), Kind::Sub { next: got, params: Some(p), flow: WINDOW_FLOW.to_string(), entry: WINDOW_ENTRY.to_string() })?;
                gives.push(crate::ocarina_hold::after(f, t, window)?);
                subs.push(window);
            }
            let k: Kind = crate::progressive::chain_to(f, fam, &gives)?;
            let main: u16 = push(f, format!("{name}_Tier"), k)?;
            (main, subs)
        }
        Style::Tiers(fam, first) => {
            let give: u16 = push(f, format!("{name}_Give"), Kind::Sub { next: got, params: Some(give_params(first)), flow: String::new(), entry: GIVE_ENTRY.to_string() })?;
            f.entries.push(Entry { name: name.clone(), subs: vec![give], vars: Vec::new(), main: give, pad: 0 });
            crate::progressive::wrap_grant(f, give as usize, &format!("{name}_Give"), "ActorName", fam)?;
            return Ok(());
        }
        Style::Dungeon(actor, k, dg) => {
            let first: u16 = crate::gift_keys::credit(f, *k, dg, got)?;
            let p: Cont = Cont { items: vec![("ActorName".to_string(), Val::Str(actor.clone()))] };
            let window: u16 = push(f, format!("{name}_Show"), Kind::Sub { next: first, params: Some(p), flow: WINDOW_FLOW.to_string(), entry: WINDOW_ENTRY.to_string() })?;
            (window, vec![window])
        }
        Style::Window(actor) => {
            let p: Cont = Cont { items: vec![("ActorName".to_string(), Val::Str(actor.clone()))] };
            let window: u16 = push(f, format!("{name}_Show"), Kind::Sub { next: got, params: Some(p), flow: WINDOW_FLOW.to_string(), entry: WINDOW_ENTRY.to_string() })?;
            let add: u16 = crate::ocarina_hold::after(f, actor, window)?;
            (add, vec![window])
        }
    };
    f.entries.push(Entry { name, subs, vars: Vec::new(), main, pad: 0 });
    Ok(())
}

fn add_dispatch(f: &mut Flow, recs: &[Record]) -> Res<()> {
    if f.entries.iter().any(|e| e.name == DISPATCH) {
        return Err(Error::Msg(format!("mailbox: {GIVE_FLOW} already has an entry {DISPATCH}")));
    }
    let (a, set): (u16, u16) = action_slot(f, HOLDER, SET)?;
    let mut first: u16 = NONE;
    let mut calls: Vec<u16> = Vec::with_capacity(recs.len());
    for r in recs.iter().rev() {
        let name: String = r.event();
        let call: u16 = push(f, format!("{name}_Call"), Kind::Sub { next: NONE, params: None, flow: String::new(), entry: name.clone() })?;
        calls.push(call);
        let k: Kind = ask(f, &r.got(), NONE, call)?;
        let held: u16 = push(f, format!("{name}_Held"), k)?;
        let clear: u16 = push(f, format!("{name}_Clear"), Kind::Action { next: held, actor: a, act: set, params: Some(clear_bool(r.recv())) })?;
        let k: Kind = ask(f, &r.recv(), clear, first)?;
        first = push(f, format!("{name}_Asked"), k)?;
    }
    calls.reverse();
    f.entries.push(Entry { name: DISPATCH.to_string(), subs: calls, vars: Vec::new(), main: first, pad: 0 });
    Ok(())
}

pub fn add_entries(b: &mut Bf, recs: &[Record]) -> Res<usize> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == GIVE_FLOW).ok_or(Error::Format("mailbox: Dm_SY_GetInEvent has no flow of that name"))?;
    if !f.entries.iter().any(|e| e.name == GIVE_ENTRY) {
        return Err(Error::Msg(format!("mailbox: {GIVE_FLOW} has no {GIVE_ENTRY} entry")));
    }
    for r in recs {
        add_one(f, r)?;
    }
    if !recs.is_empty() {
        add_dispatch(f, recs)?;
    }
    Ok(recs.len())
}

fn text<'a>(n: &'a Node, k: &str) -> &'a str {
    n.get(k).and_then(|v| v.as_str()).unwrap_or("")
}

fn template(y: &Byml, id: &str) -> Res<Node> {
    y.root
        .as_arr()
        .ok_or(Error::Format("mailbox: the table root is not a list"))?
        .iter()
        .find(|r| text(r, "__RowId") == id)
        .cloned()
        .ok_or_else(|| Error::Msg(format!("mailbox: the table has no {id} row")))
}

fn insert(y: &mut Byml, r: Node) -> Res<()> {
    let id: String = text(&r, "__RowId").to_string();
    let list: &mut Vec<Node> = y.root.as_arr_mut().ok_or(Error::Format("mailbox: the table root is not a list"))?;
    if list.iter().any(|x| text(x, "__RowId") == id) {
        return Err(Error::Msg(format!("mailbox: the table already has a row {id}")));
    }
    let at: usize = list.partition_point(|x| text(x, "__RowId") < id.as_str());
    list.insert(at, r);
    Ok(())
}

pub fn add_settings(y: &mut Byml, recs: &[Record]) -> Res<usize> {
    if recs.is_empty() {
        return Ok(0);
    }
    let mut n: Node = template(y, SETTING_TEMPLATE)?;
    n.set("__RowId", Node::Str(DISPATCH.to_string()));
    n.set("EntryPointName", Node::Str(DISPATCH.to_string()));
    n.set("FlowchartFileName", Node::Str(FLOWCHART.to_string()));
    insert(y, n)?;
    Ok(1)
}

fn names(s: &BS<String>) -> Node {
    let mut v: Vec<String> = s.iter().cloned().collect();
    v.sort_by_key(|x| x.to_lowercase());
    Node::Arr(v.into_iter().map(Node::Str).collect())
}

pub fn add_analyzed(y: &mut Byml, recs: &[Record], need: &Need) -> Res<usize> {
    if recs.is_empty() {
        return Ok(0);
    }
    let mut n: Node = template(y, ANALYZED_TEMPLATE)?;
    let mut flows: BS<String> = need.flows.clone();
    flows.remove("");
    flows.remove(GIVE_FLOW);
    if recs.iter().any(|r| r.windowed()) {
        flows.insert(WINDOW_FLOW.to_string());
    }
    n.set("__RowId", Node::Str(DISPATCH.to_string()));
    n.set(SUBS, names(&flows));
    if !need.kids.is_empty() {
        n.set(KIDS, names(&need.kids));
    }
    insert(y, n)?;
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_name_their_flags_and_event_and_refuse_bad_input() {
        let r: Record = Record::new("0123456789ab", "Item_Arrow_Fire").unwrap();
        assert_eq!((r.recv(), r.got(), r.event()), ("OOTRR.AP.Recv.0123456789ab".to_string(), "OOTRR.AP.Got.0123456789ab".to_string(), "OOTRR_AP_0123456789ab".to_string()));
        assert_eq!(flags(std::slice::from_ref(&r)), vec!["OOTRR.AP.Recv.0123456789ab".to_string(), "OOTRR.AP.Got.0123456789ab".to_string(), PENDING.to_string()]);
        assert!(Record::new("0123456789AB", "Item_X").is_err());
        assert!(Record::new("0123", "Item_X").is_err());
        assert!(Record::new("0123456789ab", "Item_KeyRing_GerudoTrainingGround").is_err());
        assert!(Record::new("0123456789ab", "Item X").is_err());
    }

    #[test]
    fn items_pick_the_give_their_kind_needs() {
        let d: Data = Data::load().unwrap();
        let k: &str = "00000000a11e";
        assert!(matches!(Record::of(&d, k, "Progressive Hookshot").unwrap().style, Style::Tiers(_, _)));
        assert!(matches!(Record::of(&d, k, "Zeldas Lullaby").unwrap().style, Style::Window(_)));
        assert!(matches!(Record::of(&d, k, "Boomerang").unwrap().style, Style::Get(_)));
        assert!(Record::of(&d, k, "No Such Item").is_err());
    }

    #[test]
    fn dungeon_items_from_another_world_are_credited_to_their_dungeon() {
        use crate::dungeon_grant::Kind as G;
        use crate::gift_keys::Item as I;
        let d: Data = Data::load().unwrap();
        let k: &str = "00000000a11e";
        let style = |n: &str| Record::of(&d, k, n).unwrap().style;
        assert_eq!(style("Small Key (Forest Temple)"), Style::Dungeon("Item_Dungeon_SmallKey".to_string(), I::Small, "ForestTemple"));
        assert_eq!(style("Small Key (Ganons Castle)"), Style::Dungeon("Item_Dungeon_SmallKey".to_string(), I::Small, "InsideGanonCastle"));
        assert_eq!(style("Boss Key (Forest Temple)"), Style::Dungeon("Item_Dungeon_BossKey".to_string(), I::Other(G::Boss), "ForestTemple"));
        assert_eq!(style("Boss Key (Ganons Castle)"), Style::Dungeon("Item_Dungeon_BossKey".to_string(), I::Other(G::Boss), "InsideGanonCastle"));
        assert_eq!(style("Map (Deku Tree)"), Style::Dungeon("Item_Dungeon_Map".to_string(), I::Other(G::Map), "InsideTheDekuTree"));
        assert_eq!(style("Compass (Forest Temple)"), Style::Dungeon("Item_Dungeon_Compass".to_string(), I::Other(G::Compass), "ForestTemple"));
        assert!(Record::of(&d, k, "Small Key (Thieves Hideout)").is_err());
        assert!(Record::of(&d, k, "Small Key Ring (Forest Temple)").is_err());
    }

    /// Double Defense from another world stopped the whole build: its actor name is 29 characters.
    #[test]
    fn every_item_another_world_can_send_gets_a_record() {
        let d: Data = Data::load().unwrap();
        let k: &str = "00000000a11e";
        assert_eq!(Record::of(&d, k, "Double Defense").unwrap().style, Style::Get("Item_FairyBless_DefenseDouble".to_string()));
        assert_eq!(Record::of(&d, k, "Nocturne of Shadow").unwrap().style, Style::Window("Item_Melody_ShadowNocturne".to_string()));
        assert_eq!(Record::of(&d, k, "Deku Sticks (3)").unwrap().style, Style::Get("Item_DekuStickBundle_Small".to_string()));
        let mut refused: Vec<String> = Vec::new();
        for i in &d.items {
            if LOCAL_KINDS.contains(&i.kind.as_str()) || i.actor.is_none() {
                continue;
            }
            if let Err(e) = Record::of(&d, k, &i.name) {
                refused.push(format!("{}: {e}", i.name));
            }
        }
        assert_eq!(refused, Vec::<String>::new());
    }

    #[test]
    fn a_progressive_ocarina_is_shown_in_the_window_and_never_held_up() {
        use crate::bfevfl::Actor;
        let d: Data = Data::load().unwrap();
        let name: String = d.items.iter().find(|i| d.family(&i.name) == Some("Ocarina")).map(|i| i.name.clone()).unwrap();
        let r: Record = Record::of(&d, "00000000a11e", &name).unwrap();
        assert!(r.windowed());
        assert!(!Record::of(&d, "00000000a11e", "Progressive Hookshot").unwrap().windowed());
        let actor = |n: &str| Actor { name: n.to_string(), sub: String::new(), arg: String::new(), actions: Vec::new(), queries: Vec::new(), params: None, ep: 0, cut: 0 };
        let mut f: Flow = Flow { name: GIVE_FLOW.to_string(), actors: vec![actor("EventSystemActor"), actor(HOLDER)], events: Vec::new(), entries: Vec::new() };
        add_one(&mut f, &r).unwrap();
        assert!(!f.events.iter().any(|e| matches!(&e.kind, Kind::Sub { entry, .. } if entry == GIVE_ENTRY)));
        let e: &Entry = f.entries.iter().find(|e| e.name == r.event()).unwrap();
        assert!(matches!(f.events[e.main as usize].kind, Kind::Switch { .. }));
        let shown: Vec<Option<&Val>> = e.subs.iter().map(|s| match &f.events[*s as usize].kind {
            Kind::Sub { params: Some(c), flow, entry, .. } if flow == WINDOW_FLOW && entry == WINDOW_ENTRY => c.get("ActorName"),
            _ => None,
        }).collect();
        let want: Vec<Val> = crate::ocarina_hold::OCARINAS.iter().map(|t| Val::Str(t.to_string())).collect();
        assert_eq!(shown, want.iter().map(Some).collect::<Vec<_>>());
        for t in want {
            let added: bool = f.events.iter().any(|e| matches!(&e.kind, Kind::Action { params: Some(c), .. } if c.get("Actor") == Some(&t)));
            let equipped: bool = f.events.iter().any(|e| matches!(&e.kind, Kind::Action { params: Some(c), .. } if c.get("GameDataName") == Some(&Val::Str(crate::ocarina_hold::EQUIP.to_string())) && c.get("Value") == Some(&t)));
            assert!(added && equipped, "{t:?}");
        }
    }
}
