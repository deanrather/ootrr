import json
import time
import math
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _paths  # noqa: E402
import placer_logic as L
import warp_runner as W

os.environ["PLACER_NO_THREADS"] = "1"


def pt(n, name, x, z, m="A"):
    return {"n": n, "map": m, "name": name, "pos": [x, 0.0, z], "facing": 0, "time": "t"}


class Logic(unittest.TestCase):
    def test_by_hp(self):
        cat = [("Enemy_ReDead", "ReDead"), ("Enemy_Keese", "Keese"), ("Enemy_Nope", "Nope"), ("Enemy_Lizalfos", "Lizalf")]
        hp = {"Enemy_ReDead": 12.0, "Enemy_Keese": 1.0, "Enemy_Lizalfos": 9.0}
        self.assertEqual([c[0] for c in L.by_hp(cat, hp)], ["Enemy_Keese", "Enemy_Lizalfos", "Enemy_ReDead", "Enemy_Nope"])
        real = L.load_hp()
        if real:
            self.assertLess(real["Enemy_Keese"], real["Enemy_ReDead"])

    def test_facing_and_repeat(self):
        a = L.make_point([], "A", "Enemy_Keese", [0, 0, 0], 30.0, "t")
        b = L.make_point([a], "A", "Enemy_ReDead", [0, 0, 0], 30.0, "t", face_link=True)
        self.assertAlmostEqual(L.point_yaw(a), math.radians(30.0))
        self.assertAlmostEqual(L.point_yaw(b), math.radians(210.0))
        self.assertAlmostEqual(L.point_yaw({"facing": 30.0}), math.radians(210.0))
        self.assertEqual(L.last_name([a, b], "A"), "Enemy_ReDead")
        self.assertEqual(L.last_name([a, b], "B"), "Enemy_ReDead")
        self.assertIsNone(L.last_name([], "A"))

    def test_spawn_spot(self):
        entry = {"enemies": [{"n": "Enemy_Skulltula", "p": [2, 110, 0], "r": 1}, {"n": "Enemy_Skulltula", "p": [1, 140, 0], "r": 1},
                             {"n": "Enemy_Skulltula", "p": [0, 105, 0], "r": 2}]}
        here = [0.0, 100.0, 0.0]
        self.assertAlmostEqual(L.spawn_spot("Enemy_Skulltula", here, entry, 1)[1], 100.0 + L.HANG_AT)
        self.assertAlmostEqual(L.roof_guess(entry, here, 1), 110.0)
        self.assertAlmostEqual(L.spawn_spot("Enemy_Keese_Fire", here, entry, 1)[1], 102.0)
        self.assertEqual(L.spawn_spot("Enemy_Dodongo", here, entry, 1), here)

    def test_hit_ignores_height(self):
        pts = [{"n": 1, "map": "A", "name": "Enemy_Keese", "pos": [0.0, 102.0, 0.5], "facing": 0, "time": "t"}]
        self.assertIn("Enemy_Keese", L.hit_points(pts, "A", [0.0, 100.0, 0.0]))
        self.assertNotIn("Enemy_Keese", L.hit_points(pts, "A", [0.0, 80.0, 0.0]))

    def test_suggested(self):
        entry = {"enemies": [{"n": "Enemy_Keese_Fire", "p": [0, 0, 0], "r": 1}, {"n": "Enemy_Dodongo", "p": [0, 0, 0], "r": 2},
                             {"n": "Enemy_Beamos", "p": [0, 0, 0], "r": 2}, {"n": "Enemy_DungeonBoss_Dodongo", "p": [0, 0, 0], "r": 2}]}
        self.assertEqual(L.suggested(entry, 2), ["Enemy_Dodongo", "Enemy_Beamos", "Enemy_Keese_Fire"])
        self.assertEqual(L.suggested(None, None), [])

    def test_late_spawn(self):
        old = {"id": 1, "name": "Enemy_ReDead", "x": 0.0, "y": 0.0, "z": 0.5, "hp": 12}
        new = {"id": 2, "name": "Enemy_ReDead", "x": 1.0, "y": 0.0, "z": 0.0, "hp": 12}
        far = {"id": 3, "name": "Enemy_ReDead", "x": 9.0, "y": 0.0, "z": 0.0, "hp": 12}
        self.assertEqual(L.late_spawn([old], [old, new], "Enemy_ReDead", [0.0, 0.0, 0.0])["id"], 2)
        self.assertIsNone(L.late_spawn([old], [old, far], "Enemy_ReDead", [0.0, 0.0, 0.0]))
        self.assertIsNone(L.late_spawn([old], [old], "Enemy_ReDead", [0.0, 0.0, 0.0]))

    def test_nearby_counts(self):
        pts = [pt(1, "Enemy_Keese", 0, 5), pt(2, "Enemy_Keese", 0, 29), pt(3, "Enemy_Keese", 0, 31), pt(4, "Enemy_Poe", 1, 1), pt(5, "Enemy_Poe", 0, 0, "B")]
        self.assertEqual(L.nearby_counts(pts, "A", [0, 0, 0]), {"Enemy_Keese": 2, "Enemy_Poe": 1})

    def test_hit_nearest_and_radius(self):
        pts = [pt(1, "Enemy_Keese", 1.4, 0), pt(2, "Enemy_Keese", 0.5, 0), pt(3, "Enemy_Poe", 1.6, 0)]
        hit = L.hit_points(pts, "A", [0, 0, 0])
        self.assertEqual(list(hit), ["Enemy_Keese"])
        self.assertEqual(hit["Enemy_Keese"]["n"], 2)

    def test_add_remove(self):
        pts = [pt(1, "Enemy_Keese", 0, 0), pt(7, "Enemy_Poe", 3, 3)]
        p = L.make_point(pts, "A", "Enemy_Wolfos", [1.234, 2, 3], 45.55, "now")
        self.assertEqual(p["n"], 8)
        self.assertEqual(p["pos"], [1.23, 2.0, 3.0])
        pts.append(p)
        self.assertEqual([q["n"] for q in L.remove_point(pts, 7)], [1, 8])

    def test_yaw_faces_link(self):
        self.assertAlmostEqual(L.yaw_for(0), math.pi)
        self.assertAlmostEqual(L.yaw_for(-90), math.radians(90))

    def test_alive_and_new_enemy(self):
        p = pt(1, "Enemy_Keese", 0, 0)
        es = [{"id": 5, "name": "Enemy_Keese", "x": 0.5, "y": 0, "z": 0, "hp": 2}, {"id": 6, "name": "Enemy_Keese", "x": 9, "y": 0, "z": 0, "hp": 0}]
        self.assertEqual(L.alive_for(p, es, None)["id"], 5)
        self.assertEqual(L.alive_for(p, es, 6)["id"], 5)
        self.assertIsNone(L.alive_for(p, [es[1]], 6))
        self.assertEqual(L.new_enemy([es[1]], es + [{"id": 9, "name": "Enemy_Keese", "x": 0, "y": 0, "z": 0.1, "hp": 1}], "Enemy_Keese", [0, 0, 0])["id"], 9)
        self.assertIsNone(L.new_enemy(es, es, "Enemy_Keese", [0, 0, 0]))

    def test_to_kill(self):
        es = [{"id": 1, "x": 3, "y": 0, "z": 0, "hp": 2}, {"id": 2, "x": 0, "y": 0, "z": 0, "hp": 2}, {"id": 3, "x": 50, "y": 0, "z": 0, "hp": 2}, {"id": 4, "x": 1, "y": 0, "z": 0, "hp": 0}]
        self.assertEqual([e["id"] for e in L.to_kill(es, [0, 0, 1])], [1])

    def test_catalog_names_exist(self):
        path = os.environ.get("DEVTOOLS_MONSTERS_CSV", "")
        if not path or not os.path.exists(path):
            self.skipTest("no sheet")
        import csv
        ids = {r["ID"] for r in csv.DictReader(open(path, encoding="utf-8-sig"))}
        missing = [n for n, _ in L.CATALOG if n not in ids]
        self.assertEqual(missing, [])
        self.assertEqual(len({n for n, _ in L.CATALOG}), len(L.CATALOG))


