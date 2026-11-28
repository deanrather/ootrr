use std::fs;
use std::path::{Path as P, PathBuf as PB};

use crate::bfevfl::{Bf, Cont, Event, Flow, Kind, Val};
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::giver_test::LINE;
use crate::modpack::{Host, Pack};
use crate::pickup_pack::write::start;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst, TABLE_PATH};
use crate::settings::seed::SeedFile;
use crate::slice::data::Data;
use crate::slice::edit::Built;
use crate::staging::MARGIN;
use crate::zs;
use crate::TITLE_ID;

pub const NPC: &str = "Npc_Kokiri_012_Child";
pub const PATH: &str = "Event/EventFlow/Npc_Kokiri_012_Child.bfevfl.zs";
pub const ENTRY: &str = "Talk";
pub const AFTER: &str = "Event0";
pub const GIVE: (&str, &str) = ("Dm_SY_GetInEvent", "GetInEvent_Goron");
pub const EVENT: &str = "OOTRR_GiverTest_Give";
pub const PARAM: &str = "ActorName";

pub fn actor_of(d: &Data, item: &str) -> Res<(String, Option<&'static str>)> {
    let i: &crate::slice::data::Item = d.item(item).ok_or_else(|| E::Msg(format!("{LINE}: {item} is not an item")))?;
    if let Some(f) = crate::progressive::family_of(&i.tiers) {
        let first: &str = i.tiers.first().ok_or_else(|| E::Msg(format!("{LINE}: {item} has no tiers")))?;
        return Ok((first.to_string(), Some(f)));
    }
    let a: String = i.actor.clone().ok_or_else(|| E::Msg(format!("{LINE}: {item} has no actor to hand out")))?;
    Ok((a, None))
}

pub fn reaches(f: &Flow, from: u16, to: usize) -> bool {
    let mut seen: Vec<bool> = vec![false; f.events.len()];
    let mut todo: Vec<u16> = vec![from];
    while let Some(i) = todo.pop() {
        let Some(e) = f.events.get(usize::from(i)) else {
            continue;
        };
        if seen[usize::from(i)] {
            continue;
        }
        seen[usize::from(i)] = true;
        if usize::from(i) == to {
            return true;
        }
        match &e.kind {
            Kind::Action { next, .. } | Kind::Sub { next, .. } | Kind::Join { next } => todo.push(*next),
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => todo.extend(cases.iter().map(|c| c.ev)),
            Kind::Fork { join, forks } => todo.extend(forks.iter().copied().chain([*join])),
            Kind::Label { .. } => {}
        }
    }
    false
}

pub fn edit(f: &mut Flow, actor: &str, family: Option<&str>) -> Res<u16> {
    if f.events.iter().any(|e| e.name.starts_with(EVENT)) {
        return Err(E::Msg(format!("{LINE}: {} already holds the story give", f.name)));
    }
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| f.events[i].name == AFTER).collect();
    let [greet] = hits.as_slice() else {
        return Err(E::Msg(format!("{LINE}: {} has {} events named {AFTER}, the game files differ", f.name, hits.len())));
    };
    let greet: usize = *greet;
    let then: u16 = match &f.events[greet].kind {
        Kind::Sub { next, flow, entry, .. } if flow == "InitTalk" && entry == "InitTalk_KokiriOnly" => *next,
        _ => return Err(E::Msg(format!("{LINE}: {} {AFTER} is not the Kokiri greeting, the game files differ", f.name))),
    };
    let callers: Vec<usize> = (0..f.entries.len()).filter(|&k| reaches(f, f.entries[k].main, greet)).collect();
    if !callers.iter().any(|&k| f.entries[k].name == ENTRY) {
        return Err(E::Msg(format!("{LINE}: the {ENTRY} entry of {} does not reach {AFTER}, the game files differ", f.name)));
    }
    let at: u16 = u16::try_from(f.events.len()).map_err(|_| E::Format("giver test: the flow has too many events"))?;
    let c: Cont = Cont { items: vec![(PARAM.to_string(), Val::Str(actor.to_string()))] };
    f.events.push(Event { name: EVENT.to_string(), kind: Kind::Sub { next: then, params: Some(c), flow: GIVE.0.to_string(), entry: GIVE.1.to_string() } });
    if let Kind::Sub { next, .. } = &mut f.events[greet].kind {
        *next = at;
    }
    for k in callers {
        f.entries[k].subs.push(at);
    }
    if let Some(fam) = family {
        crate::progressive::wrap_grant_as(f, usize::from(at), EVENT, PARAM, fam, &crate::ocarina_hold::forms_of(fam))?;
    }
    Ok(at)
}

fn local(base: &P, path: &str) -> PB {
    path.split('/').fold(base.to_path_buf(), |a, s| a.join(s))
}

pub fn after(seed: &SeedFile, romfs: &P, load: &P, mod_name: &str, host: Host, mut b: Built) -> Res<Built> {
    let Some(m) = &seed.giver_test else {
        return Ok(b);
    };
    b.report.push_str(&format!("{}\n", crate::giver_test::NOT_BEATABLE));
    let d: Data = Data::load()?;
    let (actor, family): (String, Option<&str>) = actor_of(&d, &m.item)?;
    if family.is_none() && crate::ocarina_hold::shown(&actor).is_some() {
        b.report.push_str(&format!("{LINE}: {} has no get animation, so the story give is left out\n", m.item));
        return Ok(b);
    }
    let mut rf: Romfs = Romfs::open(romfs)?;
    let root: PB = b.root.join("romfs");
    let own: PB = root.join(TABLE_PATH);
    let mut rst: Rst = if own.is_file() { Rst::parse(&rf.zs.decompress(&fs::read(&own)?)?)? } else { rsizetable::load(&mut rf)? };
    let pk: Pack = Pack::create_for(load, TITLE_ID, mod_name, host)?;
    if pk.root != b.root {
        return Err(E::Msg(format!("{LINE}: the story give went to {} and not to the seed mod {}", pk.root.display(), b.root.display())));
    }
    let modded: bool = local(&root, PATH).is_file();
    let first: Vec<u8> = start(&mut rf, Some(&root), PATH)?;
    let mut bf: Bf = Bf::parse(&first)?;
    let f: &mut Flow = bf.flows.iter_mut().find(|f| f.name == NPC).ok_or_else(|| E::Msg(format!("{LINE}: {PATH} has no flow {NPC}")))?;
    edit(f, &actor, family)?;
    let raw: Vec<u8> = bf.write()?;
    let old: usize = if modded { first.len() } else { rf.read_zs(PATH)?.len() };
    pk.write_romfs(PATH, &zs::compress_raw(&raw, dict_id(&rf.read(PATH)?))?)?;
    if raw.len() > old {
        rst.grow(PATH, old as u64, raw.len() as u64, MARGIN)?;
    }
    b.counts.files += usize::from(!modded);
    if let Some((o, _)) = crate::flow_extra::analyzed::worked(&mut rf, &root)? {
        pk.write_romfs(&o.path, &o.z)?;
        if o.new > o.old {
            rst.grow(&o.path, o.old as u64, o.new as u64, MARGIN)?;
        }
    }
    pk.write_romfs(TABLE_PATH, &rst.to_zs()?)?;
    b.report.push_str(&format!("{LINE}: story give: the Kokiri {NPC} near Link's house calls {}:{} with {actor} after the greeting on every talk\n", GIVE.0, GIVE.1));
    Ok(b)
}
