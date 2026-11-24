use crate::bfevfl::{Bf, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::error::{Error, Res};

pub const SETTING: &str = "skip_tower_escape";
pub const NUM: u16 = 164;
pub const FLOW: &str = "Defeat_Ganon";
pub const TOWER: &str = "CrumblingGanonCastle";
pub const ARRIVE: &str = "CrumblingGanonCastle_Arrive";
pub const FIELD: &str = "FinalBattleField";
pub const ENTRANCE: &str = "FinalBattleField_Entrance";
pub const ESCAPED: &str = "InsideGanonCaslte_Dungeon_Escape";
pub const ANCHOR: &str = "ChangeBancBeforehandAnchor";
pub const ANCHOR_FILE: &str = "Banc/InsideGanonCastle/InsideGanonCastle_Dynamic.bcett.byml.zs";
pub const MARK: &[u8] = b"ChangeBancBeforehandAnchor";
const CHANGE: &str = "GameSystemActor.EventReserveBancChange";

pub const NOTE: &str = "Skip the tower escape: after Ganondorf falls, Link goes straight to the field below the tower, as if he had just run out of the collapsing castle, and the Ganon fight follows as usual. The banc change after Ganondorf's death in Defeat_Ganon loads FinalBattleField at FinalBattleField_Entrance with the event that plays at the end of the escape (InsideGanonCaslte_Dungeon_Escape), and the anchor in Ganon's tower that preloads the next map preloads that field. The logic no longer needs the Longshot for the escape. Experimental, untested in the game.";

fn text(c: &crate::bfevfl::Cont, k: &str) -> Option<String> {
    match c.get(k) {
        Some(Val::Str(s)) => Some(s.clone()),
        _ => None,
    }
}

fn put(c: &mut crate::bfevfl::Cont, k: &str, v: &str) {
    for (x, y) in c.items.iter_mut() {
        if x == k {
            *y = Val::Str(v.to_string());
        }
    }
}

pub fn patch_flow(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == FLOW).ok_or(Error::Format("tower escape: Defeat_Ganon has no Defeat_Ganon flow"))?;
    let hits: Vec<usize> = (0..f.events.len())
        .filter(|&i| match &f.events[i].kind {
            Kind::Action { actor, act, params: Some(c), .. } => {
                f.actors.get(*actor as usize).is_some_and(|a| a.actions.get(*act as usize).is_some_and(|q| format!("{}.{q}", a.name) == CHANGE))
                    && text(c, "NextBanc").as_deref() == Some(TOWER)
                    && text(c, "BootEventName").as_deref() == Some(ARRIVE)
            }
            _ => false,
        })
        .collect();
    let [i] = hits.as_slice() else {
        return Err(Error::Msg(format!("tower escape: expected one banc change to the collapsing tower, found {}", hits.len())));
    };
    let c = f.events[*i].kind.params_mut().ok_or(Error::Format("tower escape: the banc change has no parameters"))?;
    put(c, "NextBanc", FIELD);
    put(c, "NextStartPos", ENTRANCE);
    put(c, "BootEventName", ESCAPED);
    put(c, "EventStarterName", ENTRANCE);
    Ok(())
}

pub fn patch_anchor(y: &mut Byml) -> usize {
    let Some(list) = y.root.get_mut("Actors").and_then(|a| a.as_arr_mut()) else {
        return 0;
    };
    let mut n: usize = 0;
    for a in list.iter_mut() {
        if a.get("Gyaml").and_then(|g| g.as_str()) != Some(ANCHOR) {
            continue;
        }
        let Some(d) = a.get_mut("Dynamic") else {
            continue;
        };
        if d.get("NextBancParamName").and_then(|v| v.as_str()) == Some(TOWER) {
            d.set("NextBancParamName", Node::Str(FIELD.to_string()));
            n += 1;
        }
    }
    n
}
