use crate::ainb::build::Spec;
use crate::ainb::{audit, Ainb, Plug};
use crate::difficulty::hook::{self, flag, PARALLEL};
use crate::error::{Error, Res};

pub const STATE: u16 = 500;
pub const SEQUENCE: u16 = 2;
pub const DRAW: &str = "QueryGenericGetRandomBool";
pub const PLAY: &str = "ExecuteASPlay";
pub const COUNTER: &str = "ExecuteGenericCounter";
pub const SLOW: &str = "ExecutePhysicsCctReduceVelocityByPlug";
pub const GO: &str = "ExecuteGenericSetResultSuccess";
pub const POSE: &str = "Wait";
pub const COMMAND: &str = "ASCommand";
pub const PAUSE_FRAMES: f32 = 39.0;
pub const PAUSE_EXTRA: f32 = 39.0;
pub const SLOW_RATIO: f32 = 0.7;
pub const ON_FINISH: i32 = 2;
pub const FIRST_CHILD: i32 = 1;
pub const ENTRIES: [&str; 2] = ["Enter", "Update"];
pub const UPDATE: &str = "Update";
pub const STR: usize = 3;
pub const REC: usize = 12;
pub const FEINT_NODES: usize = 8;
pub const PICK_NODES: usize = 3;
pub const GAP_NODES: usize = 6;

#[derive(Clone, Copy, Debug)]
pub enum Swing {
    Copy(usize),
    Named(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub els: usize,
    pub state: usize,
    pub body: usize,
    pub swing: Swing,
    pub cuts: &'static [f32],
    pub gap: Option<(usize, usize)>,
}

impl Shape {
    pub fn added(&self) -> usize {
        let split: usize = if self.cuts.len() > 1 { PICK_NODES - 1 } else { 0 };
        let feints: usize = self.cuts.len() * FEINT_NODES + split + PICK_NODES + 1;
        feints + if self.gap.is_some() { GAP_NODES } else { 0 }
    }

