#[cfg(test)]
mod tests {
    use std::collections::BTreeSet as BS;

    use crate::bfevfl::flow::NONE;
    use crate::bfevfl::{Bf, Flow, Kind};
    use crate::flow_extra::apply::{always_ops, edit_flow};
    use crate::romfs::Romfs;

    struct Cut {
        file: &'static str,
        start: usize,
        gone: &'static [usize],
    }

    const CUTS: [Cut; 5] = [
        Cut { file: "SimpleShop.bfevfl", start: 71, gone: &[69, 74, 114, 75, 76, 73] },
        Cut { file: "MiniGame_Diving.bfevfl", start: 14, gone: &[33] },
        Cut { file: "Goto_HyruleCastle.bfevfl", start: 149, gone: &[13] },
        Cut { file: "Npc_Hylia_049_Adult.bfevfl", start: 223, gone: &[85] },
        Cut { file: "Minigame_FindOutCucco.bfevfl", start: 262, gone: &[24, 25] },
    ];

    fn next_of(f: &Flow, i: usize) -> Vec<usize> {
        let to = |n: &u16| -> Vec<usize> { if *n == NONE { Vec::new() } else { vec![usize::from(*n)] } };
        match &f.events[i].kind {
            Kind::Action { next, .. } | Kind::Join { next } | Kind::Sub { next, .. } => to(next),
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter().flat_map(|c| to(&c.ev)).collect(),
            Kind::Fork { join, forks } => forks.iter().chain([join]).flat_map(to).collect(),
            Kind::Label { .. } => Vec::new(),
        }
    }

    fn reach(f: &Flow, start: usize) -> (BS<usize>, BS<usize>) {
        let mut seen: BS<usize> = BS::new();
        let mut ends: BS<usize> = BS::new();
        let mut todo: Vec<usize> = vec![start];
        while let Some(i) = todo.pop() {
            if !seen.insert(i) {
                continue;
            }
            let n: Vec<usize> = next_of(f, i);
            if n.is_empty() {
                ends.insert(i);
            }
            todo.extend(n);
        }
        (seen, ends)
    }

    #[test]
    fn every_cut_flow_skips_only_its_scene_and_still_reaches_the_vanilla_endings() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Event/EventFlow").is_dir() {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let ops: std::collections::BTreeMap<String, Vec<crate::flow_extra::op::Op>> = always_ops().unwrap();
        for c in CUTS.iter() {
            let raw: Vec<u8> = rf.read_zs(&format!("Event/EventFlow/{}.zs", c.file)).unwrap();
            let was: Bf = Bf::parse(&raw).unwrap();
            let now: Bf = edit_flow(&raw, &[], &ops[c.file]).unwrap();
            assert_eq!(Bf::parse(&now.write().unwrap()).unwrap(), now, "{}", c.file);
            assert_eq!(now.flows.len(), was.flows.len());
            let (a, b): (&Flow, &Flow) = (&was.flows[0], &now.flows[0]);
            assert_eq!(a.events.len(), b.events.len(), "{}: no event added", c.file);
            assert_eq!(a.entries, b.entries, "{}: entries unchanged", c.file);
            assert_eq!(a.actors, b.actors, "{}: actors unchanged", c.file);
            let changed: Vec<usize> = (0..a.events.len()).filter(|i| a.events[*i] != b.events[*i]).collect();
            assert_eq!(changed, vec![c.start], "{}", c.file);
            let (before, before_ends): (BS<usize>, BS<usize>) = reach(a, c.start);
            let (after, after_ends): (BS<usize>, BS<usize>) = reach(b, c.start);
            for g in c.gone {
                assert!(before.contains(g), "{}: event {g} was not reachable in the game's flow", c.file);
                assert!(!after.contains(g), "{}: event {g} is still reachable", c.file);
            }
            assert!(after.is_subset(&before), "{}: the edit made new events reachable", c.file);
            assert!(!after_ends.is_empty() && after_ends.is_subset(&before_ends), "{}: ends {after_ends:?} of {before_ends:?}", c.file);
        }
    }

    #[test]
    fn the_cut_scenes_leave_each_grant_and_item_text_in_place() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.join("Event/EventFlow").is_dir() {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let ops: std::collections::BTreeMap<String, Vec<crate::flow_extra::op::Op>> = always_ops().unwrap();
        for (c, grant) in CUTS.iter().zip([63usize, 316, 149, 223, 262]) {
            let raw: Vec<u8> = rf.read_zs(&format!("Event/EventFlow/{}.zs", c.file)).unwrap();
            let now: Bf = edit_flow(&raw, &[], &ops[c.file]).unwrap();
            let f: &Flow = &now.flows[0];
            let reaches_grant: bool = f.entries.iter().any(|e| e.main != NONE && reach(f, usize::from(e.main)).0.contains(&grant));
            assert!(reaches_grant, "{}: grant event {grant} stays reachable", c.file);
        }
    }
}
