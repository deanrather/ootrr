use std::collections::BTreeMap as BM;
use std::fs;
use std::path::Path as P;
use std::sync::Arc;

use crate::error::{Error as E, Res};

const DMADATA: usize = 0x7430;
const FONT_TABLE: usize = 0x10_26A0;
const BANK_TABLE: usize = 0x10_31C0;
const VELOCITY_TABLE: usize = 0x0E_FD24;
const GATE_TABLE: usize = 0x0E_FD34;
const ENVELOPE_TABLE: usize = 0x0E_FD44;
const ARGS_TABLE: usize = 0x0F_0790;
const FONT_COUNT: usize = 38;
const FILE_MAX: usize = 16 << 20;
const BAD: E = E::Format("custom music: the N64 ROM is damaged or is not Ocarina of Time (NTSC 1.0)");

pub type Envelope = Arc<[(i16, i16)]>;

#[derive(Debug)]
pub struct Sample {
    pub data: Vec<f32>,
    pub start: usize,
    pub end: usize,
    pub loops: bool,
}

#[derive(Clone, Debug)]
pub struct Sound {
    pub sample: Arc<Sample>,
    pub tuning: f32,
}

#[derive(Clone, Debug)]
pub struct Instrument {
    pub low_to: u8,
    pub high_from: u8,
    pub release: u8,
    pub envelope: Option<Envelope>,
    pub sounds: [Option<Sound>; 3],
}

