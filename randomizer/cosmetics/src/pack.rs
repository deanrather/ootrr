use std::fs;
use std::path::Path as P;

use crate::error::{Error as E, Res};
use crate::inflate;

const ZLIB: u8 = 0x02;
const COMPRESSED: u32 = 0x200;
const IMPLODED: u32 = 0x100;
const ENCRYPTED: u32 = 0x1_0000;
const SINGLE: u32 = 0x100_0000;
const FILE_MAX: usize = 4 << 20;
const RESOURCE_HEAD: usize = 64;
const SEQUENCE_KIND: &[u8] = b"QESO";
const FOLDER: &str = "custom/music/";
const BAD: E = E::Format("custom music: the pack is damaged or is not an .otr file");

fn crypt_table() -> Vec<u32> {
    let mut t: Vec<u32> = vec![0; 0x500];
    let mut seed: u32 = 0x0010_0001;
    for i in 0..0x100 {
        for k in 0..5 {
            seed = (seed * 125 + 3) % 0x2A_AAAB;
            let high: u32 = (seed & 0xFFFF) << 16;
            seed = (seed * 125 + 3) % 0x2A_AAAB;
            t[i + k * 0x100] = high | (seed & 0xFFFF);
        }
    }
    t
}

fn hash(t: &[u32], name: &[u8], kind: usize) -> u32 {
    let mut a: u32 = 0x7FED_7FED;
    let mut b: u32 = 0xEEEE_EEEE;
    for c in name {
        let c: u32 = u32::from(c.to_ascii_uppercase());
        a = t[(kind << 8) + c as usize] ^ a.wrapping_add(b);
        b = c.wrapping_add(a).wrapping_add(b).wrapping_add(b << 5).wrapping_add(3);
    }
    a
}

fn decrypt(t: &[u32], data: &[u8], mut key: u32) -> Vec<u32> {
    let mut seed: u32 = 0xEEEE_EEEE;
    data.chunks_exact(4)
        .map(|w| {
            seed = seed.wrapping_add(t[0x400 + (key & 0xFF) as usize]);
            let v: u32 = u32::from_le_bytes([w[0], w[1], w[2], w[3]]) ^ key.wrapping_add(seed);
            key = (!key << 21).wrapping_add(0x1111_1111) | (key >> 11);
            seed = v.wrapping_add(seed).wrapping_add(seed << 5).wrapping_add(3);
            v
        })
        .collect()
}

