#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub base: u64,
    pub alloc: u64,
    pub size: u64,
    pub state: u32,
    pub protect: u32,
}

pub const COMMIT: u32 = 0x1000;
pub const FREE: u32 = 0x10000;

pub fn pids(_name: &str) -> Vec<u32> {
    Vec::new()
}

pub struct Proc {
    pub pid: u32,
}

impl Proc {
    pub fn open(_pid: u32) -> Option<Proc> {
        None
    }

    pub fn read(&self, _at: u64, _b: &mut [u8]) -> bool {
        false
    }

    pub fn write(&self, _at: u64, _b: &[u8]) -> bool {
        false
    }

    pub fn query(&self, _at: u64) -> Option<Region> {
        None
    }

    pub fn path(&self) -> Option<std::path::PathBuf> {
        None
    }

    pub fn alive(&self) -> bool {
        false
    }
}
