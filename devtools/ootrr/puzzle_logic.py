"""Timed silver rupee puzzles marked from the phone panel's Puzzle card: pure data functions, no game, no server.

The marks live in SESSION/notes/panel_puzzles.json: {"high": N, "current": id or null, "puzzles": [puzzle, ...]}.
A puzzle is {id, map, seconds, switch, chest, counter, wall_a, wall_b, gems, time}; every mark is {n, pos, facing, time}
(Link's feet and facing in degrees), gems a list of marks. Mark numbers are never reused.

The export matches the Room rows of randomizer/core/src/difficulty/gems.rs (branch feature/timed-gems): switch at the mark,
switch_rotate [0, facing in radians, 0], each rupee at Link's feet plus GEM_RISE, seconds 10 to 300, 1 to 10 rupees, a chest
or a wall. The chest row wants the chest actor's hash, which only the game files hold: the export gives the marked spot and
leaves chest null for the converter or a person to fill (the nearest Obj_TBox in that map).

    python -I puzzle_logic.py export SESSION OUT.json   writes the export and prints the Rust rows
"""
import json
import math
import os
import sys

FILE_NAME: str = "panel_puzzles.json"
EXPORT_NAME: str = "timed-gems.json"
PUZZLE_NEAR: float = 60.0
LEAST: float = 10.0
MOST: float = 300.0
FEWEST: int = 1
MOST_GEMS: int = 10
DEFAULT_SECONDS: float = 20.0
GEM_RISE: float = 0.5
COUNTER_RISE: float = 3.0
RUN: float = 6.0
SINGLE: tuple[str, ...] = ("switch", "chest", "counter", "wall_a", "wall_b")
KINDS: tuple[str, ...] = SINGLE + ("gem",)
LABELS: dict[str, str] = {"switch": "Switch", "chest": "Chest", "counter": "Counter", "wall_a": "Wall A", "wall_b": "Wall B", "gem": "Rupee"}


def dist(a: list[float], b: list[float]) -> float:
    return math.sqrt(sum((float(a[i]) - float(b[i])) ** 2 for i in range(3)))


def empty() -> dict:
    return {"high": 0, "current": None, "puzzles": []}


def tidy(state: dict | list | None) -> dict:
    if not isinstance(state, dict):
        return empty()
    out: dict = {"high": int(state.get("high", 0)), "current": state.get("current"), "puzzles": list(state.get("puzzles", []))}
    for p in out["puzzles"]:
        p.setdefault("gems", [])
        p.setdefault("seconds", DEFAULT_SECONDS)
        for k in SINGLE:
            p.setdefault(k, None)
    return out


def marks_of(p: dict) -> list[tuple[str, dict]]:
    out: list[tuple[str, dict]] = [(k, p[k]) for k in SINGLE if p.get(k)]
    return out + [("gem", g) for g in p.get("gems", [])]


def puzzle_dist(p: dict, pos: list[float]) -> float:
    ms: list[tuple[str, dict]] = marks_of(p)
    return min(dist(m["pos"], pos) for _, m in ms) if ms else 1e9


def on_map(state: dict, map_name: str) -> list[dict]:
    return [p for p in state["puzzles"] if p.get("map") == map_name]


def find(state: dict, pid: object) -> dict | None:
    for p in state["puzzles"]:
        if p.get("id") == pid:
            return p
    return None


def active(state: dict, map_name: str, pos: list[float], radius: float = PUZZLE_NEAR) -> dict | None:
    cur: dict | None = find(state, state.get("current"))
    if cur is not None and cur.get("map") == map_name and (not marks_of(cur) or puzzle_dist(cur, pos) <= radius):
        return cur
    here: list[dict] = [p for p in on_map(state, map_name) if puzzle_dist(p, pos) <= radius]
    return min(here, key=lambda p: puzzle_dist(p, pos)) if here else None