    pub fn chance(&self, p: f32) -> f32 {
        if self.gap.is_some() || self.cuts.len() > 1 { p / 2.0 } else { p }
    }
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
        .prop(3, COMMAND, cmd.to_le_bytes().to_vec())
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

pub fn prop_at(a: &Ainb, el: usize, k: usize, name: &str) -> Option<usize> {
    let p = a.params.get(el)?;
    let (b, n): (usize, usize) = (p.pe[k][0] as usize, p.pe[k][1] as usize);
    (b..b + n).find(|g| a.prop_name(k, *g) == Some(name))
}

pub fn word(a: &Ainb, k: usize, g: usize, at: usize) -> Option<u32> {
    let r: &[u8] = a.props[k].get(g * REC + at..g * REC + at + 4)?;
    Some(u32::from_le_bytes([r[0], r[1], r[2], r[3]]))
}

pub fn command(a: &Ainb, el: usize) -> Option<(String, u32)> {
    let g: usize = prop_at(a, el, STR, COMMAND)?;
    Some((a.s(word(a, STR, g, 8)?).to_string(), word(a, STR, g, 4)?))
}

fn set_flags(a: &mut Ainb, el: usize, v: u32) -> Res<()> {
    let g: usize = prop_at(a, el, STR, COMMAND).ok_or(Error::Format("ainb: the new play has no command"))?;
    let d: &mut [u8] = a.props[STR].get_mut(g * REC + 4..g * REC + 8).ok_or(Error::Format("ainb: property out of range"))?;
    d.copy_from_slice(&v.to_le_bytes());
    Ok(())
}

fn swing_of(a: &mut Ainb, sh: &Shape, label: &str) -> Res<(u32, u32)> {
    match sh.swing {
        Swing::Named(n) => Ok((a.pool.add(n), 0)),
        Swing::Copy(el) => {
            if el >= a.els.len() || a.el_name(el) != PLAY {
                return Err(Error::Msg(format!("{label}: element {el} is no longer the attack's animation")));
            }
            let (cmd, fl): (String, u32) = command(a, el).ok_or_else(|| Error::Msg(format!("{label}: the attack's animation has no command")))?;
            Ok((a.pool.add(&cmd), fl))
        }
    }
}

fn pause(a: &mut Ainb, seed: &str) -> Res<usize> {
    let pose: u32 = a.pool.add(POSE);
    let empty: u32 = a.pool.add("");
    let hold: usize = a.add_node(&counter(&format!("{seed} pause timer"), PAUSE_FRAMES, PAUSE_EXTRA))?;
    let idle: usize = a.add_node(&play(&format!("{seed} pause"), pose, empty))?;
    a.add_node(&timed(&format!("{seed} wait"), &[hold, idle]))
}

fn feint(a: &mut Ainb, seed: &str, swing: (u32, u32), cut: f32) -> Res<usize> {
    let empty: u32 = a.pool.add("");
    let timer: usize = a.add_node(&counter(&format!("{seed} swing timer"), cut, 0.0))?;
    let fake: usize = a.add_node(&play(&format!("{seed} fake swing"), swing.0, empty))?;
    set_flags(a, fake, swing.1)?;
    let slow: usize = a.add_node(&Spec::node(SLOW, &format!("{seed} slow")).constant(2, "AngularVelocityReduceRatio", 1f32.to_le_bytes().to_vec()).idle(2, "LinearVelocityReduceRatio", SLOW_RATIO.to_le_bytes().to_vec()))?;
    let windup: usize = a.add_node(&timed(&format!("{seed} wind up"), &[timer, fake, slow]))?;
    let wait: usize = pause(a, seed)?;
    a.add_node(&sequence(&format!("{seed} feint"), windup, wait))
}

fn chance<'a>(a: &mut Ainb, seed: &'a str, p: f32, yes: usize) -> Res<usize> {
    let roll: usize = a.add_node(&draw(&format!("{seed} roll"), p))?;
    let go: usize = a.add_node(&Spec::node(GO, &format!("{seed} no feint")))?;
    a.add_node(&gate(&format!("{seed} gate"), roll, yes, go))
}

pub fn entry(a: &Ainb, sh: &Shape, label: &str) -> Res<Vec<usize>> {
    if a.els.len() != sh.els || a.el_type(sh.state) != STATE {
        return Err(Error::Msg(format!("{label}: the attack module no longer has its usual shape")));
    }
    let base: usize = a.params[sh.state].pl[2][1] as usize;
    let at: Vec<usize> = a.params[sh.state].group(2).iter().enumerate().filter(|(_, g)| g.a as usize == sh.body && ENTRIES.contains(&a.s(g.b))).map(|(j, _)| base + j).collect();
    if !at.iter().any(|j| a.s(a.params[sh.state].plugs[*j].b) == UPDATE) {
        return Err(Error::Msg(format!("{label}: the attack module's state no longer runs its body on update")));
    }
    if let Some((seq, pos)) = sh.gap {
        if a.el_type(seq) != SEQUENCE || a.params[seq].group(2).len() <= pos {
            return Err(Error::Msg(format!("{label}: the combo sequence is no longer there")));
        }
    }
    Ok(at)
}

pub fn build(a: &mut Ainb, sh: &Shape, p: f32, label: &str) -> Res<usize> {
    let at: Vec<usize> = entry(a, sh, label)?;
    let tag: String = format!("ootrr enemy feints {label}");
    let q: f32 = sh.chance(p);
    let swing: (u32, u32) = swing_of(a, sh, label)?;
    let mut fs: Vec<usize> = Vec::new();
    for (i, c) in sh.cuts.iter().enumerate() {
        fs.push(feint(a, &format!("{tag} {i}"), swing, *c)?);
    }
    let branch: usize = match fs.as_slice() {
        [one] => *one,
        [first, second] => {
            let roll: usize = a.add_node(&draw(&format!("{tag} which"), 0.5))?;
            a.add_node(&gate(&format!("{tag} which gate"), roll, *first, *second))?
        }
        _ => return Err(Error::Msg(format!("{label}: one or two cut points are supported"))),
    };
    let pick: usize = chance(a, &tag, if fs.len() > 1 { p } else { q }, branch)?;
    let top: usize = a.add_node(&sequence(&format!("{tag} then attack"), pick, sh.body))?;
    for j in at {
        a.params[sh.state].plugs[j].a = top as u32;
    }
    if let Some((seq, pos)) = sh.gap {
        let seed: String = format!("{tag} gap");
        let wait: usize = pause(a, &seed)?;
        let gated: usize = chance(a, &seed, q, wait)?;
        let name: u32 = a.pool.add("");
        a.params[seq].add_plug(2, pos, Plug { a: gated as u32, b: name, extra: Vec::new() })?;
    }
    Ok(top)
}

pub fn patch(raw: &[u8], sh: &Shape, p: f32, label: &str) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    if taken(&a) {
        return Err(Error::Msg(format!("{label}: the attack module is already patched")));
    }
    build(&mut a, sh, p, label)?;
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(Error::Msg(format!("{label}: {}", bad.join("; "))));
    }
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if back.write()? != out || back.els.len() != a.els.len() || back.params != a.params || back.props != a.props || back.ins != a.ins || back.outs != a.outs || back.qids != a.qids || back.pool != a.pool {
        return Err(Error::Msg(format!("{label}: the written graph does not read back")));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ainb::show::show;
    use crate::difficulty::feint::{Target, TARGETS};
    use crate::difficulty::hook::SWITCH;
    use crate::romfs::Romfs;
    use crate::sarc::{self, Entry};

    fn dump() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from)?;
        if p.is_dir() { Some(p) } else { None }
    }

    pub fn copies(rf: &mut Romfs) -> Vec<(String, String, Vec<u8>)> {
        let mut packs: Vec<String> = vec![hook::GLOBAL.to_string()];
        packs.extend(rf.list("Pack/Actor").unwrap());
        let mut out: Vec<(String, String, Vec<u8>)> = Vec::new();
        for f in packs {
            let Ok(raw) = rf.read_zs(&f) else { continue };
            let Ok(es) = sarc::parse(&raw) else { continue };
            for e in es.into_iter().filter(|e| TARGETS.iter().any(|t| t.graph == e.name)) {
                out.push((f.clone(), e.name, e.data));
            }
        }
        out
    }

    fn shape(a: &Ainb, i: usize) -> Vec<String> {
        let all: String = show(a);
        let from: usize = all.find(&format!("element {i} type")).unwrap();
        let body: &str = &all[from..];
        let end: usize = body[1..].find("\nelement ").map(|e| e + 1).unwrap_or(body.len());
        body[..end].lines().skip(1).filter(|l| !l.contains("plug ")).map(|l| l.split(" = ").next().unwrap_or(l).split(" from ").next().unwrap_or(l).to_string()).collect()
    }

    fn bare(e: &[u8; crate::ainb::EL]) -> Vec<u8> {
        e.iter().enumerate().map(|(i, b)| if (0x14..0x18).contains(&i) { 0 } else { *b }).collect()
    }

    fn kids(a: &Ainb, i: usize) -> Vec<usize> {
        a.params[i].group(2).iter().map(|g| g.a as usize).collect()
    }

    fn check(t: &Target, raw: &[u8]) {
        let sh: &Shape = &t.shape;
        let old: Ainb = Ainb::parse(raw).unwrap();
        assert_eq!(old.write().unwrap(), raw, "{}: the untouched module rewrites byte for byte", t.kind);
        assert!(!taken(&old), "{}: the vanilla module has no random draw", t.kind);
        let at: Vec<usize> = entry(&old, sh, t.kind).unwrap();
        let want: (String, u32) = match sh.swing {
            Swing::Copy(el) => command(&old, el).unwrap(),
            Swing::Named(n) => (n.to_string(), 0),
        };
        let out: Vec<u8> = patch(raw, sh, 0.25, t.kind).unwrap();
        let a: Ainb = Ainb::parse(&out).unwrap();
        let n: usize = old.els.len();
        assert_eq!(a.els.len(), n + sh.added(), "{}", t.kind);
        for i in 0..n {
            assert_eq!(bare(&a.els[i]), bare(&old.els[i]), "{}: element {i} keeps its record", t.kind);
            if i != sh.state && sh.gap.map(|g| g.0) != Some(i) {
                assert_eq!(a.params[i], old.params[i], "{}: element {i} is unchanged", t.kind);
            }
        }
        for k in 0..6 {
            assert_eq!(&a.props[k][..old.props[k].len()], &old.props[k][..]);
            assert_eq!(&a.ins[k][..old.ins[k].len()], &old.ins[k][..]);
            assert_eq!(&a.outs[k][..old.outs[k].len()], &old.outs[k][..]);
        }
        assert_eq!(&a.qids[..old.qids.len()], &old.qids[..]);
        let top: usize = (n..a.els.len()).find(|i| a.el_type(*i) == SEQUENCE && kids(&a, *i).get(1) == Some(&sh.body)).unwrap();
        for j in &at {
            assert_eq!(a.params[sh.state].plugs[*j].a as usize, top, "{}: the state runs the feint first", t.kind);
        }
        let others: Vec<(u32, u32)> = a.params[sh.state].plugs.iter().enumerate().filter(|(j, _)| !at.contains(j)).map(|(_, g)| (g.a, g.b)).collect();
        let was: Vec<(u32, u32)> = old.params[sh.state].plugs.iter().enumerate().filter(|(j, _)| !at.contains(j)).map(|(_, g)| (g.a, g.b)).collect();
        assert_eq!(others, was, "{}: the state's other plugs are kept", t.kind);
        let pick: usize = kids(&a, top)[0];
        assert_eq!(a.el_type(pick), SWITCH);
        let plays: Vec<usize> = (n..a.els.len()).filter(|i| a.el_name(*i) == PLAY).collect();
        let fakes: Vec<usize> = plays.iter().copied().filter(|i| command(&a, *i).map(|c| c.0) != Some(POSE.to_string())).collect();
        assert_eq!(fakes.len(), sh.cuts.len(), "{}", t.kind);
        for f in &fakes {
            assert_eq!(command(&a, *f).unwrap(), want, "{}: the fake swing plays the real attack's command, binding kept", t.kind);
        }
        let pauses: usize = plays.len() - fakes.len();
        assert_eq!(pauses, sh.cuts.len() + sh.gap.map_or(0, |_| 1));
        let shown: String = show(&a);
        for c in sh.cuts {
            assert!(shown.contains(&format!("prop f32 BaseFrameTime = {c}\n  prop f32 RandomTimeMax = 0")), "{}: cut {c}", t.kind);
        }
        assert_eq!(shown.matches(&format!("prop f32 BaseFrameTime = {PAUSE_FRAMES}\n  prop f32 RandomTimeMax = {PAUSE_EXTRA}")).count(), pauses);
        let q: f32 = sh.chance(0.25);
        let draws: Vec<usize> = a.find_els(DRAW);
        assert_eq!(draws.len(), 1 + usize::from(sh.cuts.len() > 1) + usize::from(sh.gap.is_some()));
        if sh.cuts.len() > 1 {
            assert!(shown.contains("in f32 TrueProbability = 0.25 flags 0x800000"));
            assert!(shown.contains("in f32 TrueProbability = 0.5 flags 0x800000"));
        } else {
            assert!(shown.contains(&format!("in f32 TrueProbability = {q} flags 0x800000")), "{}", t.kind);
        }
        for d in &draws {
            assert!(shown.contains(&format!("element {d} type 0 {DRAW}\n  prop bool OutputOnlyOnEnter = true")));
        }
        for s in (n..a.els.len()).filter(|i| a.el_type(*i) == PARALLEL) {
            assert_eq!(a.el_name(kids(&a, s)[0]), COUNTER, "{}: each feint stage ends on its own timer", t.kind);
        }
        if let Some((seq, pos)) = sh.gap {
            let ks: Vec<usize> = kids(&a, seq);
            let was: Vec<usize> = kids(&old, seq);
            assert_eq!(ks.len(), was.len() + 1);
            assert_eq!(&ks[..pos], &was[..pos]);
            assert_eq!(&ks[pos + 1..], &was[pos..]);
            assert_eq!(a.el_type(ks[pos]), SWITCH, "{}: the pause between swings is gated", t.kind);
            let yes: usize = a.params[ks[pos]].group(2).iter().find(|g| a.s(g.b) == "True").unwrap().a as usize;
            assert_eq!(a.el_type(yes), PARALLEL);
            assert!(shown.contains(&format!("in f32 TrueProbability = {q} flags 0x800000")));
        }
        assert_eq!(shown.matches("Blackboard").count(), show(&old).matches("Blackboard").count(), "{}: the feint reads and writes no key", t.kind);
        assert!(patch(&out, sh, 0.25, t.kind).is_err(), "{}: a patched module is not patched twice", t.kind);
        let all: Ainb = Ainb::parse(&patch(raw, sh, 1.0, t.kind).unwrap()).unwrap();
        let sure: usize = if sh.cuts.len() > 1 { 1 } else if sh.gap.is_some() { 0 } else { 1 };
        assert_eq!(show(&all).matches("in f32 FalseProbability = 0 flags 0x800000").count(), sure, "{}", t.kind);
    }

    #[test]
    fn every_attack_module_gains_its_feints_in_front_of_the_vanilla_attack() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let found: Vec<(String, String, Vec<u8>)> = copies(&mut rf);
        for t in &TARGETS {
            let mine: Vec<&(String, String, Vec<u8>)> = found.iter().filter(|c| c.1 == t.graph).collect();
            assert!(!mine.is_empty(), "{} is in some pack", t.graph);
            assert!(mine.iter().all(|c| c.2 == mine[0].2), "{}: every copy is the same file, so every copy gets the same edit", t.graph);
            check(t, &mine[0].2);
        }
        let n: Vec<(&str, usize)> = TARGETS.iter().map(|t| (t.kind, found.iter().filter(|c| c.1 == t.graph).count())).collect();
        assert_eq!(n, vec![("Stalfos swing", 1), ("Stalfos combo", 4), ("Stalfos jump", 1), ("Lizalfos swing", 1), ("Dinolfos swing", 1), ("Iron Knuckle overhead", 1), ("Iron Knuckle double", 4), ("Deku Baba bite", 2)]);
    }

    #[test]
    fn the_stalfos_swings_play_the_command_their_caller_binds() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let found: Vec<(String, String, Vec<u8>)> = copies(&mut rf);
        for g in [TARGETS[0].graph, TARGETS[1].graph] {
            let raw: &[u8] = &found.iter().find(|c| c.1 == g).unwrap().2;
            let a: Ainb = Ainb::parse(raw).unwrap();
            let el: usize = a.find_els(PLAY)[0];
            assert_eq!(command(&a, el).unwrap().1, 0x8000_0000, "{g}: the vanilla swing reads the module's ASCommand parameter");
            assert_eq!(&a.bb[0..2], &[1, 0], "{g}: one string parameter");
        }
    }

    #[test]
    fn the_new_nodes_have_the_shapes_the_game_uses() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let found: Vec<(String, String, Vec<u8>)> = copies(&mut rf);
        let es: Vec<Entry> = sarc::parse(&rf.read_zs(hook::GLOBAL).unwrap()).unwrap();
        let moblin: Vec<Entry> = sarc::parse(&rf.read_zs("Pack/Actor/Enemy_Moblin_Club.pack.zs").unwrap()).unwrap();
        let m: Ainb = Ainb::parse(&sarc::get(&moblin, "AI/MoblinClub.action.basicattack.module.ainb").unwrap().data).unwrap();
        let dark: Ainb = Ainb::parse(&sarc::get(&es, "AI/ExecuteDarkLinkMirrorAction.module.ainb").unwrap().data).unwrap();
        let stalfos: Ainb = Ainb::parse(&found.iter().find(|c| c.1 == TARGETS[0].graph).unwrap().2).unwrap();
        let ms: String = show(&m);
        let seq: usize = (0..m.els.len()).find(|i| m.el_type(*i) == SEQUENCE && ms.contains(&format!("element {i} type 2 \n  prop s32 BusyPolicy = 0\n  prop s32 NumLoop = 0\n  prop s32 PlayPolicy = 0\n  prop s32 ResultPolicy = 0\n  prop bool IsDelayJudge = false\n  prop bool IsLoop = false"))).unwrap();
        let tm: usize = (0..m.els.len()).find(|i| m.el_type(*i) == PARALLEL && ms.contains(&format!("element {i} type 3 \n  prop s32 EndPolicy = 0\n  prop s32 ResultPolicy = 1"))).unwrap();
        let sw: usize = (0..stalfos.els.len()).find(|i| stalfos.el_type(*i) == SWITCH).unwrap();
        for t in &TARGETS {
            let raw: &[u8] = &found.iter().find(|c| c.1 == t.graph).unwrap().2;
            let old: Ainb = Ainb::parse(raw).unwrap();
            let a: Ainb = Ainb::parse(&patch(raw, &t.shape, 0.25, t.kind).unwrap()).unwrap();
            for i in old.els.len()..a.els.len() {
                let want: Vec<String> = match (a.el_type(i), a.el_name(i)) {
                    (0, PLAY) => shape(&stalfos, stalfos.find_els(PLAY)[0]),
                    (0, SLOW) => shape(&stalfos, stalfos.find_els(SLOW)[0]),
                    (0, GO) => shape(&stalfos, stalfos.find_els(GO)[0]),
                    (0, COUNTER) => shape(&m, m.find_els(COUNTER)[0]),
                    (0, DRAW) => shape(&dark, dark.find_els(DRAW)[0]),
                    (SEQUENCE, _) => shape(&m, seq),
                    (PARALLEL, _) => shape(&m, tm),
                    (SWITCH, _) => shape(&stalfos, sw),
                    other => panic!("{}: unexpected new node {other:?}", t.kind),
                };
                assert_eq!(shape(&a, i), want, "{}: element {i}", t.kind);
            }
        }
    }

    #[test]
    fn a_changed_module_is_refused() {
        assert!(patch(b"nope", &TARGETS[0].shape, 0.25, "t").is_err());
        let mut a: Ainb = crate::ainb::tests::synth();
        assert!(entry(&a, &TARGETS[0].shape, "t").is_err());
        assert!(build(&mut a, &TARGETS[0].shape, 0.25, "t").is_err());
    }
}
