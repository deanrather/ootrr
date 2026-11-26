use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::qol::{edit_plain, edit_zs, Have};
use crate::romfs::Romfs;
use crate::rsizetable::{crc32, hash};
use crate::sarc;
use crate::staging::Out;

pub const NUM: u16 = 206;
pub const SETTING: &str = "disable_low_health_warnings";
pub const NOTE: &str = "Suppresses the low-health beep, critical-health pose and expression, heart flashing, and red pulse on Link. The red pulse change needs playtesting; normal damage feedback should remain.";

const CHILD: &str = "AS/Player_Child.root.asb.zs";
const ADULT: &str = "AS/Player_Adult.root.asb.zs";
const HUD: &str = "UI/Layout2Archive/Common.Product.100.Nin_Ounce_NVN2.blarc.zs";
const STREAM: &str = "Sound/Resource/Stream/SE_Sys_HeartAlert.bwav";
const BANK: &str = "Sound/Resource/Player.bars.zs";
pub const PACKS: [&str; 2] = [crate::qol::bunny::FILE, crate::qol::bunny::ADULT_FILE];
pub const FILES: [&str; 7] = [CHILD, ADULT, HUD, STREAM, BANK, PACKS[0], PACKS[1]];
const PARAM: &str = "Player/PlayerGameParam/Player.game__player__PlayerGameParam.bgyml";
const CURVE: &str = "DamageColorCurve";

const SELECTORS: [(usize, u32, u32); 6] = [
    (907, 908, 1396),
    (13682, 13683, 13680),
    (13717, 13718, 13708),
    (13728, 13729, 13719),
    (13885, 13886, 13891),
    (13920, 13921, 13922),
];

fn bad() -> Error {
    Error::Format("low-health warnings: unexpected or truncated game resource")
}

fn u16_at(b: &[u8], at: usize) -> Res<u16> {
    Ok(u16::from_le_bytes(b.get(at..at + 2).ok_or_else(bad)?.try_into().map_err(|_| bad())?))
}

fn u32_at(b: &[u8], at: usize) -> Res<u32> {
    Ok(u32::from_le_bytes(b.get(at..at + 4).ok_or_else(bad)?.try_into().map_err(|_| bad())?))
}

fn u64_at(b: &[u8], at: usize) -> Res<usize> {
    let value: usize = usize::try_from(u64::from_le_bytes(b.get(at..at + 8).ok_or_else(bad)?.try_into().map_err(|_| bad())?)).map_err(|_| bad())?;
    if value > b.len() {
        return Err(bad());
    }
    Ok(value)
}

fn put32(b: &mut [u8], at: usize, v: u32) -> Res<()> {
    b.get_mut(at..at + 4).ok_or_else(bad)?.copy_from_slice(&v.to_le_bytes());
    Ok(())
}

fn named(b: &[u8], at: usize, name: &str) -> bool {
    b.get(at..at + name.len() + 1).is_some_and(|s| s[..name.len()] == *name.as_bytes() && s[name.len()] == 0)
}

fn animation(raw: &[u8]) -> Res<Vec<u8>> {
    if raw.get(..4) != Some(b"ASB ") || u32_at(raw, 4)? != 0x41b {
        return Err(bad());
    }
    let bb: usize = u32_at(raw, 0x38)? as usize;
    let pool: usize = u32_at(raw, 0x3c)? as usize;
    let count: usize = u16_at(raw, bb + 32)? as usize;
    let start: usize = u16_at(raw, bb + 34)? as usize;
    let mut entries: Vec<usize> = Vec::new();
    for i in 0..count {
        let off: u32 = u32_at(raw, bb + 0x38 + 4 * (start + i))? & 0xff_ffff;
        if named(raw, pool + off as usize, "IsCautionLife") {
            entries.push(i);
        }
    }
    if entries.as_slice() != [18] {
        return Err(bad());
    }
    let nodes: usize = u32_at(raw, 0x18)? as usize;
    let count: usize = u32_at(raw, 0x14)? as usize;
    let mut out: Vec<u8> = raw.to_vec();
    for (index, yes, no) in SELECTORS {
        if index >= count || yes as usize >= count || no as usize >= count {
            return Err(bad());
        }
        let node: usize = nodes + index * 0x24;
        let param: usize = u32_at(raw, node + 8)? as usize;
        let flags: u32 = u32_at(raw, param)?;
        let table: usize = param + 0x14;
        let header: &[u8] = raw.get(table + 4..table + 6).ok_or_else(bad)?;
        if u16_at(raw, node)? != 21 || flags & 0x8300_ffff != 0x8000_0012 || header != [2, 0] {
            return Err(bad());
        }
        let first: usize = u32_at(raw, table + 12)? as usize;
        let second: usize = u32_at(raw, table + 16)? as usize;
        if first == second || u32_at(raw, first)? != yes || u32_at(raw, second)? != no {
            return Err(bad());
        }
        put32(&mut out, first, no)?;
    }
    Ok(out)
}

