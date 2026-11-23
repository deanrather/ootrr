use std::fs;
use std::path::PathBuf as PB;

use crate::byml::{Byml, Node};
use crate::cow_clone::rows::heap;
use crate::error::{Error as E, Res};
use crate::flow_locations::{dict_id, path_of};
use crate::kaepora::{chain, flow_bytes, Link, BASE, OWL};
use crate::modpack::Pack;
use crate::pickup_pack::rows::{self, ACTOR_INFO, GAME_ACTOR_INFO, HEAP, ROW_ID};
use crate::pickup_pack::sizes::{file_size, pack_size};
use crate::pickup_pack::tag::{Table, TAG};
use crate::pickup_pack::PACK_DICT;
use crate::progressive::{ANALYZED_TABLE, SETTING_TABLE};
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst};
use crate::sarc::{self, Entry};
use crate::settings::seed::SeedFile;
use crate::slice::data::Data;
use crate::staging::{grow_out, Out};
use crate::zs;

pub fn pack_of(name: &str) -> String {
    format!("Pack/Actor/{name}.pack.zs")
}

pub fn actor_file(name: &str) -> String {
    format!("Actor/{name}.engine__actor__ActorParam.bgyml")
}

pub fn schedule_file(name: &str) -> String {
    format!("Component/AIScheduleParam/{name}.game__component__AIScheduleParam.bgyml")
}

pub fn flow_file(name: &str) -> String {
    path_of(&format!("{name}.bfevfl"))
}

pub fn sources() -> Vec<String> {
    vec![pack_of(BASE), schedule_file(BASE), flow_file(BASE), ACTOR_INFO.to_string(), GAME_ACTOR_INFO.to_string(), TAG.to_string(), SETTING_TABLE.to_string(), ANALYZED_TABLE.to_string()]
}

fn swap(n: &mut Node, from: &str, to: &str) {
    match n {
        Node::Str(s) if s.contains(from) => *s = s.replace(from, to),
        Node::Arr(v) => v.iter_mut().for_each(|x| swap(x, from, to)),
        Node::Map(v) => v.iter_mut().for_each(|(_, x)| swap(x, from, to)),
        _ => {}
    }
}

pub fn build(base: &[Entry]) -> Res<Vec<Entry>> {
    let old: String = actor_file(BASE);
    let at: usize = base.iter().position(|e| e.name == old).ok_or_else(|| E::Msg(format!("kaepora: the {BASE} pack has no {old}, the game files differ")))?;
    if base.iter().any(|e| e.name == actor_file(OWL)) {
        return Err(E::Msg(format!("kaepora: the {BASE} pack already holds {OWL}")));
    }
    let mut y: Byml = Byml::parse(&base[at].data)?;
    let want: String = format!("?{}", schedule_file(BASE));
    let c: &mut Node = y.root.get_mut("Components").ok_or(E::Format("kaepora: the owl actor file has no Components"))?;
    if c.get("AIScheduleRef").and_then(|v| v.as_str()) != Some(want.as_str()) {
        return Err(E::Format("kaepora: the owl actor file names another schedule, the game files differ"));
    }
    c.set("AIScheduleRef", Node::Str(format!("?{}", schedule_file(OWL))));
    let mut out: Vec<Entry> = base.to_vec();
    out.remove(at);
    out.push(Entry { name: actor_file(OWL), data: y.write()? });
    Ok(out)
}

pub fn schedule(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    swap(&mut y.root, BASE, OWL);
    if format!("{:?}", y.root).contains(BASE) {
        return Err(E::Format("kaepora: the schedule still names the Trail owl"));
    }
    y.write()
}

fn id(n: &Node) -> &str {
    n.get(ROW_ID).and_then(|v| v.as_str()).unwrap_or("")
}

pub fn event_rows(y: &mut Byml) -> Res<usize> {
    let prefix: String = format!("{BASE}_");
    let from: Vec<Node> = y.root.as_arr().ok_or(E::Format("kaepora: an event table is not a row list"))?.iter().filter(|r| id(r).starts_with(&prefix)).cloned().collect();
    if from.is_empty() {
        return Err(E::Msg(format!("kaepora: the event tables have no {BASE} rows, the game files differ")));
    }
    let rows: &mut Vec<Node> = y.root.as_arr_mut().ok_or(E::Format("kaepora: an event table is not a row list"))?;
    for mut r in from.iter().cloned() {
        swap(&mut r, BASE, OWL);
        let name: String = id(&r).to_string();
        if rows.iter().any(|x| id(x) == name) {
            return Err(E::Msg(format!("kaepora: the event table already holds {name}")));
        }
        let at: usize = rows.partition_point(|x| id(x) < name.as_str());
        rows.insert(at, r);
    }
    Ok(from.len())
}

