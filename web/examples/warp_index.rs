use std::path::PathBuf;

use ootrr_core::error::Res;
use ootrr_core::json::Json;
use ootrr_web::tracker::index;
use ootrr_web::tracker::targets;

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: warp_index <game romfs folder> <output index.json>");
        std::process::exit(2);
    }
    let root: PathBuf = PathBuf::from(&args[0]);
    let idx: index::Index = index::build(&root)?;
    let ts: &[targets::Target] = targets::all()?;
    let names: Vec<Json> = ts
        .iter()
        .map(|t| {
            Json::obj()
                .with("id", t.id.as_str().into())
                .with("name", t.name.as_str().into())
                .with("kind", t.kind.as_str().into())
                .with("scene", t.scene.as_deref().map(Json::from).unwrap_or(Json::Null))
                .with("area", t.area.as_deref().map(Json::from).unwrap_or(Json::Null))
        })
        .collect();
    let out: Json = idx.to_json().with("targets", Json::Arr(names));
    std::fs::write(&args[1], out.to_string())?;
    eprintln!("{} locations, {} maps", idx.locs.len(), idx.maps.len());
    Ok(())
}
