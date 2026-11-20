use crate::ainb::{audit, Ainb};
use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::qol::Have;
use crate::romfs::Romfs;
use crate::sarc::{self, Entry};
use crate::staging::Out;
use crate::zs;

pub const BOOL: usize = 1;
pub const GLOBAL: &str = "Pack/AI.Global.Product.111.pack.zs";
pub const SMALL: &str = "Pack/Actor/Enemy_Leever.pack.zs";
pub const FIXED: &str = "Pack/Actor/Enemy_Leever_Fixed.pack.zs";
pub const SET_VERB: &str = "ExecuteBSASetActionVerb";
pub const DISAPPEAR_VERB: i32 = 96;
pub const PARAM: &str = "GameParameter/LeeverParam/Enemy_Leever.game__enemy__LeeverParam.bgyml";
pub const WAIT: &str = "PreparePopupTimeRand";
pub const SAND_QUERY: &str = "QueryPhysicsIsMaterialContactToSphere";
pub const GLOBAL_GRAPHS: [&str; 4] = ["AI/Leever.action.Run.module.ainb", "AI/Leever.action.Disappear.module.ainb", "AI/Leever.action.InterruptRepelled.module.ainb", "AI/Leever.action.InterruptSmallHit.module.ainb"];
pub const SMALL_GRAPHS: [&str; 1] = ["AI/Enemy_Leever_LandBattle.actionseq.root.ainb"];
pub const STAY_GRAPHS: [&str; 1] = SMALL_GRAPHS;
pub const NOTE: &str = "At Lon Lon Ranch the guards are Leevers: a giant one and five small ones, which come up out of the grass and fight until you beat them. The game's Leevers only come up on sand and dive away on their own, so their sand test and their dive away are cut out of the Leever graphs; this also stops the Leevers of the Haunted Wasteland sinking at the edge of the sand and diving away.";

pub fn unbury(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    let queries: Vec<usize> = a.find_els(SAND_QUERY);
    if queries.len() != 1 {
        return Err(Error::Format("leevers: the graph does not have exactly one sand test"));
    }
    let q: i16 = queries[0] as i16;
    let mut cut: Vec<(usize, String)> = Vec::new();
    for i in 0..a.els.len() {
        for g in a.input_ids(i, BOOL) {
            let Some(x) = a.input(BOOL, g) else {
                continue;
            };
            if x.src == q {
                cut.push((i, a.s(x.name).to_string()));
            }
        }
    }
    if cut.is_empty() {
        return Err(Error::Format("leevers: nothing reads the sand test"));
    }
    for (i, name) in &cut {
        a.force_bool(*i, name, false)?;
    }
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(Error::Msg(format!("leevers: {}", bad.join("; "))));
    }
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if back.write()? != out || back.params != a.params || back.ins != a.ins {
        return Err(Error::Format("leevers: the written graph does not read back"));
    }
    Ok(out)
}

pub fn stay(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    let verbs: Vec<usize> = a.find_els(SET_VERB).into_iter().filter(|i| a.in_s32(*i, "ActionVerb") == Some(DISAPPEAR_VERB)).collect();
    if verbs.len() != 1 {
        return Err(Error::Format("leevers: the graph does not have exactly one disappear verb"));
    }
    let mut cut: usize = 0;
    for i in 0..a.els.len() {
        if a.el_type(i) != 2 {
            continue;
        }
        for j in (0..a.params[i].group(2).len()).rev() {
            if a.params[i].group(2)[j].a as usize == verbs[0] {
                a.params[i].remove_plug(2, j)?;
                cut += 1;
            }
        }
    }
    if cut != 1 {
        return Err(Error::Format("leevers: the disappear verb is not the last step of exactly one sequence"));
    }
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(Error::Msg(format!("leevers: {}", bad.join("; "))));
    }
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if back.write()? != out || back.params != a.params || back.ins != a.ins {
        return Err(Error::Format("leevers: the written graph does not read back"));
    }
    Ok(out)
}

pub fn no_wait(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    if !matches!(y.root.get(WAIT), Some(Node::F32(_))) {
        return Err(Error::Format("leevers: the parameter file has no popup wait"));
    }
    y.root.set(WAIT, Node::F32(0.0));
    let out: Vec<u8> = y.write()?;
    if Byml::parse(&out)?.root != y.root {
        return Err(Error::Format("leevers: the parameter file does not read back"));
    }
    Ok(out)
}

pub fn patch(es: &mut [Entry], names: &[&str], wait: bool) -> Res<()> {
    for n in names.iter().copied().chain(wait.then_some(PARAM)) {
        let e: &mut Entry = es.iter_mut().find(|e| e.name == n).ok_or_else(|| Error::Msg(format!("leevers: the pack has no {n}, the game files differ")))?;
        e.data = if n == PARAM {
            no_wait(&e.data)?
        } else if STAY_GRAPHS.contains(&n) {
            stay(&unbury(&e.data)?)?
        } else {
            unbury(&e.data)?
        };
    }
    Ok(())
}

