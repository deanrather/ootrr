//! The game's memory on Linux, through `/proc/<pid>/mem` and `/proc/<pid>/maps`. Works for a
//! native Longshot and for one under Wine or Proton, whose Windows addresses are the process's own.

use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use crate::procmaps::{self, Map};

pub use crate::procmaps::{COMMIT, FREE};

const TOP: u64 = 0x8000_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub base: u64,
    pub alloc: u64,
    pub size: u64,
    pub state: u32,
    pub protect: u32,
}

fn names(pid: &str) -> [String; 3] {
    let comm: String = fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim().to_string();
    let exe: String = fs::read_link(format!("/proc/{pid}/exe")).map(|p| p.display().to_string()).unwrap_or_default();
    let arg0: String = fs::read(format!("/proc/{pid}/cmdline")).ok().and_then(|b| b.split(|c| *c == 0).next().map(|a| String::from_utf8_lossy(a).to_string())).unwrap_or_default();
    [comm, exe, arg0]
}

pub fn pids(name: &str) -> Vec<u32> {
    let Ok(dir) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut out: Vec<u32> = Vec::new();
    for e in dir.flatten() {
        let n: String = e.file_name().to_string_lossy().to_string();
        let Ok(pid) = n.parse::<u32>() else { continue };
        let ns: [String; 3] = names(&n);
        let refs: [&str; 3] = [ns[0].as_str(), ns[1].as_str(), ns[2].as_str()];
        if procmaps::names_match(name, &refs) {
            out.push(pid);
        }
    }
    out.sort_unstable();
    out
}

/// Why the game's memory could not be opened, in words a player can act on.
pub fn open_hint(pid: u32) -> String {
    let scope: String = fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope").unwrap_or_default().trim().to_string();
    match File::open(format!("/proc/{pid}/mem")) {
        Ok(_) => "the game is running, but its memory could not be read".to_string(),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && !scope.is_empty() && scope != "0" => format!(
            "Linux does not let other programs read the game's memory (kernel.yama.ptrace_scope is {scope}). Run `sudo sysctl kernel.yama.ptrace_scope=0` in a terminal, which lasts until the next restart, then reconnect"
        ),
        Err(e) => format!("the game's memory could not be opened: {e}. Run the game and the OOTRR Client as the same user, and not in a Flatpak or Snap sandbox"),
    }
}

pub struct Proc {
    pub pid: u32,
    mem: File,
    maps: Vec<Map>,
}

impl Proc {
    pub fn open(pid: u32) -> Option<Proc> {
        let path: String = format!("/proc/{pid}/mem");
        let mem: File = OpenOptions::new().read(true).write(true).open(&path).or_else(|_| File::open(&path)).ok()?;
        let maps: Vec<Map> = procmaps::parse(&fs::read_to_string(format!("/proc/{pid}/maps")).ok()?);
        Some(Proc { pid, mem, maps })
    }

    pub fn read(&self, at: u64, b: &mut [u8]) -> bool {
        self.mem.read_exact_at(b, at).is_ok()
    }

    pub fn write(&self, at: u64, b: &[u8]) -> bool {
        self.mem.write_all_at(b, at).is_ok()
    }

    pub fn query(&self, at: u64) -> Option<Region> {
        procmaps::query(&self.maps, at, TOP).map(|(base, alloc, size, state, protect)| Region { base, alloc, size, state, protect })
    }

    pub fn path(&self) -> Option<PathBuf> {
        fs::read_link(format!("/proc/{}/exe", self.pid)).ok()
    }

    pub fn alive(&self) -> bool {
        PathBuf::from(format!("/proc/{}", self.pid)).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_process_reads_its_own_memory_through_proc() {
        let pid: u32 = std::process::id();
        let p: Proc = Proc::open(pid).expect("open own /proc/self/mem");
        let data: Vec<u8> = (0..64u8).collect();
        let at: u64 = data.as_ptr() as u64;
        let mut back: Vec<u8> = vec![0; data.len()];
        assert!(p.read(at, &mut back));
        assert_eq!(back, data);
        let r: Region = p.query(at).unwrap();
        assert_eq!(r.state, COMMIT);
        assert!(r.base <= at && at < r.base + r.size);
        assert!(p.alive());
        assert!(pids("definitely-not-a-process-name").is_empty());
    }
}
