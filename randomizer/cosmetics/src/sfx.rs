use std::collections::{BTreeMap as BM, BTreeSet as BS};
use std::fs;
use std::io::Read;
use std::path::{Path as P, PathBuf as PB};

use crate::error::{Error as E, Res};
use crate::rng::Rng;
use crate::romfs::Romfs;
use crate::rsizetable;
use crate::zs;

const RES: &str = "Sound/Resource";
const STREAM: &str = "Sound/Resource/Stream";
const MAX_BYTES: u64 = 131072;
const BLOCK: usize = 6;
const MIN_POOL: usize = 4;
const NEAR: usize = 8;
const SKIP: [&str; 8] = ["Env", "EnvAnimal", "Dm", "Demo", "Land", "CrumblingGanonCastle", "Escape", "Sys"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Off,
    Grouped,
    Anywhere,
}

impl Sfx {
    pub fn parse(s: &str) -> Option<Sfx> {
        match s {
            "off" => Some(Sfx::Off),
            "grouped" => Some(Sfx::Grouped),
            "anywhere" => Some(Sfx::Anywhere),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Sfx::Off => "off",
            Sfx::Grouped => "grouped",
            Sfx::Anywhere => "anywhere",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cand {
    pub name: String,
    pub group: String,
    pub samples: u32,
    pub bytes: u64,
    pub crc: u32,
    pub pre: usize,
}

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

pub fn group_of(name: &str) -> Option<&str> {
    name.strip_prefix("SE_")?.split('_').next().filter(|g| !g.is_empty())
}

pub fn eligible(name: &str, head: &[u8], bytes: u64) -> Option<Cand> {
    if head.len() < 0x50 || &head[..4] != b"BWAV" || bytes > MAX_BYTES {
        return None;
    }
    let group: &str = group_of(name)?;
    if SKIP.contains(&group) {
        return None;
    }
    let mono: bool = le16(head, 0xE) == 1;
    let codec: bool = le16(head, 0x10) == 1;
    let rate: bool = le32(head, 0x14) == 48000;
    let once: bool = le32(head, 0x4C) == 0xFFFF_FFFF;
    if !(mono && codec && rate && once) {
        return None;
    }
    Some(Cand { name: name.to_string(), group: group.to_string(), samples: le32(head, 0x1C), bytes, crc: le32(head, 8), pre: 0 })
}

pub fn scan(dir: &P) -> Res<Vec<Cand>> {
    let mut out: Vec<Cand> = Vec::new();
    for e in fs::read_dir(dir)? {
        let e = e?;
        let file: String = e.file_name().to_string_lossy().to_string();
        let Some(stem) = file.strip_suffix(".bwav") else {
            continue;
        };
        if !stem.starts_with("SE_") {
            continue;
        }
        let bytes: u64 = e.metadata()?.len();
        if bytes > MAX_BYTES {
            continue;
        }
        let mut head: Vec<u8> = vec![0; 0x50];
        let mut f: fs::File = fs::File::open(e.path())?;
        if f.read_exact(&mut head).is_err() {
            continue;
        }
        if let Some(c) = eligible(stem, &head, bytes) {
            out.push(c);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn pools(c: &[Cand], m: Sfx) -> Vec<Vec<&Cand>> {
    let mut by: BM<String, Vec<&Cand>> = BM::new();
    for x in c {
        if x.pre == 0 {
            continue;
        }
        let key: String = format!("{} {}", x.pre, if m == Sfx::Anywhere { "" } else { x.group.as_str() });
        by.entry(key).or_default().push(x);
    }
    by.into_values().filter(|v| v.len() >= MIN_POOL).collect()
}

pub fn plan(c: &[Cand], m: Sfx, rng: &mut Rng) -> BM<String, String> {
    let mut out: BM<String, String> = BM::new();
    if m == Sfx::Off {
        return out;
    }
    for mut pool in pools(c, m) {
        pool.sort_by(|a, b| (a.samples, &a.name).cmp(&(b.samples, &b.name)));
        for block in pool.chunks(BLOCK) {
            let mut to: Vec<&Cand> = block.to_vec();
            rng.shuffle(&mut to);
            for (a, b) in block.iter().zip(to) {
                if a.name != b.name {
                    out.insert(a.name.clone(), b.name.clone());
                }
            }
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub at: BM<String, Vec<(String, usize)>>,
    pub block: BM<String, Vec<u8>>,
    pub mixed: Vec<String>,
}

pub fn entries(b: &[u8]) -> Res<Vec<(u32, usize)>> {
    if b.len() < 0x10 || &b[..4] != b"BARS" {
        return Err(E::Format("bars: bad magic"));
    }
    let n: usize = le32(b, 0xC) as usize;
    let pairs: usize = 0x10 + 4 * n;
    if b.len() < pairs + 8 * n {
        return Err(E::Format("bars: the entry table leaves the file"));
    }
    Ok((0..n).map(|i| (le32(b, 0x10 + 4 * i), le32(b, pairs + 8 * i + 4) as usize)).collect())
}

pub fn span(b: &[u8], w: usize) -> Option<usize> {
    let h: &[u8] = b.get(w..w + 0x50)?;
    if &h[..4] != b"BWAV" || le16(h, 0xE) != 1 {
        return None;
    }
    let end: usize = w + le32(h, 0x44) as usize + (le32(h, 0x1C) as usize).div_ceil(14) * 8;
    (end <= b.len()).then_some(end - w)
}

pub fn read_bank(b: &[u8], bars: &str, want: &BM<u32, &Cand>, bank: &mut Bank) -> Res<()> {
    let all: Vec<(u32, usize)> = entries(b)?;
    let mut uses: BM<usize, usize> = BM::new();
    for (_, w) in &all {
        *uses.entry(*w).or_default() += 1;
    }
    for (h, w) in all {
        let Some(c) = want.get(&h) else {
            continue;
        };
        let pre: bool = uses[&w] == 1 && b.get(w..w + 0x10).is_some_and(|x| &x[..4] == b"BWAV" && le16(x, 0xC) == 1 && le32(x, 8) == c.crc);
        match span(b, w).filter(|_| pre) {
            Some(n) => {
                bank.at.entry(c.name.clone()).or_default().push((bars.to_string(), w));
                bank.block.entry(c.name.clone()).or_insert_with(|| b[w..w + n].to_vec());
            }
            None => bank.mixed.push(c.name.clone()),
        }
    }
    Ok(())
}

pub fn fit(cands: &mut Vec<Cand>, bank: &Bank) {
    for c in cands.iter_mut() {
        let len: usize = bank.block.get(&c.name).map(|b| b.len()).unwrap_or(0);
        c.pre = if bank.mixed.contains(&c.name) { 0 } else { len };
    }
    cands.retain(|c| c.pre > 0);
}

pub fn rewrite(raw: &mut [u8], bars: &str, map: &BM<String, String>, bank: &Bank) -> Res<usize> {
    let mut n: usize = 0;
    for (target, from) in map {
        let block: &[u8] = bank.block.get(from).ok_or(E::Format("a moved sound has no prefetch block"))?;
        for (f, w) in bank.at.get(target).map(|v| v.as_slice()).unwrap_or(&[]) {
            if f != bars {
                continue;
            }
            let slot: &mut [u8] = raw.get_mut(*w..*w + block.len()).ok_or(E::Format("bars: a prefetch block leaves the file"))?;
            if span(slot, 0) != Some(block.len()) {
                return Err(E::Format("bars: two moved sounds have prefetch blocks of different sizes"));
            }
            slot.copy_from_slice(block);
            n += 1;
        }
    }
    Ok(n)
}

#[derive(Clone, Debug, Default)]
pub struct Shorts {
    pub at: BM<String, Vec<(String, usize)>>,
    pub block: BM<String, Vec<u8>>,
    pub cap: BM<String, usize>,
    pub bad: BS<String>,
}

pub fn amta_name(b: &[u8], a: usize) -> Option<&str> {
    let n: usize = le32(b.get(a..a + 0xC)?, 8) as usize;
    let s: &[u8] = b.get(a..a + n)?;
    s.split(|c| *c == 0).rev().find(|x| !x.is_empty()).and_then(|x| std::str::from_utf8(x).ok())
}

pub fn read_shorts(b: &[u8], bars: &str, streamed: &BS<String>, sh: &mut Shorts) -> Res<()> {
    let all: Vec<(u32, usize)> = entries(b)?;
    let n: usize = all.len();
    let amta: Vec<usize> = (0..n).map(|i| le32(b, 0x10 + 4 * n + 8 * i) as usize).collect();
    let mut starts: Vec<usize> = all.iter().map(|x| x.1).chain(amta.iter().copied()).collect();
    starts.push(b.len());
    starts.sort_unstable();
    let mut uses: BM<usize, usize> = BM::new();
    for (_, w) in &all {
        *uses.entry(*w).or_default() += 1;
    }
    for ((h, w), a) in all.iter().zip(amta) {
        let Some(name) = amta_name(b, a).filter(|x| rsizetable::hash(x) == *h && x.starts_with("SE_")) else {
            continue;
        };
        let Some(g) = group_of(name) else {
            continue;
        };
        let head: Option<&[u8]> = b.get(*w..*w + 0x50);
        let ok: bool = !SKIP.contains(&g)
            && !streamed.contains(name)
            && uses[w] == 1
            && head.is_some_and(|x| &x[..4] == b"BWAV" && le16(x, 0xC) == 0 && le16(x, 0xE) == 1 && le16(x, 0x10) == 1 && le32(x, 0x14) == 48000 && le32(x, 0x4C) == 0xFFFF_FFFF);
        let len: Option<usize> = span(b, *w).filter(|_| ok);
        let cap: usize = starts.iter().copied().find(|x| *x > *w).unwrap_or(b.len()) - w;
        match len.filter(|l| *l <= cap) {
            Some(l) => {
                sh.at.entry(name.to_string()).or_default().push((bars.to_string(), *w));
                sh.block.entry(name.to_string()).or_insert_with(|| b[*w..*w + l].to_vec());
                let c: &mut usize = sh.cap.entry(name.to_string()).or_insert(cap);
                *c = (*c).min(cap);
            }
            None => {
                sh.bad.insert(name.to_string());
            }
        }
    }
    Ok(())
}

pub fn plan_short(sh: &Shorts, m: Sfx, rng: &mut Rng) -> BM<String, String> {
    let mut out: BM<String, String> = BM::new();
    if m == Sfx::Off {
        return out;
    }
    let mut by: BM<String, Vec<(usize, &String)>> = BM::new();
    for (name, block) in &sh.block {
        if sh.bad.contains(name) {
            continue;
        }
        let key: String = if m == Sfx::Anywhere { String::new() } else { group_of(name).unwrap_or("").to_string() };
        by.entry(key).or_default().push((block.len(), name));
    }
    for mut pool in by.into_values().filter(|v| v.len() >= MIN_POOL) {
        pool.sort();
        for (i, (_, t)) in pool.iter().enumerate() {
            let cap: usize = sh.cap[*t];
            let lo: usize = i.saturating_sub(NEAR);
            let hi: usize = (i + NEAR + 1).min(pool.len());
            let fit: Vec<&String> = pool[lo..hi].iter().filter(|x| x.0 <= cap).map(|x| x.1).collect();
            let pick: &String = fit[rng.below(fit.len() as u64) as usize];
            if pick != *t {
                out.insert((*t).clone(), pick.clone());
            }
        }
    }
    out
}

pub fn rewrite_short(raw: &mut [u8], bars: &str, map: &BM<String, String>, sh: &Shorts) -> Res<usize> {
    let mut n: usize = 0;
    for (target, from) in map {
        let block: &[u8] = sh.block.get(from).ok_or(E::Format("a moved sound has no data in its bank"))?;
        if block.len() > sh.cap[target] {
            return Err(E::Format("bars: a moved sound does not fit the slot of the sound it replaces"));
        }
        for (f, w) in sh.at.get(target).map(|v| v.as_slice()).unwrap_or(&[]) {
            if f != bars {
                continue;
            }
            raw.get_mut(*w..*w + block.len()).ok_or(E::Format("bars: a sound leaves the file"))?.copy_from_slice(block);
            n += 1;
        }
    }
    Ok(n)
}

fn check_bank_layout(old: &[u8], edited: &[u8]) -> Res<()> {
    let n: usize = entries(old)?.len();
    let table: usize = 0x10 + 12 * n;
    if edited.len() != old.len() || edited.get(..table) != old.get(..table) {
        return Err(E::Format("sound effects: an existing sound bank has a different layout and cannot be safely shuffled"));
    }
    Ok(())
}

pub fn apply(mod_root: &P, romfs: &P, m: Sfx, seed: &str) -> Res<Vec<String>> {
    let mut notes: Vec<String> = Vec::new();
    if m == Sfx::Off {
        return Ok(notes);
    }
    let mut rf: Romfs = Romfs::open(romfs)?;
    let src: PB = romfs.join(STREAM);
    let mut cands: Vec<Cand> = scan(&src)?;
    let want: BM<u32, &Cand> = cands.iter().map(|c| (rsizetable::hash(&c.name), c)).collect();
    let streamed: BS<String> = fs::read_dir(&src)?.filter_map(|e| e.ok()?.file_name().to_string_lossy().strip_suffix(".bwav").map(|s| s.to_string())).collect();
    let mut bank: Bank = Bank::default();
    let mut sh: Shorts = Shorts::default();
    let banks: Vec<String> = rf.list(RES)?.into_iter().filter(|r| r.ends_with(".bars.zs")).collect();
    for r in &banks {
        let raw: Vec<u8> = rf.read_zs(r)?;
        read_bank(&raw, r, &want, &mut bank)?;
        read_shorts(&raw, r, &streamed, &mut sh)?;
    }
    fit(&mut cands, &bank);
    let mut rng: Rng = Rng::from_text(&format!("{seed} sfx {}", m.id()));
    let map: BM<String, String> = plan(&cands, m, &mut rng);
    let short: BM<String, String> = plan_short(&sh, m, &mut rng);
    if map.is_empty() && short.is_empty() {
        return Err(E::Format("no sound effect could be moved"));
    }
    let dst: PB = mod_root.join("romfs").join(STREAM);
    fs::create_dir_all(&dst)?;
    let mut bytes: u64 = 0;
    for (target, from) in &map {
        let data: Vec<u8> = fs::read(src.join(format!("{from}.bwav")))?;
        bytes += data.len() as u64;
        fs::write(dst.join(format!("{target}.bwav")), data)?;
    }
    let mut touched: Vec<&String> = map.keys().flat_map(|t| bank.at.get(t).into_iter().flatten().map(|x| &x.0)).chain(short.keys().flat_map(|t| sh.at.get(t).into_iter().flatten().map(|x| &x.0))).collect();
    touched.sort();
    touched.dedup();
    for r in &touched {
        let out: PB = mod_root.join("romfs").join(r.as_str());
        let original: Vec<u8> = rf.read(r)?;
        let z: Vec<u8> = if out.try_exists()? { fs::read(&out)? } else { original.clone() };
        let mut raw: Vec<u8> = rf.zs.decompress(&z)?;
        if z != original {
            let old: Vec<u8> = rf.zs.decompress(&original)?;
            check_bank_layout(&old, &raw)?;
        }
        rewrite(&mut raw, r, &map, &bank)?;
        rewrite_short(&mut raw, r, &short, &sh)?;
        if let Some(d) = out.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(&out, zs::compress_raw(&raw, zs::dict_of(&z))?)?;
        bytes += raw.len() as u64;
    }
    notes.push(format!(
        "Sound effects are shuffled {}. {} streamed and {} short effects moved, {} sound banks rewritten to match, {} MB added to the mod. Cutscene, ambient, menu and looping sounds keep their own.",
        if m == Sfx::Grouped { "within each kind of sound" } else { "across all kinds of sound" },
        map.len(),
        short.len(),
        touched.len(),
        bytes / 1_000_000
    ));
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(ch: u16, rate: u32, samples: u32, loop_end: u32) -> Vec<u8> {
        let mut h: Vec<u8> = vec![0; 0x60];
        h[..4].copy_from_slice(b"BWAV");
        h[0xE..0x10].copy_from_slice(&ch.to_le_bytes());
        h[0x10..0x12].copy_from_slice(&1u16.to_le_bytes());
        h[0x14..0x18].copy_from_slice(&rate.to_le_bytes());
        h[0x1C..0x20].copy_from_slice(&samples.to_le_bytes());
        h[0x4C..0x50].copy_from_slice(&loop_end.to_le_bytes());
        h
    }

    fn cand(name: &str, samples: u32) -> Cand {
        Cand { name: name.to_string(), group: group_of(name).unwrap().to_string(), samples, bytes: 100, crc: 0, pre: 8320 }
    }

    #[test]
    fn only_short_mono_one_shots_outside_the_skipped_groups_qualify() {
        let ok: Vec<u8> = head(1, 48000, 900, 0xFFFF_FFFF);
        assert!(eligible("SE_Player_Jump00", &ok, 100).is_some());
        assert!(eligible("SE_Env_Wind00", &ok, 100).is_none());
        assert!(eligible("SE_Sys_Cancel00", &ok, 100).is_none());
        assert!(eligible("SE_Player_Jump00", &head(2, 48000, 900, 0xFFFF_FFFF), 100).is_none());
        assert!(eligible("SE_Player_Jump00", &head(1, 48000, 900, 500), 100).is_none());
        assert!(eligible("SE_Player_Jump00", &head(1, 96000, 900, 0xFFFF_FFFF), 100).is_none());
        assert!(eligible("SE_Player_Jump00", &ok, MAX_BYTES + 1).is_none());
        assert!(eligible("Vo_PlAdult_Flail01", &ok, 100).is_none());
    }

    #[test]
    fn grouped_effects_trade_inside_their_group_and_near_in_length() {
        let c: Vec<Cand> = (0..12).map(|i| cand(&format!("SE_Player_A{i:02}"), 1000 + i * 10)).chain((0..8).map(|i| cand(&format!("SE_Item_B{i:02}"), 50 + i))).collect();
        let m: BM<String, String> = plan(&c, Sfx::Grouped, &mut Rng::from_text("x"));
        assert!(!m.is_empty());
        for (a, b) in &m {
            assert_eq!(group_of(a), group_of(b));
            let (sa, sb) = (c.iter().find(|x| &x.name == a).unwrap().samples as i64, c.iter().find(|x| &x.name == b).unwrap().samples as i64);
            assert!((sa - sb).abs() <= 60);
        }
    }

    #[test]
    fn anywhere_mixes_groups_and_is_deterministic() {
        let c: Vec<Cand> = (0..6).map(|i| cand(&format!("SE_Player_A{i}"), 100 + i)).chain((0..6).map(|i| cand(&format!("SE_Item_B{i}"), 100 + i))).collect();
        let a: BM<String, String> = plan(&c, Sfx::Anywhere, &mut Rng::from_text("seed"));
        let b: BM<String, String> = plan(&c, Sfx::Anywhere, &mut Rng::from_text("seed"));
        assert_eq!(a, b);
        assert!(a.iter().any(|(x, y)| group_of(x) != group_of(y)));
        assert!(plan(&c, Sfx::Off, &mut Rng::from_text("seed")).is_empty());
    }

    #[test]
    fn a_target_gets_exactly_one_source_and_every_source_is_used_once() {
        let c: Vec<Cand> = (0..30).map(|i| cand(&format!("SE_Npc_C{i:02}"), 500 + i * 3)).collect();
        let m: BM<String, String> = plan(&c, Sfx::Grouped, &mut Rng::from_text("q"));
        let mut from: Vec<&String> = m.values().collect();
        from.sort();
        from.dedup();
        assert_eq!(from.len(), m.len());
    }

    #[test]
    fn small_groups_are_left_alone() {
        let c: Vec<Cand> = (0..3).map(|i| cand(&format!("SE_Rare_D{i}"), 10 + i)).collect();
        assert!(plan(&c, Sfx::Grouped, &mut Rng::from_text("q")).is_empty());
    }

    #[test]
    fn mode_names_round_trip() {
        for m in [Sfx::Off, Sfx::Grouped, Sfx::Anywhere] {
            assert_eq!(Sfx::parse(m.id()), Some(m));
        }
        assert_eq!(Sfx::parse("loud"), None);
    }

    fn bwav(crc: u32, pre: u16, ns: u32, fill: u8) -> Vec<u8> {
        let mut b: Vec<u8> = vec![0; 0x80];
        b[..4].copy_from_slice(b"BWAV");
        b[8..12].copy_from_slice(&crc.to_le_bytes());
        b[0xC..0xE].copy_from_slice(&pre.to_le_bytes());
        b[0xE..0x10].copy_from_slice(&1u16.to_le_bytes());
        b[0x1C..0x20].copy_from_slice(&ns.to_le_bytes());
        b[0x44..0x48].copy_from_slice(&0x80u32.to_le_bytes());
        b.extend(std::iter::repeat_n(fill, (ns as usize).div_ceil(14) * 8));
        b
    }

    fn bars(rows: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let n: usize = rows.len();
        let mut b: Vec<u8> = b"BARS".to_vec();
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&[0xFF, 0xFE, 0, 1]);
        b.extend_from_slice(&(n as u32).to_le_bytes());
        for (name, _) in rows {
            b.extend_from_slice(&rsizetable::hash(name).to_le_bytes());
        }
        let table: usize = b.len();
        b.resize(table + 8 * n, 0);
        for (i, (_, w)) in rows.iter().enumerate() {
            let at: usize = b.len();
            b[table + 8 * i + 4..table + 8 * i + 8].copy_from_slice(&(at as u32).to_le_bytes());
            b.extend_from_slice(w);
        }
        b
    }

    fn real_cand(name: &str, crc: u32) -> Cand {
        Cand { name: name.to_string(), group: "Player".to_string(), samples: 1000, bytes: 100, crc, pre: 0 }
    }

    #[test]
    fn a_moved_sound_takes_its_prefetch_block_and_shared_blocks_never_move() {
        let rows: Vec<(&str, Vec<u8>)> = vec![("SE_Player_A", bwav(11, 1, 28, 0xAA)), ("SE_Player_B", bwav(22, 1, 28, 0xBB)), ("SE_Player_C", bwav(33, 0, 28, 0xCC)), ("SE_Player_D", bwav(99, 1, 28, 0xDD)), ("SE_Player_E", bwav(55, 1, 28, 0xEE)), ("SE_Player_F", Vec::new())];
        let mut raw: Vec<u8> = bars(&rows);
        let e: Vec<(u32, usize)> = entries(&raw).unwrap();
        let t: usize = 0x10 + 4 * rows.len() + 8 * 5 + 4;
        raw[t..t + 4].copy_from_slice(&(e[4].1 as u32).to_le_bytes());
        let mut cands: Vec<Cand> = vec![real_cand("SE_Player_A", 11), real_cand("SE_Player_B", 22), real_cand("SE_Player_C", 33), real_cand("SE_Player_D", 44), real_cand("SE_Player_E", 55), real_cand("SE_Player_F", 55)];
        let want: BM<u32, &Cand> = cands.iter().map(|c| (rsizetable::hash(&c.name), c)).collect();
        let mut bank: Bank = Bank::default();
        read_bank(&raw, "x.bars", &want, &mut bank).unwrap();
        drop(want);
        fit(&mut cands, &bank);
        let names: Vec<&str> = cands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["SE_Player_A", "SE_Player_B"]);
        assert!(cands.iter().all(|c| c.pre == 0x80 + 16));
        let len: usize = raw.len();
        let map: BM<String, String> = [("SE_Player_A".to_string(), "SE_Player_B".to_string()), ("SE_Player_B".to_string(), "SE_Player_A".to_string())].into_iter().collect();
        assert_eq!(rewrite(&mut raw, "x.bars", &map, &bank).unwrap(), 2);
        assert_eq!(rewrite(&mut raw.clone(), "y.bars", &map, &bank).unwrap(), 0);
        assert_eq!(raw.len(), len);
        let e: Vec<(u32, usize)> = entries(&raw).unwrap();
        assert_eq!(e[4].1, e[5].1);
        assert_eq!(le32(&raw, e[0].1 + 8), 22);
        assert_eq!(le32(&raw, e[1].1 + 8), 11);
        assert_eq!(raw[e[0].1 + 0x80], 0xBB);
        assert_eq!(le32(&raw, e[2].1 + 8), 33);
    }

    #[test]
    fn shuffling_keeps_other_edits_to_a_compatible_bank() {
        let rows = [("SE_Player_A", bwav(11, 1, 28, 0xAA)), ("SE_Player_B", bwav(22, 1, 28, 0xBB)), ("SE_Sys_HeartAlert", bwav(33, 1, 28, 0xCC))];
        let original = bars(&rows);
        let e = entries(&original).unwrap();
        let mut edited = original.clone();
        edited[e[2].1 + 0x80] = 0;
        check_bank_layout(&original, &edited).unwrap();
        let cands = [real_cand("SE_Player_A", 11), real_cand("SE_Player_B", 22)];
        let want = cands.iter().map(|c| (rsizetable::hash(&c.name), c)).collect();
        let mut bank = Bank::default();
        read_bank(&original, "x.bars", &want, &mut bank).unwrap();
        let map = [("SE_Player_A".to_string(), "SE_Player_B".to_string())].into_iter().collect();
        rewrite(&mut edited, "x.bars", &map, &bank).unwrap();
        assert_eq!(edited[e[0].1 + 0x80], 0xBB);
        assert_eq!(edited[e[2].1 + 0x80], 0);
        edited[0x10] ^= 1;
        assert!(check_bank_layout(&original, &edited).is_err());
    }

    #[test]
    fn sounds_without_a_prefetch_block_or_of_another_block_size_never_trade() {
        let mut c: Vec<Cand> = (0..6).map(|i| cand(&format!("SE_Player_A{i}"), 100 + i)).chain((0..6).map(|i| Cand { pre: 5064, ..cand(&format!("SE_Player_B{i}"), 100 + i) })).collect();
        c.extend((0..6).map(|i| Cand { pre: 0, ..cand(&format!("SE_Player_C{i}"), 100 + i) }));
        let m: BM<String, String> = plan(&c, Sfx::Anywhere, &mut Rng::from_text("z"));
        assert!(!m.is_empty());
        let pre = |n: &String| c.iter().find(|x| &x.name == n).unwrap().pre;
        assert!(m.iter().all(|(a, b)| pre(a) == pre(b) && pre(a) > 0));
    }

    #[test]
    fn the_real_banks_match_every_moved_stream_and_two_seeds_differ() {
        let Some(root) = std::env::var("OOTRR_ROMFS").map(PB::from).ok().filter(|p| p.join("Pack").join("ZsDic.pack.zs").is_file()) else {
            return;
        };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let mut maps: Vec<BM<String, u32>> = Vec::new();
        for seed in ["first", "second"] {
            let out: PB = std::env::temp_dir().join(format!("ootrr_sfx_{}_{seed}", std::process::id()));
            let _ = fs::remove_dir_all(&out);
            let notes: Vec<String> = apply(&out, &root, Sfx::Grouped, seed).unwrap();
            println!("{notes:?}");
            let streams: BM<String, u32> = fs::read_dir(out.join("romfs").join(STREAM))
                .unwrap()
                .map(|e| {
                    let e = e.unwrap();
                    let b: Vec<u8> = fs::read(e.path()).unwrap();
                    (e.file_name().to_string_lossy().trim_end_matches(".bwav").to_string(), le32(&b, 8))
                })
                .collect();
            let hashes: BM<u32, &String> = streams.keys().map(|k| (rsizetable::hash(k), k)).collect();
            let mut checked: BM<&String, usize> = BM::new();
            let mut shorts: usize = 0;
            let bank_dir: PB = out.join("romfs").join(RES);
            for e in fs::read_dir(&bank_dir).unwrap() {
                let e = e.unwrap();
                let name: String = e.file_name().to_string_lossy().to_string();
                if !name.ends_with(".bars.zs") {
                    continue;
                }
                let new: Vec<u8> = rf.zs.decompress(&fs::read(e.path()).unwrap()).unwrap();
                let old: Vec<u8> = rf.read_zs(&format!("{RES}/{name}")).unwrap();
                assert_eq!(new.len(), old.len(), "{name}");
                let ends: BS<usize> = entries(&old).unwrap().into_iter().map(|e| e.1).chain([old.len()]).collect();
                for ((h, w), (_, v)) in entries(&new).unwrap().into_iter().zip(entries(&old).unwrap()) {
                    assert_eq!(w, v);
                    if let Some(k) = hashes.get(&h) {
                        assert_eq!(le32(&new, w + 8), streams[*k], "{name} {k}");
                        *checked.entry(k).or_default() += 1;
                    } else if new[w..w + 0x50] != old[w..w + 0x50] {
                        let next: usize = *ends.range(w + 1..).next().unwrap();
                        assert!(le16(&new, w + 0xC) == 0 && span(&new, w).is_some_and(|n| w + n <= next), "{name} {w}");
                        shorts += 1;
                    }
                }
            }
            assert_eq!(checked.len(), streams.len());
            println!("{seed}: {} streamed and {shorts} short bank entries changed", streams.len());
            assert!(shorts > 2000, "{shorts}");
            maps.push(streams);
            let _ = fs::remove_dir_all(&out);
        }
        assert!(maps[0].len() > 1000);
        assert_ne!(maps[0], maps[1]);
    }

    fn named(rows: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let n: usize = rows.len();
        let mut b: Vec<u8> = b"BARS".to_vec();
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&[0xFF, 0xFE, 0, 1]);
        b.extend_from_slice(&(n as u32).to_le_bytes());
        for (name, _) in rows {
            b.extend_from_slice(&rsizetable::hash(name).to_le_bytes());
        }
        let table: usize = b.len();
        b.resize(table + 8 * n, 0);
        for (i, (name, w)) in rows.iter().enumerate() {
            let a: usize = b.len();
            let mut m: Vec<u8> = b"AMTA".to_vec();
            m.extend_from_slice(&[0; 4]);
            let size: usize = 0x10 + name.len() + 1;
            m.extend_from_slice(&(size as u32).to_le_bytes());
            m.extend_from_slice(&[0; 4]);
            m.extend_from_slice(name.as_bytes());
            m.push(0);
            b.extend_from_slice(&m);
            while !b.len().is_multiple_of(64) {
                b.push(0);
            }
            let at: usize = b.len();
            b[table + 8 * i..table + 8 * i + 4].copy_from_slice(&(a as u32).to_le_bytes());
            b[table + 8 * i + 4..table + 8 * i + 8].copy_from_slice(&(at as u32).to_le_bytes());
            b.extend_from_slice(w);
        }
        b
    }

    fn short(ns: u32, fill: u8) -> Vec<u8> {
        let mut w: Vec<u8> = bwav(ns, 0, ns, fill);
        w[0x10..0x12].copy_from_slice(&1u16.to_le_bytes());
        w[0x14..0x18].copy_from_slice(&48000u32.to_le_bytes());
        w[0x4C..0x50].copy_from_slice(&u32::MAX.to_le_bytes());
        w
    }

    #[test]
    fn short_sounds_only_move_into_slots_they_fit_and_banks_keep_their_size() {
        let rows: Vec<(&str, Vec<u8>)> = (0..6).map(|i| ("", short(14 * (i + 1), 0x10 + i as u8))).collect();
        let names: Vec<String> = (0..6).map(|i| format!("SE_Player_S{i}")).collect();
        let rows: Vec<(&str, Vec<u8>)> = rows.into_iter().enumerate().map(|(i, r)| (names[i].as_str(), r.1)).collect();
        let raw: Vec<u8> = named(&rows);
        let len: usize = raw.len();
        let mut sh: Shorts = Shorts::default();
        read_shorts(&raw, "x.bars", &BS::new(), &mut sh).unwrap();
        assert_eq!(sh.block.len(), 6);
        let mut hit: bool = false;
        for s in 0..30 {
            let m: BM<String, String> = plan_short(&sh, Sfx::Grouped, &mut Rng::from_text(&format!("s{s}")));
            for (t, f) in &m {
                assert!(sh.block[f].len() <= sh.cap[t]);
            }
            if !m.is_empty() {
                let mut r: Vec<u8> = raw.clone();
                assert!(rewrite_short(&mut r, "x.bars", &m, &sh).unwrap() > 0);
                assert_eq!(r.len(), len);
                hit = true;
            }
        }
        assert!(hit);
        let mut streamed: BS<String> = BS::new();
        streamed.insert("SE_Player_S0".to_string());
        let mut sh2: Shorts = Shorts::default();
        read_shorts(&raw, "x.bars", &streamed, &mut sh2).unwrap();
        assert!(sh2.bad.contains("SE_Player_S0"));

    }
}