class Multiplier(unittest.TestCase):
    ENTRY = {"enemies": [{"n": "Enemy_Keese", "p": [0, 0, 0], "r": 1}, {"n": "Enemy_Keese_Fire", "p": [0, 0, 1], "r": 1},
                         {"n": "Enemy_Skulltula_Gold", "p": [0, 0, 2], "r": 1}, {"n": "Enemy_DungeonBoss_Gohma", "p": [0, 0, 3], "r": 1},
                         {"n": "Enemy_Wolfos_Dictionary", "p": [0, 0, 3], "r": 1}, {"n": "Enemy_Stalfos_Weak", "p": [90, 0, 0], "r": 2}],
             "refs": [[[0, 0, 0], 1], [[1, 0, 0], 1], [[2, 0, 0], 1], [[90, 0, 0], 2], [[91, 0, 0], 2]]}

    def test_real_enemy(self):
        self.assertTrue(L.real_enemy("Enemy_Keese"))
        for bad in ("Enemy_Skulltula_Gold", "Enemy_DungeonBoss_Gohma", "Enemy_Scrub_Business", "Enemy_Scrub_Business_Leaf", "Enemy_Keese_Dictionary", "Bg_Door"):
            self.assertFalse(L.real_enemy(bad))

    def test_kind_of(self):
        self.assertEqual(L.kind_of("Enemy_Keese_Fire"), "Enemy_Keese_Fire")
        self.assertEqual(L.kind_of("Enemy_Peahat_Larva_Independent"), "Enemy_Peahat_Larva")
        self.assertEqual(L.kind_of("Enemy_Stalfos_Weak"), "Enemy_Stalfos_Weak")
        self.assertEqual(L.kind_of("Enemy_Stalfos_Weak_Other"), "Enemy_Stalfos_Weak")
        self.assertEqual(L.kind_of("Enemy_Stalfos_Other"), "Enemy_Stalfos")
        self.assertIsNone(L.kind_of("Enemy_Nothing"))

    def test_multiplier_text(self):
        self.assertEqual(L.mult_text(4, 0), "1x")
        self.assertEqual(L.mult_text(5, 7), "2.4x")
        self.assertEqual(L.mult_text(2, 18), "10x+")
        self.assertEqual(L.mult_text(2, 100), "10x+")
        self.assertEqual(L.mult_text(0, 3), "+3")
        self.assertIsNone(L.multiplier(0, 3))
        self.assertEqual(L.multiplier(2, 2), 2.0)

    def test_room_at(self):
        self.assertEqual(L.room_at(self.ENTRY["refs"], [1, 0, 0]), 1)
        self.assertEqual(L.room_at(self.ENTRY["refs"], [90, 0, 5]), 2)
        self.assertIsNone(L.room_at(self.ENTRY["refs"], [500, 0, 0]))
        self.assertIsNone(L.room_at([], [0, 0, 0]))

    def test_room_stats_counts_only_this_room(self):
        pts = [pt(1, "Enemy_Keese", 1, 0), pt(2, "Enemy_Keese", 2, 0), pt(3, "Enemy_Keese", 90, 0), pt(4, "Enemy_Keese", 1, 0, "B")]
        s = L.room_stats(self.ENTRY, pts, "A", [1, 0, 0])
        self.assertEqual((s["room"], s["vanilla"], s["added"], s["mult"]), (1, 2, 2, "2x"))
        self.assertEqual(s["kinds"], {"Enemy_Keese": 1, "Enemy_Keese_Fire": 1})
        s2 = L.room_stats(self.ENTRY, pts, "A", [90, 0, 0])
        self.assertEqual((s2["room"], s2["vanilla"], s2["added"], s2["mult"]), (2, 1, 1, "2x"))

    def test_no_room_data_uses_whole_map(self):
        entry = {"enemies": [{"n": "Enemy_Keese", "p": [0, 0, 0], "r": None}] * 4, "refs": []}
        s = L.room_stats(entry, [pt(1, "Enemy_Keese", 0, 0)], "A", [0, 0, 0])
        self.assertEqual((s["room"], s["vanilla"], s["added"], s["mult"]), (None, 4, 1, "1.2x"))

    def test_header(self):
        self.assertEqual(L.header_text("Dodongo Cavern", 6, 11, 0), "Dodongo Cavern: room 6 11/110")
        self.assertEqual(L.header_text("Dodongo Cavern", 6, 11, 4), "Dodongo Cavern: room 6 15/110")
        self.assertEqual(L.header_text("Hyrule Field", None, 49, 2), "Hyrule Field 51/490")

    def test_map_key_and_names(self):
        v = {"HyruleField_Child": {}, "DodongoCavern": {}}
        self.assertEqual(L.map_key(v, "DodongoCavern"), "DodongoCavern")
        self.assertEqual(L.map_key(v, "HyruleField", "child"), "HyruleField_Child")
        self.assertIsNone(L.map_key(v, "Nowhere"))
        self.assertEqual(L.place_name({"HyruleField": "Hyrule Field"}, "HyruleField_Adult"), "Hyrule Field")
        self.assertEqual(L.place_name({}, "Zed"), "Zed")

    def test_real_data_dodongo(self):
        v = L.load_vanilla()
        if "DodongoCavern" not in v:
            self.skipTest("no vanilla_enemies.json")
        n = len(L.vanilla_in(v["DodongoCavern"], None))
        self.assertGreater(n, 30)
        self.assertEqual(L.place_name(L.load_places(), "DodongoCavern"), "Dodongo Cavern" if L.load_places() else "DodongoCavern")


