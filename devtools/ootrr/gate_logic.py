"""Gates placed from the phone panel's Manage Gate card: pure data functions, no game, no server.

Gates live in SESSION/notes/panel_gates.json, a list of {id, map, left, right, guards, time}: left and right are [x, y, z] or None,
guards are placer point numbers (the n of SESSION/notes/spawnpoints.json). A point guards at most one gate.
The gates the mod ships (gates.rs, read by shipped_gates.py) are merged in with "shipped": true and their guard "spots"; one the
card edits is saved to the file in full (same id) and exported as an edit, one left alone is never saved or exported.

    python -I gate_logic.py export SESSION OUT.json   writes the gates as an ootrr-gate-session file (the Longshot gate editor's
                                                     gates.json format), which the gate tooling (json_to_gates.py, outside devtools) turns into
                                                     gates.rs rows.
"""
import json
import math
import os
import sys

GATE_NEAR: float = 10.0
PAIR_REACH: float = 60.0
GUARD_REACH: float = 30.0
SIDES: tuple[str, ...] = ("left", "right")
FILE_NAME: str = "panel_gates.json"


def dist(a: list[float], b: list[float]) -> float:
    return math.sqrt(sum((float(a[i]) - float(b[i])) ** 2 for i in range(3)))


def seg_dist(p: list[float], a: list[float], b: list[float]) -> float:
    d: list[float] = [float(b[i]) - float(a[i]) for i in range(3)]
    dd: float = sum(v * v for v in d)
    if dd <= 1e-9:
        return dist(p, a)
    t: float = max(0.0, min(1.0, sum((float(p[i]) - float(a[i])) * d[i] for i in range(3)) / dd))
    return dist(p, [float(a[i]) + d[i] * t for i in range(3)])


def posts(g: dict) -> list[list[float]]:
    return [g[s] for s in SIDES if g.get(s)]


def complete(g: dict) -> bool:
    return bool(g.get("left")) and bool(g.get("right"))


def mid(g: dict) -> list[float] | None:
    ps: list[list[float]] = posts(g)
    if not ps:
        return None
    return [round(sum(float(p[i]) for p in ps) / len(ps), 2) for i in range(3)]


def gate_dist(g: dict, pos: list[float]) -> float:
    if complete(g):
        return seg_dist(pos, g["left"], g["right"])
    ps: list[list[float]] = posts(g)
    return min(dist(p, pos) for p in ps) if ps else 1e9


def on_map(gates: list[dict], map_name: str) -> list[dict]:
    return [g for g in gates if g.get("map") == map_name]


STICK_EXTRA: float = 30.0


def guard_spots(g: dict, points: list[dict]) -> list[list[float]]:
    out: list[list[float]] = []
    for n in g.get("guards", []):
        p: dict | None = point_on(points, n)
        if p and p.get("map") == g.get("map"):
            out.append([float(v) for v in p["pos"]])
    if not out:
        out = [[float(v) for v in s] for s in g.get("spots", [])]
    return out


def area_dist(g: dict, pos: list[float], points: list[dict]) -> float:
    """metres from pos to the gate's whole area: its wall (or posts), and the path from the wall's middle to each guard, so a
    spot beside a guard, or the walk between the two, still counts as the gate"""
    best: float = gate_dist(g, pos)
    m: list[float] | None = mid(g)
    for s in guard_spots(g, points):
        best = min(best, dist(pos, s))
        if m is not None:
            best = min(best, seg_dist(pos, m, s))
    return best


def select_gate(gates: list[dict], map_name: str, pos: list[float], points: list[dict], sticky: int | None = None,
                pinned: int | None = None, radius: float = GATE_NEAR) -> tuple[dict | None, str]:
    """The gate the card works on: the one picked by hand (pinned) while it is on this map; else the gate chosen last time while Link
    is still inside its area plus STICK_EXTRA (unless Link is inside another gate's area and nearer to it); else the gate whose area is nearest, if Link is within `radius` of it"""
    here: list[dict] = [g for g in on_map(gates, map_name) if posts(g)]
    if pinned is not None:
        g: dict | None = next((x for x in here if x.get("id") == pinned), None)
        if g is not None:
            return g, "pinned"
    best: dict | None = min(here, key=lambda x: area_dist(x, pos, points)) if here else None
    held: dict | None = next((x for x in here if x.get("id") == sticky), None) if sticky is not None else None
    if held is not None and area_dist(held, pos, points) <= radius + STICK_EXTRA:
        if best is not None and best is not held and area_dist(best, pos, points) <= radius and area_dist(best, pos, points) < area_dist(held, pos, points):
            return best, "auto"
        return held, "auto"
    if best is not None and area_dist(best, pos, points) <= radius:
        return best, "auto"
    return None, ""