def next_id(state: dict) -> int:
    return max([int(p.get("id", 0)) for p in state["puzzles"]] + [0]) + 1


def new_puzzle(state: dict, map_name: str, when: str = "") -> dict:
    p: dict = {"id": next_id(state), "map": map_name, "seconds": DEFAULT_SECONDS, "gems": [], "time": when}
    for k in SINGLE:
        p[k] = None
    state["puzzles"].append(p)
    state["current"] = p["id"]
    return p


def drop_empty(state: dict) -> None:
    state["puzzles"] = [p for p in state["puzzles"] if marks_of(p) or p.get("id") == state.get("current")]


def clamp_seconds(s: float) -> float:
    return float(max(LEAST, min(MOST, round(float(s)))))


def set_seconds(p: dict, s: float) -> float:
    p["seconds"] = clamp_seconds(s)
    return p["seconds"]


def mark(state: dict, map_name: str, pos: list[float], facing: float, kind: str, when: str = "") -> tuple[dict, str, dict]:
    if kind not in KINDS:
        raise ValueError(f"unknown mark kind {kind}")
    p: dict | None = active(state, map_name, pos)
    if p is None:
        p = new_puzzle(state, map_name, when)
    if kind == "gem" and len(p["gems"]) >= MOST_GEMS:
        raise ValueError(f"puzzle {p['id']} already has {MOST_GEMS} rupees, the most a room takes")
    state["high"] = int(state.get("high", 0)) + 1
    m: dict = {"n": state["high"], "pos": [round(float(v), 2) for v in pos], "facing": round(float(facing or 0.0), 1), "time": when}
    state["current"] = p["id"]
    if kind == "gem":
        p["gems"].append(m)
        return p, "placed", m
    how: str = "moved" if p.get(kind) else "placed"
    p[kind] = m
    return p, how, m


def label_of(kind: str, p: dict, m: dict) -> str:
    if kind == "gem":
        i: int = next((i for i, g in enumerate(p["gems"]) if g["n"] == m["n"]), 0)
        return f"Rupee {i + 1}"
    if kind == "switch":
        return f"Switch {int(p.get('seconds', DEFAULT_SECONDS))}s"
    return LABELS[kind]


def nearest_mark(state: dict, map_name: str, pos: list[float]) -> tuple[dict, str, dict] | None:
    best: tuple[float, dict, str, dict] | None = None
    for p in on_map(state, map_name):
        for kind, m in marks_of(p):
            d: float = dist(m["pos"], pos)
            if best is None or d < best[0]:
                best = (d, p, kind, m)
    return (best[1], best[2], best[3]) if best else None


def remove_mark(state: dict, n: int) -> tuple[dict, str, dict] | None:
    for p in state["puzzles"]:
        for kind, m in marks_of(p):
            if m["n"] == n:
                if kind == "gem":
                    p["gems"] = [g for g in p["gems"] if g["n"] != n]
                else:
                    p[kind] = None
                drop_empty(state)
                return p, kind, m
    return None


def gem_spots(p: dict) -> list[list[float]]:
    return [[g["pos"][0], round(float(g["pos"][1]) + GEM_RISE, 2), g["pos"][2]] for g in p.get("gems", [])]


def counter_spot(p: dict) -> tuple[list[float] | None, bool]:
    if p.get("counter"):
        return list(p["counter"]["pos"]), False
    gs: list[list[float]] = gem_spots(p)
    if not gs:
        return None, True
    return [round(sum(g[i] for g in gs) / len(gs), 2) if i != 1 else round(max(g[1] for g in gs) + COUNTER_RISE, 2) for i in range(3)], True


def route(p: dict) -> float:
    if not p.get("switch"):
        return 0.0
    at: list[float] = p["switch"]["pos"]
    left: list[list[float]] = [g["pos"] for g in p.get("gems", [])]
    total: float = 0.0
    while left:
        nxt: list[float] = min(left, key=lambda g: dist(at, g))
        total += dist(at, nxt)
        at = nxt
        left.remove(nxt)
    goal: dict | None = p.get("chest")
    return total + (dist(at, goal["pos"]) if goal else 0.0)


