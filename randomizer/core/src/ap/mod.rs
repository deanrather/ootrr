use std::path::Path as P;

use crate::ainb::Ainb;
use crate::bfevfl::Bf;
use crate::byml::Byml;
use crate::error::{Error, Res};
use crate::flow_extra::analyzed::{Need, Reach};
use crate::flow_locations::dict_id;
use crate::flow_sim::load::Disk;
use crate::progressive::{ANALYZED_TABLE, SETTING_TABLE};
use crate::romfs::Romfs;
use crate::staging::{edit_staged, rewrite, Out};
use crate::zs;

pub mod mailbox;
pub mod markers;
pub mod saved;
pub mod watch;

use mailbox::Record;

pub fn edit_zs(rf: &mut Romfs, outs: &mut Vec<Out>, path: &str, f: impl FnOnce(&[u8]) -> Res<Vec<u8>>) -> Res<()> {
    match outs.iter().position(|o| o.path == path) {
        Some(i) => {
            let id: Option<u32> = dict_id(&outs[i].z);
            let raw: Vec<u8> = rf.zs.decompress(&outs[i].z)?;
            let out: Vec<u8> = f(&raw)?;
            outs[i].new = out.len();
            outs[i].z = zs::compress_raw(&out, id)?;
        }
        None => outs.push(rewrite(rf, path, f)?),
    }
    Ok(())
}

fn flow_edit(raw: &[u8], what: &str, f: impl FnOnce(&mut Bf) -> Res<()>) -> Res<Vec<u8>> {
    let mut b: Bf = Bf::parse(raw)?;
    f(&mut b)?;
    let out: Vec<u8> = b.write()?;
    if Bf::parse(&out)? != b {
        return Err(Error::Msg(format!("{what}: the patched flow does not parse back to the edited model")));
    }
    Ok(out)
}

fn table_edit(raw: &[u8], f: impl FnOnce(&mut Byml) -> Res<()>) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    f(&mut y)?;
    y.write()
}

pub fn give_need(rf: &Romfs) -> Res<Need> {
    let mut src: Disk = Disk::new(Romfs::open(&rf.root)?, None, Vec::new());
    let mut r: Reach = Reach::new(&mut src);
    r.need(mailbox::GIVE_FLOW, mailbox::GIVE_ENTRY)
}

