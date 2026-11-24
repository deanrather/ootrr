use std::fs;
use std::path::PathBuf as PB;

use crate::byml::{Byml, Node};
use crate::cow_clone::rows::heap;
use crate::difficulty::ambush::{actor, fresh, vec3};
use crate::difficulty::respawn;
use crate::difficulty::volvagia_die::{self as die_graph, Mode, DIE_MODULE, HEAP_ROOM, HOLDER_CLONE, HOLDER_KEY, HOLDER_OWN};
use crate::error::{Error as E, Res};
use crate::flow_locations::dict_id;
use crate::modpack::Pack;
use crate::pickup_pack::rows::{self, ACTOR_INFO, GAME_ACTOR_INFO, HEAP, ROW_ID};
use crate::pickup_pack::sizes::{file_size, pack_size, FILE_MARGIN};
use crate::pickup_pack::tag::{Table, TAG};
use crate::pickup_pack::PACK_DICT;
use crate::placement::hash_of;
use crate::romfs::Romfs;
use crate::rsizetable::{self, Rst};
use crate::sarc::{self, Entry};
use crate::staging::{grow_out, Out, BANC_DICT, MARGIN};
use crate::zs;

pub const BASE: &str = "Enemy_DungeonBoss_Volvagia";
pub const CLASS: &str = "Enemy_Volvagia_Roam";
pub const STAGE: &str = "Banc/DreamWorld/VolvagiaStage_Static.bcett.byml.zs";
pub const ANCHOR: &str = "DestinationAnchor";
pub const CENTRE: &str = "RoomCenter";
pub const HOLES: [&str; 8] = ["Zero", "One", "Two", "Three", "Five", "Six", "Seven", "Eight"];
pub const APPEAR_KEY: &str = "IsIntendedForEventAppear";
pub const EVENTS: &str = "EventStartParameters";
pub const DIE: &str = "Die";
pub const APPEAR: &str = "Appear";
pub const CONDITION: &str = "EventStartConditionType";
pub const EVENT_NAME: &str = "EventName";
pub const GROUPS: [&str; 2] = ["FarDeleteGroups", "SimultaneousGroups"];
pub const SEED: &str = "ootrr volvagia roams";
pub const WIDE: [&str; 7] = ["AirDiveStartDistance", "BeforeCircleMoveDistance", "CircleMoveDistance", "FinalCircleMoveDistance", "JumpOutCircleMoveDistance", "MaxLeaveDistRoomCenter", "FireBreathMaxDistanceRoomCenter"];
pub const BOSS_SHOW: &str = "?Component/BattlePresentationParam/EnemyCommon.game__component__BattlePresentationParam.bgyml";
pub const DEATH_KEY: &str = "DeathRef";
pub const DEATH_OWN: &str = "Component/DeathParam/Enemy_DungeonBoss_Volvagia.game__component__DeathParam.bgyml";
pub const DEATH_PLAIN: &str = "Component/DeathParam/DungeonBossCommon.game__component__DeathParam.bgyml";
pub const DEATH_CLONE: &str = "Component/DeathParam/Enemy_Volvagia_Roam.game__component__DeathParam.bgyml";
pub const DEATH_TABLE_BASE: &str = "DungeonBossVolvagia";
pub const DESTROY: &str = "IsRequestDestruction";
pub const PROCEDURES: &str = "DeathProcedures";
pub const WAIT: &str = "IsWaitForSignal";
pub const DROP: &str = "IsDropItem";
pub const CELL_COST: u64 = 256;
pub const ENABLED: [&str; 7] = ["AI", "AncientWeapon", "Crushed", "Frozen", "LifeDepleted", "SideCrushed", "UpperCrushed"];
pub const FROM: &str = "Enemy_Floormaster";
pub const DROP_KEY: &str = "DropRef";
pub const DROP_DEFAULT: &str = "Component/DropParam/Default.game__component__DropParam.bgyml";
pub const DROP_PLAIN: &str = "Component/DropParam/Drop_Default.game__component__DropParam.bgyml";
pub const DROP_BOARD: &str = "Component/Blackboard/BlackboardParamTable/Drop.engine__component__BlackboardParamTable.bgyml";
pub const DROP_TABLE_NONE: &str = "Drop/DropTable/DropTable_None.game__drop__DropTable.bgyml";
pub const DROP_SUB_NONE: &str = "Drop/DropSubTable/DropTable_None.game__drop__DropSubTable.bgyml";
pub const DROP_PARAM_CLONE: &str = "Component/DropParam/Enemy_Volvagia_Roam.game__component__DropParam.bgyml";
pub const DROP_TABLE_CLONE: &str = "Drop/DropTable/Enemy_Volvagia_Roam.game__drop__DropTable.bgyml";
pub const DROP_SUB_CLONE: &str = "Drop/DropSubTable/Enemy_Volvagia_Roam.game__drop__DropSubTable.bgyml";
pub const RUPEE: &str = "Obj_Rupee_Blue";
pub const RUPEE_COUNT: u32 = 10;
pub const RUPEE_TYPE: u32 = 995_674_644;
pub const BEATEN: &str = "HavePlayedEvent.Dm_FireTemple_BossOut";
pub const PRESENCE: &str = "Presence";
pub const NOTE: &str = "Volvagia roams Hyrule Field: as an adult, a copy of Volvagia under its own actor name flies over the open middle of Hyrule Field, coming out of the ground and diving back in at eight spots around one point, and breathes fire at you. It has no boss music, no intro scene and no death scene: beating it leaves a purple rupee (50 rupees) and changes no story flag, a beaten one is never written down, so it is back the next time the field loads, and it stops appearing once you have beaten the real Volvagia in the Fire Temple (the flag that the Fire Temple victory scene has played). The Fire Temple fight is unchanged. Functional WIP: when beaten in the field she vanishes at once and leaves a purple rupee (50 rupees) on the ground; she does not heal over time and does not flee.";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arena {
    pub name: &'static str,
    pub file: &'static str,
    pub at: [f32; 3],
}

pub const TEST: Arena = Arena { name: "ruined Market plaza (test only)", file: "Banc/Market/Market_Adult_Static.bcett.byml.zs", at: [-1.9, 100.07, 12.4] };
pub const FIELD: Arena = Arena { name: "adult Hyrule Field", file: "Banc/HyruleField/HyruleField_Adult_Static.bcett.byml.zs", at: [-36.0, 148.2, 120.0] };

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub arena: Arena,
    pub lift: f32,
    pub scale: f32,
}

pub const SHAPE: Shape = Shape { arena: FIELD, lift: 0.0, scale: 1.0 };

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
    format!("Game/DungeonBossCommonParam/{name}.game__enemy__DungeonBossCommonParam.bgyml")
}

pub fn volvagia_file(name: &str) -> String {
    format!("GameParameter/DungeonBossVolvagiaParam/{name}.game__enemy__DungeonBossVolvagiaParam.bgyml")
}

pub fn own() -> [String; 8] {
    [actor_file(CLASS), table_file(CLASS), boss_file(CLASS), volvagia_file(CLASS), DEATH_CLONE.to_string(), DROP_PARAM_CLONE.to_string(), DROP_TABLE_CLONE.to_string(), DROP_SUB_CLONE.to_string()]
}

pub fn owned(m: Mode) -> Vec<String> {
    let mut v: Vec<String> = own().to_vec();
    if m.holds() {
        v.push(HOLDER_CLONE.to_string());
    }
    v
}

pub fn death_table() -> String {
    respawn::file(CLASS)
}

fn work(p: &str) -> String {
    match p.strip_suffix(".bgyml") {
        Some(s) => format!("Work/{s}.gyml"),
        None => format!("Work/{p}"),
    }
}

fn get<'a>(es: &'a [Entry], p: &str) -> Res<&'a Entry> {
    es.iter().find(|e| e.name == p).ok_or_else(|| E::Msg(format!("volvagia roams: the {BASE} pack has no {p}, the game files differ")))
}

fn comps(y: &mut Byml) -> Res<&mut Node> {
    y.root.get_mut("Components").ok_or(E::Format("volvagia roams: a parameter file has no Components"))
}

fn expect(n: &Node, k: &str, want: &str) -> Res<()> {
    let have: &str = n.get(k).and_then(|v| v.as_str()).unwrap_or("");
    if have != want {
        return Err(E::Msg(format!("volvagia roams: {k} holds {have} and not {want}, the game files differ")));
    }
    Ok(())
}

