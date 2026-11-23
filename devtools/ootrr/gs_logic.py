"""Gold Skulltula spots marked from the phone panel's Gold Skulltula card: pure data functions, no game, no server.

The inventory is cache/gs-list.json (from the Gold Skulltula work, made from `ootrr gs-list`):
one record per Gold Skulltula hash, with every map file that holds it. The marks live in SESSION/notes/panel_gs.json:
{"high": N, "marks": [mark, ...], "skipped": [hash, ...]}, a mark {n, hash, name, map, pos, facing, surface, to, time}:
pos and facing are Link's (feet, degrees), to is where the Skulltula goes. One mark per Gold Skulltula per map; a new one
for the same Gold Skulltula moves it. Mark numbers are never reused.

The export is the file the moved-Gold-Skulltulas feature reads (data/gs_positions.json, feature/gs-relocate, gs_move.rs;
the same shape the Gold Skulltula tooling writes): {"format": 1, "entries": [{hash, name, file, to, facing | rotate}],
"skipped": [{name, hash}]}.
  wall:   to = Link + 0.8 ahead + 1.0 up (the tour's 'gs here'), facing = Link's facing: the mod turns it back at Link
          (euler(pi/2, facing + pi)). Stand facing the wall.
  ground: to = Link's feet, rotate = [0, facing in radians, 0]: it stands upright and looks where Link looks. Stand on the
          spot facing the way the player will come from.
A Gold Skulltula that sits at the same spot in the other age's map (same_as_first) gets the same move there too, unless that
map has its own mark.

    python -I gs_logic.py export SESSION OUT.json   writes the export and prints a report
"""
import json
import math
import os
import sys

HERE: str = os.path.dirname(os.path.abspath(__file__))
FILE_NAME: str = "panel_gs.json"
EXPORT_NAME: str = "gs_positions.json"
sys.path.insert(0, HERE)
import paths as PATHS  # noqa: E402

LIST_CANDIDATES: tuple[str, ...] = (PATHS.cache("gs-list.json"),)
TOUR: str = PATHS.cache("gs-tour.json")
AHEAD: float = 0.8
UP: float = 1.0
SURFACES: tuple[str, ...] = ("wall", "ground")


def dist(a: list[float], b: list[float]) -> float:
    return math.sqrt(sum((float(a[i]) - float(b[i])) ** 2 for i in range(3)))


def empty() -> dict:
    return {"high": 0, "marks": [], "skipped": []}


def tidy(state) -> dict:
    if not isinstance(state, dict):
        return empty()
    return {"high": int(state.get("high", 0)), "marks": [m for m in state.get("marks", []) if isinstance(m, dict)],
            "skipped": [str(h) for h in state.get("skipped", [])]}


def short(name: str) -> str:
    return name.split(" GS ", 1)[-1] if " GS " in name else name


def files_on(rec: dict, map_name: str) -> list[dict]:
    return [f for f in rec.get("files", []) if f.get("map") == map_name]


def on_map(gs: list[dict], map_name: str) -> list[dict]:
    return [r for r in gs if files_on(r, map_name)]


def at_on(rec: dict, map_name: str) -> list[float]:
    f: list[dict] = files_on(rec, map_name)
    return [float(v) for v in (f[0]["at"] if f else rec["at"])]


def by_hash(gs: list[dict], h: str) -> dict | None:
    for r in gs:
        if str(r.get("hash")) == str(h):
            return r
    return None


def nearest_original(gs: list[dict], map_name: str, pos: list[float]) -> dict | None:
    here: list[dict] = on_map(gs, map_name)
    return min(here, key=lambda r: dist(at_on(r, map_name), pos)) if here else None


def target(gs: list[dict], map_name: str, pos: list[float], chosen: str | None) -> tuple[dict | None, bool]:
    if chosen:
        r: dict | None = by_hash(gs, chosen)
        if r is not None and files_on(r, map_name):
            return r, True
    return nearest_original(gs, map_name, pos), False


def forward(deg: float) -> tuple[float, float]:
    r: float = math.radians(deg)
    return math.sin(r), math.cos(r)


def spot(pos: list[float], facing: float, surface: str) -> list[float]:
    if surface == "ground":
        return [round(float(v), 3) for v in pos]
    fx, fz = forward(facing)
    return [round(float(pos[0]) + fx * AHEAD, 3), round(float(pos[1]) + UP, 3), round(float(pos[2]) + fz * AHEAD, 3)]


def mark_of(state: dict, h: str, map_name: str) -> dict | None:
    for m in state["marks"]:
        if str(m.get("hash")) == str(h) and m.get("map") == map_name:
            return m
    return None