fn le(b: &[u8], at: usize) -> Res<u32> {
    let w: &[u8] = b.get(at..at + 4).ok_or(BAD)?;
    Ok(u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

struct Archive {
    data: Vec<u8>,
    sector: usize,
    table: Vec<u32>,
    hashes: Vec<u32>,
    blocks: Vec<u32>,
}

impl Archive {
    fn open(data: Vec<u8>) -> Res<Archive> {
        if data.get(..4) != Some(b"MPQ\x1a") {
            return Err(BAD);
        }
        let shift: u32 = le(&data, 12)? >> 16;
        if shift > 12 {
            return Err(BAD);
        }
        let table: Vec<u32> = crypt_table();
        let part = |at: usize, count: usize, key: &str| -> Res<Vec<u32>> {
            let from: usize = le(&data, at)? as usize;
            let bytes: &[u8] = data.get(from..from.saturating_add(count.saturating_mul(16))).ok_or(BAD)?;
            Ok(decrypt(&table, bytes, hash(&table, key.as_bytes(), 3)))
        };
        let hashes: Vec<u32> = part(16, le(&data, 24)? as usize, "(HASH TABLE)")?;
        let blocks: Vec<u32> = part(20, le(&data, 28)? as usize, "(BLOCK TABLE)")?;
        Ok(Archive { sector: 512 << shift, table, hashes, blocks, data })
    }

    fn read(&self, name: &[u8]) -> Res<Option<Vec<u8>>> {
        let n: usize = self.hashes.len() / 4;
        if n == 0 {
            return Err(BAD);
        }
        let (a, b): (u32, u32) = (hash(&self.table, name, 1), hash(&self.table, name, 2));
        let start: usize = hash(&self.table, name, 0) as usize % n;
        for k in 0..n {
            let row: &[u32] = &self.hashes[(start + k) % n * 4..][..4];
            if row[3] == 0xFFFF_FFFF {
                break;
            }
            if row[0] == a && row[1] == b {
                return self.block(row[3] as usize).map(Some);
            }
        }
        Ok(None)
    }

    fn block(&self, index: usize) -> Res<Vec<u8>> {
        let row: &[u32] = self.blocks.get(index * 4..index * 4 + 4).ok_or(BAD)?;
        let (from, packed, size, flags): (usize, usize, usize, u32) = (row[0] as usize, row[1] as usize, row[2] as usize, row[3]);
        if size > FILE_MAX || flags & (ENCRYPTED | IMPLODED) != 0 {
            return Err(E::Format("custom music: the pack stores a file in a way that is not supported"));
        }
        let raw: &[u8] = self.data.get(from..from.saturating_add(packed)).ok_or(BAD)?;
        if flags & COMPRESSED == 0 {
            return Ok(raw.get(..size).ok_or(BAD)?.to_vec());
        }
        if flags & SINGLE != 0 {
            return unpack(raw, size);
        }
        let count: usize = size.div_ceil(self.sector);
        let mut out: Vec<u8> = Vec::with_capacity(size);
        for k in 0..count {
            let (a, b): (usize, usize) = (le(raw, 4 * k)? as usize, le(raw, 4 * k + 4)? as usize);
            out.extend(unpack(raw.get(a..b).ok_or(BAD)?, self.sector.min(size - out.len()))?);
        }
        Ok(out)
    }
}

fn unpack(chunk: &[u8], size: usize) -> Res<Vec<u8>> {
    if chunk.len() == size {
        return Ok(chunk.to_vec());
    }
    if chunk.first() != Some(&ZLIB) {
        return Err(E::Format("custom music: the pack uses a compression that is not supported"));
    }
    inflate::zlib(&chunk[1..], size)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tune {
    pub name: String,
    pub fanfare: bool,
    pub data: Vec<u8>,
    pub fonts: Vec<u8>,
}

pub fn is_fanfare(path: &str) -> bool {
    path.rsplit('_').next().is_some_and(|t| t.eq_ignore_ascii_case("fanfare"))
}

pub fn label(path: &str) -> String {
    let file: &str = path.rsplit('/').next().unwrap_or(path);
    file.rsplit_once('_').map_or(file, |x| x.0).to_string()
}

pub fn sequence(path: &str, raw: &[u8]) -> Option<Tune> {
    if raw.get(4..8) != Some(SEQUENCE_KIND) {
        return None;
    }
    let size: usize = u32::from_le_bytes(raw.get(RESOURCE_HEAD..RESOURCE_HEAD + 4)?.try_into().ok()?) as usize;
    let from: usize = RESOURCE_HEAD + 4;
    let data: &[u8] = raw.get(from..from.checked_add(size)?)?;
    let rest: &[u8] = raw.get(from + size..)?;
    let count: usize = u32::from_le_bytes(rest.get(3..7)?.try_into().ok()?) as usize;
    if count == 0 || count > 16 {
        return None;
    }
    let fonts: Vec<u8> = rest.get(7..7 + count)?.to_vec();
    Some(Tune { name: label(path), fanfare: is_fanfare(path), data: data.to_vec(), fonts })
}

pub fn read(file: &P) -> Res<Vec<Tune>> {
    let data: Vec<u8> = fs::read(file).map_err(|e| E::Msg(format!("custom music: {} could not be read: {e}", file.display())))?;
    let archive: Archive = Archive::open(data)?;
    let list: Vec<u8> = archive.read(b"(listfile)")?.ok_or(E::Format("custom music: the pack has no file list"))?;
    let mut out: Vec<Tune> = Vec::new();
    for line in list.split(|c| *c == b'\r' || *c == b'\n').filter(|l| l.starts_with(FOLDER.as_bytes())) {
        let Some(raw) = archive.read(line)? else {
            continue;
        };
        out.extend(sequence(&String::from_utf8_lossy(line), &raw));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    if out.is_empty() {
        return Err(E::Format("custom music: the pack holds no Ship of Harkinian music"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_underscore_word_says_whether_a_tune_is_a_fanfare() {
        assert!(is_fanfare("custom/music/Game - Win_Fanfare"));
        assert!(!is_fanfare("custom/music/Game - Fanfare Field_bgm"));
        assert_eq!(label("custom/music/A_B - Town_bgm"), "A_B - Town");
    }

    #[test]
    fn a_sequence_resource_gives_its_notes_and_instrument_sets_and_a_short_one_is_left_out() {
        let mut raw: Vec<u8> = vec![0; RESOURCE_HEAD];
        raw[4..8].copy_from_slice(SEQUENCE_KIND);
        raw.extend(3u32.to_le_bytes());
        raw.extend([0xD3, 0x20, 0xFF]);
        raw.extend([0, 2, 1]);
        raw.extend(1u32.to_le_bytes());
        raw.push(3);
        let t: Tune = sequence("custom/music/x_bgm", &raw).unwrap();
        assert_eq!((t.data, t.fonts, t.fanfare), (vec![0xD3, 0x20, 0xFF], vec![3], false));
        assert_eq!(sequence("custom/music/x_bgm", &raw[..raw.len() - 1]), None);
    }
}