class FakeGame:
    def __init__(self):
        self.st = {"map": "A", "pos": [1.0, 0.0, 0.0], "facing": 90.0}
        self.live = []
        self.nid = 100
        self.bad = {"Enemy_Poe"}
        self.captions = []
        self.killed = []

    def status(self):
        return self.st

    def enemies(self, scan=False):
        return self.st["pos"], [dict(e) for e in self.live]

    def spawn(self, name, pos, yaw, quick=False):
        if name in self.bad:
            return False, "no"
        self.nid += 1
        self.live.append({"id": self.nid, "name": name, "x": pos[0], "y": pos[1], "z": pos[2], "hp": 2})
        return True, ""

    def damage(self, i):
        self.killed.append(i)
        self.live = [e for e in self.live if e["id"] != i]

    def caption(self, *a):
        self.captions.append(a)

    hit = {"hit": True, "pos": [5.0, 0.0, 5.0], "normal": [0.0, 1.0, 0.0], "surface": "floor", "yaw_out": None, "dist": 4.0}
    events = []
    marker = None

    def aim_cast(self):
        return dict(self.hit)

    def aim_marker(self, on):
        self.marker = on
        return True

    def aim_events(self, since):
        return {"events": [dict(e) for e in self.events if e["n"] > since], "next": max([e["n"] for e in self.events] + [0]) + 1}


class PlaceholderSpots(unittest.TestCase):
    def test_only_enemies_with_a_display_model(self):
        pts = [pt(1, "Enemy_Moblin", 1.0, 2.0), pt(2, "Enemy_Lizalfos", 3.0, 4.0), pt(3, "Obj_Pot", 5.0, 6.0), pt(4, "Enemy_Keese", 7.0, 8.0, m="B")]
        lines = L.placeholder_spots(pts, "A").split(chr(10))
        self.assertEqual(lines, ["Enemy_Moblin 1.00 0.00 2.00 180.0"])

    def test_the_yaw_is_the_markers(self):
        p = pt(1, "Enemy_Moblin", 1.0, 2.0)
        p["yaw"] = 90.0
        self.assertTrue(L.placeholder_spots([p], "A").endswith(" 90.0"))
        self.assertEqual(L.placeholder_spots([], "A"), "")

    def test_request(self):
        self.assertEqual(L.placeholder_request("A", "x", 1), {"cmd": "placeholders", "op": "set", "map": "A", "spots": "x", "on": True})


class RadarBoxes(unittest.TestCase):
    BOXES = {"Enemy_Stalfos": [[-1.14, -0.01, -0.66], [0.6, 2.63, 1.36]], "Enemy_Keese": [[-0.2, -0.02, -0.28], [0.2, 0.22, 0.28]]}

    def test_box_goes_between_the_yaw_and_the_label(self):
        pts = [pt(1, "Enemy_Stalfos", 10.0, -2.0), pt(2, "Enemy_Poe", 0.0, 0.0)]
        lines = L.radar_marks(None, pts, "A", boxes=self.BOXES).split("\n")
        self.assertEqual(lines[0], "P 10.00 0.00 -2.00 180.0 [-1.14 -0.01 -0.66 0.60 2.63 1.36] Stalfos")
        self.assertEqual(lines[1], "P 0.00 0.00 0.00 180.0 Poe")

    def test_boxes_need_the_facing_so_they_turn_with_it(self):
        pts = [pt(1, "Enemy_Keese", 1.0, 2.0)]
        self.assertTrue(L.radar_marks(None, pts, "A", arrows=False, boxes=self.BOXES).startswith("P "))
        self.assertEqual(L.radar_marks(None, pts, "A", boxes=None), "p 1.00 0.00 2.00 Keese")
        self.assertEqual(L.radar_marks(None, pts, "A", boxes={}), "p 1.00 0.00 2.00 Keese")

    def test_vanilla_marks_carry_no_box(self):
        entry = {"enemies": [{"n": "Enemy_Stalfos", "p": [1, 2, 3], "r": 0}]}
        self.assertEqual(L.radar_marks(entry, [], "A", boxes=self.BOXES), "v 1.00 2.00 3.00 Stalfos")

    def test_the_shipped_file_covers_every_placer_kind(self):
        boxes = L.load_boxes()
        self.assertEqual([n for n, _ in L.CATALOG if n not in boxes], [])
        for n, b in boxes.items():
            self.assertTrue(all(b[0][i] <= b[1][i] for i in range(3)), n)
        st = boxes["Enemy_Stalfos"]
        self.assertAlmostEqual(st[0][0], -1.14, 2)
        self.assertAlmostEqual(st[1][1], 2.63, 2)
        self.assertEqual(L.box_text({"X": [[0, 0]]}, "X"), "")
        self.assertEqual(L.box_text(boxes, "Nothing"), "")

    def test_the_line_fits_the_hosts_parser_buffer(self):
        pts = [pt(1, "Enemy_Stalfos", -1234.56, 9876.54)]
        for line in L.radar_marks(None, pts, "A", boxes=self.BOXES).split("\n"):
            self.assertLess(len(line), 200)


