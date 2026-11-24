use std::fs;
use std::path::PathBuf as PB;

use crate::ainb::{audit, Ainb};
use crate::byml::{Byml, Node};
use crate::cow_clone::rows::heap;
use crate::difficulty::ambush::{actor, fresh};
use crate::difficulty::hook::SWITCH;
use crate::difficulty::respawn;
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::modpack::Pack;
use crate::pickup_pack::rows::{self, ACTOR_INFO, GAME_ACTOR_INFO, HEAP, ROW_ID};
use crate::pickup_pack::sizes::{file_size, pack_size, FILE_MARGIN};
use crate::pickup_pack::tag::{Table, TAG};
use crate::pickup_pack::PACK_DICT;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst};
use crate::sarc::{self, Entry};
use crate::staging::{grow_out, Out, BANC_DICT};
use crate::zs;

pub const BASE: &str = "Enemy_MiddleBoss_GerudoThief";
pub const CLASS: &str = "Enemy_Gerudo_Fighter";
pub const EVENTS: &str = "EventStartParameters";
pub const EVENT_NAME: &str = "EventName";
pub const DREAM_NAME: &str = "EventNameInDreamWorld";
pub const KEYS: [&str; 2] = ["Arrest", "Death"];
pub const SHOW: &str = "?Component/BattlePresentationParam/EnemyCommon.game__component__BattlePresentationParam.bgyml";
pub const DEATH_KEY: &str = "DeathRef";
pub const DROP_KEY: &str = "DropRef";
pub const DEATH_BASE: &str = "Component/DeathParam/Enemy_MiddleBoss_GerudoThief.game__component__DeathParam.bgyml";
pub const DEATH_CLONE: &str = "Component/DeathParam/Enemy_Gerudo_Fighter.game__component__DeathParam.bgyml";
pub const PROCEDURES: &str = "DeathProcedures";
pub const DESTROY: &str = "IsRequestDestruction";
pub const WAIT: &str = "IsWaitForSignal";
pub const DROP: &str = "IsDropItem";
pub const CELL_COST: u64 = 256;
pub const ENABLED: [&str; 11] = ["AncientWeapon", "ChemicalCriticalHitFire", "Crushed", "Drowned", "FallIntoAbyss", "Frozen", "LifeCondition", "LifeDepleted", "PhysicsKinematicCollision", "SideCrushed", "UpperCrushed"];
pub const FROM: &str = "Enemy_Floormaster";
pub const DROP_PARAM_BASE: &str = "Component/DropParam/Enemy_MiddleBoss_GerudoThief.game__component__DropParam.bgyml";
pub const DROP_PARAM_CLONE: &str = "Component/DropParam/Enemy_Gerudo_Fighter.game__component__DropParam.bgyml";
pub const DROP_TABLE_NONE: &str = "Drop/DropTable/DropTable_None.game__drop__DropTable.bgyml";
pub const DROP_SUB_NONE: &str = "Drop/DropSubTable/DropTable_None.game__drop__DropSubTable.bgyml";
pub const DROP_TABLE_CLONE: &str = "Drop/DropTable/Enemy_Gerudo_Fighter.game__drop__DropTable.bgyml";
pub const DROP_SUB_CLONE: &str = "Drop/DropSubTable/Enemy_Gerudo_Fighter.game__drop__DropSubTable.bgyml";
pub const RUPEE: &str = "Obj_Rupee_Red";
pub const RUPEE_COUNT: u32 = 1;
pub const RUPEE_TYPE: u32 = 995_674_644;
pub const SPIN: &str = "AI/Enemy_MiddleBoss_GerudoThief.action.LargeAttack.module.ainb";
pub const ARREST: &str = "IsRequestArrestEvent";
pub const SET_BOOL: &str = "ExecuteBlackboardSetBool";
pub const IDLE: &str = "ExecuteGenericDummyAction";
pub const KEEP_AI: bool = false;
pub const TEST_SPOT: bool = false;
pub const NO_RECORD: &str = "IsSetBoolGameDataRecordingDeath";
pub const SEED: &str = "ootrr gerudo fighter";
pub const TEST_FILE: &str = "Banc/Market/Market_Child_Static.bcett.byml.zs";
pub const TEST_AT: [f32; 3] = [92.0, 104.1, -34.0];
pub const NOTE: &str = "Gerudo fighters (part of More enemies): a copy of the Gerudo Thief miniboss of the fortress under its own actor name (Enemy_Gerudo_Fighter) can be placed almost anywhere: it has no talk or defeat scene, a spin hit that would have sent you to the fortress jail only hurts, it plays the ordinary enemy music, and it dies like an ordinary enemy and drops a red rupee. With More enemies at 500 % or more it joins the minibosses; it is never placed in the fortress, where the real thieves stay as they are. A beaten one is not written down.";

pub fn pack_of(name: &str) -> String {
    format!("Pack/Actor/{name}.pack.zs")
}

pub fn actor_file(name: &str) -> String {
    format!("Actor/{name}.engine__actor__ActorParam.bgyml")
}

pub fn table_file(name: &str) -> String {
    format!("GameParameter/GameParameterTable/{name}.engine__actor__GameParameterTable.bgyml")
}

pub fn boss_file(name: &str) -> String {
    format!("Game/DungeonBossCommonParam/{name}_01.game__enemy__DungeonBossCommonParam.bgyml")
}

pub fn own() -> [String; 7] {
    [actor_file(CLASS), table_file(CLASS), boss_file(CLASS), DEATH_CLONE.to_string(), DROP_PARAM_CLONE.to_string(), DROP_TABLE_CLONE.to_string(), DROP_SUB_CLONE.to_string()]
}

pub fn death_table_of(name: &str) -> String {
    respawn::file(name)
}

pub fn death_table() -> String {
    death_table_of(CLASS)
}

fn work(p: &str) -> String {
    match p.strip_suffix(".bgyml") {
        Some(s) => format!("Work/{s}.gyml"),
        None => format!("Work/{p}"),
    }
}

fn get<'a>(es: &'a [Entry], p: &str) -> Res<&'a Entry> {
    es.iter().find(|e| e.name == p).ok_or_else(|| E::Msg(format!("gerudo fighter: the {BASE} pack has no {p}, the game files differ")))
}

