use std::collections::{BTreeMap as BM, BTreeSet as BS};

use crate::byml::{Byml, Node};
use crate::entr::data::{load, Cat, Door, DEST_KEYS};
use crate::entr::layout::{ages_fit, Layout};
use crate::entr::mode::{self, Mode};
use crate::entr::pools::DUNGEONS;
use crate::entr::text;
use crate::entrance_swap::diff;
use crate::entrances::ENTRANCE;
use crate::error::{Error as E, Res};
use crate::placement::hash_of;
use crate::romfs::Romfs;
use crate::settings::seed::SeedFile;
use crate::slice::edit::Edited;

pub type Move = (u64, u64);

type Vals = Vec<(&'static str, Option<Node>)>;

pub fn pick_like(cat: &Cat, target: u64, from: &[u64]) -> Res<u64> {
    let t: &Door = cat.door(target)?;
    let mut best: Option<(u8, u64)> = None;
    for h in from {
        let d: &Door = cat.door(*h)?;
        let score: u8 = match (d.age == t.age, d.alt() == t.alt()) {
            (true, true) => 0,
            (true, false) => 1,
            _ => 2,
        };
        if best.is_none_or(|b| score < b.0) {
            best = Some((score, *h));
        }
    }
    best.map(|b| b.1).ok_or(E::Format("an entrance swap has no source actor"))
}

pub fn moves(cat: &Cat, l: &Layout) -> Res<Vec<Move>> {
    l.check()?;
    let mut out: Vec<Move> = Vec::new();
    for (spot, d) in l.at.iter().enumerate() {
        if spot == *d {
            continue;
        }
        for t in DUNGEONS[spot].doors {
            out.push((*t, pick_like(cat, *t, DUNGEONS[*d].doors)?));
        }
        out.push((DUNGEONS[*d].back, DUNGEONS[spot].back));
    }
    Ok(out)
}

pub fn swap_moves(cat: &Cat, a: (&[u64], &[u64]), b: (&[u64], &[u64])) -> Res<Vec<Move>> {
    let mut out: Vec<Move> = Vec::new();
    for t in a.0 {
        out.push((*t, pick_like(cat, *t, b.0)?));
    }
    for t in b.0 {
        out.push((*t, pick_like(cat, *t, a.0)?));
    }
    for t in b.1 {
        out.push((*t, pick_like(cat, *t, a.1)?));
    }
    for t in a.1 {
        out.push((*t, pick_like(cat, *t, b.1)?));
    }
    Ok(out)
}

pub fn actor<'a>(y: &'a Byml, h: u64) -> Option<&'a Node> {
    y.root.get("Actors")?.as_arr()?.iter().find(|a| hash_of(a) == Some(h))
}

fn actor_mut(y: &mut Byml, h: u64) -> Option<&mut Node> {
    y.root.get_mut("Actors")?.as_arr_mut()?.iter_mut().find(|a| hash_of(a) == Some(h))
}

fn is_entrance(n: &Node) -> bool {
    n.get("Gyaml").and_then(|g| g.as_str()) == Some(ENTRANCE)
}

pub fn vals(n: &Node) -> Vals {
    let d: Option<&Node> = n.get("Dynamic");
    DEST_KEYS.iter().map(|k| (*k, d.and_then(|x| x.get(k)).cloned())).collect()
}

fn put(n: &mut Node, v: &Vals) -> Res<()> {
    let d: &mut Node = n.get_mut("Dynamic").ok_or(E::Format("entrance actor has no Dynamic map"))?;
    for (k, x) in v {
        match x {
            Some(val) => {
                d.set(k, val.clone());
            }
            None => {
                d.remove(k);
            }
        }
    }
    Ok(())
}

fn hashes_by_index(y: &Byml) -> Vec<u64> {
    y.root.get("Actors").and_then(|a| a.as_arr()).map(|l| l.iter().map(|a| hash_of(a).unwrap_or(0)).collect()).unwrap_or_default()
}

fn allowed(path: &str, idx: &[u64], want: &BS<u64>) -> bool {
    let Some(rest) = path.strip_prefix(".Actors[") else {
        return false;
    };
    let Some((i, tail)) = rest.split_once(']') else {
        return false;
    };
    let Ok(i) = i.parse::<usize>() else {
        return false;
    };
    let key: &str = tail.split(": ").next().unwrap_or("");
    let on_target: bool = idx.get(i).is_some_and(|h| want.contains(h));
    on_target && DEST_KEYS.iter().any(|k| key == format!(".Dynamic.{k}"))
}

pub struct Staged {
    pub actors: usize,
    pub files: Vec<String>,
}

