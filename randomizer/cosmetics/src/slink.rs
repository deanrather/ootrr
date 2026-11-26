use std::collections::BTreeMap as BM;

use crate::error::{Error as E, Res};
use crate::rsizetable::hash;

pub const SLINK: &str = "SLink2/slink2.Product.111.bslnk.zs";
const CALL: usize = 0x30;
const TEXT: u8 = 1;
const GROUP: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub user: u32,
    pub key: String,
    pub block: usize,
    pub words: Vec<usize>,
}

pub struct Link {
    pub nt: usize,
    pub names: BM<String, u32>,
    pub calls: Vec<Call>,
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], o: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?)).ok()
}

fn text(b: &[u8], at: usize) -> Option<&str> {
    let tail: &[u8] = b.get(at..)?;
    let z: usize = tail.iter().take(256).position(|c| *c == 0)?;
    std::str::from_utf8(&tail[..z]).ok().filter(|s| !s.is_empty())
}

fn bad() -> E {
    E::Format("slink2: the file is not a version 0x22 XLNK")
}

impl Link {
    pub fn read(b: &[u8]) -> Res<Link> {
        if b.get(..4) != Some(b"XLNK".as_slice()) || u32_at(b, 8) != Some(0x22) {
            return Err(bad());
        }
        let users: usize = u32_at(b, 0x48).ok_or_else(bad)? as usize;
        let nt: usize = u64_at(b, 0x58).filter(|n| *n < b.len()).ok_or_else(bad)?;
        let def: usize = 0x60 + 12 * users;
        let params: usize = def + u32_at(b, def).ok_or_else(bad)? as usize;
        let mut names: BM<String, u32> = BM::new();
        let mut at: usize = nt;
        while at < b.len() {
            let z: usize = b[at..].iter().position(|c| *c == 0).unwrap_or(b.len() - at);
            if let Ok(s) = std::str::from_utf8(&b[at..at + z]) {
                names.entry(s.to_string()).or_insert((at - nt) as u32);
            }
            at += z + 1;
        }
        let mut offs: Vec<(usize, u32)> = Vec::with_capacity(users);
        for i in 0..users {
            offs.push((u64_at(b, 0x60 + 4 * users + 8 * i).ok_or_else(bad)?, u32_at(b, 0x60 + 4 * i).ok_or_else(bad)?));
        }
        let mut ends: Vec<usize> = offs.iter().map(|x| x.0).collect();
        ends.push(nt);
        ends.sort_unstable();
        let mut calls: Vec<Call> = Vec::new();
        for (o, user) in offs {
            let end: usize = ends.iter().copied().find(|e| *e > o).unwrap_or(nt);
            let mut p: usize = o;
            while p + CALL <= end {
                let hit: Option<Call> = (|| {
                    let key: &str = text(b, nt + u32_at(b, p)? as usize)?;
                    if hash(key) != u32_at(b, p + 0x18)? {
                        return None;
                    }
                    let block: usize = params + u32_at(b, p + 0x20)? as usize;
                    let mask: u64 = u64_at(b, block)? as u64;
                    let mut words: Vec<usize> = Vec::new();
                    let bits: Vec<usize> = (0..64).filter(|i| mask >> i & 1 == 1).collect();
                    for (i, bit) in bits.iter().enumerate() {
                        let w: usize = block + 8 + 4 * i;
                        let v: u32 = u32_at(b, w)?;
                        if *bit < 2 && (v >> 24) as u8 == TEXT && text(b, nt + (v & 0xFF_FFFF) as usize) == Some(key) {
                            words.push(w);
                        }
                    }
                    Some(Call { user, key: key.to_string(), block, words })
                })();
                match hit {
                    Some(c) => {
                        calls.push(c);
                        p += CALL;
                    }
                    None => p += 4,
                }
            }
        }
        Ok(Link { nt, names, calls })
    }

    pub fn asset(&self, b: &[u8], c: &Call) -> Option<String> {
        let v: u32 = u32_at(b, c.block + 8)?;
        ((v >> 24) as u8 == TEXT).then(|| text(b, self.nt + (v & 0xFF_FFFF) as usize).map(|s| s.to_string()))?
    }

