use std::path::PathBuf as PB;

use ootrr_core::custom;
use ootrr_core::json::Json;

use crate::body;
use crate::error::{Fail, Hr};
use crate::pathsafe;

#[derive(Clone, Copy)]
enum Kind {
    Pack,
    Rom,
}

fn sentence(text: &str) -> String {
    let t: &str = text.split_once("custom music: ").map_or(text, |x| x.1).trim_end_matches('.');
    let mut rest: std::str::Chars = t.chars();
    match rest.next() {
        Some(c) => format!("{}{}.", c.to_uppercase(), rest.as_str()),
        None => String::new(),
    }
}

fn look(kind: Kind, text: &str) -> Result<String, String> {
    let p: PB = pathsafe::abs(text.trim().trim_matches('"')).map_err(|e| sentence(&e.to_string()))?;
    if p.is_dir() {
        return Err("That is a folder, not a file.".to_string());
    }
    if !p.is_file() {
        return Err("That file does not exist.".to_string());
    }
    match kind {
        Kind::Pack => custom::check_pack(&p).map(|n| format!("Music pack found, tunes inside: {n}.")),
        Kind::Rom => custom::check_rom(&p).map(|_| "Ocarina of Time N64 ROM found.".to_string()),
    }
    .map_err(|e| sentence(&e.to_string()))
}

/// Says whether a typed music pack or N64 ROM path is one that Generate can read.
pub fn run(b: &Json) -> Hr<Json> {
    let kind: Kind = match body::opt_text(b, "kind")? {
        Some("pack") => Kind::Pack,
        Some("rom") => Kind::Rom,
        _ => return Err(Fail::bad("kind: use pack or rom")),
    };
    let found: Result<String, String> = look(kind, body::opt_text(b, "path")?.unwrap_or(""));
    let ok: bool = found.is_ok();
    let message: String = found.unwrap_or_else(|e| e);
    Ok(Json::obj().with("ok", ok.into()).with("message", message.as_str().into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path as P;

    fn tmp(tag: &str) -> PB {
        let p: PB = std::env::temp_dir().join(format!("ootrr_web_music_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn ask(kind: &str, path: &str) -> (bool, String) {
        let j: Json = run(&Json::obj().with("kind", kind.into()).with("path", path.into())).unwrap();
        (j.get("ok").and_then(|x| x.as_bool()).unwrap(), j.get("message").and_then(|x| x.as_str()).unwrap().to_string())
    }

    fn text(p: &P) -> String {
        p.to_string_lossy().to_string()
    }

    #[test]
    fn a_wrong_path_says_what_is_wrong() {
        let d: PB = tmp("bad");
        for (kind, name) in [("pack", "fake.otr"), ("rom", "fake.z64")] {
            let file: PB = d.join(name);
            fs::write(&file, b"not really one of these").unwrap();
            let (ok, why): (bool, String) = ask(kind, &text(&file));
            assert!(!ok && why.starts_with("The ") && why.ends_with('.') && !why.contains("custom music"), "{why}");
            assert_eq!(ask(kind, &text(&d)), (false, "That is a folder, not a file.".to_string()));
            assert_eq!(ask(kind, &text(&d.join("missing"))), (false, "That file does not exist.".to_string()));
            assert_eq!(ask(kind, &format!(" \"{}\" ", text(&d.join("missing")))).1, "That file does not exist.");
            assert_eq!(ask(kind, "relative.otr"), (false, "The path must be absolute.".to_string()));
            assert!(!ask(kind, "").0);
        }
        assert_eq!(run(&Json::obj().with("kind", "exe".into())).unwrap_err().status, 400);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn real_files_are_recognised_quoted_or_not_and_each_is_refused_as_the_other() {
        let env = |name: &str| std::env::var(name).ok().filter(|p| P::new(p).is_file());
        let (Some(otr), Some(n64)) = (env("OOTRR_OTR"), env("OOTRR_N64_ROM")) else {
            return;
        };
        for (kind, file) in [("pack", &otr), ("rom", &n64)] {
            for typed in [file.clone(), format!("\"{file}\""), format!("  \"{file}\"  ")] {
                let (ok, why): (bool, String) = ask(kind, &typed);
                println!("{kind}: {why}");
                assert!(ok, "{why}");
            }
        }
        for (kind, file) in [("pack", &n64), ("rom", &otr)] {
            let (ok, why): (bool, String) = ask(kind, file);
            println!("{kind} given the other file: {why}");
            assert!(!ok);
        }
    }
}