fn comps(y: &mut Byml) -> Res<&mut Node> {
    y.root.get_mut("Components").ok_or(E::Format("gerudo fighter: a parameter file has no Components"))
}

fn expect(n: &Node, k: &str, want: &str) -> Res<()> {
    let have: &str = n.get(k).and_then(|v| v.as_str()).unwrap_or("");
    if have != want {
        return Err(E::Msg(format!("gerudo fighter: {k} holds {have} and not {want}, the game files differ")));
    }
    Ok(())
}

pub fn actor_param(base: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    let c: &mut Node = comps(&mut y)?;
    expect(c, "GameParameterTableRef", &format!("?{}", table_file(BASE)))?;
    expect(c, DEATH_KEY, &format!("?{DEATH_BASE}"))?;
    expect(c, DROP_KEY, &format!("?{DROP_PARAM_BASE}"))?;
    let mut inner: Node = Node::Map(Vec::new());
    inner.set(DROP_KEY, Node::Str(format!("?{DROP_PARAM_CLONE}")));
    inner.set("BattlePresentationRef", Node::Str(SHOW.to_string()));
    inner.set(DEATH_KEY, Node::Str(format!("?{DEATH_CLONE}")));
    inner.set("GameParameterTableRef", Node::Str(format!("?{}", table_file(CLASS))));
    y.root = Node::Map(vec![("$parent".to_string(), Node::Str(work(&actor_file(BASE)))), ("Components".to_string(), inner)]);
    y.write()
}

pub fn table(base: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    let c: &mut Node = comps(&mut y)?;
    expect(c, "DungeonBossCommonParam", &format!("?{}", boss_file(BASE)))?;
    c.set("DungeonBossCommonParam", Node::Str(format!("?{}", boss_file(CLASS))));
    y.write()
}

pub fn boss(base: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    let ev: &mut Node = y.root.get_mut(EVENTS).ok_or(E::Format("gerudo fighter: the boss parameters start no events, the game files differ"))?;
    for k in KEYS {
        let e: &mut Node = ev.get_mut(k).ok_or_else(|| E::Msg(format!("gerudo fighter: the boss parameters have no {k} event, the game files differ")))?;
        if e.get("EventStartConditionType").is_none() || !e.set(EVENT_NAME, Node::Str(String::new())) {
            return Err(E::Msg(format!("gerudo fighter: the {k} event has no start condition, the game files differ")));
        }
        if let Node::Map(m) = e {
            m.retain(|(n, _)| n != DREAM_NAME);
        }
    }
    y.write()
}

pub fn death(base: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    expect(&y.root, PROCEDURES, &work(&death_table_of(BASE)))?;
    if !y.root.set(PROCEDURES, Node::Str(work(&death_table()))) {
        return Err(E::Format("gerudo fighter: the thief death file is not a map"));
    }
    y.write()
}

pub fn cells(raw: &[u8]) -> Res<usize> {
    match Byml::parse(raw)?.root.get(respawn::CELLS) {
        Some(Node::Map(c)) => Ok(c.len()),
        _ => Err(E::Format("gerudo fighter: a death table has no cell list")),
    }
}

pub fn table_size(rst: &Rst, peer: &str, peer_raw: &[u8], out: &[u8]) -> Res<u32> {
    let have: u64 = rst.get(peer).ok_or_else(|| E::Msg(format!("gerudo fighter: {peer} has no size table entry")))? as u64;
    let more: u64 = cells(out)?.saturating_sub(cells(peer_raw)?) as u64;
    let grow: u64 = rsizetable::align(out.len().saturating_sub(peer_raw.len()) as u64) + CELL_COST * more;
    u32::try_from(have + grow + FILE_MARGIN).map_err(|_| E::Format("gerudo fighter: the death table size does not fit in 32 bits"))
}

pub fn procedures(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    let c: &Node = y.root.get(respawn::CELLS).and_then(|c| c.get(respawn::CELL)).ok_or(E::Format("gerudo fighter: the thief death table has no cell for a beaten enemy"))?;
    if c.get(DROP) != Some(&Node::Bool(false)) {
        return Err(E::Msg(format!("gerudo fighter: the thief death table drops something already ({DROP}), the game files differ")));
    }
    let cells: &mut Node = y.root.get_mut(respawn::CELLS).ok_or(E::Format("gerudo fighter: the thief death table has no cell list"))?;
    for r in ENABLED {
        let k: String = format!("{r}___Data");
        if cells.get(&k).is_none() {
            let m: Node = Node::Map(vec![("$type".to_string(), Node::Str(respawn::CELL_TYPE.to_string())), ("ColumnKey".to_string(), Node::Str("Data".to_string())), ("RowKey".to_string(), Node::Str(r.to_string()))]);
            if !cells.set(&k, m) {
                return Err(E::Format("gerudo fighter: the cell list of the thief death table is not a map"));
            }
        }
        let c: &mut Node = cells.get_mut(&k).ok_or(E::Format("gerudo fighter: a cell of the thief death table is missing"))?;
        let ok: bool = c.set(DESTROY, Node::Bool(true)) && c.set(WAIT, Node::Bool(false)) && c.set(DROP, Node::Bool(true)) && c.set(respawn::KEPT, Node::Bool(false)) && c.set(respawn::SAVED, Node::Bool(false));
        if !ok {
            return Err(E::Format("gerudo fighter: a cell of the thief death table is not a map"));
        }
    }
    y.write()
}

pub fn rupee_item() -> Node {
    Node::Map(vec![
        ("BadLuckValueRate".to_string(), Node::F32(1.0)),
        ("DropActorName".to_string(), Node::Str(format!("Work/Actor/{RUPEE}.engine__actor__ActorParam.gyml"))),
        ("DropCreateCount".to_string(), Node::U32(RUPEE_COUNT)),
        ("DropProbability".to_string(), Node::I32(100)),
        ("IsReplace".to_string(), Node::Bool(true)),
        ("MostDepletedResource".to_string(), Node::Arr(Vec::new())),
        ("Type".to_string(), Node::U32(RUPEE_TYPE)),
    ])
}

fn retarget(raw: &[u8], list: &str, to: &str, name: bool) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    if !matches!(y.root.get(list), Some(Node::Arr(_))) {
        return Err(E::Msg(format!("gerudo fighter: a drop file has no {list}, the game files differ")));
    }
    y.root.set(list, Node::Arr(vec![Node::Str(work(to))]));
    if !name {
        y.root.remove("DropTableName");
    }
    y.write()
}

