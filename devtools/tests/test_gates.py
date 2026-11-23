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
import gate_logic as G
import gate_routes as R
import goto
import placer_logic as L
import shipped_gates as SG
import warp_runner as W


def pt(n, name, x, z, m="A", y=0.0):
    return {"n": n, "map": m, "name": name, "pos": [x, y, z], "facing": 0, "time": "t"}


class Logic(unittest.TestCase):
    def test_place_creates_then_completes(self):
        gates: list[dict] = []
        g, how = G.place_post(gates, "A", [0, 0, 0], "left")
        self.assertEqual((how, g["id"], g["right"]), ("new", 1, None))
        g2, how2 = G.place_post(gates, "A", [8, 0, 0], "right")
        self.assertIs(g2, g)
        self.assertEqual(how2, "placed")
        self.assertTrue(G.complete(g))
        g3, how3 = G.place_post(gates, "A", [1, 0, 1], "left")
        self.assertEqual((g3["id"], how3, g3["left"]), (1, "moved", [1.0, 0.0, 1.0]))
        self.assertEqual(len(gates), 1)

    def test_preview_says_what_a_post_press_will_do(self):
        gates: list[dict] = []
        self.assertEqual(G.place_preview(gates, "A", [0, 0, 0], "left"), {"how": "new", "gate": 1, "dist": None})
        G.place_post(gates, "A", [0, 0, 0], "left")
        self.assertEqual(G.place_preview(gates, "A", [8, 0, 0], "right"), {"how": "placed", "gate": 1, "dist": 8.0})
        self.assertEqual(G.place_preview(gates, "A", [1, 0, 0], "left"), {"how": "moved", "gate": 1, "dist": 1.0})
        for side in G.SIDES:
            want: dict = G.place_preview(gates, "A", [3, 0, 0], side)
            g, how = G.place_post([dict(x) for x in gates], "A", [3, 0, 0], side)
            self.assertEqual((want["how"], want["gate"]), (how, g["id"]))
        self.assertEqual(len(gates), 1)

    def test_wide_gate_pairs_its_far_post(self):
        gates: list[dict] = []
        G.place_post(gates, "A", [0, 0, 0], "left")
        g, how = G.place_post(gates, "A", [35, 0, 0], "right")
        self.assertEqual((len(gates), how), (1, "placed"))
        _, how = G.place_post(gates, "A", [200, 0, 0], "left")
        self.assertEqual((len(gates), how), (2, "new"))
        _, how = G.place_post(gates, "B", [0, 0, 0], "right")
        self.assertEqual((len(gates), how), (3, "new"))

    def test_near_uses_the_wall_segment(self):
        gates: list[dict] = [{"id": 1, "map": "A", "left": [0, 0, 0], "right": [30, 0, 0], "guards": []}]
        self.assertIsNotNone(G.nearest_gate(gates, "A", [15, 0, 9]))
        self.assertIsNotNone(G.nearest_gate(gates, "A", [0, 0, 9]))
        self.assertIsNone(G.nearest_gate(gates, "A", [15, 0, 11]))
        self.assertIsNone(G.nearest_gate(gates, "B", [15, 0, 0]))
        self.assertEqual(G.nearest_gate(gates, "A", [15, 0, 50], None)["id"], 1)
        one: list[dict] = [{"id": 2, "map": "A", "left": None, "right": [0, 0, 0], "guards": []}]
        self.assertIsNotNone(G.nearest_gate(one, "A", [9, 0, 0]))
        self.assertIsNone(G.nearest_gate(one, "A", [11, 0, 0]))
        self.assertIsNone(G.nearest_gate([{"id": 3, "map": "A", "left": None, "right": None, "guards": []}], "A", [0, 0, 0]))

    def test_nearest_of_two(self):
        gates: list[dict] = [{"id": 1, "map": "A", "left": [0, 0, 0], "right": [4, 0, 0], "guards": []},
                             {"id": 2, "map": "A", "left": [0, 0, 12], "right": [4, 0, 12], "guards": []}]
        self.assertEqual(G.nearest_gate(gates, "A", [2, 0, 7])["id"], 2)
        self.assertEqual(G.nearest_gate(gates, "A", [2, 0, 5])["id"], 1)

    def test_mark_and_unmark(self):
        gates: list[dict] = [{"id": 1, "map": "A", "left": [0, 0, 0], "right": [4, 0, 0], "guards": []},
                             {"id": 2, "map": "A", "left": [50, 0, 0], "right": [54, 0, 0], "guards": []}]
        pts: list[dict] = [pt(1, "Enemy_Lizalfos", 2, 5), pt(2, "Enemy_Keese", 2, 9), pt(3, "Enemy_Keese", 2, 5, "B"), pt(4, "Enemy_Wolfos", 2, 60)]
        g: dict = gates[0]
        here: list[float] = [2, 0, 4]
        self.assertEqual(G.mark_target(g, pts, "A", here)["n"], 1)
        self.assertIsNone(G.mark_guard(gates, 1, 1))
        self.assertEqual(g["guards"], [1])
        self.assertEqual(G.mark_target(g, pts, "A", here)["n"], 2)
        G.mark_guard(gates, 1, 2)
        self.assertIsNone(G.mark_target(g, pts, "A", here))
        G.mark_guard(gates, 1, 2)
        self.assertEqual(g["guards"], [1, 2])
        self.assertEqual(G.unmark_target(g, pts, [2, 0, 10])["n"], 2)
        self.assertTrue(G.unmark_guard(gates, 1, 2))
        self.assertFalse(G.unmark_guard(gates, 1, 2))
        self.assertEqual(g["guards"], [1])
        moved: dict | None = G.mark_guard(gates, 2, 1)
        self.assertEqual(moved["id"], 1)
        self.assertEqual((gates[0]["guards"], gates[1]["guards"]), ([], [1]))
        self.assertEqual(G.guard_of(gates, 1)["id"], 2)
        self.assertIsNone(G.mark_target(None, pts, "A", here))
        self.assertIsNone(G.unmark_target(gates[0], pts, here))

    def test_remove_and_prune(self):
        gates: list[dict] = [{"id": 1, "map": "A", "left": [0, 0, 0], "right": [4, 0, 0], "guards": [1, 2, 9]},
                             {"id": 2, "map": "A", "left": [50, 0, 0], "right": [54, 0, 0], "guards": [3]}]
        pts: list[dict] = [pt(1, "Enemy_Lizalfos", 2, 5), pt(2, "Enemy_Keese", 2, 9, "B"), pt(3, "Enemy_Keese", 52, 5)]
        self.assertTrue(G.prune(gates, pts))
        self.assertEqual(gates[0]["guards"], [1])
        self.assertFalse(G.prune(gates, pts))
        left: list[dict] = G.remove_gate(gates, 1)
        self.assertEqual([g["id"] for g in left], [2])
        self.assertEqual(G.next_id(left), 3)
        self.assertEqual(len(pts), 3)

    def test_radar(self):
        gates: list[dict] = [{"id": 3, "map": "A", "left": [0, 1, 0], "right": None, "guards": [7]},
                             {"id": 4, "map": "B", "left": [0, 1, 0], "right": [1, 1, 0], "guards": [8]}]
        self.assertEqual(G.radar_lines(gates, "A"), ["p 0.00 1.00 0.00 Gate 3 L"])
        self.assertEqual(len(G.radar_lines(gates, "B")), 2)
        tags: dict[int, str] = G.guard_tags(gates, "A")
        self.assertEqual(tags, {7: " G3"})
        pts: list[dict] = [pt(7, "Enemy_Lizalfos", 2, 5), pt(6, "Enemy_Keese", 3, 5)]
        marks: str = L.radar_marks(None, pts, "A", False, extra=G.radar_lines(gates, "A"), tags=tags)
        lines: list[str] = marks.split("\n")
        self.assertEqual(lines[0], "p 0.00 1.00 0.00 Gate 3 L")
        self.assertEqual(lines[1], "p 2.00 0.00 5.00 Lizalf G3")
        self.assertEqual(lines[2], "p 3.00 0.00 5.00 Keese")
        self.assertEqual(L.radar_marks(None, pts, "A"), "p 2.00 0.00 5.00 Lizalf\np 3.00 0.00 5.00 Keese")

    def test_barrier_faces_guards(self):
        guard: list[float] = [0.0, 0.0, 5.0]
        for a, b in (([-4, 0, 0], [4, 0, 0]), ([4, 0, 0], [-4, 0, 0])):
            bar: dict = G.barrier(a, b, guard)
            self.assertAlmostEqual(bar["width"], 8.0)
            fwd: tuple[float, float] = (math.sin(bar["yaw"]), math.cos(bar["yaw"]))
            self.assertGreater(fwd[0] * guard[0] + fwd[1] * guard[2], 0.0)
            d: tuple[float, float] = (math.cos(bar["yaw"]), -math.sin(bar["yaw"]))
            self.assertAlmostEqual(bar["a"][0], -d[0] * 4.0, places=2)
            self.assertAlmostEqual(bar["b"][2], d[1] * 4.0, places=2)

    def test_export(self):
        gates: list[dict] = [{"id": 2, "map": "ZoraRiver_Child", "left": [-4, 100, 0], "right": [4, 100, 0], "guards": [1, 2]},
                             {"id": 1, "map": "ZoraRiver_Child", "left": [0, 0, 0], "right": None, "guards": [1]},
                             {"id": 3, "map": "X", "left": [0, 0, 0], "right": [3, 0, 0], "guards": []}]
        pts: list[dict] = [pt(1, "Enemy_Lizalfos", 0, -6, "ZoraRiver_Child", 100.0), pt(2, "Enemy_Lizalfos", 2, -7, "ZoraRiver_Child", 100.0)]
        session, notes = G.export_session(gates, pts)
        self.assertEqual(session["format"], "ootrr-gate-session")
        self.assertEqual([g["id"] for g in session["gates"]], [2])
        self.assertEqual(len(notes), 2)
        g: dict = session["gates"][0]
        self.assertEqual((g["age"], g["map"], g["label"]), ("child", "ZoraRiver_Child", "Gate 2"))
        self.assertEqual([k["kind"] for k in g["guards"]], ["Enemy_Lizalfos", "Enemy_Lizalfos"])
        self.assertAlmostEqual(g["guards"][0]["yaw"], math.atan2(0.0, 6.0), places=3)
        bar: dict = g["barriers"][0]
        self.assertEqual(bar["centre"], [0.0, 100.0, 0.0])
        self.assertGreater(math.sin(bar["yaw"]) * 1.0 + math.cos(bar["yaw"]) * -6.5, 0.0)
        text: str = G.dump_session(session)
        self.assertEqual(json.loads(text), session)

    def test_export_feeds_json_to_gates(self):
        conv: str = os.path.join(HERE, "..", "gate-editor", "json_to_gates.py")
        if not os.path.isfile(conv):
            self.skipTest("json_to_gates.py not found")
        tmp: str = tempfile.mkdtemp()
        try:
            notes: str = os.path.join(tmp, "notes")
            os.makedirs(notes)
            G.save(os.path.join(notes, G.FILE_NAME), [{"id": 1, "map": "ZoraRiver_Child", "left": [-4, 100, 0], "right": [4, 100, 0], "guards": [5]}])
            G.save(os.path.join(notes, "spawnpoints.json"), [pt(5, "Enemy_Lizalfos", 0, -6, "ZoraRiver_Child", 100.0)])
            out: str = os.path.join(tmp, "gates.json")
            r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "gate_logic.py"), "export", tmp, out], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            r2 = subprocess.run([sys.executable, "-I", conv, out, "--first-id", "40"], capture_output=True, text=True)
            self.assertEqual(r2.returncode, 0, r2.stderr)
            self.assertIn("id: 40", r2.stdout)
            self.assertIn("ZoraRiver_Child", r2.stdout)
            self.assertIn("ZORA_GUARD", r2.stdout)
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


