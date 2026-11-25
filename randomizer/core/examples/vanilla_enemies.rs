use std::collections::BTreeMap;
use std::path::PathBuf;

use ootrr_core::byml::{Byml, Node};
use ootrr_core::error::Res;
use ootrr_core::json::Json;
use ootrr_core::romfs::Romfs;

fn num(n: &Node) -> f64 {
    match n {
        Node::F32(v) => *v as f64,
        Node::F64(v) => *v,
        Node::I32(v) => *v as f64,
        _ => 0.0,
    }
}

fn pos(n: &Node) -> Json {
    let a: &[Node] = n.get("Translate").and_then(|t| t.as_arr()).unwrap_or(&[]);
    let v: Vec<Json> = (0..3).map(|i| Json::from((a.get(i).map(num).unwrap_or(0.0)).round() as i64)).collect();
    Json::Arr(v)
}

fn room(n: &Node) -> Json {
    match n.get("Dynamic").and_then(|d| d.get("DefaultDungeonRoomIndex")) {
        Some(Node::I32(v)) => Json::from(*v as i64),
        _ => Json::Null,
    }
}

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: vanilla_enemies <game romfs folder> <output json>");
        std::process::exit(2);
    }
    let mut rf: Romfs = Romfs::open(&PathBuf::from(&args[0]))?;
    let files: Vec<String> = rf.list("Banc")?.into_iter().filter(|f| f.ends_with(".bcett.byml.zs")).collect();
    let mut merged: BTreeMap<String, (Vec<Json>, Vec<Json>)> = BTreeMap::new();
    for f in files {
        let stem: String = f.rsplit('/').next().unwrap_or("").trim_end_matches(".bcett.byml.zs").trim_end_matches("_Static").trim_end_matches("_Dynamic").to_string();
        let y: Byml = match rf.read_byml(&f) {
            Ok(y) => y,
            Err(e) => {
                eprintln!("{f}: {e:?}");
                continue;
            }
        };
        let acts: &[Node] = y.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]);
        let slot: &mut (Vec<Json>, Vec<Json>) = merged.entry(stem).or_default();
        let enemies: &mut Vec<Json> = &mut slot.0;
        let refs: &mut Vec<Json> = &mut slot.1;
        for a in acts {
            let name: &str = a.get("Gyaml").and_then(|g| g.as_str()).unwrap_or("");
            let r: Json = room(a);
            if r != Json::Null {
                let p: Json = pos(a);
                refs.push(Json::Arr(vec![p, r.clone()]));
            }
            if name.starts_with("Enemy_") {
                enemies.push(Json::obj().with("n", name.into()).with("p", pos(a)).with("r", r));
            }
        }
    }
    let mut maps: Json = Json::obj();
    for (stem, (enemies, refs)) in merged {
        maps = maps.with(&stem, Json::obj().with("enemies", Json::Arr(enemies)).with("refs", Json::Arr(refs)));
    }
    std::fs::write(&args[1], Json::obj().with("maps", maps).to_string())?;
    Ok(())
}