class PlacerFlow(unittest.TestCase):
    def setUp(self):
        import tempfile
        import placer_server as S
        self.dir = tempfile.mkdtemp()
        self.g = FakeGame()
        S.time.sleep = lambda s: None
        self.p = S.Placer(self.dir, self.g)
        self.p.sync_inline = True
        self.p.status = self.g.st

    def test_add_highlight_remove(self):
        r = self.p.press("Enemy_Keese")
        self.assertTrue(r["ok"])
        self.assertEqual(len(self.p.points), 1)
        self.assertEqual(self.g.live, [])
        v = {b["name"]: b for b in self.p.view()["buttons"]}
        self.assertFalse(v["Enemy_Keese"]["hit"])
        self.assertEqual(v["Enemy_Keese"]["count"], 1)
        self.assertFalse(v["Enemy_Wolfos"]["hit"])
        self.p.spawn_all()
        self.assertEqual(len(self.g.live), 1)
        self.p.press("Enemy_Keese")
        self.assertEqual(len(self.p.points), 2)
        self.p.remove_nearest()
        self.assertEqual(len(self.p.points), 1)

    def test_boxes_toggle_and_reach_the_radar_lines(self):
        self.p.press("Enemy_Stalfos")
        self.assertFalse(self.p.view()["boxes"])
        self.assertTrue(self.p.set_boxes(True)["boxes"])
        self.p.set_overlay(True)
        self.assertTrue(self.p.view()["boxes"])
        self.assertIn("[", self.p_marks())
        r = self.p.set_boxes(False)
        self.assertFalse(r["boxes"])
        self.assertFalse(self.p.view()["boxes"])
        self.assertNotIn("[", self.p_marks())
        self.assertTrue(self.p.set_boxes(None)["boxes"])

    def sent_requests(self):
        sent = []
        self.g.ctl = lambda *a, **k: sent.append(json.loads(a[1])) or '{"radar": {}, "placeholders": {}}'
        self.p.radar_sent = ("", "", 0.0)
        self.p.push_radar()
        return sent

    def test_every_overlay_layer_is_off_by_default(self):
        self.p.press("Enemy_Stalfos")
        v = self.p.view()
        self.assertEqual((v["overlay"], v["boxes"], v["models"]), (False, False, False))
        self.assertFalse(W.any_on(self.p))
        sent = self.sent_requests()
        self.assertEqual([r["cmd"] for r in sent], ["radar", "placeholders"])
        self.assertFalse(sent[1]["on"])

    def test_an_old_overlay_file_from_the_defaults_on_era_is_not_trusted(self):
        import placer_server as S
        with open(os.path.join(self.dir, "notes", "panel_overlay.json"), "w") as f:
            json.dump({"gates": False, "puzzles": True, "gs": True, "boxes": False, "models": False, "placer": True, "gate_models": False}, f)
        p2 = S.Placer(self.dir, self.g)
        self.assertFalse(W.any_on(p2))
        p2.set_overlay(True)
        p3 = S.Placer(self.dir, self.g)
        self.assertTrue(p3.view()["overlay"])
        self.assertFalse(p3.view()["boxes"])

    def test_restart_keeps_the_last_choice_not_a_default(self):
        import placer_server as S
        self.p.set_boxes(True)
        self.p.set_overlay(False)
        p2 = S.Placer(self.dir, self.g)
        self.assertEqual((p2.view()["boxes"], p2.view()["overlay"]), (True, False))

    def test_game_overlay_follows_the_layers_and_comes_back_after_a_game_restart(self):
        self.g.debug_state = lambda: self.g.dbg
        sent = []
        self.g.dbg = True
        self.g.ctl = lambda *a, **k: sent.append(json.loads(a[1])) or "{}"
        self.p.reconcile()
        self.assertEqual([r.get("state") for r in sent if r.get("cmd") == "debug"], ["off"])
        self.p.set_overlay(True)
        sent.clear()
        self.g.dbg = False
        self.p.reconcile()
        self.assertEqual([r.get("state") for r in sent if r.get("cmd") == "debug"], ["on"])
        self.p.set_overlay(False)
        self.assertEqual([r.get("state") for r in sent if r.get("cmd") == "debug"][-1], "off")

    def test_aim_mode_is_read_from_the_game(self):
        self.g.aim_marker_state = lambda: True
        self.assertFalse(self.p.view()["aim_on"])
        self.p.reconcile()
        self.assertTrue(self.p.view()["aim_on"])
        self.g.aim_marker_state = lambda: False
        self.p.reconcile()
        self.assertFalse(self.p.view()["aim_on"])

    def test_display_models_toggle_sends_the_spots_and_follows_a_moved_marker(self):
        self.p.press("Enemy_Stalfos")
        self.p.press("Enemy_Lizalfos")
        self.assertTrue(self.p.set_models(True)["models"])
        self.assertTrue(self.p.view()["models"])
        sent = [r for r in self.sent_requests() if r["cmd"] == "placeholders"]
        self.assertEqual(len(sent), 1)
        self.assertEqual((sent[0]["op"], sent[0]["map"], sent[0]["on"]), ("set", "A", True))
        lines = sent[0]["spots"].split(chr(10))
        self.assertEqual(len(lines), 1)
        self.assertTrue(lines[0].startswith("Enemy_Stalfos "))
        self.assertEqual(len(lines[0].split()), 5)
        self.assertEqual([r["cmd"] for r in self.sent_requests()], ["radar"])
        got = []
        self.g.ctl = lambda *a, **k: got.append(json.loads(a[1])) or '{"radar": {}, "placeholders": {}}'
        self.p.remove_nearest()
        self.p.press("Enemy_Keese")
        self.p.push_radar()
        ph = [r for r in got if r["cmd"] == "placeholders"]
        self.assertTrue(ph)
        self.assertNotEqual(ph[-1]["spots"], sent[0]["spots"])
        self.assertIn("Enemy_Keese ", ph[-1]["spots"])
        self.p.set_models(False)
        self.p.push_radar()
        ph = [r for r in got if r["cmd"] == "placeholders"]
        self.assertFalse(ph[-1]["on"])

    def test_display_models_choice_is_remembered(self):
        import placer_server as S
        self.p.set_models(True)
        again = S.Placer(self.dir, self.g)
        self.assertTrue(again.view()["models"])

    def test_the_client_path_follows_longshot_ctl(self):
        import importlib
        import placer_server as S
        os.environ["LONGSHOT_CTL"] = "X:/other/longshot_ctl.py"
        try:
            S = importlib.reload(S)
            self.assertEqual(S.CTL[-1], "X:/other/longshot_ctl.py")
        finally:
            del os.environ["LONGSHOT_CTL"]
            importlib.reload(S)

    def p_marks(self):
        import placer_server as S
        sent = []
        self.g.ctl = lambda *a, **k: sent.append(a[1]) or '{"radar": {}}'
        self.p.radar_sent = ("", "", 0.0)
        self.p.push_radar()
        radar = [x for x in sent if '"cmd": "radar"' in x]
        return radar[0] if radar else ""

    def test_marker_only_needs_no_game_spawn(self):
        r = self.p.press("Enemy_Poe")
        self.assertTrue(r["ok"])
        self.assertEqual(len(self.p.points), 1)
        self.assertEqual(self.g.live, [])

    def test_numbers_never_reused(self):
        self.p.press("Enemy_Poe")
        self.p.press("Enemy_Keese")
        self.assertEqual([p["n"] for p in self.p.points], [1, 2])
        self.p.remove_nearest()
        self.p.press("Enemy_Wolfos")
        self.assertEqual(sorted(p["n"] for p in self.p.points)[-1], 3)

    def test_spawn_all_and_kill_all(self):
        self.p.press("Enemy_Keese")
        self.g.st["pos"] = [10.0, 0.0, 0.0]
        self.p.press("Enemy_Wolfos")
        self.assertEqual(len(self.g.live), 0)
        self.p.spawn_all()
        self.assertEqual(len(self.g.live), 2)
        self.p.spawn_all()
        self.assertEqual(len(self.g.live), 2)
        self.g.live = []
        self.p.spawn_all()
        self.assertEqual(len(self.g.live), 2)
        self.p.kill_all()
        self.assertEqual(self.g.live, [])
        saved = json.load(open(os.path.join(self.dir, "notes", "spawnpoints.json")))
        self.assertEqual([x["name"] for x in saved], ["Enemy_Keese", "Enemy_Wolfos"])


WALL = {"hit": True, "pos": [10.0, 3.0, 0.0], "normal": [-1.0, 0.0, 0.0], "surface": "wall", "yaw_out": -math.pi / 2, "dist": 5.0}