pub fn stage(rf: &mut Romfs, cat: &Cat, mv: &[Move], edits: &mut Vec<Edited>) -> Res<Staged> {
    let mut vanilla: BM<String, Byml> = BM::new();
    let mut by_file: BM<String, Vec<(u64, Vals)>> = BM::new();
    let mut seen: BS<u64> = BS::new();
    for (t, s) in mv {
        if !seen.insert(*t) {
            return Err(E::Msg(format!("entrance {t} is rewritten twice")));
        }
        let src: &Door = cat.door(*s)?;
        if !vanilla.contains_key(&src.file) {
            let y: Byml = Byml::parse(&rf.read_zs(&src.file)?)?;
            vanilla.insert(src.file.clone(), y);
        }
        let n: &Node = actor(&vanilla[&src.file], *s).ok_or_else(|| E::Msg(format!("entrance {s} was not found in {}, the game files differ", src.file)))?;
        if !is_entrance(n) {
            return Err(E::Msg(format!("actor {s} in {} is not an entrance", src.file)));
        }
        by_file.entry(cat.door(*t)?.file.clone()).or_default().push((*t, vals(n)));
    }
    let mut out: Staged = Staged { actors: 0, files: Vec::new() };
    for (f, list) in by_file {
        let at: Option<usize> = edits.iter().position(|e| e.file == f);
        let (raw, old): (Vec<u8>, usize) = match at {
            Some(i) => (edits[i].bytes.clone(), edits[i].old_len),
            None => {
                let b: Vec<u8> = rf.read_zs(&f)?;
                let n: usize = b.len();
                (b, n)
            }
        };
        let before: Byml = Byml::parse(&raw)?;
        let mut y: Byml = before.clone();
        let want: BS<u64> = list.iter().map(|x| x.0).collect();
        for (t, v) in &list {
            let n: &mut Node = actor_mut(&mut y, *t).ok_or_else(|| E::Msg(format!("entrance {t} was not found in {f}, the game files differ")))?;
            if !is_entrance(n) {
                return Err(E::Msg(format!("actor {t} in {f} is not an entrance")));
            }
            put(n, v)?;
            out.actors += 1;
        }
        let bytes: Vec<u8> = y.write()?;
        let back: Byml = Byml::parse(&bytes)?;
        if back != y {
            return Err(E::Msg(format!("{f}: the entrance edit does not read back as written")));
        }
        let mut d: Vec<String> = Vec::new();
        diff(&before.root, &back.root, "", &mut d);
        let idx: Vec<u64> = hashes_by_index(&before);
        if let Some(p) = d.iter().find(|p| !allowed(p, &idx, &want)) {
            return Err(E::Msg(format!("{f}: the entrance edit changed something else, {p}")));
        }
        out.files.push(f.clone());
        match at {
            Some(i) => edits[i].bytes = bytes,
            None => edits.push(Edited {
                file: f,
                old_len: old,
                bytes,
                hits: Vec::new(),
                used: Vec::new(),
                hearts: Vec::new(),
                wraps: Vec::new(),
                touched: Vec::new(),
            }),
        }
    }
    Ok(out)
}

pub fn apply(rf: &mut Romfs, seed: &SeedFile, edits: &mut Vec<Edited>) -> Res<Vec<String>> {
    if seed.entrances.is_empty() {
        return Ok(Vec::new());
    }
    let m: Mode = mode::parse(seed.settings()?.settings.text(mode::SETTING).unwrap_or("off"))?;
    if m != Mode::Dungeons {
        return Err(E::Msg(format!("this seed lists entrances but its settings say {} is {}, generate it again", mode::SETTING, m.id())));
    }
    let l: Layout = Layout::from_map(&seed.entrances)?;
    let cat: Cat = load()?;
    let bad: Vec<String> = ages_fit(&cat, &l)?;
    if let Some(b) = bad.first() {
        return Err(E::Msg(format!("this seed's entrance layout is not safe to build: {b}")));
    }
    let mv: Vec<Move> = moves(&cat, &l)?;
    let st: Staged = stage(rf, &cat, &mv, edits)?;
    let mut notes: Vec<String> = text::report(&l).lines().map(|x| x.to_string()).collect();
    notes.push(format!("{} entrance actors were rewritten in {} map files for the dungeon entrances.", st.actors, st.files.len()));
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entr::pools::find;

    #[test]
    fn a_dungeon_swap_moves_every_door_and_both_ways_out() {
        let c: Cat = load().unwrap();
        let mut l: Layout = Layout::vanilla();
        let deku: usize = find("Deku Tree").unwrap();
        let jabu: usize = find("Jabu Jabus Belly").unwrap();
        l.at.swap(deku, jabu);
        let mv: Vec<Move> = moves(&c, &l).unwrap();
        assert_eq!(mv.len(), 2 + 1 + 1 + 1);
        for t in DUNGEONS[deku].doors {
            assert!(mv.contains(&(*t, DUNGEONS[jabu].doors[0])));
        }
        assert!(mv.contains(&(DUNGEONS[jabu].doors[0], 5055470052273839817)));
        assert!(mv.contains(&(DUNGEONS[jabu].back, DUNGEONS[deku].back)));
        assert!(mv.contains(&(DUNGEONS[deku].back, DUNGEONS[jabu].back)));
        assert!(moves(&c, &Layout::vanilla()).unwrap().is_empty());
    }

    #[test]
    fn sources_follow_the_target_age_and_variant() {
        let c: Cat = load().unwrap();
        let well: &[u64] = DUNGEONS[find("Bottom of the Well").unwrap()].doors;
        assert_eq!(pick_like(&c, 6398355764627578761, well).unwrap(), 6398355764627578761);
        assert_eq!(pick_like(&c, 11591212048579065524, well).unwrap(), 7724348420662166424);
        assert_eq!(pick_like(&c, 5055470052273839817, well).unwrap(), 7282308262581659982);
    }

    #[test]
    fn only_destination_keys_on_targets_pass_the_diff_guard() {
        let idx: Vec<u64> = vec![7, 9];
        let want: BS<u64> = [9].into_iter().collect();
        assert!(allowed(".Actors[1].Dynamic.NextStartPosName: a -> b", &idx, &want));
        assert!(allowed(".Actors[1].Dynamic.InitEventName: removed", &idx, &want));
        assert!(!allowed(".Actors[0].Dynamic.NextStartPosName: a -> b", &idx, &want));
        assert!(!allowed(".Actors[1].Dynamic.FirstEventName: a -> b", &idx, &want));
        assert!(!allowed(".Actors[1].Translate[0]: 1 -> 2", &idx, &want));
    }
}
