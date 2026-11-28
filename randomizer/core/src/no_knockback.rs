use crate::ainb::Ainb;
use crate::error::{Error, Res};
use crate::sarc::{self, Entry};

pub const SMALL: &str = "AI/Player.scheduler.reactionsmallhit.module.ainb";
pub const MEDIUM: &str = "AI/Player.scheduler.reactionmediumhit.module.ainb";
pub const HUGE: &str = "AI/Player.scheduler.reactionhugehit.module.ainb";
pub const FILES: [(&str, i32); 3] = [(SMALL, 134), (MEDIUM, 135), (HUGE, 136)];
pub const SETTERS: [&str; 2] = ["ExecuteBSASetActionVerb", "ExecuteBSASetActionVerbAndForceChangeByPlug"];
pub const INPUT: &str = "ActionVerb";
pub const WAIT_VERB: i32 = 40;
pub const PACKS: [&str; 3] = ["Pack/Actor/Player_Adult.pack.zs", "Pack/Actor/Player_Base.pack.zs", "Pack/Actor/Player_Child.pack.zs"];
pub const FLAG: &str = "--no-knockback";
pub const NOTE: &str = "Test build: a hit on Link plays no reaction, so he is not knocked back or staggered. Damage is unchanged.";

pub fn from_flag(a: &str) -> Res<bool> {
    if a == FLAG {
        return Ok(true);
    }
    if a.starts_with("--no-knockback=") {
        return Err(Error::Msg(format!("{FLAG} takes no value")));
    }
    Ok(false)
}

pub fn patch_graph(raw: &[u8], file: &str, from: i32, to: i32) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    let mut hits: Vec<usize> = Vec::new();
    for s in SETTERS {
        for i in a.find_els(s) {
            if a.in_s32(i, INPUT) == Some(from) {
                hits.push(i);
            }
        }
    }
    let [el] = hits.as_slice() else {
        return Err(Error::Msg(format!("no knockback: {file} has {} verb setters with {INPUT} {from}, expected one", hits.len())));
    };
    let g: usize = a.find_in(*el, 0, INPUT).ok_or(Error::Format("no knockback: the verb setter lost its input"))?;
    if a.input(0, g).is_none_or(|x| x.src != -1) {
        return Err(Error::Msg(format!("no knockback: {file} {INPUT} is not a constant")));
    }
    a.set_in_val(0, g, &to.to_le_bytes())?;
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if out.len() != raw.len() || back.in_s32(*el, INPUT) != Some(to) {
        return Err(Error::Msg(format!("no knockback: {file} does not read back")));
    }
    Ok(out)
}