pub fn drop_files(base: &[Entry], donor: &[Entry]) -> Res<Vec<Entry>> {
    let sub: &Entry = donor.iter().find(|e| e.name == DROP_SUB_NONE).ok_or_else(|| E::Msg(format!("gerudo fighter: the {FROM} pack has no {DROP_SUB_NONE}, the game files differ")))?;
    let mut out: Vec<Entry> = Vec::new();
    out.push(Entry { name: DROP_PARAM_CLONE.to_string(), data: retarget(&get(base, DROP_PARAM_BASE)?.data, "DropTableResourceList", DROP_TABLE_CLONE, true)? });
    out.push(Entry { name: DROP_TABLE_CLONE.to_string(), data: retarget(&get(base, DROP_TABLE_NONE)?.data, "SubTableList", DROP_SUB_CLONE, false)? });
    let mut s: Byml = Byml::parse(&sub.data)?;
    s.root.set("Items", Node::Arr(vec![rupee_item()]));
    out.push(Entry { name: DROP_SUB_CLONE.to_string(), data: s.write()? });
    Ok(out)
}

pub fn spin(raw: &[u8]) -> Res<Vec<u8>> {
    let mut a: Ainb = Ainb::parse(raw)?;
    let sets: Vec<usize> = a.find_els(SET_BOOL).into_iter().filter(|i| a.prop_str(*i, "BlackboardKey") == Some(ARREST)).collect();
    let [set] = sets.as_slice() else {
        return Err(E::Format("gerudo fighter: the spin attack does not set the arrest request once, the game files differ"));
    };
    let (p, pos): (usize, usize) = a.parent_of(*set).map_err(|_| E::Format("gerudo fighter: the arrest request has no single parent, the game files differ"))?;
    let kids: Vec<usize> = a.params[p].group(2).iter().map(|k| k.a as usize).collect();
    let [x, y] = kids.as_slice() else {
        return Err(E::Format("gerudo fighter: the node above the arrest request is not a two way switch, the game files differ"));
    };
    let other: usize = if *x == *set { *y } else { *x };
    if a.el_type(p) != SWITCH || a.el_name(other) != IDLE {
        return Err(E::Format("gerudo fighter: the arrest request is not behind a switch with an idle other side, the game files differ"));
    }
    a.params[p].plugs[pos].a = other as u32;
    let bad: Vec<String> = audit(&a);
    if !bad.is_empty() {
        return Err(E::Msg(format!("gerudo fighter: the spin attack graph: {}", bad.join("; "))));
    }
    let out: Vec<u8> = a.write()?;
    let back: Ainb = Ainb::parse(&out)?;
    if back.write()? != out || back.params != a.params {
        return Err(E::Format("gerudo fighter: the written spin attack graph does not read back"));
    }
    Ok(out)
}

pub fn build(base: &[Entry], donor: &[Entry], keep_ai: bool) -> Res<Vec<Entry>> {
    if base.iter().any(|e| e.name.contains(CLASS)) {
        return Err(E::Msg(format!("gerudo fighter: the {BASE} pack already holds files named {CLASS}")));
    }
    let mut out: Vec<Entry> = base.to_vec();
    if !keep_ai {
        let e: &mut Entry = out.iter_mut().find(|e| e.name == SPIN).ok_or_else(|| E::Msg(format!("gerudo fighter: the {BASE} pack has no {SPIN}, the game files differ")))?;
        e.data = spin(&e.data)?;
    }
    out.extend(drop_files(base, donor)?);
    out.push(Entry { name: actor_file(CLASS), data: actor_param(&get(base, &actor_file(BASE))?.data)? });
    out.push(Entry { name: table_file(CLASS), data: table(&get(base, &table_file(BASE))?.data)? });
    out.push(Entry { name: boss_file(CLASS), data: boss(&get(base, &boss_file(BASE))?.data)? });
    out.push(Entry { name: DEATH_CLONE.to_string(), data: death(&get(base, DEATH_BASE)?.data)? });
    get(base, &SHOW[1..])?;
    Ok(out)
}

pub fn rows(info: &mut Byml, game: &mut Byml, base_len: usize, len: usize) -> Res<usize> {
    let mut a: Node = rows::need(info, BASE, ACTOR_INFO)?.clone();
    let h: i32 = heap(rows::own_heap(info, BASE)?, base_len, len)?;
    let v: Node = match a.get(HEAP) {
        Some(Node::U32(_)) => Node::U32(h.max(0) as u32),
        _ => Node::I32(h),
    };
    a.set(HEAP, v);
    a.set(ROW_ID, Node::Str(CLASS.to_string()));
    let mut g: Node = rows::need(game, BASE, GAME_ACTOR_INFO)?.clone();
    g.set(ROW_ID, Node::Str(CLASS.to_string()));
    if rows::find(info, CLASS).is_some() || rows::find(game, CLASS).is_some() {
        return Err(E::Msg(format!("gerudo fighter: the actor tables already hold {CLASS}")));
    }
    let i: usize = rows::insert(info, a)?;
    let j: usize = rows::insert(game, g)?;
    if i != j {
        return Err(E::Msg(format!("gerudo fighter: {CLASS} sits at {i} in the actor table and at {j} in the game actor table")));
    }
    Ok(i)
}

pub fn tags(t: &mut Table) -> Res<()> {
    let own: Vec<String> = t.tags_of(BASE);
    if own.is_empty() {
        return Err(E::Msg(format!("gerudo fighter: {BASE} has no tag row, the game files differ")));
    }
    if t.find(CLASS).is_some() {
        return Err(E::Msg(format!("gerudo fighter: the tag table already holds {CLASS}")));
    }
    t.insert(CLASS, &own)?;
    Ok(())
}

pub fn placed(hash: u64, at: [f32; 3], yaw: f32) -> Node {
    let mut n: Node = actor(CLASS, hash, at, crate::difficulty::ambush::euler(0.0, yaw));
    n.set("Dynamic", dynamic());
    n
}

pub fn dynamic() -> Node {
    Node::Map(vec![(NO_RECORD.to_string(), Node::Bool(false))])
}

#[derive(Clone, Debug, Default)]
pub struct Made {
    pub outs: Vec<Out>,
    pub sizes: Vec<(String, u32)>,
    pub lines: Vec<String>,
}