pub fn stage(rf: &mut Romfs, outs: &mut Vec<Out>, work: &P, seed: &str, recs: &[Record]) -> Res<()> {
    let seed_flag: String = markers::seed_flag(seed);
    let goal_flag: String = markers::goal_flag(seed);
    let mut names: Vec<String> = vec![seed_flag.clone(), goal_flag.clone()];
    names.extend(mailbox::flags(recs));
    edit_zs(rf, outs, crate::chest_slot::LIST, |raw| table_edit(raw, |y| saved::add_bools(y, &names).map(|_| ())))?;
    for (h, flag) in [(markers::SEED, &seed_flag), (markers::GOAL, &goal_flag)] {
        edit_zs(rf, outs, h.file, |raw| flow_edit(raw, "markers", |b| markers::patch(b, h, flag).map(|_| ())))?;
    }
    if recs.is_empty() {
        return Ok(());
    }
    let need: Need = give_need(rf)?;
    edit_zs(rf, outs, mailbox::GIVE_FILE, |raw| flow_edit(raw, "mailbox", |b| mailbox::add_entries(b, recs).map(|_| ())))?;
    edit_zs(rf, outs, SETTING_TABLE, |raw| table_edit(raw, |y| mailbox::add_settings(y, recs).map(|_| ())))?;
    edit_zs(rf, outs, ANALYZED_TABLE, |raw| table_edit(raw, |y| mailbox::add_analyzed(y, recs, &need).map(|_| ())))?;
    edit_staged(rf, outs, work, watch::MAIN, false, |raw| {
        let mut a: Ainb = Ainb::parse(raw)?;
        watch::add_watchers(&mut a, recs)?;
        let bad: Vec<String> = crate::ainb::audit(&a);
        if !bad.is_empty() {
            return Err(Error::Msg(format!("watchers: the edited MainUpdate fails its audit: {}", bad.join("; "))));
        }
        a.write()
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfevfl::{Flow, Kind, Val};
    use crate::byml::Node;

    #[test]
    fn one_record_touches_every_file_and_each_reads_back_whole() {
        let p = ootrr_game::test_romfs::path();
        if !p.join("Pack").join("ZsDic.pack.zs").is_file() {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            return;
        }
        let mut rf: Romfs = Romfs::open(&p).unwrap();
        let mut outs: Vec<Out> = Vec::new();
        let r: Record = Record::new("00000000a11e", "Item_Arrow_Fire").unwrap();
        stage(&mut rf, &mut outs, P::new("nowhere-on-purpose"), "testseed", std::slice::from_ref(&r)).unwrap();
        let paths: Vec<&str> = outs.iter().map(|o| o.path.as_str()).collect();
        for want in [crate::chest_slot::LIST, markers::SEED.file, markers::GOAL.file, mailbox::GIVE_FILE, SETTING_TABLE, ANALYZED_TABLE, watch::MAIN] {
            assert!(paths.contains(&want), "{want} missing from {paths:?}");
        }
        let mut get = |path: &str| -> Vec<u8> {
            let o: &Out = outs.iter().find(|o| o.path == path).unwrap();
            if path.ends_with(".zs") { rf.zs.decompress(&o.z).unwrap() } else { o.z.clone() }
        };
        let list: Byml = Byml::parse(&get(crate::chest_slot::LIST)).unwrap();
        let l = crate::flags::FlagList::from_byml(&list).unwrap();
        for f in [r.recv(), r.got(), markers::seed_flag("testseed"), markers::goal_flag("testseed")] {
            assert!(l.by_hash(saved::hash(&f) as u64).is_some(), "{f}");
        }
        let b: Bf = Bf::parse(&get(mailbox::GIVE_FILE)).unwrap();
        let f: &Flow = b.flows.iter().find(|f| f.name == mailbox::GIVE_FLOW).unwrap();
        let e = f.entries.iter().find(|e| e.name == r.event()).unwrap();
        let Kind::Sub { next, params, flow, entry } = &f.events[e.main as usize].kind else { panic!("the entry does not start with the give") };
        assert_eq!((flow.as_str(), entry.as_str()), ("", mailbox::GIVE_ENTRY));
        assert_eq!(params.as_ref().unwrap().get("ActorName"), Some(&Val::Str("Item_Arrow_Fire".to_string())));
        let Kind::Action { params: got, .. } = &f.events[*next as usize].kind else { panic!("no Got action after the give") };
        assert_eq!(got.as_ref().unwrap().get("GameDataName"), Some(&Val::Str(r.got())));
        let s: Byml = Byml::parse(&get(SETTING_TABLE)).unwrap();
        let row: &Node = s.root.as_arr().unwrap().iter().find(|x| x.get("__RowId").and_then(|v| v.as_str()) == Some(&r.event())).unwrap();
        assert_eq!(row.get("FlowchartFileName").and_then(|v| v.as_str()), Some(mailbox::FLOWCHART));
        let an: Byml = Byml::parse(&get(ANALYZED_TABLE)).unwrap();
        let row: &Node = an.root.as_arr().unwrap().iter().find(|x| x.get("__RowId").and_then(|v| v.as_str()) == Some(&r.event())).unwrap();
        let subs: Vec<&str> = row.get("SubFlowList").unwrap().as_arr().unwrap().iter().filter_map(|x| x.as_str()).collect();
        assert!(subs.contains(&"Dm_SY_Get") || subs.contains(&"GetPouchContent"), "{subs:?}");
        let a: Ainb = Ainb::parse(&get(watch::MAIN)).unwrap();
        assert!(crate::ainb::audit(&a).is_empty());
    }
}
