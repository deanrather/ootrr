//! The Archipelago page: read an `.apootrr` patch, then build its mod with the folders from the
//! Configuration page. The folder checks are the seed build's own ([`api_build::place`]).

use std::path::{Path as P, PathBuf as PB};
use std::sync::{Arc, Mutex};

use ootrr_core::ap::build::{self as ap_build, Summary};
use ootrr_core::ap::patch::Patch;
use ootrr_core::ap::{export, world};
use ootrr_core::cosmetic::{self, Cosmetics, Music};
use ootrr_core::sfx::Sfx;
use ootrr_core::json::Json;
use ootrr_core::settings::Settings;
use ootrr_core::slice::data::Data;

use crate::ap_host;
use crate::api_build::{self, Place};
use crate::api_settings;
use crate::body;
use crate::error::{Fail, Hr};
use crate::game_source::{self, Prepared};
use crate::jobs::{Ctl, Jobs};
use crate::pathsafe;
use crate::resp::Resp;

fn refuse(m: &str) -> Fail {
    Fail::new(422, m)
}

fn patch_of(b: &Json) -> Hr<Patch> {
    ap_build::read(body::text(b, "patch_text")?).map_err(|e| refuse(&format!("Patch file: {e}")))
}

fn summary_json(s: &Summary) -> Json {
    Json::obj()
        .with("slot", s.slot.as_str().into())
        .with("seed_name", s.seed_name.as_str().into())
        .with("preset", s.preset.as_str().into())
        .with("settings_string", s.settings.as_str().into())
        .with("locations", Json::Int(s.locations as i64))
        .with("foreign", Json::Int(s.foreign as i64))
        .with("receive", Json::Int(s.receive as i64))
        .with("marker", s.marker.as_str().into())
        .with("mod_name", s.mod_name.as_str().into())
}

/// `POST /api/ap/read {patch_text}`: what the patch holds, without building.
pub fn read(b: &Json) -> Hr<Json> {
    let p: Patch = patch_of(b)?;
    Ok(summary_json(&ap_build::summary(&p)))
}

/// The note left in the mod folder as Spoiler.txt, so "Delete all other seeds" treats an
/// Archipelago mod like a seed.
fn note(s: &Summary) -> String {
    format!(
        "OOTRR Archipelago mod\nSlot: {}\nArchipelago seed: {}\nPreset: {}\nSettings string: {}\n\nThe spoiler of an Archipelago game is on the Archipelago server.\n",
        s.slot, s.seed_name, s.preset, s.settings
    )
}

