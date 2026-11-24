use crate::bfevfl::flow::NONE;
use crate::bfevfl::{Bf, Cont, Entry, Event, Flow, Kind, Val};
use crate::byml::{Byml, Node};
use crate::chest_slot::LIST;
use crate::error::{Error, Res};
use crate::flags::hash_name;
use crate::msbt::{self, Msbt};
use crate::romfs::Romfs;
use crate::sarc;
use crate::settings::seed::SeedFile;
use crate::slice::data::Data;
use crate::staging::Out;
use crate::world::{Placement, World};

pub const MENU_PACK: &str = "Pack/Bootup.Nin_Ounce_NVN2.pack.zs";
pub const MENU_FILE: &str = "Game/UI/ChallengeMenu/ChallengeMenuTable/ChallengeMenuTable.game__ui__ChallengeMenuTable.bgyml";
pub const TABLE: &str = "RSDB/Challenge.Product.111.rstbl.byml.zs";
pub const MENU_OPEN: &str = "UI_IsFinishChallengeMenuOpenEvent";
pub const PARENT: &str = "OOTRR_DungeonKeys";
pub const BOOT_FLOW: &str = "DefaultBootEvent";
pub const UPDATE: &str = "OOTRRKeyUpdate";
const ROW_ID: &str = "__RowId";
const TEMPLATE_ROW: &str = "Work/Challenge/SubEvent_GetMelodySun.game__challenge__Challenge.gyml";
const TEMPLATE_FLAG: &str = "Step_SubEvent_GetMelodySun";
const TEMPLATE_TEXT: &str = "ChallengeMsg/Info_Meet_DekuTree.msbt";
const EXP_TEMPLATE: &str = "SageOfForest_IsCompleted_Exp";
const ANY_FLAG: &str = "IsGet.Weapon_Sword_Kokiri";
const ALWAYS: &str = "OOTRR_Keys_Always_Exp";
const DESCRIPTION: &str = "Total keys earned for this location.";
const SUMMARY: &str = "Small Keys and Boss Keys for each dungeon.";
const HEADING: &str = "Keys";
const MAIN: u32 = 460_120_899;
const DROPPED: [&str; 6] = ["DependFlagName", "GameSafetySetting", "RequestActor", "RequestLocation", "IsCompleteTarget", "IsListAdventureLog"];
const PARENT_PANEL: &str = "DoorOfTime";
const CHILD_PANEL: &str = "Get_StoneOfFire";
const OURS: &str = "OOTRR_";
const QUIET: &str = "IsNotChangeAdventureLog";
const STEP: &str = "Tracking";
const PREFIX: &str = "OOTRRKeys";
const CHECKLIST_KEYS: [&str; 3] = ["NextActionContent", "UseNextActionContentStep", "IsDisplayGameDataFalse"];
const OP_BOOL: u64 = 0;
const OP_OR: u64 = 4;
const OP_NOT: u64 = 5;

pub struct Dungeon {
    pub data: &'static str,
    pub title: &'static str,
    pub logic: &'static str,
    pub boss: bool,
}

const fn dg(data: &'static str, title: &'static str, logic: &'static str, boss: bool) -> Dungeon {
    Dungeon { data, title, logic, boss }
}

pub const DUNGEONS: [Dungeon; 8] = [
    dg("ForestTemple", "Forest Temple", "Forest Temple", true),
    dg("FireTemple", "Fire Temple", "Fire Temple", true),
    dg("WaterTemple", "Water Temple", "Water Temple", true),
    dg("ShadowTemple", "Shadow Temple", "Shadow Temple", true),
    dg("SpiritTemple", "Spirit Temple", "Spirit Temple", true),
    dg("BottomOfTheWell", "Bottom of the Well", "Bottom of the Well", false),
    dg("GerudoTrainingGround", "Gerudo Training Ground", "Gerudo Training Ground", false),
    dg("InsideGanonCastle", "Ganon's Castle", "Ganons Castle", true),
];

#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub caps: Vec<u32>,
}

pub fn start_ops() -> crate::start_state::Ops {
    crate::start_state::Ops { flags: vec![(MENU_OPEN.to_string(), true)], ..Default::default() }
}

fn no_boss(dn: &Dungeon) -> String {
    format!("OOTRR_Keys_{}_NoBoss_Exp", dn.data)
}

