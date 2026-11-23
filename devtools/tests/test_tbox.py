"""python -I test_tbox.py   (the foreground test is skipped unless TBOX_TEST_WINDOWS=1; Notepad stands in for the game: the foreground check takes a process name)"""
import json
import os
import subprocess
import tempfile
import time
import unittest

import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _paths  # noqa: E402
import tbox


class T(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        os.makedirs(os.path.join(self.d, "notes"))

    def notes(self):
        with open(os.path.join(self.d, "notes", "notes.jsonl"), encoding="utf-8") as f:
            return [json.loads(x) for x in f]

    def test_append(self):
        with open(os.path.join(self.d, "notes", "state.json"), "w") as f:
            json.dump({"t": time.time(), "status": {"map": "Kakariko", "pos": [1, 2, 3], "life": 48, "junk": 1}}, f)
        e = tbox.append_note(self.d, "  hello\nworld  ")
        self.assertEqual(e["text"], "hello\nworld")
        n = self.notes()[0]
        self.assertEqual((n["kind"], n["source"], n["map"], n["pos"]), ("note", "t-box", "Kakariko", [1, 2, 3]))
        self.assertNotIn("junk", n)
        self.assertIsNone(tbox.append_note(self.d, "   "))
        tbox.append_note(self.d, "two")
        self.assertEqual(len(self.notes()), 2)

    def test_stale_state(self):
        with open(os.path.join(self.d, "notes", "state.json"), "w") as f:
            json.dump({"t": time.time() - 100, "status": {"map": "X"}}, f)
        tbox.append_note(self.d, "a")
        self.assertNotIn("map", self.notes()[0])

    def test_last_reply(self):
        self.assertEqual(tbox.last_reply(self.d), "")
        with open(os.path.join(self.d, "notes", "messages.jsonl"), "w") as f:
            f.write('{"text": "one", "time": "1"}\n{"text": "two", "time": "2"}\n')
        self.assertIn("two", tbox.last_reply(self.d))

    def test_rebinds(self):
        os.makedirs(os.path.join(self.d, "user"))
        with open(os.path.join(self.d, "user", "longshot_options.ini"), "w") as f:
            f.write("key_a=69,0\nkey_walk=84,0\n")
        self.assertEqual(tbox.key_rebinds(self.d, 84), ["walk"])
        self.assertEqual(tbox.key_rebinds(self.d, 85), [])

    @unittest.skipUnless(os.environ.get("TBOX_TEST_WINDOWS") == "1", "opens Notepad and takes focus: set TBOX_TEST_WINDOWS=1 to run")
    def test_foreground(self):
        p = subprocess.Popen(["notepad.exe"])
        try:
            end = time.time() + 10
            fg = None
            while time.time() < end and not fg:
                time.sleep(0.5)
                h = tbox.user32.FindWindowW("Notepad", None)
                if h:
                    tbox.force_foreground(h)
                fg = tbox.foreground("notepad.exe")
            if not fg:
                self.skipTest("could not bring Notepad to the front")
            self.assertEqual(len(fg[1]), 4)
            self.assertIsNone(tbox.foreground("longshot.exe"))
            self.assertIsNone(tbox.foreground("notepad.exe", own_pid=tbox.window_pid(fg[0])))
        finally:
            p.kill()


if __name__ == "__main__":
    unittest.main()
