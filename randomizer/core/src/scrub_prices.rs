use crate::bfevfl::{Bf, Cont, Event, Val};
use crate::dungeon_text::{is_english, NAMES};
use crate::error::{Error as E, Res};
use crate::flow_locations::{dict_id, path_of, StagedFile};
use crate::msbt::{self, Msbt, Tok};
use crate::romfs::Romfs;
use crate::sarc;
use crate::settings::seed::SeedFile;
use crate::settings::Settings;
use crate::slice::data::Data;
use crate::staging::{rewrite, Out};
use crate::zs;

pub const SETTING: &str = "scrub_prices";
pub const NUM: u16 = 98;
pub const FIXED: &str = "scrub_price_fixed";
pub const FIXED_NUM: u16 = 99;
pub const HINT: &str = "scrub_hint_text";
pub const HINT_NUM: u16 = 100;
pub const MAX: i32 = 99;
const CAT: &str = "scrub_sale";
const FILE: &str = "Enemy_Scrub_Business_Common.bfevfl";
const PRICE: &str = "Price";
const SAYS: &str = "SellerMessageId";
const MSG_FILE: &str = "EventFlowMsg/Enemy_Event.msbt";
const MSG_ID: &str = "EventFlowMsg/Enemy_Event";
const LABEL: &str = "OOTRR_Scrub_";
const PRICE_TAG: &str = "{2.12:100053696d706c6553686f705f5072696365ffff}";
const MYSTERY: &str = "Mystery Item";
const MYSTERY_OTHER: &str = "???";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Vanilla,
    Cheap,
    Fixed(i32),
}

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

pub fn read_hint(s: &Settings, bad: &mut Vec<String>) -> bool {
    s.flag(HINT).unwrap_or_else(|e| {
        bad.push(e.to_string());
        false
    })
}

pub fn price(m: Mode, seed: &str, loc: &str) -> Option<i32> {
    match m {
        Mode::Vanilla => None,
        Mode::Fixed(n) => Some(n),
        Mode::Cheap => {
            let a: u32 = crate::murmur::murmur3_32(format!("{seed}|{loc}|a").as_bytes(), 0x5c2b);
            let b: u32 = crate::murmur::murmur3_32(format!("{seed}|{loc}|b").as_bytes(), 0x5c2b);
            Some(((a % 20).min(b % 20) * 5) as i32)
        }
    }
}

fn label(loc: &str) -> String {
    let key: String = loc.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!("{LABEL}{key}")
}

struct Sale {
    label: String,
    actor: Option<String>,
    price: Option<i32>,
    events: Vec<(usize, String)>,
}

fn sales(d: &Data, seed: &SeedFile, m: Mode) -> Vec<Sale> {
    let mut out: Vec<Sale> = Vec::new();
    for (name, item) in &seed.placements {
        let Some(l) = d.flow.by_name(name).filter(|l| l.cat == CAT) else {
            continue;
        };
        let events: Vec<(usize, String)> = l.edits.iter().filter(|e| e.file == FILE).map(|e| (e.index, e.event.clone())).collect();
        if events.is_empty() {
            continue;
        }
        out.push(Sale { label: label(name), actor: d.item(item).and_then(|i| i.actor.clone()), price: price(m, &seed.seed, name), events });
    }
    out
}

/// Sets one parameter of a scrub sale event. When Menu scrubs as one time sales (crate::scrubs_b)
/// already turned the shared one time sale into a call with arguments, the value goes to the rows

/// The events a sale event became when its item was wrapped into tier or bag checks
/// (crate::flow_wrap): every event reachable from it, within a few steps, that carries `key`.
pub fn wrapped_sales(f: &crate::bfevfl::Flow, at: usize, key: &str) -> Vec<usize> {
    use crate::bfevfl::Kind;
    let mut out: Vec<usize> = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    let mut todo: Vec<(usize, usize)> = vec![(at, 0)];
    while let Some((i, depth)) = todo.pop() {
        if i >= f.events.len() || seen.contains(&i) || depth > 6 {
            continue;
        }
        seen.push(i);
        let k: &Kind = &f.events[i].kind;
        if i != at && k.params().is_some_and(|c| c.items.iter().any(|(n, _)| n == key)) {
            out.push(i);
            continue;
        }
        let next: Vec<u16> = match k {
            Kind::Switch { cases, .. } | Kind::SubSwitch { cases, .. } => cases.iter().map(|c| c.ev).collect(),
            Kind::Action { next, .. } | Kind::Sub { next, .. } | Kind::Join { next } => vec![*next],
            Kind::Fork { join, forks } => forks.iter().copied().chain([*join]).collect(),
            Kind::Label { .. } => Vec::new(),
        };
        todo.extend(next.into_iter().filter(|n| *n != crate::bfevfl::flow::NONE).map(|n| (usize::from(n), depth + 1)));
    }
    out
}