def nearest_gate(gates: list[dict], map_name: str, pos: list[float], radius: float | None = GATE_NEAR) -> dict | None:
    here: list[dict] = [g for g in on_map(gates, map_name) if posts(g)]
    if not here:
        return None
    best: dict = min(here, key=lambda g: gate_dist(g, pos))
    if radius is not None and gate_dist(best, pos) > radius:
        return None
    return best


def pending_gate(gates: list[dict], map_name: str, pos: list[float], side: str, reach: float = PAIR_REACH) -> dict | None:
    waiting: list[dict] = [g for g in on_map(gates, map_name) if not g.get(side) and posts(g) and gate_dist(g, pos) <= reach]
    return min(waiting, key=lambda g: gate_dist(g, pos)) if waiting else None


def find(gates: list[dict], gate_id: int) -> dict | None:
    for g in gates:
        if g.get("id") == gate_id:
            return g
    return None


def next_id(gates: list[dict]) -> int:
    return max([int(g.get("id", 0)) for g in gates] + [0]) + 1


def guard_of(gates: list[dict], n: int) -> dict | None:
    for g in gates:
        if n in g.get("guards", []):
            return g
    return None


def place_post(gates: list[dict], map_name: str, pos: list[float], side: str, when: str = "") -> tuple[dict, str]:
    if side not in SIDES:
        raise ValueError("side must be left or right")
    spot: list[float] = [round(float(v), 2) for v in pos]
    g: dict | None = nearest_gate(gates, map_name, spot) or pending_gate(gates, map_name, spot, side)
    if g is None:
        g = {"id": next_id(gates), "map": map_name, "left": None, "right": None, "guards": [], "time": when}
        gates.append(g)
        g[side] = spot
        return g, "new"
    how: str = "moved" if g.get(side) else "placed"
    g[side] = spot
    return g, how


def place_preview(gates: list[dict], map_name: str, pos: list[float], side: str) -> dict:
    g: dict | None = nearest_gate(gates, map_name, pos) or pending_gate(gates, map_name, pos, side)
    if g is None:
        return {"how": "new", "gate": next_id(gates), "dist": None}
    return {"how": "moved" if g.get(side) else "placed", "gate": g["id"], "dist": round(dist(g[side], pos), 1) if g.get(side) else round(gate_dist(g, pos), 1)}


def remove_gate(gates: list[dict], gate_id: int) -> list[dict]:
    return [g for g in gates if g.get("id") != gate_id]


def point_on(points: list[dict], n: int) -> dict | None:
    for p in points:
        if p.get("n") == n:
            return p
    return None


def mark_target(gate: dict | None, points: list[dict], map_name: str, pos: list[float], reach: float = GUARD_REACH) -> dict | None:
    if gate is None:
        return None
    mine: set = set(gate.get("guards", []))
    here: list[dict] = [p for p in points if p.get("map") == map_name and p.get("n") not in mine and dist(p["pos"], pos) <= reach]
    return min(here, key=lambda p: dist(p["pos"], pos)) if here else None


def unmark_target(gate: dict | None, points: list[dict], pos: list[float]) -> dict | None:
    if gate is None:
        return None
    mine: list[dict] = [p for p in (point_on(points, n) for n in gate.get("guards", [])) if p]
    return min(mine, key=lambda p: dist(p["pos"], pos)) if mine else None


GUARD_DELETE_REACH: float = 8.0


def guard_entries(g: dict, points: list[dict], pos: list[float] | None = None) -> list[dict]:
    """the guards the overlay draws for this gate: its marked points, or (when none are marked) the mod's own spots; each row has
    source "marked" (n = placer point) or "shipped" (i = index in the gate's spots)"""
    out: list[dict] = []
    for n in g.get("guards", []):
        p: dict | None = point_on(points, n)
        if p and p.get("map") == g.get("map"):
            out.append({"source": "marked", "n": n, "i": None, "name": p["name"], "label": short(p["name"]), "pos": [float(v) for v in p["pos"]]})
    if not out:
        for i, s in enumerate(g.get("spots", [])):
            k: str = g.get("kind") or "vanilla guard"
            out.append({"source": "shipped", "n": None, "i": i, "name": k, "label": guard_label(g), "pos": [float(v) for v in s]})
    if pos is not None:
        for e in out:
            e["dist"] = round(dist(e["pos"], pos), 1)
        out.sort(key=lambda e: e["dist"])
    return out