fn checklist(dn: &Dungeon) -> Vec<String> {
    let mut v: Vec<String> = vec![ALWAYS.to_string()];
    if dn.boss {
        v.push(key_flag(dn, "IsGetBossKey"));
        v.push(no_boss(dn));
    }
    v
}

pub fn child_id(dn: &Dungeon) -> String {
    format!("OOTRR_Keys_{}", dn.data)
}

fn key_flag(dn: &Dungeon, member: &str) -> String {
    crate::dungeon_credit::flag(dn.data, member)
}

fn number_tag(name: &str) -> Res<String> {
    let n: u16 = u16::try_from(name.len()).map_err(|_| Error::Format("key log: flag name too long"))?;
    let mut hex: String = String::new();
    for b in n.to_le_bytes().into_iter().chain(name.bytes()).chain([0xFF, 0xFF, 0x00]) {
        hex.push_str(&format!("{b:02x}"));
    }
    Ok(format!("{{2.1:{hex}}}"))
}

pub fn plan(d: &Data, seed: &SeedFile) -> Res<Plan> {
    let (w, pl): (World, Placement) = crate::slice::starts::rebuild(d, seed)?;
    let caps: Vec<u32> = DUNGEONS
        .iter()
        .map(|dn| {
            let n: usize = match w.item(&format!("Small Key ({})", dn.logic)) {
                Some(k) => w.real.iter().filter(|l| pl.get(**l) == Some(k)).count(),
                None => 0,
            };
            u32::try_from(n).unwrap_or(u32::MAX)
        })
        .collect();
    Ok(Plan { caps })
}

pub fn read_flags() -> Vec<String> {
    let mut v: Vec<String> = vec![ANY_FLAG.to_string()];
    for dn in &DUNGEONS {
        v.push(key_flag(dn, "SmallKeyTotalNum"));
        v.push(key_flag(dn, "SmallKeyNum"));
        if dn.boss {
            v.push(key_flag(dn, "IsGetBossKey"));
        }
    }
    v
}

struct Quest {
    id: String,
    texts: Vec<(String, String)>,
}

fn quests(p: &Plan) -> Res<Vec<Quest>> {
    let mut v: Vec<Quest> = vec![Quest {
        id: PARENT.to_string(),
        texts: vec![
            ("Name".to_string(), "Dungeon Keys".to_string()),
            ("Ready".to_string(), SUMMARY.to_string()),
            ("Ready_NextAction".to_string(), HEADING.to_string()),
            (STEP.to_string(), SUMMARY.to_string()),
            (format!("{STEP}_NextAction"), HEADING.to_string()),
            ("Complete".to_string(), SUMMARY.to_string()),
        ],
    }];
    for (dn, cap) in DUNGEONS.iter().zip(&p.caps) {
        let mut texts: Vec<(String, String)> = vec![
            ("Name".to_string(), dn.title.to_string()),
            ("Ready".to_string(), DESCRIPTION.to_string()),
            ("Ready_NextAction".to_string(), HEADING.to_string()),
            (STEP.to_string(), DESCRIPTION.to_string()),
            (format!("{STEP}_NextAction"), HEADING.to_string()),
            (format!("{STEP}_NextActionContent_00"), format!("Small Keys: {} ({}/{cap} Found)", number_tag(&key_flag(dn, "SmallKeyNum"))?, number_tag(&key_flag(dn, "SmallKeyTotalNum"))?)),
            ("Complete".to_string(), DESCRIPTION.to_string()),
        ];
        if dn.boss {
            texts.push((format!("{STEP}_NextActionContent_01"), "Boss Key: 1/1".to_string()));
            texts.push((format!("{STEP}_NextActionContent_02"), "Boss Key: 0/1".to_string()));
        }
        v.push(Quest { id: child_id(dn), texts });
    }
    Ok(v)
}

fn ids() -> Vec<String> {
    std::iter::once(PARENT.to_string()).chain(DUNGEONS.iter().map(child_id)).collect()
}

pub fn row_id(q: &str) -> String {
    format!("Work/Challenge/{q}.game__challenge__Challenge.gyml")
}

pub fn flag(q: &str) -> String {
    format!("Step_{q}")
}

