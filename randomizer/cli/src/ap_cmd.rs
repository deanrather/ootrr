use std::path::Path as P;

use ootrr_core::ap::{self, mailbox::Record, markers, saved};
use ootrr_core::error::{Error, Res};
use ootrr_core::romfs::Romfs;
use ootrr_core::staging::{commit, Committed, Out};
use ootrr_mem::game::Game;
use ootrr_mem::gdm;

pub const TEST_SEED: &str = "aptest";

pub fn test_mod(a: &[String]) -> Res<()> {
    let root: &str = a.first().map(|s| s.as_str()).ok_or(Error::Format("missing romfs_root"))?;
    let load: &str = a.get(1).map(|s| s.as_str()).ok_or(Error::Format("missing load_dir"))?;
    let name: &str = a.get(2).map(|s| s.as_str()).ok_or(Error::Format("missing mod_name"))?;
    let actors: &[String] = a.get(3..).unwrap_or(&[]);
    if actors.is_empty() {
        return Err(Error::Format("give at least one item actor, for example Item_Arrow_Fire"));
    }
    let recs: Vec<Record> = actors.iter().enumerate().map(|(i, x)| Record::new(&format!("{:012x}", i + 1), x)).collect::<Res<_>>()?;
    let mut rf: Romfs = Romfs::open(P::new(root))?;
    let mut outs: Vec<Out> = Vec::new();
    ap::stage(&mut rf, &mut outs, P::new(root), TEST_SEED, &recs)?;
    let c: Committed = commit(&mut rf, P::new(load), name, &outs, None)?;
    print!("{}", c.report);
    println!("seed flag {} (set when a new game reaches Kokiri Forest)", markers::seed_flag(TEST_SEED));
    println!("goal flag {} (set after Ganon, before the ending)", markers::goal_flag(TEST_SEED));
    for r in &recs {
        println!("{}: ootrr ap-flag {} on   gives it, then {} turns true", r.actor, r.recv(), r.got());
    }
    Ok(())
}

pub fn flag(a: &[String]) -> Res<()> {
    let name: &str = a.first().map(|s| s.as_str()).ok_or(Error::Format("missing flag name"))?;
    let g: Game = Game::attach()?;
    let h: u32 = saved::hash(name);
    match a.get(1).map(|s| s.as_str()) {
        None => println!("{name} = {}", gdm::get_bool(&g, h)?),
        Some(v @ ("on" | "off")) => {
            let was: bool = gdm::set_bool(&g, h, v == "on")?;
            println!("{name}: {was} -> {}", v == "on");
        }
        Some(x) => return Err(Error::Msg(format!("the value must be on or off, not {x}"))),
    }
    Ok(())
}
