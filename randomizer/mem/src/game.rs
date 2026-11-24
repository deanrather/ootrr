use ootrr_core::error::{Error as E, Res};

use crate::win::{pids, Proc, Region, COMMIT, FREE};

pub const EXE: &str = "longshot.exe";
const MOD0: [u8; 16] = [0x4d, 0x4f, 0x44, 0x30, 0x54, 0x67, 0x03, 0x01, 0x24, 0xfb, 0x0f, 0x01, 0x6c, 0x34, 0x25, 0x01];
const MOD0_AT: u64 = 0x03c4_705c;
const IMAGE: u64 = 0x8000_0000;
const IMAGE_SPAN: u64 = 0x1000_0000;
const MIN_ARENA: u64 = 0x10_0000_0000;
const TOP: u64 = 0x8000_0000_0000;
const CHUNK: usize = 0x40_0000;
const READABLE: u32 = 0x02 | 0x04 | 0x08 | 0x20 | 0x40 | 0x80;

pub struct Game {
    p: Proc,
    pub arena: u64,
    pub main: u64,
}

fn arenas(p: &Proc) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    let mut at: u64 = 0;
    let mut cur: Option<(u64, u64)> = None;
    while at < TOP {
        let Some(r): Option<Region> = p.query(at) else {
            break;
        };
        if r.size == 0 {
            break;
        }
        let end: u64 = r.base + r.size;
        if r.state != FREE {
            cur = match cur {
                Some((a, _)) if a == r.alloc => Some((a, end)),
                _ => {
                    if let Some((a, e)) = cur
                        && e - a >= MIN_ARENA
                    {
                        out.push(a);
                    }
                    Some((r.alloc, end))
                }
            };
        }
        at = end;
    }
    if let Some((a, e)) = cur
        && e - a >= MIN_ARENA
    {
        out.push(a);
    }
    out
}

fn header_ok(p: &Proc, arena: u64, base: u64) -> bool {
    let mut b: [u8; 8] = [0; 8];
    p.read(arena + base, &mut b)
        && u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 1
        && u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as u64 == MOD0_AT
}

fn main_base(p: &Proc, arena: u64) -> Option<u64> {
    let mut at: u64 = arena + IMAGE;
    let end: u64 = arena + IMAGE + IMAGE_SPAN;
    while at < end {
        let r: Region = p.query(at)?;
        if r.size == 0 {
            return None;
        }
        if r.state == COMMIT && r.protect & READABLE != 0 {
            let stop: u64 = (r.base + r.size).min(end);
            let mut o: u64 = r.base.max(at);
            while o < stop {
                let n: usize = ((stop - o) as usize).min(CHUNK + MOD0.len());
                let mut b: Vec<u8> = vec![0; n];
                if p.read(o, &mut b) {
                    let mut i: usize = 0;
                    while i + MOD0.len() <= b.len() {
                        if b[i..i + MOD0.len()] == MOD0 {
                            let hit: u64 = o + i as u64 - arena;
                            if let Some(base) = hit.checked_sub(MOD0_AT)
                                && header_ok(p, arena, base)
                            {
                                return Some(base);
                            }
                        }
                        i += 4;
                    }
                }
                o += CHUNK as u64;
            }
        }
        at = r.base + r.size;
    }
    None
}

impl Game {
    pub fn attach() -> Res<Game> {
        let ids: Vec<u32> = pids(EXE);
        if ids.is_empty() {
            return Err(E::Msg("game not running".to_string()));
        }
        for pid in ids {
            let Some(p): Option<Proc> = Proc::open(pid) else {
                continue;
            };
            for a in arenas(&p) {
                if let Some(m) = main_base(&p, a) {
                    return Ok(Game { p, arena: a, main: m });
                }
            }
        }
        Err(E::Msg("the game is starting, its memory is not ready yet".to_string()))
    }

    pub fn alive(&self) -> bool {
        self.p.alive()
    }

    pub fn exe(&self) -> Option<std::path::PathBuf> {
        self.p.path()
    }

    pub fn pid(&self) -> u32 {
        self.p.pid
    }

    pub fn read(&self, at: u64, n: usize) -> Option<Vec<u8>> {
        let mut b: Vec<u8> = vec![0; n];
        if self.p.read(self.arena.checked_add(at)?, &mut b) { Some(b) } else { None }
    }

    pub fn write(&self, at: u64, b: &[u8]) -> bool {
        match self.arena.checked_add(at) {
            Some(a) => self.p.write(a, b),
            None => false,
        }
    }

    pub fn u64_at(&self, at: u64) -> Option<u64> {
        let b: Vec<u8> = self.read(at, 8)?;
        Some(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
}