class AimFlow(unittest.TestCase):
    def setUp(self):
        import tempfile
        import placer_server as S
        self.S = S
        self.dir = tempfile.mkdtemp()
        self.g = FakeGame()
        self.g.events = []
        S.time.sleep = lambda s: None
        self.p = S.Placer(self.dir, self.g)
        self.p.status = self.g.st

    def place_event(self, n, hit):
        e = dict(hit, kind="place", via="pad", n=n, map="A")
        self.g.events.append(e)

    def test_selected_kind_and_view(self):
        self.assertIsNone(self.p.view()["selected"])
        self.p.press("Enemy_Keese")
        v = self.p.view()
        self.assertEqual((v["selected"]["name"], v["selected"]["text"], v["selected"]["chosen"]), ("Enemy_Keese", "Keese", True))
        self.assertFalse(v["aim_on"])
        self.assertEqual(len(self.p.points), 1)

    def test_selected_survives_restart(self):
        import placer_server as S
        self.p.press("Enemy_Wolfos")
        p2 = S.Placer(self.dir, self.g)
        self.assertEqual(p2.view()["selected"]["name"], "Enemy_Wolfos")

    def test_no_selection_refuses(self):
        r = self.p.place_at_aim()
        self.assertFalse(r["ok"])
        self.assertEqual(self.p.points, [])

    def test_phone_place_floor_faces_link_and_lifts_fliers(self):
        self.p.press("Enemy_Keese")
        r = self.p.place_at_aim()
        self.assertTrue(r["ok"])
        q = self.p.points[-1]
        self.assertEqual(q["pos"], [5.0, 2.0, 5.0])
        self.assertAlmostEqual(q["yaw"], math.degrees(math.atan2(1.0 - 5.0, 0.0 - 5.0)), places=1)
        self.assertEqual((q["surface"], q["wall"], q["normal"]), ("floor", False, [0.0, 1.0, 0.0]))

    def test_phone_place_wall_faces_out_without_lift(self):
        self.p.press("Enemy_Keese")
        self.g.hit = dict(WALL)
        self.p.place_at_aim()
        q = self.p.points[-1]
        self.assertEqual(q["pos"], [10.0, 3.0, 0.0])
        self.assertAlmostEqual(q["yaw"], -90.0)
        self.assertEqual((q["surface"], q["wall"], q["normal"]), ("wall", True, [-1.0, 0.0, 0.0]))
        self.assertAlmostEqual(L.point_yaw(q), -math.pi / 2)

    def test_flier_on_a_wall_keeps_the_marker_coordinate_even_without_a_yaw(self):
        self.p.selected = "Enemy_Keese"
        self.g.hit = dict(WALL, yaw_out=None)
        self.p.place_at_aim()
        q = self.p.points[-1]
        self.assertEqual(q["pos"], [10.0, 3.0, 0.0])
        self.assertAlmostEqual(q["yaw"], -90.0)
        self.p.spawn_all()
        self.assertEqual([e["y"] for e in self.g.live], [3.0])

    def test_sort_data_size_and_recently_used(self):
        self.p.press("Enemy_Wolfos")
        self.p.press("Enemy_Keese")
        b = {x["name"]: x for x in self.p.view()["buttons"]}
        self.assertGreater(b["Enemy_Keese"]["used"], b["Enemy_Wolfos"]["used"])
        self.assertEqual(b["Enemy_Stalfos"]["used"], 0)
        self.assertGreater(b["Enemy_Dodongo"]["size"], b["Enemy_Keese"]["size"])
        import placer_server as S
        p2 = S.Placer(self.dir, self.g)
        self.assertGreater({x["name"]: x for x in p2.view()["buttons"]}["Enemy_Keese"]["used"], 0)

    def test_aim_goes_to_the_card_that_turned_it_on(self):
        self.assertTrue(self.p.set_aim(True, "gs")["ok"])
        self.assertEqual((self.p.view()["aim_on"], self.p.view()["aim_for"]), (True, "gs"))
        self.assertTrue(self.p.set_aim(True)["ok"])
        self.assertEqual(self.p.view()["aim_for"], "enemy")
        self.p.set_aim(None, "enemy")
        self.assertFalse(self.p.view()["aim_on"])

    def test_phone_place_faces_link_wherever_he_stands(self):
        self.p.press("Enemy_Dodongo")
        self.g.st["pos"] = [5.0, 0.0, 15.0]
        self.g.st["facing"] = 270.0
        self.p.place_at_aim()
        self.assertAlmostEqual(self.p.points[-1]["yaw"], 0.0, places=1)
        self.g.st["pos"] = [-5.0, 0.0, 5.0]
        self.p.place_at_aim()
        self.assertAlmostEqual(self.p.points[-1]["yaw"], -90.0, places=1)

    def test_wall_keeps_facing_out_even_with_link_elsewhere(self):
        self.p.press("Enemy_Dodongo")
        self.g.hit = dict(WALL)
        self.g.st["pos"] = [10.0, 3.0, 20.0]
        self.p.place_at_aim()
        self.assertAlmostEqual(self.p.points[-1]["yaw"], -90.0)

    def test_ceiling_and_slope_face_link(self):
        self.p.press("Enemy_Dodongo")
        for surface, normal in (("ceiling", [0.0, -1.0, 0.0]), ("slope", [0.5, 0.8, 0.0])):
            self.g.hit = {"hit": True, "pos": [5.0, 6.0, 5.0], "normal": normal, "surface": surface, "yaw_out": None, "dist": 3.0}
            self.p.place_at_aim()
            q = self.p.points[-1]
            self.assertEqual(q["surface"], surface)
            self.assertAlmostEqual(q["yaw"], math.degrees(math.atan2(-4.0, -5.0)), places=1)
        self.assertEqual(self.p.points[-2]["pos"][1], 6.0)

    def test_event_link_and_yaw_face_used(self):
        self.p.selected = "Enemy_Dodongo"
        self.p.set_aim(True)
        e = dict(self.g.hit, kind="place", via="pad", n=1, map="A", link=[5.0, 0.0, 12.0], yaw_face=0.0, face="link")
        self.g.events.append(e)
        self.g.st["pos"] = [50.0, 0.0, 50.0]
        self.p.aim_poll()
        self.assertAlmostEqual(self.p.points[0]["yaw"], 0.0, places=1)
        e2 = dict(self.g.hit, kind="place", via="pad", n=2, map="A", yaw_face=math.pi / 2)
        e2["pos"] = [5.0, 0.0, 5.0]
        self.g.events.append(e2)
        self.g.st["pos"] = [5.0, 0.0, 5.0]
        self.p.aim_poll()
        self.assertAlmostEqual(self.p.points[1]["yaw"], 90.0, places=1)

    def test_no_link_falls_back_to_opposite_of_link_facing(self):
        self.assertAlmostEqual(L.aim_spot({"pos": [1, 0, 1], "surface": "floor"}, "Enemy_Dodongo", 90.0)["yaw"], 270.0)

    def test_phone_place_no_hit(self):
        self.p.press("Enemy_Keese")
        self.g.hit = {"hit": False}
        self.assertFalse(self.p.place_at_aim()["ok"])
        self.assertEqual(len(self.p.points), 1)

    def test_plain_marker_has_surface_fields(self):
        self.p.press("Enemy_Dodongo")
        q = self.p.points[0]
        self.assertEqual((q["surface"], q["wall"], q["normal"]), ("floor", False, [0.0, 1.0, 0.0]))
        self.assertTrue(L.radar_marks(None, self.p.points, "A"))
        self.assertTrue(L.placeholder_spots(self.p.points, "A"))

    def test_aim_mode_taps_only_select(self):
        self.assertTrue(self.p.set_aim(True)["ok"])
        self.assertTrue(self.g.marker)
        r = self.p.press("Enemy_Keese")
        self.assertTrue(r["ok"])
        self.assertEqual(self.p.points, [])
        self.assertTrue(self.p.view()["aim_on"])
        self.assertFalse(self.p.set_aim(None)["aim_on"])
        self.assertFalse(self.g.marker)

    def test_aim_mode_skips_old_events_then_applies_new(self):
        self.place_event(1, WALL)
        self.p.selected = "Enemy_Dodongo"
        self.p.set_aim(True)
        self.assertEqual(self.p.aim_poll(), 0)
        self.assertEqual(self.p.points, [])
        self.place_event(2, WALL)
        self.assertEqual(self.p.aim_poll(), 1)
        self.assertEqual(self.p.aim_poll(), 0)
        self.assertEqual(len(self.p.points), 1)
        q = self.p.points[0]
        self.assertEqual((q["name"], q["wall"], q["via"]), ("Enemy_Dodongo", True, "aim"))

    def test_events_place_nudge_undo(self):
        self.p.set_aim(True)
        self.p.press("Enemy_ReDead")
        self.place_event(1, self.g.hit)
        self.g.events.append({"kind": "nudge", "via": "pad", "n": 2, "d": [0.25, 0.0, -0.5]})
        self.p.aim_poll()
        q = self.p.points[0]
        self.assertEqual(q["name"], "Enemy_ReDead")
        self.assertEqual(q["pos"], [5.25, 0.0, 4.5])
        self.place_event(3, WALL)
        self.g.events.append({"kind": "nudge", "via": "pad", "n": 4, "d": [0.0, 0.25, 0.0]})
        self.p.aim_poll()
        self.assertEqual(self.p.points[1]["pos"], [10.0, 3.25, 0.0])
        self.assertEqual(self.p.points[0]["pos"], [5.25, 0.0, 4.5])
        self.g.events.append({"kind": "undo", "via": "pad", "n": 5})
        self.p.aim_poll()
        self.assertEqual(len(self.p.points), 1)
        self.g.events.append({"kind": "undo", "via": "pad", "n": 6})
        self.p.aim_poll()
        self.assertEqual(self.p.points, [])
        self.g.events.append({"kind": "undo", "via": "pad", "n": 7})
        self.g.events.append({"kind": "nudge", "via": "pad", "n": 8, "d": [1, 0, 0]})
        self.p.aim_poll()
        self.assertEqual(self.p.points, [])

    def test_remove_aimed_at_phone_removes_nearest_and_kills(self):
        self.p.selected = "Enemy_Dodongo"
        self.p.place_at_aim()
        self.g.hit = dict(self.g.hit, pos=[9.0, 0.0, 5.0])
        self.p.place_at_aim()
        self.p.spawn_all()
        self.assertEqual(len(self.g.live), 2)
        self.g.hit = dict(self.g.hit, pos=[5.6, 0.0, 5.3])
        r = self.p.remove_at_aim()
        self.assertTrue(r["ok"])
        self.assertEqual([q["pos"][0] for q in self.p.points], [9.0])
        self.assertEqual(len(self.g.killed), 1)
        self.assertEqual(len(self.g.live), 1)

    def test_remove_aimed_at_nothing_near(self):
        self.p.selected = "Enemy_Dodongo"
        self.p.place_at_aim()
        self.g.hit = dict(self.g.hit, pos=[8.0, 0.0, 5.0])
        r = self.p.remove_at_aim()
        self.assertFalse(r["ok"])
        self.assertIn("no marker within 1.5 m", r["message"])
        self.assertEqual(len(self.p.points), 1)
        self.g.hit = {"hit": False}
        self.assertFalse(self.p.remove_at_aim()["ok"])
        self.assertEqual(len(self.p.points), 1)

    def test_remove_ignores_other_maps_and_picks_nearest(self):
        self.p.selected = "Enemy_Dodongo"
        self.p.place_at_aim()
        self.p.points.append(dict(self.p.points[0], n=50, pos=[5.4, 0.0, 5.0], map="B"))
        self.p.points.append(dict(self.p.points[0], n=51, pos=[5.9, 0.0, 5.0]))
        self.g.hit = dict(self.g.hit, pos=[5.7, 0.0, 5.0])
        self.p.remove_at_aim()
        self.assertEqual(sorted(q["n"] for q in self.p.points), [1, 50])

    def test_remove_event_from_the_pad(self):
        self.p.selected = "Enemy_Dodongo"
        self.p.set_aim(True)
        self.place_event(1, self.g.hit)
        self.p.aim_poll()
        self.p.spawn_all()
        self.g.events.append(dict(self.g.hit, kind="remove", via="pad", n=2, map="A", pos=[5.5, 0.0, 5.5]))
        self.assertEqual(self.p.aim_poll(), 1)
        self.assertEqual(self.p.points, [])
        self.assertEqual(self.g.live, [])
        self.g.events.append(dict(self.g.hit, kind="remove", via="pad", n=3, map="A"))
        self.assertEqual(self.p.aim_poll(), 1)
        self.assertEqual(self.p.points, [])

    def test_undo_kills_the_spawned_enemy(self):
        self.p.selected = "Enemy_Dodongo"
        self.p.set_aim(True)
        self.place_event(1, self.g.hit)
        self.p.aim_poll()
        self.p.spawn_all()
        self.assertEqual(len(self.g.live), 1)
        self.g.events.append({"kind": "undo", "via": "pad", "n": 2})
        self.p.aim_poll()
        self.assertEqual(self.p.points, [])
        self.assertEqual(self.g.live, [])
        self.assertEqual(len(self.g.killed), 1)

    def test_event_counter_reset_after_game_restart(self):
        self.p.selected = "Enemy_Dodongo"
        self.p.set_aim(True)
        self.p.aim_since = 40
        self.place_event(1, self.g.hit)
        self.assertEqual(self.p.aim_poll(), 0)
        self.assertEqual(self.p.aim_poll(), 1)

    def test_selected_falls_back_to_last_placed(self):
        self.p.press("Enemy_Poe")
        self.p.selected = None
        self.assertEqual(self.p.view()["selected"]["name"], "Enemy_Poe")
        self.assertFalse(self.p.view()["selected"]["chosen"])
        self.assertTrue(self.p.place_at_aim()["ok"])