RS: str = """
pub const POE_GUARD: &str = "Enemy_Poe";
pub const MARKET_GUARD: &str = HAND;
pub struct Gate {
    pub id: u8,
}
pub const GATES: [Gate; 3] = [
    Gate { id: 6, name: "the path, with a comma", map: "KokiriForest_Child", a: [45.09, 113.77, -26.27], b: [45.43, 112.83, -19.5], spot: [38.26, 113.25, -17.34], more: &[], also: &[], sink: 0.0, radius: RADIUS, new_kind: "", more_kind: "", loose: &[], scale: 1.0, closed_forest_only: true },
    Gate { id: 11, name: "the ruined Market", map: "Market_Adult", a: [0.65, 102.04, 52.21], b: [-4.51, 102.07, 52.27], spot: [-1.67, 100.07, 32.58], more: &[], also: &[([4.48, 100.26, -1.96], [4.52, 100.26, 2.08])], sink: 0.0, radius: RADIUS, new_kind: MARKET_GUARD, more_kind: "", loose: &[], scale: 1.0, closed_forest_only: false },
    Gate { id: 17, name: "the Graveyard, the way to the Shadow Temple", map: "Graveyard_Child", a: [72.35, 108.56, 0.62], b: [72.35, 108.46, 4.34], spot: [67.6, 108.64, 2.5], more: &[[69.0, 108.62, 0.9], [69.0, 108.54, 4.3]], also: &[], sink: 0.0, radius: RADIUS, new_kind: POE_GUARD, more_kind: "", loose: &[], scale: 1.0, closed_forest_only: false },
];
"""