pub fn actor_param(base: &[u8], m: Mode) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    let c: &mut Node = comps(&mut y)?;
    expect(c, "GameParameterTableRef", &format!("?{}", table_file(BASE)))?;
    expect(c, DEATH_KEY, &format!("?{DEATH_OWN}"))?;
    expect(c, HOLDER_KEY, &format!("?{HOLDER_OWN}"))?;
    let mut inner: Node = Node::Map(Vec::new());
    if m.holds() {
        inner.set(HOLDER_KEY, Node::Str(format!("?{HOLDER_CLONE}")));
    }
    inner.set(DROP_KEY, Node::Str(format!("?{DROP_PARAM_CLONE}")));
    inner.set("BattlePresentationRef", Node::Str(BOSS_SHOW.to_string()));
    inner.set(DEATH_KEY, Node::Str(format!("?{DEATH_CLONE}")));
    inner.set("GameParameterTableRef", Node::Str(format!("?{}", table_file(CLASS))));
    y.root = Node::Map(vec![("$parent".to_string(), Node::Str(work(&actor_file(BASE)))), ("Components".to_string(), inner)]);
    y.write()
}

pub fn death(base: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    let have: &str = y.root.get(PROCEDURES).and_then(|v| v.as_str()).unwrap_or("");
    let want: String = work(&respawn::file(DEATH_TABLE_BASE));
    if have != want {
        return Err(E::Msg(format!("volvagia roams: the Volvagia death file names {have} and not {want}, the game files differ")));
    }
    if !y.root.set(PROCEDURES, Node::Str(work(&death_table()))) {
        return Err(E::Format("volvagia roams: the Volvagia death file is not a map"));
    }
    y.write()
}

pub fn cells(raw: &[u8]) -> Res<usize> {
    match Byml::parse(raw)?.root.get(respawn::CELLS) {
        Some(Node::Map(c)) => Ok(c.len()),
        _ => Err(E::Format("volvagia roams: a death table has no cell list")),
    }
}

pub fn table_size(rst: &Rst, peer: &str, peer_raw: &[u8], out: &[u8]) -> Res<u32> {
    let have: u64 = rst.get(peer).ok_or_else(|| E::Msg(format!("volvagia roams: {peer} has no size table entry")))? as u64;
    let more: u64 = cells(out)?.saturating_sub(cells(peer_raw)?) as u64;
    let grow: u64 = rsizetable::align(out.len().saturating_sub(peer_raw.len()) as u64) + CELL_COST * more;
    u32::try_from(have + grow + FILE_MARGIN).map_err(|_| E::Format("volvagia roams: the death table size does not fit in 32 bits"))
}

pub fn procedures(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(&respawn::always(raw)?)?;
    let c: &mut Node = y.root.get_mut(respawn::CELLS).and_then(|c| c.get_mut(respawn::CELL)).ok_or(E::Format("volvagia roams: the Volvagia death table has no cell for a beaten enemy"))?;
    if c.get(DESTROY) != Some(&Node::Bool(false)) {
        return Err(E::Msg(format!("volvagia roams: the Volvagia death table does not keep the body ({DESTROY}), the game files differ")));
    }
    if c.get(WAIT) != Some(&Node::Bool(true)) {
        return Err(E::Msg(format!("volvagia roams: the Volvagia death table does not wait for the death scene ({WAIT}), the game files differ")));
    }
    let cells: &mut Node = y.root.get_mut(respawn::CELLS).ok_or(E::Format("volvagia roams: the Volvagia death table has no cell list"))?;
    for r in ENABLED {
        let k: String = format!("{r}___Data");
        if cells.get(&k).is_none() {
            let m: Node = Node::Map(vec![("$type".to_string(), Node::Str(respawn::CELL_TYPE.to_string())), ("ColumnKey".to_string(), Node::Str("Data".to_string())), ("RowKey".to_string(), Node::Str(r.to_string()))]);
            if !cells.set(&k, m) {
                return Err(E::Format("volvagia roams: the cell list of the Volvagia death table is not a map"));
            }
        }
        let c: &mut Node = cells.get_mut(&k).ok_or(E::Format("volvagia roams: a cell of the Volvagia death table is missing"))?;
        let ok: bool = c.set(DESTROY, Node::Bool(true)) && c.set(WAIT, Node::Bool(false)) && c.set(DROP, Node::Bool(true)) && c.set(respawn::KEPT, Node::Bool(false)) && c.set(respawn::SAVED, Node::Bool(false));
        if !ok {
            return Err(E::Format("volvagia roams: a cell of the Volvagia death table is not a map"));
        }
    }
    y.write()
}

pub fn table(base: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    let c: &mut Node = comps(&mut y)?;
    expect(c, "DungeonBossCommonParam", &format!("?{}", boss_file(BASE)))?;
    expect(c, "DungeonBossVolvagiaParam", &format!("?{}", volvagia_file(BASE)))?;
    c.set("DungeonBossCommonParam", Node::Str(format!("?{}", boss_file(CLASS))));
    c.set("DungeonBossVolvagiaParam", Node::Str(format!("?{}", volvagia_file(CLASS))));
    y.write()
}

fn condition(n: &Node) -> Option<Node> {
    match n.get(CONDITION)? {
        Node::U32(v) => Some(Node::U32(*v)),
        Node::I32(v) => Some(Node::I32(*v)),
        _ => None,
    }
}

pub fn boss(base: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    let ev: &Node = y.root.get(EVENTS).ok_or(E::Format("volvagia roams: the boss parameters start no events, the game files differ"))?;
    let appear: &Node = ev.get(APPEAR).ok_or(E::Format("volvagia roams: the boss parameters have no Appear event, the game files differ"))?;
    let die: &Node = ev.get(DIE).ok_or(E::Format("volvagia roams: the boss parameters have no Die event, the game files differ"))?;
    let quiet: Node = condition(appear).ok_or(E::Format("volvagia roams: the Appear event has no start condition"))?;
    let loud: Node = condition(die).ok_or(E::Format("volvagia roams: the Die event has no start condition"))?;
    if quiet == loud {
        return Err(E::Format("volvagia roams: the Appear and Die events start the same way, the game files differ"));
    }
    let mut d: Node = Node::Map(Vec::new());
    d.set(EVENT_NAME, Node::Str(String::new()));
    d.set(CONDITION, quiet);
    y.root.set(EVENTS, Node::Map(vec![(DIE.to_string(), d)]));
    y.write()
}

pub fn widen(base: &[u8], scale: f32) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(base)?;
    for k in WIDE {
        let v: f32 = match y.root.get(k) {
            Some(Node::F32(v)) => *v,
            _ => return Err(E::Msg(format!("volvagia roams: the Volvagia parameters have no {k}, the game files differ"))),
        };
        y.root.set(k, Node::F32(v * scale));
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
        return Err(E::Msg(format!("volvagia roams: a drop file has no {list}, the game files differ")));
    }
    y.root.set(list, Node::Arr(vec![Node::Str(work(to))]));
    if !name {
        y.root.remove("DropTableName");
    }
    y.write()
}

pub fn drop_files(donor: &[Entry]) -> Res<Vec<Entry>> {
    let take = |n: &str| -> Res<Entry> { donor.iter().find(|e| e.name == n).cloned().ok_or_else(|| E::Msg(format!("volvagia roams: the {FROM} pack has no {n}, the game files differ"))) };
    let mut out: Vec<Entry> = vec![take(DROP_DEFAULT)?, take(DROP_BOARD)?];
    out.push(Entry { name: DROP_PARAM_CLONE.to_string(), data: retarget(&take(DROP_PLAIN)?.data, "DropTableResourceList", DROP_TABLE_CLONE, true)? });
    out.push(Entry { name: DROP_TABLE_CLONE.to_string(), data: retarget(&take(DROP_TABLE_NONE)?.data, "SubTableList", DROP_SUB_CLONE, false)? });
    let mut sub: Byml = Byml::parse(&take(DROP_SUB_NONE)?.data)?;
    sub.root.set("Items", Node::Arr(vec![rupee_item()]));
    out.push(Entry { name: DROP_SUB_CLONE.to_string(), data: sub.write()? });
    Ok(out)
}

