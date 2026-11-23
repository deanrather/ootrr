import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import _paths  # noqa: E402

HERE = _paths.OOTRR
import gs_logic as S
import gs_routes as R
import warp_runner as W

KF: str = "KokiriForest_Child"
KF_FILE: str = "Banc/KokiriForest/KokiriForest_Child_Static.bcett.byml.zs"


def rec(name: str, h: str, files: list[tuple[str, str, list[float], bool]], room=None) -> dict:
    fs: list[dict] = [{"file": f, "map": m, "hash": h, "at": at, "rot": [0.0, 0.0, 0.0], "same_as_first": same} for f, m, at, same in files]
    return {"name": name, "hash": h, "map": fs[0]["map"], "file": fs[0]["file"], "at": fs[0]["at"], "age": "child", "night_only": True,
            "appearance": ["appears at night"], "room": room, "rule": "is_child", "rule_plain": "child", "files": fs}


INVENTORY: list[dict] = [
    rec("KF GS Know It All House", "17067594426901953848", [(KF_FILE, KF, [-83.79, 117.854, -17.619], True)]),
    rec("KF GS Bean Patch", "9623998892552467148", [(KF_FILE, KF, [-30.0, 110.0, 10.0], True)]),
    rec("LW GS Bean Patch Near Bridge", "555", [("Banc/LostWood/LostWood_Child_Static.bcett.byml.zs", "LostWood_Child", [1.0, 100.0, 1.0], True),
                                                ("Banc/LostWood/LostWood_Adult_Static.bcett.byml.zs", "LostWood_Adult", [1.0, 100.0, 1.0], True)]),
]
TOUR: list[dict] = [{"id": 1, "hash": "17067594426901953848", "goto": [{"label": "x", "map": KF, "warps": [[-85.55, 118.35, -19.99]]}]}]


