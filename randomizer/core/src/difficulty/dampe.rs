use crate::ainb::build::Spec;
use crate::ainb::{audit, Ainb, Param, Plug};
use crate::byml::{Byml, Node};
use crate::difficulty::backup::{GETB, IDLE, LIFT, MAT33, SETB};
use crate::difficulty::hook::{flag, switch, PARALLEL};
use crate::difficulty::spawn::{route, CONJUNCTION};
use crate::error::{Error, Res};
use crate::qol::{edit_zs, Have};
use crate::romfs::Romfs;
use crate::sarc;
use crate::staging::Out;

pub const PACK: &str = "Pack/Actor/Npc_Hylia_Dampe_001.pack.zs";
pub const HOLDER: &str = "Component/ActorHolderParam/Npc_Hylia_Dampe_001.game__component__ActorHolderParam.bgyml";
pub const UTILITY: &str = "AI/Npc_Hylia_Dampe_001.utility.root.ainb";
pub const TABLE: &str = "Component/Blackboard/BlackboardParamTable/Npc_Hylia_Dampe_001.engine__component__BlackboardParamTable.bgyml";
pub const BOOLS: &str = "BlackboardParamBoolArray";
pub const BOOL_MODEL: &str = "IsGoPointPlayerSelected";
pub const BY_A_HOLE: &str = "OotrrDampeByDugHole";
pub const ACTOR_BB: i32 = 1;
pub const HELD: &str = "ActorHoldSettings";
pub const HOLD_KEY: &str = "Ootrr";
pub const MODEL: &str = "RupeeRed";
pub const WAKE: &str = "OneShotActorHolderTryCancelSleepActor";
pub const CUT: &str = "OneShotActorHolderDisconnectActorLink";
pub const SOIL: &str = "QueryMiniGameDiggingHoleDistanceToNearPoint";
pub const LESS: &str = "QueryMathLessF32";
pub const U32: &str = "const u32";
pub const S32_SWITCH: u16 = 1;
pub const S32_LINK: usize = 5;
pub const CHOICE: &str = "QueryAvoidDuplicationRandomChoice";
pub const OVER: &str = "bb::OverwriteParam";
pub const ALWAYS: &str = "NPCCommon.utility.module";
pub const MARK: &str = "OOTRRDampe";
pub const NEAR: f32 = 2.0;
pub const BESIDE: [f32; 3] = [1.5, 0.5, 0.0];
pub const MONSTERS: [&str; 4] = ["Enemy_Poe", "Enemy_Floormaster", "Enemy_ReDead", "Enemy_DeadHand"];
pub const NOTE: &str = "Each time Dampe digs up a rupee on his grave digging tour, a monster comes out of the hole with it: a Poe, a Floormaster, a ReDead or a Dead Hand, one in four each. The Piece of Heart dig at the end is left alone.";

pub fn hold_key(monster: &str) -> String {
    format!("{HOLD_KEY}{}", monster.trim_start_matches("Enemy_"))
}

