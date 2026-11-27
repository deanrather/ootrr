use std::collections::BTreeSet as BS;

use crate::byml::{Byml, Node};
use crate::rsizetable::hash;
use crate::silence::SCENE;
use crate::slink::Link;

pub const MUSIC: [&str; 2] = ["Bgm", "BgmInsideWorldFolder"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Music,
    Fanfare,
    Kept,
}

fn lowers_music(y: &Byml, ducker: &str) -> bool {
    let Some(targets) = y.root.get("ManualDucking").and_then(|m| m.get(ducker)).and_then(|d| d.get("Targets")).and_then(|t| t.as_arr()) else {
        return false;
    };
    targets.iter().any(|t| t.get("TargetGroupName").and_then(|n| n.as_str()).is_some_and(|n| MUSIC.contains(&n)) && matches!(t.get("Volume"), Some(Node::F32(v)) if *v < 1.0))
}

pub fn ducking_groups(y: &Byml) -> BS<String> {
    let Some(Node::Map(all)) = y.root.get("GroupParam") else {
        return BS::new();
    };
    let ducker = |g: &Node| -> Option<String> { g.get("Ducking")?.get("ManualDuckerName")?.as_str().filter(|n| !n.is_empty()).map(|n| n.to_string()) };
    all.iter().filter(|(_, g)| ducker(g).is_some_and(|d| lowers_music(y, &d))).map(|(name, _)| name.clone()).collect()
}

pub struct Sorts {
    fanfare: BS<u32>,
    kept: BS<u32>,
}

impl Sorts {
    pub fn read(link: &Link, raw: &[u8], groups: &Byml, loops: &dyn Fn(&str) -> bool) -> Sorts {
        let duck: BS<String> = ducking_groups(groups);
        let mut fanfare: BS<u32> = BS::new();
        let mut once: BS<u32> = BS::new();
        for c in &link.calls {
            let (Some(asset), Some(group)) = (link.asset(raw, c), link.group(raw, c)) else {
                continue;
            };
            if duck.contains(&group) {
                fanfare.insert(c.user);
            } else if group == SCENE && asset.starts_with("BGM_") && !loops(&asset) {
                once.insert(c.user);
            }
        }
        let kept: BS<u32> = once.difference(&fanfare).copied().collect();
        Sorts { fanfare, kept }
    }

    pub fn of(&self, user: &str) -> Sort {
        let h: u32 = hash(user);
        if self.fanfare.contains(&h) {
            Sort::Fanfare
        } else if self.kept.contains(&h) {
            Sort::Kept
        } else {
            Sort::Music
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(rows: Vec<(&str, Node)>) -> Node {
        Node::Map(rows.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    fn target(group: &str, volume: f32) -> Node {
        map(vec![("TargetGroupName", Node::Str(group.to_string())), ("Volume", Node::F32(volume))])
    }

    fn groups(own: &[(&str, &str)], duckers: &[(&str, Vec<Node>)]) -> Byml {
        let params: Vec<(&str, Node)> = own.iter().map(|(g, d)| (*g, map(vec![("Default", map(vec![("Volume", Node::F32(1.0))])), ("Ducking", map(vec![("ManualDuckerName", Node::Str(d.to_string()))]))]))).collect();
        let ducks: Vec<(&str, Node)> = duckers.iter().map(|(n, t)| (*n, map(vec![("Targets", Node::Arr(t.clone()))]))).collect();
        Byml { root: map(vec![("GroupParam", map(params)), ("ManualDucking", map(ducks))]), version: 7, big: true }
    }

    #[test]
    fn a_group_ducks_when_its_own_ducker_lowers_a_music_group() {
        let y: Byml = groups(
            &[("FanfareDefault", "fan"), ("PlayableDemoVoice", "voice"), ("Even", "even"), ("BgmDefault", ""), ("Lost", "missing")],
            &[("fan", vec![target("Bgm", 0.6), target("DemoBgmDefault", 0.6)]), ("voice", vec![target("World", 0.6)]), ("even", vec![target("BgmInsideWorldFolder", 1.0)])],
        );
        assert_eq!(ducking_groups(&y), BS::from(["FanfareDefault".to_string()]));
    }
}