class Logic(unittest.TestCase):
    def test_spot_follows_the_tour_rule_on_walls_and_the_feet_on_the_ground(self):
        self.assertEqual(S.spot([0.0, 0.0, 0.0], 90.0, "wall"), [0.8, 1.0, 0.0])
        self.assertEqual(S.spot([0.0, 0.0, 0.0], 0.0, "wall"), [0.0, 1.0, 0.8])
        self.assertEqual(S.spot([1.0, 2.0, 3.0], 45.0, "ground"), [1.0, 2.0, 3.0])

    def test_target_is_the_nearest_original_unless_chosen(self):
        r, chosen = S.target(INVENTORY, KF, [-80.0, 117.0, -17.0], None)
        self.assertEqual((r["hash"], chosen), ("17067594426901953848", False))
        r, chosen = S.target(INVENTORY, KF, [-80.0, 117.0, -17.0], "9623998892552467148")
        self.assertEqual((r["hash"], chosen), ("9623998892552467148", True))
        r, chosen = S.target(INVENTORY, KF, [-80.0, 117.0, -17.0], "555")
        self.assertEqual((r["hash"], chosen), ("17067594426901953848", False))
        self.assertEqual(S.target(INVENTORY, "Market_Child", [0.0, 0.0, 0.0], None), (None, False))

    def test_one_mark_per_skulltula_per_map_and_skip_clears_it(self):
        s: dict = S.empty()
        r: dict = INVENTORY[0]
        m1, how = S.mark(s, r, KF, [-80.0, 117.0, -17.0], 90.0, "wall")
        self.assertEqual((how, m1["n"], m1["to"]), ("placed", 1, [-79.2, 118.0, -17.0]))
        m2, how = S.mark(s, r, KF, [-81.0, 117.0, -17.0], 0.0, "ground")
        self.assertEqual((how, m2["n"], len(s["marks"])), ("moved", 2, 1))
        with self.assertRaises(ValueError):
            S.mark(s, r, "LostWood_Child", [0.0, 0.0, 0.0], 0.0, "wall")
        with self.assertRaises(ValueError):
            S.mark(s, r, KF, [0.0, 0.0, 0.0], 0.0, "ceiling")
        self.assertEqual(S.nearest_mark(s, KF, [0.0, 0.0, 0.0])["n"], 2)
        s2: dict = S.empty()
        S.mark(s2, r, KF, [-80.0, 117.0, -17.0], 0.0, "ground", to=[-70.0, 117.0, -17.0], via="aim")
        self.assertEqual(S.nearest_mark(s2, KF, [-80.0, 117.0, -17.0])["n"], 1)
        self.assertAlmostEqual(S.dist(S.nearest_mark(s2, KF, [-80.0, 117.0, -17.0])["to"], [-80.0, 117.0, -17.0]), 10.0)
        self.assertTrue(S.toggle_skip(s, r["hash"]))
        self.assertEqual((s["marks"], s["skipped"]), ([], [r["hash"]]))
        S.mark(s, r, KF, [-80.0, 117.0, -17.0], 90.0, "wall")
        self.assertEqual(s["skipped"], [])
        self.assertEqual(S.remove_mark(s, 3)["n"], 3)
        self.assertIsNone(S.remove_mark(s, 3))

    def test_export_is_what_gs_move_reads(self):
        s: dict = S.empty()
        S.mark(s, INVENTORY[0], KF, [-80.0, 117.0, -17.0], 90.0, "wall")
        S.mark(s, INVENTORY[1], KF, [-30.0, 110.0, 12.0], 180.0, "ground")
        S.mark(s, INVENTORY[2], "LostWood_Child", [2.0, 100.0, 2.0], 0.0, "wall")
        out, notes = S.export(s, INVENTORY)
        self.assertEqual(out["format"], 1)
        es: list[dict] = out["entries"]
        self.assertEqual(len(es), 4)
        for e in es:
            self.assertTrue(e["hash"].isdigit())
            self.assertTrue(e["file"].startswith("Banc/") and e["file"].endswith(".bcett.byml.zs"))
            self.assertEqual(len(e["to"]), 3)
            self.assertNotEqual("facing" in e, "rotate" in e)
        self.assertEqual((es[0]["name"], es[0]["file"], es[0]["to"], es[0]["facing"]), ("KF GS Know It All House", KF_FILE, [-79.2, 118.0, -17.0], 90.0))
        self.assertEqual(es[1]["rotate"], [0.0, round(math.pi, 4), 0.0])
        self.assertEqual([e["file"].split("/")[-1] for e in es[2:]], ["LostWood_Child_Static.bcett.byml.zs", "LostWood_Adult_Static.bcett.byml.zs"])
        self.assertTrue(any("LostWood_Adult" in n for n in notes))
        S.mark(s, INVENTORY[2], "LostWood_Adult", [5.0, 100.0, 5.0], 0.0, "wall")
        out, _ = S.export(s, INVENTORY)
        self.assertEqual(len(out["entries"]), 4)
        S.toggle_skip(s, INVENTORY[0]["hash"])
        out, _ = S.export(s, INVENTORY)
        self.assertEqual(out["skipped"], [{"name": "KF GS Know It All House", "hash": "17067594426901953848"}])

    def test_export_parses_like_the_tour_converter_output(self):
        s: dict = S.empty()
        S.mark(s, INVENTORY[0], KF, [-78.79, 115.854, -17.619], 90.0, "wall")
        e: dict = S.export(s, INVENTORY)[0]["entries"][0]
        self.assertEqual(set(e) - {"surface", "from"}, {"hash", "name", "file", "to", "facing"})

    def test_radar_lines(self):
        s: dict = S.empty()
        m, _ = S.mark(s, INVENTORY[0], KF, [-80.0, 117.0, -17.0], 90.0, "wall")
        lines: list[str] = S.radar_lines(s, INVENTORY, KF, m["n"])
        self.assertEqual(lines[0], "p -79.20 118.00 -17.00 G>Know It All H")
        self.assertEqual(lines[1], "v -83.79 117.85 -17.62 old Know It All")
        self.assertEqual(len(lines), 3)
        self.assertTrue(all(len(x.split(" ", 4)[4]) <= 15 for x in lines))
        self.assertEqual(S.radar_lines(s, INVENTORY, "Market_Child"), [])

    def test_warp_list_has_marks_first_then_tour_originals(self):
        s: dict = S.empty()
        S.mark(s, INVENTORY[0], KF, [-80.0, 117.0, -17.0], 90.0, "wall")
        w: list[dict] = S.warp_list(s, INVENTORY, S.tour_warps(TOUR))
        self.assertEqual([x["kind"] for x in w], ["mark", "original"])
        self.assertEqual((w[0]["pos"], w[0]["map"]), ([-80.0, 117.0, -17.0], KF))
        self.assertEqual(w[1]["pos"], [-85.55, 118.35, -19.99])

    def test_load_tolerates_junk(self):
        d: str = tempfile.mkdtemp()
        try:
            p: str = os.path.join(d, "x.json")
            self.assertEqual(S.load(p), S.empty())
            with open(p, "w") as f:
                f.write("[1]")
            self.assertEqual(S.load(p), S.empty())
            self.assertEqual(S.load_list(os.path.join(d, "none.json")), ([], None))
        finally:
            shutil.rmtree(d, ignore_errors=True)

    def test_real_inventory(self):
        gs, where = S.load_list()
        if where is None:
            self.skipTest("no cache/gs-list.json")
        self.assertEqual(len(gs), 100)
        self.assertTrue(all(r["files"] and str(r["hash"]).isdigit() for r in gs))