fn vec3(v: [f32; 3]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn patch_holder(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    let held: &mut Node = y.root.get_mut(HELD).ok_or(Error::Format("dampe: his holder has no entries"))?;
    let model: Node = held.get(MODEL).cloned().ok_or(Error::Format("dampe: his holder has no red rupee"))?;
    for m in MONSTERS {
        if held.get(&hold_key(m)).is_some() {
            return Err(Error::Format("dampe: his holder already lists the monsters"));
        }
        let mut e: Node = model.clone();
        e.set("Actor", Node::Str(format!("Work/Actor/{m}.engine__actor__ActorParam.gyml")));
        held.set(&hold_key(m), e);
    }
    y.write()
}

pub fn patch_table(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    let rows: &mut Vec<Node> = y.root.get_mut(BOOLS).and_then(Node::as_arr_mut).ok_or(Error::Format("dampe: his blackboard table has no bool list"))?;
    if rows.iter().any(|r| r.get("BBKey").and_then(Node::as_str) == Some(BY_A_HOLE)) {
        return Err(Error::Format("dampe: his blackboard table already has the dig key"));
    }
    let mut row: Node = rows.iter().find(|r| r.get("BBKey").and_then(Node::as_str) == Some(BOOL_MODEL)).cloned().ok_or(Error::Format("dampe: his blackboard table lacks its model key"))?;
    row.set("BBKey", Node::Str(BY_A_HOLE.to_string()));
    rows.push(row);
    y.write()
}

fn key_of(a: &mut Ainb, k: &str) -> Vec<u8> {
    a.pool.add(k).to_le_bytes().to_vec()
}

fn par(a: &mut Ainb, seed: &str, kids: &[usize]) -> Res<usize> {
    let mut s: Spec = Spec::composite(PARALLEL, seed).prop(0, "EndPolicy", vec![0; 4]).prop(0, "ResultPolicy", vec![0; 4]);
    for k in kids {
        s = s.kid(*k, "");
    }
    a.add_node(&s)
}

fn set_by_a_hole(a: &mut Ainb, seed: &str, on: bool) -> Res<usize> {
    let k: Vec<u8> = key_of(a, BY_A_HOLE);
    a.add_node(
        &Spec::node(SETB, seed)
            .prop(0, "BlackboardType", ACTOR_BB.to_le_bytes().to_vec())
            .prop(1, "ExecuteOnce", flag(false))
            .prop(1, "IsIgnoreNotExistsError", flag(false))
            .prop(3, "BlackboardKey", k)
            .constant(1, "Bool", flag(on)),
    )
}

fn unlink_reader(a: &mut Ainb, reader: usize) -> Res<()> {
    let r: &mut Param = &mut a.params[reader];
    if r.pl[0][0] == 1 {
        r.remove_plug(0, 0)?;
    }
    Ok(())
}

fn wake_spec<'a>(seed: &'a str, key: Vec<u8>, pos: usize) -> Spec<'a> {
    Spec::node(WAKE, seed)
        .prop(1, "ExecuteEveryFrame", flag(false))
        .prop(1, "IsForeachActor", flag(false))
        .idle(0, "Index", vec![0; 4])
        .constant(3, "Key", key)
        .linked(4, "Pos", pos, 0)
        .idle(4, "Scale", vec3([1.0, 1.0, 1.0]))
        .idle_ptr("OverwriteParam", OVER)
        .idle_ptr("Rot", MAT33)
}

fn release(a: &mut Ainb, m: &str, pos: usize, first: bool) -> Res<usize> {
    let key: Vec<u8> = key_of(a, &hold_key(m));
    let wake_seed: String = format!("{MARK} wake {m}");
    let wake: usize = a.add_node(&wake_spec(&wake_seed, key.clone(), pos))?;
    if first {
        route(a, pos, wake, "Result", "Pos")?;
    } else {
        unlink_reader(a, wake)?;
    }
    let cut_seed: String = format!("{MARK} cut {m}");
    let cut: usize = a.add_node(&Spec::node(CUT, &cut_seed).prop(1, "ExecuteEveryFrame", flag(false)).prop(1, "IsForeachActor", flag(false)).idle(0, "Index", vec![0; 4]).constant(3, "Key", key))?;
    par(a, &format!("{MARK} release {m}"), &[wake, cut])
}

fn case_extra(v: i32) -> Vec<u8> {
    let mut e: Vec<u8> = vec![0; 4];
    e.extend_from_slice(&v.to_le_bytes());
    e
}

fn pick(a: &mut Ainb, outs: &[usize]) -> Res<usize> {
    let roll_seed: String = format!("{MARK} roll");
    let roll: usize = a.add_node(&Spec::query(CHOICE, &roll_seed).prop(0, "ChoiceNum", (outs.len() as i32).to_le_bytes().to_vec()).out(0, "ChoiceNo"))?;
    let pick_seed: String = format!("{MARK} pick");
    let mut s: Spec = Spec::composite(S32_SWITCH, &pick_seed)
        .prop(0, "CalculateTiming", vec![0; 4])
        .prop(0, "InputValue", vec![0; 4])
        .prop(1, "ChildFrameSync", flag(false))
        .prop(1, "IsNoSelectIfSameInstance", flag(false))
        .prop(1, "IsNoSelectWhenChildBusy", flag(false))
        .linked(0, "Input", roll, 0);
    s.link_extra = 8;
    let sel: usize = a.add_node(&s)?;
    let link: Plug = a.params[sel].remove_plug(0, 0)?;
    let empty: u32 = a.pool.add("");
    let last: usize = outs.len() - 1;
    for (i, o) in outs.iter().enumerate() {
        let v: i32 = if i == last { 0 } else { i as i32 };
        a.params[sel].add_plug(2, i, Plug { a: *o as u32, b: empty, extra: case_extra(v) })?;
    }
    a.params[sel].add_plug(S32_LINK, 0, link)?;
    Ok(sel)
}

