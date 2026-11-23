"""Writes cache/places-all.json: one Go to place per randomizer check, from OOTRR's data/tracker_targets.json joined to the
game's own map placements (cache/actor-index.json): {label, group, map, warps: [[x, y + 1, z]], check, kind}. The hand-measured
places in data/places.json come first and win on equal labels.
"""
import json
import os
import sys

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import paths as PATHS  # noqa: E402


def build(checkout: str, ix: dict, base: list) -> list[dict]:
    p: str = os.path.join(checkout, "data", "tracker_targets.json")
    try:
        with open(p, encoding="utf-8-sig") as f:
            targets: list = json.load(f).get("targets", [])
    except (OSError, ValueError):
        return list(base)
    out: list[dict] = list(base)
    labels: set = {x.get("label") for x in out}
    for t in targets:
        h: str = str(t.get("id", "")).split(":", 1)[-1]
        e = ix.get(h)
        if not e:
            continue
        label: str = "%s: %s" % (t.get("kind", ""), t.get("name", ""))
        if label in labels:
            label += " [%s]" % h[-6:]
        labels.add(label)
        out.append({"label": label, "group": "Check: " + (t.get("area") or t.get("scene") or "Other"), "map": e[1],
                    "warps": [[round(e[3], 2), round(e[4] + 1.0, 2), round(e[5], 2)]], "check": t["id"], "kind": t.get("kind", "")})
    return out


def write(checkout: str, ix: dict, out: str | None = None) -> int:
    try:
        with open(os.path.join(PATHS.DATA, "places.json"), encoding="utf-8-sig") as f:
            base: list = json.load(f)
    except (OSError, ValueError):
        base = []
    rows: list[dict] = build(checkout, ix, base)
    p: str = out or PATHS.cache("places-all.json")
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w", encoding="utf-8") as f:
        json.dump(rows, f, indent=1)
    return len(rows)