impl Instrument {
    pub fn sound(&self, note: u8) -> Option<&Sound> {
        let i: usize = if note < self.low_to { 0 } else if note <= self.high_from { 1 } else { 2 };
        self.sounds[i].as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct Drum {
    pub release: u8,
    pub pan: u8,
    pub envelope: Envelope,
    pub sound: Sound,
}

#[derive(Debug, Default)]
pub struct Font {
    pub instruments: Vec<Option<Instrument>>,
    pub drums: Vec<Option<Drum>>,
}

pub fn word(b: &[u8], at: usize) -> Res<u32> {
    let w: &[u8] = b.get(at..at.saturating_add(4)).ok_or(BAD)?;
    Ok(u32::from_be_bytes([w[0], w[1], w[2], w[3]]))
}

fn half(b: &[u8], at: usize) -> Res<i16> {
    let w: &[u8] = b.get(at..at.saturating_add(2)).ok_or(BAD)?;
    Ok(i16::from_be_bytes([w[0], w[1]]))
}

fn byte(b: &[u8], at: usize) -> Res<u8> {
    b.get(at).copied().ok_or(BAD)
}

pub fn big_endian(mut b: Vec<u8>) -> Res<Vec<u8>> {
    match b.get(..4) {
        Some([0x80, 0x37, 0x12, 0x40]) => {}
        Some([0x37, 0x80, 0x40, 0x12]) => b.chunks_exact_mut(2).for_each(|c| c.reverse()),
        Some([0x40, 0x12, 0x37, 0x80]) => b.chunks_exact_mut(4).for_each(|c| c.reverse()),
        _ => return Err(BAD),
    }
    Ok(b)
}

pub fn yaz0(d: &[u8]) -> Res<Vec<u8>> {
    if d.get(..4) != Some(b"Yaz0") {
        return Err(BAD);
    }
    let size: usize = word(d, 4)? as usize;
    if size > FILE_MAX {
        return Err(BAD);
    }
    let mut out: Vec<u8> = Vec::with_capacity(size);
    let mut at: usize = 16;
    let mut code: u8 = 0;
    let mut bits: u8 = 0;
    while out.len() < size {
        if bits == 0 {
            code = byte(d, at)?;
            at += 1;
            bits = 8;
        }
        if code & 0x80 != 0 {
            out.push(byte(d, at)?);
            at += 1;
        } else {
            let (a, b): (u8, u8) = (byte(d, at)?, byte(d, at + 1)?);
            at += 2;
            let back: usize = ((usize::from(a & 15) << 8) | usize::from(b)) + 1;
            let mut n: usize = usize::from(a >> 4) + 2;
            if a >> 4 == 0 {
                n = usize::from(byte(d, at)?) + 0x12;
                at += 1;
            }
            let from: usize = out.len().checked_sub(back).ok_or(BAD)?;
            for k in 0..n {
                let v: u8 = out[from + k];
                out.push(v);
            }
        }
        code <<= 1;
        bits -= 1;
    }
    Ok(out)
}

pub fn decode(raw: &[u8], small: bool, book: &[i32]) -> Vec<f32> {
    let bits: usize = if small { 2 } else { 4 };
    let frame: usize = 1 + 2 * bits;
    let mut out: Vec<f32> = Vec::with_capacity(raw.len() / frame * 16);
    let mut p1: i64 = 0;
    let mut p2: i64 = 0;
    for f in raw.chunks_exact(frame) {
        let scale: u32 = u32::from(f[0] >> 4);
        let base: usize = usize::from(f[0] & 15) * 16;
        let Some(c) = book.get(base..base + 16) else {
            break;
        };
        let mut values: [i64; 16] = [0; 16];
        let mut n: usize = 0;
        for b in &f[1..] {
            for k in 0..8 / bits {
                let mut v: i64 = i64::from(b >> (8 - bits * (k + 1))) & ((1 << bits) - 1);
                if v >= 1 << (bits - 1) {
                    v -= 1 << bits;
                }
                values[n] = v << scale;
                n += 1;
            }
        }
        for group in values.chunks_exact(8) {
            let mut got: [i64; 8] = [0; 8];
            for i in 0..8 {
                let mut sum: i64 = i64::from(c[i]) * p1 + i64::from(c[8 + i]) * p2 + (group[i] << 11);
                for j in 0..i {
                    sum += i64::from(c[8 + i - 1 - j]) * group[j];
                }
                got[i] = (sum >> 11).clamp(-32768, 32767);
            }
            out.extend(got.iter().map(|v| *v as f32));
            p1 = got[6];
            p2 = got[7];
        }
    }
    out
}

pub struct Tables {
    pub velocity: [u8; 16],
    pub gate: [u8; 16],
    pub args: [u8; 80],
    pub envelope: Envelope,
}

fn bytes<const N: usize>(b: &[u8], at: usize) -> Res<[u8; N]> {
    b.get(at..at.saturating_add(N)).and_then(|s| s.try_into().ok()).ok_or(BAD)
}

fn tables(code: &[u8]) -> Res<Tables> {
    let envelope: Vec<(i16, i16)> = (0..4).map(|i| Ok((half(code, ENVELOPE_TABLE + 4 * i)?, half(code, ENVELOPE_TABLE + 4 * i + 2)?))).collect::<Res<Vec<(i16, i16)>>>()?;
    Ok(Tables { velocity: bytes(code, VELOCITY_TABLE)?, gate: bytes(code, GATE_TABLE)?, args: bytes(code, ARGS_TABLE)?, envelope: envelope.into() })
}

pub struct Rom {
    pub tables: Tables,
    bank: Vec<u8>,
    table: Vec<u8>,
    fonts: Vec<(usize, usize, u8, u8, usize, usize)>,
    banks: Vec<(usize, usize)>,
}

fn file(rom: &[u8], row: usize) -> Res<Vec<u8>> {
    let at: usize = DMADATA + 16 * row;
    let (start, end, from, to): (usize, usize, usize, usize) = (word(rom, at)? as usize, word(rom, at + 4)? as usize, word(rom, at + 8)? as usize, word(rom, at + 12)? as usize);
    if to != 0 {
        return yaz0(rom.get(from..to).ok_or(BAD)?);
    }
    let size: usize = end.checked_sub(start).ok_or(BAD)?;
    Ok(rom.get(from..from.saturating_add(size)).ok_or(BAD)?.to_vec())
}

impl Rom {
    pub fn open(path: &P) -> Res<Rom> {
        let raw: Vec<u8> = fs::read(path).map_err(|e| E::Msg(format!("custom music: {} could not be read: {e}", path.display())))?;
        let rom: Vec<u8> = big_endian(raw)?;
        let mut code: Option<Vec<u8>> = None;
        for row in 6..40 {
            let at: usize = DMADATA + 16 * row;
            if (word(&rom, at + 4)?.wrapping_sub(word(&rom, at)?) as usize) <= FONT_TABLE {
                continue;
            }
            let c: Vec<u8> = file(&rom, row)?;
            if half(&c, FONT_TABLE).ok() == Some(FONT_COUNT as i16) {
                code = Some(c);
                break;
            }
        }
        let code: Vec<u8> = code.ok_or(BAD)?;
        let mut fonts: Vec<(usize, usize, u8, u8, usize, usize)> = Vec::new();
        for i in 0..FONT_COUNT {
            let at: usize = FONT_TABLE + 16 + 16 * i;
            fonts.push((word(&code, at)? as usize, word(&code, at + 4)? as usize, byte(&code, at + 10)?, byte(&code, at + 11)?, usize::from(byte(&code, at + 12)?), usize::from(byte(&code, at + 13)?)));
        }
        let count: usize = usize::try_from(half(&code, BANK_TABLE)?).map_err(|_| BAD)?;
        let mut banks: Vec<(usize, usize)> = Vec::new();
        for i in 0..count {
            let at: usize = BANK_TABLE + 16 + 16 * i;
            banks.push((word(&code, at)? as usize, word(&code, at + 4)? as usize));
        }
        Ok(Rom { tables: tables(&code)?, bank: file(&rom, 3)?, table: file(&rom, 5)?, fonts, banks })
    }

    pub fn font(&self, index: usize, cache: &mut BM<(usize, usize), Arc<Sample>>) -> Res<Font> {
        let &(addr, size, bank1, bank2, ninst, ndrum) = self.fonts.get(index).ok_or(BAD)?;
        let b: &[u8] = self.bank.get(addr..addr.saturating_add(size)).ok_or(BAD)?;
        let mut sound = |at: usize| -> Res<Option<Sound>> {
            let head: usize = word(b, at)? as usize;
            if head == 0 {
                return Ok(None);
            }
            let tuning: f32 = f32::from_bits(word(b, at + 4)?);
            let info: u32 = word(b, head)?;
            let codec: u32 = info >> 28;
            let which: u8 = if (info >> 26) & 3 == 0 { bank1 } else { bank2 };
            let mut entry: (usize, usize) = *self.banks.get(usize::from(which)).ok_or(BAD)?;
            if entry.1 == 0 {
                entry = *self.banks.get(entry.0).ok_or(BAD)?;
            }
            let start: usize = entry.0 + word(b, head + 4)? as usize;
            let bytes: usize = (info & 0xFF_FFFF) as usize;
            if let Some(s) = cache.get(&(start, bytes)) {
                return Ok(Some(Sound { sample: s.clone(), tuning }));
            }
            if codec != 0 && codec != 3 {
                return Err(BAD);
            }
            let around: usize = word(b, head + 8)? as usize;
            let at_book: usize = word(b, head + 12)? as usize;
            let order: u32 = word(b, at_book)?;
            let count: usize = word(b, at_book + 4)? as usize;
            if order != 2 || count > 16 {
                return Err(BAD);
            }
            let book: Vec<i32> = (0..16 * count).map(|i| half(b, at_book + 8 + 2 * i).map(i32::from)).collect::<Res<Vec<i32>>>()?;
            let data: Vec<f32> = decode(self.table.get(start..start.saturating_add(bytes)).ok_or(BAD)?, codec == 3, &book);
            let end: usize = (word(b, around + 4)? as usize).min(data.len());
            let s: Arc<Sample> = Arc::new(Sample { start: word(b, around)? as usize, end, loops: word(b, around + 8)? != 0, data });
            cache.insert((start, bytes), s.clone());
            Ok(Some(Sound { sample: s, tuning }))
        };
        let envelope = |mut at: usize| -> Envelope {
            let mut out: Vec<(i16, i16)> = Vec::new();
            while out.len() < 32 {
                let (Ok(a), Ok(v)) = (half(b, at), half(b, at + 2)) else {
                    break;
                };
                out.push((a, v));
                at += 4;
            }
            out.into()
        };
        let mut out: Font = Font::default();
        for i in 0..ninst {
            let at: usize = word(b, 8 + 4 * i)? as usize;
            if at == 0 {
                out.instruments.push(None);
                continue;
            }
            let (low_to, high_from, release): (u8, u8, u8) = (byte(b, at + 1)?, byte(b, at + 2)?, byte(b, at + 3)?);
            let env: usize = word(b, at + 4)? as usize;
            let low: Option<Sound> = if low_to != 0 { sound(at + 8)? } else { None };
            let mid: Option<Sound> = sound(at + 16)?;
            let high: Option<Sound> = if high_from != 127 { sound(at + 24)? } else { None };
            out.instruments.push(Some(Instrument { low_to, high_from, release, envelope: (env != 0).then(|| envelope(env)), sounds: [low, mid, high] }));
        }
        let drums: usize = word(b, 0)? as usize;
        for i in 0..ndrum {
            let at: usize = if drums == 0 { 0 } else { word(b, drums + 4 * i)? as usize };
            let drum: Option<Drum> = match if at == 0 { None } else { sound(at + 4)? } {
                Some(s) => Some(Drum { release: byte(b, at)?, pan: byte(b, at + 1)?, envelope: envelope(word(b, at + 12)? as usize), sound: s }),
                None => None,
            };
            out.drums.push(drum);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_three_rom_byte_orders_read_the_same_and_other_files_are_refused() {
        let z64: Vec<u8> = vec![0x80, 0x37, 0x12, 0x40, 1, 2, 3, 4];
        assert_eq!(big_endian(vec![0x37, 0x80, 0x40, 0x12, 2, 1, 4, 3]).unwrap(), z64);
        assert_eq!(big_endian(vec![0x40, 0x12, 0x37, 0x80, 4, 3, 2, 1]).unwrap(), z64);
        assert!(big_endian(b"MPQ\x1a....".to_vec()).is_err());
    }

    #[test]
    fn the_player_tables_are_read_from_their_places_and_a_short_file_is_refused() {
        let mut code: Vec<u8> = vec![0; ARGS_TABLE + 80];
        code[VELOCITY_TABLE] = 1;
        code[GATE_TABLE + 15] = 2;
        code[ENVELOPE_TABLE + 13] = 3;
        code[ARGS_TABLE + 79] = 4;
        let t: Tables = tables(&code).unwrap();
        assert_eq!((t.velocity[0], t.gate[15], t.args[79]), (1, 2, 4));
        assert_eq!(t.envelope.to_vec(), vec![(0, 0), (0, 0), (0, 0), (3, 0)]);
        code.pop();
        assert!(tables(&code).is_err());
    }

    #[test]
    fn silence_with_an_empty_book_stays_silent() {
        let out: Vec<f32> = decode(&[0u8; 18], false, &[0; 16]);
        assert_eq!(out, vec![0.0; 32]);
    }
}