pub fn build(base: &[Entry], scale: f32, donor: &[Entry], m: Mode) -> Res<Vec<Entry>> {
    if base.iter().any(|e| e.name.contains(CLASS)) {
        return Err(E::Msg(format!("volvagia roams: the {BASE} pack already holds files named {CLASS}")));
    }
    let mut out: Vec<Entry> = base.to_vec();
    for e in drop_files(donor)? {
        if !out.iter().any(|x| x.name == e.name) {
            out.push(e);
        }
    }
    let d: Vec<u8> = die_graph::die(&get(base, DIE_MODULE)?.data, m)?;
    out.iter_mut().find(|e| e.name == DIE_MODULE).ok_or_else(|| E::Msg(format!("volvagia roams: the {BASE} pack has no {DIE_MODULE}, the game files differ")))?.data = d;
    if m.holds() {
        out.push(Entry { name: HOLDER_CLONE.to_string(), data: die_graph::holder(&get(base, HOLDER_OWN)?.data)? });
    }
    out.push(Entry { name: actor_file(CLASS), data: actor_param(&get(base, &actor_file(BASE))?.data, m)? });
    out.push(Entry { name: table_file(CLASS), data: table(&get(base, &table_file(BASE))?.data)? });
    out.push(Entry { name: boss_file(CLASS), data: boss(&get(base, &boss_file(BASE))?.data)? });
    out.push(Entry { name: volvagia_file(CLASS), data: widen(&get(base, &volvagia_file(BASE))?.data, scale)? });
    out.push(Entry { name: DEATH_CLONE.to_string(), data: death(&get(base, DEATH_OWN)?.data)? });
    get(base, &BOSS_SHOW[1..])?;
    Ok(out)
}

pub fn rows(info: &mut Byml, game: &mut Byml, base_len: usize, len: usize, extra: i32) -> Res<usize> {
    let mut a: Node = rows::need(info, BASE, ACTOR_INFO)?.clone();
    let own: i32 = rows::own_heap(info, BASE)?.checked_add(extra).ok_or(E::Format("volvagia roams: the heap size does not fit"))?;
    let h: i32 = heap(own, base_len, len)?;
    let v: Node = match a.get(HEAP) {
        Some(Node::U32(_)) => Node::U32(h.max(0) as u32),
        _ => Node::I32(h),
    };
    a.set(HEAP, v);
    a.set(ROW_ID, Node::Str(CLASS.to_string()));
    let mut g: Node = rows::need(game, BASE, GAME_ACTOR_INFO)?.clone();
    g.set(ROW_ID, Node::Str(CLASS.to_string()));
    if rows::find(info, CLASS).is_some() || rows::find(game, CLASS).is_some() {
        return Err(E::Msg(format!("volvagia roams: the actor tables already hold {CLASS}")));
    }
    let i: usize = rows::insert(info, a)?;
    let j: usize = rows::insert(game, g)?;
    if i != j {
        return Err(E::Msg(format!("volvagia roams: {CLASS} sits at {i} in the actor table and at {j} in the game actor table")));
    }
    Ok(i)
}

pub fn tags(t: &mut Table) -> Res<()> {
    let own: Vec<String> = t.tags_of(BASE);
    if own.is_empty() {
        return Err(E::Msg(format!("volvagia roams: {BASE} has no tag row, the game files differ")));
    }
    if t.find(CLASS).is_some() {
        return Err(E::Msg(format!("volvagia roams: the tag table already holds {CLASS}")));
    }
    t.insert(CLASS, &own)?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    pub start: [f32; 3],
    pub points: Vec<(String, [f32; 3])>,
}

fn text<'a>(n: &'a Node, k: &str) -> &'a str {
    n.get(k).and_then(|v| v.as_str()).unwrap_or("")
}

fn dst(l: &Node) -> Option<u64> {
    match l.get("Dst")? {
        Node::U64(x) => Some(*x),
        Node::I64(x) => Some(*x as u64),
        _ => None,
    }
}

pub fn ring(stage: &Byml) -> Res<Ring> {
    let acts: &[Node] = stage.root.get("Actors").and_then(|a| a.as_arr()).ok_or(E::Format("volvagia roams: the Volvagia stage has no actor list"))?;
    let bosses: Vec<&Node> = acts.iter().filter(|a| text(a, "Gyaml") == BASE).collect();
    let [b] = bosses.as_slice() else {
        return Err(E::Msg(format!("volvagia roams: the Volvagia stage holds {} {BASE} and not one, the game files differ", bosses.len())));
    };
    let links: &[Node] = b.get("Links").and_then(|l| l.as_arr()).unwrap_or(&[]);
    let mut points: Vec<(String, [f32; 3])> = Vec::new();
    for name in HOLES.iter().chain([CENTRE].iter()) {
        let hits: Vec<u64> = links.iter().filter(|l| text(l, "Name") == *name).filter_map(dst).collect();
        let [h] = hits.as_slice() else {
            return Err(E::Msg(format!("volvagia roams: Volvagia has {} links named {name} and not one, the game files differ", hits.len())));
        };
        let a: &Node = acts.iter().find(|a| hash_of(a) == Some(*h)).ok_or_else(|| E::Msg(format!("volvagia roams: the {name} link points at nothing")))?;
        if text(a, "Gyaml") != ANCHOR {
            return Err(E::Msg(format!("volvagia roams: the {name} link points at {} and not an anchor", text(a, "Gyaml"))));
        }
        let at: [f32; 3] = vec3(a, "Translate").ok_or(E::Format("volvagia roams: an anchor has no position"))?;
        points.push((name.to_string(), at));
    }
    if links.iter().any(|l| text(l, "Name") == "Four") {
        return Err(E::Format("volvagia roams: Volvagia now has a hole named Four, the game files differ"));
    }
    let centre: [f32; 3] = points.last().map(|p| p.1).ok_or(E::Format("volvagia roams: no room centre"))?;
    let flat: Vec<(String, [f32; 3])> = points.into_iter().map(|(n, p)| (n, [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]])).collect();
    let start: [f32; 3] = vec3(b, "Translate").ok_or(E::Format("volvagia roams: Volvagia has no position"))?;
    Ok(Ring { start: [start[0] - centre[0], start[1] - centre[1], start[2] - centre[2]], points: flat })
}

fn link(src: u64, dst: u64, name: &str) -> Node {
    let mut l: Node = Node::Map(Vec::new());
    l.set("Dst", Node::U64(dst));
    l.set("Gyaml", Node::Str("Reference".to_string()));
    l.set("Name", Node::Str(name.to_string()));
    l.set("Src", Node::U64(src));
    l
}

fn next(raws: &[&[u8]], taken: &mut Vec<u64>, seed: &str) -> u64 {
    let mut n: u32 = 0;
    loop {
        let h: u64 = fresh(raws, &format!("{seed} {n}"));
        if !taken.contains(&h) {
            taken.push(h);
            return h;
        }
        n += 1;
    }
}

pub fn presence() -> Node {
    Node::Map(vec![("FlagName".to_string(), Node::Str(BEATEN.to_string())), ("IsCheckLazy".to_string(), Node::Bool(false)), ("IsNegation".to_string(), Node::Bool(true))])
}

pub fn place(y: &mut Byml, r: &Ring, s: &Shape, raws: &[&[u8]]) -> Res<Vec<u64>> {
    let mut taken: Vec<u64> = Vec::new();
    let seed: String = format!("{SEED} {}", s.arena.file);
    let body: u64 = next(raws, &mut taken, &format!("{seed} body"));
    let c: [f32; 3] = s.arena.at;
    let mut nodes: Vec<Node> = Vec::new();
    let mut links: Vec<Node> = Vec::new();
    let mut all: Vec<u64> = vec![body];
    for (name, off) in &r.points {
        let h: u64 = next(raws, &mut taken, &format!("{seed} {name}"));
        let at: [f32; 3] = [c[0] + off[0] * s.scale, c[1] + s.lift, c[2] + off[2] * s.scale];
        let mut a: Node = actor(ANCHOR, h, at, [0.0; 3]);
        a.remove("Rotate");
        nodes.push(a);
        links.push(link(body, h, name));
        all.push(h);
    }
    let mut b: Node = actor(CLASS, body, [c[0] + r.start[0] * s.scale, c[1] + s.lift, c[2] + r.start[2] * s.scale], [0.0; 3]);
    b.set(PRESENCE, presence());
    b.set("Links", Node::Arr(links));
    let acts: &mut Vec<Node> = y.root.get_mut("Actors").and_then(|a| a.as_arr_mut()).ok_or(E::Format("volvagia roams: the map has no actor list"))?;
    acts.push(b);
    acts.extend(nodes);
    all.sort();
    for g in GROUPS {
        if let Some(l) = y.root.get_mut(g).and_then(|a| a.as_arr_mut()) {
            l.push(Node::Arr(all.iter().map(|h| Node::U64(*h)).collect()));
        }
    }
    Ok(all)
}