/// that pass that argument instead.
fn set(f: &mut crate::bfevfl::Flow, index: usize, name: &str, key: &str, v: Val) -> Res<()> {
    let e: &mut Event = f.events.get_mut(index).ok_or_else(|| E::Msg(format!("scrub sales: event index {index} is out of range")))?;
    if e.name != name {
        return Err(E::Msg(format!("scrub sales: event {index} is named {} and not {name}, the game files differ", e.name)));
    }
    if !e.kind.params().is_some_and(|c| c.items.iter().any(|(k, _)| k == key)) {
        // A sale wrapped into tier or bag checks: set every copy of the sale.
        let copies: Vec<usize> = wrapped_sales(f, index, key);
        if copies.is_empty() {
            return Err(E::Msg(format!("scrub sales: {name} has no {key}")));
        }
        for i in copies {
            let n: String = f.events[i].name.clone();
            set(f, i, &n, key, v.clone())?;
        }
        return Ok(());
    }
    let e: &mut Event = &mut f.events[index];
    let c: &mut Cont = e.kind.params_mut().ok_or_else(|| E::Msg(format!("scrub sales: {name} has no parameters")))?;
    let slot: &mut Val = c.items.iter_mut().find(|(k, _)| k == key).map(|(_, x)| x).ok_or_else(|| E::Msg(format!("scrub sales: {name} has no {key}")))?;
    match (slot, &v) {
        (Val::Arg(a), _) if crate::scrubs_b::is_sale(index, name) => {
            let a: String = a.clone();
            crate::scrubs_b::set_rows(f, &a, &v).map_err(|e| E::Msg(format!("scrub sales: {name} passes {key} as {a}, {e}")))
        }
        (x @ Val::Int(_), Val::Int(_)) | (x @ Val::Str(_), Val::Str(_)) => {
            *x = v;
            Ok(())
        }
        _ => Err(E::Msg(format!("scrub sales: {name} has no {key} of the expected kind"))),
    }
}

fn edit_flow(raw: &[u8], sales: &[Sale]) -> Res<Vec<u8>> {
    let mut b: Bf = Bf::parse(raw)?;
    let f = b.flows.first_mut().ok_or(E::Format("scrub sales: the flow file holds no flow"))?;
    for s in sales {
        for (index, name) in &s.events {
            if let Some(p) = s.price {
                set(f, *index, name, PRICE, Val::Int(p))?;
            }
            set(f, *index, name, SAYS, Val::Str(format!("{MSG_ID}:{}", s.label)))?;
        }
    }
    let out: Vec<u8> = b.write()?;
    if Bf::parse(&out)? != b {
        return Err(E::Format("scrub sales: the edited flow does not parse back"));
    }
    Ok(out)
}

pub fn stage(rf: &mut Romfs, d: &Data, seed: &SeedFile, m: Mode, files: &mut Vec<StagedFile>) -> Res<()> {
    let ss: Vec<Sale> = sales(d, seed, m);
    if ss.is_empty() {
        return Ok(());
    }
    let path: String = path_of(FILE);
    match files.iter().position(|x| x.path == path) {
        Some(i) => {
            let id: Option<u32> = dict_id(&files[i].z);
            let raw: Vec<u8> = rf.zs.decompress(&files[i].z)?;
            let out: Vec<u8> = edit_flow(&raw, &ss)?;
            files[i].new = out.len();
            files[i].z = zs::compress_raw(&out, id)?;
        }
        None => {
            let o: Out = rewrite(rf, &path, |raw: &[u8]| edit_flow(raw, &ss))?;
            files.push(StagedFile { path: o.path, z: o.z, old: o.old, new: o.new });
        }
    }
    Ok(())
}

fn lit(out: &mut Vec<Tok>, s: &str) -> Res<()> {
    out.extend(msbt::from_markup(s)?);
    Ok(())
}

fn compose(item: &[Tok], english: bool) -> Res<Vec<Tok>> {
    let mut out: Vec<Tok> = Vec::new();
    if english {
        lit(&mut out, "{3.0:0f0000}I've got a {0.3:0000}")?;
        out.extend(item.iter().cloned());
        lit(&mut out, &format!("{{0.3:ffff}} for you! It's\n{{0.3:0000}}{PRICE_TAG} rupees{{0.3:ffff}}.{{5.0}} Wanna buy it?"))?;
    } else {
        lit(&mut out, "{3.0:0f0000}{0.3:0000}")?;
        out.extend(item.iter().cloned());
        lit(&mut out, &format!("{{0.3:ffff}}\n{{0.3:0000}}{PRICE_TAG}{{0.3:ffff}}{{5.0}}"))?;
    }
    Ok(out)
}