class FakeGame:
    def __init__(self) -> None:
        self.st: dict = {"map": KF, "pos": [-80.0, 117.0, -17.0], "facing": 90.0}
        self.captions: list[tuple] = []
        self.sent: list[tuple] = []

    def status(self) -> dict:
        return dict(self.st)

    def caption(self, *a) -> None:
        self.captions.append(a)

    def ctl(self, *args: str, timeout: int = 30, retries: int = 6) -> str:
        self.sent.append(args)
        if args[0] == "teleport":
            self.st = {"map": args[1], "pos": [0.0, 0.0, 0.0], "facing": 0.0}
        if args[0] == "warp":
            self.st["pos"] = [float(v) for v in args[1:4]]
        return "ok"

    def sleep(self, s: float) -> None:
        pass

    hit: dict = {"hit": True, "pos": [-82.0, 118.0, -17.5], "normal": [1.0, 0.0, 0.0], "surface": "wall", "yaw_out": math.pi / 2, "dist": 3.0}
    appeared: bool = True

    def aim_cast(self) -> dict:
        return dict(self.hit)

    def spawn(self, name: str, pos: list, yaw: float, quick: bool = False) -> tuple:
        self.spawned: tuple = (name, list(pos), yaw)
        return self.appeared, "" if self.appeared else "refused"


class FakePlacer:
    def __init__(self, folder: str) -> None:
        self.overlay_flags: dict = {k: k not in ("models", "gate_models") for k in W.LAYERS}
        self.aim_on: bool = False
        self.aim_for: str = "enemy"
        self.game: FakeGame = FakeGame()
        self.points_path: str = os.path.join(folder, "spawnpoints.json")
        self.points: list[dict] = []
        self.places: dict = {"KokiriForest": "Kokiri Forest"}
        self.status: dict = self.game.status()
        self.gs_list: str = os.path.join(folder, "gs-list.json")
        self.gs_tour: str = os.path.join(folder, "steps.json")
        S.save(self.gs_list, {"format": 1, "gs": INVENTORY})
        S.save(self.gs_tour, TOUR)

    def at(self, m: str, pos: list[float]) -> None:
        self.game.st = {"map": m, "pos": list(pos), "facing": 90.0}
        self.status = self.game.status()