fn idle(a: &mut Ainb, what: &str) -> Res<usize> {
    a.add_node(&Spec::node(IDLE, &format!("{MARK} idle {what}")))
}

pub fn patch_utility(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    if a.pool.find(BY_A_HOLE).is_some() {
        return Err(Error::Format("dampe: his utility graph is already edited"));
    }
    let always: Vec<usize> = a.find_els(ALWAYS);
    let [module] = always.as_slice() else {
        return Err(Error::Format("dampe: expected one common utility module"));
    };
    let (top, _): (usize, usize) = a.parent_of(*module)?;
    if a.el_type(top) != PARALLEL {
        return Err(Error::Format("dampe: his utility modules are not in a parallel node"));
    }
    let soil_seed: String = format!("{MARK} nearest soil");
    let soil: usize = a.add_node(&Spec::query(SOIL, &soil_seed).spare(0, "SoilType").out(1, "IsDug").out(2, "DistanceFromActor").spare(3, "InstanceName").out(4, "Postion").spare_ptr("BaseProcId", U32).spare_ptr("Rotation", MAT33))?;
    let close_seed: String = format!("{MARK} close to it");
    let close: usize = a.add_node(&Spec::query(LESS, &close_seed).linked(2, "F32", soil, 0).constant(2, "F32Base", NEAR.to_le_bytes().to_vec()).out(1, "Bool"))?;
    route(&mut a, soil, close, "DistanceFromActor", "F32")?;
    let by_seed: String = format!("{MARK} by a dug hole");
    let by: usize = a.add_node(&Spec::query(CONJUNCTION, &by_seed).linked(1, "ValueA", soil, 0).linked(1, "ValueB", close, 0).out(1, "Result"))?;
    route(&mut a, soil, by, "IsDug", "ValueA")?;
    route(&mut a, close, by, "Bool", "ValueB")?;
    let beside_seed: String = format!("{MARK} beside the hole");
    let beside: usize = a.add_node(&Spec::query(LIFT, &beside_seed).linked(4, "InputA", soil, 0).constant(4, "InputB", vec3(BESIDE)).out(4, "Result"))?;
    route(&mut a, soil, beside, "Postion", "InputA")?;
    let mut outs: Vec<usize> = Vec::new();
    for (i, m) in MONSTERS.iter().enumerate() {
        outs.push(release(&mut a, m, beside, i == 0)?);
    }
    let picked: usize = pick(&mut a, &outs)?;
    let mark: usize = set_by_a_hole(&mut a, &format!("{MARK} remember"), true)?;
    let fire: usize = par(&mut a, &format!("{MARK} fire"), &[mark, picked])?;
    let wait_dig: usize = idle(&mut a, "no dug hole")?;
    let arrive: usize = a.add_node(&switch(&format!("{MARK} arrive"), by, fire, wait_dig))?;
    let forget: usize = set_by_a_hole(&mut a, &format!("{MARK} forget"), false)?;
    let wait_leave: usize = idle(&mut a, "still by the hole")?;
    let leave: usize = a.add_node(&switch(&format!("{MARK} leave"), by, wait_leave, forget))?;
    let was_k: Vec<u8> = key_of(&mut a, BY_A_HOLE);
    let was_seed: String = format!("{MARK} was by a dug hole");
    let was: usize = a.add_node(&Spec::query(GETB, &was_seed).prop(0, "BlackboardType", ACTOR_BB.to_le_bytes().to_vec()).prop(3, "BlackboardKey", was_k).out(1, "Bool"))?;
    let gate: usize = a.add_node(&switch(&format!("{MARK} gate"), was, leave, arrive))?;
    let kids: usize = a.params[top].group(2).len();
    let empty: u32 = a.pool.add("");
    a.params[top].add_plug(2, kids, Plug { a: gate as u32, b: empty, extra: Vec::new() })?;
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(Error::Msg(format!("dampe: his utility graph: {}", bad.join("; "))));
    }
    a.write()
}

