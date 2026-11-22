use std::collections::BTreeSet as Set;

use crate::ainb::build::Spec;
use crate::ainb::{audit, Ainb};
use crate::difficulty::hook::{self, flag, PARALLEL};
use crate::error::{Error, Res};
use crate::qol::{edit_zs, Have};
use crate::romfs::Romfs;
use crate::sarc::{self, Entry};
use crate::staging::Out;
use crate::zs;

pub const SETTING: &str = "enemy_feints";
pub const NUM: u16 = 166;
pub const GRAPH: &str = "AI/Stalfos.action.BasicAttack.module.ainb";
pub const MODULE: &str = "Stalfos.action.BasicAttack.module";
pub const STATE: u16 = 500;
pub const SEQUENCE: u16 = 2;
pub const DRAW: &str = "QueryGenericGetRandomBool";
pub const PLAY: &str = "ExecuteASPlay";
pub const COUNTER: &str = "ExecuteGenericCounter";
pub const SLOW: &str = "ExecutePhysicsCctReduceVelocityByPlug";
pub const GO: &str = "ExecuteGenericSetResultSuccess";
pub const SWING: &str = "WeaponAttack";
pub const PAUSE: &str = "Wait";
pub const SWING_FRAMES: f32 = 18.0;
pub const PAUSE_FRAMES: f32 = 30.0;
pub const PAUSE_EXTRA: f32 = 30.0;
pub const SLOW_RATIO: f32 = 0.7;
pub const ON_FINISH: i32 = 2;
pub const FIRST_CHILD: i32 = 1;
pub const ENTRIES: [&str; 2] = ["Enter", "Update"];
pub const HEAP_EXTRA: i32 = 0x4000;
pub const ADDED: usize = 12;
pub const LABEL: &str = "enemy feints";

pub fn percent(choice: &str) -> Option<i64> {
    match choice {
        "quarter" => Some(25),
        "half" => Some(50),
        _ => None,
    }
}

pub fn note(pct: i64) -> Option<String> {
    (pct > 0).then(|| format!("About {pct} in 100 Stalfos sword attacks are feints: the Stalfos starts its swing, stops short, waits half a second to a second, then swings for real. Only Stalfos so far. Experimental, untested in the game."))
}

pub fn taken(a: &Ainb) -> bool {
    !a.find_els(DRAW).is_empty()
}

fn counter<'a>(seed: &'a str, frames: f32, extra: f32) -> Spec<'a> {
    Spec::node(COUNTER, seed)
        .prop(0, "ResultBeforeFinish", 0i32.to_le_bytes().to_vec())
        .prop(0, "ResultOnFinish", ON_FINISH.to_le_bytes().to_vec())
        .prop(2, "BaseFrameTime", frames.to_le_bytes().to_vec())
        .prop(2, "RandomTimeMax", extra.to_le_bytes().to_vec())
        .spare(2, "ElapsedFrameTime")
        .spare(2, "PercentComplete")
        .spare(2, "RemainingFrameTime")
}

fn play(seed: &str, cmd: u32, empty: u32) -> Spec<'_> {
    Spec::node(PLAY, seed)
        .prop(1, "BusyUntilFinished", flag(false))
        .prop(1, "IsIgnoreSameCommand", flag(false))
        .prop(2, "PlayRate", 1f32.to_le_bytes().to_vec())
        .prop(3, "ASCommand", cmd.to_le_bytes().to_vec())
        .prop(3, "Partial", empty.to_le_bytes().to_vec())
}

fn timed<'a>(seed: &'a str, kids: &[usize]) -> Spec<'a> {
    let mut s: Spec = Spec::composite(PARALLEL, seed).prop(0, "EndPolicy", vec![0; 4]).prop(0, "ResultPolicy", FIRST_CHILD.to_le_bytes().to_vec());
    for k in kids {
        s = s.kid(*k, "");
    }
    s
}

fn sequence<'a>(seed: &'a str, first: usize, second: usize) -> Spec<'a> {
    Spec::composite(SEQUENCE, seed)
        .prop(0, "BusyPolicy", vec![0; 4])
        .prop(0, "NumLoop", vec![0; 4])
        .prop(0, "PlayPolicy", vec![0; 4])
        .prop(0, "ResultPolicy", vec![0; 4])
        .prop(1, "IsDelayJudge", flag(false))
        .prop(1, "IsLoop", flag(false))
        .prop(1, "IsUpdateNextInFrame", flag(false))
        .kid(first, "")
        .kid(second, "")
}

