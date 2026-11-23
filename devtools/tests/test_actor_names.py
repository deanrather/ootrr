import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _paths  # noqa: E402
import actor_names as A

import paths as PATHS  # noqa: E402

CHECKOUT = PATHS.ootrr_checkout()


def fake_checkout(d: str) -> str:
    os.makedirs(os.path.join(d, "data"))
    with open(os.path.join(d, "data", "locations.json"), "w", encoding="utf-8") as f:
        json.dump({"locations": [
            {"name": "A Chest", "kind": "chest", "refs": [{"hash": "111"}, {"hash": "112"}]},
            {"name": "Other", "kind": "pot", "refs": [{"hash": "111"}]},
            {"name": "Only Loc", "kind": "pot", "refs": [{"hash": "113"}]},
        ]}, f)
    with open(os.path.join(d, "data", "tracker_targets.json"), "w", encoding="utf-8") as f:
        json.dump({"targets": [
            {"id": "chest:111", "name": "T R01 Chest 1", "ootr": "A Chest", "kind": "chest", "scene": "S", "map": "S", "room": 1},
            {"id": "pot:200", "name": "T Pot", "kind": "pot", "keys": ["200", "201"]},
            {"id": "gift:Named_Gift", "name": "Gift", "kind": "gift"},
        ]}, f)
    return d


class Names(unittest.TestCase):
    def test_keys_by_decimal_id_and_repeats(self):
        with tempfile.TemporaryDirectory() as d:
            r = A.build(fake_checkout(d))
        ids = r["ids"]
        self.assertTrue(r["ok"])
        self.assertEqual([x[0] for x in ids["111"]["l"]], ["A Chest", "Other"])
        self.assertEqual(ids["111"]["t"][0][1], "A Chest")
        self.assertEqual(ids["112"]["t"], [])
        self.assertEqual(ids["113"]["t"], [])
        self.assertEqual(ids["201"]["t"][0][0], "T Pot")
        self.assertNotIn("Named_Gift", ids)

    def test_missing_files_say_so(self):
        with tempfile.TemporaryDirectory() as d:
            r = A.build(d)
        self.assertFalse(r["ok"])
        self.assertFalse(r["loc_found"])

    def test_writes_js_and_labels(self):
        with tempfile.TemporaryDirectory() as d:
            c = fake_checkout(os.path.join(d, "o"))
            js, tsv = os.path.join(d, "a.js"), os.path.join(d, "l.tsv")
            A.write(c, js, tsv)
            text = open(js, encoding="utf-8").read()
            self.assertTrue(text.startswith("window.OOTRR_NAMES={"))
            self.assertIn("window.actNamesSet", text)
            lines = open(tsv, encoding="utf-8").read().splitlines()
        self.assertIn("111\t111 A Chest", lines)
        self.assertIn("200\t200 T Pot", lines)

    @unittest.skipUnless(os.path.isfile(os.path.join(CHECKOUT, "data", "locations.json")), "no OOTRR checkout")
    def test_real_files_known_ids(self):
        r = A.build(CHECKOUT)
        e = r["ids"]["8579518566955184384"]
        self.assertEqual(e["l"][0][0], "Deku Tree Map Chest")
        self.assertEqual(e["t"][0][0], "Deku Tree R00 Chest 1")
        self.assertEqual("0x%016x" % 2527478394367885385, "0x2313683561c5c049")
        self.assertGreater(len(r["ids"]), 1000)


if __name__ == "__main__":
    unittest.main()
