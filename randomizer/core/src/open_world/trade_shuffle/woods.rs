//! Grog and Fado follow the trade item that is out, live (adult trade shuffle).
//!
//! In the game the adult Lost Woods stump holds Grog at quest steps 03-04, Fado at 05-06 and a
//! Skull Kid otherwise: the banc gates Grog and Fado on `IsAfter_Step04` (Presence, read when the
//! map loads) and the prefab graph [`MODULE`] creates all three from step checks. With the
//! shuffle the step is only settled at hand ins and gets, so switching trade items in the menu
//! changed nothing until a later settle and a reload.
//!
//! Here the graph reads the pouch counts instead. Only one adult trade item is out (count 1, the
//! others are parked, see super), so: Grog stands while Cojiro or the Odd Mushroom is out, Fado
//! while the Odd Potion or the Poacher's Saw is, the Skull Kid when neither does. Each one's
//! Logic_Delete reads the false side of its own gate (the game's own pattern for an area actor in
//! LostWood_Adult_Prefab_09a5), so a switch in the menu swaps them as soon as the game unpauses.
//! Their banc Presence reads an always-true expression, so both are placed and the graph alone
//! decides.

use crate::ainb::build::{Spec, QUERY};
use crate::ainb::{Ainb, Plug};
use crate::byml::{Byml, Node};
use crate::error::{Error, Res};
use crate::gerudo_card::{give_queries, multi_node, qpos, queries, rewire, AND, BOOL, FALSE_OUT, OR, TRUE_OUT};

use super::{count_name, ORDER};

pub const MODULE: &str = "Logic/LostWood_Adult_Prefab_b5e7.logic.module.ainb";
pub const MAP: &str = "Banc/LostWood/LostWood_Adult_Static.bcett.byml.zs";
/// The banc Presence of Grog (negated) and Fado.
pub const PRESENCE: &str = "SubEvent_GetSwordBigGoron_IsAfter_Step04_Exp";
pub const ALWAYS: &str = crate::always::NAME;

pub const NPC: &str = "ActorLogicNpc";
pub const SIMPLE: &str = "ActorLogicSimpleActor";
pub const GROG: &str = "Npc_Hylia_017_Adult_7d7e";
pub const FADO: &str = "Npc_Kokiri_Fado_Adult_7409";
pub const SKULL_KID: &str = "Enemy_SkullKid_5a60";
pub const GROG_GYAML: &str = "Npc_Hylia_017_Adult";
pub const FADO_GYAML: &str = "Npc_Kokiri_Fado_Adult";

/// The trade items each one stands for, as places in [`ORDER`].
pub const GROG_ITEMS: [usize; 2] = [2, 3];
pub const FADO_ITEMS: [usize; 2] = [4, 5];

pub const CREATE: &str = "Logic_Create";
pub const DELETE: &str = "Logic_Delete";
const GET_INT: &str = "HoldGameDataGetIntSimple";
const COMPARE: &str = "OperateCompareInt";
const S32: usize = 0;
const STR: usize = 3;
const BOOL_IN: usize = 16;
/// OperateCompareInt outputs, in order: A_Bigger, A_Smaller, Equal.
pub const EQUAL: u16 = 2;
const TAG: &str = "ootrr trade woods";

fn named(a: &Ainb, kind: &str, instance: &str) -> Res<usize> {
    let hits: Vec<usize> = a.find_els(kind).into_iter().filter(|&i| a.prop_str(i, "InstanceName") == Some(instance)).collect();
    match hits.as_slice() {
        [i] => Ok(*i),
        _ => Err(Error::Msg(format!("trade woods: expected one {kind} {instance} in {MODULE}, found {}", hits.len()))),
    }
}