fn pack(rf: &mut Romfs, have: Have, path: &str, names: &[&str], wait: bool) -> Res<Out> {
    let z: Vec<u8> = match have(path) {
        Some(b) => b,
        None => rf.read(path)?,
    };
    let raw: Vec<u8> = rf.zs.decompress(&z)?;
    let mut es: Vec<Entry> = sarc::parse(&raw)?;
    patch(&mut es, names, wait)?;
    let out: Vec<u8> = sarc::write(&es)?;
    Ok(Out { path: path.to_string(), old: raw.len(), new: out.len(), z: zs::compress_raw(&out, zs::dict_of(&z))? })
}

pub fn stage(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    Ok(vec![pack(rf, have, GLOBAL, &GLOBAL_GRAPHS, false)?, pack(rf, have, SMALL, &SMALL_GRAPHS, true)?, pack(rf, have, FIXED, &SMALL_GRAPHS, true)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from)?;
        if p.is_dir() { Some(p) } else { None }
    }

    fn none(_p: &str) -> Option<Vec<u8>> {
        None
    }

    fn readers(a: &Ainb) -> usize {
        let q: i16 = a.find_els(SAND_QUERY)[0] as i16;
        (0..a.els.len()).map(|i| a.input_ids(i, BOOL).into_iter().filter(|g| a.input(BOOL, *g).is_some_and(|x| x.src == q)).count()).sum()
    }

    fn entry(rf: &mut Romfs, path: &str, name: &str) -> Vec<u8> {
        let es: Vec<Entry> = sarc::parse(&rf.read_zs(path).unwrap()).unwrap();
        sarc::get(&es, name).unwrap().data.clone()
    }

    #[test]
    fn a_graph_without_a_sand_test_is_refused() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let raw: Vec<u8> = entry(&mut rf, SMALL, "AI/Enemy_Leever.action.root.ainb");
        assert!(unbury(&raw).is_err());
    }

    #[test]
    fn nothing_reads_the_sand_test_after_the_edit_and_the_rest_of_each_graph_is_kept() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        for (pack, names) in [(GLOBAL, &GLOBAL_GRAPHS[..]), (SMALL, &SMALL_GRAPHS[..]), (FIXED, &SMALL_GRAPHS[..])] {
            for n in names {
                let old: Ainb = Ainb::parse(&entry(&mut rf, pack, n)).unwrap();
                assert!(readers(&old) > 0, "{n}");
                let new: Ainb = Ainb::parse(&unbury(&entry(&mut rf, pack, n)).unwrap()).unwrap();
                assert_eq!(readers(&new), 0, "{n}");
                assert!(audit(&new).is_empty(), "{n}");
                assert_eq!(new.els.len(), old.els.len(), "{n}");
                assert_eq!(new.find_els(SAND_QUERY).len(), 1, "{n}");
            }
        }
    }

    #[test]
    fn only_the_leever_graphs_change_in_each_pack() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let outs: Vec<Out> = stage(&mut rf, &none).unwrap();
        assert_eq!(outs.len(), 3);
        let plan: [(&[&str], bool); 3] = [(&GLOBAL_GRAPHS[..], false), (&SMALL_GRAPHS[..], true), (&SMALL_GRAPHS[..], true)];
        for (o, (names, wait)) in outs.iter().zip(plan) {
            let before: Vec<Entry> = sarc::parse(&rf.read_zs(&o.path).unwrap()).unwrap();
            let after: Vec<Entry> = sarc::parse(&rf.zs.decompress(&o.z).unwrap()).unwrap();
            assert_eq!(before.len(), after.len());
            for (b, a) in before.iter().zip(after.iter()) {
                assert_eq!(b.name, a.name);
                assert_eq!(b.data != a.data, names.contains(&b.name.as_str()) || (wait && b.name == PARAM), "{}", b.name);
            }
        }
    }

    #[test]
    fn the_edit_is_the_same_every_time() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let a: Vec<Out> = stage(&mut rf, &none).unwrap();
        let b: Vec<Out> = stage(&mut rf, &none).unwrap();
        assert!(a.iter().zip(b.iter()).all(|(x, y)| x.path == y.path && x.z == y.z));
    }

    #[test]
    fn the_popup_wait_of_every_leever_is_taken_away_and_nothing_else_in_the_file() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        for pack in [SMALL, FIXED] {
            let raw: Vec<u8> = entry(&mut rf, pack, PARAM);
            let old: Byml = Byml::parse(&raw).unwrap();
            assert_eq!(old.root.get(WAIT), Some(&Node::F32(60.0)));
            let new: Byml = Byml::parse(&no_wait(&raw).unwrap()).unwrap();
            assert_eq!(new.root.get(WAIT), Some(&Node::F32(0.0)));
            let (Node::Map(a), Node::Map(b)) = (&old.root, &new.root) else {
                panic!("not a map");
            };
            assert_eq!(a.len(), b.len());
            assert!(a.iter().zip(b.iter()).all(|(x, y)| x.0 == y.0 && (x.0 == WAIT || x.1 == y.1)));
        }
        assert!(no_wait(&entry(&mut rf, SMALL, "Actor/Enemy_Leever.engine__actor__ActorParam.bgyml")).is_err());
    }
}
