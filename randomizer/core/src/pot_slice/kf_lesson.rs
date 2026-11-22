use crate::ainb::Ainb;
use crate::error::{Error, Res};
use crate::romfs::Romfs;
use crate::staging::Out;

pub const MODULE: &str = "Logic/KokiriForest_Child_Level_All_02_6960.logic.module.ainb";
pub const LIFT: &str = "ActorLogicLiftableActor";
pub const LESSON_ROCK: &str = "Obj_LiftRock_A_02_2143";
pub const CHECK_ROCK: &str = "Obj_LiftRock_A_01_0de2";
const CREATE: &str = "Logic_Create";
const DELETE: &str = "Logic_Delete";

fn rock(a: &Ainb, name: &str) -> Option<usize> {
    a.find_els(LIFT).into_iter().find(|&i| a.prop_str(i, "InstanceName") == Some(name))
}

pub fn patch(a: &mut Ainb) -> Res<usize> {
    let i: usize = rock(a, CHECK_ROCK).ok_or(Error::Format("kokiri lesson: the check rock is missing, the game files differ"))?;
    if rock(a, LESSON_ROCK).is_none() {
        return Err(Error::Format("kokiri lesson: the lesson rock is missing, the game files differ"));
    }
    a.force_bool(i, CREATE, true)?;
    a.force_bool(i, DELETE, false)?;
    Ok(i)
}

pub fn patch_bytes(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    patch(&mut a)?;
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if back.write()? != out || !crate::ainb::audit(&back).is_empty() {
        return Err(Error::Format("kokiri lesson: the patched module does not read back"));
    }
    Ok(out)
}

pub fn stage(rf: &Romfs) -> Res<Out> {
    let raw: Vec<u8> = rf.read(MODULE)?;
    let out: Vec<u8> = patch_bytes(&raw)?;
    Ok(Out { path: MODULE.to_string(), old: raw.len(), new: out.len(), z: out })
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn the_check_rock_is_there_from_the_start_and_the_lesson_rock_is_untouched() {
        let Some(root) = real() else {
            return;
        };
        let rf: Romfs = Romfs::open(&root).unwrap();
        let old: Ainb = Ainb::parse(&rf.read(MODULE).unwrap()).unwrap();
        let c: usize = rock(&old, CHECK_ROCK).unwrap();
        let l: usize = rock(&old, LESSON_ROCK).unwrap();
        let src = |a: &Ainb, i: usize, n: &str| a.input(1, a.find_in(i, 1, n).unwrap()).unwrap().src;
        assert!(src(&old, c, CREATE) >= 0 && src(&old, c, DELETE) >= 0);
        let o: Out = stage(&rf).unwrap();
        let new: Ainb = Ainb::parse(&o.z).unwrap();
        assert!(crate::ainb::audit(&new).is_empty());
        assert_eq!((src(&new, c, CREATE), src(&new, c, DELETE)), (-1, -1));
        let val = |a: &Ainb, i: usize, n: &str| a.input(1, a.find_in(i, 1, n).unwrap()).unwrap().val;
        assert_eq!(val(&new, c, CREATE), vec![1, 0, 0, 0]);
        assert_eq!(val(&new, c, DELETE), vec![0, 0, 0, 0]);
        assert_eq!(new.els.len(), old.els.len());
        assert_eq!(new.params[l], old.params[l]);
        for n in [CREATE, DELETE] {
            assert_eq!(new.input(1, new.find_in(l, 1, n).unwrap()), old.input(1, old.find_in(l, 1, n).unwrap()));
        }
        assert!(new.params[c].plugs.is_empty());
    }
}
