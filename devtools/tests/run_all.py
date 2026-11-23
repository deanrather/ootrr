"""Runs every unit test in this folder: python devtools/tests/run_all.py  (no game, no Longshot; test_pwa needs node)."""
import os
import subprocess
import sys

HERE: str = os.path.dirname(os.path.abspath(__file__))
bad: int = 0
for f in sorted(os.listdir(HERE)):
    if f.startswith("test_") and f.endswith(".py"):
        r = subprocess.run([sys.executable, "-I", "-B", os.path.join(HERE, f)], capture_output=True, text=True)
        tail: str = next((l for l in reversed(r.stderr.splitlines()) if l.startswith(("OK", "FAILED"))), "?")
        print("%-28s %s" % (f, tail))
        bad += r.returncode != 0
sys.exit(1 if bad else 0)