/// `POST /api/ap/build {patch_text, game_dir, update_dir, include_update, mod_dir, mod_name, host, delete_others, title_logo, music, sfx, colors, color_parts, cosmetic_seed}`.
pub fn build(jobs: &Arc<Jobs>, b: &Json) -> Hr<Json> {
    let p: Patch = patch_of(b)?;
    let pl: Place = api_build::place(b)?;
    if jobs.running() > 0 || ap_build::running() {
        return Err(Fail::new(409, "Another build is running. Wait for it to finish, then build again."));
    }
    let cos: Cosmetics = api_build::cosmetics(b)?;
    let title: bool = cos.title;
    let target: String = pathsafe::text(&pl.target);
    let n: usize = p.placements.len();
    let id: u64 = jobs.start(move |c: &Ctl| {
        c.begin(&format!("Patch read, {n} shuffled locations for slot {}", p.slot));
        c.ok(5);
        if let Some(s) = pl.source.step() {
            c.begin(s);
        }
        let game: Prepared = match game_source::prepare(&pl.source, cos.music != Music::Off || cos.sfx != Sfx::Off) {
            Ok(g) => g,
            Err(e) => return fail(c, &e),
        };
        c.ok(8);
        let romfs: PB = game.romfs.clone();
        if title {
            c.begin("Check that no other mod changes the title screen");
            if let Err(e) = ootrr_core::title_logo::preflight(&pl.target, &romfs) {
                return fail(c, &e.to_string());
            }
            c.ok(10);
        }
        c.begin("Check the world with OOTRR's logic and build the mod");
        let (mut built, s) = match ap_build::build(&p, &romfs, &pl.load, &pl.name, pl.host) {
            Ok(x) => x,
            Err(e) => return fail(c, &e.to_string()),
        };
        c.ok(80);
        if title {
            c.begin("Draw the Randomizer line under the title logo");
            match ootrr_core::title_logo::apply(&built.root, &romfs) {
                Ok(notes) => {
                    for n in notes {
                        built.report.push_str(&format!("{n}\n"));
                    }
                }
                Err(e) => return fail(c, &e.to_string()),
            }
        }
        if cos.any() {
            c.begin("Write the cosmetic changes");
            match cosmetic::apply(&built.root, &romfs, &cos) {
                Ok(notes) => {
                    for n in notes {
                        built.report.push_str(&format!("{n}\n"));
                    }
                }
                Err(e) => return fail(c, &e.to_string()),
            }
        }
        c.ok(90);
        c.begin("Write the slot note");
        if let Err(e) = api_build::put_spoiler(&built.root, Some(&note(&s))) {
            return fail(c, &format!("The mod was written, but {} could not be: {e}", api_build::SPOILER_FILE));
        }
        if pl.with_update {
            c.begin("Add the v1.1.1 update files to the mod");
            match game_source::add_update(&pl.source, &built.root) {
                Ok(n) => built.report.push_str(&format!("Added {n} v1.1.1 update files to the mod. Do not share this mod: it now holds game files.\n")),
                Err(e) => return fail(c, &format!("The mod was written, but the v1.1.1 update files could not be added: {e}")),
            }
        }
        if pl.delete_others {
            c.begin("Delete the other seeds in the mod folder");
            match api_build::delete_others(&built.root) {
                Ok(gone) if gone.is_empty() => built.report.push_str("No other seeds to delete.\n"),
                Ok(gone) => built.report.push_str(&format!("Deleted {} other seeds: {}.\n", gone.len(), gone.join(", "))),
                Err(e) => return fail(c, &format!("The mod was written, but the other seeds could not all be deleted: {e}")),
            }
        }
        c.ok(95);
        c.begin("Mod files written");
        c.ok(100);
        c.finish("done", &built.report);
    })?;
    Ok(Json::obj()
        .with("job_id", Json::Int(id as i64))
        .with("target", target.into())
        .with("writes_files", true.into())
        .with("notice", "Building the Archipelago mod. Only the mod folder is written, the game folder is only read.".into()))
}

fn fail(c: &Ctl, m: &str) {
    c.mark("failed");
    c.finish("failed", m);
}

const APP: &str = env!("CARGO_PKG_VERSION");
const WORLDS: &str = "custom_worlds";
/// The launcher of an installed Archipelago, and of a source checkout (found, but not started by us).
const LAUNCHERS: [&str; 2] = ["ArchipelagoLauncher.exe", "ArchipelagoLauncher"];
const SOURCE_LAUNCHER: &str = "Launcher.py";
const CLIENT: &str = "OOTRR Client";
const MAX_WORLD: u64 = 64 << 20;
const MAX_HOST: u64 = 4 << 20;
/// Archipelago's limit for a slot name.
const MAX_NAME: usize = 16;

static WORLD: Mutex<Option<Arc<Vec<u8>>>> = Mutex::new(None);

/// The world file, packed once for the life of the app (its contents only depend on the exe).
fn world_bytes() -> Hr<Arc<Vec<u8>>> {
    let mut g = WORLD.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(w) = g.as_ref() {
        return Ok(Arc::clone(w));
    }
    let w: Arc<Vec<u8>> = Arc::new(world::pack(APP).map_err(|e| Fail::new(409, &e.to_string()))?);
    *g = Some(Arc::clone(&w));
    Ok(w)
}

fn launcher(folder: &P) -> Option<PB> {
    LAUNCHERS.iter().map(|n| folder.join(n)).find(|p| p.is_file())
}

fn found(folder: &P) -> bool {
    launcher(folder).is_some() || folder.join(SOURCE_LAUNCHER).is_file()
}

fn guess() -> Option<PB> {
    let mut tries: Vec<PB> = Vec::new();
    if cfg!(windows) {
        if let Some(d) = std::env::var_os("ProgramData") {
            tries.push(PB::from(d).join("Archipelago"));
        }
        tries.push(PB::from("C:\\ProgramData\\Archipelago"));
    } else if let Some(h) = std::env::var_os("HOME") {
        tries.push(PB::from(h).join("Archipelago"));
    }
    tries.into_iter().find(|p| found(p))
}

fn small(p: &P, max: u64) -> Option<Vec<u8>> {
    match std::fs::metadata(p) {
        Ok(m) if m.is_file() && m.len() <= max => std::fs::read(p).ok(),
        _ => None,
    }
}

