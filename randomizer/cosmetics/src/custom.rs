use std::collections::{BTreeMap as BM, BTreeSet as BS};
use std::fs;
use std::io::Read;
use std::path::{Path as P, PathBuf as PB};
use std::sync::Arc;

use crate::bwav::{self, Coded};
use crate::error::{Error as E, Res};
use crate::n64render::{self, Take};
use crate::n64rom::{Rom, Sample, Tables};
use crate::n64seq::Fonts;
use crate::pack::{self, Tune};
use crate::rng::Rng;
use crate::romfs::Romfs;
use crate::rsizetable::hash;
use crate::sfx;
use crate::zs;

const RES: &str = "Sound/Resource";
const STREAM: &str = "Sound/Resource/Stream";
const MUSIC_LEVEL: f32 = 750.0;
const FANFARE_LEVEL: f32 = 1700.0;
const CEILING: f32 = 30000.0;
const AUDIBLE: f32 = 50.0;
const MOST_THREADS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub file: String,
    pub fanfare: bool,
}

struct Place {
    bank: String,
    at: usize,
    prefetch: bool,
}

pub fn level(audio: &[[f32; 2]], target: f32) -> Option<f32> {
    let mut sum: f64 = 0.0;
    let mut n: usize = 0;
    let mut peak: f32 = 0.0;
    for v in audio.iter().flatten() {
        peak = peak.max(v.abs());
        if v.abs() > AUDIBLE {
            sum += f64::from(*v).powi(2);
            n += 1;
        }
    }
    (n > 0).then(|| (target / (sum / n as f64).sqrt() as f32).min(CEILING / peak))
}

pub fn stream_of(take: &Take, fanfare: bool) -> Option<Vec<u8>> {
    let gain: f32 = level(&take.audio, if fanfare { FANFARE_LEVEL } else { MUSIC_LEVEL })?;
    let mut sides: Vec<Vec<i16>> = (0..2).map(|s| bwav::resample(&take.audio, n64render::RATE, s, gain)).collect();
    let mut from: Option<usize> = take.repeat.map(|r| r.0 * bwav::RATE as usize / n64render::RATE);
    if !fanfare && from.is_none() {
        from = Some(0);
    }
    for side in &mut sides {
        if side.len() < bwav::PREFETCH {
            side.resize(bwav::PREFETCH, 0);
        }
    }
    let coded: Vec<Coded> = sides.iter().map(|s| bwav::encode(s)).collect();
    Some(bwav::write(&coded, from))
}

pub fn deal(slots: &[Slot], tunes: &[Tune], rng: &mut Rng) -> Vec<(Slot, Vec<usize>)> {
    let mut out: Vec<(Slot, Vec<usize>)> = Vec::new();
    for fanfare in [false, true] {
        let mut order: Vec<usize> = (0..tunes.len()).filter(|i| tunes[*i].fanfare == fanfare).collect();
        rng.shuffle(&mut order);
        let wanted: Vec<&Slot> = slots.iter().filter(|s| s.fanfare == fanfare).collect();
        if order.is_empty() {
            continue;
        }
        let spare: usize = order.len().saturating_sub(wanted.len());
        for (i, slot) in wanted.iter().enumerate() {
            let mut picks: Vec<usize> = vec![order[i % order.len()]];
            if spare > 0 {
                picks.extend((0..3).map(|k| order[wanted.len() + (i + k * wanted.len()) % spare]));
            }
            out.push(((*slot).clone(), picks));
        }
    }
    out
}

fn stereo(file: &P) -> bool {
    let mut head: [u8; 16] = [0; 16];
    fs::File::open(file).and_then(|mut f| f.read_exact(&mut head)).is_ok() && head.starts_with(b"BWAV") && head[0xE] == 2
}

fn places(rf: &mut Romfs, slots: &[Slot]) -> Res<BM<String, Vec<Place>>> {
    let want: BM<u32, &str> = slots.iter().map(|s| (hash(&s.file), s.file.as_str())).collect();
    let mut out: BM<String, Vec<Place>> = BM::new();
    let banks: Vec<String> = rf.list(RES)?.into_iter().filter(|r| r.ends_with(".bars.zs")).collect();
    for bank in banks {
        let raw: Vec<u8> = rf.read_zs(&bank)?;
        for (h, at) in sfx::entries(&raw)? {
            let Some(file) = want.get(&h) else {
                continue;
            };
            let head: &[u8] = raw.get(at..at + 16).ok_or(E::Format("custom music: a sound bank entry leaves the file"))?;
            if head.starts_with(b"BWAV") && head[0xE] == 2 {
                out.entry(file.to_string()).or_default().push(Place { bank: bank.clone(), at, prefetch: head[0xC] == 1 });
            }
        }
    }
    Ok(out)
}