#[derive(Clone, Debug, Default)]
pub struct Made {
    pub outs: Vec<Out>,
    pub sizes: Vec<(String, u32)>,
    pub grown: Vec<(String, usize, usize)>,
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

pub fn stage(rf: &mut Romfs, have: &dyn Fn(&str) -> Option<Vec<u8>>, rst: &Rst, s: &Shape, dm: Mode) -> Res<Made> {
    let mut m: Made = Made::default();
    if rf.root.join(pack_of(CLASS)).is_file() {
        return Err(E::Msg(format!("volvagia roams: the game already ships {CLASS}")));
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
    let es: Vec<Entry> = build(&base, s.scale, &donor, dm)?;
    let raw: Vec<u8> = sarc::write(&es)?;
    for e in &es {
        if let Some(b) = base.iter().find(|b| b.name == e.name).filter(|b| e.data.len() > b.data.len()) {
            m.grown.push((e.name.clone(), b.data.len(), e.data.len()));
        }
    }
    let near: Vec<(String, usize)> = base.iter().chain(donor.iter()).map(|e| (e.name.clone(), e.data.len())).collect();
    for p in owned(dm) {
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
    rows(&mut info, &mut game, braw.len(), raw.len(), if dm.holds() { HEAP_ROOM } else { 0 })?;
    for (p, y, old, id) in [(ACTOR_INFO, &info, iraw.len(), iid), (GAME_ACTOR_INFO, &game, graw.len(), gid)] {
        let out: Vec<u8> = y.write()?;
        m.outs.push(Out { path: p.to_string(), z: zs::compress_raw(&out, Some(id))?, old, new: out.len() });
    }
    let (traw, tid): (Vec<u8>, u32) = current(rf, have, TAG)?;
    let mut t: Table = Table::parse(&traw)?;
    tags(&mut t)?;
    let tout: Vec<u8> = t.write()?;
    m.outs.push(Out { path: TAG.to_string(), z: zs::compress_raw(&tout, Some(tid))?, old: traw.len(), new: tout.len() });
    let tp: String = respawn::file(DEATH_TABLE_BASE);
    let traw2: Vec<u8> = match have(&tp) {
        Some(b) => b,
        None => rf.read(&tp)?,
    };
    let dt: Vec<u8> = procedures(&traw2)?;
    m.sizes.push((death_table(), table_size(rst, &tp, &rf.read(&tp)?, &dt)?));
    m.outs.push(Out { path: death_table(), old: 0, new: dt.len(), z: dt });
    let r: Ring = ring(&rf.read_byml(STAGE)?)?;
    let (mraw, mid): (Vec<u8>, u32) = current(rf, have, s.arena.file)?;
    let dynamic: String = s.arena.file.replace("_Static.", "_Dynamic.");
    let land: Vec<u8> = current(rf, have, &dynamic).map(|x| x.0).unwrap_or_default();
    let mut y: Byml = Byml::parse(&mraw)?;
    let placed: Vec<u64> = place(&mut y, &r, s, &[&mraw, &land])?;
    let mout: Vec<u8> = y.write()?;
    m.outs.push(Out { path: s.arena.file.to_string(), z: zs::compress_raw(&mout, Some(mid))?, old: mraw.len(), new: mout.len() });
    m.lines.push(format!("{CLASS}: new actor class, pack {} bytes with {} files ({} of its own, {} grown) and its own death table, death {dm:?}; it is not placed once {BEATEN} is set", raw.len(), es.len(), owned(dm).len(), m.grown.len()));
    m.lines.push(format!("{}: {CLASS} and {} anchors placed around {:?}, lift {}, scale {} (actor {})", s.arena.file, placed.len() - 1, s.arena.at, s.lift, s.scale, placed.first().map(|h| h.to_string()).unwrap_or_default()));
    Ok(m)
}

pub fn sized(rf: &mut Romfs, t: &mut Rst, m: &Made) -> Res<usize> {
    let mut n: usize = 0;
    for (p, sz) in &m.sizes {
        t.set_size(p, *sz)?;
        n += 1;
    }
    for (p, old, new) in &m.grown {
        if t.grow(p, *old as u64, *new as u64, MARGIN)?.is_none() {
            return Err(E::Msg(format!("volvagia roams: {p} has no size table entry to grow")));
        }
        n += 1;
    }
    for o in m.outs.iter().filter(|o| !m.sizes.iter().any(|s| s.0 == o.path)) {
        n += grow_out(rf, t, o)?;
    }
    Ok(n)
}

pub fn parent_of(p: &str) -> String {
    match p {
        DROP_PARAM_CLONE => DROP_PLAIN.to_string(),
        DROP_TABLE_CLONE => DROP_TABLE_NONE.to_string(),
        DROP_SUB_CLONE => DROP_SUB_NONE.to_string(),
        _ => p.replace(CLASS, BASE),
    }
}

pub fn sources(s: &Shape) -> Vec<String> {
    vec![pack_of(BASE), pack_of(FROM), ACTOR_INFO.to_string(), GAME_ACTOR_INFO.to_string(), TAG.to_string(), respawn::file(DEATH_TABLE_BASE), STAGE.to_string(), s.arena.file.to_string()]
}

pub fn write(rf: &mut Romfs, pk: &Pack, rst: &mut Option<Rst>) -> Res<(usize, Vec<String>)> {
    let root: PB = pk.work.join("romfs");
    if rst.is_none() {
        *rst = Some(rsizetable::load(rf)?);
    }
    let t: &mut Rst = rst.as_mut().ok_or(E::Format("volvagia roams: no size table"))?;
    let have = |p: &str| -> Option<Vec<u8>> { fs::read(p.split('/').fold(root.clone(), |a, s| a.join(s))).ok() };
    let m: Made = stage(rf, &have, t, &SHAPE, die_graph::MODE)?;
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

    #[test]
    fn the_shipped_shape_is_the_field_unlifted_and_unscaled() {
        assert_eq!(SHAPE, Shape { arena: FIELD, lift: 0.0, scale: 1.0 });
        assert_eq!(work("Actor/X.engine__actor__ActorParam.bgyml"), "Work/Actor/X.engine__actor__ActorParam.gyml");
        assert_eq!(own().len(), 8);
        assert!(death_table().ends_with("_game__death__DeathReasonProcedures.pp__CombinationDataTableData.bgyml") && death_table().contains(CLASS));
        assert!(own().iter().all(|p| p.contains(CLASS)));
        assert!(CLASS.len() <= BASE.len() && !CLASS.starts_with(BASE));
        assert_eq!(TEST.file.replace("_Static.", "_Dynamic."), "Banc/Market/Market_Adult_Dynamic.bcett.byml.zs");
    }

    #[test]
    fn the_clone_pack_is_the_volvagia_pack_plus_eight_files_of_its_own_and_the_two_shared_drop_files() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        assert!(!r.join(pack_of(CLASS)).is_file());
        let base: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of(BASE)).unwrap()).unwrap();
        let dn: Vec<Entry> = donor(&mut rf);
        let es: Vec<Entry> = build(&base, 2.0, &dn, Mode::Spawn).unwrap();
        assert_eq!(es.len(), base.len() + 11);
        let back: Vec<Entry> = sarc::parse(&sarc::write(&es).unwrap()).unwrap();
        for e in base.iter().filter(|e| e.name != DIE_MODULE) {
            assert_eq!(sarc::get(&back, &e.name).map(|x| &x.data), Some(&e.data), "{}", e.name);
        }
        assert_eq!(sarc::get(&back, DIE_MODULE).map(|x| x.data.clone()), Some(die_graph::die(&sarc::get(&base, DIE_MODULE).unwrap().data, Mode::Spawn).unwrap()));
        let a: Byml = y(&back, &actor_file(CLASS));
        assert_eq!(s(&a.root, &["$parent"]), "Work/Actor/Enemy_DungeonBoss_Volvagia.engine__actor__ActorParam.gyml");
        assert_eq!(s(&a.root, &["Components", "GameParameterTableRef"]), format!("?{}", table_file(CLASS)));
        assert_eq!(s(&a.root, &["Components", "BattlePresentationRef"]), BOSS_SHOW);
        assert_eq!(s(&a.root, &["Components", DEATH_KEY]), format!("?{DEATH_CLONE}"));
        assert_eq!(sarc::get(&back, DEATH_OWN).map(|x| &x.data), sarc::get(&base, DEATH_OWN).map(|x| &x.data));
        let d: Byml = y(&back, DEATH_CLONE);
        assert!(!format!("{:?}", d.root).contains(BASE));
        assert_eq!(s(&d.root, &[PROCEDURES]), work(&death_table()));
        assert_eq!(s(&d.root, &["$parent"]), s(&y(&base, DEATH_OWN).root, &["$parent"]));
        assert_eq!(a.root.get("Components").and_then(|c| if let Node::Map(m) = c { Some(m.len()) } else { None }), Some(5));
        assert_eq!(s(&a.root, &["Components", HOLDER_KEY]), format!("?{HOLDER_CLONE}"));
        assert_eq!(sarc::get(&back, HOLDER_OWN).map(|x| &x.data), sarc::get(&base, HOLDER_OWN).map(|x| &x.data));
        assert_eq!(sarc::get(&back, HOLDER_CLONE).map(|x| x.data.clone()), Some(die_graph::holder(&sarc::get(&base, HOLDER_OWN).unwrap().data).unwrap()));
        let pay: Vec<Entry> = build(&base, 2.0, &dn, Mode::Pay).unwrap();
        assert_eq!(pay.len(), base.len() + 10);
        assert!(sarc::get(&pay, HOLDER_CLONE).is_none());
        assert!(y(&pay, &actor_file(CLASS)).root.get("Components").and_then(|c| c.get(HOLDER_KEY)).is_none());
        assert_eq!(s(&a.root, &["Components", DROP_KEY]), format!("?{DROP_PARAM_CLONE}"));
        let t: Byml = y(&back, &table_file(CLASS));
        assert_eq!(s(&t.root, &["Components", "DungeonBossCommonParam"]), format!("?{}", boss_file(CLASS)));
        assert_eq!(s(&t.root, &["Components", "DungeonBossVolvagiaParam"]), format!("?{}", volvagia_file(CLASS)));
        assert_eq!(s(&t.root, &["Components", "EnemyCommonParam"]), "?GameParameter/EnemyCommonParam/Enemy_DungeonBoss_Volvagia.game__enemy__EnemyCommonParam.bgyml");
        let old: Byml = y(&base, &boss_file(BASE));
        let b: Byml = y(&back, &boss_file(CLASS));
        let ev: &Node = b.root.get(EVENTS).unwrap();
        assert!(ev.get(APPEAR).is_none());
        assert_eq!(s(ev, &[DIE, EVENT_NAME]), "");
        assert_eq!(ev.get(DIE).and_then(condition), old.root.get(EVENTS).and_then(|e| e.get(APPEAR)).and_then(condition));
        assert_ne!(ev.get(DIE).and_then(condition), old.root.get(EVENTS).and_then(|e| e.get(DIE)).and_then(condition));
        let txt: String = format!("{:?}", b.root);
        assert!(!txt.contains("SageOfFire") && !txt.contains("BossIn") && txt.contains("ChanceTimeParameters"), "{txt}");
        assert_eq!(s(&b.root, &["$parent"]), s(&old.root, &["$parent"]));
        let w: Byml = y(&back, &volvagia_file(CLASS));
        let wo: Byml = y(&base, &volvagia_file(BASE));
        for k in WIDE {
            match (w.root.get(k), wo.root.get(k)) {
                (Some(Node::F32(a)), Some(Node::F32(b))) => assert_eq!(*a, *b * 2.0, "{k}"),
                _ => panic!("{k}"),
            }
        }
        assert_eq!(format!("{:?}", w.root.get("AirDiveHeight")), format!("{:?}", wo.root.get("AirDiveHeight")));
        assert!(build(&es, 1.0, &dn, Mode::Spawn).is_err());
    }