def guard_pick(g: dict | None, points: list[dict], pos: list[float], reach: float = GUARD_DELETE_REACH) -> dict | None:
    if g is None:
        return None
    rows: list[dict] = guard_entries(g, points, pos)
    return rows[0] if rows and rows[0]["dist"] <= reach else None


def remove_spot(gates: list[dict], gate_id: int, i: int) -> bool:
    g: dict | None = find(gates, gate_id)
    if g is None or not isinstance(i, int) or i < 0 or i >= len(g.get("spots", [])):
        return False
    g["spots"] = [s for k, s in enumerate(g["spots"]) if k != i]
    return True


def mark_guard(gates: list[dict], gate_id: int, n: int) -> dict | None:
    old: dict | None = guard_of(gates, n)
    if old is not None and old.get("id") != gate_id:
        old["guards"] = [x for x in old["guards"] if x != n]
    g: dict | None = find(gates, gate_id)
    if g is not None and n not in g.setdefault("guards", []):
        g["guards"].append(n)
    return old if old is not None and old.get("id") != gate_id else None


def unmark_guard(gates: list[dict], gate_id: int, n: int) -> bool:
    g: dict | None = find(gates, gate_id)
    if g is None or n not in g.get("guards", []):
        return False
    g["guards"] = [x for x in g["guards"] if x != n]
    return True


def prune(gates: list[dict], points: list[dict]) -> bool:
    known: dict[int, str] = {int(p["n"]): p.get("map", "") for p in points if "n" in p}
    changed: bool = False
    for g in gates:
        keep: list[int] = [n for n in g.get("guards", []) if known.get(n) == g.get("map")]
        if keep != g.get("guards", []):
            g["guards"] = keep
            changed = True
    return changed


def short(name: str) -> str:
    return name.replace("Enemy_", "").replace("MiddleBoss_", "")


VANILLA: dict = {}
OWN: dict = {}
OWN_REACH: float = 4.0
NICE: dict[str, str] = {"Dekubaba": "Deku Baba", "Iron Knuckle": "Iron Knuckle"}


def nice(name: str) -> str:
    out: str = short(name).replace("_", " ")
    for k, v in NICE.items():
        out = out.replace(k, v)
    return out


def own_kind(g: dict, live: list[dict] | None = None) -> str | None:
    key: tuple = (g.get("id"), g.get("map"))
    if key in OWN and OWN[key]:
        return OWN[key]
    spots: list = g.get("spots", [])
    found: str | None = None
    if spots and VANILLA:
        name: str | None = next((k for k in (g.get("map"), str(g.get("map", "")).rsplit("_", 1)[0]) if k in VANILLA), None)
        best: float = OWN_REACH
        for e in (VANILLA.get(name) or {}).get("enemies", []) if name else []:
            for s in spots:
                d: float = math.hypot(float(e["p"][0]) - float(s[0]), float(e["p"][2]) - float(s[2]))
                if d <= best and abs(float(e["p"][1]) - float(s[1])) <= 3.0:
                    best, found = d, str(e["n"])
    if found is None and live:
        best = OWN_REACH
        for e in live:
            for s in spots:
                d = math.hypot(float(e["x"]) - float(s[0]), float(e["z"]) - float(s[2]))
                if d <= best and str(e.get("name", "")).startswith("Enemy_"):
                    best, found = d, str(e["name"])
    if found:
        OWN[key] = found
    return found


def guard_label(g: dict, full: bool = True, live: list[dict] | None = None) -> str:
    if g.get("kind"):
        return short(g["kind"])
    o: str | None = own_kind(g, live)
    if o:
        n: str = nice(o)
        if full:
            return n.replace(" Withered", "") + " (the game's own)" if o.endswith("_Withered") else n + " (the game's own)"
        return n if len(n) <= 11 or " " not in n else " ".join(n.split()[:-1])
    return "vanilla guard" if full else "vanilla"


def original(shipped: list[dict], gate_id: int) -> dict | None:
    for s in shipped:
        if s.get("id") == gate_id:
            return s
    return None


