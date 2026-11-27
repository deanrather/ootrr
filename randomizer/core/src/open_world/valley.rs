//! The Gerudo Valley bridge follows the carpenters' rescue and the bridge setting, nothing else.
//!
//! The valley read "SageOfSpirit is after Step01" ([`fortress::CLEARED`]) for the repaired bridge
//! (the prefab logic module that picks the broken or the fixed bridge), its navmesh
//! (GameBancParam NavMeshSceneAndConditionUnits), the fortress preload anchors (the maps'
//! ChangeBancBeforehandAnchor and the GameBancParam OptimizeChangeBancUnits, by name hash) and
//! the carpenters' tent props. Since the fortress follows the Membership Card (user rule,
//! 2026-11-21), that expression reads the card, so the bridge was mended by the card and stayed
//! broken after a rescue whose card scene held something else.
//!
//! Every valley reader now reads the rescue ([`RESCUED`], played at the end of the card scene and
//! set by the open fortress preset; fast fortress plays it after its one carpenter). With the
//! gerudo_bridge_open setting the bridge gate is forced repaired (as skips::bridge_bytes does) and the
//! navmesh and preload anchors read [`ALWAYS`]; the tent props keep following the rescue, as they
//! are the carpenters' camp, not the bridge.

use crate::ainb::Ainb;
use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::gerudo_card::{bool_node, give_queries, qpos, queries, rewire, BOOL};
use crate::qol::{edit_zs, Have};
use crate::romfs::Romfs;
use crate::staging::Out;

use super::fortress;
use super::skips;

pub const OLD: &str = fortress::CLEARED;
pub const RESCUED: &str = fortress::MARK;
pub const ALWAYS: &str = crate::always::NAME;
pub const MODULE: &str = skips::BRIDGE_MODULE;
pub const PARAM: &str = "Banc/GameBancParam/GerudoValley_Adult.game__banc__GameBancParam.bgyml";
pub const MAPS: [&str; 2] = ["Banc/GerudoValley/GerudoValley_Adult_Dynamic.bcett.byml.zs", "Banc/GerudoValley/GerudoValley_Adult_Static.bcett.byml.zs"];
const READ_EXP: &str = "HoldGameDataGetBoolExpSimple";
const EXP_IN: &str = "GameDataNameBoolExp";

pub fn hash(name: &str) -> u32 {
    ootrr_game::murmur::murmur3_32(name.as_bytes(), 0)
}

/// What the bridge itself (navmesh, preload anchors) reads.
pub fn bridge_flag(open: bool) -> &'static str {
    if open {
        ALWAYS
    } else {
        RESCUED
    }
}

