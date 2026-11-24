use std::collections::{BTreeMap as BM, BTreeSet as BS};

use crate::byml::Byml;
use crate::dungeon_grant::{Grants, Marks};
use crate::error::{Error as E, Res};
use crate::slice::edit::Edited;
use crate::slice::skull_edit::Drop;

pub struct State<'a> {
    pub silver: &'a BM<u64, &'static str>,
    pub silver_used: BS<&'static str>,
    pub silver_hit: BS<u64>,
    pub grants: &'a Grants,
    pub marks: Marks,
    pub skulls: &'a [Drop],
    pub skull_hit: BS<u64>,
    pub day: bool,
    pub day_woke: usize,
    pub qol: crate::qol::Opts,
    pub qol_hits: [usize; 2],
}

impl<'a> State<'a> {
    pub fn new(silver: &'a BM<u64, &'static str>, grants: &'a Grants, skulls: &'a [Drop]) -> State<'a> {
        State { silver, silver_used: BS::new(), silver_hit: BS::new(), grants, marks: Marks::default(), skulls, skull_hit: BS::new(), day: false, day_woke: 0, qol: crate::qol::Opts::default(), qol_hits: [0; 2] }
    }
}

pub type Pass = fn(&str, &[u8], Option<Edited>, &mut State) -> Res<Option<Edited>>;

pub const PASSES: [(&str, Pass); 5] = [("silver rupees", silver), ("dungeon item grants", grants), ("skulltula drops", skulls), ("skulltulas by day", day), ("qol map edits", qol_maps)];

pub type Check = fn(&State) -> Res<()>;

pub const CHECKS: [(&str, Check); 5] = [("every skulltula drop was placed", skulls_found), ("every dungeon item grant was marked", grants_marked), ("every silver rupee was placed", silver_found), ("night skulltulas were found", day_woke), ("every qol map edit was made", qol_found)];

pub fn run(file: &str, raw: &[u8], e: Option<Edited>, st: &mut State) -> Res<Option<Edited>> {
    let mut e: Option<Edited> = e;
    for (_, p) in PASSES {
        e = p(file, raw, e, st)?;
    }
    Ok(e)
}

pub fn finish(st: &State) -> Res<()> {
    for (_, c) in CHECKS {
        c(st)?;
    }
    Ok(())
}

fn silver(file: &str, raw: &[u8], e: Option<Edited>, st: &mut State) -> Res<Option<Edited>> {
    crate::slice::silver::apply(file, raw, e, st.silver, &mut st.silver_used, &mut st.silver_hit)
}

fn grants(file: &str, raw: &[u8], e: Option<Edited>, st: &mut State) -> Res<Option<Edited>> {
    crate::slice::dungeon_items::apply(file, raw, e, st.grants, &mut st.marks)
}

fn skulls(file: &str, raw: &[u8], e: Option<Edited>, st: &mut State) -> Res<Option<Edited>> {
    crate::slice::skull_edit::apply(file, raw, e, st.skulls, &mut st.skull_hit)
}

fn day(file: &str, raw: &[u8], e: Option<Edited>, st: &mut State) -> Res<Option<Edited>> {
    crate::slice::skull_day::apply(file, raw, e, st.day, &mut st.day_woke)
}

fn day_woke(st: &State) -> Res<()> {
    crate::slice::skull_day::all_woke(st.day, st.day_woke)
}

fn skulls_found(st: &State) -> Res<()> {
    crate::slice::skull_edit::all_found(st.skulls, &st.skull_hit)
}

fn grants_marked(st: &State) -> Res<()> {
    crate::dungeon_grant::all_marked(st.grants, &st.marks)
}

fn silver_found(st: &State) -> Res<()> {
    crate::slice::silver::all_found(st.silver, &st.silver_hit)
}

fn qol_maps(file: &str, raw: &[u8], e: Option<Edited>, st: &mut State) -> Res<Option<Edited>> {
    let o: crate::qol::Opts = st.qol;
    if !o.maps() {
        return Ok(e);
    }
    let tower: bool = o.skip_escape && file == crate::qol::tower_escape::ANCHOR_FILE;
    if !tower {
        return Ok(e);
    }
    let mut y: Byml = match &e {
        Some(x) => Byml::parse(&x.bytes)?,
        None => Byml::parse(raw)?,
    };
    let mut n: usize = 0;
    if tower {
        let k: usize = crate::qol::tower_escape::patch_anchor(&mut y);
        st.qol_hits[1] += k;
        n += k;
    }
    if n == 0 {
        return Ok(e);
    }
    let bytes: Vec<u8> = y.write()?;
    Ok(Some(match e {
        Some(mut x) => {
            x.bytes = bytes;
            x
        }
        None => Edited {
            file: file.to_string(),
            old_len: raw.len(),
            bytes,
            hits: Vec::new(),
            used: Vec::new(),
            hearts: Vec::new(),
            wraps: Vec::new(),
            touched: Vec::new(),
        },
    }))
}

fn qol_found(st: &State) -> Res<()> {
    let o: crate::qol::Opts = st.qol;
    let want: [(bool, usize, &str); 2] = [
        (false, 0, "unused"),
        (o.skip_escape, 1, "tower preload anchors"),
    ];
    for (i, (on, n, what)) in want.iter().enumerate() {
        if *on && st.qol_hits[i] != *n {
            return Err(E::Msg(format!("qol: expected {n} {what} in the maps, found {}, the game files differ", st.qol_hits[i])));
        }
    }
    Ok(())
}
