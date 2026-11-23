import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import _paths  # noqa: E402

HERE = _paths.OOTRR
import puzzle_logic as P
import warp_runner as W
import puzzle_routes as R

KF: str = "KokiriForest_Child"
SWITCH: list[float] = [-81.44, 118.74, 19.26]
GEMS: list[list[float]] = [[-83.72, 118.78, 31.71], [-87.39, 119.37, 16.41], [-85.71, 118.65, 29.51], [-86.27, 118.65, 38.08],
                           [-77.33, 118.65, 30.01], [-88.09, 127.84, 55.2], [-76.07, 118.65, 36.68]]


def kokiri() -> dict:
    s: dict = P.empty()
    P.mark(s, KF, SWITCH, -45.1, "switch")
    for g in GEMS:
        P.mark(s, KF, g, 0.0, "gem")
    P.mark(s, KF, [-80.0, 121.0, 60.0], 0.0, "chest")
    return s


class Logic(unittest.TestCase):
    def test_marks_build_one_puzzle_and_numbers_are_never_reused(self):
        s: dict = kokiri()
        self.assertEqual(len(s["puzzles"]), 1)
        p: dict = s["puzzles"][0]
        self.assertEqual(len(p["gems"]), 7)
        self.assertEqual(s["high"], 9)
        P.remove_mark(s, 9)
        _, _, m = P.mark(s, KF, [-80.0, 121.0, 60.0], 0.0, "chest")
        self.assertEqual(m["n"], 10)

    def test_single_kinds_move(self):
        s: dict = kokiri()
        p, how, _ = P.mark(s, KF, [-81.0, 118.7, 20.0], 10.0, "switch")
        self.assertEqual(how, "moved")
        self.assertEqual(p["switch"]["pos"], [-81.0, 118.7, 20.0])

    def test_far_marks_start_a_new_puzzle_and_new_selects_it(self):
        s: dict = kokiri()
        p, how, _ = P.mark(s, KF, [400.0, 100.0, 0.0], 0.0, "switch")
        self.assertEqual((p["id"], how), (2, "placed"))
        self.assertEqual(P.active(s, KF, [-81.0, 118.0, 20.0])["id"], 1)
        self.assertIsNone(P.active(s, "Other", [-81.0, 118.0, 20.0]))
        q: dict = P.new_puzzle(s, KF)
        self.assertEqual(P.active(s, KF, SWITCH)["id"], q["id"])
        P.mark(s, KF, SWITCH, 0.0, "gem")
        self.assertEqual(len(q["gems"]), 1)

    def test_at_most_ten_rupees(self):
        s: dict = P.empty()
        for i in range(P.MOST_GEMS):
            P.mark(s, "A", [float(i), 0.0, 0.0], 0.0, "gem")
        with self.assertRaises(ValueError):
            P.mark(s, "A", [0.0, 0.0, 1.0], 0.0, "gem")
        with self.assertRaises(ValueError):
            P.mark(s, "A", [0.0, 0.0, 1.0], 0.0, "door")

    def test_nearest_and_remove_drop_empty_puzzles(self):
        s: dict = P.empty()
        P.mark(s, "A", [0.0, 0.0, 0.0], 0.0, "switch")
        P.mark(s, "A", [5.0, 0.0, 0.0], 0.0, "gem")
        p, kind, m = P.nearest_mark(s, "A", [4.0, 0.0, 0.0])
        self.assertEqual((kind, m["n"]), ("gem", 2))
        self.assertIsNone(P.nearest_mark(s, "B", [0.0, 0.0, 0.0]))
        P.remove_mark(s, 2)
        s["current"] = None
        P.remove_mark(s, 1)
        self.assertEqual(s["puzzles"], [])
        self.assertIsNone(P.remove_mark(s, 99))

    def test_problems_mirror_the_rust_check(self):
        s: dict = P.empty()
        P.mark(s, "A", [0.0, 0.0, 0.0], 0.0, "gem")
        p: dict = s["puzzles"][0]
        probs: list[str] = P.problems(p)
        self.assertTrue(any("switch" in x for x in probs))
        self.assertTrue(any("guards nothing" in x for x in probs))
        P.mark(s, "A", [1.0, 0.0, 0.0], 0.0, "wall_a")
        self.assertTrue(any("both ends" in x for x in P.problems(p)))
        P.mark(s, "A", [2.0, 0.0, 0.0], 0.0, "wall_b")
        P.mark(s, "A", [0.0, 0.0, 3.0], 0.0, "switch")
        self.assertEqual(P.problems(p), [])
        self.assertEqual(P.set_seconds(p, 2), P.LEAST)
        self.assertEqual(P.set_seconds(p, 9999), P.MOST)
        P.mark(s, "A", [0.0, 0.0, 59.0], 0.0, "gem")
        P.set_seconds(p, 10)
        self.assertTrue(any("straight run" in x for x in P.problems(p)))

    def test_export_matches_the_shipped_kokiri_room(self):
        out: dict = P.export(kokiri())
        r: dict = out["rooms"][0]
        self.assertEqual(r["switch"], SWITCH)
        self.assertEqual(r["switch_rotate"], [0.0, -0.787, 0.0])
        self.assertEqual(r["gems"][0], [-83.72, 119.28, 31.71])
        self.assertEqual(r["gems"][5], [-88.09, 128.34, 55.2])
        self.assertEqual(r["rupees"]["2"], [-87.39, 119.37, 16.41])
        self.assertEqual((r["seconds"], r["guard"], r["chest"], r["chest_pos"]), (20.0, {"kind": "none"}, None, [-80.0, 121.0, 60.0]))
        self.assertTrue(r["counter_default"])
        self.assertEqual(r["problems"], [])
        self.assertTrue(any("hash" in h for h in r["hints"]))
        rust: str = out["rust"]
        self.assertIn("pub const ROOMS: [Room; 1] = [", rust)
        self.assertIn("switch: [-81.44, 118.74, 19.26],", rust)
        self.assertIn("switch_rotate: [0.0, -0.787, 0.0],", rust)
        self.assertIn("gems: &[[-83.72, 119.28, 31.71], [-87.39, 119.87, 16.41]", rust)
        self.assertIn("seconds: 20.0,", rust)
        self.assertIn("guard: Guard::None,", rust)
        self.assertTrue(rust.rstrip().endswith("];"))

    def test_wall_row(self):
        s: dict = P.empty()
        P.mark(s, "A", [0.0, 0.0, 0.0], 0.0, "wall_a")
        P.mark(s, "A", [4.0, 0.0, 0.0], 0.0, "wall_b")
        self.assertIn("Guard::Wall { a: [0.0, 0.0, 0.0], b: [4.0, 0.0, 0.0] }", P.rust_row(P.room_row(s["puzzles"][0])))

    def test_radar_lines_mark_the_nearest(self):
        s: dict = kokiri()
        lines: list[str] = P.radar_lines(s, KF, 1)
        self.assertEqual(lines[0], "p -81.44 118.74 19.26 >P1 Switch 20s")
        self.assertTrue(all(len(x.split(" ", 4)[4]) <= 15 for x in lines))
        self.assertIn("P1 Rupee 7", lines[-1])
        self.assertEqual(P.radar_lines(s, "Other"), [])

    def test_load_tolerates_junk(self):
        d: str = tempfile.mkdtemp()
        try:
            path: str = os.path.join(d, "x.json")
            self.assertEqual(P.load(path), P.empty())
            with open(path, "w") as f:
                f.write("[1, 2]")
            self.assertEqual(P.load(path), P.empty())
            P.save(path, kokiri())
            self.assertEqual(len(P.load(path)["puzzles"][0]["gems"]), 7)
        finally:
            shutil.rmtree(d, ignore_errors=True)