pub fn text_file(q: &str) -> String {
    format!("ChallengeMsg/Info_{q}.msbt")
}

fn id_of(r: &Node) -> &str {
    r.get(ROW_ID).and_then(Node::as_str).unwrap_or("")
}

fn hash_of(r: &Node) -> Option<u32> {
    match r.get("Hash") {
        Some(Node::U32(h)) => Some(*h),
        _ => None,
    }
}

pub fn edit_table(y: &mut Byml) -> Res<usize> {
    let rows: &mut Vec<Node> = y.root.as_arr_mut().ok_or(Error::Format("key log: the quest table is not a list"))?;
    if rows.iter().any(|r| id_of(r) == row_id(PARENT)) {
        return Err(Error::Format("key log: the quest table already has the key quests"));
    }
    let base: Node = rows.iter().find(|r| id_of(r) == TEMPLATE_ROW).cloned().ok_or(Error::Format("key log: no template quest row"))?;
    let step: Node = base.get("Steps").and_then(Node::as_arr).and_then(|s| s.first()).cloned().ok_or(Error::Format("key log: the template quest has no steps"))?;
    let qs: Vec<String> = ids();
    for q in &qs {
        let mut r: Node = base.clone();
        for k in DROPPED {
            r.remove(k);
        }
        r.set(ROW_ID, Node::Str(row_id(q)));
        r.set("Category", Node::U32(MAIN));
        let steps: Vec<Node> = ["Ready", STEP]
            .iter()
            .map(|s| {
                let mut x: Node = step.clone();
                x.set("Name", Node::Str((*s).to_string()));
                x
            })
            .collect();
        r.set("Steps", Node::Arr(steps));
        rows.push(r);
    }
    Ok(qs.len())
}

/// The game pops a "next objective" card each time a quest step moves on, unless the step is
/// marked as not changing the adventure log (minigame and in-dungeon steps already are). Threads of
/// Time no longer lists the game's quests, so every game step gets the mark; our key quests keep
/// theirs off, their menu text follows the step and their requests already skip the card.
pub fn quiet_steps(y: &mut Byml) -> Res<usize> {
    let rows: &mut Vec<Node> = y.root.as_arr_mut().ok_or(Error::Format("key log: the quest table is not a list"))?;
    let ours: String = format!("Work/Challenge/{OURS}");
    let mut seen: usize = 0;
    let mut n: usize = 0;
    for r in rows.iter_mut().filter(|r| !id_of(r).starts_with(&ours)) {
        let Some(steps) = r.get_mut("Steps").and_then(Node::as_arr_mut) else { continue };
        for st in steps.iter_mut() {
            match st.get(QUIET) {
                Some(Node::Bool(true)) => seen += 1,
                Some(Node::Bool(false)) => {
                    st.set(QUIET, Node::Bool(true));
                    n += 1;
                }
                _ => return Err(Error::Format("key log: a quest step has no adventure log mark, the game files differ")),
            }
        }
    }
    if seen + n == 0 {
        return Err(Error::Format("key log: the quest table has no steps"));
    }
    Ok(n)
}

fn formula_names() -> Vec<String> {
    std::iter::once(ALWAYS.to_string()).chain(DUNGEONS.iter().filter(|d| d.boss).map(no_boss)).collect()
}

pub fn added_hashes() -> Vec<u32> {
    ids().iter().map(|q| hash_name(&flag(q))).chain(formula_names().iter().map(|n| hash_name(n))).collect()
}

fn tok(op: u64, a: Option<u64>) -> Node {
    Node::Arr(std::iter::once(Node::U64(op)).chain(a.map(Node::U64)).collect())
}