pub fn patch_pack(raw: &[u8]) -> Res<Vec<u8>> {
    let mut es: Vec<Entry> = sarc::parse(raw)?;
    for (file, from) in FILES {
        let g: Vec<u8> = sarc::get(&es, file).ok_or(Error::Msg(format!("no knockback: the player pack has no {file}")))?.data.clone();
        sarc::set(&mut es, file, patch_graph(&g, file, from, WAIT_VERB)?);
    }
    sarc::write(&es)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::romfs::Romfs;
    use std::path::PathBuf;

    fn dump() -> Option<PathBuf> {
        let p: PathBuf = ootrr_game::test_romfs::path();
        if p.is_dir() {
            Some(p)
        } else {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
            None
        }
    }

    fn count(x: &Ainb, v: i32) -> usize {
        SETTERS.iter().flat_map(|s| x.find_els(s)).filter(|&i| x.in_s32(i, INPUT) == Some(v)).count()
    }

    #[test]
    fn the_flag_takes_no_value() {
        assert!(!from_flag("--generate").unwrap());
        assert!(from_flag("--no-knockback").unwrap());
        assert!(from_flag("--no-knockback=still").is_err());
        assert!(from_flag("--no-knockback=flinch").is_err());
        assert!(from_flag("--no-knockback=").is_err());
        assert!(!from_flag("--no-knockbackx").unwrap());
    }

    #[test]
    fn a_graph_changes_only_the_verb_value_in_place() {
        let Some(root) = dump() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        for pack in PACKS {
            let es: Vec<Entry> = sarc::parse(&rf.read_zs(pack).unwrap()).unwrap();
            for (file, from) in FILES {
                let raw: &[u8] = &sarc::get(&es, file).unwrap().data;
                let a: Ainb = Ainb::parse(raw).unwrap();
                assert_eq!(a.write().unwrap(), raw, "{file} writes back unchanged");
                let out: Vec<u8> = patch_graph(raw, file, from, WAIT_VERB).unwrap();
                assert_eq!(out.len(), raw.len());
                let diff: Vec<usize> = (0..raw.len()).filter(|&i| raw[i] != out[i]).collect();
                assert!(!diff.is_empty() && diff.len() <= 4, "{file}: {diff:?}");
                assert!(diff.windows(2).all(|w| w[1] - w[0] < 4), "{file} changes one word: {diff:?}");
                let b: Ainb = Ainb::parse(&out).unwrap();
                assert_eq!(count(&b, WAIT_VERB), count(&a, WAIT_VERB) + 1);
                assert_eq!(count(&b, from), 0);
                assert!(patch_graph(&out, file, from, WAIT_VERB).is_err(), "a second patch is refused");
                assert!(patch_graph(raw, file, from + 1, WAIT_VERB).is_err(), "an unexpected vanilla value is refused");
            }
        }
    }

    #[test]
    fn a_god_mode_build_carries_the_zero_damage_and_the_wait_verbs_in_every_player_pack() {
        let Some(root) = dump() else { return };
        let d: crate::slice::data::Data = crate::slice::data::Data::load().unwrap();
        let mut s: crate::settings::Settings = crate::settings::Settings::default();
        s.set(crate::difficulty::HURT, crate::settings::Value::s("god")).unwrap();
        let g: crate::slice::generate::Generated = crate::slice::generate::generate_data(&d, &s, "god build", &crate::slice::generate::GenOpts::default()).unwrap();
        assert!(!g.seed_file.no_knockback);
        let load: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/god_mods").join(std::process::id().to_string());
        let b: crate::slice::edit::Built = crate::slice::edit::build_for(&g.seed_file, &root, &load, "ootrr-god", crate::modpack::Host::Eden).unwrap();
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        for pack in PACKS {
            let z: Vec<u8> = std::fs::read(pack.split('/').fold(b.root.join("romfs"), |a, p| a.join(p))).unwrap();
            let es: Vec<Entry> = sarc::parse(&rf.zs.decompress(&z).unwrap()).unwrap();
            for (file, from) in FILES {
                let a: Ainb = Ainb::parse(&sarc::get(&es, file).unwrap().data).unwrap();
                assert_eq!(count(&a, from), 0, "{pack} {file} still plays the knockback verb");
                assert!(count(&a, WAIT_VERB) > 0, "{pack} {file}");
            }
            let old: Vec<Entry> = sarc::parse(&rf.read_zs(pack).unwrap()).unwrap();
            let was: &[u8] = &sarc::get(&old, crate::difficulty::hurt::FILE).unwrap().data;
            let zero: Vec<u8> = crate::difficulty::hurt::edit(was, crate::difficulty::hurt::Hurt::God).unwrap().unwrap();
            assert_eq!(sarc::get(&es, crate::difficulty::hurt::FILE).unwrap().data, zero, "{pack} carries the God mode damage file");
        }
    }

    #[test]
    fn every_pack_holding_the_graphs_is_patched_alike() {
        let Some(root) = dump() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let mut holders: Vec<String> = Vec::new();
        for p in rf.list("Pack").unwrap() {
            if !p.ends_with(".pack.zs") || p.contains("ZsDic") {
                continue;
            }
            let es: Vec<Entry> = match rf.read_zs(&p).and_then(|b| sarc::parse(&b)) {
                Ok(e) => e,
                Err(_) => continue,
            };
            if FILES.iter().any(|(f, _)| sarc::get(&es, f).is_some()) {
                holders.push(p);
            }
        }
        assert_eq!(holders, PACKS.iter().map(|p| p.to_string()).collect::<Vec<String>>(), "every pack with a copy is in PACKS");
        let mut raws: Vec<Vec<u8>> = Vec::new();
        let mut vanilla: Vec<Vec<u8>> = Vec::new();
        for p in PACKS {
            let old: Vec<Entry> = sarc::parse(&rf.read_zs(p).unwrap()).unwrap();
            let es: Vec<Entry> = sarc::parse(&patch_pack(&rf.read_zs(p).unwrap()).unwrap()).unwrap();
            assert_eq!(es.len(), old.len());
            for e in &es {
                let was: &Entry = sarc::get(&old, &e.name).unwrap();
                let changed: bool = FILES.iter().any(|(f, _)| *f == e.name);
                assert_eq!(e.data != was.data, changed, "{} {}", p, e.name);
                assert_eq!(e.data.len(), was.data.len());
            }
            raws.push(FILES.iter().flat_map(|(f, _)| sarc::get(&es, f).unwrap().data.clone()).collect());
            vanilla.push(FILES.iter().flat_map(|(f, _)| sarc::get(&old, f).unwrap().data.clone()).collect());
        }
        assert!(vanilla.windows(2).all(|w| w[0] == w[1]), "the vanilla copies differ");
        assert!(raws.windows(2).all(|w| w[0] == w[1]), "copies differ after patching");
    }
}