fn quiet_curve(b: &mut [u8], record: usize, name: &str, curves: usize, middle: f32, value: f32) -> Res<()> {
    if !named(b, u64_at(b, record)?, name) || u32_at(b, record + 0x20)? as usize != curves || u32_at(b, record + 0x24)? != 8 {
        return Err(bad());
    }
    let table: usize = u64_at(b, record + 0x18)?;
    for i in 0..curves {
        let curve: usize = u64_at(b, table + 8 * i)?;
        let values: usize = u64_at(b, curve + 8)?;
        let data: usize = u64_at(b, values + 8)?;
        if u32_at(b, values + 16)? != 6 || u32_at(b, values + 20)? != 4 {
            return Err(bad());
        }
        let expected: [f32; 6] = [value, 0.0, middle, 0.0, value, 0.0];
        for (j, f) in expected.iter().enumerate() {
            if u32_at(b, data + 4 * j)? != f.to_bits() {
                return Err(bad());
            }
        }
        put32(b, data + 8, value.to_bits())?;
    }
    Ok(())
}

fn layout(b: &mut [u8], gauge: bool) -> Res<()> {
    if b.get(..4) != Some(b"BFFH") || u32_at(b, 8)? as usize != b.len() {
        return Err(bad());
    }
    let (pinch, count, notice): (usize, usize, usize) = if gauge { (0x2e48, 1, 0x34e8) } else { (0x2ce0, 4, 0x32d0) };
    quiet_curve(b, pinch, "Pinch", count, 0.85, 0.0)?;
    quiet_curve(b, notice, "HeartNoticeLoop", 1, 0.2, 1.0)
}

fn hud(raw: &[u8]) -> Res<Vec<u8>> {
    if raw.get(6..8) != Some([0xff, 0xfe].as_slice()) {
        return Err(bad());
    }
    let entries: Vec<sarc::Entry> = sarc::parse(raw)?;
    let sfat: usize = u16_at(raw, 4)? as usize;
    let data: usize = u32_at(raw, 12)? as usize;
    let mut out: Vec<u8> = raw.to_vec();
    let mut counts: [usize; 2] = [0, 0];
    for (i, entry) in entries.iter().enumerate() {
        let gauge: bool = match entry.name.as_str() {
            "PaHeartGaugeChildNoGauge_00.buil" => false,
            "PaHeartGaugeChildGauge_00.buil" => true,
            _ => continue,
        };
        let node: usize = sfat + 12 + i * 16;
        let start: usize = data + u32_at(raw, node + 8)? as usize;
        let end: usize = data + u32_at(raw, node + 12)? as usize;
        if end < start || end - start != entry.data.len() {
            return Err(bad());
        }
        layout(out.get_mut(start..end).ok_or_else(bad)?, gauge)?;
        counts[usize::from(gauge)] += 1;
    }
    if counts != [4, 4] {
        return Err(bad());
    }
    Ok(out)
}

pub fn pulse_pack(raw: &[u8]) -> Res<Vec<u8>> {
    let mut entries: Vec<sarc::Entry> = sarc::parse(raw)?;
    let param = sarc::get(&entries, PARAM).ok_or_else(bad)?;
    let mut byml: Byml = Byml::parse(&param.data)?;
    let curve = byml.root.get_mut(CURVE).ok_or_else(bad)?;
    if curve.get("Type").and_then(Node::as_str) != Some("Hermit") || curve.get("MaxX") != Some(&Node::F32(1.0)) {
        return Err(bad());
    }
    let data = curve.get("Data").and_then(Node::as_arr).ok_or_else(bad)?;
    if data.len() != 14 || !data.iter().all(|n| matches!(n, Node::F32(v) if v.is_finite())) || !data.iter().any(|n| matches!(n, Node::F32(v) if *v != 0.0)) {
        return Err(bad());
    }
    // Hermit data consists of evenly spaced value/tangent pairs, not explicit x coordinates.
    curve.set("Data", Node::Arr(vec![Node::F32(0.0); data.len()]));
    sarc::set(&mut entries, PARAM, byml.write()?);
    sarc::write(&entries)
}