class UiView(unittest.TestCase):
    def setUp(self):
        import tempfile
        import placer_server as S
        self.S = S
        self.dir = tempfile.mkdtemp()
        self.g = FakeGame()
        S.time.sleep = lambda s: None
        self.p = S.Placer(self.dir, self.g)
        self.p.status = self.g.st

    def test_buttons_carry_what_the_tiles_show(self):
        v = self.p.view()
        self.assertNotIn("suggested", v)
        b = {x["name"]: x for x in v["buttons"]}
        self.assertEqual((b["Enemy_Moblin"]["text"], b["Enemy_Moblin"]["wip"]), ("Moblin", True))
        self.assertEqual((b["Enemy_Keese"]["text"], b["Enemy_Keese"]["wip"], b["Enemy_Keese"]["full"]), ("Keese", False, "Keese"))
        self.assertEqual(b["Enemy_Keese_Fire"]["full"], "Keese Fire")
        self.assertEqual(b["Enemy_Keese"]["hab"], "fly")
        self.assertIn("hp", b["Enemy_Keese"])
        self.assertEqual(v["kill_radius"], int(L.NEAR_RADIUS))
        self.assertEqual(set(v["habitats"]), set(L.HABITAT_KINDS))
        self.assertIsNone(v["nearest"])

    def test_nearest_says_spawned_and_remove_kills_it(self):
        self.p.press("Enemy_ReDead")
        n = self.p.view()["nearest"]
        self.assertEqual((n["label"], n["alive"], n["rise"]), ("ReDead", False, 0.0))
        self.p.spawn_all()
        self.p.live_list = self.g.enemies()
        self.assertTrue(self.p.view()["nearest"]["alive"])
        r = self.p.remove_nearest()
        self.assertIn("killed", r["message"])
        self.assertEqual((self.g.live, self.p.points), ([], []))

    def test_kill_all_kills_redeads_like_backspace(self):
        self.g.live = [{"id": "7", "name": "Enemy_ReDead", "x": 3.0, "y": 0.0, "z": 0.0, "hp": 12},
                       {"id": "8", "name": "Enemy_Gibdo", "x": 2.0, "y": 0.0, "z": 0.0, "hp": 12},
                       {"id": "9", "name": "Enemy_ReDead", "x": 90.0, "y": 0.0, "z": 0.0, "hp": 12}]
        r = self.p.kill_all()
        self.assertTrue(r["ok"])
        self.assertEqual(self.g.killed, ["7", "8"])
        self.assertEqual([e["id"] for e in self.g.live], ["9"])
        self.g.live = []
        self.assertFalse(self.p.kill_all()["ok"])

    def test_radar_marks_the_nearest(self):
        pts = [pt(1, "Enemy_Keese_Fire", 10.0, -2.0), pt(2, "Enemy_ReDead", 0.0, 0.0)]
        lines = L.radar_marks(None, pts, "A", near=2).split("\n")
        self.assertEqual(lines, ["p 10.00 0.00 -2.00 FireKee", "p 0.00 0.00 0.00 >ReDead"])
        self.assertEqual(L.radar_marks(None, pts, "A").split("\n")[1], "p 0.00 0.00 0.00 ReDead")

    def test_pages_and_api_over_http(self):
        import threading
        import urllib.error
        import urllib.request
        srv = self.S.ThreadingHTTPServer(("127.0.0.1", 0), self.S.handler(self.p, "k"))
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        base = f"http://127.0.0.1:{srv.server_address[1]}"

        def fetch(path, data=None, headers=None):
            req = urllib.request.Request(base + path, data=data, headers=headers or {}, method="POST" if data is not None else "GET")
            with urllib.request.urlopen(req, timeout=10) as r:
                return r.status, r.headers.get("Content-Type"), r.read()
        try:
            for page, title in (("/placer", b"Enemy Placer"), ("/gate", b"Enemy-Gate Manager"), ("/puzzle", b"Timed Rupee Puzzle Manager")):
                code, kind, body = fetch(page + "?key=k")
                self.assertEqual((code, kind.split(";")[0]), (200, "text/html"))
                self.assertIn(title, body)
                self.assertIn(b'href="/ui.css"', body)
                self.assertIn(b'src="/ui.js"', body)
                self.assertNotIn(b"https://", body)
            self.assertIn(b"--near", fetch("/ui.css")[2])
            self.assertIn(b"confirmTap", fetch("/ui.js")[2])
            with self.assertRaises(urllib.error.HTTPError):
                fetch("/placer")
            self.assertIn("buttons", json.loads(fetch("/api/state?key=k")[2]))
            self.assertIn("will", json.loads(fetch("/api/gate/state?key=k")[2]))
            self.assertIn("current", json.loads(fetch("/api/puzzle/state?key=k")[2]))
            hdr = {"X-Placer": "1", "X-Panel-Key": "k", "Content-Type": "application/json"}
            r = json.loads(fetch("/api/press", json.dumps({"name": "Enemy_Keese"}).encode(), hdr)[2])
            self.assertTrue(r["ok"])
            r = json.loads(fetch("/api/repeat", json.dumps({"map": "A", "pos": [5.0, 0.0, 0.0], "facing": 0.0}).encode(), hdr)[2])
            self.assertTrue(r["ok"])
            self.assertEqual([p["name"] for p in self.p.points], ["Enemy_Keese", "Enemy_Keese"])
            r = json.loads(fetch("/api/puzzle/gem", b"{}", hdr)[2])
            self.assertTrue(r["ok"])
            with self.assertRaises(urllib.error.HTTPError):
                fetch("/api/killall", b"{}", {"X-Panel-Key": "k"})
        finally:
            srv.shutdown()
            srv.server_close()