class FakeGame:
    def __init__(self) -> None:
        self.st: dict = {"map": KF, "pos": list(SWITCH), "facing": -45.1}
        self.captions: list[tuple] = []

    def status(self) -> dict:
        return dict(self.st)

    def caption(self, *a) -> None:
        self.captions.append(a)


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
        self.gems_rs: str = os.path.join(folder, "no-gems.rs")

    def at(self, pos: list[float]) -> None:
        self.game.st["pos"] = list(pos)
        self.status = self.game.status()


class Routes(unittest.TestCase):
    def setUp(self):
        self.tmp: str = tempfile.mkdtemp()
        self.p: FakePlacer = FakePlacer(self.tmp)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def post(self, op: str, body: dict | None = None) -> dict:
        code, out, _ = R.post(self.p, "/api/puzzle/" + op, body or {})
        self.assertEqual(code, 200)
        return json.loads(out)

    def state(self) -> dict:
        return json.loads(R.get(self.p, "/api/puzzle/state")[1])

    def test_flow(self):
        s: dict = self.state()
        self.assertIsNone(s["current"])
        self.assertEqual(s["place"], "Kokiri Forest")
        self.assertFalse(self.post("seconds", {"delta": 5})["ok"])
        self.assertTrue(self.post("switch")["ok"])
        self.assertTrue(self.post("seconds", {"delta": 5})["ok"])
        self.assertFalse(self.post("seconds", {"seconds": "x"})["ok"])
        for g in GEMS:
            self.p.at(g)
            self.assertTrue(self.post("gem")["ok"])
        self.p.at([-80.0, 121.0, 60.0])
        self.assertTrue(self.post("chest")["ok"])
        s = self.state()
        c: dict = s["current"]
        self.assertEqual((c["id"], c["seconds"], c["gems"], c["problems"]), (1, 25.0, 7, []))
        self.assertTrue(c["has"]["chest"])
        self.assertEqual(s["nearest"]["kind"], "chest")
        self.assertEqual([m["near"] for m in c["marks"]].count(True), 1)
        r: dict = self.post("export")
        self.assertTrue(r["ok"])
        self.assertIn("seconds: 25.0,", r["rust"])
        with open(os.path.join(self.tmp, P.EXPORT_NAME), encoding="utf-8") as f:
            saved: dict = json.load(f)
        self.assertEqual(saved["format"], "ootrr-timed-gems")
        self.assertEqual(saved["rooms"][0]["name"], "timed rupees 1 in Kokiri Forest")
        self.assertTrue(self.post("removenearest")["ok"])
        self.assertFalse(self.state()["current"]["has"]["chest"])
        self.assertTrue(self.post("new")["ok"])
        self.assertEqual(self.state()["current"]["id"], 2)
        self.assertTrue(self.post("new")["ok"])
        self.assertEqual([p["id"] for p in R.manager(self.p).state["puzzles"]], [1, 2])
        self.assertTrue(any("Puzzle 1" in c[0] for c in self.p.game.captions))
        self.assertTrue(R.radar_lines(self.p, KF))
        self.assertIsNone(R.post(self.p, "/api/press", {}))
        self.assertIsNone(R.get(self.p, "/api/state"))
        self.assertIn(b"Timed rupee puzzle", R.get(self.p, "/puzzle")[1])
        self.assertEqual(json.loads(R.get(self.p, "/api/puzzle/export")[1])["format"], "ootrr-timed-gems")

    def test_overlay_toggle_hides_the_puzzle_layer(self):
        self.assertTrue(self.post("switch")["ok"])
        self.assertTrue(self.state()["overlay"])
        self.assertEqual(len(R.radar_lines(self.p, KF)), 1)
        r: dict = self.post("overlay")
        self.assertEqual((r["ok"], r["on"]), (True, False))
        self.assertEqual(R.radar_lines(self.p, KF), [])
        self.assertFalse(self.state()["overlay"])
        self.p.overlay_flags = None
        self.assertEqual(R.radar_lines(self.p, KF), [])
        self.assertTrue(self.post("overlay")["on"])
        self.assertEqual(len(R.radar_lines(self.p, KF)), 1)

    def test_not_ready(self):
        self.p.game.st = {}
        self.p.status = {}
        self.assertFalse(self.post("gem")["ok"])
        self.assertFalse(self.post("removenearest")["ok"])
        self.assertFalse(self.state()["connected"])

    def test_cli_export(self):
        notes: str = os.path.join(self.tmp, "notes")
        os.makedirs(notes)
        P.save(os.path.join(notes, P.FILE_NAME), kokiri())
        out: str = os.path.join(self.tmp, "out.json")
        r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "puzzle_logic.py"), "export", self.tmp, out], capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("Guard::None", r.stdout)
        self.assertIn("hash", r.stderr)


