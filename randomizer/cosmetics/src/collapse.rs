use std::collections::BTreeSet as BS;

use crate::byml::{Byml, Node};
use crate::error::{Error as E, Res};

pub struct Area {
    pub events: &'static [&'static str],
    pub key: &'static str,
    pub quiet_at_night: bool,
}

pub struct Family {
    pub areas: &'static [Area],
    pub ambience: bool,
}

const fn area(events: &'static [&'static str], key: &'static str) -> Area {
    Area { events, key, quiet_at_night: false }
}

const fn day_only(events: &'static [&'static str], key: &'static str) -> Area {
    Area { events, key, quiet_at_night: true }
}

pub const TOWNS: Family = Family {
    areas: &[
        day_only(&["BGM_Market"], "BGM_Market_DayTime_01"),
        area(&["BGM_KakarikoVillage_Child", "BGM_FishingPond_Child"], "BGM_KakarikoVillage_Child_Daytime_01"),
        area(&["BGM_KakarikoVillage_Adult", "BGM_FishingPond_Adult"], "BGM_KakarikoVillage_Adult_Daytime_01"),
        area(&["BGM_KokiriForest"], "BGM_Kokiri_Daytime_01"),
        area(&["BGM_GerudoFortress"], "BGM_GerudoFortress_Daytime_01"),
    ],
    ambience: false,
};

pub const FIELDS: Family = Family {
    areas: &[
        day_only(&["BGM_HyruleField"], "BGM_SEQ_FIELDRA0"),
        day_only(&["BGM_HyruleField_Garden"], "BGM_HyruleField_RA0_Garden"),
        day_only(&["BGM_HyruleField_ZoraFountain"], "BGM_HyruleField_RA0_ZoraFountain"),
    ],
    ambience: false,
};

pub const RANCH: Family = Family { areas: &[day_only(&["BGM_LonLonRanch_Child", "BGM_LonLonRanch_Adult"], "BGM_LonLonRanch_Morning")], ambience: false };

pub const SPIRIT: Family = Family { areas: &[area(&["BGM_SpiritTemple"], "BGM_SpiritTemple_Loop")], ambience: false };

pub const DEKU_TREE: Family = Family { areas: &[area(&["BGM_InsideTheDekuTree", "BGM_EnemyMuseum", "BGM_Common_DekuTree"], "BGM_InsideTheDekuTree_Melody_01")], ambience: true };

pub const CAVES: Family = Family { areas: &[area(&["BGM_Dungeon", "BGM_Common_Dungeon", "BGM_Common_Dungeon_Scene"], "BGM_Dungeon_Pad")], ambience: true };

pub const DUNGEONS: Family = Family {
    areas: &[
        area(&["BGM_FireTemple"], "BGM_FireTemple_Main"),
        area(&["BGM_IceCavern"], "BGM_IceCavern_01"),
        area(&["BGM_InsideJabuJabuBelly"], "BGM_InsideJabuJabuBelly_Main"),
        area(&["BGM_InsideGanonCastle"], "BGM_InsideGanonCastle"),
        area(&["BGM_GanonTower"], "BGM_GanonTower_Organ_01"),
    ],
    ambience: true,
};

pub const FAMILIES: [&Family; 7] = [&TOWNS, &FIELDS, &RANCH, &SPIRIT, &DEKU_TREE, &CAVES, &DUNGEONS];

pub fn areas() -> impl Iterator<Item = (&'static Area, bool)> {
    FAMILIES.iter().flat_map(|f| f.areas.iter().map(|a| (a, f.ambience)))
}

pub fn of(event: &str) -> Option<(&'static Area, bool)> {
    areas().find(|(a, _)| a.events.contains(&event))
}

fn map(entries: Vec<(&str, Node)>) -> Node {
    Node::Map(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn text(s: &str) -> Node {
    Node::Str(s.to_string())
}

fn seconds() -> Node {
    map(vec![("Type", text("Second")), ("Value", Node::F32(0.0))])
}

fn kind(n: &Node) -> Option<&str> {
    n.get("Type").and_then(|t| t.as_str())
}

fn go(to: &Node, when: Option<&[u8]>) -> Res<Node> {
    let mut out: Vec<(&str, Node)> = Vec::new();
    if let Some(c) = when {
        out.push(("ConditionBin", Node::Bin(c.to_vec())));
    }
    out.push(("DestinationNode", to.get("Name").cloned().ok_or(E::Format("music table: a node has no name"))?));
    out.push(("Type", text("Playing")));
    Ok(map(out))
}

fn asset(from: &Node, key: &str) -> Res<Node> {
    let one: Node = Node::F32(1.0);
    let mut out: Node = map(vec![
        ("FadeInTime", seconds()),
        ("FadeInType", text("Linear")),
        ("FadeOutTime", seconds()),
        ("FadeOutType", text("Linear")),
        ("GUID", from.get("GUID").cloned().ok_or(E::Format("music table: a node has no id"))?),
        ("IsDurationAuto", Node::Bool(true)),
        ("IsDurationInfinity", Node::Bool(false)),
        ("Key", text(key)),
        ("Name", from.get("Name").cloned().ok_or(E::Format("music table: a node has no name"))?),
        ("NodeVolume", one.clone()),
        ("Transition", Node::Arr(Vec::new())),
        ("Type", text("Asset")),
        ("Volume", Node::Arr(vec![one; 8])),
    ]);
    if let Some(sync) = from.get("SyncGroupHash") {
        out.set("SyncGroupHash", sync.clone());
    }
    Ok(out)
}

fn ending<'a>(nodes: &'a [Node], name: &str) -> BS<(usize, &'a [u8])> {
    let end: Vec<u8> = format!("\0{name}\0").into_bytes();
    nodes
        .iter()
        .filter_map(|n| n.get("Transition").and_then(|t| t.as_arr()))
        .flatten()
        .filter_map(|t| match t.get("ConditionBin") {
            Some(Node::Bin(b)) if b.ends_with(&end) => Some((b.len(), b.as_slice())),
            _ => None,
        })
        .collect()
}

fn test_of<'a>(c: &'a [u8], name: &str) -> Option<&'a [u8]> {
    c.get(4..c.len().checked_sub(name.len() + 1)?)
}