fn draw(seed: &str, p: f32) -> Spec<'_> {
    let mut s: Spec = Spec::query(DRAW, seed)
        .constant(2, "FalseProbability", (1.0 - p).to_le_bytes().to_vec())
        .constant(2, "TrueProbability", p.to_le_bytes().to_vec())
        .out(1, "Bool");
    for x in s.props.iter_mut().filter(|x| x.1 == "OutputOnlyOnEnter") {
        x.2 = flag(true);
    }
    s
}

fn gate(seed: &str, input: usize, yes: usize, no: usize) -> Spec<'_> {
    let mut s: Spec = hook::switch(seed, input, yes, no);
    for x in s.props.iter_mut().filter(|x| x.1 == "IsNoSelectWhenChildBusy") {
        x.2 = flag(true);
    }
    s
}

pub fn entry(a: &Ainb) -> Res<(usize, Vec<usize>, usize)> {
    let states: Vec<usize> = (0..a.els.len()).filter(|i| a.el_type(*i) == STATE).collect();
    let [st] = states.as_slice() else {
        return Err(Error::Msg(format!("{LABEL}: the attack module no longer has one state root")));
    };
    let base: usize = a.params[*st].pl[2][1] as usize;
    let mut at: Vec<usize> = Vec::new();
    let mut body: Vec<usize> = Vec::new();
    for (j, g) in a.params[*st].group(2).iter().enumerate() {
        if ENTRIES.contains(&a.s(g.b)) {
            at.push(base + j);
            body.push(g.a as usize);
        }
    }
    body.dedup();
    match body.as_slice() {
        [b] if at.len() == ENTRIES.len() && a.el_type(*b) == PARALLEL => Ok((*st, at, *b)),
        _ => Err(Error::Msg(format!("{LABEL}: the attack module no longer starts with one parallel"))),
    }
}

pub fn build(a: &mut Ainb, p: f32) -> Res<usize> {
    let (st, at, attack): (usize, Vec<usize>, usize) = entry(a)?;
    let tag: String = format!("ootrr {LABEL}");
    let s = |n: &str| -> String { format!("{tag} {n}") };
    let swing: u32 = a.pool.add(SWING);
    let pause: u32 = a.pool.add(PAUSE);
    let empty: u32 = a.pool.add("");
    let roll: usize = a.add_node(&draw(&s("roll"), p))?;
    let cut: usize = a.add_node(&counter(&s("swing timer"), SWING_FRAMES, 0.0))?;
    let fake: usize = a.add_node(&play(&s("fake swing"), swing, empty))?;
    let slow: usize = a.add_node(&Spec::node(SLOW, &s("slow")).constant(2, "AngularVelocityReduceRatio", 1f32.to_le_bytes().to_vec()).idle(2, "LinearVelocityReduceRatio", SLOW_RATIO.to_le_bytes().to_vec()))?;
    let windup: usize = a.add_node(&timed(&s("wind up"), &[cut, fake, slow]))?;
    let hold: usize = a.add_node(&counter(&s("pause timer"), PAUSE_FRAMES, PAUSE_EXTRA))?;
    let idle: usize = a.add_node(&play(&s("pause"), pause, empty))?;
    let wait: usize = a.add_node(&timed(&s("wait"), &[hold, idle]))?;
    let feint: usize = a.add_node(&sequence(&s("feint"), windup, wait))?;
    let go: usize = a.add_node(&Spec::node(GO, &s("no feint")))?;
    let pick: usize = a.add_node(&gate(&s("gate"), roll, feint, go))?;
    let top: usize = a.add_node(&sequence(&s("then attack"), pick, attack))?;
    for j in at {
        a.params[st].plugs[j].a = top as u32;
    }
    Ok(top)
}

pub fn patch(raw: &[u8], p: f32) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    if taken(&a) {
        return Err(Error::Msg(format!("{LABEL}: the attack module is already patched")));
    }
    build(&mut a, p)?;
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(Error::Msg(format!("{LABEL}: {}", bad.join("; "))));
    }
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if back.write()? != out || back.els.len() != a.els.len() || back.params != a.params || back.props != a.props || back.ins != a.ins || back.outs != a.outs || back.qids != a.qids || back.pool != a.pool {
        return Err(Error::Msg(format!("{LABEL}: the written graph does not read back")));
    }
    Ok(out)
}