ROOMS_RS: str = """pub const KOKIRI_SWORD_CHEST: u64 = 16757992849254467386;
pub const ROOMS: [Room; 1] = [Room {
    id: 1,
    name: "the Kokiri Sword chest at the end of the boulder maze",
    map: "KokiriForest_Child",
    switch: [-81.44, 118.74, 19.26],
    switch_rotate: [0.0, -0.787, 0.0],
    gems: &[[-83.72, 119.28, 31.71], [-87.39, 119.87, 16.41]],
    counter: [-80.45, 121.82, 35.5],
    seconds: 20.0,
    guard: Guard::None,
    chest: Some(KOKIRI_SWORD_CHEST),
    room: None,
}];
"""


class Shipped(unittest.TestCase):
    def setUp(self):
        self.tmp: str = tempfile.mkdtemp()
        self.rs: str = os.path.join(self.tmp, "gems.rs")
        with open(self.rs, "w", encoding="utf-8") as f:
            f.write(ROOMS_RS)
        self.p: FakePlacer = FakePlacer(self.tmp)
        self.p.gems_rs = self.rs

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_the_mods_room_is_read_and_round_trips_to_the_same_rust_row(self):
        import shipped_puzzles as SP
        rooms, where = SP.load(self.rs)
        self.assertEqual((len(rooms), where), (1, self.rs))
        r: dict = rooms[0]
        self.assertEqual((r["shipped"], r["map"], r["chest_hash"], len(r["gems"])), (1, KF, "16757992849254467386", 2))
        self.assertEqual(r["gems"][0]["pos"], [-83.72, 118.78, 31.71])
        self.assertEqual(r["warp"], [-81.44, 119.3, 21.8])
        s: dict = P.empty()
        self.assertEqual(P.merge_shipped(s, rooms), 1)
        self.assertEqual(P.merge_shipped(s, rooms), 0)
        out: dict = P.export(s)
        self.assertIn("chest: Some(16757992849254467386)", out["rust"])
        self.assertIn("gems: &[[-83.72, 119.28, 31.71], [-87.39, 119.87, 16.41]]", out["rust"])
        self.assertIn("switch_rotate: [0.0, -0.787, 0.0]", out["rust"])
        self.assertEqual(out["rooms"][0]["problems"], [])

    def test_the_card_knows_the_mods_room_warps_to_it_and_saves_the_export_itself(self):
        st: dict = json.loads(R.get(self.p, "/api/puzzle/state")[1])
        self.assertEqual(st["on_map"], 1)
        self.assertEqual(st["current"]["shipped"], 1)
        ws: list[dict] = json.loads(R.get(self.p, "/api/puzzle/warps")[1])["puzzles"]
        self.assertEqual([(w["id"], w["how"], w["pos"]) for w in ws], [(1, "measured", [-81.44, 119.3, 21.8])])
        code, out, _ = R.post(self.p, "/api/puzzle/seconds", {"delta": 5})
        self.assertTrue(json.loads(out)["ok"])
        with open(os.path.join(self.tmp, P.EXPORT_NAME), encoding="utf-8") as f:
            self.assertIn("seconds: 25.0", json.load(f)["rust"])
        self.assertFalse(json.loads(R.post(self.p, "/api/puzzle/goto", {"id": 9})[1])["ok"])


if __name__ == "__main__":
    unittest.main()