    fn group_word(&self, b: &[u8], c: &Call) -> Option<usize> {
        let mask: u64 = u64_at(b, c.block)? as u64;
        (mask & 1 == 1 && mask >> GROUP & 1 == 1).then(|| c.block + 8 + 4 * (mask & ((1 << GROUP) - 1)).count_ones() as usize)
    }

    pub fn group(&self, b: &[u8], c: &Call) -> Option<String> {
        let v: u32 = u32_at(b, self.group_word(b, c)?)?;
        ((v >> 24) as u8 == TEXT).then(|| text(b, self.nt + (v & 0xFF_FFFF) as usize).map(|s| s.to_string()))?
    }

    pub fn regroup(&self, b: &mut [u8], from: &str, to: &str, pick: &dyn Fn(&str) -> bool) -> Res<usize> {
        let mut by_word: BM<usize, bool> = BM::new();
        for c in &self.calls {
            let Some(w) = self.group_word(b, c) else {
                continue;
            };
            let wanted: bool = self.group(b, c).as_deref() == Some(from) && self.asset(b, c).is_some_and(|a| pick(&a));
            *by_word.entry(w).or_insert(true) &= wanted;
        }
        let r: u32 = *self.names.get(to).ok_or(E::Format("slink2: the new sound group has no name in the file"))?;
        let mut n: usize = 0;
        for (w, _) in by_word.into_iter().filter(|x| x.1) {
            let v: u32 = ((TEXT as u32) << 24) | r;
            b.get_mut(w..w + 4).ok_or(E::Format("slink2: a parameter leaves the file"))?.copy_from_slice(&v.to_le_bytes());
            n += 1;
        }
        Ok(n)
    }

    pub fn calls_of(&self, user: &str) -> Vec<&Call> {
        let h: u32 = hash(user);
        self.calls.iter().filter(|c| c.user == h).collect()
    }

