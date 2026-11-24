use crate::bfevfl::{Bf, Cont, Event, Val};
use crate::error::{Error as E, Res};
use crate::flow_locations::{dict_id, path_of, StagedFile};
use crate::open_world::skips::{BEAN_FILE, BEAN_MINUS, BEAN_SALES, BEAN_SALESMAN};
use crate::romfs::Romfs;
use crate::scrub_prices::{price, Mode, MAX};
use crate::settings::seed::SeedFile;
use crate::settings::Settings;
use crate::staging::{rewrite, Out};
use crate::zs;

pub const SETTING: &str = "merchant_prices";
pub const NUM: u16 = 101;
pub const FIXED: &str = "merchant_price_fixed";
pub const FIXED_NUM: u16 = 102;
const MEDIGORON: &str = "GC Medigoron";
const MEDI_FILE: &str = "Npc_Goron_Medigoron_Adult.bfevfl";
const MEDI_ROWS: [(usize, &str, &str, i32); 3] = [(6, "Event11", "Price", 1), (6, "Event11", "MinusRupeeValue", -1), (27, "Event29", "Value", 1)];

pub fn read(s: &Settings, bad: &mut Vec<String>) -> Mode {
    match s.text(SETTING) {
        Ok("vanilla") => Mode::Vanilla,
        Ok("cheap") => Mode::Cheap,
        Ok("fixed") => match s.int(FIXED) {
            Ok(n) => Mode::Fixed((n as i32).clamp(0, MAX)),
            Err(e) => {
                bad.push(e.to_string());
                Mode::Vanilla
            }
        },
        Ok(v) => {
            bad.push(format!("{SETTING} is {v}, which is not a known choice"));
            Mode::Vanilla
        }
        Err(e) => {
            bad.push(e.to_string());
            Mode::Vanilla
        }
    }
}

fn set_int(f: &mut crate::bfevfl::Flow, index: usize, name: &str, key: &str, v: i32) -> Res<()> {
    let e: &mut Event = f.events.get_mut(index).ok_or_else(|| E::Msg(format!("merchant prices: event index {index} is out of range")))?;
    if e.name != name {
        return Err(E::Msg(format!("merchant prices: event {index} is named {} and not {name}, the game files differ", e.name)));
    }
    if !e.kind.params().is_some_and(|c| c.items.iter().any(|(k, _)| k == key)) {
        // A sale wrapped into tier or bag checks: set every copy of the sale.
        let copies: Vec<usize> = crate::scrub_prices::wrapped_sales(f, index, key);
        if copies.is_empty() {
            return Err(E::Msg(format!("merchant prices: {name} has no number {key}")));
        }
        for i in copies {
            let n: String = f.events[i].name.clone();
            set_int(f, i, &n, key, v)?;
        }
        return Ok(());
    }
    let e: &mut Event = &mut f.events[index];
    let c: &mut Cont = e.kind.params_mut().ok_or_else(|| E::Msg(format!("merchant prices: {name} has no parameters")))?;
    match c.items.iter_mut().find(|(k, _)| k == key) {
        Some((_, Val::Int(x))) => {
            *x = v;
            Ok(())
        }
        _ => Err(E::Msg(format!("merchant prices: {name} has no number {key}"))),
    }
}

fn medigoron_flow(raw: &[u8], p: i32) -> Res<Vec<u8>> {
    let mut b: Bf = Bf::parse(raw)?;
    let f = b.flows.first_mut().ok_or(E::Format("merchant prices: Medigoron's file holds no flow"))?;
    for (i, name, key, sign) in MEDI_ROWS {
        set_int(f, i, name, key, p * sign)?;
    }
    let out: Vec<u8> = b.write()?;
    if Bf::parse(&out)? != b {
        return Err(E::Format("merchant prices: Medigoron's edited flow does not parse back"));
    }
    Ok(out)
}

pub fn stage_medigoron(rf: &mut Romfs, seed: &SeedFile, m: Mode, shuffled: bool, files: &mut Vec<StagedFile>) -> Res<()> {
    let Some(p) = price(m, &seed.seed, MEDIGORON).filter(|_| shuffled) else {
        return Ok(());
    };
    let path: String = path_of(MEDI_FILE);
    match files.iter().position(|x| x.path == path) {
        Some(i) => {
            let id: Option<u32> = dict_id(&files[i].z);
            let raw: Vec<u8> = rf.zs.decompress(&files[i].z)?;
            let out: Vec<u8> = medigoron_flow(&raw, p)?;
            files[i].new = out.len();
            files[i].z = zs::compress_raw(&out, id)?;
        }
        None => {
            let o: Out = rewrite(rf, &path, |raw: &[u8]| medigoron_flow(raw, p))?;
            files.push(StagedFile { path: o.path, z: o.z, old: o.old, new: o.new });
        }
    }
    Ok(())
}

fn bean_flow(raw: &[u8], p: i32) -> Res<Vec<u8>> {
    let mut b: Bf = Bf::parse(raw)?;
    let f = b.flows.first_mut().ok_or(E::Format("merchant prices: the Bean Merchant's file holds no flow"))?;
    for (ev, key) in BEAN_SALES {
        let i: usize = f.events.iter().position(|e| e.name == ev).ok_or_else(|| E::Msg(format!("merchant prices: no event {ev}")))?;
        set_int(f, i, ev, key, p)?;
        if ev == BEAN_MINUS {
            set_int(f, i, ev, "MinusRupeeValue", -p)?;
        }
    }
    let out: Vec<u8> = b.write()?;
    if Bf::parse(&out)? != b {
        return Err(E::Format("merchant prices: the Bean Merchant's edited flow does not parse back"));
    }
    Ok(out)
}

pub fn stage_bean(rf: &mut Romfs, seed: &SeedFile, m: Mode, shuffled: bool, outs: &mut [Out]) -> Res<()> {
    let Some(p) = price(m, &seed.seed, BEAN_SALESMAN).filter(|_| shuffled && m != Mode::Vanilla) else {
        return Ok(());
    };
    let o: &mut Out = outs
        .iter_mut()
        .find(|o| o.path == BEAN_FILE)
        .ok_or(E::Format("merchant prices: the Bean Merchant is shuffled but his file was not written"))?;
    let id: Option<u32> = dict_id(&o.z);
    let raw: Vec<u8> = rf.zs.decompress(&o.z)?;
    let out: Vec<u8> = bean_flow(&raw, p)?;
    o.new = out.len();
    o.z = zs::compress_raw(&out, id)?;
    Ok(())
}
