use std::fs;
use std::path::{Path as P, PathBuf as PB};

use crate::byml::{Byml, Node};
use crate::error::{Error as E, Res};
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst};
use crate::slink::{Link, SLINK};
use crate::zs;

pub const GROUP_FILE: &str = "Sound/Group/GroupSetting.Product.111.byml.zs";
pub const QUIET: [&str; 4] = ["BgmDefault", "BgmEndReverb", "BgmEnemyBattle", "BgmInsideWorld"];
pub const SCENE: &str = "DemoBgmDefault";
const AREA: &str = "BgmDefault";
const MARGIN: u32 = 0x400;

pub fn is_scene_music(asset: &str, jingles: &[&str]) -> bool {
    asset.starts_with("BGM_") && !jingles.contains(&asset)
}

pub fn mute(y: &mut Byml) -> Res<()> {
    for g in QUIET {
        let set: bool = y.root.get_mut("GroupParam").and_then(|p| p.get_mut(g)).and_then(|e| e.get_mut("Default")).is_some_and(|d| d.set("Volume", Node::F32(0.0)));
        if !set {
            return Err(E::Msg(format!("music: the sound groups have no volume for {g}")));
        }
    }
    Ok(())
}

fn put(mod_root: &P, rel: &str, data: &[u8]) -> Res<()> {
    let out: PB = mod_root.join("romfs").join(rel);
    if let Some(d) = out.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(&out, data)?;
    Ok(())
}