class Shipped(unittest.TestCase):
    def test_parse_fixture(self):
        gs: list[dict] = SG.parse(RS)
        self.assertEqual([g["id"] for g in gs], [6, 11, 17])
        g17: dict = gs[2]
        self.assertEqual((g17["map"], g17["kind"], g17["left"], g17["right"]), ("Graveyard_Child", "Enemy_Poe", [72.35, 108.56, 0.62], [72.35, 108.46, 4.34]))
        self.assertEqual(len(g17["spots"]), 3)
        self.assertEqual(gs[0]["name"], "the path, with a comma")
        self.assertEqual(gs[0]["kind"], "")
        self.assertEqual(gs[1]["kind"], "Enemy_DeadHand")
        self.assertEqual(gs[1]["also"], [[[4.48, 100.26, -1.96], [4.52, 100.26, 2.08]]])
        self.assertTrue(all(g["shipped"] and g["guards"] == [] for g in gs))

    def test_real_gates_rs_has_the_graveyard_gates(self):
        gs, where = SG.load()
        if where is None:
            self.skipTest("no gates.rs on this PC")
        with open(where, encoding="utf-8") as f:
            rows: int = len([x for x in f.read().splitlines() if x.strip().startswith("Gate { id:")])
        self.assertEqual(len(gs), rows)
        maps: dict[int, str] = {g["id"]: g["map"] for g in gs}
        self.assertEqual((maps.get(17), maps.get(18)), ("Graveyard_Child", "Graveyard_Adult"))
        self.assertTrue(all(g["map"] and g["spots"] and G.complete(g) for g in gs))

    def test_missing_file_gives_none(self):
        self.assertEqual(SG.load(os.path.join(tempfile.gettempdir(), "no-such-gates.rs")), ([], None))

    def test_merge_save_revert(self):
        shipped: list[dict] = SG.parse(RS)
        own: list[dict] = [{"id": 6, "map": "A", "left": [0, 0, 0], "right": [3, 0, 0], "guards": []}]
        gates: list[dict] = G.merge(own, shipped)
        self.assertEqual(sorted(g["id"] for g in gates), [6, 11, 17, 18])
        self.assertEqual(G.to_save(gates, shipped), [g for g in gates if g["map"] == "A"])
        self.assertEqual(G.next_id(gates), 19)
        g17: dict = G.find(gates, 17)
        g, how = G.place_post(gates, "Graveyard_Child", [72.35, 108.56, 1.0], "left")
        self.assertEqual((g["id"], how), (17, "moved"))
        self.assertTrue(G.edited(g17, shipped))
        saved: list[dict] = G.to_save(gates, shipped)
        self.assertEqual(sorted(x["id"] for x in saved), [17, 18])
        again: list[dict] = G.merge(json.loads(json.dumps(saved)), shipped)
        self.assertEqual(G.find(again, 17)["left"], [72.35, 108.56, 1.0])
        self.assertEqual(len(again), 4)
        self.assertTrue(G.revert(gates, shipped, 17))
        self.assertFalse(G.edited(G.find(gates, 17), shipped))
        self.assertFalse(G.revert(gates, shipped, 18))

    def test_radar_shows_shipped_posts_and_guard_spots(self):
        gates: list[dict] = G.merge([], SG.parse(RS))
        lines: list[str] = G.radar_lines(gates, "Graveyard_Child")
        self.assertEqual(lines[:2], ["p 72.35 108.56 0.62 Gate 17 L", "p 72.35 108.46 4.34 Gate 17 R"])
        self.assertEqual(lines[2], "p 67.60 108.64 2.50 G17 Poe")
        self.assertEqual(len(lines), 5)
        self.assertTrue(all(len(x.split(" ", 4)[4]) <= 15 for x in lines))
        self.assertIn("G6 vanilla", G.radar_lines(gates, "KokiriForest_Child")[2])
        self.assertEqual([x.split(" ", 4)[4] for x in G.radar_lines(gates, "Market_Adult")], ["Gate 11 L", "Gate 11 R", "Gate 11 L2", "Gate 11 R2", "G11 DeadHand"])

    def test_export_skips_unedited_and_uses_spots_for_edits(self):
        shipped: list[dict] = SG.parse(RS)
        gates: list[dict] = G.merge([], shipped)
        session, _ = G.export_session(gates, [], shipped)
        self.assertEqual(session["gates"], [])
        G.place_post(gates, "Graveyard_Child", [72.35, 108.56, 0.0], "left")
        session, notes = G.export_session(gates, [], shipped)
        self.assertEqual([g["id"] for g in session["gates"]], [17])
        self.assertEqual([k["kind"] for k in session["gates"][0]["guards"]], ["Enemy_Poe"] * 3)
        self.assertIn("edit of gates.rs gate 17", session["gates"][0]["label"])
        self.assertTrue(any("shipped" in n for n in notes))

    def test_warp_list(self):
        gates: list[dict] = G.merge([{"id": 30, "map": "A", "left": [1, 2, 3], "right": None, "guards": []}], SG.parse(RS))
        w: list[dict] = G.warp_list(gates, {17: [63.6, 109.2, 2.52]})
        self.assertEqual([x["id"] for x in w], [6, 11, 17, 30])
        by: dict[int, dict] = {x["id"]: x for x in w}
        self.assertEqual((by[17]["pos"], by[17]["how"], by[17]["map"]), ([63.6, 109.2, 2.52], "measured", "Graveyard_Child"))
        self.assertEqual((by[11]["pos"], by[11]["how"]), ([-1.67, 100.07, 32.58], "guard spot"))
        self.assertEqual((by[30]["pos"], by[30]["how"]), ([1.0, 2.0, 3.0], "post"))
        self.assertEqual(by[17]["label"], "Gate 17: the Graveyard, the way to the Shadow Temple")
        self.assertEqual(by[6]["age"], "child")

    def test_gate_warps_moved_out_of_the_places_list(self):
        with open(os.path.join(_paths.DATA, "gate_warps.json"), encoding="utf-8") as f:
            warps: list[dict] = json.load(f)
        measured: dict[int, list[float]] = G.measured_warps(warps)
        self.assertEqual(measured[17], [63.6, 109.2, 2.52])
        self.assertEqual(len(measured), 11)
        for name in ("places.json",):
            with open(os.path.join(_paths.DATA, name), encoding="utf-8-sig") as f:
                places: list[dict] = json.load(f)
            self.assertEqual([p["label"] for p in places if p.get("label", "").startswith("Gate ") or p.get("group") == "Gates"], [], name)