pub fn add_flags(y: &mut Byml) -> Res<usize> {
    let list: &mut Vec<Node> = y.root.get_mut("Data").and_then(|d| d.get_mut("Enum")).and_then(Node::as_arr_mut).ok_or(Error::Format("key log: the game data list has no Enum table"))?;
    let k: u32 = hash_name(TEMPLATE_FLAG);
    let base: Node = list.iter().find(|r| hash_of(r) == Some(k)).cloned().ok_or(Error::Format("key log: no template step flag"))?;
    let qs: Vec<String> = ids();
    let values: [&str; 4] = ["NotReady", "Ready", STEP, "Complete"];
    for q in &qs {
        let h: u32 = hash_name(&flag(q));
        if list.iter().any(|r| hash_of(r) == Some(h)) {
            return Err(Error::Msg(format!("key log: {} clashes with a game flag", flag(q))));
        }
        let mut r: Node = base.clone();
        r.set("Hash", Node::U32(h));
        r.set("DefaultValue", Node::U32(hash_name(values[0])));
        r.set("RawValues", Node::Arr(values.iter().map(|v| Node::Str((*v).to_string())).collect()));
        r.set("Values", Node::Arr(values.iter().map(|v| Node::U64(hash_name(v) as u64)).collect()));
        r.set("SaveFileIndex", Node::I32(-1));
        list.push(r);
    }
    let k: u32 = hash_name(EXP_TEMPLATE);
    let exps: &mut Vec<Node> = y.root.get_mut("Data").and_then(|d| d.get_mut("BoolExp")).and_then(Node::as_arr_mut).ok_or(Error::Format("key log: the game data list has no BoolExp table"))?;
    let base: Node = exps.iter().find(|r| hash_of(r) == Some(k)).cloned().ok_or(Error::Format("key log: no template formula"))?;
    let any: u64 = hash_name(ANY_FLAG) as u64;
    let mut formulas: Vec<(String, Vec<Node>)> = vec![(ALWAYS.to_string(), vec![tok(OP_BOOL, Some(any)), tok(OP_BOOL, Some(any)), tok(OP_NOT, None), tok(OP_OR, None)])];
    for dn in DUNGEONS.iter().filter(|d| d.boss) {
        formulas.push((no_boss(dn), vec![tok(OP_BOOL, Some(hash_name(&key_flag(dn, "IsGetBossKey")) as u64)), tok(OP_NOT, None)]));
    }
    for (name, terms) in &formulas {
        let h: u32 = hash_name(name);
        if exps.iter().any(|r| hash_of(r) == Some(h)) {
            return Err(Error::Msg(format!("key log: {name} clashes with a game formula")));
        }
        let mut r: Node = base.clone();
        r.set("Hash", Node::U32(h));
        r.set("Values", Node::Arr(terms.clone()));
        exps.push(r);
    }
    Ok(qs.len() + formulas.len())
}

pub fn add_text(raw: &[u8], p: &Plan) -> Res<Vec<u8>> {
    let mut es: Vec<sarc::Entry> = sarc::parse(raw)?;
    let base: Vec<u8> = sarc::get(&es, TEMPLATE_TEXT).map(|e| e.data.clone()).ok_or(Error::Format("key log: no template quest text"))?;
    for q in &quests(p)? {
        let mut m: Msbt = msbt::parse(&base)?;
        for (l, s) in &q.texts {
            m.put_markup(l, s)?;
        }
        sarc::set(&mut es, &text_file(&q.id), m.write()?);
    }
    sarc::write(&es)
}

fn panel_name(r: &Node) -> &str {
    r.get("ChallengeShort").and_then(Node::as_str).unwrap_or("")
}

pub fn edit_menu(y: &mut Byml) -> Res<usize> {
    let rows: &mut Vec<Node> = y.root.get_mut("ChallengeMenuParamArray").and_then(Node::as_arr_mut).ok_or(Error::Format("key log: the menu table has no panel list"))?;
    let mut parent: Node = rows.iter().find(|r| panel_name(r) == PARENT_PANEL).cloned().ok_or(Error::Format("key log: no Door of Time panel"))?;
    let child: Node = rows.iter().find(|r| panel_name(r) == CHILD_PANEL).cloned().ok_or(Error::Format("key log: no Fire Stone panel"))?;
    parent.set("ChallengeShort", Node::Str(PARENT.to_string()));
    for k in CHECKLIST_KEYS {
        parent.remove(k);
    }
    let mut group: Vec<Node> = vec![parent];
    for dn in &DUNGEONS {
        let mut x: Node = child.clone();
        x.set("ChallengeShort", Node::Str(child_id(dn)));
        for k in CHECKLIST_KEYS {
            x.remove(k);
        }
        x.set("NextActionContent", Node::Arr(checklist(dn).into_iter().map(Node::Str).collect()));
        x.set("UseNextActionContentStep", Node::Arr(vec![Node::Str(STEP.to_string())]));
        group.push(x);
    }
    rows.retain(|r| panel_name(r).starts_with(OURS));
    let n: usize = group.len();
    rows.splice(0..0, group);
    Ok(n)
}

