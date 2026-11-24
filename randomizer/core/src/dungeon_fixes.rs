//! Softlock fixes every seed carries.
//!
//! Forest Temple Bow room (Banc room 17, logic prefab Room_06 6b8c). Entering from the Red Poe
//! room locks the barred door behind Link (DungeonSwitchShutterWithLockSet, LockMinusZ) until the
//! eye switch above it is shot, and the eye only reacts once the Stalfos are dead and the chest
//! has appeared. With the Bow shuffled out of that chest, Link was shut in. The lock now opens
//! when the Stalfos are cleared (the chest has appeared, or did on an earlier visit), the way the
//! Lower Stalfos room opens its door. The eye switch still works and still sets its flag.

use std::path::Path;

use crate::ainb::Ainb;
use crate::error::{Error, Res};
use crate::romfs::Romfs;
use crate::staging::Out;

pub const BOW_ROOM: &str = "Logic/ForestTemple_Level_Room_06_Prefab_6b8c.logic.module.ainb";
pub const NOTE: &str = "Forest Temple: killing the Stalfos in the Bow room opens the barred door back to the Red Poe room, as in the Lower Stalfos room, so the room no longer locks Link in when its chest holds something other than the Bow. The eye switch above the door still opens it too. The key door to the Blue Poe room is unchanged.";

const SHUTTER: &str = "DungeonSwitchShutterWithLockSet_3b94.logic.module";
const LOCK_IN: &str = "LockMinusZ";
const LOCK: &str = "OperateORFixInputTrue";
const FOCUS: &str = "Gimmick_FocusOnGimmick_Basic_ff0d.logic.module";
const EYE: &str = "ActorLogicBootUpActorIsFirstOnEventTrigger";
const AND: &str = "OperateAND";
const MULTI: &str = "BoolMulti";
const BOOL: usize = 1;

fn one(v: Vec<usize>, what: &str) -> Res<usize> {
    match v.as_slice() {
        [x] => Ok(*x),
        _ => Err(Error::Msg(format!("dungeon fixes: expected one {what} in the Bow room module, found {}", v.len()))),
    }
}

/// The multi-link table slots an element's BoolMulti input reads.
fn multi_slots(a: &Ainb, el: usize) -> Res<Vec<usize>> {
    let g: usize = a.find_in(el, BOOL, MULTI).ok_or(Error::Format("dungeon fixes: a gate has no multi input"))?;
    let x = a.input(BOOL, g).ok_or(Error::Format("dungeon fixes: input out of range"))?;
    if x.src > -100 || x.plug < 1 {
        return Err(Error::Format("dungeon fixes: a gate input is not a multi link"));
    }
    let start: usize = (-100 - i32::from(x.src)) as usize;
    Ok((start..start + x.plug as usize).collect())
}

fn entry(a: &Ainb, j: usize) -> Res<(u16, u16)> {
    crate::door_open::multi_entry(a, j).ok_or(Error::Format("dungeon fixes: the multi link table is too short"))
}

/// The gate element feeding `input` of `el`, with the output it reads.
fn source(a: &Ainb, el: usize, input: &str) -> Res<(usize, i16)> {
    let g: usize = a.find_in(el, BOOL, input).ok_or(Error::Msg(format!("dungeon fixes: no {input} input")))?;
    let x = a.input(BOOL, g).ok_or(Error::Format("dungeon fixes: input out of range"))?;
    if x.src < 0 {
        return Err(Error::Msg(format!("dungeon fixes: {input} is not linked")));
    }
    Ok((x.src as usize, x.plug))
}

/// The slot of the lock's multi input that reads the eye switch focus, and the (element, output)
/// that means "Stalfos cleared": the second input of the AND that starts that focus, next to
/// the eye switch.
fn plan(a: &Ainb) -> Res<(usize, (u16, u16))> {
    let shutter: usize = one(a.find_els(SHUTTER), "shutter set")?;
    let (lock, _) = source(a, shutter, LOCK_IN)?;
    if a.el_name(lock) != LOCK {
        return Err(Error::Format("dungeon fixes: the Bow room lock is not the OR gate it was, the game files differ"));
    }
    let focus: usize = one(a.find_els(FOCUS), "eye switch focus")?;
    let slots: Vec<usize> = multi_slots(a, lock)?;
    let slot: usize = one(slots.iter().copied().filter(|&j| entry(a, j).is_ok_and(|(e, _)| e as usize == focus)).collect(), "lock input reading the eye switch focus")?;
    let (gate, _) = source(a, focus, "play")?;
    if a.el_name(gate) != AND {
        return Err(Error::Format("dungeon fixes: the eye switch focus no longer starts from an AND gate, the game files differ"));
    }
    let both: Vec<(u16, u16)> = multi_slots(a, gate)?.into_iter().map(|j| entry(a, j)).collect::<Res<Vec<_>>>()?;
    let cleared: Vec<(u16, u16)> = both.iter().copied().filter(|(e, _)| a.el_name(*e as usize) != EYE).collect();
    let [c] = cleared.as_slice() else {
        return Err(Error::Format("dungeon fixes: the eye switch gate does not pair the eye with one other input, the game files differ"));
    };
    if both.len() != 2 {
        return Err(Error::Format("dungeon fixes: the eye switch gate does not read exactly two inputs, the game files differ"));
    }
    Ok((slot, *c))
}