fn text_file(raw: &[u8], sales: &[(String, Option<String>)], english: bool) -> Res<Vec<u8>> {
    let mut es: Vec<sarc::Entry> = sarc::parse(raw)?;
    let names: Option<Msbt> = sarc::get(&es, NAMES).map(|e| msbt::parse(&e.data)).transpose()?;
    let e: &sarc::Entry = sarc::get(&es, MSG_FILE).ok_or_else(|| E::Msg(format!("the language archive has no {MSG_FILE}")))?;
    let mut m: Msbt = msbt::parse(&e.data)?;
    let mystery: Vec<Tok> = msbt::from_markup(if english { MYSTERY } else { MYSTERY_OTHER })?;
    for (lab, actor) in sales {
        let named: Option<Vec<Tok>> = match (actor, &names) {
            (Some(a), Some(n)) => n.text(&format!("{a}_Name")).ok().filter(|t| !t.is_empty()),
            _ => None,
        };
        m.put_tokens(lab, &compose(named.as_deref().unwrap_or(&mystery), english)?)?;
    }
    sarc::set(&mut es, MSG_FILE, m.write()?);
    sarc::write(&es)
}

pub fn stage_text(rf: &mut Romfs, d: &Data, seed: &SeedFile, hint: bool, more: &mut Vec<Out>) -> Res<()> {
    let ss: Vec<(String, Option<String>)> = sales(d, seed, Mode::Vanilla).into_iter().map(|s| (s.label, if hint { s.actor } else { None })).collect();
    if ss.is_empty() {
        return Ok(());
    }
    for l in crate::hints::languages(rf)? {
        let english: bool = is_english(&l);
        crate::dungeon_grant::merge(rf, more, &l, |raw: &[u8]| text_file(raw, &ss, english))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cheap_prices_stay_in_range_and_repeat_for_the_same_seed() {
        for n in 0..200 {
            let s: String = format!("seed {n}");
            let p: i32 = price(Mode::Cheap, &s, "DMC Deku Scrub").unwrap();
            assert!((0..=95).contains(&p) && p % 5 == 0, "{p}");
            assert_eq!(Some(p), price(Mode::Cheap, &s, "DMC Deku Scrub"));
        }
        assert_eq!(price(Mode::Vanilla, "x", "DMC Deku Scrub"), None);
        assert_eq!(price(Mode::Fixed(25), "x", "DMC Deku Scrub"), Some(25));
    }

    #[test]
    fn a_price_and_line_for_the_crater_scrub_reach_the_rows_once_menu_scrubs_take_its_sale() {
        let mut b: Bf = crate::scrubs_b::flow::tests::sample();
        crate::scrubs_b::base(&mut b.flows[0]).unwrap();
        let sale: Sale = Sale { label: label("DMC Deku Scrub"), actor: None, price: Some(20), events: vec![(155, "Event293".to_string())] };
        let out: Bf = Bf::parse(&edit_flow(&b.write().unwrap(), &[sale]).unwrap()).unwrap();
        let f: &crate::bfevfl::Flow = &out.flows[0];
        let c: &Cont = f.events[155].kind.params().unwrap();
        assert_eq!(c.get(PRICE), Some(&Val::Arg("SalePrice".to_string())));
        assert_eq!(c.get(SAYS), Some(&Val::Arg("SaleMsg".to_string())));
        for i in [327, 172, 171] {
            let r: &Cont = f.events[i].kind.params().unwrap();
            assert_eq!(r.get("SalePrice"), Some(&Val::Int(20)), "{i}");
            assert_eq!(r.get("SaleMsg"), Some(&Val::Str(format!("{MSG_ID}:OOTRR_Scrub_DMCDekuScrub"))), "{i}");
        }
        // Without the menu scrubs the sale event itself takes the values.
        let plain: Bf = crate::scrubs_b::flow::tests::sample();
        let sale: Sale = Sale { label: label("DMC Deku Scrub"), actor: None, price: None, events: vec![(155, "Event293".to_string())] };
        let out: Bf = Bf::parse(&edit_flow(&plain.write().unwrap(), &[sale]).unwrap()).unwrap();
        let c: &Cont = out.flows[0].events[155].kind.params().unwrap();
        assert_eq!(c.get(PRICE), Some(&Val::Int(100)));
        assert_eq!(c.get(SAYS), Some(&Val::Str(format!("{MSG_ID}:OOTRR_Scrub_DMCDekuScrub"))));
    }

    #[test]
    fn each_scrub_gets_its_own_line_with_the_live_price() {
        assert_eq!(label("DMC Deku Scrub"), "OOTRR_Scrub_DMCDekuScrub");
        assert_ne!(label("DMC Deku Scrub"), label("HF Deku Scrub Grotto"));
        let t: Vec<Tok> = compose(&msbt::from_markup("Longshot").unwrap(), true).unwrap();
        let s: String = msbt::to_markup(&t);
        assert!(s.contains("Longshot") && s.contains(PRICE_TAG), "{s}");
    }
}
