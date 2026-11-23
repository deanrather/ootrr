use crate::ainb::build::Spec;
use crate::ainb::{audit, Ainb};
use crate::difficulty::hook::{self, flag, PARALLEL};
use crate::error::{Error, Res};

pub const STATE: u16 = 500;
pub const SEQUENCE: u16 = 2;
pub const DRAW: &str = "QueryGenericGetRandomBool";
pub const PLAY: &str = "ExecuteASPlay";
pub const COUNTER: &str = "ExecuteGenericCounter";
pub const RATE: &str = "ExecuteSetASRate";
pub const BUSY: &str = "ExecuteGenericSetResultBusy";
pub const IDLE: &str = "ExecuteGenericDummyAction";
pub const COMMAND: &str = "ASCommand";
pub const PAUSE_FRAMES: f32 = 39.0;
pub const PAUSE_EXTRA: f32 = 39.0;
pub const HOLD_RATE: f32 = 0.0;
pub const SWAY_RATE: f32 = 0.5;
pub const SWAY_FRAMES: f32 = 8.0;
pub const MAIN_SLOT: i32 = 0;
pub const ON_FINISH: i32 = 2;
pub const FIRST_CHILD: i32 = 1;
pub const ENTRIES: [&str; 2] = ["Enter", "Update"];
pub const UPDATE: &str = "Update";
pub const STR: usize = 3;
pub const REC: usize = 12;
pub const HOLD_NODES: usize = 7;
pub const SWAY_NODES: usize = 6;
pub const PICK_NODES: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    Sway,
    Freeze,
}

impl Hold {
    pub fn nodes(self) -> usize {
        match self {
            Hold::Freeze => HOLD_NODES,
            Hold::Sway => HOLD_NODES + SWAY_NODES,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub els: usize,
    pub state: usize,
    pub body: usize,
    pub holds: &'static [f32],
    pub gap: Option<(usize, usize, f32)>,
}

impl Shape {
    pub fn added(&self, how: Hold) -> usize {
        let split: usize = if self.holds.len() > 1 { PICK_NODES } else { 0 };
        let front: usize = self.holds.len() * how.nodes() + split + PICK_NODES;
        front + if self.gap.is_some() { how.nodes() + PICK_NODES } else { 0 }
    }

    pub fn chance(&self, p: f32) -> f32 {
        if self.gap.is_some() { 1.0 - (1.0 - p).max(0.0).sqrt() } else { p }
    }

    pub fn per_attack(&self, p: f32) -> f32 {
        let q: f32 = self.chance(p);
        if self.gap.is_some() { 1.0 - (1.0 - q) * (1.0 - q) } else { q }
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

fn rate(seed: &str, v: f32) -> Spec<'_> {
    Spec::node(RATE, seed).prop(1, "IsUndoOnLeave", flag(true)).idle(0, "ASSlot", MAIN_SLOT.to_le_bytes().to_vec()).constant(2, "ASRate", v.to_le_bytes().to_vec())
}

fn timed<'a>(seed: &'a str, kids: &[usize]) -> Spec<'a> {
    let mut s: Spec = Spec::composite(PARALLEL, seed).prop(0, "EndPolicy", vec![0; 4]).prop(0, "ResultPolicy", FIRST_CHILD.to_le_bytes().to_vec());
    for k in kids {
        s = s.kid(*k, "");
    }
    s
}

fn sequence<'a>(seed: &'a str, kids: &[usize]) -> Spec<'a> {
    run_of(seed, kids, false)
}

fn run_of<'a>(seed: &'a str, kids: &[usize], looped: bool) -> Spec<'a> {
    let mut s: Spec = Spec::composite(SEQUENCE, seed)
        .prop(0, "BusyPolicy", vec![0; 4])
        .prop(0, "NumLoop", vec![0; 4])
        .prop(0, "PlayPolicy", vec![0; 4])
        .prop(0, "ResultPolicy", vec![0; 4])
        .prop(1, "IsDelayJudge", flag(false))
        .prop(1, "IsLoop", flag(looped))
        .prop(1, "IsUpdateNextInFrame", flag(looped));
    for k in kids {
        s = s.kid(*k, "");
    }
    s
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