pub fn patch_bow_room(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    let (slot, (el, out)) = plan(&a)?;
    let o: usize = 8 * slot;
    a.multi[o..o + 2].copy_from_slice(&el.to_le_bytes());
    a.multi[o + 2..o + 4].copy_from_slice(&out.to_le_bytes());
    let bytes: Vec<u8> = a.write()?;
    let b: Ainb = Ainb::parse(&bytes)?;
    if entry(&b, slot)? != (el, out) {
        return Err(Error::Format("dungeon fixes: the Bow room module does not read back"));
    }
    Ok(bytes)
}

/// Stages every fix into `late`, reading files the build already wrote from `work`.
pub fn stage(rf: &mut Romfs, late: &mut Vec<Out>, work: &Path) -> Res<()> {
    let have = |p: &str| -> Option<Vec<u8>> { std::fs::read(p.split('/').fold(work.to_path_buf(), |a, s| a.join(s))).ok() };
    late.push(crate::qol::edit_plain(rf, &have, BOW_ROOM, patch_bow_room)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.is_dir() {
            Some(p)
        } else {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
            None
        }
    }

    fn lock_inputs(a: &Ainb) -> Vec<(u16, u16)> {
        let shutter: usize = a.find_els(SHUTTER)[0];
        let (lock, out) = source(a, shutter, LOCK_IN).unwrap();
        assert_eq!((a.el_name(lock), out), (LOCK, 0), "the lock reads the OR's False output");
        multi_slots(a, lock).unwrap().into_iter().map(|j| entry(a, j).unwrap()).collect()
    }

    #[test]
    fn the_bow_room_lock_reads_the_stalfos_clear_instead_of_the_eye_focus() {
        let Some(root) = dump() else {
            return;
        };
        let rf: Romfs = Romfs::open(&root).unwrap();
        let raw: Vec<u8> = rf.read(BOW_ROOM).unwrap();
        let a: Ainb = Ainb::parse(&raw).unwrap();
        assert_eq!(a.write().unwrap(), raw);
        let before: Vec<(u16, u16)> = lock_inputs(&a);
        assert!(before.iter().any(|(e, _)| a.el_name(*e as usize) == FOCUS));
        let out: Vec<u8> = patch_bow_room(&raw).unwrap();
        assert_eq!(out.len(), raw.len());
        let diff: usize = (0..raw.len()).filter(|&i| raw[i] != out[i]).count();
        assert!((1..=4).contains(&diff), "{diff} bytes changed");
        let b: Ainb = Ainb::parse(&out).unwrap();
        let after: Vec<(u16, u16)> = lock_inputs(&b);
        assert!(!after.iter().any(|(e, _)| b.el_name(*e as usize) == FOCUS));
        // The new input is the OR of the chest-appeared flag and the chest event, which the
        // Stalfos clear drives: the same gate the eye switch waits for.
        let (cleared, out_ix): (u16, u16) = *after.iter().find(|x| !before.contains(x)).unwrap();
        assert_eq!((b.el_name(cleared as usize), out_ix), ("OperateOR", 1));
        let reads: Vec<String> = multi_slots(&b, cleared as usize).unwrap().into_iter().map(|j| b.el_name(entry(&b, j).unwrap().0 as usize).to_string()).collect();
        assert!(reads.iter().any(|n| n == "HoldGameDataGetBoolSimple"), "{reads:?}");
        assert!(reads.iter().any(|n| n.starts_with("Gimmick_TreasureBoxAppear")), "{reads:?}");
        // Everything that kept the door shut but the focus is still read.
        assert!(before.iter().filter(|(e, _)| a.el_name(*e as usize) != FOCUS).all(|x| after.contains(x)));
        assert!(patch_bow_room(&out).is_err(), "a second run must refuse");
    }
}