/// Repoints the module's readers of [`OLD`] at [`RESCUED`]; with `open` the bridge gate is first
/// forced repaired. Returns how many links moved.
pub fn patch_module(a: &mut Ainb, open: bool) -> Res<usize> {
    if open {
        let g: usize = skips::bridge_gate(a)?;
        skips::force_bool(a, g, "BoolMulti", true)?;
    }
    let reads: Vec<usize> = a.find_els(READ_EXP).into_iter().filter(|&i| a.in_str(i, EXP_IN) == Some(OLD)).collect();
    if reads.is_empty() {
        return Err(Error::Msg(format!("gerudo valley: {MODULE} does not read {OLD}, the game files differ")));
    }
    // Single links (node, input, source, plug name) and multi links (node, plug name, first
    // entry, entry count, the entries that read the expression).
    let mut single: Vec<(usize, usize, i16, u32)> = Vec::new();
    let mut multi: Vec<(usize, u32, usize, usize, Vec<(usize, u16)>)> = Vec::new();
    for i in 0..a.els.len() {
        for g in a.input_ids(i, BOOL) {
            let x = a.input(BOOL, g).ok_or(Error::Format("gerudo valley: input out of range"))?;
            if x.src >= 0 && reads.contains(&(x.src as usize)) {
                single.push((i, g, x.src, x.name));
            } else if x.src <= -100 && x.plug > 0 {
                let s: usize = (-100 - i32::from(x.src)) as usize;
                let n: usize = x.plug as usize;
                let mut hit: Vec<(usize, u16)> = Vec::new();
                for j in s..s + n {
                    let (el, _) = crate::door_open::multi_entry(a, j).ok_or(Error::Format("gerudo valley: the multi link table is too short"))?;
                    if reads.contains(&usize::from(el)) {
                        hit.push((j, el));
                    }
                }
                if !hit.is_empty() {
                    multi.push((i, x.name, s, n, hit));
                }
            }
        }
    }
    let moved: usize = single.len() + multi.iter().map(|m| m.4.len()).sum::<usize>();
    if moved == 0 {
        return if open { Ok(0) } else { Err(Error::Msg(format!("gerudo valley: nothing in {MODULE} reads {OLD}"))) };
    }
    let read: usize = bool_node(a, RESCUED, "ootrr valley bridge rescued read")?;
    let el: u16 = u16::try_from(read).map_err(|_| Error::Format("gerudo valley: element index out of range"))?;
    for (i, g, src, name) in &single {
        rewire(a, *i, *g, *src, *name, read, 0)?;
    }
    for (i, name, start, n, hit) in &multi {
        for (j, _) in hit {
            a.multi[8 * j..8 * j + 2].copy_from_slice(&el.to_le_bytes());
            a.multi[8 * j + 2..8 * j + 4].copy_from_slice(&0u16.to_le_bytes());
        }
        let p = &mut a.params[*i];
        let (n0, s0): (usize, usize) = (p.pl[0][0] as usize, p.pl[0][1] as usize);
        for k in s0..s0 + n0 {
            if p.plugs[k].b == *name && hit.iter().any(|(_, s)| p.plugs[k].a == u32::from(*s)) {
                p.plugs[k].a = read as u32;
            }
        }
        let q: u16 = qpos(a, read)?;
        let mut want: Vec<u16> = queries(a, *i);
        if want.len() == *n {
            for (j, _) in hit {
                want[j - start] = q;
            }
        } else {
            let old: Vec<u16> = hit.iter().filter_map(|(_, s)| a.query_pos(usize::from(*s)).and_then(|p| u16::try_from(p).ok())).collect();
            want.retain(|w| !old.contains(w));
            if !want.contains(&q) {
                want.push(q);
            }
        }
        give_queries(a, *i, &want)?;
    }
    Ok(moved)
}

pub fn module_bytes(raw: &[u8], open: bool) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    patch_module(&mut a, open)?;
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if back.write()? != out || !crate::ainb::audit(&back).is_empty() {
        return Err(Error::Format("gerudo valley: the bridge module does not read back"));
    }
    Ok(out)
}

/// Swaps every reader of [`OLD`] under `n`: presences (the tent props) to [`RESCUED`], the
/// bridge's own readers to `bridge`. Any other reader is an error. Returns how many moved.
pub fn swap(n: &mut Node, key: &str, bridge: &str) -> Res<usize> {
    let old_hash: u32 = hash(OLD);
    match n {
        Node::Map(m) => {
            let mut k: usize = 0;
            for (name, v) in m.iter_mut() {
                k += swap(v, name, bridge)?;
            }
            Ok(k)
        }
        Node::Arr(v) => v.iter_mut().map(|x| swap(x, key, bridge)).sum(),
        Node::Str(s) if s == OLD => {
            *s = match key {
                "FlagName" => RESCUED,
                "EnableGameData" | "ConditionGameDataPath" => bridge,
                _ => return Err(Error::Msg(format!("gerudo valley: {key} reads {OLD}, which this edit does not know"))),
            }
            .to_string();
            Ok(1)
        }
        Node::U32(h) if key == "EnableGameDataHash" && *h == old_hash => {
            *h = hash(bridge);
            Ok(1)
        }
        _ => Ok(0),
    }
}

pub fn byml_bytes(raw: &[u8], what: &str, open: bool) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    if swap(&mut y.root, "", bridge_flag(open))? == 0 {
        return Err(Error::Msg(format!("gerudo valley: {what} does not read {OLD}, the game files differ")));
    }
    let out: Vec<u8> = y.write()?;
    if Byml::parse(&out)?.root != y.root {
        return Err(Error::Msg(format!("gerudo valley: {what} does not read back")));
    }
    Ok(out)
}

fn merge_zs(rf: &mut Romfs, outs: &mut Vec<Out>, have: Have, path: &str, f: impl FnOnce(&[u8]) -> Res<Vec<u8>>) -> Res<()> {
    match outs.iter_mut().find(|o| o.path == path) {
        Some(o) => {
            let id: Option<u32> = crate::flow_locations::dict_id(&o.z);
            let raw: Vec<u8> = rf.zs.decompress(&o.z)?;
            let b: Vec<u8> = f(&raw)?;
            o.new = b.len();
            o.z = crate::zs::compress_raw(&b, id)?;
        }
        None => outs.push(edit_zs(rf, have, path, f)?),
    }
    Ok(())
}

