import json
import os
import shutil
import subprocess
import sys
import unittest

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import _paths  # noqa: E402

HERE = _paths.OOTRR
import actor_kinds as K

INDEX: str = os.path.join(_paths.ROOT, "cache", "actor-index.json")
SAMPLES: dict = {
    "Obj_TBox_A_01": "chests", "Obj_TBox_A_01_NoBake": "chests", "Obj_HrlCity_Shop_TBox_A_01": "chests", "Obj_TBoxPedestal_A_01": "doors",
    "Obj_Pot_A_01": "pots", "Obj_Pot_A_06_Dream": "pots", "Obj_BrokenBox_A_01": "pots", "Obj_Kokiri_Beehive_A_01": "pots",
    "Obj_Plant_Grass_A_01": "grass", "Obj_Plant_Grass_A_04": "grass", "Obj_Plant_MagicBeanLeaf_A_02": "doors",
    "Tree_Kokiri_BillboardTree_A_01": "trees", "Tree_Kokiri_BillboardTree_A_02": "trees", "Obj_Tree_A_01": "trees", "Obj_Gerudo_PalmTree_A_01": "trees",
    "Animal_Kokiri_Insect_A": "bugs", "Animal_Kokiri_Insect_D": "bugs", "Animal_Butterfly": "bugs", "Animal_ButterflyToFairy": "bugs", "Animal_Insect": "bugs",
    "Animal_Cucco_CatchCucco": "animals", "Animal_Fish_Bass": "animals", "EnvironmentalAnimal_Bird_Single": "animals", "Animal_Cow": "animals",
    "Obj_Rupee_Green": "rupees", "Obj_Rupee_Blue": "rupees", "Obj_Rupee_Red": "rupees", "Obj_Rupee_Purple": "rupees", "Obj_RupeeToken_Silver": "rupees",
    "Obj_silverRupeeCounter": "scenery", "Obj_Heart": "hearts", "Obj_Heart_Piece": "hearts", "Obj_Heart_Container": "hearts",
    "Obj_LiftRock_A_01": "rocks", "Obj_BrokenRock_A_01": "rocks", "Obj_BombFlower_A_03": "rocks",
    "Enemy_Skulltula_Gold": "gs", "Enemy_Skulltula": "enemies", "Enemy_Tektite_Blue": "enemies", "Enemy_FlyingPot_SpiritTemple": "enemies",
    "Enemy_Scrub_Mad": "npcs", "Npc_Navi": "npcs", "Shop_Bottle_Bug": "npcs",
    "Obj_Fairy": "fairies", "Obj_FairySpirit": "fairies", "Obj_GossipStone_A_01": "signs", "Obj_SignBoard_A_01": "signs",
    "Obj_SwitchShutter_A_02": "doors", "Obj_HookShotTarget_A_01": "doors", "Obj_MovableBox_A_01": "doors",
    "Land_Kokiri_Ground_A_01": "scenery", "AreaBox_Water": "scenery", "DestinationAnchor_A": "scenery", "FldObj_AirWall_Animal": "scenery",
    "EftLoc_Insects": "scenery", "DropKeepAwayArea_TBox_A_01": "scenery", "Obj_Common_Fragment_BrokenRock_A_00": "scenery",
    "Obj_Tombstone_A_01": "other", "Obj_Flag_A_01": "other",
}


class Kinds(unittest.TestCase):
    def test_samples(self):
        for name, want in SAMPLES.items():
            self.assertEqual(K.kind_of(name), want, name)

    def test_ids_are_unique_and_every_kind_has_a_label_and_a_regex(self):
        ids: list = [k["id"] for k in K.KINDS]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(ids[-1], "other")
        self.assertNotIn("locations", ids)
        for k in K.KINDS:
            self.assertTrue(k["label"] and k["re"])
        self.assertEqual(K.table()["locations"]["id"], "locations")

    def test_defaults_show_location_objects_and_hide_scenery(self):
        on: dict = {k["id"]: k["on"] for k in K.KINDS}
        self.assertFalse(on["scenery"])
        for k in ("gs", "chests", "pots", "grass", "trees", "rocks", "rupees", "hearts", "bugs", "enemies"):
            self.assertTrue(on[k], k)
        self.assertTrue(K.LOCATIONS["on"])

    @unittest.skipUnless(os.path.isfile(INDEX), "actor-index.json is made by build_cache.py from the game")
    def test_placed_game_actors_land_in_their_kinds(self):
        by_actor, names = K.count_index(INDEX)
        for kid in K.KIND_IDS:
            self.assertGreater(sum(names.get(kid, {}).values()), 0, kid)
        want: dict = {"Obj_Pot_": "pots", "Obj_Rupee_": "rupees", "Tree_Kokiri_": "trees", "Obj_Plant_Grass_": "grass", "Animal_Kokiri_Insect_": "bugs",
                      "Obj_TBox_A": "chests", "Obj_LiftRock_": "rocks", "Obj_Heart": "hearts", "Enemy_Skulltula_Gold": "gs"}
        for prefix, kid in want.items():
            for n in set(x for c in names.values() for x in c if x.startswith(prefix) and "Dictionary" not in x):
                self.assertEqual(K.kind_of(n), kid, n)
        self.assertGreater(sum(names["trees"].values()), 400)
        self.assertGreater(sum(names["bugs"].values()), 100)

    @unittest.skipUnless(shutil.which("node") and os.path.isfile(INDEX), "needs node and actor-index.json")
    def test_javascript_regexes_agree_with_python_on_every_name(self):
        with open(INDEX, encoding="utf-8") as f:
            ids: dict = json.load(f)["ids"]
        every: list = sorted({v[6] for v in ids.values()})
        script: str = ("const t=JSON.parse(require('fs').readFileSync(0,'utf8'));const rx=t.kinds.map(k=>[k.id,new RegExp(k.re)]);"
                       "const out=t.names.map(n=>{for(const [i,r] of rx) if(r.test(n)) return i; return 'other';});console.log(JSON.stringify(out));")
        r = subprocess.run(["node", "-e", script], input=json.dumps({"kinds": K.table()["kinds"], "names": every}), capture_output=True, text=True, timeout=60)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(r.stdout), [K.kind_of(n) for n in every])


if __name__ == "__main__":
    unittest.main()
