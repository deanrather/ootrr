//! King Zora's spare tunic. After the thaw, talking to King Zora without a Zora Tunic in the pouch
//! ("You're hopeless...") sets `Npc_Zora_011_Shop_Adult_CanGetZoraArmor`, and the adult Zora shop
//! keeper then hands over a free Zora Tunic ("King Zora sent word that we should give this to
//! you"). With King Zora's thaw gift shuffled (location "ZD King Zora Thawed") that is a second,
//! unshuffled Zora Tunic the logic never counts, so the keeper's check on that flag now always
//! takes its "not owed" branch: the shop talks as usual and never gives the tunic.

use crate::bfevfl::{Bf, Flow, Kind, Val};
use crate::error::{Error, Res};
use crate::romfs::Romfs;

use super::guard;

pub const FLOW: &str = "Npc_Zora_011_Shop_Adult";
pub const FLAG: &str = "Npc_Zora_011_Shop_Adult_CanGetZoraArmor";
const QUERY: &str = "EventQueryIsGameDataBool";

/// Points the true case of every bool switch on [`FLAG`] at its false case. Returns how many
/// switches were cut; errors when none still has a separate true branch (the game files differ, or
/// the edit already ran).
pub fn cut(b: &mut Bf) -> Res<usize> {
    let f: &mut Flow = guard::flow_mut(b, FLOW)?;
    let queries: Vec<Vec<bool>> = f.actors.iter().map(|a| a.queries.iter().map(|q| q == QUERY).collect()).collect();
    let mut n: usize = 0;
    for i in 0..f.events.len() {
        let Kind::Switch { actor, query, cases, params } = &mut f.events[i].kind else { continue };
        let named: bool = matches!(params.as_ref().and_then(|p| p.get("GameDataName")), Some(Val::Str(s)) if s == FLAG);
        let is_bool: bool = queries.get(*actor as usize).and_then(|a| a.get(*query as usize)).copied().unwrap_or(false);
        if !named || !is_bool {
            continue;
        }
        let no: u16 = cases.iter().find(|c| c.val == 1).map(|c| c.ev).ok_or_else(|| Error::Msg(format!("zora tunic: {FLOW} event {i} has no false case")))?;
        if cases.iter().all(|c| c.ev == no) {
            continue;
        }
        for c in cases.iter_mut() {
            c.ev = no;
        }
        n += 1;
    }
    if n == 0 {
        return Err(Error::Msg(format!("zora tunic: {FLOW} has no switch on {FLAG} left to cut, the game files differ")));
    }
    Ok(n)
}

/// Stages the cut as a late file, on top of what the shelf shuffle wrote to the same flow.
pub fn stage_late(rf: &mut Romfs, late: &mut Vec<crate::staging::Out>, work: &std::path::Path) -> Res<()> {
    crate::story_skip::stage_late(rf, late, work, FLOW, |b| cut(b).map(|_| ()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfevfl::flow::NONE;
    use std::path::PathBuf as PB;

    fn real() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(p)
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn reached(f: &Flow) -> Vec<bool> {
        let mut seen: Vec<bool> = vec![false; f.events.len()];
        let mut todo: Vec<u16> = f.entries.iter().map(|e| e.main).collect();
        while let Some(at) = todo.pop() {
            if at == NONE || seen[at as usize] {
                continue;
            }
            seen[at as usize] = true;
            match &f.events[at as usize].kind {
                Kind::Action { next, .. } | Kind::Join { next } | Kind::Sub { next, .. } => todo.push(*next),
                Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
                Kind::Fork { join, forks } => {
                    todo.push(*join);
                    todo.extend(forks.iter().copied());
                }
                Kind::Label { .. } => {}
            }
        }
        seen
    }

    fn gives_tunic(f: &Flow, i: usize) -> bool {
        matches!(&f.events[i].kind, Kind::Sub { params: Some(p), .. } if p.get("ActorName") == Some(&Val::Str("Obj_Armor_Zora".to_string())))
    }

    #[test]
    fn the_keeper_never_reaches_the_free_tunic() {
        let Some(root) = real() else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let mut b: Bf = Bf::parse(&rf.read_zs(&format!("Event/EventFlow/{FLOW}.bfevfl.zs")).unwrap()).unwrap();
        let f: &Flow = b.flows.iter().find(|f| f.name == FLOW).unwrap();
        let before: Vec<bool> = reached(f);
        assert!((0..f.events.len()).any(|i| before[i] && gives_tunic(f, i)), "the game keeper should give the tunic before the cut");
        assert_eq!(cut(&mut b).unwrap(), 1);
        let f: &Flow = b.flows.iter().find(|f| f.name == FLOW).unwrap();
        let after: Vec<bool> = reached(f);
        assert!(!(0..f.events.len()).any(|i| after[i] && gives_tunic(f, i)));
        assert!(cut(&mut b).is_err(), "took the cut twice");
        let back: Bf = Bf::parse(&b.write().unwrap()).unwrap();
        assert_eq!(back, b);
    }
}
