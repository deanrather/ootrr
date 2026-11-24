//! The menu's hint for the trade slot: each adult trade item's description (its caption in
//! `ActorMsg/PouchContent.msbt`, shown when the cursor is on the slot) ends with a line that says
//! A switches trade items. The caption box holds three lines, so a caption that already fills it
//! is replaced by a two-line one ([`SHORT`]) before the hint.

use super::ORDER;
use crate::error::{Error, Res};
use crate::msbt::{self, Msbt, Tok};
use crate::sarc::{self, Entry};

pub const FILE: &str = "ActorMsg/PouchContent.msbt";
pub const HINT: &str = "Press A again to switch trade items.";
/// Lines the caption box shows.
pub const LINES: usize = 3;

/// A two-line caption per trade item of [`ORDER`], for the ones whose own caption has three.
pub const SHORT: [&str; 11] = [
    "This special egg is like a normal\negg but much smaller.",
    "Hatched from the pocket egg, this cucco\neasily fits in the palm of your hand.",
    "This special blue cucco belongs to the\ncucco lady. It seems he rarely crows.",
    "A mushroom from the Lost Woods. Granny\nin Kakariko Village can use it fresh.",
    "Granny in Kakariko Village made this\nmedicine for someone in the Lost Woods.",
    "The master craftsman's son left this\nold but well-kept saw behind.",
    "A broken Goron blade. Biggoron on\nDeath Mountain may be able to fix it.",
    "A prescription for eye drops that soothe\nash-inflamed eyes. King Zora can fill it.",
    "The key to making eye drops. The\nLakeside Laboratory knows what to do.",
    "A wonder cure that instantly soothes\nash-inflamed eyes. Biggoron needs it.",
    "A claim check for a sword that\nBiggoron is repairing.",
];

pub fn label(actor: &str) -> String {
    format!("{actor}_Caption")
}

fn lines(t: &[Tok]) -> usize {
    1 + t.iter().map(|k| if let Tok::Text(s) = k { s.matches('\n').count() } else { 0 }).sum::<usize>()
}

/// The caption with the hint as its last line.
pub fn hinted(m: &Msbt, k: usize) -> Res<Vec<Tok>> {
    let name: String = label(ORDER[k].actor);
    let mut t: Vec<Tok> = m.text(&name)?;
    let ends: bool = matches!(t.last(), Some(Tok::Text(s)) if s.ends_with(HINT));
    if ends {
        return Err(Error::Msg(format!("trade shuffle: {name} already has the switch hint")));
    }
    if lines(&t) >= LINES {
        t = msbt::from_markup(SHORT[k])?;
    }
    t.push(Tok::Text(format!("\n{HINT}")));
    Ok(t)
}

/// A language archive with the hint on every adult trade item's caption.
pub fn archive(raw: &[u8]) -> Res<Vec<u8>> {
    let mut es: Vec<Entry> = sarc::parse(raw)?;
    let b: Vec<u8> = sarc::get(&es, FILE).ok_or(Error::Format("trade shuffle: a message archive has no PouchContent.msbt"))?.data.clone();
    let mut m: Msbt = msbt::parse(&b)?;
    for k in 0..ORDER.len() {
        let t: Vec<Tok> = hinted(&m, k)?;
        m.set_tokens(&label(ORDER[k].actor), &t)?;
    }
    let w: Vec<u8> = m.write()?;
    msbt::parse(&w)?;
    sarc::set(&mut es, FILE, w);
    sarc::write(&es)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::romfs::Romfs;

    #[test]
    fn the_short_captions_leave_room_for_the_hint() {
        for s in SHORT {
            assert_eq!(s.lines().count(), LINES - 1, "{s}");
            assert!(s.lines().all(|l| l.len() <= 42), "{s}");
        }
        assert!(HINT.len() <= 42);
    }

    #[test]
    fn every_language_gets_the_hint_as_the_last_caption_line() {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        let Ok(mut rf) = Romfs::open(&p) else {
            return;
        };
        let Ok(langs) = crate::hints::languages(&rf) else {
            return;
        };
        assert!(!langs.is_empty());
        for l in langs {
            let raw: Vec<u8> = rf.read_zs(&l).unwrap();
            let out: Vec<u8> = archive(&raw).unwrap();
            assert!(archive(&out).is_err(), "{l}: a second pass is refused");
            let before: Msbt = msbt::parse(&sarc::get(&sarc::parse(&raw).unwrap(), FILE).unwrap().data).unwrap();
            let m: Msbt = msbt::parse(&sarc::get(&sarc::parse(&out).unwrap(), FILE).unwrap().data).unwrap();
            for (k, x) in ORDER.iter().enumerate() {
                let text: String = msbt::to_markup(&m.text(&label(x.actor)).unwrap());
                let rows: Vec<&str> = text.split('\n').collect();
                assert!(rows.len() <= LINES, "{l} {}: {text}", x.actor);
                assert_eq!(rows.last().copied(), Some(HINT), "{l} {}", x.actor);
                let old: String = msbt::to_markup(&before.text(&label(x.actor)).unwrap());
                if old.split('\n').count() < LINES {
                    assert!(text.starts_with(&old), "{l} {}: a caption with room keeps its own text", x.actor);
                } else {
                    assert!(text.starts_with(SHORT[k]), "{l} {}", x.actor);
                }
                assert_eq!(m.raw(&format!("{}_Name", x.actor)), before.raw(&format!("{}_Name", x.actor)), "names are untouched");
            }
            assert_eq!(m.names().len(), before.names().len(), "{l}");
        }
    }
}