pub fn actor_rows(info: &mut Byml, game: &mut Byml, base_len: usize, len: usize) -> Res<usize> {
    let mut a: Node = rows::need(info, BASE, ACTOR_INFO)?.clone();
    let h: i32 = heap(rows::own_heap(info, BASE)?, base_len, len)?;
    let v: Node = match a.get(HEAP) {
        Some(Node::U32(_)) => Node::U32(h.max(0) as u32),
        _ => Node::I32(h),
    };
    a.set(HEAP, v);
    a.set(ROW_ID, Node::Str(OWL.to_string()));
    let mut g: Node = rows::need(game, BASE, GAME_ACTOR_INFO)?.clone();
    g.set(ROW_ID, Node::Str(OWL.to_string()));
    if rows::find(info, OWL).is_some() || rows::find(game, OWL).is_some() {
        return Err(E::Msg(format!("kaepora: the actor tables already hold {OWL}")));
    }
    let i: usize = rows::insert(info, a)?;
    let j: usize = rows::insert(game, g)?;
    if i != j {
        return Err(E::Msg(format!("kaepora: {OWL} sits at {i} in the actor table and at {j} in the game actor table")));
    }
    Ok(i)
}

pub fn tags(t: &mut Table) -> Res<()> {
    let own: Vec<String> = t.tags_of(BASE);
    if own.is_empty() {
        return Err(E::Msg(format!("kaepora: {BASE} has no tag row, the game files differ")));
    }
    if t.find(OWL).is_some() {
        return Err(E::Msg(format!("kaepora: the tag table already holds {OWL}")));
    }
    t.insert(OWL, &own)?;
    Ok(())
}

pub fn flow(raw: &[u8], links: &[Link]) -> Res<Vec<u8>> {
    let out: Vec<u8> = flow_bytes(raw, links)?;
    let mut b: crate::bfevfl::Bf = crate::bfevfl::Bf::parse(&out)?;
    if b.name == BASE {
        b.name = OWL.to_string();
    }
    let f: &mut crate::bfevfl::Flow = b.flows.iter_mut().find(|f| f.name == BASE).ok_or(E::Format("kaepora: the owl flow is not in its file"))?;
    f.name = OWL.to_string();
    for a in f.actors.iter_mut().filter(|a| a.name == BASE && a.sub.is_empty()) {
        a.name = OWL.to_string();
    }
    let out: Vec<u8> = b.write()?;
    if crate::bfevfl::Bf::parse(&out)? != b {
        return Err(E::Format("kaepora: the copied owl flow does not read back"));
    }
    Ok(out)
}

#[derive(Clone, Debug, Default)]
pub struct Made {
    pub outs: Vec<Out>,
    pub sizes: Vec<(String, u32)>,
}

fn current(rf: &mut Romfs, have: &dyn Fn(&str) -> Option<Vec<u8>>, p: &str) -> Res<(Vec<u8>, u32)> {
    let z: Vec<u8> = match have(p) {
        Some(z) => z,
        None => rf.read(p)?,
    };
    let id: u32 = dict_id(&z).or_else(|| dict_id(&rf.read(p).unwrap_or_default())).unwrap_or(crate::staging::BANC_DICT);
    Ok((rf.zs.decompress(&z)?, id))
}