/// "Is this trade item out?": its pouch count equals 1.
fn out_node(a: &mut Ainb, actor: &str) -> Res<u16> {
    let seed: String = format!("{TAG} {actor}");
    let name: Vec<u8> = a.pool.add(&count_name(actor)).to_le_bytes().to_vec();
    let mut g: Spec = Spec::node(GET_INT, &seed).idle(S32, "GameDataArrayIndex", (-1i32).to_le_bytes().to_vec()).constant(STR, "GameDataNameInt", name).out(S32, "Int");
    g.role = QUERY;
    let get: usize = a.add_node(&g)?;
    let seed2: String = format!("{seed} out");
    let mut c: Spec = Spec::node(COMPARE, &seed2)
        .linked(S32, "Int_A", get, 0)
        .constant(S32, "Int_B", 1i32.to_le_bytes().to_vec())
        .spare(BOOL, "A_Bigger")
        .spare(BOOL, "A_Smaller")
        .out(BOOL, "Equal");
    c.role = QUERY;
    let id: usize = a.add_node(&c)?;
    u16::try_from(id).map_err(|_| Error::Format("trade woods: element index too large"))
}

/// The single source of a linked bool input.
fn source(a: &Ainb, i: usize, input: &str) -> Res<(usize, i16, i16)> {
    let g: usize = a.find_in(i, BOOL, input).ok_or_else(|| Error::Msg(format!("trade woods: element {i} has no {input}")))?;
    let x = a.input(BOOL, g).ok_or(Error::Format("trade woods: input out of range"))?;
    Ok((g, x.src, x.plug))
}

/// Points a constant bool input of element `i` at output `out` of `gate`.
pub fn link_const(a: &mut Ainb, i: usize, input: &str, gate: usize, out: i16) -> Res<()> {
    let (g, src, _) = source(a, i, input)?;
    if src != -1 {
        return Err(Error::Msg(format!("trade woods: {input} of element {i} is already linked")));
    }
    let to: i16 = i16::try_from(gate).map_err(|_| Error::Format("trade woods: element index too large"))?;
    let o: usize = g * BOOL_IN;
    let rec: &mut [u8] = a.ins[BOOL].get_mut(o..o + BOOL_IN).ok_or(Error::Format("trade woods: input out of range"))?;
    rec[4..6].copy_from_slice(&to.to_le_bytes());
    rec[6..8].copy_from_slice(&out.to_le_bytes());
    rec[8..12].copy_from_slice(&0u32.to_le_bytes());
    let name: u32 = a.pool.add(input);
    let extra: Vec<u8> = a.params[i].group(0).first().map(|p| p.extra.clone()).unwrap_or_default();
    let at: usize = a.params[i].group(0).len();
    a.params[i].add_plug(0, at, Plug { a: gate as u32, b: name, extra })?;
    let mut want: Vec<u16> = queries(a, i);
    let q: u16 = qpos(a, gate)?;
    if !want.contains(&q) {
        want.push(q);
        give_queries(a, i, &want)?;
    }
    Ok(())
}

/// Element `i` is created while `gate` is true and deleted while it is false.
fn follow(a: &mut Ainb, i: usize, gate: usize, delete_out: Option<i16>) -> Res<()> {
    let (g, src, _) = source(a, i, CREATE)?;
    if src < 0 {
        return Err(Error::Msg(format!("trade woods: {CREATE} of element {i} is not linked, the game files differ")));
    }
    let create: u32 = a.pool.add(CREATE);
    rewire(a, i, g, src, create, gate, TRUE_OUT)?;
    if let Some(d) = delete_out {
        link_const(a, i, DELETE, gate, d)?;
    }
    Ok(())
}

