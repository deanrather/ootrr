use std::fs;
use std::path::{Path as P, PathBuf as PB};

use crate::byml::{Byml, Node};
use crate::error::{Error as E, Res};
use crate::fanfare;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst};
use crate::slink::{Link, SLINK};
use crate::zs;

pub const GROUP_FILE: &str = "Sound/Group/GroupSetting.Product.111.byml.zs";
pub const QUIET: [&str; 4] = ["BgmDefault", "BgmEndReverb", "BgmEnemyBattle", "BgmInsideWorld"];
pub const SCENE: &str = "DemoBgmDefault";
const AREA: &str = "BgmDefault";
const MARGIN: u32 = 0x400;

pub fn is_scene_music(asset: &str) -> bool {
    asset.starts_with("BGM_")
}

pub fn muted(y: &Byml) -> Vec<String> {
    let mut out: Vec<String> = QUIET.iter().map(|g| g.to_string()).collect();
    out.extend(fanfare::ducking_groups(y));
    out
}

pub fn mute(y: &mut Byml) -> Res<()> {
    let all: Vec<String> = muted(y);
    if all.len() == QUIET.len() {
        return Err(E::Format("music: no sound group ducks the music, so the fanfares cannot be found"));
    }
    for g in all {
        let set: bool = y.root.get_mut("GroupParam").and_then(|p| p.get_mut(&g)).and_then(|e| e.get_mut("Default")).is_some_and(|d| d.set("Volume", Node::F32(0.0)));
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

pub fn apply(mod_root: &P, rf: &mut Romfs) -> Res<Vec<String>> {
    let z: Vec<u8> = rf.read(SLINK)?;
    let mut raw: Vec<u8> = rf.zs.decompress(&z)?;
    let link: Link = Link::read(&raw)?;
    let moved: usize = link.regroup(&mut raw, SCENE, AREA, &is_scene_music)?;
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
    Ok(vec!["No Music: background music, fanfares and jingles are silent. Sound effects, the ocarina and performing characters are still heard.".to_string()])
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap as BM, BTreeSet as BS};

    use super::*;

    fn groups(names: &[&str]) -> Byml {
        let entry = |n: &str| -> Node {
            let ducker: &str = if n == "FanfareDefault" { "fanfare playing" } else { "" };
            Node::Map(vec![
                ("Default".to_string(), Node::Map(vec![("Volume".to_string(), Node::F32(1.0))])),
                ("Ducking".to_string(), Node::Map(vec![("ManualDuckerName".to_string(), Node::Str(ducker.to_string()))])),
            ])
        };
        let all: Vec<(String, Node)> = names.iter().map(|n| (n.to_string(), entry(n))).collect();
        let target: Node = Node::Map(vec![("TargetGroupName".to_string(), Node::Str("Bgm".to_string())), ("Volume".to_string(), Node::F32(0.6))]);
        let ducks: Node = Node::Map(vec![("fanfare playing".to_string(), Node::Map(vec![("Targets".to_string(), Node::Arr(vec![target]))]))]);
        Byml { root: Node::Map(vec![("GroupParam".to_string(), Node::Map(all)), ("ManualDucking".to_string(), ducks)]), version: 7, big: true }
    }

    fn volume(y: &Byml, g: &str) -> Option<Node> {
        y.root.get("GroupParam")?.get(g)?.get("Default")?.get("Volume").cloned()
    }

    #[test]
    fn the_music_and_fanfare_groups_are_muted_and_a_missing_group_is_refused() {
        let others: [&str; 3] = ["OcarinaPlaying", "BgmActorPlaying", SCENE];
        let mut y: Byml = groups(&[&QUIET[..], &["FanfareDefault"], &others[..]].concat());
        mute(&mut y).unwrap();
        for g in QUIET.iter().chain(["FanfareDefault"].iter()) {
            assert_eq!(volume(&y, g), Some(Node::F32(0.0)), "{g}");
        }
        for g in others {
            assert_eq!(volume(&y, g), Some(Node::F32(1.0)), "{g}");
        }
        assert!(mute(&mut groups(&[&QUIET[1..], &["FanfareDefault"]].concat())).is_err());
        assert!(mute(&mut groups(&QUIET)).is_err(), "no group ducks the music");
    }

    #[test]
    fn scene_music_is_every_bgm_track() {
        assert!(is_scene_music("BGM_Shop_Desert"));
        assert!(is_scene_music("BGM_SleepInBed"));
        assert!(is_scene_music("BGM_Dm_TimeTemple_0030_Jingle"));
        assert!(!is_scene_music("ExampleMelody_SunSong"));
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
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        apply(&out, &mut rf).unwrap();
        let read = |rf: &mut Romfs, rel: &str| -> Vec<u8> { rf.zs.decompress(&fs::read(out.join("romfs").join(rel)).unwrap()).unwrap() };

        let quiet: Vec<String> = muted(&rf.read_byml(GROUP_FILE).unwrap());
        assert_eq!(quiet.len(), QUIET.len() + 1, "{quiet:?}");
        let was: Vec<u8> = rf.read_zs(SLINK).unwrap();
        let now: Vec<u8> = read(&mut rf, SLINK);
        assert_eq!(was.len(), now.len());
        let before: BM<(u32, String), (String, String)> = calls(&Link::read(&was).unwrap(), &was);
        let after: BM<(u32, String), (String, String)> = calls(&Link::read(&now).unwrap(), &now);
        assert_eq!(before.len(), after.len());
        let audible_before: BS<&String> = before.values().filter(|(a, g)| a.starts_with("BGM_") && !QUIET.contains(&g.as_str())).map(|x| &x.0).collect();
        let mut moved: usize = 0;
        let mut fanfares: BS<&String> = BS::new();
        let mut audible: BS<&String> = BS::new();
        for (k, (asset, group)) in &after {
            let (a, g): &(String, String) = &before[k];
            assert_eq!(a, asset);
            if g != group {
                assert!(g == SCENE && group == AREA && is_scene_music(asset), "{asset} {g} {group}");
                moved += 1;
            }
            let silent: bool = quiet.contains(group);
            if !QUIET.contains(&group.as_str()) && silent {
                fanfares.insert(asset);
            }
            if asset.starts_with("BGM_") {
                assert!(silent || group == "BgmActorPlaying", "{asset} stays audible in {group}");
                if !silent {
                    audible.insert(asset);
                }
            } else {
                assert!(!silent, "{asset} would be silenced in {group}");
            }
        }
        println!("BGM_ tracks outside the four music groups in the game's files: {} {audible_before:?}", audible_before.len());
        println!("fanfares silenced by their ducking group: {} {fanfares:?}", fanfares.len());
        println!("BGM_ tracks still heard: {audible:?}");
        assert_eq!(fanfares.len(), 12);
        assert_eq!(audible.into_iter().cloned().collect::<Vec<String>>(), vec!["BGM_LonLonRanch_Vocal_Adult".to_string()]);
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