pub fn stage(rf: &mut Romfs, have: &dyn Fn(&str) -> Option<Vec<u8>>, rst: &Rst, links: &[Link]) -> Res<Made> {
    let mut m: Made = Made::default();
    if rf.root.join(pack_of(OWL)).is_file() {
        return Err(E::Msg(format!("kaepora: the game already ships {OWL}")));
    }
    let bz: Vec<u8> = rf.read(&pack_of(BASE))?;
    let braw: Vec<u8> = rf.zs.decompress(&bz)?;
    let base: Vec<Entry> = sarc::parse(&braw)?;
    let es: Vec<Entry> = build(&base)?;
    let raw: Vec<u8> = sarc::write(&es)?;
    let near: Vec<(String, usize)> = base.iter().map(|e| (e.name.clone(), e.data.len())).collect();
    let own: String = actor_file(OWL);
    let own_len: usize = es.iter().find(|e| e.name == own).map(|e| e.data.len()).unwrap_or(0);
    m.sizes.push((own.clone(), file_size(rst, &near, &own, own_len)?));
    m.sizes.push((pack_of(OWL), pack_size(raw.len())?));
    m.outs.push(Out { path: pack_of(OWL), z: zs::compress_raw(&raw, Some(dict_id(&bz).unwrap_or(PACK_DICT)))?, old: 0, new: raw.len() });
    let sraw: Vec<u8> = rf.read(&schedule_file(BASE))?;
    let sch: Vec<u8> = schedule(&sraw)?;
    let sp: String = schedule_file(OWL);
    m.sizes.push((sp.clone(), file_size(rst, &[(schedule_file(BASE), sraw.len())], &sp, sch.len())?));
    m.outs.push(Out { path: sp, new: sch.len(), z: sch, old: 0 });
    let (fraw, fid): (Vec<u8>, u32) = current(rf, have, &flow_file(BASE))?;
    let fl: Vec<u8> = flow(&fraw, links)?;
    let fp: String = flow_file(OWL);
    m.sizes.push((fp.clone(), file_size(rst, &[(flow_file(BASE), fraw.len())], &fp, fl.len())?));
    m.outs.push(Out { path: fp, new: fl.len(), z: zs::compress_raw(&fl, Some(fid))?, old: 0 });
    let (iraw, iid): (Vec<u8>, u32) = current(rf, have, ACTOR_INFO)?;
    let (graw, gid): (Vec<u8>, u32) = current(rf, have, GAME_ACTOR_INFO)?;
    let (traw, tid): (Vec<u8>, u32) = current(rf, have, TAG)?;
    let mut info: Byml = Byml::parse(&iraw)?;
    let mut game: Byml = Byml::parse(&graw)?;
    let mut tag: Table = Table::parse(&traw)?;
    actor_rows(&mut info, &mut game, braw.len(), raw.len())?;
    tags(&mut tag)?;
    for (p, y, old, id) in [(ACTOR_INFO, &info, iraw.len(), iid), (GAME_ACTOR_INFO, &game, graw.len(), gid)] {
        let out: Vec<u8> = y.write()?;
        m.outs.push(Out { path: p.to_string(), z: zs::compress_raw(&out, Some(id))?, old, new: out.len() });
    }
    let tout: Vec<u8> = tag.write()?;
    m.outs.push(Out { path: TAG.to_string(), z: zs::compress_raw(&tout, Some(tid))?, old: traw.len(), new: tout.len() });
    for t in [SETTING_TABLE, ANALYZED_TABLE] {
        let (traw, tid): (Vec<u8>, u32) = current(rf, have, t)?;
        let mut y: Byml = Byml::parse(&traw)?;
        event_rows(&mut y)?;
        let out: Vec<u8> = y.write()?;
        m.outs.push(Out { path: t.to_string(), z: zs::compress_raw(&out, Some(tid))?, old: traw.len(), new: out.len() });
    }
    Ok(m)
}

pub fn sized(rf: &mut Romfs, t: &mut Rst, m: &Made) -> Res<usize> {
    let mut n: usize = 0;
    for (p, sz) in &m.sizes {
        t.set_size(p, *sz)?;
        n += 1;
    }
    for o in m.outs.iter().filter(|o| !m.sizes.iter().any(|s| s.0 == o.path) && o.old > 0) {
        n += grow_out(rf, t, o)?;
    }
    Ok(n)
}