def problems(p: dict) -> list[str]:
    out: list[str] = []
    if not p.get("switch"):
        out.append("no switch yet")
    n: int = len(p.get("gems", []))
    if not FEWEST <= n <= MOST_GEMS:
        out.append(f"{n} rupees: a room takes {FEWEST} to {MOST_GEMS}")
    s: float = float(p.get("seconds", DEFAULT_SECONDS))
    if not LEAST <= s <= MOST:
        out.append(f"{s:g} s is outside {LEAST:g} to {MOST:g}")
    if bool(p.get("wall_a")) != bool(p.get("wall_b")):
        out.append("the wall needs both ends")
    if not p.get("chest") and p.get("chest_hash") is None and not (p.get("wall_a") and p.get("wall_b")):
        out.append("no chest or wall: the puzzle guards nothing")
    if p.get("switch") and n and s < route(p) / RUN:
        out.append(f"{s:g} s is under a straight run ({route(p):.0f} m at {RUN:g} m/s = {route(p) / RUN:.0f} s)")
    return out


def hints(p: dict) -> list[str]:
    out: list[str] = []
    if p.get("chest") and p.get("chest_hash") is None:
        c: list[float] = p["chest"]["pos"]
        out.append(f"chest: fill in the hash of the chest nearest [{c[0]:.2f}, {c[1]:.2f}, {c[2]:.2f}] in {p['map']}")
    if not p.get("counter") and p.get("gems"):
        out.append("counter: no spot marked, placed above the rupees' middle")
    return out


def room_row(p: dict, place: str = "") -> dict:
    sw: dict | None = p.get("switch")
    counter, default = counter_spot(p)
    guard: dict = {"kind": "wall", "a": p["wall_a"]["pos"], "b": p["wall_b"]["pos"]} if p.get("wall_a") and p.get("wall_b") else {"kind": "none"}
    return {"id": int(p["id"]), "name": p.get("name") or (f"timed rupees {p['id']}" + (f" in {place}" if place else "")), "map": p["map"],
            "switch": list(sw["pos"]) if sw else None,
            "switch_rotate": [0.0, round(math.radians(float(sw.get("facing") or 0.0)), 3), 0.0] if sw else None,
            "gems": gem_spots(p), "rupees": {str(i + 1): list(g["pos"]) for i, g in enumerate(p.get("gems", []))},
            "counter": counter, "counter_default": default, "seconds": clamp_seconds(p.get("seconds", DEFAULT_SECONDS)),
            "guard": guard, "chest": None if p.get("chest_hash") is None else str(p["chest_hash"]), "chest_pos": list(p["chest"]["pos"]) if p.get("chest") else None, "room": None,
            "problems": problems(p), "hints": hints(p)}


def num(v: float) -> str:
    s: str = repr(round(float(v), 3))
    return s if "." in s or "e" in s else s + ".0"


def vec(v: list[float] | None) -> str:
    return "[" + ", ".join(num(x) for x in (v or [0.0, 0.0, 0.0])) + "]"


def rust_row(r: dict) -> str:
    g: dict = r["guard"]
    guard: str = f"Guard::Wall {{ a: {vec(g['a'])}, b: {vec(g['b'])} }}" if g["kind"] == "wall" else "Guard::None"
    lines: list[str] = ["Room {", f"    id: {r['id']},", f"    name: \"{r['name'].replace(chr(34), chr(39))}\",", f"    map: \"{r['map']}\",",
                        f"    switch: {vec(r['switch'])},", f"    switch_rotate: {vec(r['switch_rotate'])},",
                        "    gems: &[" + ", ".join(vec(x) for x in r["gems"]) + "],", f"    counter: {vec(r['counter'])},",
                        f"    seconds: {num(r['seconds'])},", f"    guard: {guard},", f"    chest: {'None' if r['chest'] is None else 'Some(%s)' % r['chest']},", "    room: None,", "}"]
    return "\n".join(lines)