pub fn callers(es: &[Entry]) -> Set<String> {
    es.iter().filter(|e| e.name.ends_with(".ainb") && e.name != GRAPH && hook::has(&e.data, MODULE)).map(|e| e.name.clone()).collect()
}

pub fn runs(es: &[Entry], shared: &Set<String>) -> bool {
    sarc::get(es, GRAPH).is_some() || !callers(es).is_empty() || !hook::shared_of(es, shared).is_empty()
}

fn repack(path: &str, z: &[u8], old: usize, es: &[Entry]) -> Res<Out> {
    let out: Vec<u8> = sarc::write(es)?;
    Ok(Out { path: path.to_string(), old, new: out.len(), z: zs::compress_raw(&out, zs::dict_of(z))? })
}

fn patch_pack(es: &mut [Entry], p: f32) -> Res<bool> {
    let Some(e) = es.iter_mut().find(|e| e.name == GRAPH) else {
        return Ok(false);
    };
    e.data = patch(&e.data, p)?;
    Ok(true)
}

pub fn stage(rf: &mut Romfs, have: Have, dir: &str, pct: i64) -> Res<(Set<String>, Vec<Out>)> {
    let p: f32 = pct as f32 / 100.0;
    let (z, old, mut es): (Vec<u8>, usize, Vec<Entry>) = hook::opened(rf, have, hook::GLOBAL)?;
    let shared: Set<String> = callers(&es);
    if !patch_pack(&mut es, p)? {
        return Err(Error::Msg(format!("{LABEL}: the AI pack has no Stalfos attack module")));
    }
    let mut outs: Vec<Out> = vec![repack(hook::GLOBAL, &z, old, &es)?];
    let mut who: Set<String> = Set::new();
    for f in rf.list(dir)? {
        let Some(name) = hook::actor_of(&f).map(str::to_string) else {
            continue;
        };
        let (z, old, mut es): (Vec<u8>, usize, Vec<Entry>) = hook::opened(rf, have, &f)?;
        if !runs(&es, &shared) {
            continue;
        }
        who.insert(name);
        if patch_pack(&mut es, p)? {
            outs.push(repack(&f, &z, old, &es)?);
        }
    }
    if who.is_empty() {
        return Err(Error::Msg(format!("{LABEL}: no actor runs the Stalfos attack module")));
    }
    outs.push(edit_zs(rf, have, hook::ACTOR_INFO, |raw| hook::grow_heap(raw, &who, HEAP_EXTRA, LABEL))?);
    Ok((who, outs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ainb::show::show;
    use crate::byml::{Byml, Node};
    use crate::difficulty::hook::SWITCH;
    use crate::pickup_pack::rows;

    fn dump() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from)?;
        if p.is_dir() { Some(p) } else { None }
    }

    fn none(_p: &str) -> Option<Vec<u8>> {
        None
    }

    fn global(rf: &mut Romfs) -> Vec<Entry> {
        sarc::parse(&rf.read_zs(hook::GLOBAL).unwrap()).unwrap()
    }

    fn shape(a: &Ainb, i: usize) -> Vec<String> {
        let all: String = show(a);
        let from: usize = all.find(&format!("element {i} type")).unwrap();
        let body: &str = &all[from..];
        let end: usize = body[1..].find("\nelement ").map(|e| e + 1).unwrap_or(body.len());
        body[..end].lines().skip(1).filter(|l| !l.contains("plug ")).map(|l| l.split(" = ").next().unwrap_or(l).split(" from ").next().unwrap_or(l).to_string()).collect()
    }

    #[test]
    fn the_levels_read_and_default_off() {
        assert_eq!((percent("off"), percent("quarter"), percent("half"), percent("x")), (None, Some(25), Some(50), None));
        assert!(note(0).is_none());
        assert!(note(25).unwrap().contains("25 in 100"));
        use crate::settings::{Settings, Value};
        let mut bad: Vec<String> = Vec::new();
        let o: crate::difficulty::Opts = crate::difficulty::read(&Settings::default(), &mut bad);
        assert!(bad.is_empty() && o.feints == 0 && !o.any());
        let mut s: Settings = Settings::default();
        s.set(SETTING, Value::s("half")).unwrap();
        let back: Settings = Settings::decode(&s.encode().unwrap()).unwrap().settings;
        let o: crate::difficulty::Opts = crate::difficulty::read(&back, &mut bad);
        assert!(bad.is_empty() && o.feints == 50 && o.any() && !o.packs());
        assert_eq!(o.notes().len(), 1);
        s.set(SETTING, Value::s("off")).unwrap();
        assert_eq!(s.encode().unwrap(), Settings::default().encode().unwrap());
    }

    #[test]
    fn garbage_is_refused() {
        assert!(patch(b"nope", 0.25).is_err());
    }

    #[test]
    fn the_attack_module_gains_a_feint_in_front_of_the_vanilla_attack() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let es: Vec<Entry> = global(&mut rf);
        let raw: &[u8] = &sarc::get(&es, GRAPH).unwrap().data;
        let old: Ainb = Ainb::parse(raw).unwrap();
        assert_eq!(old.write().unwrap(), raw, "the untouched module rewrites byte for byte");
        assert_eq!(old.els.len(), 27);
        let (st, at, attack): (usize, Vec<usize>, usize) = entry(&old).unwrap();
        assert_eq!((st, at.len(), attack), (9, 2, 0));
        let out: Vec<u8> = patch(raw, 0.25).unwrap();
        let a: Ainb = Ainb::parse(&out).unwrap();
        assert_eq!(a.els.len(), old.els.len() + ADDED);
        let bare = |e: &[u8; crate::ainb::EL]| -> Vec<u8> { e.iter().enumerate().map(|(i, b)| if (0x14..0x18).contains(&i) { 0 } else { *b }).collect() };
        for i in 0..old.els.len() {
            assert_eq!(bare(&a.els[i]), bare(&old.els[i]), "element {i} keeps its record");
        }
        for i in 0..old.els.len() {
            if i != st {
                assert_eq!(a.params[i], old.params[i], "element {i} is unchanged");
            }
        }
        let n: usize = old.els.len();
        for k in 0..6 {
            assert_eq!(&a.props[k][..old.props[k].len()], &old.props[k][..]);
            assert_eq!(&a.ins[k][..old.ins[k].len()], &old.ins[k][..]);
            assert_eq!(&a.outs[k][..old.outs[k].len()], &old.outs[k][..]);
        }
        assert_eq!(&a.qids[..old.qids.len()], &old.qids[..]);
        let top: usize = n + ADDED - 1;
        let kids: Vec<(u32, String)> = a.params[st].group(2).iter().map(|g| (g.a, a.s(g.b).to_string())).collect();
        assert_eq!(kids, vec![(top as u32, "Enter".to_string()), (top as u32, "Update".to_string()), (10, "Leave".to_string())]);
        assert_eq!(a.el_type(top), SEQUENCE);
        let seq: Vec<usize> = a.params[top].group(2).iter().map(|g| g.a as usize).collect();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[1], attack, "the vanilla attack runs second, unchanged");
        assert_eq!(a.parent_of(attack).unwrap().0, top);
        let pick: usize = seq[0];
        assert_eq!(a.el_type(pick), SWITCH);
        let roll: usize = a.find_els(DRAW)[0];
        let shown: String = show(&a);
        assert!(shown.contains(&format!("in bool Input = false from {roll}.0")));
        assert!(shown.contains("in f32 TrueProbability = 0.25 flags 0x800000"));
        assert!(shown.contains("in f32 FalseProbability = 0.75 flags 0x800000"));
        assert!(shown.contains("prop f32 BaseFrameTime = 18"));
        assert!(shown.contains("prop f32 BaseFrameTime = 30"));
        assert!(shown.contains("prop f32 RandomTimeMax = 30"));
        assert!(shown.contains("prop str ASCommand = \"Wait\""));
        assert_eq!(shown.matches("prop str ASCommand = \"WeaponAttack\"").count(), 2);
        assert!(shape(&a, roll).contains(&"  prop bool OutputOnlyOnEnter".to_string()));
        assert!(shown.contains(&format!("element {roll} type 0 {DRAW}\n  prop bool OutputOnlyOnEnter = true")));
        let branch: Vec<(usize, String)> = a.params[pick].group(2).iter().map(|g| (g.a as usize, a.s(g.b).to_string())).collect();
        assert_eq!(branch[1].1, "False");
        assert_eq!(a.el_name(branch[1].0), GO);
        let feint: usize = branch[0].0;
        assert_eq!((branch[0].1.as_str(), a.el_type(feint)), ("True", SEQUENCE));
        for stage in a.params[feint].group(2).iter().map(|g| g.a as usize) {
            assert_eq!(a.el_type(stage), PARALLEL);
            let first: usize = a.params[stage].group(2)[0].a as usize;
            assert_eq!(a.el_name(first), COUNTER, "each stage ends on its own timer");
        }
        assert_eq!(shown.matches("Blackboard").count(), show(&old).matches("Blackboard").count(), "the feint reads and writes no key");
        assert!(patch(&out, 0.25).is_err(), "a patched module is not patched twice");
        let all: Vec<u8> = patch(raw, 1.0).unwrap();
        assert!(show(&Ainb::parse(&all).unwrap()).contains("in f32 FalseProbability = 0 flags 0x800000"));
    }

    #[test]
    fn the_new_nodes_have_the_shapes_the_game_uses() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let es: Vec<Entry> = global(&mut rf);
        let raw: &[u8] = &sarc::get(&es, GRAPH).unwrap().data;
        let old: Ainb = Ainb::parse(raw).unwrap();
        let a: Ainb = Ainb::parse(&patch(raw, 0.25).unwrap()).unwrap();
        let mine = |name: &str| -> usize { (old.els.len()..a.els.len()).find(|i| a.el_name(*i) == name).unwrap() };
        assert_eq!(shape(&a, mine(PLAY)), shape(&old, old.find_els(PLAY)[0]));
        assert_eq!(shape(&a, mine(SLOW)), shape(&old, old.find_els(SLOW)[0]));
        assert_eq!(shape(&a, mine(GO)), shape(&old, old.find_els(GO)[0]));
        let comp = |a: &Ainb, from: usize, ty: u16| -> usize { (from..a.els.len()).find(|i| a.el_type(*i) == ty).unwrap() };
        assert_eq!(shape(&a, comp(&a, old.els.len(), PARALLEL)), shape(&old, comp(&old, 0, PARALLEL)));
        assert_eq!(shape(&a, comp(&a, old.els.len(), SWITCH)), shape(&old, comp(&old, 0, SWITCH)));
        let moblin: Vec<Entry> = sarc::parse(&rf.read_zs("Pack/Actor/Enemy_Moblin_Club.pack.zs").unwrap()).unwrap();
        let m: Ainb = Ainb::parse(&sarc::get(&moblin, "AI/MoblinClub.action.basicattack.module.ainb").unwrap().data).unwrap();
        assert_eq!(shape(&a, mine(COUNTER)), shape(&m, m.find_els(COUNTER)[0]));
        let seq: usize = (0..m.els.len()).find(|i| m.el_type(*i) == SEQUENCE && show(&m).contains(&format!("element {i} type 2 \n  prop s32 BusyPolicy = 0\n  prop s32 NumLoop = 0\n  prop s32 PlayPolicy = 0\n  prop s32 ResultPolicy = 0\n  prop bool IsDelayJudge = false\n  prop bool IsLoop = false"))).unwrap();
        assert_eq!(shape(&a, comp(&a, old.els.len(), SEQUENCE)), shape(&m, seq));
        let timed: usize = (0..m.els.len()).find(|i| m.el_type(*i) == PARALLEL && show(&m).contains(&format!("element {i} type 3 \n  prop s32 EndPolicy = 0\n  prop s32 ResultPolicy = 1"))).unwrap();
        assert_eq!(shape(&a, comp(&a, old.els.len(), PARALLEL)), shape(&m, timed));
        let dark: Ainb = Ainb::parse(&sarc::get(&es, "AI/ExecuteDarkLinkMirrorAction.module.ainb").unwrap().data).unwrap();
        assert_eq!(shape(&a, mine(DRAW)), shape(&dark, dark.find_els(DRAW)[0]));
    }

    #[test]
    fn the_stalfos_actors_get_room_and_only_the_module_changes() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let shared: Set<String> = callers(&global(&mut rf));
        assert!(shared.contains("AI/Stalfos.action.JumpAttack.module.ainb"), "{shared:?}");
        let (who, outs): (Set<String>, Vec<Out>) = stage(&mut rf, &none, "Pack/Actor", 25).unwrap();
        assert!(who.contains("Enemy_Stalfos"), "{who:?}");
        for x in ["Player_Child", "Enemy_Keese", "Enemy_Lizalfos", "Enemy_Moblin_Club"] {
            assert!(!who.contains(x), "{x} does not run the Stalfos attack");
        }
        assert_eq!(who.iter().map(String::as_str).collect::<Vec<&str>>(), vec!["Enemy_Stalfos", "Enemy_Stalfos_AlwaysRespawn", "Enemy_Stalfos_ForChild", "Enemy_Stalfos_Weak"]);
        let paths: Vec<&str> = outs.iter().map(|x| x.path.as_str()).collect();
        assert_eq!((paths[0], paths[paths.len() - 1], paths.len()), (hook::GLOBAL, hook::ACTOR_INFO, 2), "{paths:?}");
        assert_eq!(zs::dict_of(&outs[0].z), zs::dict_of(&rf.read(hook::GLOBAL).unwrap()));
        let now: Vec<Entry> = sarc::parse(&rf.zs.decompress(&outs[0].z).unwrap()).unwrap();
        let was: Vec<Entry> = global(&mut rf);
        assert!(outs[0].new > outs[0].old);
        let changed: Vec<&str> = now.iter().zip(was.iter()).filter(|(x, y)| x.name != y.name || x.data != y.data).map(|(x, _)| x.name.as_str()).collect();
        assert_eq!(changed, vec![GRAPH]);
        let before: Byml = rf.read_byml(hook::ACTOR_INFO).unwrap();
        let after: Byml = Byml::parse(&rf.zs.decompress(&outs[1].z).unwrap()).unwrap();
        let heap = |y: &Byml, n: &str| -> Option<Node> { rows::find(y, n)?.get(rows::HEAP).cloned() };
        let h: i32 = match heap(&before, "Enemy_Stalfos") {
            Some(Node::I32(v)) => v,
            other => panic!("{other:?}"),
        };
        assert_eq!(heap(&after, "Enemy_Stalfos"), Some(Node::I32(h + HEAP_EXTRA)));
        let mut t: crate::rsizetable::Rst = crate::rsizetable::load(&mut rf).unwrap();
        let was_size: Option<u32> = t.get(GRAPH);
        for o in &outs {
            crate::staging::grow_out(&mut rf, &mut t, o).unwrap();
        }
        assert!(was_size.is_some() && t.get(GRAPH) > was_size, "the module's own size table entry grew");
    }

    #[test]
    fn feints_stack_on_regen_in_the_shared_pack() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let o: crate::difficulty::Opts = crate::difficulty::Opts { regen: true, feints: 50, ..crate::difficulty::Opts::default() };
        let outs: Vec<Out> = crate::difficulty::stage(&mut rf, &o, &none, "t").unwrap();
        let g: &Out = outs.iter().find(|x| x.path == hook::GLOBAL).unwrap();
        let es: Vec<Entry> = sarc::parse(&rf.zs.decompress(&g.z).unwrap()).unwrap();
        assert!(taken(&Ainb::parse(&sarc::get(&es, GRAPH).unwrap().data).unwrap()));
        let base: Ainb = Ainb::parse(&sarc::get(&es, "AI/EnemyBase.utility.root.ainb").unwrap().data).unwrap();
        assert!(!base.find_els(crate::difficulty::regen::HEAL).is_empty(), "regen is kept");
        let table: Byml = Byml::parse(&rf.zs.decompress(&outs.iter().find(|x| x.path == hook::ACTOR_INFO).unwrap().z).unwrap()).unwrap();
        let before: Byml = rf.read_byml(hook::ACTOR_INFO).unwrap();
        let h = |y: &Byml| -> Option<Node> { rows::find(y, "Enemy_Stalfos")?.get(rows::HEAP).cloned() };
        let Some(Node::I32(v)) = h(&before) else { panic!() };
        assert_eq!(h(&table), Some(Node::I32(v + HEAP_EXTRA + crate::difficulty::regen::HEAP_EXTRA)));
    }
}
