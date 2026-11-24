use ootrr_core::error::{Error as E, Res};

use crate::game::Game;
use crate::snap::Snap;

const GDM_BSS: u64 = 0x04e9_0ce8;
const GDM_VT: u64 = 0x04c5_8270;
const MASTER: u64 = 0xdc0;
const COLLECTED: u64 = 0x1_0000_0000;
const MAX_CNT: u32 = 65_536;
const MAX_CAP: u32 = 131_072;
const MIN_PTR: u64 = 0x10_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Val {
    Byte(usize),
    Word(usize),
}

#[derive(Debug, Clone, Copy)]
struct Table {
    off: u64,
    stride: usize,
    vt: u64,
    val: Val,
}

const TABLES: [Table; 5] = [
    Table { off: 0x170, stride: 0x18, vt: 0x04c2_0658, val: Val::Byte(0x16) },
    Table { off: 0x230, stride: 0x28, vt: 0x04c2_0718, val: Val::Word(0x1c) },
    Table { off: 0x2f0, stride: 0x28, vt: 0x04c2_07d8, val: Val::Word(0x1c) },
    Table { off: 0x3b0, stride: 0x38, vt: 0x04c2_0898, val: Val::Word(0x1c) },
    Table { off: 0x8f0, stride: 0x28, vt: 0x04c2_0dd8, val: Val::Word(0x1c) },
];

fn r32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn r64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3], b[o + 4], b[o + 5], b[o + 6], b[o + 7]])
}

fn gdm(g: &Game) -> Res<u64> {
    let p: u64 = g.u64_at(g.main + GDM_BSS).ok_or_else(|| E::Msg("the game memory could not be read".to_string()))?;
    if p < MIN_PTR || g.u64_at(p) != Some(g.main + GDM_VT) {
        return Err(E::Msg("no game loaded yet".to_string()));
    }
    Ok(p)
}

fn table(g: &Game, base: u64, t: &Table, out: &mut Vec<(u32, u32)>) -> Res<()> {
    let bad = || E::Msg("the game data table could not be read".to_string());
    let hd: Vec<u8> = g.read(base + t.off, 0x28).ok_or_else(bad)?;
    let cnt: u32 = r32(&hd, 0x08);
    let ents: u64 = r64(&hd, 0x10);
    let ht: u64 = r64(&hd, 0x18);
    let cap: u32 = r32(&hd, 0x24);
    if cnt == 0 || cnt > MAX_CNT || cap < cnt || cap > MAX_CAP || ents < MIN_PTR || ht < MIN_PTR {
        return Err(bad());
    }
    let hb: Vec<u8> = g.read(ht, cap as usize * 8).ok_or_else(bad)?;
    let eb: Vec<u8> = g.read(ents, cnt as usize * t.stride).ok_or_else(bad)?;
    let vt: u64 = g.main + t.vt;
    for i in 0..cap as usize {
        let h: u32 = r32(&hb, i * 8);
        let idx: u32 = r32(&hb, i * 8 + 4);
        if h == 0 || idx >= cnt {
            continue;
        }
        let e: usize = idx as usize * t.stride;
        if r64(&eb, e) != vt {
            continue;
        }
        let v: u32 = match t.val {
            Val::Byte(o) => eb[e + o] as u32,
            Val::Word(o) => r32(&eb, e + o),
        };
        out.push((h, v));
    }
    Ok(())
}

fn master(g: &Game, base: u64) -> Res<Vec<u64>> {
    let bad = || E::Msg("the collected object table could not be read".to_string());
    let m: Vec<u8> = g.read(base + MASTER, 16).ok_or_else(bad)?;
    let cnt: u64 = r64(&m, 0);
    let ptr: u64 = r64(&m, 8);
    if !(100..=MAX_CNT as u64).contains(&cnt) || ptr < MIN_PTR {
        return Err(bad());
    }
    let b: Vec<u8> = g.read(ptr, cnt as usize * 16).ok_or_else(bad)?;
    let mut out: Vec<u64> = Vec::new();
    for i in 0..cnt as usize {
        let k: u64 = r64(&b, i * 16);
        if k != 0 && r64(&b, i * 16 + 8) & COLLECTED != 0 {
            out.push(k);
        }
    }
    Ok(out)
}

fn bool_entry(g: &Game, base: u64, hash: u32) -> Res<u64> {
    let t: &Table = &TABLES[0];
    let bad = || E::Msg("the game data table could not be read".to_string());
    let hd: Vec<u8> = g.read(base + t.off, 0x28).ok_or_else(bad)?;
    let cnt: u32 = r32(&hd, 0x08);
    let ents: u64 = r64(&hd, 0x10);
    let ht: u64 = r64(&hd, 0x18);
    let cap: u32 = r32(&hd, 0x24);
    if cnt == 0 || cnt > MAX_CNT || cap < cnt || cap > MAX_CAP || ents < MIN_PTR || ht < MIN_PTR {
        return Err(bad());
    }
    let hb: Vec<u8> = g.read(ht, cap as usize * 8).ok_or_else(bad)?;
    let idx: u32 = (0..cap as usize)
        .find(|i| r32(&hb, i * 8) == hash && r32(&hb, i * 8 + 4) < cnt)
        .map(|i| r32(&hb, i * 8 + 4))
        .ok_or_else(|| E::Msg(format!("the running game has no Bool flag with hash {hash:08x}, so this mod is not the one running")))?;
    let at: u64 = ents + idx as u64 * t.stride as u64;
    if g.u64_at(at) != Some(g.main + t.vt) {
        return Err(bad());
    }
    Ok(at)
}

pub fn get_bool(g: &Game, hash: u32) -> Res<bool> {
    let base: u64 = gdm(g)?;
    let at: u64 = bool_entry(g, base, hash)?;
    let Val::Byte(o) = TABLES[0].val else { return Err(E::Msg("the Bool table layout is unknown".to_string())) };
    let b: Vec<u8> = g.read(at + o as u64, 1).ok_or_else(|| E::Msg("the flag could not be read".to_string()))?;
    Ok(b[0] & 1 != 0)
}

pub fn set_bool(g: &Game, hash: u32, v: bool) -> Res<bool> {
    let base: u64 = gdm(g)?;
    let at: u64 = bool_entry(g, base, hash)?;
    let Val::Byte(o) = TABLES[0].val else { return Err(E::Msg("the Bool table layout is unknown".to_string())) };
    let pos: u64 = at + o as u64;
    let cur: u8 = g.read(pos, 1).ok_or_else(|| E::Msg("the flag could not be read".to_string()))?[0];
    let new: u8 = (cur & !1) | v as u8;
    if new != cur && !g.write(pos, &[new]) {
        return Err(E::Msg("the game refused the flag write (is it running as another user?)".to_string()));
    }
    Ok(cur & 1 != 0)
}

pub fn snap(g: &Game) -> Res<Snap> {
    let base: u64 = gdm(g)?;
    let mut rows: Vec<(u32, u32)> = Vec::new();
    for t in &TABLES {
        table(g, base, t, &mut rows)?;
    }
    let keys: Vec<u64> = master(g, base)?;
    Ok(Snap::new(rows, keys))
}
