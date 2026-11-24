use crate::byml::{Byml, Node};
use crate::difficulty::stats;
use crate::error::{Error, Res};

pub const SETTING: &str = "enemy_aggression";
pub const NUM: u16 = 160;
pub const NORMAL: i64 = 100;
pub const PEACEFUL: i64 = 0;
pub const RANGE: (i64, i64) = (0, 300);
pub const STEP: i64 = 50;
pub const CLIENT_DIR: &str = "Component/GameBalanceClientParam/";
pub const CLIENT_EXT: &str = ".game__component__GameBalanceClientParam.bgyml";
pub const SENSE_DIR: &str = "Component/PerceptionReceiverParam/";
pub const SENSE_EXT: &str = ".game__component__PerceptionReceiverParam.bgyml";
pub const SHARED: [&str; 2] = ["Default", "EnemyCommon"];
pub const PAUSES: [&str; 2] = ["GlobalPlayerNoAttackTime", "GlobalPlayerNoAttackTimePlay"];
pub const SENSES: [&str; 7] = ["IsEnabledByDefault", "IsForceSystemRange", "UseHearing", "UseIntuition", "UseSensor", "UseSight", "UseTerror"];
pub const BOSSES: [&str; 4] = ["DungeonBoss", "MiddleBoss", "BattleManager", "KotakeKoume"];
pub const AWAKE: [&str; 16] = [
    "Enemy_Scrub_Business",
    "Enemy_Scrub_ForestStage",
    "Enemy_Scrub_Deku_Brothers",
    "Enemy_Amos",
    "Enemy_IronKnuckle",
    "Enemy_DeadHand",
    "Enemy_SkullKid",
    "Enemy_Skulltula_Gold",
    "Enemy_Poe_Amy",
    "Enemy_Poe_Beth",
    "Enemy_Poe_Joelle",
    "Enemy_Poe_Meg",
    "Enemy_Poe_Sisters",
    "Enemy_Poe_Hyrule",
    "Enemy_Poe_Big",
    "Enemy_Guay_Boss",
];

pub fn is_boss(stem: &str) -> bool {
    BOSSES.iter().any(|b| stem.contains(b))
}

pub fn scales(stem: &str) -> bool {
    !is_boss(stem) && (stats::is_enemy(stem) || SHARED.contains(&stem) || stem == "Gohma_Larva")
}

pub fn calms(stem: &str) -> bool {
    stats::is_enemy(stem) && !is_boss(stem) && !AWAKE.iter().any(|a| stem.starts_with(a))
}

pub fn pace_pct(stem: &str, speed: i64, aggression: i64) -> i64 {
    let s: i64 = if stats::is_enemy(stem) { speed } else { NORMAL };
    let a: i64 = if aggression > PEACEFUL && scales(stem) { aggression } else { NORMAL };
    s * a / NORMAL
}

fn same_size(y: &Byml, raw: &[u8]) -> Res<Vec<u8>> {
    let out: Vec<u8> = y.write()?;
    if out.len() != raw.len() {
        return Err(Error::Format("enemy aggression: an edited parameter file changed size"));
    }
    Ok(out)
}

pub fn pauses(raw: &[u8], aggression: i64) -> Res<Option<Vec<u8>>> {
    if aggression <= PEACEFUL || aggression == NORMAL {
        return Ok(None);
    }
    let mut y: Byml = Byml::parse(raw)?;
    let f: f32 = aggression as f32 / NORMAL as f32;
    let mut hit: bool = false;
    for k in PAUSES {
        if let Some(Node::F32(x)) = y.root.get(k).cloned()
            && x > 0.0
        {
            y.root.set(k, Node::F32(x / f));
            hit = true;
        }
    }
    if !hit {
        return Ok(None);
    }
    Ok(Some(same_size(&y, raw)?))
}

pub fn calm(raw: &[u8]) -> Res<Option<Vec<u8>>> {
    let mut y: Byml = Byml::parse(raw)?;
    let mut hit: bool = false;
    for k in SENSES {
        if let Some(Node::Bool(true)) = y.root.get(k) {
            y.root.set(k, Node::Bool(false));
            hit = true;
        }
    }
    if !hit {
        return Ok(None);
    }
    Ok(Some(same_size(&y, raw)?))
}