def same(a: dict, b: dict) -> bool:
    return all(a.get(k) == b.get(k) for k in ("map", "left", "right", "guards", "spots"))


def edited(g: dict, shipped: list[dict]) -> bool:
    if not g.get("shipped"):
        return False
    o: dict | None = original(shipped, g["id"])
    return o is None or not same(g, o)


def fresh_copy(g: dict) -> dict:
    return json.loads(json.dumps(g))


def merge(saved: list[dict], shipped: list[dict]) -> list[dict]:
    edits: dict[int, dict] = {int(g["id"]): g for g in saved if g.get("shipped")}
    out: list[dict] = [edits.pop(int(s["id"]), None) or fresh_copy(s) for s in shipped]
    out += list(edits.values())
    for g in [g for g in saved if not g.get("shipped")]:
        if any(int(x["id"]) == int(g.get("id", 0)) for x in out):
            g["id"] = next_id(out + [g])
        out.append(g)
    return out


def to_save(gates: list[dict], shipped: list[dict]) -> list[dict]:
    return [g for g in gates if not g.get("shipped") or edited(g, shipped)]


def revert(gates: list[dict], shipped: list[dict], gate_id: int) -> bool:
    o: dict | None = original(shipped, gate_id)
    if o is None:
        return False
    for i, g in enumerate(gates):
        if g.get("id") == gate_id:
            gates[i] = fresh_copy(o)
            return True
    return False


def tag(text: str) -> str:
    return text[:15]


def radar_lines(gates: list[dict], map_name: str, spots: bool = True) -> list[str]:
    lines: list[str] = []
    for g in on_map(gates, map_name):
        for side, t in (("left", "L"), ("right", "R")):
            p = g.get(side)
            if p:
                lines.append(f"p {float(p[0]):.2f} {float(p[1]):.2f} {float(p[2]):.2f} " + tag(f"Gate {g['id']} {t}"))
        for i, wall in enumerate(g.get("also", [])):
            for p, t in zip(wall, ("L", "R")):
                lines.append(f"p {float(p[0]):.2f} {float(p[1]):.2f} {float(p[2]):.2f} " + tag(f"Gate {g['id']} {t}{i + 2}"))
        for p in g.get("spots", []) if spots and not g.get("guards") else []:
            lines.append(f"p {float(p[0]):.2f} {float(p[1]):.2f} {float(p[2]):.2f} " + tag(f"G{g['id']} {guard_label(g, False)}"))
    return lines


WALL_HEIGHT: float = 2.5
WALL_THICK: float = 0.2
GUARD_BOX: list[float] = [-0.5, 0.0, -0.5, 0.5, 1.8, 0.5]


def box_part(b: list) -> str:
    flat: list = []
    for v in b:
        if isinstance(v, (list, tuple)):
            flat.extend(v)
        else:
            flat.append(v)
    return "[" + " ".join(f"{float(v):.2f}" for v in flat) + "]"


def wall_line(a: list[float], b: list[float], text: str) -> str:
    dx: float = float(b[0]) - float(a[0])
    dz: float = float(b[2]) - float(a[2])
    width: float = math.hypot(dx, dz)
    yaw: float = math.degrees(math.atan2(-dz, dx)) + 0.0
    low: float = min(float(a[1]), float(b[1]))
    rise: float = abs(float(a[1]) - float(b[1]))
    cx: float = (float(a[0]) + float(b[0])) / 2.0
    cz: float = (float(a[2]) + float(b[2])) / 2.0
    return f"P {cx:.2f} {low:.2f} {cz:.2f} {yaw:.1f} " + box_part([-width / 2.0, 0.0, -WALL_THICK, width / 2.0, WALL_HEIGHT + rise, WALL_THICK]) + " " + tag(text)


def box_lines(gates: list[dict], map_name: str, boxes: dict[str, list] | None = None) -> list[str]:
    """Wire boxes for the in-game radar: each wall as a thin tall box, and a box at each of a shipped gate's own guard spots (the
    guard points the card marks are drawn by the placer's marks, see radar_marks box_ns)"""
    lines: list[str] = []
    for g in on_map(gates, map_name):
        if complete(g):
            lines.append(wall_line(g["left"], g["right"], f"Gate {g['id']}"))
        for i, wall in enumerate(g.get("also", [])):
            lines.append(wall_line(wall[0], wall[1], f"Gate {g['id']} w{i + 2}"))
        if g.get("guards") or not g.get("spots"):
            continue
        m: list[float] | None = mid(g)
        b: list[float] = list((boxes or {}).get(g.get("kind") or own_kind(g) or "") or GUARD_BOX)
        for s in g["spots"]:
            yaw: float = math.degrees(math.atan2(m[0] - float(s[0]), m[2] - float(s[2]))) if m else 0.0
            lines.append(f"P {float(s[0]):.2f} {float(s[1]):.2f} {float(s[2]):.2f} {yaw:.1f} {box_part(b)} " + tag(f"G{g['id']} {guard_label(g, False)}"))
    return lines