pub fn stage_menu(rf: &mut Romfs, more: &mut Vec<Out>) -> Res<()> {
    let zc: Vec<u8> = rf.read(MENU_PACK)?;
    let dict: Option<u32> = crate::zs::dict_of(&zc);
    let raw: Vec<u8> = rf.zs.decompress(&zc)?;
    let mut es: Vec<sarc::Entry> = sarc::parse(&raw)?;
    let e: &sarc::Entry = sarc::get(&es, MENU_FILE).ok_or(Error::Format("key log: the boot pack has no menu table"))?;
    let mut y: Byml = Byml::parse(&e.data)?;
    edit_menu(&mut y)?;
    sarc::set(&mut es, MENU_FILE, y.write()?);
    let out: Vec<u8> = sarc::write(&es)?;
    more.push(Out { path: MENU_PACK.to_string(), z: crate::zs::compress_raw(&out, dict)?, old: raw.len(), new: out.len() });
    Ok(())
}

pub fn stage(rf: &mut Romfs, more: &mut Vec<Out>, p: &Plan) -> Res<()> {
    stage_menu(rf, more)?;
    let l: crate::flags::FlagList = crate::flags_build::load(rf)?;
    let missing: Vec<String> = read_flags().into_iter().filter(|f| l.by_hash(hash_name(f) as u64).is_none()).collect();
    if !missing.is_empty() {
        return Err(Error::Msg(format!("key log: the game has no flag {}", missing.join(", "))));
    }
    crate::dungeon_grant::merge(rf, more, LIST, |raw| {
        let mut y: Byml = Byml::parse(raw)?;
        add_flags(&mut y)?;
        y.write()
    })?;
    for lang in crate::hints::languages(rf)? {
        crate::dungeon_grant::merge(rf, more, &lang, |raw| add_text(raw, p))?;
    }
    Ok(())
}

pub fn stage_table(rf: &mut Romfs, outs: &mut Vec<Out>) -> Res<()> {
    crate::dungeon_grant::merge(rf, outs, TABLE, |raw| {
        let mut y: Byml = Byml::parse(raw)?;
        edit_table(&mut y)?;
        quiet_steps(&mut y)?;
        y.write()
    })
}

fn idx(n: usize) -> Res<u16> {
    u16::try_from(n).ok().filter(|v| *v != NONE).ok_or(Error::Format("key log: the flow has too many events"))
}

fn push(f: &mut Flow, k: Kind) -> Res<u16> {
    let i: u16 = idx(f.events.len())?;
    f.events.push(Event { name: format!("{PREFIX}{i}"), kind: k });
    Ok(i)
}

fn ask_step(q: &str, s: &str) -> Cont {
    Cont {
        items: vec![
            ("ChallengeName".to_string(), Val::Str(q.to_string())),
            ("StepName".to_string(), Val::Str(s.to_string())),
            ("IsIgnoreCompletedChallenge".to_string(), Val::Bool(1)),
            ("IsIgnoreChallengeProgressUI".to_string(), Val::Bool(1)),
            ("IsWaitEnd".to_string(), Val::Bool(1)),
        ],
    }
}

pub fn add_update(b: &mut Bf) -> Res<u16> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == BOOT_FLOW).ok_or(Error::Format("key log: DefaultBootEvent has no flow"))?;
    if f.entries.iter().any(|e| e.name == UPDATE) {
        return Err(Error::Format("key log: DefaultBootEvent already has the update"));
    }
    let pad: u16 = f.entries.first().map(|e| e.pad).unwrap_or(0);
    let (a, x): (u16, u16) = crate::start_state::slot(f, "ChallengeSystemActor", "EventExecuteProgressChallengeStep")?;
    let mut next: u16 = NONE;
    for q in ids().iter().rev() {
        next = push(f, Kind::Action { next, actor: a, act: x, params: Some(ask_step(q, STEP)) })?;
    }
    let main: u16 = next;
    f.entries.push(Entry { name: UPDATE.to_string(), subs: Vec::new(), vars: Vec::new(), main, pad });
    let boot: usize = f.entries.iter().position(|e| e.name == BOOT_FLOW).ok_or(Error::Format("key log: DefaultBootEvent has no boot entry"))?;
    let old: u16 = f.entries[boot].main;
    let call: u16 = push(f, Kind::Sub { next: old, params: None, flow: String::new(), entry: UPDATE.to_string() })?;
    f.entries[boot].main = call;
    f.entries[boot].subs.push(call);
    Ok(main)
}

