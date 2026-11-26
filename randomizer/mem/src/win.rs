#![allow(unsafe_code)]

use std::ffi::c_void;
use std::path::PathBuf;

const SNAP_PROCESS: u32 = 0x2;
const ACCESS: u32 = 0x0410;
const WRITE_ACCESS: u32 = 0x0400 | 0x0020 | 0x0008;
const STILL_ACTIVE: u32 = 259;
const BAD_HANDLE: isize = -1;

#[repr(C)]
struct Pe {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    pri: i32,
    flags: u32,
    exe: [u16; 260],
}

#[repr(C)]
#[derive(Default)]
struct Mbi {
    base: usize,
    alloc: usize,
    alloc_protect: u32,
    partition: u16,
    size: usize,
    state: u32,
    protect: u32,
    kind: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
    fn Process32FirstW(h: isize, e: *mut Pe) -> i32;
    fn Process32NextW(h: isize, e: *mut Pe) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn ReadProcessMemory(h: isize, at: *const c_void, buf: *mut c_void, n: usize, got: *mut usize) -> i32;
    fn WriteProcessMemory(h: isize, at: *mut c_void, buf: *const c_void, n: usize, put: *mut usize) -> i32;
    fn VirtualQueryEx(h: isize, at: *const c_void, m: *mut Mbi, n: usize) -> usize;
    fn GetExitCodeProcess(h: isize, code: *mut u32) -> i32;
    fn QueryFullProcessImageNameW(h: isize, flags: u32, name: *mut u16, n: *mut u32) -> i32;
    fn CloseHandle(h: isize) -> i32;
}

pub fn open_hint(_pid: u32) -> String {
    "the game is running but could not be opened. Run the OOTRR Client with the same rights as the game (not one of them as administrator)".to_string()
}

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

pub fn pids(name: &str) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let s: isize = unsafe { CreateToolhelp32Snapshot(SNAP_PROCESS, 0) };
    if s == BAD_HANDLE || s == 0 {
        return out;
    }
    let mut e: Pe = Pe { size: size_of::<Pe>() as u32, usage: 0, pid: 0, heap: 0, module: 0, threads: 0, parent: 0, pri: 0, flags: 0, exe: [0; 260] };
    let mut ok: i32 = unsafe { Process32FirstW(s, &mut e) };
    while ok != 0 {
        let n: usize = e.exe.iter().position(|c| *c == 0).unwrap_or(e.exe.len());
        let exe: String = String::from_utf16_lossy(&e.exe[..n]);
        if exe.eq_ignore_ascii_case(name) {
            out.push(e.pid);
        }
        ok = unsafe { Process32NextW(s, &mut e) };
    }
    unsafe { CloseHandle(s) };
    out
}

pub struct Proc {
    h: isize,
    pub pid: u32,
}

impl Proc {
    pub fn open(pid: u32) -> Option<Proc> {
        let h: isize = unsafe { OpenProcess(ACCESS, 0, pid) };
        if h == 0 || h == BAD_HANDLE { None } else { Some(Proc { h, pid }) }
    }

    pub fn read(&self, at: u64, b: &mut [u8]) -> bool {
        let mut got: usize = 0;
        let ok: i32 = unsafe { ReadProcessMemory(self.h, at as usize as *const c_void, b.as_mut_ptr() as *mut c_void, b.len(), &mut got) };
        ok != 0 && got == b.len()
    }

    pub fn write(&self, at: u64, b: &[u8]) -> bool {
        let h: isize = unsafe { OpenProcess(WRITE_ACCESS, 0, self.pid) };
        if h == 0 || h == BAD_HANDLE {
            return false;
        }
        let mut put: usize = 0;
        let ok: i32 = unsafe { WriteProcessMemory(h, at as usize as *mut c_void, b.as_ptr() as *const c_void, b.len(), &mut put) };
        unsafe { CloseHandle(h) };
        ok != 0 && put == b.len()
    }

    pub fn query(&self, at: u64) -> Option<Region> {
        let mut m: Mbi = Mbi::default();
        let n: usize = unsafe { VirtualQueryEx(self.h, at as usize as *const c_void, &mut m, size_of::<Mbi>()) };
        if n == 0 {
            return None;
        }
        Some(Region { base: m.base as u64, alloc: m.alloc as u64, size: m.size as u64, state: m.state, protect: m.protect })
    }

    pub fn path(&self) -> Option<PathBuf> {
        let mut b: Vec<u16> = vec![0; 1024];
        let mut n: u32 = b.len() as u32;
        let ok: i32 = unsafe { QueryFullProcessImageNameW(self.h, 0, b.as_mut_ptr(), &mut n) };
        if ok == 0 || n == 0 {
            return None;
        }
        Some(PathBuf::from(String::from_utf16_lossy(&b[..n as usize])))
    }

    pub fn alive(&self) -> bool {
        let mut c: u32 = 0;
        let ok: i32 = unsafe { GetExitCodeProcess(self.h, &mut c) };
        ok != 0 && c == STILL_ACTIVE
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.h) };
    }
}