fn current(rf: &mut Romfs, have: &dyn Fn(&str) -> Option<Vec<u8>>, p: &str) -> Res<(Vec<u8>, u32)> {
    let z: Vec<u8> = match have(p) {
        Some(z) => z,
        None => rf.read(p)?,
    };
    let id: u32 = dict_id(&z).or_else(|| dict_id(&rf.read(p).unwrap_or_default())).unwrap_or(BANC_DICT);
    Ok((rf.zs.decompress(&z)?, id))
}

pub fn parent_of(p: &str) -> String {
    match p {
        DROP_PARAM_CLONE => DROP_PARAM_BASE.to_string(),
        DROP_TABLE_CLONE => DROP_TABLE_NONE.to_string(),
        DROP_SUB_CLONE => DROP_SUB_NONE.to_string(),
        DEATH_CLONE => DEATH_BASE.to_string(),
        _ => p.replace(CLASS, BASE),
    }
}

pub fn stage(rf: &mut Romfs, have: &dyn Fn(&str) -> Option<Vec<u8>>, rst: &Rst, test: bool, keep_ai: bool) -> Res<Made> {
    let mut m: Made = Made::default();
    if rf.root.join(pack_of(CLASS)).is_file() {
        return Err(E::Msg(format!("gerudo fighter: the game already ships {CLASS}")));
    }
    let bz: Vec<u8> = match have(&pack_of(BASE)) {
        Some(z) => z,
        None => rf.read(&pack_of(BASE))?,
    };
    let braw: Vec<u8> = rf.zs.decompress(&bz)?;
    let base: Vec<Entry> = sarc::parse(&braw)?;
    let pid: u32 = dict_id(&bz).unwrap_or(PACK_DICT);
    let dz: Vec<u8> = rf.read(&pack_of(FROM))?;
    let donor: Vec<Entry> = sarc::parse(&rf.zs.decompress(&dz)?)?.into_iter().filter(|e| e.name.contains("Drop")).collect();
    let es: Vec<Entry> = build(&base, &donor, keep_ai)?;
    let raw: Vec<u8> = sarc::write(&es)?;
    let near: Vec<(String, usize)> = base.iter().chain(donor.iter()).map(|e| (e.name.clone(), e.data.len())).collect();
    for p in own() {
        let len: usize = get(&es, &p)?.data.len();
        let learned: u32 = file_size(rst, &near, &p, len)?;
        let floor: u32 = rst.get(&parent_of(&p)).map_or(0, |n| n.saturating_add(rsizetable::align(len as u64) as u32));
        m.sizes.push((p.clone(), learned.max(floor)));
    }
    m.sizes.push((pack_of(CLASS), pack_size(raw.len())?));
    m.outs.push(Out { path: pack_of(CLASS), z: zs::compress_raw(&raw, Some(pid))?, old: 0, new: raw.len() });
    let (iraw, iid): (Vec<u8>, u32) = current(rf, have, ACTOR_INFO)?;
    let (graw, gid): (Vec<u8>, u32) = current(rf, have, GAME_ACTOR_INFO)?;
    let mut info: Byml = Byml::parse(&iraw)?;
    let mut game: Byml = Byml::parse(&graw)?;
    rows(&mut info, &mut game, braw.len(), raw.len())?;
    for (p, y, old, id) in [(ACTOR_INFO, &info, iraw.len(), iid), (GAME_ACTOR_INFO, &game, graw.len(), gid)] {
        let out: Vec<u8> = y.write()?;
        m.outs.push(Out { path: p.to_string(), z: zs::compress_raw(&out, Some(id))?, old, new: out.len() });
    }
    let (traw, tid): (Vec<u8>, u32) = current(rf, have, TAG)?;
    let mut t: Table = Table::parse(&traw)?;
    tags(&mut t)?;
    let tout: Vec<u8> = t.write()?;
    m.outs.push(Out { path: TAG.to_string(), z: zs::compress_raw(&tout, Some(tid))?, old: traw.len(), new: tout.len() });
    let tp: String = death_table_of(BASE);
    let peer: Vec<u8> = rf.read(&tp)?;
    let traw2: Vec<u8> = match have(&tp) {
        Some(b) => b,
        None => peer.clone(),
    };
    let dt: Vec<u8> = procedures(&traw2)?;
    m.sizes.push((death_table(), table_size(rst, &tp, &peer, &dt)?));
    m.outs.push(Out { path: death_table(), old: 0, new: dt.len(), z: dt });
    m.lines.push(format!("{CLASS}: new actor class, pack {} bytes with {} files ({} of its own) and its own death table; its spin attack no longer asks for the jail", raw.len(), es.len(), own().len()));
    if test {
        let (mraw, mid): (Vec<u8>, u32) = current(rf, have, TEST_FILE)?;
        let mut y: Byml = Byml::parse(&mraw)?;
        let h: u64 = fresh(&[&mraw], &format!("{SEED} test"));
        let acts: &mut Vec<Node> = y.root.get_mut("Actors").and_then(|a| a.as_arr_mut()).ok_or(E::Format("gerudo fighter: the map has no actor list"))?;
        acts.push(placed(h, TEST_AT, 0.0));
        let mout: Vec<u8> = y.write()?;
        m.outs.push(Out { path: TEST_FILE.to_string(), z: zs::compress_raw(&mout, Some(mid))?, old: mraw.len(), new: mout.len() });
        m.lines.push(format!("{TEST_FILE}: one {CLASS} placed at {TEST_AT:?} (test only)"));
    }
    Ok(m)
}

pub fn sized(rf: &mut Romfs, t: &mut Rst, m: &Made) -> Res<usize> {
    let mut n: usize = 0;
    for (p, sz) in &m.sizes {
        t.set_size(p, *sz)?;
        n += 1;
    }
    for o in m.outs.iter().filter(|o| !m.sizes.iter().any(|s| s.0 == o.path)) {
        n += grow_out(rf, t, o)?;
    }
    Ok(n)
}

pub fn sources() -> Vec<String> {
    vec![pack_of(BASE), pack_of(FROM), ACTOR_INFO.to_string(), GAME_ACTOR_INFO.to_string(), TAG.to_string(), death_table_of(BASE), TEST_FILE.to_string()]
}