pub fn write(rf: &mut Romfs, pk: &Pack, rst: &mut Option<Rst>, d: &Data, seed: &SeedFile, on: bool) -> Res<usize> {
    if !on {
        return Ok(0);
    }
    let root: PB = pk.work.join("romfs");
    if rst.is_none() {
        *rst = Some(rsizetable::load(rf)?);
    }
    let t: &mut Rst = rst.as_mut().ok_or(E::Format("kaepora: no size table"))?;
    let have = |p: &str| -> Option<Vec<u8>> { fs::read(p.split('/').fold(root.clone(), |a, s| a.join(s))).ok() };
    let m: Made = stage(rf, &have, t, &chain(d, seed))?;
    for o in &m.outs {
        pk.write_romfs(&o.path, &o.z)?;
    }
    sized(rf, t, &m)?;
    Ok(m.outs.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kaepora::told;

    fn root() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("RSDB").is_dir() { Some(p) } else { None }
    }

    fn none(_p: &str) -> Option<Vec<u8>> {
        None
    }

    fn links() -> Vec<Link> {
        vec![Link { item: "Boomerang".into(), place: "Zora's Domain".into(), flag: "IsGet.Item_Boomerang".into(), text: told("Boomerang", "Zora's Domain") }]
    }

    #[test]
    fn the_trail_owls_pack_flow_and_schedule_are_never_written_and_the_new_class_is_a_copy() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let t: Rst = rsizetable::load(&mut rf).unwrap();
        let m: Made = stage(&mut rf, &none, &t, &links()).unwrap();
        let paths: Vec<&str> = m.outs.iter().map(|o| o.path.as_str()).collect();
        for p in [pack_of(BASE), flow_file(BASE), schedule_file(BASE)] {
            assert!(!paths.contains(&p.as_str()), "{p} was written");
        }
        for p in [pack_of(OWL), flow_file(OWL), schedule_file(OWL), ACTOR_INFO.to_string(), GAME_ACTOR_INFO.to_string(), TAG.to_string(), SETTING_TABLE.to_string(), ANALYZED_TABLE.to_string()] {
            assert!(paths.contains(&p.as_str()), "{p}");
        }
        for p in [pack_of(OWL), flow_file(OWL), schedule_file(OWL), actor_file(OWL)] {
            let n: u32 = m.sizes.iter().find(|s| s.0 == p).map(|s| s.1).unwrap_or(0);
            let len: usize = m.outs.iter().find(|o| o.path == p).map(|o| o.new).unwrap_or(0);
            assert!(n as usize > len || len == 0, "{p}: {n} for {len}");
        }
        let base: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of(BASE)).unwrap()).unwrap();
        let copy: Vec<Entry> = sarc::parse(&rf.zs.decompress(&m.outs.iter().find(|o| o.path == pack_of(OWL)).unwrap().z).unwrap()).unwrap();
        assert_eq!(copy.len(), base.len());
        for e in &base {
            if e.name == actor_file(BASE) {
                assert!(sarc::get(&copy, &e.name).is_none());
            } else {
                assert_eq!(sarc::get(&copy, &e.name).map(|x| &x.data), Some(&e.data), "{}", e.name);
            }
        }
        let a: Byml = Byml::parse(&sarc::get(&copy, &actor_file(OWL)).unwrap().data).unwrap();
        assert!(format!("{:?}", a.root).contains(&schedule_file(OWL)));
        let s: Vec<u8> = m.outs.iter().find(|o| o.path == schedule_file(OWL)).unwrap().z.clone();
        let txt: String = format!("{:?}", Byml::parse(&s).unwrap().root);
        assert!(txt.contains(OWL) && !txt.contains(BASE), "{txt}");
        for t in [SETTING_TABLE, ANALYZED_TABLE] {
            let was: Byml = rf.read_byml(t).unwrap();
            let now: Byml = Byml::parse(&rf.zs.decompress(&m.outs.iter().find(|o| o.path == t).unwrap().z).unwrap()).unwrap();
            let (a, b): (&[Node], &[Node]) = (was.root.as_arr().unwrap(), now.root.as_arr().unwrap());
            assert_eq!(b.len(), a.len() + 2, "{t}");
            for r in a {
                assert!(b.contains(r), "{t}: a vanilla row changed");
            }
            assert_eq!(b.iter().filter(|r| id(r).starts_with(&format!("{OWL}_"))).count(), 2);
        }
        let fl: crate::bfevfl::Bf = crate::bfevfl::Bf::parse(&rf.zs.decompress(&m.outs.iter().find(|o| o.path == flow_file(OWL)).unwrap().z).unwrap()).unwrap();
        assert!(fl.flows.iter().all(|f| f.name == OWL && f.actors.iter().all(|a| a.name != BASE)));
    }

    #[test]
    fn a_second_copy_is_refused() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let base: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of(BASE)).unwrap()).unwrap();
        let once: Vec<Entry> = build(&base).unwrap();
        assert!(build(&once).is_err());
    }
}
