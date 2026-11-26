//! Linux's `/proc/<pid>/maps` read as the Windows memory queries game.rs walks: one region per
//! mapping, gaps as free regions, and contiguous mappings sharing one allocation base, the way a
//! reserved arena does on Windows. Kept apart from the Linux reader so it is tested everywhere.

pub const COMMIT: u32 = 0x1000;
pub const FREE: u32 = 0x10000;
pub const NO_ACCESS: u32 = 0x01;
pub const READ_ONLY: u32 = 0x02;
pub const READ_WRITE: u32 = 0x04;
pub const EXECUTE_READ: u32 = 0x20;
pub const EXECUTE_READ_WRITE: u32 = 0x40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Map {
    pub start: u64,
    pub end: u64,
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

pub fn parse(text: &str) -> Vec<Map> {
    let mut out: Vec<Map> = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(range), Some(perms)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Some((a, b)) = range.split_once('-') else {
            continue;
        };
        let (Ok(start), Ok(end)) = (u64::from_str_radix(a, 16), u64::from_str_radix(b, 16)) else {
            continue;
        };
        if end <= start {
            continue;
        }
        let p: &[u8] = perms.as_bytes();
        out.push(Map { start, end, read: p.first() == Some(&b'r'), write: p.get(1) == Some(&b'w'), exec: p.get(2) == Some(&b'x') });
    }
    out.sort_by_key(|m| m.start);
    out
}

pub fn protect(m: &Map) -> u32 {
    match (m.read, m.write, m.exec) {
        (false, _, _) => NO_ACCESS,
        (true, true, true) => EXECUTE_READ_WRITE,
        (true, false, true) => EXECUTE_READ,
        (true, true, false) => READ_WRITE,
        (true, false, false) => READ_ONLY,
    }
}

/// (base, allocation base, size, state, protect) of the region holding `at`, as VirtualQueryEx
/// gives it. A gap is one free region up to the next mapping or `top`.
pub fn query(maps: &[Map], at: u64, top: u64) -> Option<(u64, u64, u64, u32, u32)> {
    let i: usize = maps.partition_point(|m| m.end <= at);
    match maps.get(i) {
        Some(m) if m.start <= at => {
            let mut alloc: u64 = m.start;
            let mut k: usize = i;
            while k > 0 && maps[k - 1].end == maps[k].start {
                k -= 1;
                alloc = maps[k].start;
            }
            Some((m.start, alloc, m.end - m.start, COMMIT, protect(m)))
        }
        Some(m) => Some((at, at, m.start - at, FREE, NO_ACCESS)),
        None if at < top => Some((at, at, top - at, FREE, NO_ACCESS)),
        None => None,
    }
}

/// True when the process `comm`, `exe` or first argument names `want` (any case, with or without
/// `.exe`): `longshot` natively, `longshot.exe` under Wine or Proton.
pub fn names_match(want: &str, names: &[&str]) -> bool {
    let w: String = want.to_ascii_lowercase();
    let bare: &str = w.strip_suffix(".exe").unwrap_or(&w);
    names.iter().any(|n| {
        let base: String = n.rsplit(['/', '\\']).next().unwrap_or(n).trim().to_ascii_lowercase();
        let base: &str = base.strip_suffix(".exe").unwrap_or(&base);
        !base.is_empty() && base == bare
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAPS: &str = "\
00400000-00452000 r-xp 00000000 08:02 173521      /usr/bin/longshot
00651000-00652000 rw-p 00051000 08:02 173521      /usr/bin/longshot
7f0000000000-7f0080000000 ---p 00000000 00:00 0
7f0080000000-7f0090000000 rw-s 00000000 00:01 77    /memfd:HostMemory (deleted)
7f0090000000-7f8000000000 ---p 00000000 00:00 0
7fff5a000000-7fff5a021000 rw-p 00000000 00:00 0     [stack]
";

    #[test]
    fn maps_parse_into_sorted_regions_with_their_rights() {
        let m: Vec<Map> = parse(MAPS);
        assert_eq!(m.len(), 6);
        assert_eq!(m[0], Map { start: 0x400000, end: 0x452000, read: true, write: false, exec: true });
        assert_eq!(protect(&m[0]), EXECUTE_READ);
        assert_eq!(protect(&m[2]), NO_ACCESS);
        assert_eq!(protect(&m[3]), READ_WRITE);
        assert!(parse("garbage\n12-10 rw-p\n").is_empty());
    }

    #[test]
    fn contiguous_mappings_share_one_allocation_base_and_gaps_are_free() {
        let m: Vec<Map> = parse(MAPS);
        let top: u64 = 0x8000_0000_0000;
        assert_eq!(query(&m, 0x7f0085000000, top), Some((0x7f0080000000, 0x7f0000000000, 0x1000_0000, COMMIT, READ_WRITE)));
        assert_eq!(query(&m, 0x7f0090000000, top).map(|r| r.1), Some(0x7f0000000000));
        assert_eq!(query(&m, 0x500000, top), Some((0x500000, 0x500000, 0x651000 - 0x500000, FREE, NO_ACCESS)));
        assert_eq!(query(&m, 0x7fff5a021000, top), Some((0x7fff5a021000, 0x7fff5a021000, top - 0x7fff5a021000, FREE, NO_ACCESS)));
        assert_eq!(query(&m, top, top), None);
    }

    #[test]
    fn the_game_is_found_natively_and_under_wine() {
        assert!(names_match("longshot.exe", &["longshot"]));
        assert!(names_match("longshot.exe", &["longshot.exe"]));
        assert!(names_match("longshot.exe", &["", "/opt/Longshot/Longshot", ""]));
        assert!(names_match("longshot.exe", &["", "", "Z:\\home\\me\\Games\\Longshot\\longshot.exe"]));
        assert!(!names_match("longshot.exe", &["longshot-launcher", "bash", ""]));
    }
}