fn wave(b: &[u8], prefetch: bool) -> Res<(usize, usize)> {
    if b.get(..4) != Some(b"BWAV") || b.get(4..8) != Some([0xff, 0xfe, 1, 0].as_slice())
        || u16_at(b, 12)? != u16::from(prefetch) || u16_at(b, 14)? != 1
        || u16_at(b, 16)? != 1 || u32_at(b, 20)? != 48000 || u32_at(b, 0x4c)? != u32::MAX
    {
        return Err(bad());
    }
    let start: usize = u32_at(b, 0x44)? as usize;
    let len: usize = (u32_at(b, 0x1c)? as usize).div_ceil(14).checked_mul(8).ok_or_else(bad)?;
    if start < 0x5c || len == 0 || b.get(start..start.checked_add(len).ok_or_else(bad)?).is_none() {
        return Err(bad());
    }
    Ok((start, len))
}

fn silence(raw: &[u8]) -> Res<(Vec<u8>, u32)> {
    let (start, len): (usize, usize) = wave(raw, false)?;
    if u32_at(raw, 8)? != crc32(&raw[start..start + len]) {
        return Err(bad());
    }
    let mut out: Vec<u8> = raw.to_vec();
    out[start..start + len].fill(0);
    out[0x54..0x5a].fill(0);
    let crc: u32 = crc32(&out[start..start + len]);
    put32(&mut out, 8, crc)?;
    Ok((out, crc))
}

fn bank(raw: &[u8], stream: &[u8], crc: u32) -> Res<Vec<u8>> {
    if raw.get(..4) != Some(b"BARS") {
        return Err(bad());
    }
    let n: usize = u32_at(raw, 12)? as usize;
    let pairs: usize = 0x10 + 4 * n;
    raw.get(..pairs + 8 * n).ok_or_else(bad)?;
    let (stream_start, stream_len): (usize, usize) = wave(stream, false)?;
    let mut found: Vec<usize> = Vec::new();
    for i in 0..n {
        if u32_at(raw, 0x10 + 4 * i)? == hash("SE_Sys_HeartAlert") {
            found.push(u32_at(raw, pairs + 8 * i + 4)? as usize);
        }
    }
    if found.len() != 1 {
        return Err(bad());
    }
    let at: usize = found[0];
    for i in 0..n {
        if u32_at(raw, pairs + 8 * i + 4)? as usize == at && u32_at(raw, 0x10 + 4 * i)? != hash("SE_Sys_HeartAlert") {
            return Err(bad());
        }
    }
    let pre: &[u8] = raw.get(at..).ok_or_else(bad)?;
    let (start, len): (usize, usize) = wave(pre, true)?;
    if u32_at(pre, 8)? != u32_at(stream, 8)? || len > stream_len || pre[start..start + len] != stream[stream_start..stream_start + len] {
        return Err(bad());
    }
    let mut out: Vec<u8> = raw.to_vec();
    out[at + start..at + start + len].fill(0);
    out[at + 0x54..at + 0x5a].fill(0);
    put32(&mut out, at + 8, crc)?;
    Ok(out)
}