class FakeGame:
    def __init__(self) -> None:
        self.st: dict = {"map": "A", "pos": [0.0, 0.0, 0.0], "facing": 0.0}
        self.captions: list[tuple[str, str, str]] = []
        self.sent: list[tuple] = []

    def status(self) -> dict:
        return dict(self.st)

    def caption(self, test: str, expect: str, result: str) -> None:
        self.captions.append((test, expect, result))

    def ctl(self, *args: str, timeout: int = 30, retries: int = 6) -> str:
        self.sent.append(args)
        if args[0] == "teleport":
            self.st = {"map": args[1], "pos": [0.0, 0.0, 0.0], "facing": 0.0}
        if args[0] == "warp":
            self.st["pos"] = [float(v) for v in args[1:4]]
        return "ok"

    def sleep(self, s: float) -> None:
        pass


class FakePlacer:
    def __init__(self, folder: str, rs: str | None = None) -> None:
        self.overlay_flags: dict = {k: k not in ("models", "gate_models") for k in W.LAYERS}
        self.aim_on: bool = False
        self.aim_for: str = "enemy"
        self.game: FakeGame = FakeGame()
        self.points_path: str = os.path.join(folder, "spawnpoints.json")
        self.points: list[dict] = [pt(1, "Enemy_Lizalfos", 2, 4), pt(2, "Enemy_Keese", 2, 6)]
        self.status: dict = self.game.status()
        self.gates_rs: str = rs or os.path.join(folder, "no-gates.rs")


class Routes(unittest.TestCase):
    def setUp(self):
        self.tmp: str = tempfile.mkdtemp()
        self.p: FakePlacer = FakePlacer(self.tmp)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def post(self, op: str) -> dict:
        code, body, kind = R.post(self.p, "/api/gate/" + op, {})
        self.assertEqual(code, 200)
        return json.loads(body)

    def state(self) -> dict:
        code, body, _ = R.get(self.p, "/api/gate/state")
        return json.loads(body)

    def test_flow(self):
        self.assertIsNone(self.state()["near"])
        self.assertFalse(self.post("markguard")["ok"])
        self.assertFalse(self.post("remove")["ok"])
        self.assertTrue(self.post("left")["ok"])
        self.p.game.st["pos"] = [4.0, 0.0, 0.0]
        self.p.status = self.p.game.status()
        self.assertTrue(self.post("right")["ok"])
        s: dict = self.state()
        self.assertEqual((s["near"]["id"], s["near"]["width"]), (1, 4.0))
        self.assertEqual((s["will"]["right"]["how"], s["will"]["left"]["how"]), ("moved", "moved"))
        self.assertEqual(s["mark"]["n"], 1)
        self.assertTrue(self.post("markguard")["ok"])
        self.assertTrue(self.post("markguard")["ok"])
        s = self.state()
        self.assertEqual([g["n"] for g in s["near"]["guards"]], [1, 2])
        self.assertIsNone(s["mark"])
        self.assertEqual(s["unmark"]["n"], 1)
        self.assertTrue(self.post("unmarkguard")["ok"])
        with open(os.path.join(self.tmp, G.FILE_NAME), encoding="utf-8") as f:
            saved: list[dict] = json.load(f)
        self.assertEqual(saved[0]["guards"], [2])
        self.assertEqual(saved[0]["left"], [0.0, 0.0, 0.0])
        self.assertTrue(any("Gate 1" in c[0] for c in self.p.game.captions))
        args: dict = R.radar_args(self.p, "A")
        self.assertEqual(args["tags"], {2: " G1"})
        self.assertEqual(len(args["extra"]), 3)
        self.p.points = [x for x in self.p.points if x["n"] != 2]
        self.assertEqual(self.state()["near"]["guards"], [])
        self.assertTrue(self.post("remove")["ok"])
        self.assertIsNone(self.state()["near"])
        self.assertIsNone(R.post(self.p, "/api/press", {}))
        self.assertIsNone(R.get(self.p, "/api/state"))
        code, page, kind = R.get(self.p, "/gate")
        self.assertIn(b"Enemy-Gate Manager", page)

    def test_not_ready(self):
        self.p.game.st = {}
        self.p.status = {}
        self.assertFalse(self.post("left")["ok"])
        self.assertFalse(self.state()["connected"])