fn status(folder: Option<&P>) -> Json {
    let exe: Option<PB> = std::env::current_exe().ok();
    let mut out: Json = Json::obj()
        .with("guess", guess().map(|p| pathsafe::text(&p)).unwrap_or_default().as_str().into())
        .with("world_version", world::version().as_str().into())
        .with("app_version", APP.into())
        .with("exe", exe.as_deref().map(pathsafe::text).unwrap_or_default().as_str().into());
    let Some(folder) = folder else {
        return out.with("found", false.into()).with("ready", false.into());
    };
    out.put("path", pathsafe::text(folder).as_str().into());
    let is: bool = folder.is_dir() && found(folder);
    out.put("found", is.into());
    out.put("can_launch", launcher(folder).is_some().into());
    if !is {
        return out.with("ready", false.into());
    }
    let file: PB = folder.join(WORLDS).join(world::FILE);
    let inst: Option<world::Installed> = small(&file, MAX_WORLD).map(|b| world::installed(&b));
    let current: bool = inst.as_ref().is_some_and(|i| i.version == world::version() && i.built_by == APP);
    out.put(
        "world",
        Json::obj()
            .with("installed", inst.is_some().into())
            .with("version", inst.as_ref().map(|i| i.version.as_str()).unwrap_or("").into())
            .with("built_by", inst.as_ref().map(|i| i.built_by.as_str()).unwrap_or("").into())
            .with("current", current.into())
            .with("file", pathsafe::text(&file).as_str().into()),
    );
    let host: Option<String> = small(&folder.join(ap_host::FILE), MAX_HOST).and_then(|b| String::from_utf8(b).ok()).and_then(|t| ap_host::exe_in(&t));
    let known: bool = matches!((&host, &exe), (Some(h), Some(e)) if ap_host::same_file(P::new(h), e));
    out.put("host_exe", host.as_deref().unwrap_or("").into());
    out.put("knows_app", known.into());
    out.with("ready", (current && known).into())
}

fn folder_of(text: &str) -> Hr<PB> {
    Ok(pathsafe::abs(text.trim()).map_err(|e| refuse(&format!("Archipelago folder: {e}")))?)
}

/// `GET /api/ap/setup?path=`: what is in place for OOTRR in that Archipelago folder.
pub fn setup(path: Option<&str>) -> Hr<Json> {
    match path.map(str::trim) {
        None | Some("") => Ok(status(None)),
        Some(t) => Ok(status(Some(&folder_of(t)?))),
    }
}