pub fn stage(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    let stream: Vec<u8> = match have(STREAM) {
        Some(b) => b,
        None => rf.read(STREAM)?,
    };
    let (_, crc): (Vec<u8>, u32) = silence(&stream)?;
    Ok(vec![
        edit_zs(rf, have, CHILD, animation)?,
        edit_zs(rf, have, ADULT, animation)?,
        edit_zs(rf, have, HUD, hud)?,
        edit_plain(rf, have, STREAM, |b| Ok(silence(b)?.0))?,
        edit_zs(rf, have, BANK, |b| bank(b, &stream, crc))?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, Value};

    #[test]
    fn setting_is_optional_and_adds_a_note() {
        let mut settings: Settings = Settings::default();
        let mut bad: Vec<String> = Vec::new();
        assert!(!crate::qol::read(&settings, &mut bad).low_health);
        settings.set(SETTING, Value::Bool(true)).unwrap();
        let opts = crate::qol::read(&settings, &mut bad);
        assert!(bad.is_empty());
        assert!(opts.low_health && opts.any());
        assert!(opts.notes().contains(&NOTE.to_string()));
    }

    #[test]
    fn real_resources_preserve_unrelated_data() {
        let root = ootrr_game::test_romfs::path();
        if !root.is_dir() {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", root.display());
            return;
        }
        let mut rf = Romfs::open(&root).unwrap();
        let defaults = crate::qol::Opts::default();
        assert!(crate::qol::stage(&mut rf, &defaults, &|_| None).unwrap().is_empty());
        let opts = crate::qol::Opts { low_health: true, ..defaults };
        let outs = crate::qol::stage(&mut rf, &opts, &|_| None).unwrap();
        assert_eq!(outs.len(), FILES.len());
        for out in outs.iter().filter(|o| !PACKS.contains(&o.path.as_str())) {
            assert_eq!(out.old, out.new, "{}", out.path);
        }
        for path in [CHILD, ADULT] {
            let original = rf.read_zs(path).unwrap();
            let out = outs.iter().find(|o| o.path == path).unwrap();
            let changed = rf.zs.decompress(&out.z).unwrap();
            for (index, _, no) in SELECTORS {
                let nodes = u32_at(&original, 0x18).unwrap() as usize;
                let param = u32_at(&original, nodes + index * 0x24 + 8).unwrap() as usize;
                let plug = u32_at(&original, param + 0x20).unwrap() as usize;
                assert_eq!(u32_at(&changed, plug).unwrap(), no);
            }
            assert!(animation(&changed).is_err());
        }
        let original = rf.read_zs(HUD).unwrap();
        let out = outs.iter().find(|o| o.path == HUD).unwrap();
        let changed = rf.zs.decompress(&out.z).unwrap();
        let before = sarc::parse(&original).unwrap();
        let after = sarc::parse(&changed).unwrap();
        assert_eq!(before.len(), after.len());
        let mut changed_layouts = 0;
        for (old, new) in before.iter().zip(&after) {
            assert_eq!(old.name, new.name);
            if old.data != new.data {
                assert!(old.name == "PaHeartGaugeChildGauge_00.buil" || old.name == "PaHeartGaugeChildNoGauge_00.buil");
                changed_layouts += 1;
            }
        }
        assert_eq!(changed_layouts, 8);
        assert!(hud(&changed).is_err());
        let stream = rf.read(STREAM).unwrap();
        let silent = &outs.iter().find(|o| o.path == STREAM).unwrap().z;
        let (start, len) = wave(silent, false).unwrap();
        assert!(silent[start..start + len].iter().all(|b| *b == 0));
        assert_eq!(u32_at(silent, 8).unwrap(), crc32(&silent[start..start + len]));
        let original_bank = rf.read_zs(BANK).unwrap();
        let out = outs.iter().find(|o| o.path == BANK).unwrap();
        let muted_bank = rf.zs.decompress(&out.z).unwrap();
        assert_eq!(muted_bank, bank(&original_bank, &stream, u32_at(silent, 8).unwrap()).unwrap());
        assert!(bank(&muted_bank, &stream, u32_at(silent, 8).unwrap()).is_err());
        for path in PACKS {
            let before = sarc::parse(&rf.read_zs(path).unwrap()).unwrap();
            let out = outs.iter().find(|o| o.path == path).unwrap();
            let raw = rf.zs.decompress(&out.z).unwrap();
            let after = sarc::parse(&raw).unwrap();
            assert_eq!(before.len(), after.len());
            for entry in &before {
                let new = sarc::get(&after, &entry.name).unwrap();
                if entry.name == PARAM {
                    let mut old = Byml::parse(&entry.data).unwrap();
                    let updated = Byml::parse(&new.data).unwrap();
                    let curve = old.root.get_mut(CURVE).unwrap();
                    curve.set("Data", Node::Arr(vec![Node::F32(0.0); 14]));
                    assert_eq!(old.root, updated.root);
                } else {
                    assert_eq!(entry.data, new.data, "{}", entry.name);
                }
            }
            assert!(pulse_pack(&raw).is_err());
        }
    }

    #[test]
    fn pulse_edit_stacks_with_other_player_pack_edits() {
        let root = ootrr_game::test_romfs::path();
        if !root.is_dir() {
            eprintln!("skipped: no romfs at {}, set OOTRR_ROMFS", root.display());
            return;
        }
        let mut rf = Romfs::open(&root).unwrap();
        let opts = crate::qol::Opts { low_health: true, gear_cycle: true, bunny: crate::qol::bunny::Mode::Speed, ..Default::default() };
        let outs = crate::qol::stage(&mut rf, &opts, &|_| None).unwrap();
        for path in PACKS {
            let packs: Vec<_> = outs.iter().filter(|o| o.path == path).collect();
            assert_eq!(packs.len(), 1);
            let entries = sarc::parse(&rf.zs.decompress(&packs[0].z).unwrap()).unwrap();
            let player = Byml::parse(&sarc::get(&entries, PARAM).unwrap().data).unwrap();
            let data = player.root.get(CURVE).unwrap().get("Data").unwrap().as_arr().unwrap();
            assert!(data.iter().all(|n| *n == Node::F32(0.0)));
            let original = rf.read_zs(path).unwrap();
            let expected = crate::qol::bunny::patch_pack(&original, crate::qol::bunny::Mode::Speed).unwrap();
            let expected = crate::qol::gear_cycle::patch_boots_hop(&expected).unwrap();
            let expected = sarc::parse(&expected).unwrap();
            for name in [crate::qol::bunny::WALK.file, crate::qol::gear_cycle::SCHEDULER] {
                assert_eq!(sarc::get(&entries, name).unwrap().data, sarc::get(&expected, name).unwrap().data);
            }
        }
    }
}