pub fn apply(mod_root: &P, rf: &mut Romfs, jingles: &[&str]) -> Res<Vec<String>> {
    let z: Vec<u8> = rf.read(SLINK)?;
    let mut raw: Vec<u8> = rf.zs.decompress(&z)?;
    let link: Link = Link::read(&raw)?;
    let moved: usize = link.regroup(&mut raw, SCENE, AREA, &|a| is_scene_music(a, jingles))?;
    if moved == 0 {
        return Err(E::Format("music: no scene track could be moved to the silent group"));
    }
    put(mod_root, SLINK, &zs::compress_raw(&raw, zs::dict_of(&z))?)?;

    let z: Vec<u8> = rf.read(GROUP_FILE)?;
    let old: Vec<u8> = rf.zs.decompress(&z)?;
    let mut y: Byml = Byml::parse(&old)?;
    mute(&mut y)?;
    let new: Vec<u8> = y.write()?;
    put(mod_root, GROUP_FILE, &zs::compress_raw(&new, zs::dict_of(&z))?)?;

    let at: PB = mod_root.join("romfs").join(rsizetable::TABLE_PATH);
    let mut t: Rst = match fs::read(&at) {
        Ok(b) if !b.is_empty() => Rst::parse(&rf.zs.decompress(&b)?)?,
        _ => rsizetable::load(rf)?,
    };
    if t.grow(GROUP_FILE, old.len() as u64, new.len() as u64, MARGIN)?.is_none() {
        return Err(E::Format("music: the size table has no entry for the sound groups"));
    }
    put(mod_root, rsizetable::TABLE_PATH, &t.to_zs()?)?;
    Ok(vec!["No Music: background music is silent. Fanfares, jingles, sound effects and the ocarina are still heard.".to_string()])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap as BM;

    use super::*;

    fn groups(names: &[&str]) -> Byml {
        let all: Vec<(String, Node)> = names.iter().map(|n| (n.to_string(), Node::Map(vec![("Default".to_string(), Node::Map(vec![("Volume".to_string(), Node::F32(1.0))]))]))).collect();
        Byml { root: Node::Map(vec![("GroupParam".to_string(), Node::Map(all))]), version: 7, big: true }
    }

    fn volume(y: &Byml, g: &str) -> Option<Node> {
        y.root.get("GroupParam")?.get(g)?.get("Default")?.get("Volume").cloned()
    }

    #[test]
    fn only_the_music_groups_are_muted_and_a_missing_group_is_refused() {
        let others: [&str; 4] = ["FanfareDefault", "OcarinaPlaying", "BgmActorPlaying", SCENE];
        let mut y: Byml = groups(&[&QUIET[..], &others[..]].concat());
        mute(&mut y).unwrap();
        for g in QUIET {
            assert_eq!(volume(&y, g), Some(Node::F32(0.0)), "{g}");
        }
        for g in others {
            assert_eq!(volume(&y, g), Some(Node::F32(1.0)), "{g}");
        }
        assert!(mute(&mut groups(&QUIET[1..])).is_err());
    }

    #[test]
    fn scene_music_is_a_bgm_track_that_is_not_a_jingle() {
        let j: [&str; 1] = ["BGM_SleepInBed"];
        assert!(is_scene_music("BGM_Shop_Desert", &j));
        assert!(!is_scene_music("BGM_SleepInBed", &j));
        assert!(!is_scene_music("ExampleMelody_SunSong", &j));
    }

    fn calls(link: &Link, raw: &[u8]) -> BM<(u32, String), (String, String)> {
        link.calls.iter().filter_map(|c| Some(((c.user, c.key.clone()), (link.asset(raw, c)?, link.group(raw, c)?)))).collect()
    }

    #[test]
    fn the_real_files_leave_no_music_call_in_an_audible_music_group() {
        let Some(root) = std::env::var("OOTRR_ROMFS").ok().map(PB::from).filter(|p| p.join("Pack").join("ZsDic.pack.zs").is_file()) else {
            return;
        };
        let out: PB = std::env::temp_dir().join(format!("ootrr_silence_{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        let jingles: [&str; 3] = ["BGM_ItemGet_FromZelda", "BGM_SleepInBed", "BGM_SleepInBedToEnemyMuseum"];
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        apply(&out, &mut rf, &jingles).unwrap();
        let read = |rf: &mut Romfs, rel: &str| -> Vec<u8> { rf.zs.decompress(&fs::read(out.join("romfs").join(rel)).unwrap()).unwrap() };

        let was: Vec<u8> = rf.read_zs(SLINK).unwrap();
        let now: Vec<u8> = read(&mut rf, SLINK);
        assert_eq!(was.len(), now.len());
        let before = calls(&Link::read(&was).unwrap(), &was);
        let after = calls(&Link::read(&now).unwrap(), &now);
        assert_eq!(before.len(), after.len());
        let mut moved: usize = 0;
        for (k, (asset, group)) in &after {
            let (a, g) = &before[k];
            assert_eq!(a, asset);
            if g != group {
                assert!(g == SCENE && group == AREA && is_scene_music(asset, &jingles), "{asset} {g} {group}");
                moved += 1;
            }
            if asset.starts_with("BGM_") {
                let kept: bool = group == "FanfareDefault" || group == "BgmActorPlaying" || (group == SCENE && jingles.contains(&asset.as_str()));
                assert!(QUIET.contains(&group.as_str()) || kept, "{asset} stays audible in {group}");
            } else {
                assert!(!QUIET.contains(&group.as_str()), "{asset} would be silenced in {group}");
            }
        }
        let changed: usize = was.iter().zip(now.iter()).filter(|(a, b)| a != b).count();
        assert!(moved >= 30 && changed <= moved * 4, "{moved} {changed}");

        let old: Byml = rf.read_byml(GROUP_FILE).unwrap();
        let bytes: Vec<u8> = read(&mut rf, GROUP_FILE);
        let mut want: Byml = old.clone();
        mute(&mut want).unwrap();
        assert_eq!(Byml::parse(&bytes).unwrap(), want);
        let table: Rst = Rst::parse(&read(&mut rf, rsizetable::TABLE_PATH)).unwrap();
        let game: Rst = rsizetable::load(&mut rf).unwrap();
        let grown: u64 = bytes.len() as u64 - rf.read_zs(GROUP_FILE).unwrap().len() as u64;
        assert!(u64::from(table.get(GROUP_FILE).unwrap()) >= u64::from(game.get(GROUP_FILE).unwrap()) + grown);
        println!("{moved} scene calls moved, groups grew {grown} bytes");
        let _ = fs::remove_dir_all(&out);
    }
}