/// Stages the bridge module, the valley's GameBancParam and both adult valley maps on top of `outs`.
pub fn stage(rf: &mut Romfs, outs: &mut Vec<Out>, have: Have, open: bool) -> Res<()> {
    fortress::merge_plain(rf, outs, have, MODULE, |raw| module_bytes(raw, open))?;
    fortress::merge_plain(rf, outs, have, PARAM, |raw| byml_bytes(raw, PARAM, open))?;
    for m in MAPS {
        merge_zs(rf, outs, have, m, |raw| byml_bytes(raw, m, open))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real() -> Option<Romfs> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Romfs::open(&p).ok()
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn strings(n: &Node, key: &str, out: &mut Vec<(String, String)>) {
        match n {
            Node::Map(m) => m.iter().for_each(|(k, v)| strings(v, k, out)),
            Node::Arr(v) => v.iter().for_each(|x| strings(x, key, out)),
            Node::Str(s) => out.push((key.to_string(), s.clone())),
            Node::U32(h) if key == "EnableGameDataHash" => out.push((key.to_string(), h.to_string())),
            _ => {}
        }
    }

    fn readers(a: &Ainb, flag: &str, node: &str, input: &str) -> usize {
        let src: Vec<usize> = a.find_els(node).into_iter().filter(|&i| a.in_str(i, input) == Some(flag)).collect();
        let mut n: usize = 0;
        for x in (0..a.els.len()).flat_map(|i| a.input_ids(i, BOOL).into_iter().filter_map(|g| a.input(BOOL, g))) {
            if x.src >= 0 && src.contains(&(x.src as usize)) {
                n += 1;
            } else if x.src <= -100 && x.plug > 0 {
                let s: usize = (-100 - i32::from(x.src)) as usize;
                n += (s..s + x.plug as usize).filter(|&j| src.contains(&usize::from(crate::door_open::multi_entry(a, j).unwrap().0))).count();
            }
        }
        n
    }

    #[test]
    fn the_bridge_module_reads_the_rescue_or_is_forced_open() {
        let Some(rf) = real() else { return };
        let raw: Vec<u8> = rf.read(MODULE).unwrap();
        let before: usize = readers(&Ainb::parse(&raw).unwrap(), OLD, READ_EXP, EXP_IN);
        assert!(before > 0);
        let shut: Ainb = Ainb::parse(&module_bytes(&raw, false).unwrap()).unwrap();
        assert_eq!(readers(&shut, OLD, READ_EXP, EXP_IN), 0);
        assert_eq!(readers(&shut, RESCUED, crate::gerudo_card::GET_BOOL, "GameDataNameBool"), before);
        assert!(module_bytes(&module_bytes(&raw, false).unwrap(), false).is_err());
        let open: Ainb = Ainb::parse(&module_bytes(&raw, true).unwrap()).unwrap();
        assert_eq!(readers(&open, OLD, READ_EXP, EXP_IN), 0);
        assert!(skips::bridge_gate(&open).is_err());
    }

    #[test]
    fn the_valley_scene_data_reads_the_rescue_and_the_bridge_flag() {
        let Some(mut rf) = real() else { return };
        for open in [false, true] {
            let mut outs: Vec<Out> = Vec::new();
            let none = |_: &str| -> Option<Vec<u8>> { None };
            stage(&mut rf, &mut outs, &none, open).unwrap();
            assert_eq!(outs.len(), 2 + MAPS.len());
            let bridge: &str = bridge_flag(open);
            let mut all: Vec<(String, String)> = Vec::new();
            for o in &outs[1..] {
                let raw: Vec<u8> = if o.path == PARAM { o.z.clone() } else { rf.zs.decompress(&o.z).unwrap() };
                strings(&Byml::parse(&raw).unwrap().root, "", &mut all);
            }
            assert!(!all.iter().any(|(_, s)| s == OLD || *s == hash(OLD).to_string()), "{all:?}");
            let has = |k: &str, v: &str| all.iter().any(|(a, b)| a == k && b == v);
            assert!(has("FlagName", RESCUED));
            assert!(has("EnableGameData", bridge));
            assert!(has("ConditionGameDataPath", bridge));
            assert!(has("EnableGameDataHash", &hash(bridge).to_string()));
            let mut again: Vec<Out> = outs.clone();
            assert!(stage(&mut rf, &mut again, &none, open).is_err());
        }
    }
}