def guard_ns(gates: list[dict], map_name: str) -> set[int]:
    return {int(n) for g in on_map(gates, map_name) for n in g.get("guards", [])}


def point_deg(p: dict) -> float:
    return float(p["yaw"]) if "yaw" in p else float(p.get("facing") or 0.0) + 180.0


STAND_IN: dict[str, str] = {"Enemy_Lizalfos": "Enemy_MiddleBoss_Lizalfos"}


def model_kind(kind: str, no_model: tuple, refused: set) -> str | None:
    k: str = STAND_IN.get(kind, kind) if kind in no_model else kind
    return None if k in refused or (k in no_model and k == kind) else k


def placeholder_lines(gates: list[dict], points: list[dict], map_name: str, no_model: tuple = (), refused: set = frozenset()) -> list[str]:
    """The display-model list (placeholders set) for the gates' guards: the marked guard points, and a shipped gate's own spots"""
    lines: list[str] = []
    for g in on_map(gates, map_name):
        m: list[float] | None = mid(g)
        for n in g.get("guards", []):
            p: dict | None = point_on(points, n)
            k: str | None = model_kind(str(p.get("name", "")), no_model, refused) if p else None
            if not p or not str(p.get("name", "")).startswith("Enemy_") or k is None:
                continue
            x, y, z = (float(v) for v in p["pos"])
            lines.append(f"{k} {x:.2f} {y:.2f} {z:.2f} {point_deg(p):.1f}")
        gk: str | None = model_kind(g.get("kind", ""), no_model, refused)
        if not g.get("guards") and g.get("kind", "").startswith("Enemy_") and gk is not None:
            for s in g.get("spots", []):
                yaw: float = math.degrees(math.atan2(m[0] - float(s[0]), m[2] - float(s[2]))) if m else 0.0
                lines.append(f"{gk} {float(s[0]):.2f} {float(s[1]):.2f} {float(s[2]):.2f} {yaw:.1f}")
    return lines


def gate_label(g: dict) -> str:
    where: str = g.get("name") or g.get("label") or ""
    return f"Gate {g['id']}" + (f": {where}" if where else "")


def warp_spot(g: dict, measured: dict[int, list[float]]) -> tuple[list[float] | None, str]:
    if int(g["id"]) in measured and (g.get("shipped") or not posts(g)):
        return [float(v) for v in measured[int(g["id"])]], "measured"
    if g.get("spots"):
        return [float(v) for v in g["spots"][0]], "guard spot"
    ps: list[list[float]] = posts(g)
    if ps:
        return [float(v) for v in ps[0]], "post"
    return None, ""


def warp_list(gates: list[dict], measured: dict[int, list[float]]) -> list[dict]:
    out: list[dict] = []
    for g in sorted(gates, key=lambda x: int(x.get("id", 0))):
        pos, how = warp_spot(g, measured)
        if pos is None:
            continue
        out.append({"id": int(g["id"]), "map": g.get("map", ""), "label": gate_label(g), "pos": pos, "how": how,
                    "age": age_of(g.get("map", "")), "shipped": bool(g.get("shipped"))})
    return out


def measured_warps(places: list[dict]) -> dict[int, list[float]]:
    out: dict[int, list[float]] = {}
    for p in places if isinstance(places, list) else []:
        lab: str = str(p.get("label", ""))
        if lab.startswith("Gate ") and p.get("warps"):
            num: str = lab[5:].split(":")[0].strip()
            if num.isdigit():
                out[int(num)] = list(p["warps"][0])
    return out


def guard_tags(gates: list[dict], map_name: str) -> dict[int, str]:
    out: dict[int, str] = {}
    for g in on_map(gates, map_name):
        for n in g.get("guards", []):
            out[int(n)] = f" G{g['id']}"
    return out


