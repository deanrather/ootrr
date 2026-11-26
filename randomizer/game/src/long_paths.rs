use std::fs;
use std::path::{Path as P, PathBuf as PB};

pub const LIMIT: usize = 260;

#[derive(Debug, Clone, PartialEq)]
pub struct Long {
    pub count: usize,
    pub longest: PB,
    pub len: usize,
}

fn units(p: &P) -> usize {
    p.as_os_str().to_string_lossy().encode_utf16().count()
}

fn walk(dir: &P, out: &mut Vec<PB>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p: PB = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

pub fn find(work: &P, root: &P) -> Option<Long> {
    let top: PB = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let mut all: Vec<PB> = Vec::new();
    walk(work, &mut all);
    let long: Vec<(usize, PB)> = all
        .iter()
        .filter_map(|p: &PB| p.strip_prefix(work).ok())
        .map(|r: &P| top.join(r))
        .map(|p: PB| (units(&p), p))
        .filter(|(n, _)| *n >= LIMIT)
        .collect();
    let count: usize = long.len();
    let (len, longest): (usize, PB) = long.into_iter().max_by_key(|(n, _)| *n)?;
    Some(Long { count, longest, len })
}

pub fn message(l: &Long) -> String {
    format!(
        "The mod was not written: {} of its files would have paths of {} characters or more, and some emulators and Windows can't open them. Longest ({} characters): {}\nMove the mods folder nearer the drive root (for example C:\\mods) and build again.",
        l.count,
        LIMIT,
        l.len,
        l.longest.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PB {
        let d: PB = std::path::absolute(std::env::temp_dir().join(format!("ootrr-long-paths-{name}-{}", std::process::id()))).unwrap();
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("work").join("s")).unwrap();
        d
    }

    fn file_of(d: &P, sub: &str, total: usize) -> PB {
        let root: PB = d.join("mod");
        let base: usize = units(&root.join(sub)) + 1;
        let name: String = "a".repeat(total - base);
        fs::write(d.join("work").join(sub).join(&name), b"x").unwrap();
        let p: PB = root.join(sub).join(name);
        assert_eq!(units(&p), total);
        p
    }

    #[test]
    fn under_limit_passes() {
        let d: PB = dir("short");
        file_of(&d, "s", LIMIT - 1);
        assert_eq!(find(&d.join("work"), &d.join("mod")), None);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn at_limit_is_found_with_count_and_longest() {
        let d: PB = dir("long");
        file_of(&d, "s", LIMIT);
        let top: PB = file_of(&d, "s", LIMIT + 2);
        file_of(&d, "s", LIMIT - 5);
        let l: Long = find(&d.join("work"), &d.join("mod")).unwrap();
        assert_eq!(l, Long { count: 2, longest: top.clone(), len: LIMIT + 2 });
        let m: String = message(&l);
        assert!(m.starts_with("The mod was not written: 2 of its files"), "{m}");
        assert!(m.contains("260 characters or more"), "{m}");
        assert!(m.contains(&format!("Longest (262 characters): {}", top.display())), "{m}");
        assert!(m.contains("nearer the drive root"), "{m}");
        let _ = fs::remove_dir_all(&d);
    }
}
