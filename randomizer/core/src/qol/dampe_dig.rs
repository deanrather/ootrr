use crate::bfevfl::{Bf, Event, Flow, Kind, Val};
use crate::error::{Error, Res};

pub const FLOWS: [&str; 2] = ["Npc_Hylia_Dampe_001", "Npc_Hylia_Dampe_001.110"];
const NAME: &str = "Npc_Hylia_Dampe_001";
pub const DROPPED: &str = "GraveYard_DiggingHoleTour_Drop_HeartPiece";
const LOTTERY: &str = "Event219";
const RUPEE_RATE: &str = "LotteryHitSoil_RupeeRate";
const RUPEES: [u32; 6] = [0, 1, 2, 3, 4, 5];
const ASK: &str = "Event119";
const FOUND: [&str; 2] = ["Event48", "Event259"];
const DIG: &str = "Event314";

pub const NOTE: &str = "Dampé's gravedigging tour gives its check on the first dig, with no second fee: the dig lottery (Event219) has no rupee share, a rupee result goes to the Heart Piece dig while the check is not taken, and Dampé's find (Event48, Event259) goes straight to the Heart Piece dig (Event314), skipping the question and the second fee. Once the check is taken, every dig gives rupees as before. Always on.";

fn named(f: &Flow, name: &str) -> Res<usize> {
    let hits: Vec<usize> = (0..f.events.len()).filter(|&i| f.events[i].name == name).collect();
    match hits.as_slice() {
        [i] => Ok(*i),
        _ => Err(Error::Msg(format!("first dig: expected one {name} in {NAME}, found {}", hits.len()))),
    }
}

pub fn patch(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == NAME).ok_or(Error::Format("first dig: the Dampé flow has no Npc_Hylia_Dampe_001 flow"))?;
    let idx = |i: usize| crate::dungeon_credit::idx(i);
    let ask: usize = named(f, ASK)?;
    let asks: bool = f.events[ask].kind.params().and_then(|c| c.get("GameDataName")) == Some(&Val::Str(DROPPED.to_string()));
    if !asks || !matches!(&f.events[ask].kind, Kind::Switch { cases, .. } if cases.len() == 2) {
        return Err(Error::Format("first dig: Event119 no longer asks whether the Heart Piece dropped"));
    }
    let dig: u16 = idx(named(f, DIG)?)?;
    for n in FOUND {
        let i: usize = named(f, n)?;
        match &mut f.events[i].kind {
            Kind::Action { next, .. } => *next = dig,
            _ => return Err(Error::Msg(format!("first dig: {n} is not an action"))),
        }
    }
    let lottery: usize = named(f, LOTTERY)?;
    let mut found: bool = false;
    if let Some(c) = f.events[lottery].kind.params_mut() {
        for (k, v) in c.items.iter_mut() {
            if k == RUPEE_RATE {
                *v = Val::Int(0);
                found = true;
            }
        }
    }
    if !found {
        return Err(Error::Format("first dig: the dig lottery has no rupee rate"));
    }
    let targets: Vec<(u32, u16)> = match &f.events[lottery].kind {
        Kind::Switch { cases, .. } => cases.iter().filter(|c| RUPEES.contains(&c.val)).map(|c| (c.val, c.ev)).collect(),
        _ => return Err(Error::Format("first dig: the dig lottery is not a switch")),
    };
    if targets.len() != RUPEES.len() {
        return Err(Error::Format("first dig: the dig lottery lost a rupee result"));
    }
    for (val, to) in targets {
        let at: u16 = idx(f.events.len())?;
        let mut gate: Event = f.events[ask].clone();
        gate.name = format!("OOTRRFirstDig{val}");
        if let Kind::Switch { cases, .. } = &mut gate.kind {
            for c in cases.iter_mut() {
                c.ev = if c.val == 0 { to } else { dig };
            }
        }
        f.events.push(gate);
        if let Kind::Switch { cases, .. } = &mut f.events[lottery].kind {
            for c in cases.iter_mut().filter(|c| c.val == val) {
                c.ev = at;
            }
        }
    }
    Ok(())
}