fn only<'a>(found: &[&'a [u8]], name: &str, key: &str) -> Res<&'a [u8]> {
    match found {
        [c] => Ok(c),
        _ => Err(E::Msg(format!("music table: {key} needs one plain {name} condition, found {}", found.len()))),
    }
}

fn times<'a>(nodes: &'a [Node], key: &str) -> Res<(&'a [u8], &'a [u8])> {
    let days: BS<(usize, &[u8])> = ending(nodes, "Day");
    let short: usize = days.first().map_or(0, |d| d.0);
    let day: &[u8] = only(&days.iter().take_while(|d| d.0 == short).map(|d| d.1).collect::<Vec<&[u8]>>(), "Day", key)?;
    let same: Vec<&[u8]> = ending(nodes, "Night").iter().map(|n| n.1).filter(|n| test_of(n, "Night") == test_of(day, "Day")).collect();
    Ok((day, only(&same, "Night", key)?))
}

fn graph(nodes: &[Node], key: &str, quiet_at_night: bool) -> Res<Vec<Node>> {
    let missing = || E::Msg(format!("music table: no graph for {key}"));
    let start: &Node = nodes.iter().find(|n| kind(n) == Some("Start")).ok_or_else(missing)?;
    let from: &Node = nodes.iter().find(|n| kind(n) == Some("Asset") && n.get("Key").and_then(|k| k.as_str()) == Some(key)).ok_or_else(missing)?;
    let mut play: Node = asset(from, key)?;
    let mut first: Node = play.clone();
    let mut night: Vec<Node> = Vec::new();
    if quiet_at_night {
        let (day, dark): (&[u8], &[u8]) = times(nodes, key)?;
        let mut switch: Node = nodes.iter().find(|n| kind(n) == Some("Switch")).ok_or_else(missing)?.clone();
        let mut wait: Node = nodes.iter().find(|n| kind(n) == Some("Wait")).ok_or_else(missing)?.clone();
        wait.set("IsInfinity", Node::Bool(true));
        wait.set("WaitTime", seconds());
        switch.set("Transition", Node::Arr(vec![go(&wait, Some(dark))?, go(&play, None)?]));
        wait.set("Transition", Node::Arr(vec![go(&play, Some(day))?]));
        play.set("Transition", Node::Arr(vec![go(&wait, Some(dark))?]));
        first = switch.clone();
        night = vec![switch, wait];
    }
    let mut begin: Node = start.clone();
    let name: Node = first.get("Name").cloned().ok_or_else(missing)?;
    for t in begin.get_mut("Transition").and_then(|t| t.as_arr_mut()).ok_or_else(missing)? {
        t.set("DestinationNode", name.clone());
        t.remove("TransitionParamIndex");
    }
    let mut out: Vec<Node> = vec![begin];
    out.extend(night);
    out.push(play);
    Ok(out)
}

pub fn collapse(y: &mut Byml, event: &str) -> Res<()> {
    let (a, _): (&Area, bool) = of(event).ok_or_else(|| E::Msg(format!("music table: {event} is not an area theme")))?;
    let e: &mut Node = y.root.get_mut("BGM").and_then(|b| b.get_mut(event)).ok_or_else(|| E::Msg(format!("music table: no {event}")))?;
    let new: Vec<Node> = graph(e.get("Node").and_then(|n| n.as_arr()).unwrap_or(&[]), a.key, a.quiet_at_night)?;
    e.set("Node", Node::Arr(new));
    e.set("TransitionParam", Node::Arr(Vec::new()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(conditions: &[&[u8]]) -> Vec<Node> {
        let t: Vec<Node> = conditions.iter().map(|c| map(vec![("ConditionBin", Node::Bin(c.to_vec()))])).collect();
        vec![map(vec![("Transition", Node::Arr(t))])]
    }

    #[test]
    fn the_plain_day_condition_and_its_night_twin_are_taken_and_anything_else_is_refused() {
        let nodes: Vec<Node> = with(&[b"head=\0long\0Day\0", b"head=\0Day\0", b"head!\0Night\0", b"head=\0Night\0", b"head=\0Day\0"]);
        let (day, dark): (&[u8], &[u8]) = times(&nodes, "k").unwrap();
        assert_eq!((day, dark), (&b"head=\0Day\0"[..], &b"head=\0Night\0"[..]));
        assert!(times(&with(&[b"head=\0Night\0"]), "k").is_err());
        assert!(times(&with(&[b"head=\0Day\0", b"head!\0Night\0"]), "k").is_err());
        assert!(times(&with(&[b"head=\0Day\0", b"head!\0Day\0", b"head=\0Night\0"]), "k").is_err());
    }
}
