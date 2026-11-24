use std::path::{Path as P, PathBuf as PB};

use ootrr_core::error::{Error, Res};
use ootrr_core::modpack::Host;
use ootrr_core::settings::seed::SeedFile;
use ootrr_core::shops::check::{audit, Audit};
use ootrr_core::shelf::stage::{run, Done};
use ootrr_core::shelf::{audit, Job};
use ootrr_core::slice::data::Data;

fn arg<'a>(a: &'a [String], i: usize, what: &'static str) -> Res<&'a str> {
    a.get(i).map(|s| s.as_str()).ok_or(Error::Format(what))
}

pub fn audit_seeds(a: &[String]) -> Res<()> {
    if a.is_empty() {
        return Err(Error::Format("missing seed file"));
    }
    let d: Data = Data::load()?;
    let mut bad: usize = 0;
    for p in a {
        let s: SeedFile = SeedFile::read(P::new(p))?;
        let r: Audit = audit(&d, &s)?;
        let deals: usize = s.shops.values().filter(|e| e.deal).count();
        println!("{p}: beatable {} | shop slots {} | deals {} | shop slots in the playthrough {} | top price there {} | wallet problems {}", r.won, s.shops.len(), deals, r.bought, r.top, r.bad.len());
        for b in &r.bad {
            println!("  {b}");
        }
        if !r.won || !r.bad.is_empty() {
            bad += 1;
        }
    }
    if bad > 0 {
        return Err(Error::Msg(format!("{bad} seed files failed the shop audit")));
    }
    Ok(())
}

pub fn testmod(a: &[String]) -> Res<()> {
    let mut rest: Vec<String> = Vec::new();
    let mut host: Host = Host::Hookshot;
    let mut i: usize = 0;
    while i < a.len() {
        if a[i] == "--host" {
            let v: &str = a.get(i + 1).map(|s| s.as_str()).ok_or(Error::Format("missing host after --host"))?;
            host = Host::parse(v).ok_or(Error::Format("--host is eden, hookshot or nx2navi"))?;
            i += 2;
            continue;
        }
        rest.push(a[i].clone());
        i += 1;
    }
    let romfs: &str = arg(&rest, 0, "missing romfs_root")?;
    let load: &str = arg(&rest, 1, "missing load_dir")?;
    let name: &str = arg(&rest, 2, "missing mod_name")?;
    if rest.len() < 4 {
        return Err(Error::Format("give at least one Location=Item@price, for example \"KF Shop Right Side Bottom Left=Boomerang@20\""));
    }
    let d: Data = Data::load()?;
    let jobs: Vec<Job> = rest[3..].iter().map(|x| ootrr_core::shops::jobs::parse(&d, x)).collect::<Res<Vec<Job>>>()?;
    let done: Done = run(P::new(romfs), None, P::new(load), name, host, &jobs)?;
    let back: PB = done.root.join("romfs");
    for j in &jobs {
        println!("{} ({} slot {}): {} for {} rupees, keepers {}", j.label, j.shop, j.slot, j.shelf_actor(), j.price(), j.keepers.join(", "));
    }
    print!("{}", done.report);
    for l in audit::run(P::new(romfs), &back, &jobs)? {
        println!("{l}");
    }
    println!("shelf test mod at {}, {} files", done.root.display(), done.files);
    Ok(())
}