def export(state: dict, place_of=lambda m: "") -> dict:
    rooms: list[dict] = [room_row(p, place_of(p["map"])) for p in sorted(state["puzzles"], key=lambda x: int(x["id"])) if marks_of(p)]
    rust: str = "pub const ROOMS: [Room; %d] = [\n" % len(rooms) + "".join("\n".join("    " + x for x in rust_row(r).split("\n")) + ",\n" for r in rooms) + "];\n"
    return {"format": "ootrr-timed-gems", "version": 1, "note": "exported from the panel's Puzzle card (puzzle_logic.py); fields follow Room in gems.rs",
            "rooms": rooms, "rust": rust}


def merge_shipped(state: dict, shipped: list[dict]) -> int:
    n: int = 0
    for r in shipped:
        if any(p.get("shipped") == r["shipped"] and p.get("map") == r["map"] for p in state["puzzles"]):
            continue
        p: dict = {k: v for k, v in r.items()}
        p["id"] = next_id(state)
        p["time"] = "mod"
        for kind in SINGLE:
            if p.get(kind):
                state["high"] = int(state.get("high", 0)) + 1
                p[kind] = dict(p[kind], n=state["high"])
        gems: list[dict] = []
        for g in p.get("gems", []):
            state["high"] = int(state.get("high", 0)) + 1
            gems.append(dict(g, n=state["high"]))
        p["gems"] = gems
        state["puzzles"].append(p)
        n += 1
    return n


def warp_list(state: dict) -> list[dict]:
    out: list[dict] = []
    for p in sorted(state["puzzles"], key=lambda x: int(x["id"])):
        if p.get("warp"):
            out.append({"id": p["id"], "map": p["map"], "pos": list(p["warp"]), "how": "measured",
                        "label": f"Puzzle {p['id']}: beside the switch" + (" (the mod's)" if p.get("shipped") else "")})
        elif p.get("switch"):
            out.append({"id": p["id"], "map": p["map"], "pos": list(p["switch"]["pos"]), "how": "switch mark",
                        "label": f"Puzzle {p['id']}: on the switch mark"})
        elif p.get("gems"):
            out.append({"id": p["id"], "map": p["map"], "pos": list(p["gems"][0]["pos"]), "how": "rupee mark", "label": f"Puzzle {p['id']}: rupee 1"})
    return out


def radar_lines(state: dict, map_name: str, near_n: int | None = None) -> list[str]:
    lines: list[str] = []
    for p in on_map(state, map_name):
        for kind, m in marks_of(p):
            x, y, z = (float(v) for v in m["pos"])
            lab: str = f"P{p['id']} " + label_of(kind, p, m)
            lines.append(f"p {x:.2f} {y:.2f} {z:.2f} " + (">" + lab if m["n"] == near_n else lab)[:15])
    return lines


def load(path: str) -> dict:
    try:
        with open(path, encoding="utf-8-sig") as f:
            return tidy(json.load(f))
    except (OSError, ValueError):
        return empty()


def save(path: str, data) -> None:
    tmp: str = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1)
    os.replace(tmp, path)


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[0] != "export":
        print(__doc__, file=sys.stderr)
        return 2
    state: dict = load(os.path.join(argv[1], "notes", FILE_NAME))
    out: dict = export(state)
    save(argv[2], out)
    print(out["rust"])
    for r in out["rooms"]:
        for line in r["problems"]:
            print(f"room {r['id']}: {line}", file=sys.stderr)
        for line in r["hints"]:
            print(f"room {r['id']}: {line}", file=sys.stderr)
    return 1 if any(r["problems"] for r in out["rooms"]) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
