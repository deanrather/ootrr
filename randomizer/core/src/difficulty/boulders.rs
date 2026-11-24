use crate::byml::{Byml, Node};
use crate::difficulty::ambush::{actor, fresh, list, text};
use crate::error::Res;
use crate::flow_locations::dict_id;
use crate::placement::hash_of;
use crate::romfs::Romfs;
use crate::qol::Have;
use crate::staging::{Out, BANC_DICT};
use crate::zs;

pub const SPAWNER: &str = "RollingRockSpawnActor_DeathMountainTrail";
pub const KEESE: &str = "Enemy_Keese_Fire";
pub const SEED: &str = "ootrr more boulders";
pub const FIELD: &str = "SpawnTime";
pub const MAPS: [&str; 2] = ["DeathMountainTrail_Child", "DeathMountainTrail_Adult"];

pub const NOTE: &str = "Twice as many boulders roll down the adult Death Mountain Trail, and six Fire Keese flutter over the trail, as a child and as an adult.";

pub const PERCHES: [[f32; 3]; 6] = [
    [-44.3, 113.7, -17.6],
    [-10.6, 121.5, -53.2],
    [-35.75, 125.0, 6.25],
    [-23.7, 127.0, -14.7],
    [-31.3, 94.0, 65.3],
    [3.7, 202.0, -123.4],
];

pub fn file(map: &str, kind: &str) -> String {
    format!("Banc/DeathMountainTrail/{map}_{kind}.bcett.byml.zs")
}

pub fn halve(y: &mut Byml) -> usize {
    let mut n: usize = 0;
    let acts: &mut Vec<Node> = match y.root.get_mut("Actors").and_then(|a| a.as_arr_mut()) {
        Some(a) => a,
        None => return 0,
    };
    for a in acts.iter_mut() {
        if !text(a, "Gyaml").starts_with(SPAWNER) {
            continue;
        }
        if let Some(Node::F32(t)) = a.get_mut("Dynamic").and_then(|d| d.get_mut(FIELD)) {
            *t /= 2.0;
            n += 1;
        }
    }
    n
}

pub fn place(y: &mut Byml, raws: &[&[u8]], map: &str) -> Res<usize> {
    let mut taken: Vec<u64> = Vec::new();
    for a in y.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]) {
        taken.extend(hash_of(a));
    }
    let mut made: Vec<Node> = Vec::new();
    for (i, at) in PERCHES.iter().enumerate() {
        let mut n: u32 = 0;
        let h: u64 = loop {
            let h: u64 = fresh(raws, &format!("{SEED} {map} {i} {n}"));
            if !taken.contains(&h) {
                taken.push(h);
                break h;
            }
            n += 1;
        };
        made.push(actor(KEESE, h, *at, [0.0; 3]));
    }
    let count: usize = made.len();
    list(y, "Actors")?.extend(made);
    Ok(count)
}

pub fn stage(rf: &mut Romfs, have: Have) -> Res<Vec<Out>> {
    let mut outs: Vec<Out> = Vec::new();
    for map in MAPS {
        let path: String = file(map, "Static");
        let z: Vec<u8> = match have(&path) {
            Some(b) => b,
            None => rf.read(&path)?,
        };
        let dict: u32 = dict_id(&z).unwrap_or(BANC_DICT);
        let raw: Vec<u8> = rf.zs.decompress(&z)?;
        let other: String = file(map, "Dynamic");
        let land: Vec<u8> = match have(&other) {
            Some(b) => rf.zs.decompress(&b)?,
            None => rf.zs.decompress(&rf.read(&other)?)?,
        };
        let mut y: Byml = Byml::parse(&raw)?;
        halve(&mut y);
        place(&mut y, &[&raw, &land], map)?;
        let bytes: Vec<u8> = y.write()?;
        outs.push(Out { path, old: raw.len(), new: bytes.len(), z: zs::compress_raw(&bytes, Some(dict))? });
    }
    Ok(outs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump() -> Option<std::path::PathBuf> {
        std::env::var("OOTRR_ROMFS").ok().map(std::path::PathBuf::from).filter(|p| p.join("Banc").is_dir())
    }

    fn none(_: &str) -> Option<Vec<u8>> {
        None
    }

    fn times(y: &Byml) -> Vec<f32> {
        let mut v: Vec<f32> = Vec::new();
        for a in y.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]) {
            if let Some(Node::F32(t)) = a.get("Dynamic").and_then(|d| d.get(FIELD)) {
                if text(a, "Gyaml").starts_with(SPAWNER) {
                    v.push(*t);
                }
            }
        }
        v
    }

    #[test]
    fn perches_are_distinct_and_on_the_trail() {
        for (i, a) in PERCHES.iter().enumerate() {
            assert!(a[1] > 90.0 && a[1] < 210.0);
            for b in PERCHES.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn spawners_run_twice_as_often_and_keese_are_added() {
        let Some(root) = dump() else { return };
        let mut rf: Romfs = Romfs::open(&root).unwrap();
        let outs: Vec<Out> = stage(&mut rf, &none).unwrap();
        assert_eq!(outs.len(), 2);
        for (o, map) in outs.iter().zip(MAPS) {
            let was: Byml = Byml::parse(&rf.zs.decompress(&rf.read(&o.path).unwrap()).unwrap()).unwrap();
            let now: Byml = Byml::parse(&rf.zs.decompress(&o.z).unwrap()).unwrap();
            let before: Vec<f32> = times(&was);
            let after: Vec<f32> = times(&now);
            assert_eq!(before.len(), after.len());
            for (b, a) in before.iter().zip(&after) {
                assert_eq!(*a, b / 2.0);
            }
            let count = |y: &Byml| y.root.get("Actors").and_then(|a| a.as_arr()).unwrap_or(&[]).iter().filter(|a| text(a, "Gyaml") == KEESE).count();
            assert_eq!(count(&now), count(&was) + PERCHES.len(), "{map}");
            assert_eq!(before.len(), if map.ends_with("Adult") { 2 } else { 0 });
        }
    }
}