pub fn hook_opening(b: &mut Bf) -> Res<()> {
    let f: &mut Flow = b.flows.iter_mut().find(|f| f.name == crate::start_state::FLOW).ok_or(Error::Format("key log: no OpeningEvent flow"))?;
    let asks = |e: &Event| -> bool { matches!(&e.kind, Kind::Action { params: Some(p), .. } if p.items.iter().any(|(k, v)| k == "ChallengeName" && *v == Val::Str(crate::start_state::ANCHOR_CHALLENGE.to_string()))) };
    let mine: Vec<usize> = (0..f.events.len()).filter(|&i| !f.events[i].name.starts_with(crate::start_state::PREFIX) && asks(&f.events[i])).collect();
    let [anchor] = mine.as_slice() else {
        return Err(Error::Msg(format!("key log: expected one Deku Tree quest request in the opening, found {}", mine.len())));
    };
    let mut at: usize = *anchor;
    let mut seen: usize = 0;
    while let Kind::Action { next, .. } = &f.events[at].kind {
        let n: usize = usize::from(*next);
        if *next == NONE || n >= f.events.len() || !f.events[n].name.starts_with(crate::start_state::PREFIX) || seen > f.events.len() {
            break;
        }
        at = n;
        seen += 1;
    }
    let Kind::Action { next: old, .. } = f.events[at].kind else {
        return Err(Error::Format("key log: the opening hook point is not an action"));
    };
    let call: u16 = push(f, Kind::Sub { next: old, params: None, flow: BOOT_FLOW.to_string(), entry: UPDATE.to_string() })?;
    if let Kind::Action { next, .. } = &mut f.events[at].kind {
        *next = call;
    }
    for e in f.entries.iter_mut() {
        if !e.subs.contains(&call) {
            e.subs.push(call);
        }
    }
    Ok(())
}

pub fn stage_flows(rf: &mut Romfs, late: &mut Vec<Out>, work: &std::path::Path) -> Res<()> {
    crate::story_skip::stage_late(rf, late, work, BOOT_FLOW, |b| add_update(b).map(|_| ()))?;
    crate::story_skip::stage_late(rf, late, work, crate::start_state::FLOW, hook_opening)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real() -> Option<Romfs> {
        let p: std::path::PathBuf = ootrr_game::test_romfs::path();
        if p.join("Pack").join("ZsDic.pack.zs").is_file() {
            Romfs::open(&p).ok()
        } else {
            eprintln!("skipped, no RomFS dump at {} (set OOTRR_ROMFS)", p.display());
            None
        }
    }

    fn quiet(r: &Node) -> Vec<bool> {
        r.get("Steps").and_then(Node::as_arr).unwrap_or(&[]).iter().map(|s| s.get(QUIET) == Some(&Node::Bool(true))).collect()
    }

    #[test]
    fn every_game_quest_step_stops_the_next_objective_card_and_ours_stay() {
        let Some(mut rf) = real() else { return };
        let mut y: Byml = rf.read_byml(TABLE).unwrap();
        let before: usize = y.root.as_arr().unwrap().len();
        edit_table(&mut y).unwrap();
        assert!(quiet_steps(&mut y).unwrap() > 100);
        let rows: &[Node] = y.root.as_arr().unwrap();
        assert_eq!(rows.len(), before + ids().len());
        for r in rows {
            let q: Vec<bool> = quiet(r);
            assert!(!q.is_empty(), "{} has no steps", id_of(r));
            if id_of(r).starts_with(&format!("Work/Challenge/{OURS}")) {
                assert!(q.iter().all(|b| !b), "{} was quieted", id_of(r));
            } else {
                assert!(q.iter().all(|b| *b), "{} still shows the card", id_of(r));
            }
        }
        assert_eq!(quiet_steps(&mut y).unwrap(), 0);
    }
}