fn step(a: &mut Ainb, seed: &str, v: f32) -> Res<usize> {
    let t: usize = a.add_node(&counter(&format!("{seed} timer"), SWAY_FRAMES, 0.0))?;
    let r: usize = a.add_node(&rate(&format!("{seed} rate"), v))?;
    a.add_node(&timed(seed, &[t, r]))
}

fn pose(a: &mut Ainb, seed: &str, how: Hold) -> Res<usize> {
    match how {
        Hold::Freeze => a.add_node(&rate(&format!("{seed} hold pose"), HOLD_RATE)),
        Hold::Sway => {
            let back: usize = step(a, &format!("{seed} sway back"), -SWAY_RATE)?;
            let on: usize = step(a, &format!("{seed} sway on"), SWAY_RATE)?;
            a.add_node(&run_of(&format!("{seed} sway"), &[back, on], true))
        }
    }
}

fn held(a: &mut Ainb, seed: &str, run: usize, at: f32, how: Hold) -> Res<usize> {
    let wind: usize = a.add_node(&counter(&format!("{seed} wind up"), at, 0.0))?;
    let timer: usize = a.add_node(&counter(&format!("{seed} hold timer"), PAUSE_FRAMES, PAUSE_EXTRA))?;
    let still: usize = pose(a, seed, how)?;
    let hold: usize = a.add_node(&timed(&format!("{seed} hold"), &[timer, still]))?;
    let after: usize = a.add_node(&Spec::node(IDLE, &format!("{seed} then play on")))?;
    let side: usize = a.add_node(&sequence(&format!("{seed} timing"), &[wind, hold, after]))?;
    a.add_node(&timed(&format!("{seed} attack with hold"), &[run, side]))
}

fn chance(a: &mut Ainb, seed: &str, p: f32, yes: usize, no: usize) -> Res<usize> {
    let roll: usize = a.add_node(&draw(&format!("{seed} roll"), p))?;
    a.add_node(&gate(&format!("{seed} gate"), roll, yes, no))
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
    if let Some((seq, pos, _)) = sh.gap {
        if a.el_type(seq) != SEQUENCE || a.params[seq].group(2).len() <= pos {
            return Err(Error::Msg(format!("{label}: the combo sequence is no longer there")));
        }
    }
    Ok(at)
}

pub fn build(a: &mut Ainb, sh: &Shape, p: f32, how: Hold, label: &str) -> Res<usize> {
    let at: Vec<usize> = entry(a, sh, label)?;
    let tag: String = format!("ootrr enemy feints {label}");
    let q: f32 = sh.chance(p);
    let mut hs: Vec<usize> = Vec::new();
    for (i, h) in sh.holds.iter().enumerate() {
        hs.push(held(a, &format!("{tag} {i}"), sh.body, *h, how)?);
    }
    let branch: usize = match hs.as_slice() {
        [one] => *one,
        [first, second] => chance(a, &format!("{tag} which"), 0.5, *first, *second)?,
        _ => return Err(Error::Msg(format!("{label}: one or two hold points are supported"))),
    };
    let top: usize = chance(a, &tag, q, branch, sh.body)?;
    for j in at {
        a.params[sh.state].plugs[j].a = top as u32;
    }
    if let Some((seq, pos, h)) = sh.gap {
        let seed: String = format!("{tag} gap");
        let j: usize = a.params[seq].pl[2][1] as usize + pos;
        let second: usize = a.params[seq].plugs[j].a as usize;
        let wrap: usize = held(a, &seed, second, h, how)?;
        let gated: usize = chance(a, &seed, q, wrap, second)?;
        a.params[seq].plugs[j].a = gated as u32;
    }
    Ok(top)
}