pub fn patch(a: &mut Ainb) -> Res<()> {
    let grog: usize = named(a, NPC, GROG)?;
    let fado: usize = named(a, NPC, FADO)?;
    let kid: usize = named(a, SIMPLE, SKULL_KID)?;
    // The Skull Kid's gate is AND(not at the trade steps, the supply reset actor alive): keep
    // the second source.
    let (_, ksrc, _) = source(a, kid, CREATE)?;
    if ksrc < 0 || a.el_name(ksrc as usize) != AND {
        return Err(Error::Msg(format!("trade woods: the Skull Kid is not created by a gate in {MODULE}, the game files differ")));
    }
    let old: Vec<(u16, u16)> = crate::pot_slice::kf_ring::sources(a, ksrc as usize).ok_or(Error::Format("trade woods: the Skull Kid gate has no multi input"))?;
    let keep: Vec<(u16, u16)> = old.into_iter().filter(|(e, _)| a.el_name(*e as usize) == SIMPLE).collect();
    if keep.len() != 1 {
        return Err(Error::Msg(format!("trade woods: the Skull Kid gate in {MODULE} does not read the supply actor, the game files differ")));
    }
    let mut grog_srcs: Vec<(usize, u16)> = Vec::new();
    for k in GROG_ITEMS {
        grog_srcs.push((out_node(a, ORDER[k].actor)? as usize, EQUAL));
    }
    let mut fado_srcs: Vec<(usize, u16)> = Vec::new();
    for k in FADO_ITEMS {
        fado_srcs.push((out_node(a, ORDER[k].actor)? as usize, EQUAL));
    }
    let g_gate: usize = multi_node(a, OR, &format!("{TAG} grog"), &grog_srcs)?;
    let f_gate: usize = multi_node(a, OR, &format!("{TAG} fado"), &fado_srcs)?;
    let any: usize = multi_node(a, OR, &format!("{TAG} trader"), &[(g_gate, TRUE_OUT as u16), (f_gate, TRUE_OUT as u16)])?;
    let k_gate: usize = multi_node(a, AND, &format!("{TAG} skull kid"), &[(any, FALSE_OUT), (keep[0].0 as usize, keep[0].1)])?;
    follow(a, grog, g_gate, Some(FALSE_OUT as i16))?;
    follow(a, fado, f_gate, Some(FALSE_OUT as i16))?;
    // The Skull Kid leaves only when a trader takes the stump (not while the supply actor is
    // still loading, which the game never deleted him for).
    follow(a, kid, k_gate, None)?;
    link_const(a, kid, DELETE, any, TRUE_OUT)?;
    Ok(())
}

pub fn module(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    patch(&mut a)?;
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if back.write()? != out || !crate::ainb::audit(&back).is_empty() {
        return Err(Error::Format("trade woods: the patched module does not read back"));
    }
    Ok(out)
}