class ShippedRoutes(unittest.TestCase):
    def setUp(self):
        self.tmp: str = tempfile.mkdtemp()
        rs: str = os.path.join(self.tmp, "gates.rs")
        with open(rs, "w", encoding="utf-8") as f:
            f.write(RS)
        self.p: FakePlacer = FakePlacer(self.tmp, rs)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def at(self, m: str, pos: list[float]) -> None:
        self.p.game.st = {"map": m, "pos": list(pos), "facing": 0.0}
        self.p.status = self.p.game.status()

    def post(self, op: str, body: dict | None = None) -> dict:
        return json.loads(R.post(self.p, "/api/gate/" + op, body or {})[1])

    def state(self) -> dict:
        return json.loads(R.get(self.p, "/api/gate/state")[1])

    def test_the_graveyard_gate_shows_and_edits_as_a_copy(self):
        self.at("Graveyard_Child", [67.6, 108.64, 2.5])
        s: dict = self.state()
        self.assertEqual((s["near"]["id"], s["near"]["shipped"], s["near"]["edited"]), (17, True, False))
        self.assertEqual(s["near"]["name"], "the Graveyard, the way to the Shadow Temple")
        self.assertEqual(len(s["near"]["spots"]), 3)
        self.assertEqual((s["source"]["count"], s["on_map"]), (3, 1))
        r: dict = self.post("remove")
        self.assertFalse(r["ok"])
        self.assertIn("gates.rs", r["message"])
        self.assertFalse(os.path.exists(os.path.join(self.tmp, G.FILE_NAME)))
        self.at("Graveyard_Child", [72.0, 108.5, 0.5])
        self.assertTrue(self.post("left")["ok"])
        self.assertTrue(self.state()["near"]["edited"])
        with open(os.path.join(self.tmp, G.FILE_NAME), encoding="utf-8") as f:
            self.assertEqual([g["id"] for g in json.load(f)], [17])
        self.assertTrue(self.post("remove")["ok"])
        self.assertFalse(self.state()["near"]["edited"])
        with open(os.path.join(self.tmp, G.FILE_NAME), encoding="utf-8") as f:
            self.assertEqual(json.load(f), [])
        self.at("Graveyard_Child", [0.0, 100.0, 0.0])
        self.assertTrue(self.post("left")["ok"])
        self.at("Graveyard_Child", [4.0, 100.0, 0.0])
        self.assertTrue(self.post("right")["ok"])
        self.assertEqual(self.state()["near"]["id"], 18)

    def test_overlay_toggle_hides_the_gate_layer(self):
        self.assertTrue(self.state()["overlay"])
        self.assertEqual(len(R.radar_args(self.p, "Graveyard_Child")["extra"]), 6)
        r: dict = self.post("overlay")
        self.assertEqual((r["ok"], r["on"]), (True, False))
        self.assertEqual(R.radar_args(self.p, "Graveyard_Child"), {"extra": [], "tags": {}})
        self.assertFalse(self.state()["overlay"])
        with open(os.path.join(self.tmp, "panel_overlay.json"), encoding="utf-8") as f:
            self.assertFalse(json.load(f)["gates"])
        self.assertTrue(self.post("overlay", {"on": True})["on"])
        self.assertEqual(len(R.radar_args(self.p, "Graveyard_Child")["extra"]), 6)

    def test_warp_to_gate_teleports_then_warps(self):
        w: list[dict] = json.loads(R.get(self.p, "/api/gate/warps")[1])["gates"]
        self.assertEqual([x["id"] for x in w], [6, 11, 17])
        self.assertEqual(w[2]["how"], "measured")
        self.assertFalse(self.post("goto", {"id": 99})["ok"])
        self.assertFalse(self.post("goto", {"id": "x"})["ok"])
        self.assertTrue(self.post("goto", {"id": 17})["ok"])
        run = W.runner(self.p)
        for _ in range(100):
            if not run.busy:
                break
            time.sleep(0.02)
        self.assertEqual(run.ok, True, run.message)
        sent: list[str] = [c[0] for c in self.p.game.sent]
        self.assertEqual(sent, ["teleport", "warp"])
        self.assertEqual(self.p.game.st["map"], "Graveyard_Child")
        self.assertEqual(self.p.game.st["pos"], [63.6, 109.2, 2.52])
        self.assertIn("arrived", self.state()["travel"]["message"])


SESSION_GATE5: dict = {"id": 5, "map": "ZoraRiver_Child", "name": "Zora's River, the upper narrows", "left": [61.43, 108.87, -21.08], "right": [60.85, 108.64, -14.65],
                       "spots": [[56.6, 108.5, -16.0], [61.8, 104.5, -9.9]], "also": [], "kind": "Enemy_Lizalfos", "guards": [], "shipped": True}
SESSION_POINTS: list[dict] = [{"n": 13, "map": "ZoraRiver_Child", "name": "Enemy_Octorok", "pos": [62.86, 104.5, -9.44], "facing": -154.7, "yaw": -154.7, "time": "18:00:18", "via": "aim"}]


