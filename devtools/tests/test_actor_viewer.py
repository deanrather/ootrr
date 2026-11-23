import json
import os
import sys
import tempfile
import unittest
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _paths  # noqa: E402
import actor_names
import fake_actor_panel as F
import paths as PATHS


def get(port: int, path: str) -> bytes:
    return urllib.request.urlopen("http://127.0.0.1:%d%s" % (port, path)).read()


def post(port: int, path: str, body: dict) -> dict:
    r = urllib.request.Request("http://127.0.0.1:%d%s" % (port, path), json.dumps(body).encode(), {"X-Panel": "1"})
    return json.loads(urllib.request.urlopen(r).read())


class Viewer(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.js = os.path.join(cls.tmp.name, "a.js")
        cls.extras = F.extras_dir(cls.tmp.name)
        actor_names.write(PATHS.ootrr_checkout(), os.path.join(cls.extras, "actors.js"), os.path.join(cls.tmp.name, "l.tsv"))
        cls.panel, cls.game = F.make(cls.tmp.name, "new", "", cls.extras)
        cls.srv = F.serve(cls.panel, 0)
        cls.port = cls.srv.server_address[1]

    @classmethod
    def tearDownClass(cls):
        cls.srv.shutdown()
        cls.tmp.cleanup()

    def test_card_and_names_are_served(self):
        st = json.loads(get(self.port, "/state"))
        a = [e for e in st["extras"] if e["name"] == "actors"]
        self.assertEqual(len(a), 1)
        self.assertTrue(a[0]["js"])
        html = get(self.port, "/extras/actors.html").decode()
        self.assertIn("hash (hex, game/objmap)", html)
        self.assertIn("id (decimal, OOTRR)", html)
        js = get(self.port, "/extras/actors.js").decode()
        self.assertIn("8579518566955184384", js)

    def test_actors_rows_carry_placement_ids(self):
        r = post(self.port, "/bridge", {"cmd": "actors", "limit": 800})
        placed = [x for x in r["actors"] if x["hash_id"]]
        self.assertEqual(len(placed), len(F.PLACED))
        self.assertEqual(int(placed[0]["hash"], 16), int(placed[0]["hash_id"]))

    def test_labels_request_is_passed_through(self):
        r = post(self.port, "/bridge", {"cmd": "labels", "op": "on", "format": "dec", "names": "x.tsv", "placed": True})
        self.assertEqual(r["labels"]["format"], "dec")

    def test_state_names_the_ui_and_orders_and_titles_the_cards(self):
        st = json.loads(get(self.port, "/state"))
        self.assertEqual(st["title"], "Mod UI")
        titles = [e["title"] for e in st["extras"]]
        self.assertEqual(titles, ["Actor Viewer", "Enemy-Gate Manager", "Skulltula Manager", "Enemy Placer", "Timed Rupee Puzzle Manager"])
        self.assertEqual(st["steps_info"]["file"], "steps.json")

    def test_the_page_has_every_card_name_and_the_old_overlay_buttons_are_gone(self):
        page = get(self.port, "/").decode()
        self.assertIn("<title>Mod UI</title>", page)
        for name in ("Playtest Guide", "Give/Take", "Go To", "Notes", "Story Flags"):
            self.assertIn("<summary>" + name, page)
        buttons = json.loads(open(os.path.join(_paths.DATA, "buttons.json"), encoding="utf-8").read())
        self.assertEqual([b["label"] for b in buttons if "verlay" in b["label"]], [])
        self.assertNotIn("Playtest panel", page)

    def test_steps_file_can_be_pointed_elsewhere_without_a_restart(self):
        other = os.path.join(self.tmp.name, "other-steps.json")
        with open(other, "w", encoding="utf-8") as f:
            json.dump([{"id": "x1", "title": "From another file"}], f)
        before = json.loads(get(self.port, "/state"))["steps_info"]["sig"]
        with open(os.path.join(self.tmp.name, "notes", "steps-file.txt"), "w", encoding="utf-8") as f:
            f.write(other)
        st = json.loads(get(self.port, "/state"))
        self.assertEqual(st["steps"][0]["title"], "From another file")
        self.assertEqual(st["steps_info"]["file"], "other-steps.json")
        self.assertNotEqual(st["steps_info"]["sig"], before)
        os.remove(os.path.join(self.tmp.name, "notes", "steps-file.txt"))
        self.assertEqual(json.loads(get(self.port, "/state"))["steps_info"]["file"], "steps.json")

    def test_extras_file_writes_into_the_session_and_refuses_odd_names(self):
        r = post(self.port, "/extras_file", {"name": "actor-labels-live.tsv", "text": "1\tx\n"})
        self.assertTrue(r["ok"])
        with open(r["path"], encoding="utf-8") as f:
            self.assertEqual(f.read(), "1\tx\n")
        self.assertFalse(post(self.port, "/extras_file", {"name": "../evil", "text": "x"})["ok"])
        self.assertFalse(post(self.port, "/extras_file", {"name": ".hidden", "text": "x"})["ok"])

    def test_labels_from_the_names_file_are_what_the_game_counts(self):
        tsv = "100000000000000002" + chr(9) + "tree" + chr(10) + "100000000000000004" + chr(9) + "bug" + chr(10)
        names = post(self.port, "/extras_file", {"name": "t.tsv", "text": tsv})["path"]
        r = post(self.port, "/bridge", {"cmd": "labels", "op": "on", "prefixes": [], "range": 30, "max": 24, "placed": True, "named_only": True, "format": "dec", "names": names})
        self.assertEqual(r["labels"]["shown"], 2)
        self.assertEqual(sorted(self.game.hit_names), ["Animal_Kokiri_Insect_A", "Tree_Kokiri_BillboardTree_A_01"])


if __name__ == "__main__":
    unittest.main()