pub fn note(aggression: i64) -> Option<String> {
    if aggression == NORMAL {
        return None;
    }
    if aggression == PEACEFUL {
        return Some("Enemies are peaceful: ordinary enemies never notice Link, so they wait or wander instead of attacking. Bosses, minibosses and the enemies a check or a puzzle needs awake (Business Scrubs, the Deku Scrubs of the Deku Salesmen and the forest stage, Poe sisters, Big Poes, Armos, Iron Knuckles, Dead Hand, Skull Kid, Gold Skulltulas) behave as usual. Experimental: not played in the game yet.".to_string());
    }
    let t: String = format!("{:.2}", aggression as f64 / NORMAL as f64);
    let t: &str = t.trim_end_matches('0').trim_end_matches('.');
    Some(format!("The pauses between enemy attacks are divided by {t}, so enemies attack about {t} times as often. This uses the game's own attack timing, which Stalfos, Dinolfos, Floormasters, ReDeads, Gibdos, Poes, Stalchildren, Stingers, Torch Slugs, Spikes and the enemies on the common timing follow; enemies that attack as soon as they can (Keese, Deku Babas, Dodongos) do not change. Bosses and minibosses keep their own timing. Stacks with Enemy speed. Experimental: not played in the game yet."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(kv: Vec<(&str, Node)>) -> Vec<u8> {
        let y: Byml = Byml { root: Node::Map(kv.into_iter().map(|(k, v)| (k.to_string(), v)).collect()), version: 7, big: false };
        y.write().unwrap()
    }

    #[test]
    fn bosses_and_needed_enemies_stay_awake() {
        assert!(calms("Enemy_Stalfos"));
        assert!(calms("Keese"));
        assert!(calms("Enemy_Poe"));
        assert!(calms("Enemy_Wolfos_White"));
        assert!(!calms("Enemy_DungeonBoss_Gohma"));
        assert!(!calms("Enemy_MiddleBoss_DarkLink"));
        assert!(calms("Enemy_Scrub_Mad"));
        assert!(calms("Enemy_Scrub_Deku"));
        assert!(!calms("Enemy_Scrub_Business"));
        assert!(!calms("Enemy_Scrub_ForestStage_Elder"));
        assert!(!calms("Enemy_Scrub_Deku_Brothers"));
        assert!(!calms("Enemy_Poe_Joelle"));
        assert!(!calms("Enemy_IronKnuckle_White"));
        assert!(!calms("Enemy_Skulltula_Gold"));
        assert!(!calms("EnemyBaseOnlySystem"));
        assert!(!calms("Animal_Cucco"));
    }

    #[test]
    fn the_shared_timing_scales_and_boss_timing_does_not() {
        assert!(scales("Default"));
        assert!(scales("EnemyCommon"));
        assert!(scales("Keese"));
        assert!(scales("Gohma_Larva"));
        assert!(scales("Enemy_Stalfos"));
        assert!(!scales("Enemy_DungeonBoss_Ganon"));
        assert!(!scales("Enemy_MiddleBoss_GerudoThief"));
        assert!(!scales("Npc_Talon"));
    }

    #[test]
    fn speed_and_aggression_multiply() {
        assert_eq!(pace_pct("Enemy_Stalfos", 100, 200), 200);
        assert_eq!(pace_pct("Enemy_Stalfos", 150, 200), 300);
        assert_eq!(pace_pct("Enemy_Stalfos", 150, 100), 150);
        assert_eq!(pace_pct("Default", 150, 50), 50);
        assert_eq!(pace_pct("Enemy_DungeonBoss_Ganon", 150, 300), 150);
        assert_eq!(pace_pct("Enemy_Stalfos", 100, 0), 100);
    }

    #[test]
    fn pauses_divide_and_keep_size() {
        let raw: Vec<u8> = file(vec![("AttackIntervalRef", Node::Str("x".to_string())), ("GlobalPlayerNoAttackTime", Node::F32(80.0)), ("GlobalPlayerNoAttackTimePlay", Node::F32(90.0))]);
        let out: Vec<u8> = pauses(&raw, 200).unwrap().unwrap();
        let y: Byml = Byml::parse(&out).unwrap();
        assert_eq!(y.root.get("GlobalPlayerNoAttackTime"), Some(&Node::F32(40.0)));
        assert_eq!(y.root.get("GlobalPlayerNoAttackTimePlay"), Some(&Node::F32(45.0)));
        assert_eq!(out.len(), raw.len());
        let half: Byml = Byml::parse(&pauses(&raw, 50).unwrap().unwrap()).unwrap();
        assert_eq!(half.root.get("GlobalPlayerNoAttackTime"), Some(&Node::F32(160.0)));
        assert!(pauses(&raw, 100).unwrap().is_none());
        assert!(pauses(&raw, 0).unwrap().is_none());
        let zero: Vec<u8> = file(vec![("GlobalPlayerNoAttackTime", Node::F32(0.0))]);
        assert!(pauses(&zero, 300).unwrap().is_none());
    }

    #[test]
    fn calm_turns_every_sense_off() {
        let raw: Vec<u8> = file(vec![("ForceSystemRange", Node::F32(31.0)), ("IsEnabledByDefault", Node::Bool(true)), ("IsForceSystemRange", Node::Bool(true)), ("UseHearing", Node::Bool(false)), ("UseIntuition", Node::Bool(true)), ("UseSight", Node::Bool(true))]);
        let out: Vec<u8> = calm(&raw).unwrap().unwrap();
        let y: Byml = Byml::parse(&out).unwrap();
        for k in ["IsEnabledByDefault", "IsForceSystemRange", "UseHearing", "UseIntuition", "UseSight"] {
            assert_eq!(y.root.get(k), Some(&Node::Bool(false)), "{k}");
        }
        assert_eq!(y.root.get("ForceSystemRange"), Some(&Node::F32(31.0)));
        assert_eq!(out.len(), raw.len());
        assert!(calm(&out).unwrap().is_none());
    }

    #[test]
    fn notes_name_the_setting() {
        assert!(note(100).is_none());
        assert!(note(0).unwrap().starts_with("Enemies are peaceful"));
        assert!(note(150).unwrap().contains("1.5 times"));
        assert!(note(300).unwrap().contains("3 times"));
    }

    fn dump() -> Option<std::path::PathBuf> {
        let p: std::path::PathBuf = std::env::var_os("OOTRR_ROMFS").map(std::path::PathBuf::from)?;
        if p.is_dir() { Some(p) } else { None }
    }

    fn edited(rf: &mut crate::romfs::Romfs, pack: &str, o: &crate::difficulty::Opts) -> (Vec<crate::sarc::Entry>, Option<Vec<crate::sarc::Entry>>) {
        let raw: Vec<u8> = rf.read_zs(&format!("Pack/Actor/{pack}.pack.zs")).unwrap();
        let was: Vec<crate::sarc::Entry> = crate::sarc::parse(&raw).unwrap();
        let now: Option<Vec<crate::sarc::Entry>> = crate::difficulty::edit_pack(&raw, o).unwrap().map(|b| crate::sarc::parse(&b).unwrap());
        (was, now)
    }

    fn value(es: &[crate::sarc::Entry], name: &str, key: &str) -> Node {
        let e: &crate::sarc::Entry = es.iter().find(|e| e.name == name).unwrap_or_else(|| panic!("no {name}"));
        Byml::parse(&e.data).unwrap().root.get(key).cloned().unwrap_or_else(|| panic!("no {key} in {name}"))
    }

    #[test]
    fn the_game_packs_take_the_setting() {
        let Some(root) = dump() else {
            eprintln!("skipped: set OOTRR_ROMFS");
            return;
        };
        let mut rf: crate::romfs::Romfs = crate::romfs::Romfs::open(&root).unwrap();
        let twice: crate::difficulty::Opts = crate::difficulty::Opts { aggression: 200, ..crate::difficulty::Opts::default() };
        let calm_o: crate::difficulty::Opts = crate::difficulty::Opts { aggression: PEACEFUL, ..crate::difficulty::Opts::default() };
        let interval: &str = "GameBalance/AttackInterval/Enemy_Stalfos.game__gamebalance__AttackInterval.bgyml";
        let shared: &str = "Component/GameBalanceClientParam/Default.game__component__GameBalanceClientParam.bgyml";
        let sense: &str = "Component/PerceptionReceiverParam/Enemy_Stalfos.game__component__PerceptionReceiverParam.bgyml";
        let (was, now) = edited(&mut rf, "Enemy_Stalfos", &twice);
        let now: Vec<crate::sarc::Entry> = now.unwrap();
        for k in ["ShortIntervalMin", "ShortIntervalMax", "LongIntervalMax"] {
            let (Node::F32(a), Node::F32(b)) = (value(&was, interval, k), value(&now, interval, k)) else { panic!("{k} is not a float") };
            assert_eq!(b, a / 2.0, "{k}");
        }
        assert_eq!(value(&now, shared, "GlobalPlayerNoAttackTime"), Node::F32(40.0));
        assert_eq!(value(&now, sense, "IsEnabledByDefault"), Node::Bool(true));
        let (_, calmed) = edited(&mut rf, "Enemy_Stalfos", &calm_o);
        let calmed: Vec<crate::sarc::Entry> = calmed.unwrap();
        assert_eq!(value(&calmed, sense, "IsEnabledByDefault"), Node::Bool(false));
        assert_eq!(value(&calmed, sense, "IsForceSystemRange"), Node::Bool(false));
        assert_eq!(value(&calmed, interval, "ShortIntervalMin"), value(&was, interval, "ShortIntervalMin"));
        let (_, keese) = edited(&mut rf, "Enemy_Keese", &calm_o);
        assert_eq!(value(&keese.unwrap(), "Component/PerceptionReceiverParam/Keese.game__component__PerceptionReceiverParam.bgyml", "UseSight"), Node::Bool(false));
        for awake in ["Enemy_Scrub_Business", "Enemy_IronKnuckle_White", "Enemy_Poe_Joelle", "Enemy_DungeonBoss_Gohma", "Enemy_MiddleBoss_DarkLink"] {
            let (was, now) = edited(&mut rf, awake, &calm_o);
            let Some(now) = now else { continue };
            for (a, b) in was.iter().zip(now.iter()) {
                if a.name.starts_with(SENSE_DIR) && !a.name.contains("/Keese.") {
                    assert_eq!(a.data, b.data, "{awake} {} changed", a.name);
                }
            }
        }
        let (was, now) = edited(&mut rf, "Enemy_DungeonBoss_Gohma", &twice);
        let boss: &str = "GameBalance/AttackInterval/Enemy_DungeonBoss_Gohma.game__gamebalance__AttackInterval.bgyml";
        if let Some(now) = now {
            assert_eq!(value(&now, boss, "ShortIntervalMin"), value(&was, boss, "ShortIntervalMin"));
        }
    }
}
