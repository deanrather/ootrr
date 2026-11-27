pub mod sites;
#[cfg(test)]
pub mod cover;

use std::fs;
use std::path::{Path as P, PathBuf as PB};

use crate::bfevfl::{Bf, Flow};
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::modpack::{Host, Pack};
use crate::pickup_pack::write::start;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst, TABLE_PATH};
use crate::slice::edit::Built;
use crate::slice::support::Plan;
use crate::staging::MARGIN;
use crate::start_items::{Kit, Set};
use crate::zs;
use crate::TITLE_ID;

pub const FLOWS: &str = "Event/EventFlow";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GetType {
    Gift,
    Sale,
}

pub struct OnGet {
    /// The feature's name, shown in the build report and in errors.
    pub name: &'static str,
    /// Whether this build runs the hook: the feature's setting.
    pub enabled: fn(&Plan) -> bool,
    /// Prefix of every event the hook adds, unique to the feature.
    pub event_prefix: &'static str,
    /// Item actor names. Their Pouch_, Obj_ and progressive names are matched too.
    pub items: &'static [&'static str],
    /// Adds the events that should play when Link gets one of the items,
    /// leading on to `next`. Returns the first event added.
    /// `GetType` says whether it is a gift or a sale (rupee counter already open).
    pub add_events: fn(&mut Flow, GetType, next: u16) -> Res<u16>,
    /// If a new game starts with the item already owned, no get plays:
    /// these are the save values to set instead. Empty for none.
    pub start_with: fn(&Kit) -> Vec<Set>,
}

pub static ALL: [&OnGet; 1] = [&crate::full_wallet::ON_GET];

pub fn hooks(p: &Plan) -> Vec<&'static OnGet> {
    ALL.into_iter().filter(|h| (h.enabled)(p)).collect()
}

fn flow_files(base: &P) -> Res<Vec<String>> {
    let mut out: Vec<String> = sites::fixed().iter().map(|f| sites::path_of(f)).collect();
    let dir: PB = local(base, FLOWS);
    if dir.is_dir() {
        for e in fs::read_dir(&dir)? {
            let name: String = e?.file_name().to_string_lossy().to_string();
            if name.ends_with(".bfevfl.zs") {
                out.push(format!("{FLOWS}/{name}"));
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn local(base: &P, path: &str) -> PB {
    path.split('/').fold(base.to_path_buf(), |a, s| a.join(s))
}

pub fn after(hooks: &[&OnGet], romfs: &P, load: &P, mod_name: &str, host: Host, mut b: Built) -> Res<Built> {
    for h in hooks {
        b = apply(h, romfs, load, mod_name, host, b)?;
    }
    Ok(b)
}

fn apply(h: &OnGet, romfs: &P, load: &P, mod_name: &str, host: Host, mut b: Built) -> Res<Built> {
    let mut rf: Romfs = Romfs::open(romfs)?;
    let base: PB = b.root.join("romfs");
    let own: PB = base.join(TABLE_PATH);
    let mut rst: Rst = if own.is_file() { Rst::parse(&rf.zs.decompress(&fs::read(&own)?)?)? } else { rsizetable::load(&mut rf)? };
    let pk: Pack = Pack::create_for(load, TITLE_ID, mod_name, host)?;
    if pk.root != b.root {
        return Err(E::Msg(format!("{}: the event flows went to {} and not to the seed mod {}", h.name, pk.root.display(), b.root.display())));
    }
    let mut lines: Vec<String> = Vec::new();
    for path in flow_files(&base)? {
        let modded: bool = local(&base, &path).is_file();
        let first: Vec<u8> = start(&mut rf, Some(&base), &path)?;
        let mut bf: Bf = Bf::parse(&first)?;
        let mut n: usize = 0;
        for f in bf.flows.iter_mut() {
            n += sites::patch(f, h)?;
        }
        if n == 0 {
            continue;
        }
        let raw: Vec<u8> = bf.write()?;
        let old: usize = if modded { first.len() } else { rf.read_zs(&path)?.len() };
        pk.write_romfs(&path, &zs::compress_raw(&raw, dict_id(&rf.read(&path)?))?)?;
        if raw.len() > old {
            rst.grow(&path, old as u64, raw.len() as u64, MARGIN)?;
        }
        lines.push(format!("{path} ({n})"));
        b.counts.files += usize::from(!modded);
    }
    pk.write_romfs(TABLE_PATH, &rst.to_zs()?)?;
    b.report.push_str(&format!("{}: added at the get points of {} event flows: {}\n", h.name, lines.len(), lines.join(", ")));
    Ok(b)
}