class FakeMode(unittest.TestCase):
    def test_fake_game_follows_its_status_file(self):
        import tempfile
        import fake_game
        d = tempfile.mkdtemp()
        g = fake_game.FakeGame(d)
        self.assertEqual(g.status()["map"], "DodongoCavern")
        os.makedirs(os.path.join(d, "notes"))
        with open(os.path.join(d, "notes", "fake_status.json"), "w") as f:
            json.dump({"map": "B", "pos": [1.0, 2.0, 3.0]}, f)
        self.assertEqual((g.status()["map"], g.enemies()[0]), ("B", [1.0, 2.0, 3.0]))
        ok, _ = g.spawn("Enemy_Keese", [0, 0, 0], 0.0)
        self.assertTrue(ok)
        g.damage(g.live[-1]["id"])
        self.assertNotIn("Enemy_Keese", [e["name"] for e in g.live])
        self.assertIn("radar", g.ctl("raw", "{}"))


class Radar(unittest.TestCase):
    def test_marks_payload(self):
        entry = {"enemies": [{"n": "Enemy_Octorok", "p": [1, 2, 3], "r": 0}, {"n": "Enemy_Skulltula_Gold", "p": [0, 0, 0], "r": 0},
                             {"n": "Enemy_Wolfos_White", "p": [-4.5, 0, 9], "r": 1}]}
        pts = [pt(1, "Enemy_Keese_Fire", 10.0, -2.0), pt(2, "Enemy_ReDead", 0.0, 0.0, m="B")]
        text = L.radar_marks(entry, pts, "A")
        self.assertEqual(text.split("\n"), ["p 10.00 0.00 -2.00 FireKee", "v 1.00 2.00 3.00 Octo", "v -4.50 0.00 9.00 WhtWolf"])
        arrows = L.radar_marks(entry, pts, "A", arrows=True).split("\n")
        self.assertEqual(arrows[0], "P 10.00 0.00 -2.00 180.0 FireKee")
        self.assertEqual(arrows[1], "v 1.00 2.00 3.00 Octo")
        self.assertEqual(L.radar_marks(None, [], "A"), "")
        many = [pt(i, "Enemy_Keese", float(i), 0.0) for i in range(300)]
        lines = L.radar_marks(entry, many, "A").split("\n")
        self.assertEqual(len(lines), L.RADAR_MAX)
        self.assertTrue(all(x.startswith("p ") for x in lines))
        req = L.radar_request("A", text)
        self.assertEqual((req["cmd"], req["op"], req["map"]), ("radar", "set", "A"))
        self.assertEqual(L.radar_label("Enemy_Something_New"), "Something New")


