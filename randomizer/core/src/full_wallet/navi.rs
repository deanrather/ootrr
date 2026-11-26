use crate::error::{Error as E, Res};

pub const FILE: &str = "GameParameter/NaviMgrGlobalParam/GlobalGameParameterTable.game__navi__NaviMgrGlobalParam.bgyml";
pub const LIST: &str = "BalloonMessageParamList";
pub const FULL: &str = "/MaxRupee000.game__navi__NaviBalloonMessageParam.gyml";
const STR: u8 = 0xA0;
const MAP: u8 = 0xC1;
const HASH: u8 = 0x20;

fn cut() -> E {
    E::Format("full wallets: the Navi table is truncated")
}

fn u32_at(b: &[u8], o: usize) -> Res<u32> {
    let s: &[u8] = b.get(o..o + 4).ok_or_else(cut)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn count(b: &[u8], o: usize) -> Res<usize> {
    Ok((u32_at(b, o)? >> 8) as usize)
}

fn kind(b: &[u8], o: usize) -> Res<u8> {
    b.get(o).copied().ok_or_else(cut)
}

fn table(b: &[u8], o: usize) -> Res<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    if o == 0 {
        return Ok(out);
    }
    for i in 0..count(b, o)? {
        let a: usize = o + u32_at(b, o + 4 + 4 * i)? as usize;
        let tail: &[u8] = b.get(a..).ok_or_else(cut)?;
        let z: usize = tail.iter().position(|c| *c == 0).ok_or_else(cut)?;
        out.push(String::from_utf8_lossy(&tail[..z]).to_string());
    }
    Ok(out)
}

pub fn spot(b: &[u8]) -> Res<(usize, u32)> {
    if b.get(..2) != Some(b"YB") {
        return Err(E::Format("full wallets: the Navi table is not a little endian byml"));
    }
    let keys: Vec<String> = table(b, u32_at(b, 4)? as usize)?;
    let strs: Vec<String> = table(b, u32_at(b, 8)? as usize)?;
    let root: usize = u32_at(b, 12)? as usize;
    if kind(b, root)? != MAP {
        return Err(E::Format("full wallets: the Navi table's root is not a map"));
    }
    let mut list: Option<usize> = None;
    for i in 0..count(b, root)? {
        let p: usize = root + 4 + 8 * i;
        let k: usize = (u32_at(b, p)? & 0x00FF_FFFF) as usize;
        if keys.get(k).map(|s| s.as_str()) == Some(LIST) && kind(b, p + 3)? == HASH {
            list = Some(u32_at(b, p + 4)? as usize);
        }
    }
    let at: usize = list.ok_or(E::Format("full wallets: the Navi table has no balloon list"))?;
    let n: usize = count(b, at)?;
    let mut hits: Vec<usize> = Vec::new();
    for i in 0..n {
        let v: usize = at + 4 + 8 * i + 4;
        let s: Option<&String> = strs.get(u32_at(b, v)? as usize);
        if kind(b, at + 4 + 8 * n + i)? == STR && s.is_some_and(|s| s.ends_with(FULL)) {
            hits.push(v);
        }
    }
    let empty: u32 = strs.iter().position(|s| s.is_empty()).and_then(|i| u32::try_from(i).ok()).ok_or(E::Format("full wallets: the Navi table has no empty name"))?;
    match hits.as_slice() {
        [v] => Ok((*v, empty)),
        _ => Err(E::Msg(format!("full wallets: the Navi table lists the full wallet balloon {} times, expected once", hits.len()))),
    }
}

pub fn quiet(b: &[u8]) -> Res<Vec<u8>> {
    let (at, empty): (usize, u32) = spot(b)?;
    let mut out: Vec<u8> = b.to_vec();
    out[at..at + 4].copy_from_slice(&empty.to_le_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real() -> Option<Vec<u8>> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Some(crate::romfs::Romfs::open(&p).unwrap().read(FILE).unwrap())
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    #[test]
    fn only_the_full_wallet_balloon_is_blanked_like_the_game_blanks_others() {
        let Some(b) = real() else {
            return;
        };
        let out: Vec<u8> = quiet(&b).unwrap();
        assert_eq!(out.len(), b.len());
        let (at, _): (usize, u32) = spot(&b).unwrap();
        let diff: Vec<usize> = (0..b.len()).filter(|&i| b[i] != out[i]).collect();
        assert!(!diff.is_empty() && diff.iter().all(|i| (at..at + 4).contains(i)), "{diff:?}");
        assert!(quiet(&out).is_err());
        let keys: Vec<String> = table(&b, u32_at(&b, 4).unwrap() as usize).unwrap();
        assert!(keys.iter().any(|k| k == LIST));
        let strs: Vec<String> = table(&b, u32_at(&b, 8).unwrap() as usize).unwrap();
        assert_eq!(strs.iter().filter(|s| s.ends_with(FULL)).count(), 1);
    }

    #[test]
    fn the_navi_table_is_unpacked_from_a_game_package() {
        let d: crate::slice::data::Data = crate::slice::data::Data::load().unwrap();
        assert!(crate::required::unpack(&d, false).wants(FILE));
    }
}