def mark(state: dict, rec: dict, map_name: str, pos: list[float], facing: float, surface: str, when: str = "", to: list[float] | None = None, via: str = "") -> tuple[dict, str]:
    if surface not in SURFACES:
        raise ValueError("surface must be wall or ground")
    if not files_on(rec, map_name):
        raise ValueError(f"{rec['name']} is not in {map_name}: the mod moves a Gold Skulltula inside its own map only")
    old: dict | None = mark_of(state, rec["hash"], map_name)
    state["high"] = int(state.get("high", 0)) + 1
    m: dict = {"n": state["high"], "hash": str(rec["hash"]), "name": rec["name"], "map": map_name,
               "pos": [round(float(v), 2) for v in pos], "facing": round(float(facing or 0.0), 1), "surface": surface,
               "to": [round(float(v), 3) for v in to] if to else spot(pos, float(facing or 0.0), surface), "time": when}
    if via:
        m["via"] = via
    state["marks"] = [x for x in state["marks"] if x is not old] + [m]
    state["skipped"] = [h for h in state["skipped"] if h != str(rec["hash"])]
    return m, "moved" if old else "placed"


def aim_surface(hit: dict) -> str:
    return "wall" if str(hit.get("surface") or "floor") == "wall" else "ground"


def aim_facing(hit: dict, link: list[float] | None, link_facing: float = 0.0) -> float:
    pos: list[float] = [float(v) for v in hit["pos"]]
    if aim_surface(hit) == "wall":
        yo = hit.get("yaw_out")
        if yo is not None:
            out: float = math.degrees(float(yo))
        else:
            n: list = hit.get("normal") or [0.0, 0.0, 1.0]
            out = math.degrees(math.atan2(float(n[0]), float(n[2])))
        f: float = out + 180.0
        return round(((f + 180.0) % 360.0) - 180.0, 1)
    if link and len(link) == 3 and math.hypot(float(link[0]) - pos[0], float(link[2]) - pos[2]) >= 0.05:
        return round(math.degrees(math.atan2(float(link[0]) - pos[0], float(link[2]) - pos[2])), 1)
    return round(float(link_facing or 0.0), 1)


def mark_aim(state: dict, rec: dict, map_name: str, hit: dict, link: list[float] | None, link_facing: float = 0.0, when: str = "") -> tuple[dict, str]:
    pos: list[float] = [float(v) for v in hit["pos"]]
    return mark(state, rec, map_name, link if link and len(link) == 3 else pos, aim_facing(hit, link, link_facing), aim_surface(hit), when, to=pos, via="aim")


def nearest_mark_to(state: dict, map_name: str, pos: list[float], reach: float = 2.0) -> dict | None:
    here: list[dict] = [m for m in marks_on(state, map_name) if dist(m["to"], pos) <= reach]
    return min(here, key=lambda m: dist(m["to"], pos)) if here else None


def marks_on(state: dict, map_name: str) -> list[dict]:
    return [m for m in state["marks"] if m.get("map") == map_name]


def nearest_mark(state: dict, map_name: str, pos: list[float]) -> dict | None:
    here: list[dict] = marks_on(state, map_name)
    return min(here, key=lambda m: dist(m["pos"], pos)) if here else None


def remove_mark(state: dict, n: int) -> dict | None:
    for m in state["marks"]:
        if m.get("n") == n:
            state["marks"] = [x for x in state["marks"] if x is not m]
            return m
    return None


def toggle_skip(state: dict, h: str) -> bool:
    h = str(h)
    if h in state["skipped"]:
        state["skipped"] = [x for x in state["skipped"] if x != h]
        return False
    state["skipped"].append(h)
    state["marks"] = [m for m in state["marks"] if str(m.get("hash")) != h]
    return True


def tag(text: str) -> str:
    return text[:15]


def radar_lines(state: dict, gs: list[dict], map_name: str, near_n: int | None = None) -> list[str]:
    lines: list[str] = []
    for m in marks_on(state, map_name):
        x, y, z = (float(v) for v in m["to"])
        lab: str = tag(("G>" if m["n"] == near_n else "GS ") + short(m["name"]))
        lines.append(f"p {x:.2f} {y:.2f} {z:.2f} {lab}")
    for r in on_map(gs, map_name):
        x, y, z = at_on(r, map_name)
        lines.append(f"v {x:.2f} {y:.2f} {z:.2f} " + tag("old " + short(r["name"])))
    return lines


def entry(rec: dict, f: dict, m: dict) -> dict:
    e: dict = {"hash": str(rec["hash"]), "name": rec["name"], "file": f["file"], "to": list(m["to"])}
    if m.get("surface") == "ground":
        e["rotate"] = [0.0, round(math.radians(float(m.get("facing") or 0.0)), 4), 0.0]
    else:
        e["facing"] = round(float(m.get("facing") or 0.0), 1)
    e["surface"] = m.get("surface", "wall")
    e["from"] = f"panel mark {m['n']}" + ("" if f["map"] == m["map"] else f" (copied from {m['map']}: same spot in both ages)")
    return e


