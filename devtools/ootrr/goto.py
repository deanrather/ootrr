"""Teleport to a map and warp within it, safely: retry a refused teleport (one every 8 s, events), wait until Link is in
the map with no event or load, and warp only then. Never warps in the wrong map (that dropped the user under the field
twice on 23 Nov). "arrived" is printed only from a fresh status: game state play (not loading), the expected map, and
Link within ARRIVED units of the target in x and z. The status snapshot can hold the old map and position during a load,
so the load state counts, not the position alone.

    python -I goto.py RUN MAP X Y Z [--health HEARTS]   (hearts, e.g. 20; caps at full)

Exit codes: 0 arrived; 1 Link never reached the map (timeout); 2 in the map but not at the target; 3 the game is not
answering; 4 the load seems hung (the game stayed in loading for more than HUNG seconds, 60).

go() is the same logic for other tools (the panel's gate, puzzle and Gold Skulltula cards call it through warp_runner.py):
it takes a status function (the bridge's game dict) and a send function (a bridge command, returns its text).
LONGSHOT_CTL names the bridge script.
"""
import argparse
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import paths as PATHS  # noqa: E402

CTL = PATHS.longshot_ctl()
WAIT: float = 60.0
ARRIVED: float = 4.0
HUNG: float = 60.0
CHECKS: int = 6


def playing(s: dict) -> bool:
    return bool(s) and not s.get("loading") and s.get("state", "play") in ("play", "playing", None)


def ctl(run: str, *a: str) -> str:
    r = subprocess.run([sys.executable, "-I", CTL, "--run", run] + list(a), capture_output=True, text=True, timeout=40)
    return (r.stdout + r.stderr).strip()


def status(run: str) -> dict:
    for line in ctl(run, "status").splitlines():
        if line.startswith("game"):
            return json.loads(line[4:].strip())
    return {}


def settled(s: dict, map_name: str) -> bool:
    return s.get("map") == map_name and playing(s) and not s.get("event")


def arrived(s: dict, map_name: str, pos: list[float]) -> bool:
    p = s.get("pos") or [0, 0, 0]
    return s.get("map") == map_name and playing(s) and abs(float(p[0]) - pos[0]) < ARRIVED and abs(float(p[2]) - pos[2]) < ARRIVED


def go(get_status, send, map_name: str, pos: list[float], say=lambda m: None, sleep=time.sleep, clock=time.monotonic) -> tuple[bool, str]:
    end: float = clock() + WAIT
    gone: int = 0
    lstart: float | None = None
    while True:
        s: dict = get_status() or {}
        if s and not playing(s):
            if lstart is None:
                lstart = clock()
            n: float = clock() - lstart
            if n > HUNG:
                return False, f"load seems hung ({int(n)} s)"
            end = max(end, clock() + 5)
        else:
            lstart = None
        if settled(s, map_name):
            break
        if not s:
            gone += 1
            if gone >= 3:
                return False, "the game isn't answering (closed or crashed); no warp"
            sleep(1)
            continue
        gone = 0
        if clock() > end:
            return False, f"Link is not in {map_name} after {int(WAIT)} s; no warp"
        if s.get("map") != map_name and not s.get("loading") and not s.get("event"):
            out: str = send("teleport", map_name) or ""
            say(f"teleporting to {map_name}: " + (out.splitlines()[-1] if out.strip() else "sent"))
            sleep(3)
        else:
            say("waiting for the load or event to finish")
            sleep(1)
    for _ in range(10):
        out = send("warp", str(pos[0]), str(pos[1]), str(pos[2])) or ""
        if "refused" not in out:
            break
        sleep(1)
    sleep(1)
    s = get_status() or {}
    for _ in range(CHECKS):
        if arrived(s, map_name, pos):
            break
        if s and not playing(s):
            if lstart is None:
                lstart = clock()
            if clock() - lstart > HUNG:
                return False, f"load seems hung ({int(clock() - lstart)} s)"
        sleep(1)
        s = get_status() or {}
    p = s.get("pos") or [0, 0, 0]
    if arrived(s, map_name, pos):
        return True, f"arrived in {map_name} at {[round(float(v), 1) for v in p]}"
    if not playing(s):
        return False, f"{s.get('map')}: still loading after the warp; NOT at the target"
    return False, f"{s.get('map')} at {[round(float(v), 1) for v in p]}: NOT at the target"


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("run")
    p.add_argument("map")
    p.add_argument("x", type=float)
    p.add_argument("y", type=float)
    p.add_argument("z", type=float)
    p.add_argument("--health", type=int)
    a = p.parse_args()
    ok, msg = go(lambda: status(a.run), lambda *c: ctl(a.run, *c), a.map, [a.x, a.y, a.z], say=lambda m: print(m, flush=True))
    if a.health is not None:
        ctl(a.run, "health", str(a.health))
    print("goto: " + msg, flush=True)
    sys.exit(0 if ok else (3 if "answering" in msg else 4 if "hung" in msg else 2 if "NOT" in msg else 1))


if __name__ == "__main__":
    main()