class MinimapFit(unittest.TestCase):
    def test_axis_and_affine_solve(self):
        import random
        import minimap_fit as MF
        rnd = random.Random(4)
        a, b, c, d = 0.0012, -0.0015, 0.9, 0.81
        pos = [[10.0, 0.0, -20.0], [60.0, 2.0, 15.0], [-30.0, 0.0, 40.0], [5.0, 1.0, 5.0]]
        samples = [{"pos": p, "u": a * p[0] + c, "v": b * p[2] + d} for p in pos]
        r = MF.solve(samples)
        for got, want in zip(r["m"], [a, 0.0, c, 0.0, b, d]):
            self.assertAlmostEqual(got, want, places=9)
        self.assertLess(r["rms_px"], 1e-6)
        rot = [0.001, 0.0004, 0.9, -0.0003, 0.0011, 0.8]
        noisy = [{"pos": p, "u": rot[0] * p[0] + rot[1] * p[2] + rot[2] + rnd.uniform(-1, 1) / 1920,
                  "v": rot[3] * p[0] + rot[4] * p[2] + rot[5] + rnd.uniform(-1, 1) / 1080} for p in pos]
        ra = MF.solve(noisy, affine=True)
        self.assertLess(ra["max_px"], 2.0)
        self.assertGreater(MF.solve(noisy)["rms_px"], ra["rms_px"])
        with self.assertRaises(ValueError):
            MF.solve(samples[:1])
        with self.assertRaises(ValueError):
            MF.solve([{"pos": [0, 0, 0], "u": 0, "v": 0}, {"pos": [1, 0, 1], "u": 1, "v": 1}, {"pos": [2, 0, 2], "u": 2, "v": 2}], affine=True)

    def test_fit_store_and_requests(self):
        import minimap_fit as MF
        fits = MF.put_fit({}, "M", {"m": [1, 0, 0.5, 0, -1, 0.7], "clip": [0.8, 0.7, 0.97, 0.95]})
        fits = MF.put_fit(fits, "M", {"m": [2, 0, 0.5, 0, -2, 0.7], "ylo": 10.0, "yhi": 20.0})
        fits = MF.put_fit(fits, "M", {"m": [3, 0, 0.5, 0, -3, 0.7], "ylo": 10.0, "yhi": 20.0})
        self.assertEqual(len(fits["M"]), 2)
        self.assertEqual(MF.fit_for(fits, "M", 15.0)["m"][0], 3)
        self.assertEqual(MF.fit_for(fits, "M", 0.0)["m"][0], 1)
        self.assertIsNone(MF.fit_for(fits, "N", 0.0))
        reqs = MF.fit_requests("M", fits)
        self.assertEqual([(q["cmd"], q["op"], q["map"]) for q in reqs], [("radar", "fit", "M")] * 2)
        self.assertEqual(reqs[0]["clip"], "0.80000 0.70000 0.97000 0.95000")
        self.assertEqual((reqs[1]["ylo"], reqs[1]["yhi"], reqs[1]["ux"], reqs[1]["vz"]), (10.0, 20.0, 3.0, -3.0))
        self.assertEqual(MF.fit_requests("N", fits), [])

    def test_detect_red_arrow_in_minimap(self):
        try:
            from PIL import Image, ImageDraw
        except ImportError:
            self.skipTest("no PIL")
        import minimap_fit as MF
        im = Image.new("RGB", (1280, 720), (90, 60, 30))
        dr = ImageDraw.Draw(im)
        dr.ellipse((100, 40, 130, 70), fill=(230, 40, 40))
        dr.polygon([(1150, 600), (1144, 612), (1156, 612)], fill=(240, 30, 30))
        dr.point((1100, 520), fill=(240, 30, 30))
        got = MF.red_mask_centroid(im)
        self.assertAlmostEqual(got["px"][0], 1150.5, delta=1.5)
        self.assertAlmostEqual(got["px"][1], 608.0, delta=2.0)
        self.assertAlmostEqual(got["u"], got["px"][0] / 1280)
        with self.assertRaises(ValueError):
            MF.red_mask_centroid(Image.new("RGB", (1280, 720), (90, 60, 30)))


class PushRadarFits(unittest.TestCase):
    def test_push_sends_saved_fit(self):
        import tempfile
        import placer_server as S
        d = tempfile.mkdtemp()
        g = FakeGame()
        sent = []
        g.ctl = lambda *a, **k: (sent.append(json.loads(a[1])), '{"ok":true,"radar":{}}')[1]
        S.time.sleep = lambda s: None
        p = S.Placer(d, g)
        p.status = {"map": "A", "pos": [0.0, 0.0, 0.0]}
        os.makedirs(os.path.join(d, "notes"), exist_ok=True)
        with open(os.path.join(d, "notes", "minimap_fits.json"), "w") as f:
            json.dump({"A": [{"m": [0.001, 0, 0.9, 0, 0.001, 0.8]}]}, f)
        sent.clear()
        p.radar_sent = ("", "", 0.0)
        p.push_radar()
        self.assertEqual([q["op"] for q in sent], ["set", "fit", "set"])
        self.assertEqual(sent[1]["u0"], 0.9)
        self.assertEqual((sent[2]["cmd"], sent[2]["on"]), ("placeholders", False))
        sent.clear()
        p.push_radar()
        self.assertEqual(sent, [])


class LabelTests(unittest.TestCase):
    def test_labels_are_clean(self) -> None:
        for n in ("Enemy_Dekubaba", "Enemy_Dekubaba_Withered", "Enemy_Dekubaba_Withered_Dictionary", "Enemy_Gohma_Egg", "Enemy_Scrub_Deku_Brothers", "/Enemy_Dekubaba"):
            lab: str = L.radar_label(n)
            self.assertTrue(all(c.isalnum() or c == " " for c in lab), lab)
            self.assertFalse(lab.startswith("/") or lab.startswith(" "), lab)
        self.assertEqual(L.radar_label("Enemy_Dekubaba_Withered"), "WithBaba")
        self.assertEqual(L.radar_label("Enemy_Gohma_Egg"), "GohEgg")
        self.assertEqual(L.radar_label("Enemy_Moblin_Club"), "ClubMob*")

    def test_habitat(self):
        self.assertEqual(L.habitat("Enemy_Keese_Fire"), "fly")
        self.assertEqual(L.habitat("Enemy_Octorok"), "water")
        self.assertEqual(L.habitat("Enemy_Stalfos"), "walk")
        self.assertEqual(L.habitat("Enemy_Skullwalltula"), "wall")
        self.assertTrue(all(L.habitat(n) in L.HABITAT_KINDS for n, _ in L.CATALOG))

    def test_marks_labels_have_no_stray_characters(self) -> None:
        pts: list[dict] = [{"n": 1, "map": "A", "pos": [0, 0, 0], "name": "Enemy_Dekubaba", "facing": 0}]
        out: str = L.radar_marks(None, pts, "A", True, tags={1: " /G1"})
        self.assertTrue(out.endswith("Baba G1"), out)
        self.assertNotIn("/", out)


if __name__ == "__main__":
    unittest.main()
