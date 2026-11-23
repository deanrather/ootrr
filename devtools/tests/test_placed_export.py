import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _paths  # noqa: E402
import placed_export as X


def pt(n, name, pos, facing=0.0, yaw=None, m="DodongoCavern"):
    p = {"n": n, "map": m, "name": name, "pos": pos, "facing": facing, "time": "t"}
    if yaw is not None:
        p["yaw"] = yaw
    return p


class Export(unittest.TestCase):
    def test_keeps_real_enemies_and_drops_the_rest(self):
        spots, dropped = X.clean([
            pt(1, "Enemy_Keese", [1, 2, 3]),
            pt(2, "Enemy_Skulltula_Gold", [1, 2, 3]),
            pt(3, "Obj_Pot", [1, 2, 3]),
            pt(4, "Enemy_DungeonBoss_Dodongo", [1, 2, 3]),
            {"n": 5, "map": "DodongoCavern", "name": "Enemy_Keese"},
        ])
        self.assertEqual([s["n"] for s in spots], [1])
        self.assertEqual(len(dropped), 4)

    def test_facing_follows_the_placer(self):
        spots, _ = X.clean([pt(1, "Enemy_Beamos", [0, 0, 0], facing=30.0), pt(2, "Enemy_Beamos", [5, 0, 0], facing=110.9, yaw=291.3)])
        self.assertEqual(spots[0]["yaw"], -150.0)
        self.assertEqual(spots[1]["yaw"], -68.7)

    def test_rounding_repeats_and_order(self):
        spots, dropped = X.clean([
            pt(9, "Enemy_Amos", [1.004, 2.0, 3.0], m="HyruleField_Child"),
            pt(3, "Enemy_Amos", [1.0, 2.0, 3.0], m="HyruleField_Child"),
            pt(7, "Enemy_Amos", [-0.001, 2.0, 3.0]),
        ])
        self.assertEqual([(s["map"], s["n"]) for s in spots], [("DodongoCavern", 7), ("HyruleField_Child", 9)])
        self.assertEqual(spots[0]["at"], [-0.0, 2.0, 3.0])
        self.assertEqual(len(dropped), 1)

    def test_render_is_a_rust_table(self):
        spots, _ = X.clean([pt(4, "Enemy_Beamos", [10.56, 149.23, 93.85], yaw=15.3, m="HyruleField_Child"), pt(5, "Enemy_Keese", [-0.004, 100.0, -2.5], yaw=-180.0)])
        text = X.render(spots)
        self.assertTrue(text.startswith(X.HEADER) and text.endswith(X.FOOTER))
        self.assertIn('Placed { n: 5, map: "DodongoCavern", gyaml: "Enemy_Keese", at: [0.0, 100.0, -2.5], yaw: 180.0 },', text)
        self.assertIn('Placed { n: 4, map: "HyruleField_Child", gyaml: "Enemy_Beamos", at: [10.56, 149.23, 93.85], yaw: 15.3 },', text)
        self.assertNotIn("-0.0", text)

    def test_main_writes_and_checks(self):
        with tempfile.TemporaryDirectory() as d:
            src = os.path.join(d, "spawnpoints.json")
            out = os.path.join(d, "placed_spots.rs")
            with open(src, "w") as f:
                json.dump({"points": [pt(1, "Enemy_Keese", [1, 2, 3])]}, f)
            self.assertEqual(X.main(["--points", src, "--out", out, "--check"]), 1)
            self.assertEqual(X.main(["--points", src, "--out", out]), 0)
            self.assertEqual(X.main(["--points", src, "--out", out, "--check"]), 0)
            with open(out) as f:
                self.assertIn("\"Enemy_Keese\"", f.read())


    def test_import_ai_merges_without_repeats(self):
        have = [pt(1, "Enemy_Keese", [0.0, 0.0, 0.0]), pt(2, "Enemy_Amos", [50.0, 0.0, 0.0])]
        dump = {"spots": [
            {"map": "DodongoCavern", "name": "Enemy_Keese", "pos": [0.5, 0.0, 0.5], "yaw": 10.0, "room": 3, "by": "ai"},
            {"map": "DodongoCavern", "name": "Enemy_Amos", "pos": [0.5, 0.0, 0.5], "yaw": 190.0, "room": 3, "by": "ai"},
            {"map": "DodongoCavern", "name": "Enemy_Amos", "pos": [9.0, 0.0, 0.0], "yaw": 0.0, "room": 3, "by": "user"},
            {"map": "HyruleField_Child", "name": "Enemy_Keese", "pos": [0.0, 0.0, 0.0], "yaw": 0.0, "room": None, "by": "ai"},
        ]}
        out, added, skipped = X.import_ai(have, dump, "DodongoCavern", "12:00:00")
        self.assertEqual((added, skipped), (1, 1))
        new = out[-1]
        self.assertEqual((new["n"], new["name"], new["by"], new["yaw"], new["facing"], new["room"], new["time"]), (3, "Enemy_Amos", "ai", -170.0, 10.0, 3, "12:00:00"))
        spots, _ = X.clean(out)
        self.assertEqual(next(s for s in spots if s["n"] == 3)["yaw"], -170.0)

    def test_import_ai_command_keeps_a_copy(self):
        with tempfile.TemporaryDirectory() as d:
            os.makedirs(os.path.join(d, "notes"))
            path = os.path.join(d, "notes", "spawnpoints.json")
            with open(path, "w") as f:
                json.dump([pt(1, "Enemy_Keese", [0.0, 0.0, 0.0])], f)
            dump = os.path.join(d, "dump.json")
            with open(dump, "w") as f:
                json.dump({"spots": [{"map": "DodongoCavern", "name": "Enemy_Amos", "pos": [5, 0, 5], "yaw": 0.0, "room": 1, "by": "ai"}]}, f)
            self.assertEqual(X.main(["import-ai", d, "DodongoCavern", dump]), 0)
            with open(path) as f:
                self.assertEqual(len(json.load(f)), 2)
            self.assertEqual(len([n for n in os.listdir(os.path.join(d, "notes")) if "before-import" in n]), 1)
            self.assertEqual(X.main(["import-ai", d, "DodongoCavern", dump]), 0)
            with open(path) as f:
                self.assertEqual(len(json.load(f)), 2)


if __name__ == "__main__":
    unittest.main()
