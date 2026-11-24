use std::collections::BTreeSet as BS;

use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Cont, Entry, Event, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::dungeon_credit::{action_slot, set_bool};
use crate::error::{Error, Res};
use crate::flow_extra::analyzed::{Need, KIDS, SUBS};

use super::markers::{HOLDER, SET};

pub const GIVE_FILE: &str = crate::difficulty::dupes::GIVE_FILE;
pub const GIVE_FLOW: &str = crate::difficulty::dupes::GIVE_FLOW;
pub const GIVE_ENTRY: &str = "GetInEvent_NotTurn_NotNpc";
pub const FLOWCHART: &str = "Work/Event/EventFlow/Dm_SY_GetInEvent.evfc";
pub const SETTING_TEMPLATE: &str = "CrumblingGanonCastle_Arrive";
pub const ANALYZED_TEMPLATE: &str = "NaviLockOnHint";
pub const MAX_ACTOR: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub key: String,
    pub actor: String,
}

impl Record {
    pub fn new(key: &str, actor: &str) -> Res<Record> {
        if key.len() != 12 || !key.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
            return Err(Error::Msg(format!("mailbox: record key {key:?} is not 12 lowercase hex digits")));
        }
        if actor.is_empty() || actor.len() > MAX_ACTOR || !actor.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
            return Err(Error::Msg(format!("mailbox: {actor:?} is not a usable actor name (at most {MAX_ACTOR} characters, longer names crash the item get)")));
        }
        Ok(Record { key: key.to_string(), actor: actor.to_string() })
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
    recs.iter().flat_map(|r| [r.recv(), r.got()]).collect()
}

fn idx(n: usize) -> Res<u16> {
    u16::try_from(n).ok().filter(|&i| i != NONE).ok_or(Error::Format("mailbox: the give flow is full"))
}

pub fn add_entries(b: &mut Bf, recs: &[Record]) -> Res<usize> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == GIVE_FLOW).ok_or(Error::Format("mailbox: Dm_SY_GetInEvent has no flow of that name"))?;
    if !f.entries.iter().any(|e| e.name == GIVE_ENTRY) {
        return Err(Error::Msg(format!("mailbox: {GIVE_FLOW} has no {GIVE_ENTRY} entry")));
    }
    let (a, set): (u16, u16) = action_slot(f, HOLDER, SET)?;
    for r in recs {
        let name: String = r.event();
        if f.entries.iter().any(|e| e.name == name) {
            return Err(Error::Msg(format!("mailbox: {GIVE_FLOW} already has an entry {name}")));
        }
        let give: u16 = idx(f.events.len())?;
        let mark: u16 = idx(f.events.len() + 1)?;
        let params: Cont = Cont { items: vec![("ActorName".to_string(), Val::Str(r.actor.clone())), ("IsNpcEventStarter".to_string(), Val::Bool(0))] };
        f.events.push(Event { name: format!("{name}_Give"), kind: Kind::Sub { next: mark, params: Some(params), flow: String::new(), entry: GIVE_ENTRY.to_string() } });
        f.events.push(Event { name: format!("{name}_Got"), kind: Kind::Action { next: NONE, actor: a, act: set, params: Some(set_bool(r.got())) } });
        f.entries.push(Entry { name, subs: vec![give], vars: Vec::new(), main: give, pad: 0 });
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
    let base: Node = template(y, SETTING_TEMPLATE)?;
    for r in recs {
        let mut n: Node = base.clone();
        n.set("__RowId", Node::Str(r.event()));
        n.set("EntryPointName", Node::Str(r.event()));
        n.set("FlowchartFileName", Node::Str(FLOWCHART.to_string()));
        insert(y, n)?;
    }
    Ok(recs.len())
}

fn names(s: &BS<String>) -> Node {
    let mut v: Vec<String> = s.iter().cloned().collect();
    v.sort_by_key(|x| x.to_lowercase());
    Node::Arr(v.into_iter().map(Node::Str).collect())
}

pub fn add_analyzed(y: &mut Byml, recs: &[Record], need: &Need) -> Res<usize> {
    let base: Node = template(y, ANALYZED_TEMPLATE)?;
    let mut flows: BS<String> = need.flows.clone();
    flows.remove("");
    for r in recs {
        let mut n: Node = base.clone();
        n.set("__RowId", Node::Str(r.event()));
        n.set(SUBS, names(&flows));
        if !need.kids.is_empty() {
            n.set(KIDS, names(&need.kids));
        }
        insert(y, n)?;
    }
    Ok(recs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_name_their_flags_and_event_and_refuse_bad_input() {
        let r: Record = Record::new("0123456789ab", "Item_Arrow_Fire").unwrap();
        assert_eq!((r.recv(), r.got(), r.event()), ("OOTRR.AP.Recv.0123456789ab".to_string(), "OOTRR.AP.Got.0123456789ab".to_string(), "OOTRR_AP_0123456789ab".to_string()));
        assert_eq!(flags(&[r]), vec!["OOTRR.AP.Recv.0123456789ab".to_string(), "OOTRR.AP.Got.0123456789ab".to_string()]);
        assert!(Record::new("0123456789AB", "Item_X").is_err());
        assert!(Record::new("0123", "Item_X").is_err());
        assert!(Record::new("0123456789ab", "Item_KeyRing_GerudoTrainingGround").is_err());
        assert!(Record::new("0123456789ab", "Item X").is_err());
    }
}