class GuardRows(unittest.TestCase):
    def setUp(self):
        self.tmp: str = tempfile.mkdtemp()
        self.p: FakePlacer = FakePlacer(self.tmp)
        self.p.points = [dict(p) for p in SESSION_POINTS]
        R.manager(self.p).gates = [json.loads(json.dumps(SESSION_GATE5))]
        R.manager(self.p).shipped = [json.loads(json.dumps(SESSION_GATE5))]
        self.at([61.0, 104.5, -10.0])

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def at(self, pos: list[float]) -> None:
        self.p.game.st = {"map": "ZoraRiver_Child", "pos": list(pos), "facing": 0.0}
        self.p.status = self.p.game.status()

    def post(self, op: str, body: dict | None = None) -> dict:
        return json.loads(R.post(self.p, "/api/gate/" + op, body or {})[1])

    def state(self) -> dict:
        return json.loads(R.get(self.p, "/api/gate/state")[1])

    def test_the_list_shows_the_shipped_guards_the_overlay_draws(self):
        n: dict = self.state()["near"]
        self.assertEqual([(r["source"], r["label"]) for r in n["rows"]], [("shipped", "Lizalfos")] * 2)
        lines: list[str] = G.radar_lines(R.manager(self.p).gates, "ZoraRiver_Child")
        self.assertEqual(len([x for x in lines if " G5 Lizalfos" in x]), len(n["rows"]))

    def test_unmark_removes_the_nearest_shipped_spot_never_the_octorok(self):
        r: dict = self.post("unmarkguard")
        self.assertTrue(r["ok"], r)
        self.assertEqual(len(R.manager(self.p).gates[0]["spots"]), 1)
        self.assertEqual(len(self.p.points), 1)
        self.assertEqual(R.manager(self.p).gates[0]["spots"], [[56.6, 108.5, -16.0]])
        self.assertTrue(self.state()["near"]["edited"])

    def test_model_note_says_when_the_game_has_no_display_model(self):
        self.post("models", {"on": True})
        self.assertIn("miniboss Lizalfos model", self.state()["model_note"])
        self.p.ph_skipped = ["Enemy_MiddleBoss_Lizalfos"]
        self.assertEqual(self.state()["model_note"], "no display model for Lizalfos: wire box only")
        self.post("models", {"on": False})
        self.assertEqual(self.state()["model_note"], "")

    def test_far_from_every_guard_names_the_nearest_and_changes_nothing(self):
        self.at([30.0, 106.0, -20.0])
        self.post("select", {"id": 5})
        r: dict = self.post("unmarkguard")
        self.assertFalse(r["ok"])
        self.assertIn("nearest guard is Lizalfos", r["message"])
        self.assertEqual(len(R.manager(self.p).gates[0]["spots"]), 2)

    def test_row_button_deletes_that_row_from_anywhere(self):
        self.at([30.0, 106.0, -20.0])
        self.post("select", {"id": 5})
        self.assertTrue(self.post("delguard", {"id": 5, "source": "shipped", "i": 0})["ok"])
        self.assertEqual(R.manager(self.p).gates[0]["spots"], [[61.8, 104.5, -9.9]])
        self.assertFalse(self.post("delguard", {"id": 5, "source": "shipped", "i": 5})["ok"])
        self.assertTrue(self.post("remove")["ok"])
        self.assertEqual(len(R.manager(self.p).gates[0]["spots"]), 2)

    def test_marked_guards_list_and_delete(self):
        self.post("markguard")
        n: dict = self.state()["near"]
        self.assertEqual([(r["source"], r["n"]) for r in n["rows"]], [("marked", 13)])
        self.assertEqual(self.state()["unmark"]["n"], 13)
        self.assertTrue(self.post("delguard", {"id": 5, "source": "marked", "n": 13})["ok"])
        self.assertEqual(len(self.state()["near"]["rows"]), 2)
        self.assertEqual(len(self.p.points), 1)


class Goto(unittest.TestCase):
    def run_go(self, states: list[dict], target: str = "M", pos: list[float] | None = None):
        seen: list[tuple] = []
        box: dict = {"i": 0}

        def status() -> dict:
            s: dict = states[min(box["i"], len(states) - 1)]
            box["i"] += 1
            return s

        def send(*c: str) -> str:
            seen.append(c)
            return "refused" if c[0] == "warp" and len([x for x in seen if x[0] == "warp"]) < 2 else "ok"
        t: dict = {"now": 0.0}

        def sleep(s: float) -> None:
            t["now"] += s
        ok, msg = goto.go(status, send, target, pos or [1.0, 2.0, 3.0], sleep=sleep, clock=lambda: t["now"])
        return ok, msg, seen

    def test_waits_for_the_event_then_teleports_and_retries_the_warp(self):
        here: dict = {"map": "M", "pos": [1.0, 2.0, 3.0]}
        ok, msg, seen = self.run_go([{"map": "X", "pos": [0, 0, 0], "event": True}, {"map": "X", "pos": [0, 0, 0]}, {"map": "M", "loading": True}, here, here, here])
        self.assertTrue(ok, msg)
        self.assertEqual([c[0] for c in seen], ["teleport", "warp", "warp"])

    def test_never_warps_in_the_wrong_map(self):
        ok, msg, seen = self.run_go([{"map": "X", "pos": [0, 0, 0]}])
        self.assertFalse(ok)
        self.assertIn("not in M", msg)
        self.assertTrue(seen and all(c[0] == "teleport" for c in seen))

    def test_arrived_needs_a_fresh_play_state_at_the_target(self):
        here: dict = {"map": "M", "pos": [1.0, 2.0, 3.0]}
        stale: dict = {"map": "M", "pos": [1.0, 2.0, 3.0], "loading": True}
        ok, msg, seen = self.run_go([here, stale, stale, stale, here])
        self.assertTrue(ok, msg)
        ok, msg, seen = self.run_go([here] + [stale] * 20)
        self.assertFalse(ok)
        self.assertIn("NOT", msg)
        ok, msg, seen = self.run_go([here] + [{"map": "M", "pos": [1.0, 2.0, 3.0], "state": "loading"}] * 20)
        self.assertFalse(ok)

    def test_wrong_position_is_not_arrival(self):
        ok, msg, seen = self.run_go([{"map": "M", "pos": [50.0, 2.0, 3.0]}])
        self.assertFalse(ok)
        self.assertIn("NOT at the target", msg)

    def test_hung_load_is_reported(self):
        ok, msg, seen = self.run_go([{"map": "X", "pos": [0, 0, 0], "loading": True}])
        self.assertFalse(ok)
        self.assertIn("load seems hung", msg)

    def test_exit_codes_documented(self):
        self.assertIn("4 the load seems hung", goto.__doc__)


    def test_gives_up_when_the_game_is_gone(self):
        ok, msg, seen = self.run_go([{}])
        self.assertFalse(ok)
        self.assertIn("answering", msg)
        self.assertEqual(seen, [])


