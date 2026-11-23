"""Test setup: puts devtools/ootrr and devtools/panel on sys.path and points the Mod UI server at a stub Longshot bridge
unless LONGSHOT_SCRIPTS names a real Longshot scripts folder. Import it first."""
import os
import sys

TESTS: str = os.path.dirname(os.path.abspath(__file__))
ROOT: str = os.path.dirname(TESTS)
OOTRR: str = os.path.join(ROOT, "ootrr")
PANEL: str = os.path.join(ROOT, "panel")
DATA: str = os.path.join(ROOT, "data")
if not os.environ.get("LONGSHOT_SCRIPTS"):
    os.environ["LONGSHOT_SCRIPTS"] = os.path.join(TESTS, "stub")
for p in (OOTRR, PANEL):
    if p not in sys.path:
        sys.path.insert(0, p)