    #[test]
    fn the_ring_is_the_dream_world_holes_around_their_centre() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let g: Ring = ring(&rf.read_byml(STAGE).unwrap()).unwrap();
        assert_eq!(g.points.len(), 9);
        assert_eq!(g.points[8], (CENTRE.to_string(), [0.0, 0.0, 0.0]));
        for (n, p) in &g.points[..8] {
            let d: f32 = (p[0] * p[0] + p[2] * p[2]).sqrt();
            assert!(d > 4.0 && d < 16.0 && p[1].abs() < 0.01, "{n} {p:?}");
        }
        assert!(g.start[1] < -5.0, "{:?}", g.start);
        let mut ft: Romfs = Romfs::open(&r).unwrap();
        let fire: Ring = ring(&ft.read_byml("Banc/FireTemple/FireTemple_Static.bcett.byml.zs").unwrap()).unwrap();
        for (a, b) in fire.points.iter().zip(g.points.iter()) {
            assert_eq!(a.0, b.0);
            assert!((0..3).all(|i| (a.1[i] - b.1[i]).abs() < 0.01), "{a:?} {b:?}");
        }
    }

    #[test]
    fn the_rows_and_tags_copy_volvagia_under_the_new_name() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let mut info: Byml = rf.read_byml(ACTOR_INFO).unwrap();
        let mut game: Byml = rf.read_byml(GAME_ACTOR_INFO).unwrap();
        let n: usize = info.root.as_arr().unwrap().len();
        rows(&mut info, &mut game, 1000, 1800, 0).unwrap();
        assert_eq!(info.root.as_arr().unwrap().len(), n + 1);
        assert!(rows::sorted(&info).unwrap() && rows::sorted(&game).unwrap());
        let a: &Node = rows::find(&info, CLASS).unwrap();
        assert_eq!(s(a, &["ActorName"]), BASE);
        assert_eq!(s(a, &["ModelProjectName"]), BASE);
        assert!(rows::own_heap(&info, CLASS).unwrap() >= rows::own_heap(&info, BASE).unwrap() + 800);
        assert_eq!(rows::find(&game, CLASS).and_then(|g| g.get("MaxLife")).map(|v| format!("{v:?}")), rows::find(&game, BASE).and_then(|g| g.get("MaxLife")).map(|v| format!("{v:?}")));
        assert!(rows(&mut info, &mut game, 1000, 1800, 0).is_err());
        let mut t: Table = Table::parse(&rf.read_zs(TAG).unwrap()).unwrap();
        tags(&mut t).unwrap();
        assert_eq!(t.tags_of(CLASS), t.tags_of(BASE));
        assert!(Table::parse(&t.write().unwrap()).unwrap().find(CLASS).is_some());
        assert!(tags(&mut t).is_err());
    }

    #[test]
    fn each_stage_places_one_volvagia_with_its_own_anchors_and_sizes_every_new_file() {
        let Some(r) = root() else { return };
        let none = |_: &str| -> Option<Vec<u8>> { None };
        for (arena, lift, scale) in [(TEST, 0.0, 1.0), (FIELD, 12.0, 2.0)] {
            let mut rf: Romfs = Romfs::open(&r).unwrap();
            let t: Rst = rsizetable::load(&mut rf).unwrap();
            let sh: Shape = Shape { arena, lift, scale };
            let m: Made = stage(&mut rf, &none, &t, &sh, Mode::Spawn).unwrap();
            let paths: Vec<&str> = m.outs.iter().map(|o| o.path.as_str()).collect();
            assert_eq!(paths, vec![pack_of(CLASS).as_str(), ACTOR_INFO, GAME_ACTOR_INFO, TAG, death_table().as_str(), arena.file]);
            assert_eq!(m.sizes.len(), 11);
            for (p, sz) in &m.sizes {
                assert!(*sz > 0 && t.get(p).is_none(), "{p}");
            }
            let o: &Out = m.outs.iter().find(|o| o.path == arena.file).unwrap();
            let y: Byml = Byml::parse(&rf.zs.decompress(&o.z).unwrap()).unwrap();
            let old: Byml = rf.read_byml(arena.file).unwrap();
            let acts: &[Node] = y.root.get("Actors").unwrap().as_arr().unwrap();
            assert_eq!(acts.len(), old.root.get("Actors").unwrap().as_arr().unwrap().len() + 10);
            let b: Vec<&Node> = acts.iter().filter(|a| text(a, "Gyaml") == CLASS).collect();
            assert_eq!(b.len(), 1);
            assert!(acts.iter().all(|a| text(a, "Gyaml") != BASE));
            assert!(b[0].get("Dynamic").is_none());
            assert_eq!(b[0].get(PRESENCE), Some(&presence()));
            assert_eq!(s(b[0].get(PRESENCE).unwrap(), &["FlagName"]), BEATEN);
            assert_eq!(b[0].get(PRESENCE).and_then(|p| p.get("IsNegation")), Some(&Node::Bool(true)));
            let links: &[Node] = b[0].get("Links").unwrap().as_arr().unwrap();
            assert_eq!(links.len(), 9);
            for l in links {
                assert!(acts.iter().find(|a| hash_of(a) == dst(l)).unwrap().get(PRESENCE).is_none());
            }
            let mut far: f32 = 0.0;
            for l in links {
                let a: &Node = acts.iter().find(|a| hash_of(a) == dst(l)).unwrap();
                assert_eq!(text(a, "Gyaml"), ANCHOR);
                let p: [f32; 3] = vec3(a, "Translate").unwrap();
                assert!((p[1] - arena.at[1] - lift).abs() < 0.01);
                far = far.max(((p[0] - arena.at[0]).powi(2) + (p[2] - arena.at[2]).powi(2)).sqrt());
            }
            assert!(far > 10.0 * scale && far < 16.0 * scale, "{far}");
            let mut hs: Vec<u64> = acts.iter().filter_map(hash_of).collect();
            let n: usize = hs.len();
            hs.sort();
            hs.dedup();
            assert_eq!(hs.len(), n);
            for g in GROUPS {
                let a: usize = y.root.get(g).and_then(|x| x.as_arr()).map(|x| x.len()).unwrap_or(0);
                let b: usize = old.root.get(g).and_then(|x| x.as_arr()).map(|x| x.len()).unwrap_or(0);
                assert_eq!(a, b + 1, "{g}");
            }
            let pk: Vec<Entry> = sarc::parse(&rf.zs.decompress(&m.outs[0].z).unwrap()).unwrap();
            assert!(own().iter().all(|p| sarc::get(&pk, p).is_some()));
        }
    }

    #[test]
    fn the_field_spot_is_adult_only_flat_open_ground_far_from_the_other_features() {
        assert!(FIELD.file.contains("HyruleField_Adult") && !FIELD.file.contains("Child"));
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let g: crate::difficulty::ground::Ground = crate::difficulty::ground::load(&mut rf, "HyruleField_Adult").unwrap().unwrap();
        let c: [f32; 3] = FIELD.at;
        let (y, _): (f32, u32) = g.floor_near(c[0], c[1], c[2], 1.0, 1.0).unwrap();
        assert!((y - c[1]).abs() < 0.5, "{y}");
        assert!(g.clear(c) >= 12.0);
        let rg: Ring = ring(&rf.read_byml(STAGE).unwrap()).unwrap();
        for (n, off) in &rg.points {
            assert!(g.floor_near(c[0] + off[0], c[1], c[2] + off[2], 4.0, 4.0).is_some(), "{n}");
        }
        for rad in [10.0f32, 20.0, 30.0, 40.0, 50.0, 60.0] {
            for k in 0..16 {
                let a: f32 = k as f32 * std::f32::consts::FRAC_PI_8;
                let f: Option<(f32, u32)> = g.floor_near(c[0] + rad * a.cos(), c[1], c[2] + rad * a.sin(), 8.0, 8.0);
                assert!(f.is_some(), "{rad} {k}");
            }
        }
        let keep_out: [([f32; 2], &str); 12] = [
            ([0.0, 53.6], "Market gate"),
            ([220.0, 140.7], "east exit"),
            ([210.8, 10.0], "north east exit"),
            ([-314.7, 160.8], "west exit"),
            ([200.1, 309.0], "south east exit"),
            ([202.7, 148.4], "east arrival"),
            ([-53.2, 208.6], "ranch arrival"),
            ([75.0, 95.0], "fireball storm, middle road"),
            ([55.0, 190.0], "fireball storm, south central"),
            ([-170.0, 170.0], "fireball storm, west road"),
            ([134.0, 168.5], "Iron Knuckle"),
            ([-60.0, 180.0], "Lon Lon Ranch gate"),
        ];
        for (p, what) in keep_out {
            let d: f32 = ((p[0] - c[0]).powi(2) + (p[1] - c[2]).powi(2)).sqrt();
            assert!(d >= 60.0, "{what} is {d} away");
        }
    }

    #[test]
    fn the_clone_takes_the_scaled_life_and_attack_of_the_staged_volvagia_pack_and_keeps_volvagias_own_death_rules_except_the_body() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let none = |_: &str| -> Option<Vec<u8>> { None };
        let o: crate::difficulty::Opts = crate::difficulty::Opts { health: 50, damage: 200, ..crate::difficulty::Opts::default() };
        let outs: Vec<Out> = crate::difficulty::stage(&mut rf, &o, &none, "seed").unwrap();
        let staged: Out = outs.into_iter().find(|x| x.path == pack_of(BASE)).unwrap();
        let t: Rst = rsizetable::load(&mut rf).unwrap();
        let have = |p: &str| -> Option<Vec<u8>> { if p == pack_of(BASE) { Some(staged.z.clone()) } else { None } };
        let m: Made = stage(&mut rf, &have, &t, &Shape { arena: FIELD, lift: 0.0, scale: 1.0 }, Mode::Spawn).unwrap();
        let clone: Vec<Entry> = sarc::parse(&rf.zs.decompress(&m.outs[0].z).unwrap()).unwrap();
        let plain: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of(BASE)).unwrap()).unwrap();
        let life: String = format!("{}Enemy_DungeonBoss_Volvagia{}", crate::difficulty::stats::LIFE_DIR, crate::difficulty::stats::LIFE_EXT);
        let atk: String = format!("{}Enemy_DungeonBoss_Volvagia{}", crate::difficulty::stats::ATTACK_DIR, crate::difficulty::stats::ATTACK_EXT);
        let hp = |es: &[Entry]| -> i32 { if let Some(Node::I32(v)) = y(es, &life).root.get("MaxLife") { *v } else { panic!() } };
        assert_eq!(hp(&plain), 60);
        assert_eq!(hp(&clone), 30);
        assert_ne!(sarc::get(&clone, &atk).unwrap().data, sarc::get(&plain, &atk).unwrap().data);
        let d: Byml = y(&clone, &actor_file(CLASS));
        assert_eq!(s(&d.root, &["Components", DEATH_KEY]), format!("?{DEATH_CLONE}"));
        let own: &Out = m.outs.iter().find(|o| o.path == death_table()).unwrap();
        assert_eq!(own.old, 0);
        let plain_table: Vec<u8> = rf.read(&respawn::file(DEATH_TABLE_BASE)).unwrap();
        assert_ne!(own.z, plain_table);
        let made: Byml = Byml::parse(&own.z).unwrap();
        let cell: &Node = made.root.get(respawn::CELLS).and_then(|c| c.get(respawn::CELL)).unwrap();
        assert_eq!(cell.get(respawn::KEPT), Some(&Node::Bool(false)));
        assert_eq!(cell.get(respawn::SAVED), Some(&Node::Bool(false)));
        let game: Byml = Byml::parse(&plain_table).unwrap();
        assert_eq!(s(&made.root, &["$parent"]), s(&game.root, &["$parent"]));
        let (Some(Node::Map(want)), Some(Node::Map(got))) = (game.root.get(respawn::CELLS), made.root.get(respawn::CELLS)) else { panic!() };
        let mine_keys: Vec<String> = ENABLED.iter().map(|r| format!("{r}___Data")).collect();
        for k in &mine_keys {
            let c: &Node = made.root.get(respawn::CELLS).and_then(|c| c.get(k)).unwrap();
            assert_eq!(c.get(DROP), Some(&Node::Bool(true)), "{k}");
            assert_eq!(c.get(WAIT), Some(&Node::Bool(false)), "{k}");
            assert_eq!(c.get(DESTROY), Some(&Node::Bool(true)), "{k}");
            assert_eq!(c.get(respawn::KEPT), Some(&Node::Bool(false)), "{k}");
            assert_eq!(c.get(respawn::SAVED), Some(&Node::Bool(false)), "{k}");
            assert_eq!(c.get("IsIgnoreThisDeathReason"), want.iter().find(|(n, _)| n == k).and_then(|(_, v)| v.get("IsIgnoreThisDeathReason")), "{k}");
            assert_eq!(c.get("$type").and_then(|t| t.as_str()), Some(respawn::CELL_TYPE), "{k}");
        }
        for (k, v) in want {
            if mine_keys.contains(k) {
                continue;
            }
            assert_eq!(made.root.get(respawn::CELLS).and_then(|c| c.get(k)).unwrap(), v, "{k}");
        }
        assert_eq!(got.len(), want.len() + mine_keys.iter().filter(|k| !want.iter().any(|(n, _)| n == *k)).count());
        let c: &Node = made.root.get(respawn::CELLS).and_then(|c| c.get("UpperCrushed___Data")).unwrap();
        assert_eq!(c.get("RowKey").and_then(|t| t.as_str()), Some("UpperCrushed"));
        assert_eq!(c.get("ColumnKey").and_then(|t| t.as_str()), Some("Data"));
        for k in ["AI___Data", "Burned___Data", "LifeCondition___Data", "PhysicsKinematicCollision___Data"] {
            assert!(made.root.get(respawn::CELLS).and_then(|c| c.get(k)).is_some(), "{k}");
        }
        let txt: String = format!("{:?}{:?}", y(&clone, DEATH_CLONE).root, made.root);
        for bad in ["Event", "Sage", "BossIn", "Flag"] {
            assert!(!txt.replace("IsSetRespawnFlagToFalse", "").replace("IsSetGmdRespawnFlagToFalse", "").contains(bad), "{bad}");
        }
        let b: Byml = y(&clone, &boss_file(CLASS));
        assert_eq!(s(b.root.get(EVENTS).unwrap(), &[DIE, EVENT_NAME]), "");
    }

    #[test]
    fn the_clone_pack_keeps_every_staged_volvagia_file_so_hits_land_as_on_the_real_one() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let none = |_: &str| -> Option<Vec<u8>> { None };
        let o: crate::difficulty::Opts = crate::difficulty::Opts { health: 50, damage: 200, ..crate::difficulty::Opts::default() };
        let outs: Vec<Out> = crate::difficulty::stage(&mut rf, &o, &none, "seed").unwrap();
        let staged: Out = outs.into_iter().find(|x| x.path == pack_of(BASE)).unwrap();
        let st: Vec<Entry> = sarc::parse(&rf.zs.decompress(&staged.z).unwrap()).unwrap();
        let plain: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of(BASE)).unwrap()).unwrap();
        let changed: Vec<&str> = plain.iter().filter(|e| sarc::get(&st, &e.name).map(|x| &x.data) != Some(&e.data)).map(|e| e.name.as_str()).collect();
        assert!(changed.iter().all(|n| n.starts_with("Life/") || n.contains("Attack") || n.contains("Speed") || n.contains("Move")), "{changed:?}");
        assert!(!changed.iter().any(|n| n.contains("Damage") || n.contains("Guard") || n.contains("Weak") || n.contains("Death") || n.contains("Reaction")), "{changed:?}");
        let dn: Vec<Entry> = donor(&mut rf);
        let es: Vec<Entry> = build(&st, 1.0, &dn, Mode::Spawn).unwrap();
        for e in st.iter().filter(|e| e.name != DIE_MODULE) {
            assert_eq!(sarc::get(&es, &e.name).map(|x| &x.data), Some(&e.data), "{}", e.name);
        }
        let a: Byml = y(&es, &actor_file(CLASS));
        let Some(Node::Map(c)) = a.root.get("Components") else { panic!() };
        let keys: Vec<&str> = c.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec![HOLDER_KEY, "BattlePresentationRef", DEATH_KEY, DROP_KEY, "GameParameterTableRef"]);
        let own: Byml = y(&es, DEATH_CLONE);
        let game: Byml = y(&st, DEATH_OWN);
        assert_eq!(s(&own.root, &["$parent"]), s(&game.root, &["$parent"]));
        assert_eq!(s(&game.root, &[PROCEDURES]), work(&respawn::file(DEATH_TABLE_BASE)));
        for t in ["System/CombinationDataTableData/game__life__ExternalDamageReactionTable_Volvagia.pp__CombinationDataTableData.bgyml", "System/CombinationDataTableData/game__life__GuardReactionTable_Volvagia.pp__CombinationDataTableData.bgyml", &respawn::file(DEATH_TABLE_BASE)] {
            assert!(rf.read(t).is_ok(), "{t}");
        }
    }

    #[test]
    fn the_field_gate_has_the_form_the_game_uses_for_present_until_a_flag_is_set() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let mine: Node = presence();
        let Node::Map(m) = &mine else { panic!() };
        let shape = |n: &Node| -> Vec<(String, String)> {
            if let Node::Map(v) = n { v.iter().map(|(k, x)| (k.clone(), if let Node::Str(_) = x { "str".to_string() } else { format!("{x:?}") })).collect() } else { Vec::new() }
        };
        let mut seen: usize = 0;
        for f in ["Banc/BottomOfTheWell/BottomOfTheWell_Static.bcett.byml.zs", "Banc/DeathMountainCrater/DeathMountainCrater_Adult_Dynamic.bcett.byml.zs", "Banc/DeathMountainTrail/DeathMountainTrail_Adult_Dynamic.bcett.byml.zs"] {
            let y: Byml = rf.read_byml(f).unwrap();
            for a in y.root.get("Actors").and_then(|a| a.as_arr()).unwrap() {
                let Some(p) = a.get(PRESENCE) else { continue };
                if p.get("IsNegation") == Some(&Node::Bool(true)) && p.get("IsCheckLazy") == Some(&Node::Bool(false)) {
                    assert_eq!(shape(p), shape(&mine), "{f}");
                    seen += 1;
                }
            }
        }
        assert!(seen > 0);
        assert_eq!(m.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), vec!["FlagName", "IsCheckLazy", "IsNegation"]);
        assert_eq!(mine.get("IsNegation"), Some(&Node::Bool(true)));
    }

    #[test]
    fn every_enabled_death_reason_after_inheritance_is_on_the_list() {
        let Some(r) = root() else { return };
        let rf: Romfs = Romfs::open(&r).unwrap();
        let mut merged: Vec<(String, bool)> = Vec::new();
        for t in ["Default", "EnemyCommon", "DungeonBossCommon", DEATH_TABLE_BASE] {
            let y: Byml = Byml::parse(&rf.read(&respawn::file(t)).unwrap()).unwrap();
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
        let tp: String = respawn::file(DEATH_TABLE_BASE);
        let peer: Vec<u8> = rf.read(&tp).unwrap();
        let all: Vec<(String, Vec<u8>)> = game_tables(&r);
        assert!(all.len() > 100);
        let mut n: usize = 0;
        for (p, b) in &all {
            let Some(want) = t.get(p) else { continue };
            let got: u32 = table_size(&t, &tp, &peer, b).unwrap() - FILE_MARGIN as u32;
            assert!(got >= want, "{p}: {got} < {want}");
            let per: i64 = want as i64 - b.len() as i64 - 240 * cells(b).unwrap() as i64;
            assert!((5040..5400).contains(&per), "{p}: {per}");
            n += 1;
        }
        assert!(n > 100);
        let made: Vec<u8> = procedures(&peer).unwrap();
        let need: u64 = made.len() as u64 + 5400 + 240 * cells(&made).unwrap() as u64;
        assert!(table_size(&t, &tp, &peer, &made).unwrap() as u64 >= need + 512);
    }

    #[test]
    fn the_new_death_table_round_trips_and_its_cells_use_only_fields_and_kinds_the_game_tables_use() {
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
        let peer: Vec<u8> = fs::read(r.join(respawn::file(DEATH_TABLE_BASE))).unwrap();
        let made: Vec<u8> = procedures(&peer).unwrap();
        let y: Byml = Byml::parse(&made).unwrap();
        assert_eq!(y.write().unwrap(), made);
        let game: Byml = Byml::parse(&peer).unwrap();
        let top = |y: &Byml| -> Vec<String> { if let Node::Map(m) = &y.root { m.iter().map(|(k, _)| k.clone()).collect() } else { panic!() } };
        assert_eq!(top(&y), top(&game));
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
    fn the_field_one_goes_once_the_fire_temple_victory_scene_has_played() {
        let rows: &str = include_str!("../../../../data/flags.json");
        let row: &str = rows.lines().find(|l| l.contains(&format!("\"name\":\"{BEATEN}\""))).unwrap();
        assert!(row.contains("\"type\":\"Bool\"") && row.contains("\"default\":false") && row.contains("\"reset\":8") && row.contains("\"save\":0"));
        assert!(BEATEN.starts_with("HavePlayedEvent.Dm_FireTemple_"));
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let none = |_: &str| -> Option<Vec<u8>> { None };
        let t: Rst = rsizetable::load(&mut rf).unwrap();
        let m: Made = stage(&mut rf, &none, &t, &Shape { arena: FIELD, lift: 0.0, scale: 1.0 }, Mode::Spawn).unwrap();
        assert!(rf.read(&death_table()).is_err());
        let o: &Out = m.outs.iter().find(|o| o.path == death_table()).unwrap();
        assert!(t.get(&o.path).is_none());
        let game: u32 = t.get(&respawn::file(DEATH_TABLE_BASE)).unwrap();
        let game_len: usize = rf.read(&respawn::file(DEATH_TABLE_BASE)).unwrap().len();
        let mine: u32 = m.sizes.iter().find(|(p, _)| p == &o.path).map(|(_, z)| *z).unwrap();
        assert!(o.new >= game_len && mine >= game, "{mine} < {game}");
        assert!(mine as usize >= o.new + 5 * 1024);
        let mut rst: Rst = rsizetable::load(&mut rf).unwrap();
        sized(&mut rf, &mut rst, &m).unwrap();
        assert_eq!(rst.get(&o.path), Some(mine));
        let real: Byml = rf.read_byml("Banc/FireTemple/FireTemple_Static.bcett.byml.zs").unwrap();
        assert!(!format!("{:?}", real.root).contains(CLASS));
        let again: Vec<u8> = respawn::always(&o.z).unwrap();
        assert_eq!(again, o.z);
    }

    #[test]
    fn the_clone_drops_ten_blue_rupees_from_its_own_table_and_nothing_else_of_the_floormaster_is_copied() {
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let dn: Vec<Entry> = donor(&mut rf);
        let es: Vec<Entry> = drop_files(&dn).unwrap();
        let names: Vec<&str> = es.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec![DROP_DEFAULT, DROP_BOARD, DROP_PARAM_CLONE, DROP_TABLE_CLONE, DROP_SUB_CLONE]);
        for e in es.iter().take(2) {
            assert_eq!(Some(&e.data), sarc::get(&dn, &e.name).map(|x| &x.data));
        }
        let sub: Byml = y(&es, DROP_SUB_CLONE);
        let items: &[Node] = sub.root.get("Items").and_then(|i| i.as_arr()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], rupee_item());
        assert_eq!(items[0].get("Type"), Some(&Node::U32(RUPEE_TYPE)));
        assert_eq!(items[0].get("DropProbability"), Some(&Node::I32(100)));
        assert_eq!(items[0].get("DropCreateCount"), Some(&Node::U32(10)));
        assert_eq!(s(&items[0], &["DropActorName"]), "Work/Actor/Obj_Rupee_Blue.engine__actor__ActorParam.gyml");
        let table: Byml = y(&es, DROP_TABLE_CLONE);
        assert_eq!(table.root.get("SubTableList"), Some(&Node::Arr(vec![Node::Str(work(DROP_SUB_CLONE))])));
        assert!(table.root.get("DropTableName").is_none());
        let param: Byml = y(&es, DROP_PARAM_CLONE);
        assert_eq!(param.root.get("DropTableResourceList"), Some(&Node::Arr(vec![Node::Str(work(DROP_TABLE_CLONE))])));
        assert!(dn.iter().any(|e| e.name == DROP_SUB_NONE) && !format!("{:?}", y(&dn, DROP_SUB_NONE).root).contains("Rupee"));
        let pack: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of(RUPEE)).unwrap()).unwrap();
        assert!(pack.iter().any(|e| e.name == actor_file(RUPEE)));
        let kid: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of("Enemy_SkullKid")).unwrap()).unwrap();
        let gold: Byml = y(&kid, "Drop/DropSubTable/Enemy_SkullKid.game__drop__DropSubTable.bgyml");
        let shipped: &Node = &gold.root.get("Items").and_then(|i| i.as_arr()).unwrap()[0];
        assert_eq!(shipped.get("Type"), Some(&Node::U32(RUPEE_TYPE)));
        assert!(s(shipped, &["DropActorName"]).contains("Obj_Rupee_Gold"));
    }

    #[test]
    fn the_grown_die_module_gets_a_bigger_size_entry_in_both_modes() {
        let Some(r) = root() else { return };
        for dm in [Mode::Spawn, Mode::Drop, Mode::Pay] {
            let mut rf: Romfs = Romfs::open(&r).unwrap();
            let none = |_: &str| -> Option<Vec<u8>> { None };
            let t: Rst = rsizetable::load(&mut rf).unwrap();
            let m: Made = stage(&mut rf, &none, &t, &Shape { arena: FIELD, lift: 0.0, scale: 1.0 }, dm).unwrap();
            assert_eq!(m.grown.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), vec![DIE_MODULE]);
            let (_, old, new): &(String, usize, usize) = &m.grown[0];
            assert!(new > old);
            let pk: Vec<Entry> = sarc::parse(&rf.zs.decompress(&m.outs[0].z).unwrap()).unwrap();
            let g: crate::ainb::Ainb = crate::ainb::Ainb::parse(&sarc::get(&pk, DIE_MODULE).unwrap().data).unwrap();
            let plain: Vec<Entry> = sarc::parse(&rf.read_zs(&pack_of(BASE)).unwrap()).unwrap();
            let p: crate::ainb::Ainb = crate::ainb::Ainb::parse(&sarc::get(&plain, DIE_MODULE).unwrap().data).unwrap();
            assert_eq!(g.els.len(), p.els.len() + dm.added());
            let before: u32 = t.get(DIE_MODULE).unwrap();
            let mut rst: Rst = rsizetable::load(&mut rf).unwrap();
            sized(&mut rf, &mut rst, &m).unwrap();
            let after: u32 = rst.get(DIE_MODULE).unwrap();
            assert!(after as u64 >= before as u64 + rsizetable::align((new - old) as u64), "{dm:?} {before} {after}");
            assert!(m.lines[0].contains(&format!("{dm:?}")));
            let io: &Out = m.outs.iter().find(|o| o.path == ACTOR_INFO).unwrap();
            let info: Byml = Byml::parse(&rf.zs.decompress(&io.z).unwrap()).unwrap();
            let plain_info: Byml = rf.read_byml(ACTOR_INFO).unwrap();
            let room: i32 = if dm.holds() { HEAP_ROOM } else { 0 };
            assert!(rows::own_heap(&info, CLASS).unwrap() >= rows::own_heap(&plain_info, BASE).unwrap() + room, "{dm:?}");
            assert_eq!(m.sizes.iter().any(|(p, _)| p == HOLDER_CLONE), dm.holds());
            if let Some((_, z)) = m.sizes.iter().find(|(p, _)| p == HOLDER_CLONE) {
                let len: usize = sarc::get(&pk, HOLDER_CLONE).unwrap().data.len();
                assert!(*z >= t.get(HOLDER_OWN).unwrap() + rsizetable::align(len as u64) as u32, "{z}");
            }
        }
    }

    #[test]
    fn a_drop_less_donor_is_refused_and_the_drop_files_cover_their_parents_size() {
        assert!(drop_files(&[]).is_err());
        assert_eq!(parent_of(DROP_PARAM_CLONE), DROP_PLAIN);
        assert_eq!(parent_of(DROP_TABLE_CLONE), DROP_TABLE_NONE);
        assert_eq!(parent_of(DROP_SUB_CLONE), DROP_SUB_NONE);
        assert_eq!(parent_of(&actor_file(CLASS)), actor_file(BASE));
        let Some(r) = root() else { return };
        let mut rf: Romfs = Romfs::open(&r).unwrap();
        let none = |_: &str| -> Option<Vec<u8>> { None };
        let t: Rst = rsizetable::load(&mut rf).unwrap();
        let m: Made = stage(&mut rf, &none, &t, &Shape { arena: FIELD, lift: 0.0, scale: 1.0 }, Mode::Spawn).unwrap();
        let pk: Vec<Entry> = sarc::parse(&rf.zs.decompress(&m.outs[0].z).unwrap()).unwrap();
        for p in owned(Mode::Spawn) {
            let len: usize = sarc::get(&pk, &p).unwrap().data.len();
            let have: u32 = m.sizes.iter().find(|(n, _)| *n == p).map(|(_, z)| *z).unwrap();
            let parent: u32 = t.get(&parent_of(&p)).unwrap();
            assert!(have as usize >= len && have >= parent, "{p} {have} {len} {parent}");
        }
    }
}