class Selection(unittest.TestCase):
    def setUp(self):
        self.gate: dict = {"id": 1, "map": "A", "left": [0.0, 0.0, 0.0], "right": [8.0, 0.0, 0.0], "guards": [7], "shipped": False}
        self.pts: list[dict] = [pt(7, "Enemy_Lizalfos", 4.0, 26.0)]

    def pick(self, pos, sticky=None, pinned=None, gates=None):
        return G.select_gate(gates or [self.gate], "A", pos, self.pts, sticky, pinned)

    def test_standing_at_the_gate_selects_it(self):
        self.assertEqual(self.pick([4.0, 0.0, 3.0]), (self.gate, "auto"))

    def test_standing_at_the_guard_keeps_the_gate_selected(self):
        g, how = self.pick([4.0, 0.0, 26.0])
        self.assertIs(g, self.gate)
        self.assertIsNone(G.nearest_gate([self.gate], "A", [4.0, 0.0, 26.0]))

    def test_the_walk_from_the_gate_to_the_guard_stays_selected(self):
        for z in range(0, 27, 2):
            self.assertIsNotNone(self.pick([4.0, 0.0, float(z)])[0], z)

    def test_far_from_wall_and_guard_selects_nothing(self):
        self.assertEqual(self.pick([60.0, 0.0, 60.0]), (None, ""))
        self.assertEqual(self.pick([4.0, 0.0, 26.0 + G.GATE_NEAR + 3.0]), (None, ""))

    def test_the_last_gate_sticks_a_few_metres_further(self):
        pos: list[float] = [4.0, 0.0, 26.0 + G.GATE_NEAR + 3.0]
        self.assertEqual(self.pick(pos, sticky=1), (self.gate, "auto"))
        self.assertEqual(self.pick([4.0, 0.0, 26.0 + G.GATE_NEAR + G.STICK_EXTRA + 1.0], sticky=1), (None, ""))
        other: dict = {"id": 2, "map": "A", "left": [0.0, 0.0, 60.0], "right": [8.0, 0.0, 60.0], "guards": []}
        self.assertEqual(self.pick([4.0, 0.0, 55.0], sticky=1, gates=[self.gate, other]), (other, "auto"))
        self.assertEqual(self.pick([4.0, 0.0, 40.0], sticky=1, gates=[self.gate, other])[0]["id"], 1)

    def test_a_pinned_gate_stays_wherever_link_is(self):
        other: dict = {"id": 2, "map": "A", "left": [100.0, 0.0, 0.0], "right": [108.0, 0.0, 0.0], "guards": []}
        self.assertEqual(self.pick([4.0, 0.0, 3.0], pinned=2, gates=[self.gate, other]), (other, "pinned"))
        self.assertEqual(self.pick([500.0, 0.0, 500.0], pinned=1)[1], "pinned")
        self.assertEqual(self.pick([4.0, 0.0, 3.0], pinned=9)[1], "auto")

    def test_a_shipped_gates_own_spots_count_as_guards(self):
        g: dict = {"id": 5, "map": "A", "left": [0.0, 0.0, 0.0], "right": [8.0, 0.0, 0.0], "guards": [], "spots": [[4.0, 0.0, 30.0]], "shipped": True}
        self.assertIs(self.pick([4.0, 0.0, 30.0], gates=[g])[0], g)
        self.assertEqual(G.area_dist(g, [4.0, 0.0, 30.0], []), 0.0)

    def test_nearest_area_wins_between_two_gates(self):
        other: dict = {"id": 2, "map": "A", "left": [0.0, 0.0, 100.0], "right": [8.0, 0.0, 100.0], "guards": []}
        self.assertEqual(self.pick([4.0, 0.0, 96.0], gates=[self.gate, other])[0]["id"], 2)


class WireBoxes(unittest.TestCase):
    def test_a_wall_is_a_thin_box_turned_along_it(self):
        line: str = G.wall_line([0.0, 1.0, 0.0], [8.0, 1.0, 0.0], "Gate 3")
        self.assertEqual(line, "P 4.00 1.00 0.00 0.0 [-4.00 0.00 -0.20 4.00 2.50 0.20] Gate 3")
        along_z: list[str] = G.wall_line([0.0, 0.0, 0.0], [0.0, 0.0, 6.0], "Gate 4").split(" ")
        self.assertEqual(float(along_z[4]), -90.0)
        self.assertEqual(along_z[:4], ["P", "0.00", "0.00", "3.00"])

    def test_a_wall_box_runs_along_the_posts_in_the_games_own_maths(self):
        a, b = [3.0, 0.0, 2.0], [9.0, 0.0, 10.0]
        f: list[str] = G.wall_line(a, b, "Gate 1").split(" ")
        yaw: float = math.radians(float(f[4]))
        right: tuple = (math.cos(yaw), -math.sin(yaw))
        d: float = math.hypot(b[0] - a[0], b[2] - a[2])
        self.assertAlmostEqual(right[0], (b[0] - a[0]) / d, places=3)
        self.assertAlmostEqual(right[1], (b[2] - a[2]) / d, places=3)

    def test_box_lines_cover_walls_and_shipped_guard_spots_but_not_marked_guard_points(self):
        gates: list[dict] = G.merge([], SG.parse(RS))
        lines: list[str] = G.box_lines(gates, "Graveyard_Child")
        self.assertEqual(len(lines), 4)
        self.assertTrue(all(x.startswith("P ") and "[" in x and len(x.split("] ", 1)[1]) <= 15 for x in lines))
        own: list[dict] = [{"id": 1, "map": "A", "left": [0.0, 0.0, 0.0], "right": [4.0, 0.0, 0.0], "guards": [7], "also": []}]
        self.assertEqual(len(G.box_lines(own, "A")), 1)
        self.assertEqual(G.guard_ns(own, "A"), {7})
        self.assertEqual(G.guard_ns(own, "B"), set())

    def test_the_marked_guard_point_gets_its_models_box_with_no_boxes_layer(self):
        pts: list[dict] = [pt(7, "Enemy_Lizalfos", 4.0, 26.0), pt(8, "Enemy_Keese", 1.0, 1.0)]
        boxes: dict = {"Enemy_Lizalfos": [[-0.6, 0.0, -0.6], [0.6, 2.0, 0.6]], "Enemy_Keese": [[-0.3, 0.0, -0.3], [0.3, 0.5, 0.3]]}
        marks: list[str] = L.radar_marks(None, pts, "A", False, box_ns={7}, all_boxes=boxes).split("\n")
        self.assertIn("[-0.60 0.00 -0.60 0.60 2.00 0.60]", marks[0])
        self.assertTrue(marks[1].startswith("p "))
        every: list[str] = L.radar_marks(None, pts, "A", False, boxes=boxes).split("\n")
        self.assertTrue(all("[" in x for x in every))

    def test_display_models_for_the_guards_only(self):
        gate: dict = {"id": 1, "map": "A", "left": [0.0, 0.0, 0.0], "right": [8.0, 0.0, 0.0], "guards": [7, 9]}
        pts: list[dict] = [pt(7, "Enemy_Lizalfos", 4.0, 26.0), pt(8, "Enemy_Keese", 1.0, 1.0), pt(9, "Enemy_Wolfos", 6.0, 20.0)]
        lines: list[str] = G.placeholder_lines([gate], pts, "A", L.NO_DICTIONARY)
        self.assertEqual(len(lines), 2)
        self.assertTrue(any(x.startswith("Enemy_MiddleBoss_Lizalfos 4.00 0.00 26.00 ") for x in lines))
        self.assertTrue(any(x.startswith("Enemy_Wolfos 6.00 0.00 20.00 ") for x in lines))
        refused: list[str] = G.placeholder_lines([gate], pts, "A", L.NO_DICTIONARY, {"Enemy_MiddleBoss_Lizalfos"})
        self.assertEqual(len(refused), 1)
        self.assertTrue(refused[0].startswith("Enemy_Wolfos 6.00 0.00 20.00 "))
        shipped: list[dict] = G.merge([], SG.parse(RS))
        self.assertTrue(all(x.split(" ")[0].startswith("Enemy_") for x in G.placeholder_lines(shipped, [], "Market_Adult")))
        self.assertEqual(len(G.placeholder_lines(shipped, [], "Market_Adult")), 1)