def age_of(map_name: str) -> str:
    if map_name.endswith("_Child"):
        return "child"
    if map_name.endswith("_Adult"):
        return "adult"
    return ""


def barrier(a: list[float], b: list[float], toward: list[float] | None) -> dict:
    a = [float(v) for v in a]
    b = [float(v) for v in b]
    dx: float = b[0] - a[0]
    dz: float = b[2] - a[2]
    yaw: float = math.atan2(-dz, dx)
    centre: list[float] = [(a[i] + b[i]) / 2.0 for i in range(3)]
    if toward is not None and math.sin(yaw) * (toward[0] - centre[0]) + math.cos(yaw) * (toward[2] - centre[2]) < 0.0:
        a, b = b, a
        yaw = math.atan2(dz, -dx)
    width: float = math.hypot(dx, dz)
    return {"centre": [round(v, 2) for v in centre], "yaw": round(yaw, 4), "width": round(width, 2),
            "a": [round(v, 2) for v in a], "b": [round(v, 2) for v in b]}


def export_session(gates: list[dict], points: list[dict], shipped: list[dict] | None = None) -> tuple[dict, list[str]]:
    out: list[dict] = []
    notes: list[str] = []
    for g in sorted(gates, key=lambda x: int(x.get("id", 0))):
        tag: str = f"gate {g.get('id')} ({g.get('map')})"
        if g.get("shipped") and not edited(g, shipped or []):
            continue
        if not complete(g):
            notes.append(f"{tag}: skipped, it has only one post")
            continue
        guards: list[dict] = [p for p in (point_on(points, n) for n in g.get("guards", [])) if p and p.get("map") == g.get("map")]
        if not guards and g.get("shipped") and g.get("kind"):
            guards = [{"name": g["kind"], "pos": s, "n": None} for s in g.get("spots", [])]
            notes.append(f"{tag}: an edit of the shipped gate; its guards are gates.rs's {len(guards)} {short(g['kind'])} spot(s)")
        if not guards:
            notes.append(f"{tag}: skipped, no guard marked")
            continue
        toward: list[float] = [sum(float(p["pos"][i]) for p in guards) / len(guards) for i in range(3)]
        bar: dict = barrier(g["left"], g["right"], toward)
        c: list[float] = bar["centre"]
        rows: list[dict] = [{"kind": p["name"], "pos": [round(float(v), 2) for v in p["pos"]],
                             "yaw": round(math.atan2(c[0] - float(p["pos"][0]), c[2] - float(p["pos"][2])), 4), "n": p["n"]} for p in guards]
        label: str = g.get("label") or (f"Gate {g['id']} (edit of gates.rs gate {g['id']}: {g.get('name', '')})" if g.get("shipped") else f"Gate {g['id']}")
        out.append({"id": int(g["id"]), "label": label, "map": g["map"], "age": age_of(g["map"]),
                    "sink": float(g.get("sink", 0.0)), "barriers": [bar], "guards": rows})
    return {"format": "ootrr-gate-session", "version": 1, "note": "exported from the panel's Manage Gate card (gate_logic.py)", "gates": out}, notes


def dump_session(session: dict) -> str:
    lines: list[str] = [json.dumps(g, separators=(",", ":")) for g in session["gates"]]
    head: str = json.dumps({k: v for k, v in session.items() if k != "gates"}, separators=(",", ":"))[:-1]
    return head + ',"gates":[\n' + ",\n".join(lines) + "\n]}\n"


def load(path: str, default):
    try:
        with open(path, encoding="utf-8-sig") as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def save(path: str, data) -> None:
    tmp: str = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1)
    os.replace(tmp, path)


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[0] != "export":
        print(__doc__, file=sys.stderr)
        return 2
    notes_dir: str = os.path.join(argv[1], "notes")
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import shipped_gates
    shipped, _ = shipped_gates.load()
    gates: list[dict] = merge(load(os.path.join(notes_dir, FILE_NAME), []), shipped)
    points: list[dict] = load(os.path.join(notes_dir, "spawnpoints.json"), [])
    session, notes = export_session(gates, points, shipped)
    with open(argv[2], "w", encoding="utf-8", newline="\n") as f:
        f.write(dump_session(session))
    for n in notes:
        print("note:", n, file=sys.stderr)
    print(f"{len(session['gates'])} gate(s) written to {argv[2]}; next: convert it with json_to_gates.py --first-id N", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