pub fn patch(raw: &[u8], sh: &Shape, p: f32, how: Hold, label: &str) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    if taken(&a) {
        return Err(Error::Msg(format!("{label}: the attack module is already patched")));
    }
    build(&mut a, sh, p, how, label)?;
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

    fn tail_idle(a: &Ainb) -> Option<usize> {
        (0..a.els.len()).filter(|i| a.el_type(*i) == SEQUENCE && el_text(a, *i).contains("IsLoop = false")).find_map(|i| {
            let ks: Vec<usize> = kids(a, i);
            let last: usize = *ks.last()?;
            (ks.len() == 2 && a.el_type(ks[0]) == PARALLEL && a.el_name(last) == IDLE).then_some(last)
        })
    }

    fn reach(a: &Ainb, from: usize, pick: &dyn Fn(usize) -> Option<&'static str>) -> std::collections::BTreeSet<usize> {
        let mut seen: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let mut todo: Vec<usize> = vec![from];
        while let Some(i) = todo.pop() {
            if !seen.insert(i) {
                continue;
            }
            match pick(i) {
                Some(b) => todo.push(branch(a, i, b)),
                None => todo.extend(kids(a, i)),
            }
        }
        seen
    }

    fn branch(a: &Ainb, sw: usize, name: &str) -> usize {
        a.params[sw].group(2).iter().find(|g| a.s(g.b) == name).unwrap().a as usize
    }

    fn el_text(a: &Ainb, i: usize) -> String {
        let s: String = show(a);
        let from: usize = s.find(&format!("element {i} type")).unwrap();
        let b: &str = &s[from..];
        b[..b[1..].find("\nelement ").map(|e| e + 1).unwrap_or(b.len())].to_string()
    }

    fn rate_of(a: &Ainb, i: usize, v: f32) {
        assert_eq!(a.el_name(i), RATE);
        assert!(el_text(a, i).contains(&format!("prop bool IsUndoOnLeave = true\n  in s32 ASSlot = 0 flags 0x1800000\n  in f32 ASRate = {v} flags 0x800000")), "{}", el_text(a, i));
    }

    fn hold_of(a: &Ainb, wrap: usize, run: usize, at: f32, how: Hold) {
        assert_eq!(a.el_type(wrap), PARALLEL);
        assert!(el_text(a, wrap).contains("prop s32 EndPolicy = 0\n  prop s32 ResultPolicy = 1"), "the attack decides when the wrapper ends");
        let ks: Vec<usize> = kids(a, wrap);
        assert_eq!(ks.len(), 2);
        assert_eq!(ks[0], run, "the vanilla attack runs first and untouched");
        let side: Vec<usize> = kids(a, ks[1]);
        assert_eq!(a.el_type(ks[1]), SEQUENCE);
        assert!(el_text(a, ks[1]).contains("prop bool IsLoop = false"));
        assert_eq!(side.len(), 3);
        assert_eq!(a.el_name(side[0]), COUNTER);
        assert_eq!(a.el_type(side[1]), PARALLEL);
        assert!(el_text(a, side[1]).contains("prop s32 ResultPolicy = 1"), "the hold timer decides when the hold ends");
        assert_eq!(a.el_name(side[2]), IDLE, "after the hold the timing branch idles without a busy result, so the attack's own end decides");
        let hold: Vec<usize> = kids(a, side[1]);
        assert_eq!(a.el_name(hold[0]), COUNTER);
        assert!(el_text(a, side[0]).contains(&format!("prop f32 BaseFrameTime = {at}\n  prop f32 RandomTimeMax = 0\n")), "hold point {at}");
        assert!(el_text(a, hold[0]).contains(&format!("prop f32 BaseFrameTime = {PAUSE_FRAMES}\n  prop f32 RandomTimeMax = {PAUSE_EXTRA}\n")));
        match how {
            Hold::Freeze => rate_of(a, hold[1], HOLD_RATE),
            Hold::Sway => {
                let lp: usize = hold[1];
                assert_eq!(a.el_type(lp), SEQUENCE);
                assert!(el_text(a, lp).contains("prop bool IsLoop = true\n  prop bool IsUpdateNextInFrame = true"), "the sway loops until the hold timer ends");
                let steps: Vec<usize> = kids(a, lp);
                assert_eq!(steps.len(), 2);
                for (st, v) in steps.iter().zip([-SWAY_RATE, SWAY_RATE]) {
                    assert_eq!(a.el_type(*st), PARALLEL);
                    let x: Vec<usize> = kids(a, *st);
                    assert_eq!(a.el_name(x[0]), COUNTER);
                    assert!(el_text(a, x[0]).contains(&format!("prop f32 BaseFrameTime = {SWAY_FRAMES}\n  prop f32 RandomTimeMax = 0\n")));
                    rate_of(a, x[1], v);
                }
            }
        }
    }

    fn check(t: &Target, raw: &[u8], how: Hold) {
        let sh: &Shape = &t.shape;
        let old: Ainb = Ainb::parse(raw).unwrap();
        assert_eq!(old.write().unwrap(), raw, "{}: the untouched module rewrites byte for byte", t.kind);
        assert!(!taken(&old), "{}: the vanilla module has no random draw", t.kind);
        assert!(old.find_els(RATE).is_empty(), "{}: the vanilla module sets no play rate", t.kind);
        let at: Vec<usize> = entry(&old, sh, t.kind).unwrap();
        let out: Vec<u8> = patch(raw, sh, 0.25, how, t.kind).unwrap();
        let a: Ainb = Ainb::parse(&out).unwrap();
        let n: usize = old.els.len();
        assert_eq!(a.els.len(), n + sh.added(how), "{}", t.kind);
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
        assert!(a.find_els(PLAY).iter().all(|i| *i < n), "{}: no new animation is played, the attack's own one is held", t.kind);
        let top: usize = a.params[sh.state].plugs[at[0]].a as usize;
        for j in &at {
            assert_eq!(a.params[sh.state].plugs[*j].a as usize, top, "{}: the state enters the draw first", t.kind);
        }
        let others: Vec<(u32, u32)> = a.params[sh.state].plugs.iter().enumerate().filter(|(j, _)| !at.contains(j)).map(|(_, g)| (g.a, g.b)).collect();
        let was: Vec<(u32, u32)> = old.params[sh.state].plugs.iter().enumerate().filter(|(j, _)| !at.contains(j)).map(|(_, g)| (g.a, g.b)).collect();
        assert_eq!(others, was, "{}: the state's other plugs are kept", t.kind);
        assert_eq!(a.el_type(top), SWITCH);
        assert_eq!(branch(&a, top, "False"), sh.body, "{}: no feint runs the vanilla attack", t.kind);
        let yes: usize = branch(&a, top, "True");
        match sh.holds {
            [h] => hold_of(&a, yes, sh.body, *h, how),
            [h0, h1] => {
                assert_eq!(a.el_type(yes), SWITCH);
                hold_of(&a, branch(&a, yes, "True"), sh.body, *h0, how);
                hold_of(&a, branch(&a, yes, "False"), sh.body, *h1, how);
            }
            _ => panic!("{}", t.kind),
        }
        let shown: String = show(&a);
        let q: f32 = sh.chance(0.25);
        let draws: Vec<usize> = a.find_els(DRAW);
        assert_eq!(draws.len(), 1 + usize::from(sh.holds.len() > 1) + usize::from(sh.gap.is_some()));
        if sh.holds.len() > 1 {
            assert!(shown.contains("in f32 TrueProbability = 0.25 flags 0x800000"));
            assert!(shown.contains("in f32 TrueProbability = 0.5 flags 0x800000"));
        } else {
            assert!(shown.contains(&format!("in f32 TrueProbability = {q} flags 0x800000")), "{}", t.kind);
        }
        for d in &draws {
            assert!(shown.contains(&format!("element {d} type 0 {DRAW}\n  prop bool OutputOnlyOnEnter = true")));
        }
        let per: usize = if how == Hold::Sway { 2 } else { 1 };
        assert_eq!(a.find_els(RATE).len(), per * (sh.holds.len() + usize::from(sh.gap.is_some())));
        if let Some((seq, pos, h)) = sh.gap {
            let ks: Vec<usize> = kids(&a, seq);
            let was: Vec<usize> = kids(&old, seq);
            assert_eq!(ks.len(), was.len());
            for (j, (x, y)) in ks.iter().zip(was.iter()).enumerate() {
                if j != pos {
                    assert_eq!(x, y);
                }
            }
            assert_eq!(a.el_type(ks[pos]), SWITCH, "{}: the hold before the second swing is gated", t.kind);
            assert_eq!(branch(&a, ks[pos], "False"), was[pos]);
            hold_of(&a, branch(&a, ks[pos], "True"), was[pos], h, how);
            assert!(shown.contains(&format!("in f32 TrueProbability = {q} flags 0x800000")));
        }
        assert_eq!(shown.matches("Blackboard").count(), show(&old).matches("Blackboard").count(), "{}: the feint reads and writes no key", t.kind);
        assert!(patch(&out, sh, 0.25, how, t.kind).is_err(), "{}: a patched module is not patched twice", t.kind);
        let all: Ainb = Ainb::parse(&patch(raw, sh, 1.0, how, t.kind).unwrap()).unwrap();
        let sure: usize = if sh.gap.is_some() { 2 } else { 1 };
        assert_eq!(show(&all).matches("in f32 FalseProbability = 0 flags 0x800000").count(), sure, "{}", t.kind);
    }

    #[test]
    fn every_attack_module_holds_its_own_attack_pose() {
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
            check(t, &mine[0].2, Hold::Sway);
            check(t, &mine[0].2, Hold::Freeze);
        }
        let n: Vec<(&str, usize)> = TARGETS.iter().map(|t| (t.kind, found.iter().filter(|c| c.1 == t.graph).count())).collect();
        assert_eq!(n, vec![("Stalfos swing", 1), ("Stalfos combo", 4), ("Stalfos jump", 1), ("Lizalfos swing", 1), ("Dinolfos swing", 1), ("Iron Knuckle overhead", 1), ("Iron Knuckle double", 4), ("Deku Baba bite", 2)]);
    }

    #[test]
    fn the_held_attacks_are_the_ones_the_tester_measured() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let found: Vec<(String, String, Vec<u8>)> = copies(&mut rf);
        let first = |g: &str| -> (String, u32) {
            let a: Ainb = Ainb::parse(&found.iter().find(|c| c.1 == g).unwrap().2).unwrap();
            command(&a, a.find_els(PLAY)[0]).unwrap()
        };
        for g in [TARGETS[0].graph, TARGETS[1].graph] {
            assert_eq!(first(g).1, 0x8000_0000, "{g}: the swing plays the command its caller binds, so the hold always holds the right swing");
        }
        let ik: Ainb = Ainb::parse(&found.iter().find(|c| c.1 == TARGETS[6].graph).unwrap().2).unwrap();
        let (seq, pos, _): (usize, usize, f32) = TARGETS[6].shape.gap.unwrap();
        let second: usize = kids(&ik, seq)[pos];
        assert_eq!(ik.el_type(second), SWITCH, "the Iron Knuckle's second part picks the second swing or the miss");
        let s: String = show(&ik);
        assert!(s.contains("ASCommand = \"WeaponDoubleAttackFirst\""));
        assert!(s.contains("ASCommand = \"WeaponDoubleAttackSecond\"") && s.contains("ASCommand = \"WeaponDoubleAttackMiss\""));
        for h in [crate::difficulty::feint::SINGLE_HOLD[0], crate::difficulty::feint::TRIPLE_HOLDS[0]] {
            assert!(h < 36.0 - 7.0, "the first hold ends its wind-up before the first measured hit, even with the counter's lag");
        }
        for (h, hit) in [(crate::difficulty::feint::LIZALFOS_HOLD[0], 41.0), (crate::difficulty::feint::DINOLFOS_HOLD[0], 36.0)] {
            assert!(h < hit - 7.0, "the lizard holds end their wind-up before their measured hits");
        }
        let mid: f32 = crate::difficulty::feint::TRIPLE_HOLDS[1];
        assert!(mid > 47.0 && mid < 79.0 - 7.0, "the mid-combo hold sits after the first hit and before the second");
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
        let para_raw: Vec<u8> = ["Pack/Actor/Enemy_Stalfos.pack.zs", hook::GLOBAL].iter().find_map(|p| sarc::parse(&rf.read_zs(p).ok()?).ok().and_then(|x| sarc::get(&x, "AI/Stalfos.action.InterruptParalyze.module.ainb").map(|e| e.data.clone()))).unwrap();
        let para: Ainb = Ainb::parse(&para_raw).unwrap();
        let stalfos: Ainb = Ainb::parse(&found.iter().find(|c| c.1 == TARGETS[0].graph).unwrap().2).unwrap();
        let ms: String = show(&m);
        let seq: usize = (0..m.els.len()).find(|i| m.el_type(*i) == SEQUENCE && ms.contains(&format!("element {i} type 2 \n  prop s32 BusyPolicy = 0\n  prop s32 NumLoop = 0\n  prop s32 PlayPolicy = 0\n  prop s32 ResultPolicy = 0\n  prop bool IsDelayJudge = false\n  prop bool IsLoop = false"))).unwrap();
        let tm: usize = (0..m.els.len()).find(|i| m.el_type(*i) == PARALLEL && ms.contains(&format!("element {i} type 3 \n  prop s32 EndPolicy = 0\n  prop s32 ResultPolicy = 1"))).unwrap();
        let sw: usize = (0..stalfos.els.len()).find(|i| stalfos.el_type(*i) == SWITCH).unwrap();
        let red: Ainb = Ainb::parse(&sarc::get(&es, "AI/Redead.MinimumPerceptionSetTarget.module.ainb").unwrap().data).unwrap();
        let lp: usize = (0..red.els.len()).find(|i| red.el_type(*i) == SEQUENCE && el_text(&red, *i).contains("prop bool IsLoop = true\n  prop bool IsUpdateNextInFrame = true")).unwrap();
        let liz: Ainb = Ainb::parse(&found.iter().find(|c| c.1 == TARGETS[3].graph).unwrap().2).unwrap();
        let liz_idle: usize = tail_idle(&liz).expect("the Lizalfos swing idles at the end of a timed step beside its attack");
        let freeze: Vec<usize> = para.find_els(RATE);
        assert_eq!(freeze.len(), 1, "the paralysed Stalfos freezes its pose with one play-rate node");
        assert!(show(&para).contains(&format!("element {} type 0 {RATE}\n  prop bool IsUndoOnLeave = true\n  in s32 ASSlot = 0 flags 0x1800000\n  in f32 ASRate = 0 flags 0x800000", freeze[0])));
        let pairs: Vec<usize> = (0..para.els.len()).filter(|i| para.el_type(*i) == PARALLEL && kids(&para, *i).contains(&freeze[0])).collect();
        assert_eq!(pairs.len(), 1);
        assert!(kids(&para, pairs[0]).iter().any(|k| para.el_name(*k) == BUSY), "the game holds the frozen pose beside a busy result");
        for t in &TARGETS {
            let raw: &[u8] = &found.iter().find(|c| c.1 == t.graph).unwrap().2;
            let old: Ainb = Ainb::parse(raw).unwrap();
            for how in [Hold::Sway, Hold::Freeze] {
                let a: Ainb = Ainb::parse(&patch(raw, &t.shape, 0.25, how, t.kind).unwrap()).unwrap();
                for i in old.els.len()..a.els.len() {
                let want: Vec<String> = match (a.el_type(i), a.el_name(i)) {
                    (0, COUNTER) => shape(&m, m.find_els(COUNTER)[0]),
                    (0, DRAW) => shape(&dark, dark.find_els(DRAW)[0]),
                    (0, RATE) => shape(&para, freeze[0]),
                    (0, IDLE) => shape(&liz, liz_idle),
                    (SEQUENCE, _) if el_text(&a, i).contains("IsLoop = true") => shape(&red, lp),
                    (SEQUENCE, _) => shape(&m, seq),
                    (PARALLEL, _) => shape(&m, tm),
                    (SWITCH, _) => shape(&stalfos, sw),
                    other => panic!("{}: unexpected new node {other:?}", t.kind),
                };
                assert_eq!(shape(&a, i), want, "{}: element {i}", t.kind);
                }
            }
        }
    }

    #[test]
    fn every_edited_attack_keeps_its_own_end_path() {
        let Some(dir) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: Romfs = Romfs::open(&dir).unwrap();
        let found: Vec<(String, String, Vec<u8>)> = copies(&mut rf);
        for t in &TARGETS {
            let raw: &[u8] = &found.iter().find(|c| c.1 == t.graph).unwrap().2;
            let old: Ainb = Ainb::parse(raw).unwrap();
            let n: usize = old.els.len();
            let at: Vec<usize> = entry(&old, &t.shape, t.kind).unwrap();
            let vanilla: std::collections::BTreeSet<usize> = reach(&old, t.shape.body, &|_| None);
            assert!(vanilla.iter().any(|i| old.el_name(*i) == PLAY), "{}: the vanilla attack plays its animation", t.kind);
            for how in [Hold::Sway, Hold::Freeze] {
                let a: Ainb = Ainb::parse(&patch(raw, &t.shape, 0.75, how, t.kind).unwrap()).unwrap();
                assert!(a.find_els(BUSY).iter().all(|i| *i < n), "{}: no new node holds a busy result", t.kind);
                let gates: Vec<usize> = (n..a.els.len()).filter(|i| a.el_type(*i) == SWITCH).collect();
                let top: usize = a.params[t.shape.state].plugs[at[0]].a as usize;
                for bits in 0..(1u32 << gates.len()) {
                    let pick = |i: usize| -> Option<&'static str> { gates.iter().position(|g| *g == i).map(|j| if bits & (1 << j) != 0 { "True" } else { "False" }) };
                    let got: std::collections::BTreeSet<usize> = reach(&a, top, &pick);
                    for v in &vanilla {
                        assert!(got.contains(v), "{} ({how:?}, draws {bits:b}): vanilla element {v} ({}) is still run", t.kind, old.el_name(*v));
                    }
                    for i in got.iter().filter(|i| **i >= n) {
                        let ks: Vec<usize> = kids(&a, *i);
                        match a.el_type(*i) {
                            PARALLEL => assert!(ks[0] < n || a.el_name(ks[0]) == COUNTER, "{}: new parallel {i} ends with the attack or a timer", t.kind),
                            SEQUENCE if el_text(&a, *i).contains("IsLoop = false") => assert_eq!(a.el_name(*ks.last().unwrap()), IDLE, "{}: the timing branch ends idle", t.kind),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn each_attack_is_held_at_the_chosen_level() {
        for t in &TARGETS {
            for p in [0.25f32, 0.5, 0.75, 1.0] {
                let q: f32 = t.shape.chance(p);
                assert!((0.0..=1.0).contains(&q), "{}", t.kind);
                assert!((t.shape.per_attack(p) - p).abs() < 1e-5, "{} at {p}: {}", t.kind, t.shape.per_attack(p));
            }
        }
        assert_eq!(TARGETS[6].shape.chance(0.75), 0.5, "the Iron Knuckle double: two draws of a half give three in four");
    }

    #[test]
    fn a_changed_module_is_refused() {
        assert!(patch(b"nope", &TARGETS[0].shape, 0.25, Hold::Sway, "t").is_err());
        let mut a: Ainb = crate::ainb::tests::synth();
        assert!(entry(&a, &TARGETS[0].shape, "t").is_err());
        assert!(build(&mut a, &TARGETS[0].shape, 0.25, Hold::Sway, "t").is_err());
    }
}
