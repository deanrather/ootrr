"""The actor viewer's kinds: one list of (id, label, regex, shown by default), first match wins, the same list in Python (tests, the
label file) and in the page (actors.js carries it as OOTRR_NAMES.kinds_table; the regexes use only what JavaScript and Python share).

Every kind is a "show" box. "locations" is not a name test: an actor whose placement id is a randomizer location (locations.json or
the tracker) is shown while that box is ticked, whatever its kind; a kind box shows every actor of the kind, location or not.

    python -I actor_kinds.py [actor-index.json]    prints how many placed actors (and how many names) each kind takes
"""
import json
import os
import re
import sys
from collections import Counter

HERE: str = os.path.dirname(os.path.abspath(__file__))

SCENERY: str = (r"^(Land_|Area|Collision|DestinationAnchor|EftLoc|AirWall|FldObj_|DungeonRoom|VolumeFog|StartPos|Anchor|NaviAnchor|"
                r"DropKeepAway|ForbidAuto|Forbid|PushAirWall|SoundLocator|BGM_|ChangeBancBeforehand|AbilityChange|MapIcon|CameraChange|"
                r"CameraPassThrough|NavMesh|SupplyItemReset|LookAttention|InterestSpot|EventRequest|EventProxy|FieldAndDungeonEntrance|"
                r"Sensor|Sns|IndoorArea|Horse(Forbid|Invalidate|Speed)|NoEscape|FallLiftedCuccoArea|FallingRockExclusion|AreaCulling|"
                r"LocationArea|GroundControlActor|GuaySpawnPosAnchor|CmnTex|Obj_Common_|ForbiddenCreateDropArea|NavMeshCut|"
                r"OcarinaPoint|RollingRockSpawn|FishAnchor|BombFlowerChain|HeartPieceAnchor_|CameraEvent)"
                r"|(Area|AreaBox|AreaCylinder|Anchor|Anchor\d|Manager|Dictionary|Dictionary_Undefeated|DropControlActor|Counter)$"
                r"|Collision|AirWall|SwitchCheckActor")

KINDS: list = [
    {"id": "scenery", "label": "Scenery and invisible volumes", "re": SCENERY, "on": False},
    {"id": "gs", "label": "Gold Skulltulas", "re": r"^(Enemy_Skulltula_Gold|Item_Token_Gold)", "on": True},
    {"id": "chests", "label": "Chests", "re": r"TBox(?!Pedestal)", "on": True},
    {"id": "pots", "label": "Pots, crates and hives", "re": r"^(Obj_Pot_|Obj_BrokenBox_|Obj_Kokiri_Beehive|Obj_ShadowTemple_SkullPot|Obj_Gerudo_HorsebackArcheryPot)", "on": True},
    {"id": "grass", "label": "Grass tufts", "re": r"^Obj_Plant_Grass", "on": True},
    {"id": "trees", "label": "Trees", "re": r"^(Tree_|Obj_Tree_|Obj_Gerudo_PalmTree|Obj_WaterTemple_Plant_Tree)", "on": True},
    {"id": "rocks", "label": "Rocks, boulders and bomb flowers", "re": r"^(Obj_LiftRock|Obj_BrokenRock|Obj_Broken\w*Rock|Obj_DodongoCave_MeatRock|Obj_AttentionRock|Obj_BombFlower|Obj_\w*RollingRock|Obj_RollingRock)", "on": True},
    {"id": "rupees", "label": "Rupees", "re": r"^(Obj_Rupee|Obj_Kokiri_RupeeSample|Item_Rupee|TBox_Rupee)", "on": True},
    {"id": "hearts", "label": "Hearts and heart pieces", "re": r"Heart", "on": True},
    {"id": "bugs", "label": "Bugs and butterflies", "re": r"^Animal_(Kokiri_)?(Insect|Butterfly)", "on": True},
    {"id": "animals", "label": "Animals (fish, birds, cuccos, cows...)", "re": r"^(Animal_|EnvironmentalAnimal_)", "on": False},
    {"id": "fairies", "label": "Fairies", "re": r"^(Obj_Fairy|Obj_FairySpirit|FairySpirit)", "on": False},
    {"id": "enemies", "label": "Enemies", "re": r"^Enemy_(?!Scrub)", "on": True},
    {"id": "npcs", "label": "NPCs, scrubs and shops", "re": r"^(Npc_|Shop_|Enemy_Scrub)", "on": False},
    {"id": "signs", "label": "Signs and gossip stones", "re": r"(GossipStone|SignBoard|Signboard|SongOfTimeSign|HangingSign|StoneBirdStatue)", "on": False},
    {"id": "doors", "label": "Doors, switches and puzzle props", "re": r"(Door|Switch|Shutter|HookShot|MovableBox|SliderBox|CandleStick|Lamp|Lift|Target|Bean|Torch|Pedestal|Ladder|Stair|Web|Floor|Elevator|Gate|Cooking)", "on": False},
    {"id": "other", "label": "Other objects", "re": r".*", "on": False},
]
KIND_IDS: list = [k["id"] for k in KINDS]
LOCATIONS: dict = {"id": "locations", "label": "OOTRR locations (ids the randomizer uses)", "on": True}
_COMPILED: list = [(k["id"], re.compile(k["re"])) for k in KINDS]


def kind_of(name: str) -> str:
    n: str = name or ""
    for kid, rx in _COMPILED:
        if rx.search(n):
            return kid
    return "other"


def table() -> dict:
    return {"kinds": [{"id": k["id"], "label": k["label"], "re": k["re"], "on": k["on"]} for k in KINDS], "locations": LOCATIONS}


def count_index(path: str) -> tuple:
    with open(path, encoding="utf-8") as f:
        ids: dict = json.load(f)["ids"]
    by_actor: Counter = Counter(kind_of(v[6]) for v in ids.values())
    names: dict = {}
    for v in ids.values():
        names.setdefault(kind_of(v[6]), Counter())[v[6]] += 1
    return by_actor, names


if __name__ == "__main__":
    p: str = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "actor-index.json")
    a, nm = count_index(p)
    for k in KINDS:
        print("%-9s %6d actors  %4d names" % (k["id"], a[k["id"]], len(nm.get(k["id"], {}))))
    if "-v" in sys.argv:
        for kid, c in nm.items():
            print("\n==", kid)
            for n, m in c.most_common(80):
                print("  ", n, m)