def export(state: dict, gs: list[dict]) -> tuple[dict, list[str]]:
    entries: list[dict] = []
    notes: list[str] = []
    for m in sorted(state["marks"], key=lambda x: int(x["n"])):
        rec: dict | None = by_hash(gs, m["hash"])
        if rec is None:
            notes.append(f"mark {m['n']}: Gold Skulltula {m['hash']} is not in the inventory, left out")
            continue
        own: list[dict] = files_on(rec, m["map"])
        if not own:
            notes.append(f"mark {m['n']}: {rec['name']} is not in {m['map']}, left out")
            continue
        for f in own:
            entries.append(entry(rec, f, m))
        if own[0].get("same_as_first"):
            for f in rec.get("files", []):
                if f["map"] != m["map"] and f.get("same_as_first") and mark_of(state, rec["hash"], f["map"]) is None:
                    entries.append(entry(rec, f, m))
                    notes.append(f"{rec['name']}: also moved in {f['map']} (same spot in both ages); look at that age too")
        d: float = dist(m["to"], at_on(rec, m["map"]))
        if d > 60.0:
            notes.append(f"{rec['name']}: moved {d:.0f} m, check the logic by hand ({rec.get('rule_plain') or rec.get('rule', '')})")
        if rec.get("room") is not None:
            notes.append(f"{rec['name']}: dungeon room {rec['room']}, keep the new spot in the same room")
    skipped: list[dict] = [{"name": r["name"], "hash": str(r["hash"])} for r in (by_hash(gs, h) for h in state["skipped"]) if r]
    return {"format": 1, "note": "exported from the panel's Gold Skulltula card (gs_logic.py); read by gs_move.rs (data/gs_positions.json)",
            "entries": entries, "skipped": skipped}, notes


def tour_warps(steps: list) -> dict[str, tuple[str, list[float]]]:
    out: dict[str, tuple[str, list[float]]] = {}
    for s in steps if isinstance(steps, list) else []:
        for g in s.get("goto", []) or []:
            if g.get("warps") and s.get("hash"):
                out.setdefault(str(s["hash"]) + "@" + g["map"], (g["map"], [float(v) for v in g["warps"][0]]))
    return out


def warp_list(state: dict, gs: list[dict], tour: dict[str, tuple[str, list[float]]]) -> list[dict]:
    out: list[dict] = []
    for m in sorted(state["marks"], key=lambda x: (x["map"], short(x["name"]))):
        out.append({"key": f"m{m['n']}", "kind": "mark", "map": m["map"], "label": f"{short(m['name'])} (new, {m['surface']})", "pos": list(m["pos"])})
    for r in sorted(gs, key=lambda x: (x.get("map", ""), short(x["name"]))):
        for f in r.get("files", []):
            w: tuple[str, list[float]] | None = tour.get(str(r["hash"]) + "@" + f["map"])
            if w:
                out.append({"key": f"o{r['hash']}@{f['map']}", "kind": "original", "map": f["map"], "label": f"{short(r['name'])} (original)", "pos": w[1]})
    return out


def load_json(path: str, default):
    try:
        with open(path, encoding="utf-8-sig") as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def load_list(path: str | None = None) -> tuple[list[dict], str | None]:
    for p in ([path] if path else [os.environ.get("OOTRR_GS_LIST", "")] + list(LIST_CANDIDATES)):
        if p and os.path.isfile(p):
            data = load_json(p, {})
            gs: list = data.get("gs", []) if isinstance(data, dict) else []
            return [r for r in gs if isinstance(r, dict) and r.get("hash") and r.get("files")], p
    return [], None


def load(path: str) -> dict:
    return tidy(load_json(path, None))


def save(path: str, data) -> None:
    tmp: str = path + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline="\n") as f:
        json.dump(data, f, indent=1)
        f.write("\n")
    os.replace(tmp, path)


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[0] != "export":
        print(__doc__, file=sys.stderr)
        return 2
    gs, where = load_list()
    if not gs:
        print("no Gold Skulltula inventory (cache/gs-list.json or $OOTRR_GS_LIST)", file=sys.stderr)
        return 1
    out, notes = export(load(os.path.join(argv[1], "notes", FILE_NAME)), gs)
    save(argv[2], out)
    for n in notes:
        print("note:", n, file=sys.stderr)
    print(f"{len(out['entries'])} entr(ies), {len(out['skipped'])} skipped, written to {argv[2]}; copy it to data/gs_positions.json in the gs-relocate worktree", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
