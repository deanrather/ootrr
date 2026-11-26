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

pub const VOLVAGIA: Family = Family { areas: &[area(&["BGM_DungeonBoss_Volvagia"], "BGM_DungeonBoss_Volvagia_Main")], ambience: false };

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

pub const FAMILIES: [&Family; 8] = [&TOWNS, &FIELDS, &RANCH, &SPIRIT, &VOLVAGIA, &DEKU_TREE, &CAVES, &DUNGEONS];

const DAY: [u8; 24] = *b"C1\x18\x00\x08\x00\x01\x05\x06\x01\x02\x00\x09\x01\x05\x00\x0a\x00\x04\x00Day\x00";
const NIGHT: [u8; 26] = *b"C1\x1a\x00\x08\x00\x01\x05\x06\x01\x02\x00\x09\x01\x05\x00\x0a\x00\x04\x00Night\x00";

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

fn graph(nodes: &[Node], key: &str, quiet_at_night: bool) -> Res<Vec<Node>> {
    let missing = || E::Msg(format!("music table: no graph for {key}"));
    let start: &Node = nodes.iter().find(|n| kind(n) == Some("Start")).ok_or_else(missing)?;
    let from: &Node = nodes.iter().find(|n| kind(n) == Some("Asset") && n.get("Key").and_then(|k| k.as_str()) == Some(key)).ok_or_else(missing)?;
    let mut play: Node = asset(from, key)?;
    let mut first: Node = play.clone();
    let mut night: Vec<Node> = Vec::new();
    if quiet_at_night {
        let known = |c: &[u8]| nodes.iter().filter_map(|n| n.get("Transition").and_then(|t| t.as_arr())).flatten().any(|t| matches!(t.get("ConditionBin"), Some(Node::Bin(b)) if b.as_slice() == c));
        if !known(&DAY) || !known(&NIGHT) {
            return Err(missing());
        }
        let mut switch: Node = nodes.iter().find(|n| kind(n) == Some("Switch")).ok_or_else(missing)?.clone();
        let mut wait: Node = nodes.iter().find(|n| kind(n) == Some("Wait")).ok_or_else(missing)?.clone();
        wait.set("IsInfinity", Node::Bool(true));
        wait.set("WaitTime", seconds());
        switch.set("Transition", Node::Arr(vec![go(&wait, Some(&NIGHT))?, go(&play, None)?]));
        wait.set("Transition", Node::Arr(vec![go(&play, Some(&DAY))?]));
        play.set("Transition", Node::Arr(vec![go(&wait, Some(&NIGHT))?]));
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