pub fn write(rf: &mut Romfs, pk: &Pack, rst: &mut Option<Rst>) -> Res<(usize, Vec<String>)> {
    let root: PB = pk.work.join("romfs");
    if rst.is_none() {
        *rst = Some(rsizetable::load(rf)?);
    }
    let t: &mut Rst = rst.as_mut().ok_or(E::Format("gerudo fighter: no size table"))?;
    let have = |p: &str| -> Option<Vec<u8>> { fs::read(p.split('/').fold(root.clone(), |a, s| a.join(s))).ok() };
    let m: Made = stage(rf, &have, t, TEST_SPOT, KEEP_AI)?;
    for o in &m.outs {
        pk.write_romfs(&o.path, &o.z)?;
    }
    sized(rf, t, &m)?;
    Ok((m.outs.len(), m.lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> Option<PB> {
        let p: PB = ootrr_game::test_romfs::path();
        if p.join("Pack/Actor").is_dir() { Some(p) } else { None }
    }

    fn y(es: &[Entry], p: &str) -> Byml {
        Byml::parse(&sarc::get(es, p).unwrap().data).unwrap()
    }

    fn donor(rf: &mut Romfs) -> Vec<Entry> {
        sarc::parse(&rf.read_zs(&pack_of(FROM)).unwrap()).unwrap().into_iter().filter(|e| e.name.contains("Drop")).collect()
    }

    fn s(n: &Node, path: &[&str]) -> String {
        let mut c: &Node = n;
        for k in path {
            c = c.get(k).unwrap();
        }
        c.as_str().unwrap_or("").to_string()
    }

    fn base(rf: &mut Romfs) -> Vec<Entry> {
        sarc::parse(&rf.read_zs(&pack_of(BASE)).unwrap()).unwrap()
    }

    #[test]
    fn the_names_and_paths_are_the_clones_own() {
        assert_eq!(work("Actor/X.engine__actor__ActorParam.bgyml"), "Work/Actor/X.engine__actor__ActorParam.gyml");
        assert_eq!(own().len(), 7);
        assert!(own().iter().all(|p| p.contains(CLASS)));
        assert!(death_table().ends_with("_game__death__DeathReasonProcedures.pp__CombinationDataTableData.bgyml") && death_table().contains(CLASS));
        assert!(CLASS.len() <= BASE.len() && !CLASS.starts_with(BASE) && CLASS.starts_with("Enemy_") && !CLASS.contains("MiddleBoss"));
        assert_eq!(parent_of(&actor_file(CLASS)), actor_file(BASE));
        assert_eq!(parent_of(&boss_file(CLASS)), boss_file(BASE));
        assert_eq!(parent_of(&table_file(CLASS)), table_file(BASE));
        assert_eq!(rupee_item().get("DropProbability"), Some(&Node::I32(100)));
    }

    #[test]
    fn the_clone_pack_is_the_thief_pack_plus_its_own_files_and_the_spin_graph() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        assert!(!r.join(pack_of(CLASS)).is_file());
        let b: Vec<Entry> = base(&mut rf);
        let dn: Vec<Entry> = donor(&mut rf);
        let es: Vec<Entry> = build(&b, &dn, false).unwrap();
        assert_eq!(es.len(), b.len() + 7);
        let back: Vec<Entry> = sarc::parse(&sarc::write(&es).unwrap()).unwrap();
        for e in &b {
            if e.name == SPIN {
                assert_ne!(sarc::get(&back, &e.name).map(|x| &x.data), Some(&e.data));
            } else {
                assert_eq!(sarc::get(&back, &e.name).map(|x| &x.data), Some(&e.data), "{}", e.name);
            }
        }
        let a: Byml = y(&back, &actor_file(CLASS));
        assert_eq!(s(&a.root, &["$parent"]), "Work/Actor/Enemy_MiddleBoss_GerudoThief.engine__actor__ActorParam.gyml");
        assert_eq!(s(&a.root, &["Components", "BattlePresentationRef"]), SHOW);
        assert_eq!(s(&a.root, &["Components", DEATH_KEY]), format!("?{DEATH_CLONE}"));
        assert_eq!(s(&a.root, &["Components", DROP_KEY]), format!("?{DROP_PARAM_CLONE}"));
        assert_eq!(s(&a.root, &["Components", "GameParameterTableRef"]), format!("?{}", table_file(CLASS)));
        let Some(Node::Map(c)) = a.root.get("Components") else { panic!() };
        assert_eq!(c.len(), 4);
        let t: Byml = y(&back, &table_file(CLASS));
        assert_eq!(s(&t.root, &["Components", "DungeonBossCommonParam"]), format!("?{}", boss_file(CLASS)));
        assert_eq!(s(&t.root, &["Components", "EnemyCommonParam"]), "?GameParameter/EnemyCommonParam/Enemy_MiddleBoss_GerudoThief.game__enemy__EnemyCommonParam.bgyml");
        let d: Byml = y(&back, DEATH_CLONE);
        assert_eq!(s(&d.root, &[PROCEDURES]), work(&death_table()));
        assert_eq!(s(&d.root, &["$parent"]), s(&y(&b, DEATH_BASE).root, &["$parent"]));
        assert!(build(&es, &dn, false).is_err());
        let keep: Vec<Entry> = build(&b, &dn, true).unwrap();
        assert_eq!(sarc::get(&keep, SPIN).map(|x| &x.data), sarc::get(&b, SPIN).map(|x| &x.data));
    }

    #[test]
    fn the_boss_parameters_keep_both_event_keys_and_conditions_with_blank_names() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let b: Vec<Entry> = base(&mut rf);
        let old: Byml = y(&b, &boss_file(BASE));
        let new: Byml = Byml::parse(&boss(&sarc::get(&b, &boss_file(BASE)).unwrap().data).unwrap()).unwrap();
        let (ev_old, ev_new): (&Node, &Node) = (old.root.get(EVENTS).unwrap(), new.root.get(EVENTS).unwrap());
        for k in KEYS {
            assert_eq!(s(ev_new, &[k, EVENT_NAME]), "");
            assert!(!s(ev_old, &[k, EVENT_NAME]).is_empty());
            assert_eq!(ev_new.get(k).and_then(|e| e.get("EventStartConditionType")), ev_old.get(k).and_then(|e| e.get("EventStartConditionType")));
            assert!(ev_new.get(k).and_then(|e| e.get(DREAM_NAME)).is_none());
        }
        assert_eq!(ev_new.get("Death").and_then(|e| e.get("EventStartConditionType")), Some(&Node::U32(2_204_074_072)));
        assert_eq!(ev_new.get("Arrest").and_then(|e| e.get("EventStartConditionType")), Some(&Node::U32(541_910_881)));
        let txt: String = format!("{:?}", new.root);
        assert!(!txt.contains("GerudoFortress") && !txt.contains("MiddleBoss"));
        assert_eq!(s(&new.root, &["$parent"]), s(&old.root, &["$parent"]));
    }

    #[test]
    fn the_spin_attack_can_no_longer_set_the_arrest_request() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let b: Vec<Entry> = base(&mut rf);
        let raw: &[u8] = &sarc::get(&b, SPIN).unwrap().data;
        let was: Ainb = Ainb::parse(raw).unwrap();
        let out: Vec<u8> = spin(raw).unwrap();
        let now: Ainb = Ainb::parse(&out).unwrap();
        assert_eq!(out.len(), raw.len());
        assert!(audit(&now).is_empty());
        let sets = |a: &Ainb| -> Vec<usize> { a.find_els(SET_BOOL).into_iter().filter(|i| a.prop_str(*i, ARREST) == Some(ARREST) || a.prop_str(*i, "BlackboardKey") == Some(ARREST)).collect() };
        let (w, n): (Vec<usize>, Vec<usize>) = (sets(&was), sets(&now));
        assert_eq!((w.len(), n.len()), (1, 1));
        assert!(was.parent_of(w[0]).is_ok());
        assert!(now.parent_of(n[0]).is_err(), "the arrest request still has a parent");
        assert_eq!(was.els.len(), now.els.len());
        let mut diff: usize = 0;
        for i in 0..was.params.len() {
            if was.params[i] != now.params[i] {
                diff += 1;
            }
        }
        assert_eq!(diff, 1);
        assert!(spin(&out).is_err());
    }

    #[test]
    fn the_death_table_drops_and_never_waits_in_every_enabled_cell_and_changes_nothing_else() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let tp: String = death_table_of(BASE);
        let game_raw: Vec<u8> = rf.read(&tp).unwrap();
        let made: Vec<u8> = procedures(&game_raw).unwrap();
        let game: Byml = Byml::parse(&game_raw).unwrap();
        let got: Byml = Byml::parse(&made).unwrap();
        assert_eq!(got.write().unwrap(), made);
        assert_eq!(s(&got.root, &["$parent"]), s(&game.root, &["$parent"]));
        let (Some(Node::Map(want)), Some(Node::Map(cs))) = (game.root.get(respawn::CELLS), got.root.get(respawn::CELLS)) else { panic!() };
        let keys: Vec<String> = ENABLED.iter().map(|r| format!("{r}___Data")).collect();
        for k in &keys {
            let c: &Node = got.root.get(respawn::CELLS).and_then(|c| c.get(k)).unwrap();
            assert_eq!(c.get(DROP), Some(&Node::Bool(true)), "{k}");
            assert_eq!(c.get(WAIT), Some(&Node::Bool(false)), "{k}");
            assert_eq!(c.get(DESTROY), Some(&Node::Bool(true)), "{k}");
            assert_eq!(c.get(respawn::KEPT), Some(&Node::Bool(false)), "{k}");
            assert_eq!(c.get(respawn::SAVED), Some(&Node::Bool(false)), "{k}");
            assert_eq!(c.get("$type").and_then(|t| t.as_str()), Some(respawn::CELL_TYPE), "{k}");
        }
        let life: &Node = got.root.get(respawn::CELLS).and_then(|c| c.get(respawn::CELL)).unwrap();
        assert_eq!(life.get("DeathProcedure"), Some(&Node::U32(3_265_626_599)));
        for (k, v) in want {
            if !keys.contains(k) {
                assert_eq!(got.root.get(respawn::CELLS).and_then(|c| c.get(k)).unwrap(), v, "{k}");
            }
        }
        assert_eq!(cs.len(), want.len() + keys.iter().filter(|k| !want.iter().any(|(n, _)| n == *k)).count());
        let txt: String = format!("{:?}", got.root).replace("IsSetRespawnFlagToFalse", "").replace("IsSetGmdRespawnFlagToFalse", "");
        for bad in ["Event", "Flag", "Gerudo"] {
            assert!(!txt.replace(BASE, "").contains(bad) || bad == "Gerudo", "{bad}");
        }
    }

    #[test]
    fn every_enabled_death_reason_after_inheritance_is_on_the_list() {
        let Some(r) = root() else { return };
        let rf: Romfs = Romfs::open(&r).unwrap();
        let mut merged: Vec<(String, bool)> = Vec::new();
        for t in ["Default", "EnemyCommon", BASE] {
            let y: Byml = Byml::parse(&rf.read(&death_table_of(t)).unwrap()).unwrap();
            let Some(Node::Map(c)) = y.root.get(respawn::CELLS) else { panic!() };
            for (k, v) in c {
                if let Some(Node::Bool(b)) = v.get("IsIgnoreThisDeathReason") {
                    match merged.iter_mut().find(|(n, _)| n == k) {
                        Some(e) => e.1 = *b,
                        None => merged.push((k.clone(), *b)),
                    }
                } else if !merged.iter().any(|(n, _)| n == k) {
                    merged.push((k.clone(), false));
                }
            }
        }
        let mut on: Vec<String> = merged.into_iter().filter(|(_, i)| !*i).map(|(k, _)| k.trim_end_matches("___Data").to_string()).collect();
        on.sort();
        let mut want: Vec<String> = ENABLED.iter().map(|r| r.to_string()).collect();
        want.sort();
        assert_eq!(on, want);
    }

    fn game_tables(r: &PB) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<(String, Vec<u8>)> = Vec::new();
        for e in fs::read_dir(r.join(respawn::DIR)).unwrap() {
            let n: String = e.unwrap().file_name().to_string_lossy().to_string();
            if n.ends_with(respawn::EXT) {
                let p: String = format!("{}{n}", respawn::DIR);
                out.push((p.clone(), fs::read(r.join(&p)).unwrap()));
            }
        }
        out.sort();
        out
    }

    #[test]
    fn the_death_table_size_covers_what_every_game_table_of_its_length_and_cells_needs() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let t: Rst = rsizetable::load(&mut rf).unwrap();
        let tp: String = death_table_of(BASE);
        let peer: Vec<u8> = rf.read(&tp).unwrap();
        let all: Vec<(String, Vec<u8>)> = game_tables(&r);
        assert!(all.len() > 100);
        let mut n: usize = 0;
        for (p, b) in &all {
            let Some(want) = t.get(p) else { continue };
            let per: i64 = want as i64 - b.len() as i64 - 240 * cells(b).unwrap() as i64;
            assert!((5040..5400).contains(&per), "{p}: {per}");
            let got: u32 = table_size(&t, p, b, b).unwrap() - FILE_MARGIN as u32;
            assert_eq!(got, want, "{p}");
            n += 1;
        }
        assert!(n > 100);
        let made: Vec<u8> = procedures(&peer).unwrap();
        let need: u64 = made.len() as u64 + 5400 + 240 * cells(&made).unwrap() as u64;
        let have: u32 = table_size(&t, &tp, &peer, &made).unwrap();
        assert!(have as u64 >= need + 512, "{have} < {need}");
        assert!(cells(&made).unwrap() > cells(&peer).unwrap());
    }

    #[test]
    fn the_new_death_table_uses_only_fields_and_kinds_the_game_tables_use() {
        let Some(r) = root() else { return };
        let mut fields: Vec<(String, std::mem::Discriminant<Node>)> = Vec::new();
        for (_, b) in game_tables(&r) {
            let y: Byml = Byml::parse(&b).unwrap();
            let Some(Node::Map(c)) = y.root.get(respawn::CELLS) else { continue };
            for (_, v) in c {
                let Node::Map(m) = v else { panic!() };
                for (k, x) in m {
                    if !fields.iter().any(|(n, _)| n == k) {
                        fields.push((k.clone(), std::mem::discriminant(x)));
                    }
                }
            }
        }
        let peer: Vec<u8> = fs::read(r.join(death_table_of(BASE))).unwrap();
        let made: Vec<u8> = procedures(&peer).unwrap();
        let y: Byml = Byml::parse(&made).unwrap();
        let Some(Node::Map(c)) = y.root.get(respawn::CELLS) else { panic!() };
        for (k, v) in c {
            let Node::Map(m) = v else { panic!("{k}") };
            let names: Vec<&str> = m.iter().map(|(n, _)| n.as_str()).collect();
            let mut sorted: Vec<&str> = names.clone();
            sorted.sort();
            assert_eq!(names, sorted, "{k}");
            assert_eq!(v.get("$type").and_then(|t| t.as_str()), Some(respawn::CELL_TYPE), "{k}");
            for (n, x) in m {
                let kind: Option<&std::mem::Discriminant<Node>> = fields.iter().find(|(f, _)| f == n).map(|(_, d)| d);
                assert_eq!(kind, Some(&std::mem::discriminant(x)), "{k} {n}");
            }
        }
    }

    #[test]
    fn the_clone_drops_one_red_rupee_from_its_own_table() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let b: Vec<Entry> = base(&mut rf);
        let dn: Vec<Entry> = donor(&mut rf);
        let es: Vec<Entry> = drop_files(&b, &dn).unwrap();
        let names: Vec<&str> = es.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec![DROP_PARAM_CLONE, DROP_TABLE_CLONE, DROP_SUB_CLONE]);
        let sub: Byml = y(&es, DROP_SUB_CLONE);
        let items: &[Node] = sub.root.get("Items").and_then(|i| i.as_arr()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], rupee_item());
        assert_eq!(s(&items[0], &["DropActorName"]), "Work/Actor/Obj_Rupee_Red.engine__actor__ActorParam.gyml");
        let table: Byml = y(&es, DROP_TABLE_CLONE);
        assert_eq!(table.root.get("SubTableList"), Some(&Node::Arr(vec![Node::Str(work(DROP_SUB_CLONE))])));
        let param: Byml = y(&es, DROP_PARAM_CLONE);
        assert_eq!(param.root.get("DropTableResourceList"), Some(&Node::Arr(vec![Node::Str(work(DROP_TABLE_CLONE))])));
        assert_eq!(s(&param.root, &["$parent"]), s(&y(&b, DROP_PARAM_BASE).root, &["$parent"]));
        assert!(drop_files(&b, &[]).is_err());
        let pack: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of(RUPEE)).unwrap()).unwrap();
        assert!(pack.iter().any(|e| e.name == actor_file(RUPEE)));
    }

    #[test]
    fn the_rows_and_tags_copy_the_thief_under_the_new_name() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let mut info: Byml = rf.read_byml(ACTOR_INFO).unwrap();
        let mut game: Byml = rf.read_byml(GAME_ACTOR_INFO).unwrap();
        let n: usize = info.root.as_arr().unwrap().len();
        rows(&mut info, &mut game, 1000, 1800).unwrap();
        assert_eq!(info.root.as_arr().unwrap().len(), n + 1);
        assert!(rows::sorted(&info).unwrap() && rows::sorted(&game).unwrap());
        let a: &Node = rows::find(&info, CLASS).unwrap();
        assert_eq!(s(a, &["ActorName"]), BASE);
        assert!(rows::own_heap(&info, CLASS).unwrap() >= rows::own_heap(&info, BASE).unwrap() + 800);
        assert_eq!(rows::find(&game, CLASS).and_then(|g| g.get("MaxLife")).map(|v| format!("{v:?}")), rows::find(&game, BASE).and_then(|g| g.get("MaxLife")).map(|v| format!("{v:?}")));
        assert!(rows(&mut info, &mut game, 1000, 1800).is_err());
        let mut t: Table = Table::parse(&rf.read_zs(TAG).unwrap()).unwrap();
        tags(&mut t).unwrap();
        assert_eq!(t.tags_of(CLASS), t.tags_of(BASE));
        assert!(tags(&mut t).is_err());
    }

    #[test]
    fn staging_makes_the_files_sizes_every_new_file_and_leaves_the_game_alone() {
        let Some(r) = root() else { return };
        let none = |_: &str| -> Option<Vec<u8>> { None };
        for test in [false, true] {
            let mut rf: Romfs = Romfs::open(&r).unwrap();
            let t: Rst = rsizetable::load(&mut rf).unwrap();
            let m: Made = stage(&mut rf, &none, &t, test, false).unwrap();
            let mut want: Vec<String> = vec![pack_of(CLASS), ACTOR_INFO.to_string(), GAME_ACTOR_INFO.to_string(), TAG.to_string(), death_table()];
            if test {
                want.push(TEST_FILE.to_string());
            }
            assert_eq!(m.outs.iter().map(|o| o.path.clone()).collect::<Vec<String>>(), want);
            assert_eq!(m.sizes.len(), 9);
            for (p, sz) in &m.sizes {
                assert!(*sz > 0 && t.get(p).is_none(), "{p}");
            }
            let pk: Vec<Entry> = sarc::parse(&rf.zs.decompress(&m.outs[0].z).unwrap()).unwrap();
            assert!(own().iter().all(|p| sarc::get(&pk, p).is_some()));
            for p in own() {
                let len: usize = sarc::get(&pk, &p).unwrap().data.len();
                let have: u32 = m.sizes.iter().find(|(n, _)| *n == p).map(|(_, z)| *z).unwrap();
                let parent: u32 = t.get(&parent_of(&p)).unwrap();
                assert!(have as usize >= len && have >= parent, "{p} {have} {len} {parent}");
            }
            let o: &Out = m.outs.iter().find(|o| o.path == death_table()).unwrap();
            assert!(rf.read(&o.path).is_err() && t.get(&o.path).is_none());
            let peer: u32 = t.get(&death_table_of(BASE)).unwrap();
            let mine: u32 = m.sizes.iter().find(|(p, _)| p == &o.path).map(|(_, z)| *z).unwrap();
            assert!(mine as usize >= o.new + 5 * 1024 && mine >= peer, "{mine}");
            let mut rst: Rst = rsizetable::load(&mut rf).unwrap();
            sized(&mut rf, &mut rst, &m).unwrap();
            assert_eq!(rst.get(&o.path), Some(mine));
            if test {
                let o: &Out = m.outs.iter().find(|o| o.path == TEST_FILE).unwrap();
                let y: Byml = Byml::parse(&rf.zs.decompress(&o.z).unwrap()).unwrap();
                let old: Byml = rf.read_byml(TEST_FILE).unwrap();
                let acts: &[Node] = y.root.get("Actors").unwrap().as_arr().unwrap();
                assert_eq!(acts.len(), old.root.get("Actors").unwrap().as_arr().unwrap().len() + 1);
                let b: Vec<&Node> = acts.iter().filter(|a| s(a, &["Gyaml"]) == CLASS).collect();
                assert_eq!(b.len(), 1);
                assert_eq!(b[0].get("Dynamic"), Some(&dynamic()));
                assert!(b[0].get("Links").is_none());
            }
            let real: Byml = rf.read_byml("Banc/GerudoFortress/GerudoFortress_Adult_Static.bcett.byml.zs").unwrap();
            assert!(!format!("{:?}", real.root).contains(CLASS));
        }
    }

    #[test]
    fn the_test_spot_is_inside_the_temple_of_time_and_level_with_its_props() {
        assert!(TEST_FILE.contains("Market_Child"));
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let mut near: Vec<(String, [f32; 3])> = Vec::new();
        for f in [TEST_FILE, "Banc/Market/Market_Child_Dynamic.bcett.byml.zs"] {
            let y: Byml = rf.read_byml(f).unwrap();
            for a in y.root.get("Actors").and_then(|a| a.as_arr()).unwrap() {
                if let Some(p) = crate::difficulty::ambush::vec3(a, "Translate") {
                    let d: f32 = ((p[0] - TEST_AT[0]).powi(2) + (p[2] - TEST_AT[2]).powi(2)).sqrt();
                    if d < 25.0 {
                        near.push((s(a, &["Gyaml"]), p));
                    }
                }
            }
        }
        let hall: &(String, [f32; 3]) = near.iter().find(|(g, _)| g == "Land_TempleOfTime_Inside_A_01").unwrap();
        assert!((hall.1[0] - TEST_AT[0]).abs() < 10.0 && (hall.1[2] - TEST_AT[2]).abs() < 1.0);
        let anchor: &(String, [f32; 3]) = near.iter().find(|(g, _)| g == "DestinationAnchor").unwrap();
        assert!((anchor.1[1] - TEST_AT[1]).abs() < 0.5, "{:?}", anchor);
        for (g, p) in &near {
            let d: f32 = ((p[0] - TEST_AT[0]).powi(2) + (p[2] - TEST_AT[2]).powi(2)).sqrt();
            assert!(d > 5.0 || g.starts_with("Land_") || g.ends_with("Area"), "{g} {p:?} is {d} away");
        }
    }

    #[test]
    fn the_clone_takes_the_scaled_life_and_attack_of_the_staged_thief_pack() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let none = |_: &str| -> Option<Vec<u8>> { None };
        let o: crate::difficulty::Opts = crate::difficulty::Opts { health: 50, damage: 200, ..crate::difficulty::Opts::default() };
        let outs: Vec<Out> = crate::difficulty::stage(&mut rf, &o, &none, "seed").unwrap();
        let staged: Out = outs.into_iter().find(|x| x.path == pack_of(BASE)).unwrap();
        let t: Rst = rsizetable::load(&mut rf).unwrap();
        let have = |p: &str| -> Option<Vec<u8>> { if p == pack_of(BASE) { Some(staged.z.clone()) } else { None } };
        let m: Made = stage(&mut rf, &have, &t, false, false).unwrap();
        let clone: Vec<Entry> = sarc::parse(&rf.zs.decompress(&m.outs[0].z).unwrap()).unwrap();
        let plain: Vec<Entry> = base(&mut rf);
        let life: String = format!("{}{BASE}{}", crate::difficulty::stats::LIFE_DIR, crate::difficulty::stats::LIFE_EXT);
        let atk: String = format!("{}{BASE}{}", crate::difficulty::stats::ATTACK_DIR, crate::difficulty::stats::ATTACK_EXT);
        let hp = |es: &[Entry]| -> i32 { if let Some(Node::I32(v)) = y(es, &life).root.get("MaxLife") { *v } else { panic!() } };
        assert_eq!(hp(&plain), 18);
        assert_eq!(hp(&clone), 9);
        assert_ne!(sarc::get(&clone, &atk).unwrap().data, sarc::get(&plain, &atk).unwrap().data);
    }
}