/// Grog's and Fado's banc Presence read [`ALWAYS`] (both placed, the graph decides).
pub fn map(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    let actors: &mut Vec<Node> = y.root.get_mut("Actors").and_then(|n| n.as_arr_mut()).ok_or(Error::Format("trade woods: the map has no actors"))?;
    let mut hit: Vec<String> = Vec::new();
    for a in actors.iter_mut() {
        let Some(g) = a.get("Gyaml").and_then(|g| g.as_str()).map(str::to_string) else { continue };
        if g != GROG_GYAML && g != FADO_GYAML {
            continue;
        }
        let Some(p) = a.get_mut("Presence") else { continue };
        if p.get("FlagName").and_then(|n| n.as_str()) != Some(PRESENCE) {
            continue;
        }
        p.set("FlagName", Node::Str(ALWAYS.to_string()));
        p.set("IsNegation", Node::Bool(false));
        hit.push(g);
    }
    hit.sort();
    if hit != [GROG_GYAML, FADO_GYAML] {
        return Err(Error::Msg(format!("trade woods: expected Grog and Fado gated on {PRESENCE} in {MAP}, found {hit:?}")));
    }
    let out: Vec<u8> = y.write()?;
    if Byml::parse(&out)?.root != y.root {
        return Err(Error::Format("trade woods: the map does not read back"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::romfs::Romfs;

    fn real() -> Option<Romfs> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Romfs::open(&p).ok()
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn read_by(a: &Ainb, i: usize, input: &str) -> (i16, i16) {
        let x = a.input(BOOL, a.find_in(i, BOOL, input).unwrap()).unwrap();
        (x.src, x.plug)
    }

    /// The count each "is out" compare reads, by gate output.
    fn counts(a: &Ainb, gate: usize) -> Vec<String> {
        let mut v: Vec<String> = crate::pot_slice::kf_ring::sources(a, gate)
            .unwrap()
            .into_iter()
            .map(|(c, o)| {
                assert_eq!((a.el_name(c as usize), o), (COMPARE, EQUAL));
                let (s, p) = {
                    let g = a.find_in(c as usize, S32, "Int_A").unwrap();
                    let x = a.input(S32, g).unwrap();
                    (x.src, x.plug)
                };
                assert_eq!(p, 0);
                assert_eq!(a.input(S32, a.find_in(c as usize, S32, "Int_B").unwrap()).unwrap().val[..4], 1i32.to_le_bytes());
                a.in_str(s as usize, "GameDataNameInt").unwrap().to_string()
            })
            .collect();
        v.sort();
        v
    }

    #[test]
    fn grog_fado_and_the_skull_kid_follow_the_item_out() {
        let Some(rf) = real() else { return };
        let raw: Vec<u8> = rf.read(MODULE).unwrap();
        let a: Ainb = Ainb::parse(&module(&raw).unwrap()).unwrap();
        let grog: usize = named(&a, NPC, GROG).unwrap();
        let fado: usize = named(&a, NPC, FADO).unwrap();
        let kid: usize = named(&a, SIMPLE, SKULL_KID).unwrap();
        let (g, gp) = read_by(&a, grog, CREATE);
        assert_eq!((a.el_name(g as usize), gp), (OR, TRUE_OUT));
        assert_eq!(read_by(&a, grog, DELETE), (g, FALSE_OUT as i16));
        assert_eq!(counts(&a, g as usize), vec![count_name("Item_CojiroCucco"), count_name("Item_OddMushroom")]);
        let (f, fp) = read_by(&a, fado, CREATE);
        assert_eq!((a.el_name(f as usize), fp), (OR, TRUE_OUT));
        assert_eq!(read_by(&a, fado, DELETE), (f, FALSE_OUT as i16));
        assert_eq!(counts(&a, f as usize), vec![count_name("Item_OddPowder"), count_name("Item_PoacherSaw")]);
        let (k, kp) = read_by(&a, kid, CREATE);
        assert_eq!((a.el_name(k as usize), kp), (AND, TRUE_OUT));
        let ks = crate::pot_slice::kf_ring::sources(&a, k as usize).unwrap();
        let (any, ap) = read_by(&a, kid, DELETE);
        assert_eq!(ap, TRUE_OUT);
        assert_eq!(ks[0], (any as u16, FALSE_OUT));
        assert_eq!(a.prop_str(ks[1].0 as usize, "InstanceName"), Some("SupplyItemResetSystemActor_56cd"));
        let mut traders = crate::pot_slice::kf_ring::sources(&a, any as usize).unwrap();
        traders.sort();
        let mut want = vec![(g as u16, TRUE_OUT as u16), (f as u16, TRUE_OUT as u16)];
        want.sort();
        assert_eq!(traders, want);
        // Nothing reads the quest step any more for the three of them.
        for i in [grog, fado, kid] {
            for name in [CREATE, DELETE] {
                let (s, _) = read_by(&a, i, name);
                assert!(s >= 0, "{i} {name}");
                assert_ne!(a.el_name(s as usize), "HoldChallengeCurrentStepCheckOrder");
            }
        }
        // A second patch fails instead of stacking.
        let mut b: Ainb = a.clone();
        assert!(patch(&mut b).is_err());
    }

    #[test]
    fn grog_and_fado_are_always_placed() {
        let Some(mut rf) = real() else { return };
        let raw: Vec<u8> = rf.read_zs(MAP).unwrap();
        let y: Byml = Byml::parse(&map(&raw).unwrap()).unwrap();
        let actors = y.root.get("Actors").and_then(|n| n.as_arr()).unwrap();
        for g in [GROG_GYAML, FADO_GYAML] {
            let a: Vec<&Node> = actors.iter().filter(|a| a.get("Gyaml").and_then(|x| x.as_str()) == Some(g)).collect();
            assert_eq!(a.len(), 1, "{g}");
            let p = a[0].get("Presence").unwrap();
            assert_eq!(p.get("FlagName").and_then(|x| x.as_str()), Some(ALWAYS), "{g}");
            assert_eq!(p.get("IsNegation"), Some(&Node::Bool(false)), "{g}");
        }
        assert!(map(&y.write().unwrap()).is_err());
    }
}