fn write_new(path: &P, bytes: &[u8]) -> std::io::Result<()> {
    let tmp: PB = path.with_extension("part");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// `POST /api/ap/setup {path}`: put the OOTRR world in `custom_worlds` and this app's path in
/// `host.yaml`. Only an Archipelago folder is written to.
pub fn install(b: &Json) -> Hr<Json> {
    let folder: PB = folder_of(body::short(b, "path")?)?;
    if !folder.is_dir() || !found(&folder) {
        return Err(refuse("Archipelago folder: Archipelago was not found there. Pick the folder that holds ArchipelagoLauncher."));
    }
    let exe: PB = std::env::current_exe().map_err(|e| Fail::new(500, &format!("This app could not find its own file: {e}")))?;
    let bytes: Arc<Vec<u8>> = world_bytes()?;
    let worlds: PB = folder.join(WORLDS);
    std::fs::create_dir_all(&worlds).map_err(|e| refuse(&format!("The {WORLDS} folder could not be created: {e}")))?;
    write_new(&worlds.join(world::FILE), &bytes).map_err(|e| refuse(&format!("The OOTRR world could not be written to {WORLDS}: {e}")))?;
    let host: PB = folder.join(ap_host::FILE);
    let old: String = match small(&host, MAX_HOST) {
        Some(raw) => String::from_utf8(raw).map_err(|_| refuse("host.yaml is not valid text, so it was left alone. The OOTRR world was installed."))?,
        None if host.exists() => return Err(refuse("host.yaml is too large to edit, so it was left alone. The OOTRR world was installed.")),
        None => String::new(),
    };
    let new: String = ap_host::with_exe(&old, &pathsafe::text(&exe));
    if new != old {
        write_new(&host, new.as_bytes()).map_err(|e| refuse(&format!("host.yaml could not be written: {e}. The OOTRR world was installed.")))?;
    }
    Ok(status(Some(&folder)))
}

/// `GET /api/ap/world`: the bytes of `ootrr.apworld`, for a host who does not use this app.
pub fn world_file() -> Hr<Resp> {
    Ok(Resp::new(200, "application/octet-stream", world_bytes()?.to_vec()))
}

fn slot(b: &Json, k: &str, must: bool) -> Hr<String> {
    let t: &str = body::opt_text(b, k)?.unwrap_or("").trim();
    if t.is_empty() && !must {
        return Ok(String::new());
    }
    if t.is_empty() || t.chars().count() > MAX_NAME || t.chars().any(char::is_control) {
        return Err(refuse(&format!("Player name: use 1 to {MAX_NAME} characters.")));
    }
    Ok(t.to_string())
}

/// The app's Plandomizer entries as the apworld's own `ootrr_plando` option, which works whatever
/// the host's plando setting is (Archipelago's `plando_items` is off unless the host turns it on).
fn plando_yaml(b: &Json) -> Hr<String> {
    let Some(v) = b.get("plando") else {
        return Ok(String::new());
    };
    let a: &[Json] = v.as_arr().ok_or_else(|| refuse("Plandomizer: send a list of locations and items."))?;
    if a.len() > 500 {
        return Err(refuse("Plandomizer: too many entries."));
    }
    let mut out: String = String::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for e in a {
        let (Some(l), Some(i)) = (e.get("location").and_then(|x| x.as_str()), e.get("item").and_then(|x| x.as_str())) else {
            return Err(refuse("Plandomizer: each entry needs a location and an item."));
        };
        let (l, i) = (l.trim(), i.trim());
        if l.is_empty() || i.is_empty() {
            continue;
        }
        if l.len() > 120 || i.len() > 120 || l.chars().chain(i.chars()).any(char::is_control) {
            return Err(refuse("Plandomizer: a location or item name is not allowed."));
        }
        if !seen.insert(l.to_string()) {
            return Err(refuse(&format!("Plandomizer: {l} is named twice.")));
        }
        out.push_str(&format!("    {}: {}\n", yaml_text(l), yaml_text(i)));
    }
    Ok(if out.is_empty() { out } else { format!("  ootrr_plando:\n{out}") })
}

fn yaml_text(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn file_stem(name: &str) -> String {
    let t: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if t.is_empty() { "player".to_string() } else { t }
}

/// `POST /api/ap/player {name, settings}`: the player file for these settings. A built-in preset
/// is named; anything else goes in as a settings string, checked first.
pub fn player(b: &Json) -> Hr<Json> {
    let name: String = slot(b, "name", true)?;
    let s: Settings = api_settings::from_body(b)?.settings;
    let text: String = s.encode()?;
    let mut preset: Option<world::Preset> = None;
    for p in world::presets_in_world()? {
        if p.settings.encode()? == text {
            preset = Some(p);
            break;
        }
    }
    let choice: String = match &preset {
        Some(p) => format!("  preset: {}\n", p.id),
        None => {
            let d: Data = Data::load()?;
            export::export_with(&d, &s, "custom", export::SEED, 0).map_err(|e| refuse(&format!("Archipelago cannot use these settings: {e}")))?;
            format!("  settings_string: {}\n", yaml_text(&text))
        }
    };
    let file: String = format!("name: {}\ngame: OOTRR\ndescription: Made by the OOTRR app {APP}\nOOTRR:\n{choice}{}", yaml_text(&name), plando_yaml(b)?);
    Ok(Json::obj()
        .with("file_name", format!("{}.yaml", file_stem(&name)).as_str().into())
        .with("text", file.as_str().into())
        .with("preset", preset.as_ref().map(|p| p.id.as_str()).unwrap_or("").into())
        .with("preset_name", preset.as_ref().map(|p| p.name.as_str()).unwrap_or("").into()))
}

fn address(b: &Json) -> Hr<String> {
    let t: &str = body::opt_text(b, "address")?.unwrap_or("").trim();
    let t: &str = t.strip_prefix("archipelago://").unwrap_or(t);
    if t.len() > 255 || t.starts_with('-') || !t.chars().all(|c| c.is_ascii_alphanumeric() || ".-:[]_".contains(c)) {
        return Err(refuse("Room address: use the address and port the host gave you, for example archipelago.gg:38281."));
    }
    Ok(t.to_string())
}

fn client_args(address: &str, name: &str, exe: &P) -> Vec<String> {
    let mut a: Vec<String> = vec![CLIENT.to_string(), "--".to_string(), "--ootrr".to_string(), pathsafe::text(exe)];
    if !address.is_empty() {
        a.extend(["--connect".to_string(), address.to_string()]);
    }
    if !name.is_empty() {
        a.extend(["--name".to_string(), name.to_string()]);
    }
    a
}

/// `POST /api/ap/client {path, address, name}`: start Archipelago's OOTRR Client, connecting to
/// the room when an address is given.
pub fn client(lan: bool, b: &Json) -> Hr<Json> {
    if lan {
        return Err(Fail::new(501, "The client only opens on the computer running OOTRR."));
    }
    let folder: PB = folder_of(body::short(b, "path")?)?;
    let addr: String = address(b)?;
    let name: String = slot(b, "name", false)?;
    if name.starts_with('-') {
        return Err(refuse("Player name: it may not start with a dash."));
    }
    let Some(run) = launcher(&folder) else {
        return Err(refuse("Archipelago folder: ArchipelagoLauncher was not found there. Start the OOTRR Client from Archipelago's own Launcher."));
    };
    let exe: PB = std::env::current_exe().map_err(|e| Fail::new(500, &format!("This app could not find its own file: {e}")))?;
    std::process::Command::new(&run)
        .args(client_args(&addr, &name, &exe))
        .current_dir(&folder)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| refuse(&format!("The Archipelago Launcher could not be started: {e}")))?;
    Ok(Json::obj().with("opened", true.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r#"{"format":"ootrr-ap-patch","version":1,"game":"OOTRR","preset":"default","settings":"x","seed":"archipelago","seed_name":"1","player":1,"slot":"Link","seed_key":"0123456789ab",
        "placements":{"KF Midos Top Left Chest":"AP: 0123456789ab","KF Midos Top Right Chest":"Slingshot"},
        "foreign":{"KF Midos Top Left Chest":{"item":"Key","player":"Quester","game":"APQuest","progression":true}},
        "remote":[{"key":"aaaaaaaaaaaa","item":"Boomerang","player":"Quester","location":"Top Left Chest"}],"start":[],"location_ids":{}}"#;

    #[test]
    fn reading_a_patch_names_its_slot_and_default_mod() {
        let j: Json = read(&Json::obj().with("patch_text", SMALL.into())).unwrap();
        assert_eq!(j.get("slot").and_then(|x| x.as_str()), Some("Link"));
        assert_eq!(j.get("mod_name").and_then(|x| x.as_str()), Some("ZZ OOTRR AP Link"));
        assert_eq!(j.get("receive").and_then(|x| x.as_i64()), Some(1));
        assert_eq!(j.get("foreign").and_then(|x| x.as_i64()), Some(1));
    }

    #[test]
    fn a_bad_patch_or_bad_folders_are_refused_before_a_job_starts() {
        assert_eq!(read(&Json::obj().with("patch_text", "{}".into())).unwrap_err().status, 422);
        assert!(read(&Json::obj()).is_err());
        let jobs: Arc<Jobs> = Jobs::new();
        let nowhere: String = std::env::temp_dir().join("ootrr_ap_no_such_game").display().to_string();
        let body: Json = Json::obj()
            .with("patch_text", SMALL.into())
            .with("game_dir", nowhere.as_str().into())
            .with("mod_dir", nowhere.as_str().into())
            .with("mod_name", "x".into());
        assert!(build(&jobs, &body).is_err());
        assert_eq!(jobs.running(), 0);
    }

    fn fake_archipelago(tag: &str) -> PB {
        let d: PB = std::env::temp_dir().join(format!("ootrr_ap_setup_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(LAUNCHERS[0]), b"").unwrap();
        d
    }

    fn flag(j: &Json, k: &str) -> bool {
        j.get(k).and_then(|x| x.as_bool()) == Some(true)
    }

    #[test]
    fn setup_installs_the_world_and_tells_archipelago_where_the_app_is() {
        let d: PB = fake_archipelago("install");
        let path: String = d.display().to_string();
        std::fs::write(d.join(ap_host::FILE), "server_options:\n  port: 38281\n").unwrap();
        let before: Json = setup(Some(&path)).unwrap();
        assert!(flag(&before, "found") && !flag(&before, "ready") && !flag(&before, "knows_app"));
        assert!(!flag(before.get("world").unwrap(), "installed"));
        let after: Json = install(&Json::obj().with("path", path.as_str().into())).unwrap();
        assert!(flag(&after, "ready") && flag(&after, "knows_app"));
        let w: &Json = after.get("world").unwrap();
        assert!(flag(w, "installed") && flag(w, "current"));
        assert_eq!(w.get("version").and_then(|x| x.as_str()), Some(world::version().as_str()));
        let host: String = std::fs::read_to_string(d.join(ap_host::FILE)).unwrap();
        assert!(host.starts_with("server_options:\n  port: 38281\n") && host.contains("ootrr_options:"));
        assert!(flag(&setup(Some(&path)).unwrap(), "ready"));
        std::fs::write(d.join(WORLDS).join(world::FILE), world::pack("0.0.1").unwrap()).unwrap();
        let old: Json = setup(Some(&path)).unwrap();
        assert!(!flag(&old, "ready") && flag(old.get("world").unwrap(), "installed") && !flag(old.get("world").unwrap(), "current"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn setup_only_writes_into_an_archipelago_folder() {
        let d: PB = std::env::temp_dir().join(format!("ootrr_ap_setup_empty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let path: String = d.display().to_string();
        assert!(!flag(&setup(Some(&path)).unwrap(), "found"));
        assert!(install(&Json::obj().with("path", path.as_str().into())).is_err());
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
        assert!(!flag(&setup(None).unwrap(), "ready"));
        assert!(setup(Some("relative")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_player_file_names_a_preset_or_carries_the_settings_string() {
        let plain: Json = player(&Json::obj().with("name", "Link".into()).with("settings", Json::obj())).unwrap();
        assert_eq!(plain.get("preset").and_then(|x| x.as_str()), Some("default"));
        assert_eq!(plain.get("file_name").and_then(|x| x.as_str()), Some("Link.yaml"));
        let t: &str = plain.get("text").and_then(|x| x.as_str()).unwrap();
        assert!(t.starts_with("name: \"Link\"\ngame: OOTRR\n") && t.ends_with("OOTRR:\n  preset: default\n"), "{t}");
        let own: Json = player(&Json::obj().with("name", "A \"b\"".into()).with("settings", Json::obj().with("shuffle_cows", true.into()))).unwrap();
        assert_eq!(own.get("preset").and_then(|x| x.as_str()), Some(""));
        let t: &str = own.get("text").and_then(|x| x.as_str()).unwrap();
        assert!(t.starts_with("name: \"A \\\"b\\\"\"\n") && t.contains("\n  settings_string: \"OOTRR1-"), "{t}");
        assert_eq!(own.get("file_name").and_then(|x| x.as_str()), Some("A__b_.yaml"));
        for bad in ["", "seventeen letters", "a\u{7}b"] {
            assert!(player(&Json::obj().with("name", bad.into()).with("settings", Json::obj())).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_client_gets_the_room_the_name_and_this_app() {
        let exe: PB = PB::from("ootrr.exe");
        assert_eq!(client_args("archipelago.gg:38281", "Link", &exe), ["OOTRR Client", "--", "--ootrr", "ootrr.exe", "--connect", "archipelago.gg:38281", "--name", "Link"]);
        assert_eq!(client_args("", "", &exe), ["OOTRR Client", "--", "--ootrr", "ootrr.exe"]);
        let addr = |t: &str| address(&Json::obj().with("address", t.into()));
        assert_eq!(addr(" archipelago://archipelago.gg:38281 ").unwrap(), "archipelago.gg:38281");
        assert_eq!(addr("[::1]:38281").unwrap(), "[::1]:38281");
        assert_eq!(addr("").unwrap(), "");
        for bad in ["--nogui", "a b", "a;calc", "a\"b", "host/path"] {
            assert!(addr(bad).is_err(), "{bad}");
        }
        let d: PB = fake_archipelago("client");
        let body: Json = Json::obj().with("path", d.display().to_string().as_str().into());
        assert_eq!(client(true, &body).unwrap_err().status, 501);
        assert!(client(false, &body.clone().with("name", "-x".into())).is_err());
        std::fs::remove_file(d.join(LAUNCHERS[0])).unwrap();
        assert!(client(false, &body).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_note_names_the_slot_and_where_the_spoiler_is() {
        let p: Patch = Patch::parse(SMALL).unwrap();
        let t: String = note(&ap_build::summary(&p));
        assert!(t.contains("Slot: Link") && t.contains("Archipelago server"));
    }
}
