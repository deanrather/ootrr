use std::fmt::Write;

use super::tab::{NAMES, OUTSZ, PSZ};
use super::{Ainb, Input};

fn val(a: &Ainb, k: usize, v: &[u8]) -> String {
    match k {
        0 => format!("{}", i32::from_le_bytes([v[0], v[1], v[2], v[3]])),
        1 => format!("{}", u32::from_le_bytes([v[0], v[1], v[2], v[3]]) != 0),
        2 => format!("{}", f32::from_le_bytes([v[0], v[1], v[2], v[3]])),
        3 => format!("{:?}", a.s(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))),
        4 => {
            let f = |i: usize| f32::from_le_bytes([v[i], v[i + 1], v[i + 2], v[i + 3]]);
            format!("({}, {}, {})", f(0), f(4), f(8))
        }
        _ => String::new(),
    }
}

fn props(a: &Ainb, out: &mut String, pe: &[[u32; 2]; 6]) {
    for k in 0..6 {
        let (b, n): (usize, usize) = (pe[k][0] as usize, pe[k][1] as usize);
        for g in b..b + n {
            if let Some(e) = a.props[k].get(g * PSZ[k]..(g + 1) * PSZ[k]) {
                let _ = writeln!(out, "  prop {} {} = {}", NAMES[k], a.prop_name(k, g).unwrap_or("?"), val(a, k, &e[8..]));
            }
        }
    }
}

pub fn show(a: &Ainb) -> String {
    let mut out: String = String::new();
    let _ = writeln!(out, "ainb elements {} commands {} attachments {} pool {} bytes", a.els.len(), a.cmds.len() / 0x18, a.atts.len(), a.pool.b.len());
    for i in 0..a.els.len() {
        let _ = writeln!(out, "element {} type {} {}", i, a.el_type(i), a.el_name(i));
        let p = &a.params[i];
        props(a, &mut out, &p.pe);
        for j in a.el_atts(i) {
            let _ = writeln!(out, "  attachment {} {}", j, a.att_name(j));
        }
        for k in 0..6 {
            for g in a.input_ids(i, k) {
                if let Some(x) = a.input(k, g) {
                    let from: String = if x.src == -1 { String::new() } else { format!(" from {}.{}", x.src, x.plug) };
                    let Input { name, flags, val: v, .. } = x;
                    let _ = writeln!(out, "  in {} {} = {}{} flags {:#x}", NAMES[k], a.s(name), val(a, k, &v), from, flags);
                }
            }
        }
        for k in 0..6 {
            let (b, n): (usize, usize) = (p.ioe[k][2] as usize, p.ioe[k][3] as usize);
            for g in b..b + n {
                if let Some(e) = a.outs[k].get(g * OUTSZ[k]..g * OUTSZ[k] + 4) {
                    let w: u32 = u32::from_le_bytes([e[0], e[1], e[2], e[3]]);
                    let _ = writeln!(out, "  out {} {} flags {:#x}", NAMES[k], a.s(w & 0x3fff_ffff), w >> 30);
                }
            }
        }
        for (j, g) in p.plugs.iter().enumerate() {
            let hex: String = g.extra.iter().map(|b| format!("{b:02x}")).collect();
            let _ = writeln!(out, "  plug {} -> element {} {:?} extra {} {}", j, g.a, a.s(g.b), g.extra.len(), hex);
        }
        let gs: Vec<String> = p.pl.iter().map(|g| format!("{}@{}", g[0], g[1])).collect();
        let _ = writeln!(out, "  plug groups {}", gs.join(" "));
    }
    for (j, t) in a.atts.iter().enumerate() {
        let _ = writeln!(out, "attachment {} {} flags {:#x} expressions {} {}", j, a.att_name(j), t.flags, t.exb[0], t.exb[1]);
        props(a, &mut out, &t.pe);
    }
    out
}