fn load_fonts(rom: &Rom, tunes: &[Tune]) -> Res<(Fonts, BS<u8>)> {
    let wanted: BS<u8> = tunes.iter().flat_map(|t| t.fonts.iter().copied()).collect();
    let mut cache: BM<(usize, usize), Arc<Sample>> = BM::new();
    let mut fonts: Fonts = Fonts::new();
    let mut missing: BS<u8> = BS::new();
    for f in wanted {
        match rom.font(usize::from(f), &mut cache) {
            Ok(font) => {
                fonts.insert(f, Arc::new(font));
            }
            Err(_) => {
                missing.insert(f);
            }
        }
    }
    if fonts.is_empty() {
        return Err(E::Format("custom music: none of the instrument sets the pack uses could be read from the N64 ROM"));
    }
    Ok((fonts, missing))
}

fn make(slot: &Slot, picks: &[usize], tunes: &[Tune], fonts: &Fonts, tables: &Tables) -> Option<(usize, Vec<u8>)> {
    picks.iter().find_map(|&i| {
        let t: &Tune = &tunes[i];
        if !t.fonts.iter().all(|f| fonts.contains_key(f)) {
            return None;
        }
        let take: Take = n64render::render(&t.data, &t.fonts, fonts, tables, slot.fanfare);
        stream_of(&take, slot.fanfare).map(|b| (i, b))
    })
}

type Made = (String, String, Vec<u8>, Vec<u8>);

fn work(jobs: &[(Slot, Vec<usize>)], tunes: &[Tune], fonts: &Fonts, tables: &Tables, out: &P) -> Res<Vec<Made>> {
    let mut made: Vec<Made> = Vec::new();
    for (slot, picks) in jobs {
        let Some((i, stream)) = make(slot, picks, tunes, fonts, tables) else {
            continue;
        };
        fs::write(out.join(format!("{}.bwav", slot.file)), &stream)?;
        let short = || E::Format("custom music: a rendered track is too short");
        made.push((slot.file.clone(), tunes[i].name.clone(), bwav::bank_copy(&stream, false).ok_or_else(short)?, bwav::bank_copy(&stream, true).ok_or_else(short)?));
    }
    Ok(made)
}

pub fn check_pack(file: &P) -> Res<usize> {
    Ok(pack::read(file)?.len())
}

pub fn check_rom(file: &P) -> Res<()> {
    Rom::open(file)?;
    Ok(())
}

pub fn check(packs: &[PB], rom: &P) -> Res<()> {
    for p in packs {
        check_pack(p)?;
    }
    check_rom(rom)
}