class Routes(unittest.TestCase):
    def setUp(self):
        self.tmp: str = tempfile.mkdtemp()
        self.p: FakePlacer = FakePlacer(self.tmp)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def post(self, op: str, body: dict | None = None) -> dict:
        code, out, _ = R.post(self.p, "/api/gs/" + op, body or {})
        self.assertEqual(code, 200)
        return json.loads(out)

    def state(self) -> dict:
        return json.loads(R.get(self.p, "/api/gs/state")[1])

    def test_flow(self):
        s: dict = self.state()
        self.assertEqual((s["place"], s["inventory"], s["target"]["short"], s["target"]["chosen"]), ("Kokiri Forest", 3, "Know It All House", False))
        self.assertEqual([r["name"] for r in s["here"]], ["Know It All House", "Bean Patch"])
        self.assertIsNone(s["nearest"])
        self.assertFalse(self.post("removenearest")["ok"])
        self.assertTrue(self.post("wall")["ok"])
        s = self.state()
        self.assertTrue(s["target"]["marked"])
        self.assertEqual((s["nearest"]["name"], s["marks"][0]["surface"]), ("Know It All House", "wall"))
        self.assertTrue(self.post("target", {"hash": "9623998892552467148"})["ok"])
        self.assertEqual(self.state()["target"]["short"], "Bean Patch")
        self.assertTrue(self.post("ground")["ok"])
        self.assertEqual(len(self.state()["marks"]), 2)
        self.assertTrue(self.post("target", {"hash": None})["ok"])
        self.assertFalse(self.state()["target"]["chosen"])
        r: dict = self.post("export")
        self.assertTrue(r["ok"])
        with open(os.path.join(self.tmp, S.EXPORT_NAME), encoding="utf-8") as f:
            saved: dict = json.load(f)
        self.assertEqual((saved["format"], len(saved["entries"])), (1, 2))
        self.assertEqual(json.loads(r["json"]), saved)
        with open(os.path.join(self.tmp, S.FILE_NAME), encoding="utf-8") as f:
            self.assertEqual(len(json.load(f)["marks"]), 2)
        self.assertTrue(self.post("removenearest")["ok"])
        self.assertEqual(len(self.state()["marks"]), 1)
        self.assertTrue(self.post("skip")["ok"])
        self.assertTrue(self.state()["target"]["skip"])
        self.assertTrue(any("Gold Skulltula" in c[0] for c in self.p.game.captions))
        self.assertIsNone(R.post(self.p, "/api/press", {}))
        self.assertIsNone(R.get(self.p, "/api/state"))
        self.assertIn(b"Skulltula Manager", R.get(self.p, "/gs")[1])
        self.assertEqual(json.loads(R.get(self.p, "/api/gs/export")[1])["format"], 1)

    def test_every_change_is_saved_at_once_with_no_save_button(self):
        self.assertEqual(self.state()["saved"], "")
        self.assertTrue(self.post("wall")["ok"])
        with open(os.path.join(self.tmp, S.EXPORT_NAME), encoding="utf-8") as f:
            self.assertEqual(len(json.load(f)["entries"]), 1)
        self.assertIn("saved 1 moved", self.state()["saved"])
        self.assertNotIn(b'id="export" class="btn go', open(os.path.join(R.HERE, "gs.html"), "rb").read())

    def test_place_at_aim_wall_uses_the_crosshair_spot_and_faces_into_the_wall(self):
        r = self.post("aimplace")
        self.assertTrue(r["ok"], r)
        m = self.state()["marks"][0]
        self.assertEqual(m["surface"], "wall")
        mk = json.load(open(os.path.join(self.tmp, S.FILE_NAME), encoding="utf-8"))["marks"][0]
        self.assertEqual(mk["to"], [-82.0, 118.0, -17.5])
        self.assertEqual((mk["via"], mk["facing"]), ("aim", -90.0))
        self.assertEqual(len(json.load(open(os.path.join(self.tmp, S.EXPORT_NAME), encoding="utf-8"))["entries"]), 1)

    def test_place_at_aim_on_the_floor_looks_at_link(self):
        self.p.game.hit = {"hit": True, "pos": [-80.0, 117.0, -10.0], "normal": [0.0, 1.0, 0.0], "surface": "floor", "yaw_out": None, "dist": 3.0}
        self.assertTrue(self.post("aimplace")["ok"])
        mk = json.load(open(os.path.join(self.tmp, S.FILE_NAME), encoding="utf-8"))["marks"][0]
        self.assertEqual((mk["surface"], mk["to"], mk["facing"]), ("ground", [-80.0, 117.0, -10.0], 180.0))

    def test_remove_aimed_at_takes_the_mark_near_the_crosshair_only(self):
        self.post("aimplace")
        self.p.game.hit = dict(self.p.game.hit, pos=[-70.0, 118.0, -17.5])
        self.assertFalse(self.post("aimremove")["ok"])
        self.assertEqual(len(self.state()["marks"]), 1)
        self.p.game.hit = dict(self.p.game.hit, pos=[-82.2, 118.0, -17.5])
        self.assertTrue(self.post("aimremove")["ok"])
        self.assertEqual(self.state()["marks"], [])

    def test_aim_events_from_the_controller_reach_the_gs_card(self):
        self.p.aim_on = True
        self.p.aim_for = "gs"
        R.aim_apply(self.p, dict(self.p.game.hit, kind="place", map=KF, n=1))
        self.assertEqual(len(self.state()["marks"]), 1)
        R.aim_apply(self.p, {"kind": "undo"})
        self.assertEqual(self.state()["marks"], [])

    def test_spawn_skulltula_goes_to_the_mark_else_in_front_of_link(self):
        self.assertTrue(self.post("spawn")["ok"])
        name, pos, yaw = self.p.game.spawned
        self.assertEqual(name, "Enemy_Skulltula_Gold")
        self.assertAlmostEqual(pos[0], -80.0 + 0.8, places=2)
        self.assertEqual(pos[1], 118.0)
        self.post("aimplace")
        self.assertTrue(self.post("spawn")["ok"])
        self.assertEqual(self.p.game.spawned[1], [-82.0, 118.0, -17.5])
        self.p.game.appeared = False
        r = self.post("spawn")
        self.assertFalse(r["ok"])
        self.assertIn("did not appear", r["message"])

    def test_no_skulltula_in_this_map(self):
        self.p.at("Market_Child", [0.0, 0.0, 0.0])
        self.assertIsNone(self.state()["target"])
        self.assertFalse(self.post("wall")["ok"])
        self.assertFalse(self.post("skip")["ok"])

    def test_overlay_and_warp(self):
        self.assertTrue(self.post("wall")["ok"])
        self.assertEqual(len(R.radar_lines(self.p, KF)), 3)
        self.assertFalse(self.post("overlay")["on"])
        self.assertEqual(R.radar_lines(self.p, KF), [])
        self.assertTrue(self.post("overlay")["on"])
        w: list[dict] = json.loads(R.get(self.p, "/api/gs/warps")[1])["spots"]
        self.assertEqual([x["kind"] for x in w], ["mark", "original"])
        self.p.at("LostWood_Child", [0.0, 0.0, 0.0])
        self.assertFalse(self.post("goto", {"key": "nope"})["ok"])
        self.assertTrue(self.post("goto", {"key": w[0]["key"]})["ok"])
        run = W.runner(self.p)
        for _ in range(100):
            if not run.busy:
                break
            time.sleep(0.02)
        self.assertTrue(run.ok, run.message)
        self.assertEqual([c[0] for c in self.p.game.sent], ["teleport", "warp"])
        self.assertEqual((self.p.game.st["map"], self.p.game.st["pos"]), (KF, [-80.0, 117.0, -17.0]))

    def test_not_ready(self):
        self.p.game.st = {}
        self.p.status = {}
        self.assertFalse(self.post("wall")["ok"])
        self.assertFalse(self.post("removenearest")["ok"])
        self.assertFalse(self.state()["connected"])

    def test_cli_export(self):
        notes: str = os.path.join(self.tmp, "notes")
        os.makedirs(notes)
        st: dict = S.empty()
        S.mark(st, INVENTORY[0], KF, [-80.0, 117.0, -17.0], 90.0, "wall")
        S.save(os.path.join(notes, S.FILE_NAME), st)
        out: str = os.path.join(self.tmp, "out.json")
        env: dict = dict(os.environ, OOTRR_GS_LIST=self.p.gs_list)
        r = subprocess.run([sys.executable, os.path.join(HERE, "gs_logic.py"), "export", self.tmp, out], capture_output=True, text=True, env=env)
        self.assertEqual(r.returncode, 0, r.stderr)
        with open(out, encoding="utf-8") as f:
            self.assertEqual(len(json.load(f)["entries"]), 1)


if __name__ == "__main__":
    unittest.main()
