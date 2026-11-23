"""Turn the enemy placer's spawn points into the mod's table of hand-placed enemies.

    python placed_export.py [--points SESSION/notes/spawnpoints.json] [--out <repo>/randomizer/core/src/difficulty/placed_spots.rs] [--check]
    python placed_export.py import-ai SESSION MAP DUMP.json

import-ai merges the random More-enemies fill that `ootrr more-enemies-dump` printed for MAP into
SESSION/notes/spawnpoints.json as points marked "by": "ai" (a copy of the old file is kept beside it),
skipping any spot that already has a point of the same kind within 1 m.

Reads spawnpoints.json (a list, or {"points": [...]}), keeps real enemies only (placer_logic.real_enemy),
drops exact repeats, rounds positions to 1 cm and facings to 0.1 degree, and writes a Rust const table
sorted by map then point number. The enemy's facing is the placer's own rule (placer_logic.point_yaw):
"yaw" when the point has one, else Link's facing turned round. --check writes nothing and exits 1 if the
table on disk differs from what the points would give.
"""
import argparse
import json
import math
import os
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import placer_logic as L

import paths as PATHS  # noqa: E402

POINTS: str = os.path.join(PATHS.session(), "notes", "spawnpoints.json")
OUT: str = os.path.join(PATHS.session(), "notes", "placed_spots.rs")
HEADER: str = "use crate::difficulty::placed::Placed;\n\npub const SPOTS: &[Placed] = &[\n"
FOOTER: str = "];\n"


def load(path: str) -> list[dict]:
    with open(path, encoding="utf-8") as f:
        data = json.load(f)
    return data.get("points", []) if isinstance(data, dict) else list(data)


def wrap(deg: float) -> float:
    d: float = math.fmod(deg, 360.0)
    if d <= -180.0:
        d += 360.0
    if d > 180.0:
        d -= 360.0
    return d


def num(v: float, places: int) -> str:
    r: float = round(float(v), places)
    if r == 0.0:
        r = 0.0
    s: str = f"{r:.{places}f}".rstrip("0")
    return s + "0" if s.endswith(".") else s


def clean(points: list[dict]) -> tuple[list[dict], list[str]]:
    out: list[dict] = []
    dropped: list[str] = []
    seen: set[tuple] = set()
    for p in points:
        name: str = str(p.get("name") or "")
        where: str = str(p.get("map") or "")
        pos = p.get("pos")
        tag: str = f"n={p.get('n')} {where} {name}"
        if not where or not isinstance(pos, list) or len(pos) != 3:
            dropped.append(f"{tag}: no map or position")
            continue
        if not L.real_enemy(name):
            dropped.append(f"{tag}: not a real enemy")
            continue
        at: list[float] = [round(float(v), 2) for v in pos]
        yaw: float = round(wrap(math.degrees(L.point_yaw(p))), 1)
        key: tuple = (where, name, *at)
        if key in seen:
            dropped.append(f"{tag}: repeat of an earlier point")
            continue
        seen.add(key)
        out.append({"n": int(p.get("n", 0)), "map": where, "name": name, "at": at, "yaw": yaw})
    out.sort(key=lambda q: (q["map"], q["n"]))
    return out, dropped


def render(spots: list[dict]) -> str:
    rows: list[str] = []
    for s in spots:
        at: str = ", ".join(num(v, 2) for v in s["at"])
        rows.append(f'    Placed {{ n: {s["n"]}, map: "{s["map"]}", gyaml: "{s["name"]}", at: [{at}], yaw: {num(s["yaw"], 1)} }},\n')
    return HEADER + "".join(rows) + FOOTER


def counts(spots: list[dict]) -> dict[str, int]:
    out: dict[str, int] = {}
    for s in spots:
        out[s["map"]] = out.get(s["map"], 0) + 1
    return out


SAME_KIND: float = 1.0


def import_ai(points: list[dict], dump: dict, map_name: str, when: str) -> tuple[list[dict], int, int]:
    out: list[dict] = list(points)
    added: int = 0
    skipped: int = 0
    for s in dump.get("spots", []):
        if s.get("map") != map_name or s.get("by", "ai") != "ai":
            continue
        pos: list[float] = [round(float(v), 2) for v in s["pos"]]
        if any(p.get("map") == map_name and p.get("name") == s["name"] and isinstance(p.get("pos"), list) and L.dist(p["pos"], pos) <= SAME_KIND for p in out):
            skipped += 1
            continue
        yaw: float = round(wrap(float(s.get("yaw", 0.0))), 1)
        p: dict = {"n": L.next_n(out), "map": map_name, "name": s["name"], "pos": pos, "facing": round(wrap(yaw - 180.0), 1), "yaw": yaw, "time": when, "by": "ai"}
        if s.get("room") is not None:
            p["room"] = s["room"]
        out.append(p)
        added += 1
    return out, added, skipped


def import_main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: placed_export.py import-ai SESSION MAP DUMP.json")
        return 2
    session, map_name, dump_path = argv
    path: str = os.path.join(session, "notes", "spawnpoints.json")
    with open(path, encoding="utf-8") as f:
        raw = json.load(f)
    points: list[dict] = raw.get("points", []) if isinstance(raw, dict) else list(raw)
    with open(dump_path, encoding="utf-8") as f:
        dump: dict = json.load(f)
    when: str = time.strftime("%H:%M:%S")
    merged, added, skipped = import_ai(points, dump, map_name, when)
    if added:
        backup: str = f"{path}.before-import-{time.strftime('%Y%m%d-%H%M%S')}"
        shutil.copyfile(path, backup)
        if isinstance(raw, dict):
            raw["points"] = merged
        else:
            raw = merged
        tmp: str = path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(raw, f, indent=1)
        os.replace(tmp, path)
        print(f"old file kept as {backup}")
    print(f"{map_name}: {added} AI points added, {skipped} already had a point of the same kind within {SAME_KIND} m")
    return 0


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if argv and argv[0] == "import-ai":
        return import_main(argv[1:])
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--points", default=POINTS)
    ap.add_argument("--out", default=OUT)
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args(argv)
    spots, dropped = clean(load(a.points))
    text: str = render(spots)
    for d in dropped:
        print("dropped", d)
    for m, n in sorted(counts(spots).items()):
        print(f"{m}: {n}")
    if a.check:
        try:
            with open(a.out, encoding="utf-8") as f:
                same: bool = f.read() == text
        except OSError:
            same = False
        print("table is current" if same else "table differs from the points")
        return 0 if same else 1
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(f"wrote {len(spots)} spots to {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
