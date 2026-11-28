use std::collections::{BTreeSet as BS, HashMap as HM};
use std::path::Path as P;

use crate::bfevfl::{Flow, Kind};
use crate::error::{Error as E, Res};
use crate::flow_extra::sim::Over;
use crate::flow_sim::load::{Disk, Src};
use crate::romfs::Romfs;

pub fn missing(src: &mut dyn Src, flows: &HM<String, Flow>) -> Res<Vec<String>> {
    let mut have: HM<String, Option<BS<String>>> = HM::new();
    let mut names: Vec<&String> = flows.keys().collect();
    names.sort();
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let f: &Flow = &flows[n];
        for (i, e) in f.events.iter().enumerate() {
            let (fl, en): (&str, &str) = match &e.kind {
                Kind::Sub { flow, entry, .. } | Kind::SubSwitch { flow, entry, .. } => (flow.as_str(), entry.as_str()),
                _ => continue,
            };
            let to: &str = if fl.is_empty() { n.as_str() } else { fl };
            if !have.contains_key(to) {
                let got: Option<BS<String>> = match flows.get(to) {
                    Some(t) => Some(t.entries.iter().map(|x| x.name.clone()).collect()),
                    None => src.flow(to)?.map(|t| t.entries.iter().map(|x| x.name.clone()).collect()),
                };
                have.insert(to.to_string(), got);
            }
            if let Some(Some(set)) = have.get(to)
                && !set.contains(en)
            {
                out.push(format!("{n} event {i} ({}) calls {to}:{en}, which {to} does not have", e.name));
            }
        }
    }
    Ok(out)
}

pub fn check(rf: &mut Romfs, root: &P) -> Res<()> {
    let flows: HM<String, Flow> = crate::flow_extra::analyzed::flows_in(rf, root)?;
    let mut src: Over = Over { disk: Disk::new(Romfs::open(&rf.root)?, None, Vec::new()), flows: HM::new() };
    let bad: Vec<String> = missing(&mut src, &flows)?;
    if bad.is_empty() {
        return Ok(());
    }
    Err(E::Msg(format!("build: {} sub-flow calls name an entry their flow does not have:\n  {}", bad.len(), bad.join("\n  "))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfevfl::{Bf, Entry};

    fn game() -> Option<Romfs> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if !p.is_dir() {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", p.display());
            return None;
        }
        Some(Romfs::open(&p).unwrap())
    }

    fn load(rf: &mut Romfs, stem: &str) -> Option<Flow> {
        let raw: Vec<u8> = rf.read_zs(&format!("Event/EventFlow/{stem}.bfevfl.zs")).unwrap();
        let mut b: Bf = Bf::parse(&raw).unwrap();
        let i: usize = b.flows.iter().position(|f| f.name == stem).unwrap_or(0);
        if i < b.flows.len() { Some(b.flows.swap_remove(i)) } else { None }
    }

    #[test]
    fn no_flow_of_the_game_calls_a_missing_entry() {
        let Some(mut rf) = game() else { return };
        let dir: std::path::PathBuf = rf.root.join("Event/EventFlow");
        let mut flows: HM<String, Flow> = HM::new();
        for e in std::fs::read_dir(&dir).unwrap() {
            let p: std::path::PathBuf = e.unwrap().path();
            let Some(n) = p.file_name().and_then(|s| s.to_str()).and_then(|s| s.strip_suffix(".bfevfl.zs")) else { continue };
            if n.contains('.') {
                continue;
            }
            let n: String = n.to_string();
            if let Some(f) = load(&mut rf, &n) {
                flows.insert(n, f);
            }
        }
        assert!(flows.len() > 100, "{} flows", flows.len());
        let mut src: Over = Over { disk: Disk::new(Romfs::open(&rf.root).unwrap(), None, Vec::new()), flows: HM::new() };
        let bad: Vec<String> = missing(&mut src, &flows).unwrap();
        eprintln!("game flows: {}, calls into a missing entry: {}", flows.len(), bad.len());
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[test]
    fn a_call_into_an_entry_nobody_made_is_named() {
        let Some(mut rf) = game() else { return };
        let mut f: Flow = load(&mut rf, "DoorOfTime").unwrap();
        let at: usize = f.events.len();
        f.events.push(crate::bfevfl::Event {
            name: "OOTRRProbe".to_string(),
            kind: Kind::Sub { next: crate::bfevfl::flow::NONE, params: None, flow: "Dm_SY_GetInEvent".to_string(), entry: crate::item_window::key_name("GetInEvent_NotTurn_NotNpc") },
        });
        let mut flows: HM<String, Flow> = HM::new();
        flows.insert("DoorOfTime".to_string(), f.clone());
        let mut src: Over = Over { disk: Disk::new(Romfs::open(&rf.root).unwrap(), None, Vec::new()), flows: HM::new() };
        let bad: Vec<String> = missing(&mut src, &flows).unwrap();
        assert_eq!(bad, vec![format!("DoorOfTime event {at} (OOTRRProbe) calls Dm_SY_GetInEvent:OOTRR_K_GetInEvent_NotTurn_NotNpc, which Dm_SY_GetInEvent does not have")]);
        f.events[at].kind = Kind::Sub { next: crate::bfevfl::flow::NONE, params: None, flow: String::new(), entry: "NoSuchEntry".to_string() };
        let have: Entry = f.entries[0].clone();
        flows.insert("DoorOfTime".to_string(), f.clone());
        assert_eq!(missing(&mut src, &flows).unwrap().len(), 1);
        f.events[at].kind = Kind::Sub { next: crate::bfevfl::flow::NONE, params: None, flow: String::new(), entry: have.name.clone() };
        flows.insert("DoorOfTime".to_string(), f);
        assert!(missing(&mut src, &flows).unwrap().is_empty());
    }
}