class SelectRoutes(unittest.TestCase):
    def setUp(self):
        self.tmp: str = tempfile.mkdtemp()
        self.p: FakePlacer = FakePlacer(self.tmp)
        self.syncs: list[bool] = []
        self.p.overlay_sync = self.syncs.append
        g, _ = G.place_post(R.manager(self.p).gates, "A", [0.0, 0.0, 0.0], "left")
        G.place_post(R.manager(self.p).gates, "A", [8.0, 0.0, 0.0], "right")
        G.mark_guard(R.manager(self.p).gates, 1, 1)
        self.p.points[0]["pos"] = [4.0, 0.0, 26.0]
        self.p.points[0]["name"] = "Enemy_Wolfos"

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def post(self, op: str, body: dict | None = None) -> dict:
        code, text, _ = R.post(self.p, "/api/gate/" + op, body or {})
        return json.loads(text)

    def state(self) -> dict:
        return json.loads(R.get(self.p, "/api/gate/state")[1])

    def at(self, x: float, z: float) -> None:
        self.p.game.st["pos"] = [x, 0.0, z]
        self.p.status = self.p.game.status()

    def test_the_gate_stays_selected_at_its_guard_and_in_between(self):
        self.at(4.0, 3.0)
        self.assertEqual(self.state()["near"]["id"], 1)
        for z in (10.0, 20.0, 26.0):
            self.at(4.0, z)
            s: dict = self.state()
            self.assertEqual(s["near"]["id"], 1, z)
            self.assertEqual(s["how"], "auto")
        self.at(4.0, 80.0)
        self.assertIsNone(self.state()["near"])
        self.assertEqual(self.state()["closest"]["id"], 1)

    def test_remove_and_mark_work_from_the_guard(self):
        self.at(4.0, 26.0)
        self.assertTrue(self.post("unmarkguard")["ok"])
        self.assertTrue(self.post("markguard")["ok"])
        self.assertTrue(self.post("remove")["ok"])

    def test_pick_by_hand_sticks_until_auto(self):
        self.at(4.0, 90.0)
        self.assertIsNone(self.state()["near"])
        self.assertFalse(self.post("select", {"id": 9})["ok"])
        self.assertTrue(self.post("select", {"id": 1})["ok"])
        s: dict = self.state()
        self.assertEqual((s["near"]["id"], s["pinned"], s["near"]["how"]), (1, 1, "pinned"))
        self.assertEqual([x["id"] for x in s["gates_here"]], [1])
        self.at(300.0, 300.0)
        self.assertEqual(self.state()["near"]["id"], 1)
        self.assertTrue(self.post("select", {"id": "auto"})["ok"])
        self.assertIsNone(self.state()["near"])

    def test_models_switch_and_the_overlay_switch_are_the_gates_own(self):
        self.assertFalse(self.state()["models"])
        self.assertEqual(self.post("models")["on"], True)
        self.assertTrue(self.state()["models"])
        self.assertEqual(R.placeholder_lines(self.p, "A")[0].split(" ")[0], "Enemy_Wolfos")
        self.assertEqual(self.post("models")["on"], False)
        self.assertEqual(R.placeholder_lines(self.p, "A"), [])
        self.assertEqual(self.post("overlay", {"on": False})["on"], False)
        self.assertEqual(self.post("overlay", {"on": True})["on"], True)
        self.assertEqual(self.syncs[-1], True)

    def test_a_vanilla_guard_shows_the_games_own_kind(self):
        import gate_logic
        gate_logic.OWN.clear()
        gate_logic.VANILLA = {"A": {"enemies": [{"n": "Enemy_Dekubaba_Withered", "p": [1, 0, 1], "r": None}, {"n": "Enemy_Keese", "p": [30, 0, 30], "r": None}]}}
        g = {"id": 6, "map": "A", "kind": "", "spots": [[1.5, 0.0, 1.0]], "guards": [], "left": [0, 0, 0], "right": [3, 0, 0]}
        self.assertEqual(gate_logic.guard_label(g), "Deku Baba (the game's own)")
        self.assertEqual(gate_logic.radar_lines([g], "A")[-1], "p 1.50 0.00 1.00 G6 Deku Baba")
        self.assertEqual(gate_logic.guard_entries(g, [])[0]["label"], "Deku Baba (the game's own)")
        far = dict(g, id=7, spots=[[90.0, 0.0, 90.0]])
        self.assertEqual(gate_logic.guard_label(far), "vanilla guard")
        self.assertEqual(gate_logic.guard_label(dict(g, kind="Enemy_Wolfos")), "Wolfos")
        gate_logic.VANILLA = {}
        gate_logic.OWN.clear()

    def test_in_game_overlay_goes_off_with_the_last_layer(self):
        for layer in W.SHOWING:
            W.toggle(self.p, layer, False)
        self.assertEqual(self.syncs[-1], False)
        W.toggle(self.p, "gs", True)
        self.assertEqual(self.syncs[-1], True)
        W.toggle(self.p, "boxes", False)
        self.assertEqual(self.syncs[-1], True)


if __name__ == "__main__":
    unittest.main()