pub fn apply(mod_root: &P, romfs: &P, packs: &[PB], rom: &P, slots: &BS<(String, bool)>, seed: &str) -> Res<Vec<String>> {
    let mut tunes: Vec<Tune> = Vec::new();
    for p in packs {
        tunes.extend(pack::read(p)?);
    }
    let rom: Rom = Rom::open(rom)?;
    let (fonts, missing): (Fonts, BS<u8>) = load_fonts(&rom, &tunes)?;
    tunes.retain(|t| t.fonts.iter().all(|f| !missing.contains(f)));
    let source: PB = romfs.join(STREAM);
    let slots: Vec<Slot> = slots.iter().filter(|s| stereo(&source.join(format!("{}.bwav", s.0)))).map(|s| Slot { file: s.0.clone(), fanfare: s.1 }).collect();
    let mut rf: Romfs = Romfs::open(romfs)?;
    let places: BM<String, Vec<Place>> = places(&mut rf, &slots)?;
    let slots: Vec<Slot> = slots.into_iter().filter(|s| places.contains_key(&s.file)).collect();
    let mut rng: Rng = Rng::from_text(&format!("{seed} custom music"));
    let jobs: Vec<(Slot, Vec<usize>)> = deal(&slots, &tunes, &mut rng);
    let out: PB = mod_root.join("romfs").join(STREAM);
    fs::create_dir_all(&out)?;
    let threads: usize = std::thread::available_parallelism().map_or(1, |n| n.get()).min(MOST_THREADS);
    let made: Vec<Made> = if threads < 2 {
        work(&jobs, &tunes, &fonts, &rom.tables, &out)?
    } else {
        let size: usize = jobs.len().div_ceil(threads).max(1);
        std::thread::scope(|s| {
            let running: Vec<std::thread::ScopedJoinHandle<Res<Vec<Made>>>> = jobs.chunks(size).map(|part| s.spawn(|| work(part, &tunes, &fonts, &rom.tables, &out))).collect();
            let mut all: Vec<Made> = Vec::new();
            for r in running {
                all.extend(r.join().map_err(|_| E::Format("custom music: rendering a tune failed"))??);
            }
            Ok::<Vec<Made>, E>(all)
        })?
    };
    if made.is_empty() {
        return Err(E::Format("custom music: no tune from the pack could be put in the mod"));
    }
    let banks: BS<&String> = made.iter().flat_map(|m| places[&m.0].iter().map(|p| &p.bank)).collect();
    for bank in banks {
        let at: PB = mod_root.join("romfs").join(bank.as_str());
        let original: Vec<u8> = rf.read(bank)?;
        let z: Vec<u8> = if at.try_exists()? { fs::read(&at)? } else { original.clone() };
        let mut raw: Vec<u8> = rf.zs.decompress(&z)?;
        for (file, _, head, prefetch) in &made {
            for p in places[file].iter().filter(|p| &p.bank == bank) {
                let new: &[u8] = if p.prefetch { prefetch } else { head };
                raw.get_mut(p.at..p.at + new.len()).ok_or(E::Format("custom music: a sound bank entry leaves the file"))?.copy_from_slice(new);
            }
        }
        if let Some(d) = at.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(&at, zs::compress_raw(&raw, zs::dict_of(&original))?)?;
    }
    let used: Vec<&str> = made.iter().map(|m| m.1.as_str()).collect();
    Ok(vec![format!("Custom music: {} of the game's tracks now play tunes from your pack: {}.", made.len(), used.join("; "))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmetic::{self, Cosmetics, Music};

    fn tune(name: &str, fanfare: bool) -> Tune {
        Tune { name: name.to_string(), fanfare, data: Vec::new(), fonts: vec![3] }
    }

    #[test]
    fn music_goes_to_music_and_fanfares_to_fanfares_without_repeats_while_the_pack_lasts() {
        let slots: Vec<Slot> = ["a", "b", "c"].iter().map(|f| Slot { file: f.to_string(), fanfare: false }).chain([Slot { file: "f".to_string(), fanfare: true }]).collect();
        let tunes: Vec<Tune> = ["m1", "m2", "m3", "m4"].iter().map(|n| tune(n, false)).chain([tune("f1", true)]).collect();
        let jobs: Vec<(Slot, Vec<usize>)> = deal(&slots, &tunes, &mut Rng::from_text("x"));
        let first: BS<usize> = jobs.iter().filter(|j| !j.0.fanfare).map(|j| j.1[0]).collect();
        assert_eq!(first.len(), 3);
        for (slot, picks) in &jobs {
            assert!(picks.iter().all(|i| tunes[*i].fanfare == slot.fanfare));
        }
        assert_eq!(deal(&slots, &tunes[..4], &mut Rng::from_text("x")).len(), 3);
    }

    #[test]
    fn the_real_files_take_a_real_pack_and_each_bank_copy_matches_its_stream() {
        let env = |name: &str| std::env::var(name).ok().map(PB::from).filter(|p| p.exists());
        let (Some(root), Some(otr), Some(n64)) = (env("OOTRR_ROMFS"), env("OOTRR_OTR"), env("OOTRR_N64_ROM")) else {
            return;
        };
        let out: PB = std::env::temp_dir().join(format!("ootrr_custom_{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        let c: Cosmetics = Cosmetics { music: Music::Grouped, seed: "custom test".to_string(), custom_music: vec![otr.to_string_lossy().to_string()], n64_rom: n64.to_string_lossy().to_string(), ..Cosmetics::default() };
        let started: std::time::Instant = std::time::Instant::now();
        let notes: Vec<String> = cosmetic::apply(&out, &root, &c).unwrap();
        notes.iter().for_each(|n| println!("{n}"));
        let made: PB = out.join("romfs").join(STREAM);
        let files: Vec<PB> = fs::read_dir(&made).unwrap().map(|e| e.unwrap().path()).collect();
        let slots: Vec<Slot> = files.iter().map(|p| Slot { file: p.file_stem().unwrap().to_string_lossy().to_string(), fanfare: false }).collect();
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let places: BM<String, Vec<Place>> = places(&mut rf, &slots).unwrap();
        let mut total: u64 = 0;
        for (slot, path) in slots.iter().zip(&files) {
            let stream: Vec<u8> = fs::read(path).unwrap();
            total += stream.len() as u64;
            for p in &places[&slot.file] {
                let raw: Vec<u8> = rf.zs.decompress(&fs::read(out.join("romfs").join(&p.bank)).unwrap()).unwrap();
                assert_eq!(raw.len(), rf.read_zs(&p.bank).unwrap().len());
                let want: Vec<u8> = bwav::bank_copy(&stream, p.prefetch).unwrap();
                assert!(raw[p.at..p.at + want.len()] == want[..], "{}", slot.file);
            }
        }
        println!("{} tracks, {} MB, {:.0} s", files.len(), total / 1_000_000, started.elapsed().as_secs_f32());
        assert!(files.len() >= 20);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn custom_music_without_a_shuffle_or_a_rom_is_refused() {
        let no_shuffle: Cosmetics = Cosmetics { custom_music: vec!["a.otr".to_string()], ..Cosmetics::default() };
        assert!(cosmetic::apply(P::new("unused"), P::new("unused"), &no_shuffle).is_err());
        let no_rom: Cosmetics = Cosmetics { music: Music::Anywhere, ..no_shuffle };
        assert!(cosmetic::apply(P::new("unused"), P::new("unused"), &no_rom).is_err());
    }

    #[test]
    fn loudness_is_matched_without_passing_the_ceiling() {
        assert_eq!(level(&[[1000.0, -1000.0]; 4], 500.0), Some(0.5));
        let mut spike: Vec<[f32; 2]> = vec![[100.0, 100.0]; 1000];
        spike[0] = [30000.0, 0.0];
        assert!(level(&spike, 5000.0).unwrap() <= 1.0);
        assert_eq!(level(&[[0.0, 0.0]; 4], 500.0), None);
    }
}
