import json
import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _paths  # noqa: E402


class Places(unittest.TestCase):
    def test_go_to_starts_with_quiet_places_and_the_field_is_not_first(self):
        for name in (os.path.join(_paths.DATA, "places.json"), os.path.join(_paths.PANEL, "examples", "places.json")):
            with open(name, encoding="utf-8-sig") as f:
                d = json.load(f)
            rows: list[dict] = d["places"] if isinstance(d, dict) else d
            self.assertNotIn("Hyrule Field", rows[0]["label"], name)
            self.assertNotIn("Field", rows[0]["map"], name)
        with open(os.path.join(_paths.DATA, "places.json"), encoding="utf-8-sig") as f:
            first: dict = json.load(f)[0]
        self.assertEqual(first["group"], "Quiet places")


class Goto(unittest.TestCase):
    def test_goto_uses_the_scripts_folder_it_is_given(self):
        with tempfile.TemporaryDirectory() as d:
            with open(os.path.join(d, "longshot_ctl.py"), "w", encoding="utf-8") as f:
                f.write("import sys\nprint('game {\"map\": \"Z\", \"state\": \"play\"}' if 'status' in sys.argv else 'ok ' + ' '.join(sys.argv[1:]))\n")
            env: dict = {k: v for k, v in os.environ.items() if k not in ("LONGSHOT_SCRIPTS", "LONGSHOT_CTL")}
            env["DEVTOOLS_CONFIG"] = os.path.join(d, "none.json")
            code: str = ("import sys; sys.path.insert(0, %r); import goto; goto.PATHS.os.environ['LONGSHOT_SCRIPTS'] = %r; "
                         "print(goto.status('RUN'), goto.ctl('RUN', 'x'))" % (os.path.join(_paths.ROOT, "ootrr"), d))
            r = subprocess.run([sys.executable, "-I", "-c", code], capture_output=True, text=True, env=env)
            self.assertIn("'map': 'Z'", r.stdout, r.stderr)
            self.assertIn("ok --run RUN x", r.stdout)

    def test_goto_says_so_when_no_scripts_folder_is_set(self):
        env: dict = {k: v for k, v in os.environ.items() if k not in ("LONGSHOT_SCRIPTS", "LONGSHOT_CTL")}
        env["DEVTOOLS_CONFIG"] = os.path.join(tempfile.gettempdir(), "no-such-devtools-config.json")
        r = subprocess.run([sys.executable, "-I", os.path.join(_paths.ROOT, "ootrr", "goto.py"), "RUN", "M", "1", "2", "3"], capture_output=True, text=True, env=env)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("LONGSHOT_SCRIPTS", r.stderr)


if __name__ == "__main__":
    unittest.main()