    pub fn retarget(&self, b: &mut [u8], want: &BM<(String, String), String>) -> Res<usize> {
        let by_user: BM<(u32, &str), &String> = want.iter().map(|((u, k), n)| ((hash(u), k.as_str()), n)).collect();
        let mut by_block: BM<usize, (Vec<usize>, Vec<Option<&String>>)> = BM::new();
        for c in &self.calls {
            let e = by_block.entry(c.block).or_insert((c.words.clone(), Vec::new()));
            e.1.push(by_user.get(&(c.user, c.key.as_str())).copied());
        }
        let mut n: usize = 0;
        for (words, all) in by_block.into_values() {
            let Some(Some(new)) = all.first() else {
                continue;
            };
            if words.is_empty() || all.iter().any(|x| *x != Some(*new)) {
                continue;
            }
            let Some(r) = self.names.get(*new) else {
                continue;
            };
            for w in words {
                let v: u32 = ((TEXT as u32) << 24) | r;
                b.get_mut(w..w + 4).ok_or(E::Format("slink2: a parameter leaves the file"))?.copy_from_slice(&v.to_le_bytes());
            }
            n += 1;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn fixture(users: &[(&str, &[&str])]) -> Vec<u8> {
        let mut names: Vec<u8> = Vec::new();
        let mut pos: BM<String, u32> = BM::new();
        let mut name = |s: &str, names: &mut Vec<u8>| -> u32 {
            *pos.entry(s.to_string()).or_insert_with(|| {
                let at: u32 = names.len() as u32;
                names.extend_from_slice(s.as_bytes());
                names.push(0);
                at
            })
        };
        let n: usize = users.len();
        let def: usize = 0x60 + 12 * n;
        let params: usize = def + 16;
        let mut blocks: Vec<u8> = Vec::new();
        let mut body: Vec<Vec<u8>> = Vec::new();
        for (_, keys) in users {
            let mut u: Vec<u8> = vec![0; 0x10];
            for (i, k) in keys.iter().enumerate() {
                let r: u32 = name(k, &mut names);
                let bp: u32 = blocks.len() as u32;
                blocks.extend_from_slice(&0b100011u64.to_le_bytes());
                for v in [(1u32 << 24) | r, (1u32 << 24) | r, 6] {
                    blocks.extend_from_slice(&v.to_le_bytes());
                }
                blocks.extend_from_slice(&[0; 4]);
                let mut c: Vec<u8> = vec![0; CALL];
                c[..4].copy_from_slice(&r.to_le_bytes());
                c[8..12].copy_from_slice(&(i as u32).to_le_bytes());
                c[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
                c[0x18..0x1C].copy_from_slice(&hash(k).to_le_bytes());
                c[0x20..0x24].copy_from_slice(&bp.to_le_bytes());
                u.extend_from_slice(&c);
            }
            body.push(u);
        }
        let users_at: usize = params + blocks.len();
        let mut at: usize = users_at;
        let mut offs: Vec<usize> = Vec::new();
        for u in &body {
            offs.push(at);
            at += u.len();
        }
        let nt: usize = at;
        let mut b: Vec<u8> = vec![0; 0x60];
        b[..4].copy_from_slice(b"XLNK");
        b[8..12].copy_from_slice(&0x22u32.to_le_bytes());
        b[0x48..0x4C].copy_from_slice(&(n as u32).to_le_bytes());
        b[0x58..0x60].copy_from_slice(&(nt as u64).to_le_bytes());
        for (u, _) in users {
            b.extend_from_slice(&hash(u).to_le_bytes());
        }
        for o in &offs {
            b.extend_from_slice(&(*o as u64).to_le_bytes());
        }
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&[0; 12]);
        b.extend_from_slice(&blocks);
        for u in body {
            b.extend_from_slice(&u);
        }
        b.extend_from_slice(&names);
        b
    }

    #[test]
    fn calls_are_found_and_a_retarget_changes_only_the_asset_name() {
        let mut b: Vec<u8> = fixture(&[("BGM_A", &["Key_A", "Key_A_Intro"]), ("BGM_B", &["Key_B"]), ("BGM_C", &["Key_B"])]);
        let l: Link = Link::read(&b).unwrap();
        assert_eq!(l.calls.len(), 4);
        assert_eq!(l.calls_of("BGM_A").iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), vec!["Key_A", "Key_A_Intro"]);
        assert!(l.calls.iter().all(|c| c.words.len() == 2));
        let len: usize = b.len();
        let want: BM<(String, String), String> = [(("BGM_A".to_string(), "Key_A".to_string()), "Key_B".to_string())].into_iter().collect();
        assert_eq!(l.retarget(&mut b, &want).unwrap(), 1);
        assert_eq!(b.len(), len);
        let again: Link = Link::read(&b).unwrap();
        let a: &Call = again.calls_of("BGM_A")[0];
        assert_eq!(a.key, "Key_A");
        assert!(a.words.is_empty());
        let nt: usize = u64_at(&b, 0x58).unwrap();
        let v: u32 = u32_at(&b, a.block + 8).unwrap();
        assert_eq!(text(&b, nt + (v & 0xFF_FFFF) as usize), Some("Key_B"));
        assert_eq!(again.calls_of("BGM_A")[1].words.len(), 2);
    }

    #[test]
    fn a_block_two_users_disagree_on_is_left_alone() {
        let mut b: Vec<u8> = fixture(&[("BGM_A", &["Key_A"]), ("BGM_B", &["Key_B"])]);
        let mut l: Link = Link::read(&b).unwrap();
        let shared: usize = l.calls[0].block;
        l.calls[1].block = shared;
        l.calls[1].words = l.calls[0].words.clone();
        l.calls[1].key = "Key_A".to_string();
        let want: BM<(String, String), String> = [(("BGM_A".to_string(), "Key_A".to_string()), "Key_B".to_string())].into_iter().collect();
        assert_eq!(l.retarget(&mut b, &want).unwrap(), 0);
        let both: BM<(String, String), String> = [(("BGM_A".to_string(), "Key_A".to_string()), "Key_B".to_string()), (("BGM_B".to_string(), "Key_A".to_string()), "Key_B".to_string())].into_iter().collect();
        assert_eq!(l.retarget(&mut b, &both).unwrap(), 1);
    }
}