pub fn patch_pack(raw: &[u8]) -> Res<Vec<u8>> {
    let mut es: Vec<sarc::Entry> = sarc::parse(raw)?;
    for (name, f) in [(HOLDER, patch_holder as fn(&[u8]) -> Res<Vec<u8>>), (TABLE, patch_table), (UTILITY, patch_utility)] {
        let data: Vec<u8> = sarc::get(&es, name).ok_or(Error::Msg(format!("dampe: his pack has no {name}")))?.data.clone();
        sarc::set(&mut es, name, f(&data)?);
    }
    sarc::write(&es)
}

pub fn stage(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    Ok(vec![edit_zs(rf, have, PACK, patch_pack)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Option<Romfs> {
        let Some(p) = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from) else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return None;
        };
        Romfs::open(&p).ok()
    }

    fn packs(rf: &mut Romfs) -> (Vec<sarc::Entry>, Vec<sarc::Entry>) {
        let raw: Vec<u8> = rf.read_zs(PACK).unwrap();
        let out: Vec<u8> = patch_pack(&raw).unwrap();
        assert!(patch_pack(&out).is_err());
        (sarc::parse(&raw).unwrap(), sarc::parse(&out).unwrap())
    }

    fn graph(es: &[sarc::Entry], name: &str) -> Ainb {
        Ainb::parse(&sarc::get(es, name).unwrap().data).unwrap()
    }

    fn kids(a: &Ainb, i: usize) -> Vec<usize> {
        a.params[i].group(2).iter().map(|p| p.a as usize).collect()
    }

    fn shape(a: &Ainb, i: usize) -> Vec<String> {
        let mut out: Vec<String> = vec![format!("{} type {}", a.el_name(i), a.el_type(i))];
        let p: &Param = &a.params[i];
        for k in 0..5 {
            for g in p.pe[k][0] as usize..(p.pe[k][0] + p.pe[k][1]) as usize {
                out.push(format!("prop {k} {}", a.prop_name(k, g).unwrap()));
            }
            for g in a.input_ids(i, k) {
                out.push(format!("in {k} {}", a.s(a.input(k, g).unwrap().name)));
            }
            for g in p.ioe[k][2] as usize..(p.ioe[k][2] + p.ioe[k][3]) as usize {
                let w: u32 = u32::from_le_bytes(a.outs[k][g * 4..g * 4 + 4].try_into().unwrap());
                out.push(format!("out {k} {}", a.s(w & 0x3fff_ffff)));
            }
        }
        out
    }

    fn source(a: &Ainb, reader: usize, k: usize, name: &str) -> usize {
        a.input(k, a.find_in(reader, k, name).unwrap()).unwrap().src as usize
    }

    fn prop_word(a: &Ainb, i: usize, k: usize, name: &str) -> Option<u32> {
        let p: &Param = &a.params[i];
        (p.pe[k][0] as usize..(p.pe[k][0] + p.pe[k][1]) as usize).find(|g| a.prop_name(k, *g) == Some(name)).map(|g| u32::from_le_bytes(a.props[k][g * 12 + 8..g * 12 + 12].try_into().unwrap()))
    }

    fn in_val(a: &Ainb, i: usize, k: usize, name: &str) -> Vec<u8> {
        a.input(k, a.find_in(i, k, name).unwrap()).unwrap().val
    }

    fn on_his_key(a: &Ainb, i: usize) -> bool {
        a.prop_str(i, "BlackboardKey") == Some(BY_A_HOLE) && prop_word(a, i, 0, "BlackboardType") == Some(ACTOR_BB as u32)
    }

    #[test]
    fn something_that_is_not_his_pack_is_refused() {
        assert!(patch_pack(b"not a pack").is_err());
        assert!(patch_utility(b"not a graph").is_err());
        assert!(patch_holder(b"not a holder").is_err());
        assert!(patch_table(b"not a table").is_err());
    }

    #[test]
    fn only_his_holder_his_table_and_his_utility_graph_change_and_his_event_graph_is_left_alone() {
        let Some(mut rf) = game() else {
            return;
        };
        let (was, now): (Vec<sarc::Entry>, Vec<sarc::Entry>) = packs(&mut rf);
        assert_eq!(was.len(), now.len());
        let mut changed: Vec<&str> = now.iter().filter(|e| sarc::get(&was, &e.name).is_none_or(|w| w.data != e.data)).map(|e| e.name.as_str()).collect();
        changed.sort();
        let mut want: Vec<&str> = vec![HOLDER, TABLE, UTILITY];
        want.sort();
        assert_eq!(changed, want);
        let outs: Vec<Out> = stage(&mut rf, &|_: &str| -> Option<Vec<u8>> { None }).unwrap();
        assert_eq!(outs.iter().map(|o| o.path.as_str()).collect::<Vec<&str>>(), vec![PACK]);
    }

    #[test]
    fn his_holder_keeps_a_sleeping_copy_of_each_monster_like_his_rupees() {
        let Some(mut rf) = game() else {
            return;
        };
        let (was, now): (Vec<sarc::Entry>, Vec<sarc::Entry>) = packs(&mut rf);
        let old: Byml = Byml::parse(&sarc::get(&was, HOLDER).unwrap().data).unwrap();
        let new: Byml = Byml::parse(&sarc::get(&now, HOLDER).unwrap().data).unwrap();
        let red: &Node = old.root.get(HELD).unwrap().get(MODEL).unwrap();
        for m in MONSTERS {
            let mut want: Node = red.clone();
            want.set("Actor", Node::Str(format!("Work/Actor/{m}.engine__actor__ActorParam.gyml")));
            assert_eq!(new.root.get(HELD).unwrap().get(&hold_key(m)), Some(&want));
            assert!(rf.read_zs(&format!("Pack/Actor/{m}.pack.zs")).is_ok(), "{m} has no pack");
        }
        assert_eq!(new.root.get(HELD).unwrap().get(MODEL), Some(red));
    }

    #[test]
    fn his_blackboard_gains_one_bool_shaped_like_his_own_and_his_blackboard_list_names_the_table() {
        let Some(mut rf) = game() else {
            return;
        };
        let (was, now): (Vec<sarc::Entry>, Vec<sarc::Entry>) = packs(&mut rf);
        let old: Byml = Byml::parse(&sarc::get(&was, TABLE).unwrap().data).unwrap();
        let new: Byml = Byml::parse(&sarc::get(&now, TABLE).unwrap().data).unwrap();
        let before: &[Node] = old.root.get(BOOLS).and_then(Node::as_arr).unwrap();
        let after: &[Node] = new.root.get(BOOLS).and_then(Node::as_arr).unwrap();
        assert_eq!(&after[..before.len()], before);
        let mut want: Node = before.iter().find(|r| r.get("BBKey").and_then(Node::as_str) == Some(BOOL_MODEL)).unwrap().clone();
        want.set("BBKey", Node::Str(BY_A_HOLE.to_string()));
        assert_eq!(&after[before.len()..], &[want]);
        let info: Byml = Byml::parse(&sarc::get(&now, "Component/Blackboard/BlackboardInfo/Npc_Hylia_Dampe_001.engine__component__BlackboardInfo.bgyml").unwrap().data).unwrap();
        let tables: Vec<&str> = info.root.get("BlackboardParamTables").and_then(Node::as_arr).unwrap().iter().filter_map(Node::as_str).collect();
        assert!(tables.contains(&"Work/Component/Blackboard/BlackboardParamTable/Npc_Hylia_Dampe_001.engine__component__BlackboardParamTable.gyml"));
    }

    #[test]
    fn his_utility_graph_wakes_one_monster_when_he_comes_to_stand_by_a_dug_hole_and_rearms_when_he_leaves() {
        let Some(mut rf) = game() else {
            return;
        };
        let (was, now): (Vec<sarc::Entry>, Vec<sarc::Entry>) = packs(&mut rf);
        let old: Ainb = graph(&was, UTILITY);
        let g: Ainb = graph(&now, UTILITY);
        let event: Ainb = graph(&was, "AI/Npc_Hylia_Dampe_001.event.root.ainb");
        assert!(audit(&g).is_empty(), "{:?}", audit(&g));
        let top: usize = g.parent_of(g.find_els(ALWAYS)[0]).unwrap().0;
        let (k, o): (Vec<usize>, Vec<usize>) = (kids(&g, top), kids(&old, old.parent_of(old.find_els(ALWAYS)[0]).unwrap().0));
        assert_eq!(k[..o.len()], o[..], "his own utility modules are kept");
        assert_eq!(k.len(), o.len() + 1);
        let gate: usize = k[o.len()];
        assert!(on_his_key(&g, source(&g, gate, 1, "Input")));
        let [leave, arrive] = kids(&g, gate)[..] else { panic!("the gate has two branches") };
        let by: usize = source(&g, arrive, 1, "Input");
        assert_eq!(source(&g, leave, 1, "Input"), by);
        assert_eq!(g.el_name(by), CONJUNCTION);
        let soil: usize = source(&g, by, 1, "ValueA");
        assert_eq!(shape(&g, soil), shape(&event, event.find_els(SOIL)[0]), "the nearest soil query is shaped like the one in his dig event");
        let close: usize = source(&g, by, 1, "ValueB");
        assert_eq!((g.el_name(close), source(&g, close, 2, "F32")), (LESS, soil));
        assert_eq!(in_val(&g, close, 2, "F32Base"), NEAR.to_le_bytes().to_vec());
        let [stay, forget] = kids(&g, leave)[..] else { panic!("leave has two branches") };
        assert_eq!(g.el_name(stay), IDLE);
        assert!(on_his_key(&g, forget));
        assert_eq!(in_val(&g, forget, 1, "Bool")[0], 0);
        let [fire, wait] = kids(&g, arrive)[..] else { panic!("arrive has two branches") };
        assert_eq!(g.el_name(wait), IDLE);
        let [remember, pick] = kids(&g, fire)[..] else { panic!("fire remembers and picks") };
        assert!(on_his_key(&g, remember));
        assert_eq!(in_val(&g, remember, 1, "Bool")[0], 1);
        assert_eq!(shape(&g, pick), shape(&event, 1146), "the pick is shaped like his own two way pick");
        let roll: usize = source(&g, pick, 0, "Input");
        assert_eq!(shape(&g, roll), shape(&event, 1145));
        assert_eq!(prop_word(&g, roll, 0, "ChoiceNum"), Some(MONSTERS.len() as u32));
        let values: Vec<i32> = g.params[pick].group(2).iter().map(|p| i32::from_le_bytes(p.extra[4..8].try_into().unwrap())).collect();
        assert_eq!(values, vec![0, 1, 2, 0], "cases 0, 1 and 2, the last one is the default, as in his own three way pick");
        let mut woken: Vec<String> = Vec::new();
        for r in kids(&g, pick) {
            let [wake, cut] = kids(&g, r)[..] else { panic!("each pick wakes and lets go") };
            assert_eq!((g.el_name(wake), g.el_name(cut)), (WAKE, CUT));
            assert_eq!(g.in_str(wake, "Key"), g.in_str(cut, "Key"));
            woken.push(g.in_str(wake, "Key").unwrap().to_string());
            let beside: usize = source(&g, wake, 4, "Pos");
            assert_eq!((g.el_name(beside), source(&g, beside, 4, "InputA")), (LIFT, soil));
            assert_eq!(in_val(&g, beside, 4, "InputB"), vec3(BESIDE));
        }
        assert_eq!(woken, MONSTERS.iter().map(|m| hold_key(m)).collect::<Vec<String>>());
    }
}
