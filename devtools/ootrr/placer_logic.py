import csv
import json
import math
import os
import sys

NEAR_RADIUS: float = 30.0
HIT_RADIUS: float = 1.5
SAME_SPOT: float = 2.0
WANDER: float = 8.0

CATALOG: list[tuple[str, str]] = [
    ("Enemy_Dekubaba", "Baba"), ("Enemy_Scrub_Mad", "Scrub"), ("Enemy_Skulltula", "Skull"),
    ("Enemy_Skullwalltula", "WallSk"), ("Enemy_Peahat", "Peahat"), ("Enemy_Peahat_Larva", "Larva"),
    ("Enemy_Stalchild", "Stalch"), ("Enemy_Guay", "Guay"), ("Enemy_Keese", "Keese"),
    ("Enemy_Keese_Fire", "FireKee"), ("Enemy_Keese_Ice", "IceKee"), ("Enemy_Poe", "Poe"),
    ("Enemy_Tektite_Red", "RedTek"), ("Enemy_Tektite_Blue", "BluTek"), ("Enemy_Wolfos", "Wolfos"),
    ("Enemy_Wolfos_White", "WhtWolf"), ("Enemy_Beamos", "Beamos"), ("Enemy_Amos", "Armos"),
    ("Enemy_Lizalfos", "Lizalf"), ("Enemy_Dinolfos", "Dinolf"), ("Enemy_BabyDodongo", "BabyDod"),
    ("Enemy_Dodongo", "Dodongo"), ("Enemy_Octorok", "Octo"), ("Enemy_Biri", "Biri"),
    ("Enemy_Bari", "Bari"), ("Enemy_Tailpasaran", "Tailp"), ("Enemy_ReDead", "ReDead"),
    ("Enemy_Gibdo", "Gibdo"), ("Enemy_Moblin", "Moblin*"), ("Enemy_Bubble_Blue", "BlueBub"),
    ("Enemy_Bubble_Green", "GrnBub"), ("Enemy_Bubble_Red", "RedBub"), ("Enemy_Bubble_White", "WhtBub"),
    ("Enemy_Stalfos", "Stalfos"), ("Enemy_Wallmaster", "WallM"), ("Enemy_Floormaster", "FloorM"),
    ("Enemy_TorchSlug", "Torch"), ("Enemy_LikeLike", "LikeLk"), ("Enemy_Freezard", "Freez"),
    ("Enemy_Spike", "Spike"), ("Enemy_ShellBlade", "Shell"), ("Enemy_Leever", "Leever"),
    ("Enemy_FlyingPot", "FakePot"), ("Enemy_Anubis", "Anubis*"), ("Enemy_IronKnuckle_Silver", "IronKn"),
    ("Enemy_IronKnuckle_White", "WhtIK"), ("Enemy_IronKnuckle_Black", "BlkIK"), ("Enemy_DeadHand", "DeadHd"),
    ("Enemy_Poe_Hyrule", "FirePoe"), ("Enemy_Moblin_Club", "ClubMob*"), ("Enemy_Bubble_Green_Big", "BigGrnBub"),
    ("Enemy_Dekubaba_Withered", "WithBaba"), ("Enemy_Floormaster_Little", "SmlHand"), ("Enemy_Gohma_Larva", "GohLarva"),
    ("Enemy_Gohma_Egg", "GohEgg"), ("Enemy_Leever_Big", "BigLeev"),    ("Enemy_Shabom", "Shabom"), ("Enemy_Stalchild_Big", "BigStalch"), ("Enemy_Stalfos_Weak", "WeakStalf"),
    ("Enemy_Stinger", "Stinger"),
]


def dist(a: list[float], b: list[float]) -> float:
    return math.sqrt(sum((float(a[i]) - float(b[i])) ** 2 for i in range(3)))


def yaw_for(facing: float) -> float:
    return math.radians((facing or 0.0) + 180.0)


def next_n(points: list[dict]) -> int:
    return max([int(p.get("n", 0)) for p in points] + [0]) + 1


def on_map(points: list[dict], map_name: str) -> list[dict]:
    return [p for p in points if p.get("map") == map_name]


def nearby_counts(points: list[dict], map_name: str, pos: list[float], radius: float = NEAR_RADIUS) -> dict[str, int]:
    out: dict[str, int] = {}
    for p in on_map(points, map_name):
        if dist(p["pos"], pos) <= radius:
            out[p["name"]] = out.get(p["name"], 0) + 1
    return out


FLIERS: tuple[str, ...] = ("Enemy_Keese", "Enemy_Keese_Fire", "Enemy_Keese_Ice", "Enemy_Guay", "Enemy_Bubble_Blue", "Enemy_Bubble_Green",
                            "Enemy_Bubble_Red", "Enemy_Bubble_White", "Enemy_Poe", "Enemy_Poe_Hyrule", "Enemy_Bubble_Green_Big", "Enemy_Biri", "Enemy_Bari", "Enemy_Peahat_Larva")
HANGERS: tuple[str, ...] = ("Enemy_Skulltula",)
HABITAT_KINDS: dict[str, str] = {
    "fly": "flies: place it in the air, ~2 m up",
    "still": "stays put (rooted, or waits until woken)",
    "walk": "walks on the floor",
    "water": "needs water",
    "sand": "burrows: needs open ground or sand",
    "wall": "clings to a wall",
    "ceil": "hangs from or drops off a ceiling",
}
HABITAT: dict[str, str] = {
    "Enemy_Keese": "fly", "Enemy_Keese_Fire": "fly", "Enemy_Keese_Ice": "fly", "Enemy_Guay": "fly",
    "Enemy_Bubble_Blue": "fly", "Enemy_Bubble_Green": "fly", "Enemy_Bubble_Red": "fly", "Enemy_Bubble_White": "fly",
    "Enemy_Bubble_Green_Big": "fly", "Enemy_Poe": "fly", "Enemy_Poe_Hyrule": "fly", "Enemy_Biri": "fly", "Enemy_Bari": "fly",
    "Enemy_Peahat": "fly", "Enemy_Peahat_Larva": "fly", "Enemy_Shabom": "fly", "Enemy_Stinger": "fly",
    "Enemy_Dekubaba": "still", "Enemy_Dekubaba_Withered": "still", "Enemy_Scrub_Mad": "still", "Enemy_Scrub_Deku": "still",
    "Enemy_Beamos": "still", "Enemy_Amos": "still", "Enemy_FlyingPot": "still", "Enemy_Gohma_Egg": "still", "Enemy_Freezard": "still",
    "Enemy_Octorok": "water",
    "Enemy_Leever": "sand", "Enemy_Leever_Big": "sand",
    "Enemy_Skullwalltula": "wall",
    "Enemy_Skulltula": "ceil", "Enemy_Wallmaster": "ceil",
}


def habitat(name: str) -> str:
    k: str | None = kind_of(name)
    return HABITAT.get(k or name, "walk")


def full_name(name: str) -> str:
    return name.replace("Enemy_", "").replace("_", " ")


def button_label(label: str) -> tuple[str, bool]:
    return label.rstrip("*"), label.endswith("*")
ABOVE: float = 2.0
HANG_AT: float = 0.5
ROOF_GAP: float = 0.3
ROOF_MAX: float = 25.0
ROOF_REACH: float = 30.0
HIT_RISE: float = 8.0


def flat_dist(a: list[float], b: list[float]) -> float:
    return math.hypot(float(a[0]) - float(b[0]), float(a[2]) - float(b[2]))


def roof_guess(entry: dict | None, pos: list[float], room: int | None) -> float | None:
    best: tuple[float, float] | None = None
    for e in (entry or {}).get("enemies", []):
        if kind_of(e["n"]) not in HANGERS or (room is not None and e.get("r") != room):
            continue
        y: float = float(e["p"][1])
        d: float = flat_dist(e["p"], pos)
        if pos[1] + 1.0 < y <= pos[1] + ROOF_MAX and d <= ROOF_REACH and (best is None or d < best[0]):
            best = (d, y)
    return best[1] if best else None


def spawn_spot(name: str, pos: list[float], entry: dict | None, room: int | None) -> list[float]:
    if name in HANGERS:
        return [pos[0], pos[1] + HANG_AT, pos[2]]
    if name in FLIERS:
        return [pos[0], pos[1] + ABOVE, pos[2]]
    return list(pos)


def hit_points(points: list[dict], map_name: str, pos: list[float], radius: float = HIT_RADIUS) -> dict[str, dict]:
    out: dict[str, dict] = {}
    best: dict[str, float] = {}
    for p in on_map(points, map_name):
        d: float = flat_dist(p["pos"], pos)
        if abs(float(p["pos"][1]) - float(pos[1])) > HIT_RISE:
            continue
        if d <= radius and d < best.get(p["name"], 1e9):
            best[p["name"]] = d
            out[p["name"]] = p
    return out


def nearest_point(points: list[dict], map_name: str, pos: list[float]) -> dict | None:
    here: list[dict] = on_map(points, map_name)
    return min(here, key=lambda p: dist(p["pos"], pos)) if here else None


FLOOR_NORMAL: list[float] = [0.0, 1.0, 0.0]


def make_point(points: list[dict], map_name: str, name: str, pos: list[float], facing: float, when: str, face_link: bool = False,
               normal: list[float] | None = None, surface: str = "floor") -> dict:
    f: float = float(facing or 0.0)
    return {"n": next_n(points), "map": map_name, "name": name, "pos": [round(float(v), 2) for v in pos],
            "facing": round(f, 1), "yaw": round(f + 180.0 if face_link else f, 1), "time": when,
            "normal": [round(float(v), 3) for v in (normal or FLOOR_NORMAL)], "surface": surface, "wall": surface == "wall"}


AIM_REMOVE_RADIUS: float = 1.5
AIM_REMOVE_RISE: float = 4.0


def yaw_toward(frm: list[float], to: list[float]) -> float | None:
    dx: float = float(to[0]) - float(frm[0])
    dz: float = float(to[2]) - float(frm[2])
    if math.hypot(dx, dz) < 0.05:
        return None
    return math.degrees(math.atan2(dx, dz))


def aim_link(hit: dict, status_pos: list[float] | None = None) -> list[float] | None:
    for cand in (hit.get("link"), status_pos):
        if cand and len(cand) == 3:
            return [float(v) for v in cand]
    return None


def aim_spot(hit: dict, name: str, facing: float, entry: dict | None = None, room: int | None = None, link: list[float] | None = None) -> dict:
    surface: str = str(hit.get("surface") or "floor")
    pos: list[float] = [float(v) for v in hit["pos"]]
    normal: list[float] = [float(v) for v in (hit.get("normal") or FLOOR_NORMAL)]
    yaw_out = hit.get("yaw_out")
    if surface == "wall":
        out: float | None = math.degrees(float(yaw_out)) if yaw_out is not None else (math.degrees(math.atan2(normal[0], normal[2])) if math.hypot(normal[0], normal[2]) > 0.05 else None)
        if out is None:
            out = float(facing or 0.0) + 180.0
        return {"pos": pos, "yaw": out, "surface": "wall", "normal": normal}
    spot: list[float] = pos if surface == "ceiling" else spawn_spot(name, pos, entry, room)
    toward: float | None = yaw_toward(pos, link) if link else None
    if toward is None and hit.get("yaw_face") is not None:
        toward = math.degrees(float(hit["yaw_face"]))
    if toward is None:
        toward = float(facing or 0.0) + 180.0
    return {"pos": spot, "yaw": toward, "surface": surface, "normal": normal}


def nearest_to_hit(points: list[dict], map_name: str, pos: list[float]) -> dict | None:
    near: list[dict] = [p for p in on_map(points, map_name)
                        if flat_dist(p["pos"], pos) <= AIM_REMOVE_RADIUS and abs(float(p["pos"][1]) - float(pos[1])) <= AIM_REMOVE_RISE]
    return min(near, key=lambda p: (flat_dist(p["pos"], pos), abs(float(p["pos"][1]) - float(pos[1])))) if near else None


def moved(p: dict, d: list[float]) -> dict:
    out: dict = dict(p)
    out["pos"] = [round(float(p["pos"][i]) + float(d[i]), 2) for i in range(3)]
    return out


def point_yaw(p: dict) -> float:
    return math.radians(p["yaw"]) if "yaw" in p else yaw_for(p.get("facing"))


def last_name(points: list[dict], map_name: str) -> str | None:
    return max(points, key=lambda p: int(p.get("n", 0)))["name"] if points else None


def remove_point(points: list[dict], n: int) -> list[dict]:
    return [p for p in points if p.get("n") != n]


def same_name(a: str, b: str) -> bool:
    a, b = a.lower(), b.lower()
    return a == b or a.startswith(b) or b.startswith(a)


def enemy_pos(e: dict) -> list[float]:
    return [e["x"], e["y"], e["z"]]


def alive_for(point: dict, enemies: list[dict], tracked_id: object) -> dict | None:
    live: list[dict] = [e for e in enemies if e.get("hp", 0) > 0]
    if tracked_id is not None:
        for e in live:
            if e.get("id") == tracked_id:
                return e
    near: list[dict] = [e for e in live if same_name(e.get("name", ""), point["name"]) and dist(enemy_pos(e), point["pos"]) <= WANDER]
    return min(near, key=lambda e: dist(enemy_pos(e), point["pos"])) if near else None


def new_enemy(before: list[dict], after: list[dict], name: str, pos: list[float]) -> dict | None:
    seen: set = {e.get("id") for e in before}
    fresh: list[dict] = [e for e in after if e.get("id") not in seen and e.get("hp", 0) > 0 and same_name(e.get("name", ""), name)]
    if not fresh:
        return None
    return min(fresh, key=lambda e: dist(enemy_pos(e), pos))


LATE_RADIUS: float = 3.0


def late_spawn(before: list[dict], after: list[dict], name: str, pos: list[float]) -> dict | None:
    found: dict | None = new_enemy(before, after, name, pos)
    if found and dist(enemy_pos(found), pos) <= LATE_RADIUS:
        return found
    return None


def to_kill(enemies: list[dict], link: list[float], radius: float = NEAR_RADIUS) -> list[dict]:
    out: list[dict] = []
    for e in enemies:
        if e.get("hp", 0) <= 0 or (e["x"], e["y"], e["z"]) == (0.0, 0.0, 0.0):
            continue
        if dist(enemy_pos(e), link) <= radius:
            out.append(e)
    return out


NOT_REAL: tuple[str, ...] = ("Dictionary", "DungeonBoss", "MiddleBoss", "Skulltula_Gold", "Scrub_Business", "GhostPicturePerformer", "Manager",
                             "_Leaf", "DeadHand_Hand", "Tentacle", "TrickyDoor", "CrazyFloorTile", "Floormaster_Little", "Haribote", "Truth")
ROOM_VOTERS: int = 5
ROOM_REACH: float = 40.0
MULT_CAP: float = 10.0
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import paths as PATHS  # noqa: E402

LOCATIONS: str = os.environ.get("DEVTOOLS_LOCATIONS_CSV", "")
VANILLA: str = PATHS.cache("vanilla_enemies.json")


MONSTERS: str = os.environ.get("DEVTOOLS_MONSTERS_CSV", "")


def load_hp(path: str = MONSTERS) -> dict[str, float]:
    out: dict[str, float] = {}
    try:
        with open(path, encoding="utf-8-sig") as f:
            for r in csv.DictReader(f):
                try:
                    out[r["ID"]] = float(r["HP"])
                except (KeyError, TypeError, ValueError):
                    pass
    except OSError:
        pass
    return out


def by_hp(catalog: list[tuple[str, str]], hp: dict[str, float]) -> list[tuple[str, str]]:
    return sorted(catalog, key=lambda c: (c[0] not in hp, hp.get(c[0], 0.0), c[1].lower()))


def real_enemy(name: str) -> bool:
    return name.startswith("Enemy_") and not any(bad in name for bad in NOT_REAL)


def kind_of(name: str) -> str | None:
    best: str | None = None
    for cat, _ in CATALOG:
        if (name == cat or name.startswith(cat + "_")) and (best is None or len(cat) > len(best)):
            best = cat
    return best


def load_vanilla(path: str = VANILLA) -> dict[str, dict]:
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f).get("maps", {})
    except (OSError, ValueError):
        return {}


def load_places(path: str = LOCATIONS) -> dict[str, str]:
    try:
        with open(path, encoding="utf-8-sig") as f:
            return {r["MSBT ID"]: r["Name"] for r in csv.DictReader(f)}
    except (OSError, KeyError):
        return {}


def base_map(map_name: str) -> str:
    for suffix in ("_Child", "_Adult", "_Alternative"):
        map_name = map_name.replace(suffix, "")
    return map_name


def map_key(vanilla: dict[str, dict], map_name: str, age: str = "") -> str | None:
    for key in (map_name, f"{map_name}_{(age or '').capitalize()}", f"{map_name}_Child", f"{map_name}_Adult"):
        if key in vanilla:
            return key
    return None


def place_name(places: dict[str, str], map_name: str) -> str:
    return places.get(base_map(map_name)) or map_name or "?"


def room_at(refs: list, pos: list[float]) -> int | None:
    near: list[tuple[float, int]] = sorted((dist(r[0], pos), int(r[1])) for r in refs)[:ROOM_VOTERS]
    if not near or near[0][0] > ROOM_REACH:
        return None
    votes: dict[int, float] = {}
    for d, room in near:
        votes[room] = votes.get(room, 0.0) + 1.0 / (d + 1.0)
    return max(votes, key=lambda k: votes[k])


def vanilla_in(entry: dict, room: int | None) -> list[str]:
    return [e["n"] for e in entry.get("enemies", []) if real_enemy(e["n"]) and (room is None or e.get("r") == room)]


def multiplier(vanilla: int, added: int) -> float | None:
    if vanilla <= 0:
        return None
    return (vanilla + added) / vanilla


def mult_text(vanilla: int, added: int) -> str:
    m: float | None = multiplier(vanilla, added)
    if m is None:
        return f"+{added}" if added else "no vanilla enemies"
    if m >= MULT_CAP:
        return f"{int(MULT_CAP)}x+"
    return f"{m:.1f}".rstrip("0").rstrip(".") + "x"


def room_stats(entry: dict | None, points: list[dict], map_name: str, pos: list[float]) -> dict:
    refs: list = (entry or {}).get("refs", [])
    room: int | None = room_at(refs, pos) if refs else None
    names: list[str] = vanilla_in(entry or {}, room)
    mine: list[dict] = [p for p in on_map(points, map_name) if room is None or (refs and room_at(refs, p["pos"]) == room)]
    kinds: dict[str, int] = {}
    for n in names:
        k: str | None = kind_of(n)
        if k:
            kinds[k] = kinds.get(k, 0) + 1
    ours: dict[str, int] = {}
    for p in mine:
        ours[p["name"]] = ours.get(p["name"], 0) + 1
    return {"room": room, "vanilla": len(names), "added": len(mine), "mult": mult_text(len(names), len(mine)), "kinds": kinds, "ours": ours}


def live_in_room(entry: dict | None, enemies: list[dict], room: int | None, link: list[float], radius: float) -> dict[str, int]:
    refs: list = (entry or {}).get("refs", [])
    out: dict[str, int] = {}
    for e in enemies:
        p: list[float] = enemy_pos(e)
        if p == [0.0, 0.0, 0.0] or e.get("hp", 0) <= 0:
            continue
        if room is not None and refs:
            if room_at(refs, p) != room:
                continue
        elif dist(p, link) > radius:
            continue
        k: str | None = kind_of(e["name"]) or e["name"]
        out[k] = out.get(k, 0) + 1
    return out


def suggested(entry: dict | None, room: int | None) -> list[str]:
    order: list[str] = []
    for pick in (room, None):
        for n in vanilla_in(entry or {}, pick):
            k: str | None = kind_of(n)
            if k and k not in order:
                order.append(k)
    return order


def header_text(place: str, room: int | None, vanilla: int, added: int) -> str:
    where: str = f"{place}: room {room}" if room is not None else place
    return f"{where} {vanilla + added}/{int(MULT_CAP) * vanilla}"


RADAR_MAX: int = 256
SHORT: dict[str, str] = dict(CATALOG)


def clean_label(text: str) -> str:
    return " ".join("".join(c if c.isalnum() or c == " " else " " for c in text).split())


def radar_label(name: str) -> str:
    k: str | None = kind_of(name)
    short: str | None = SHORT.get(k) if k else None
    return short or clean_label(name.replace("Enemy_", "").replace("_", " "))[:15].strip()


ACTOR_BOXES: str = PATHS.cache("actor_boxes.json")


def load_boxes(path: str = ACTOR_BOXES) -> dict[str, list]:
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def box_size(boxes: dict[str, list] | None, name: str) -> float:
    k: str | None = kind_of(name)
    b: list | None = (boxes or {}).get(name) or (boxes or {}).get(k or "")
    if not b or len(b) != 2 or len(b[0]) != 3 or len(b[1]) != 3:
        return 0.0
    return round(abs((float(b[1][0]) - float(b[0][0])) * (float(b[1][1]) - float(b[0][1])) * (float(b[1][2]) - float(b[0][2]))), 3)


def last_used(points: list[dict], used: dict[str, int] | None = None) -> dict[str, float]:
    out: dict[str, float] = {k: float(v) for k, v in (used or {}).items()}
    for p in points:
        out[p["name"]] = max(out.get(p["name"], 0.0), float(int(p.get("n", 0))) * 1e-6)
    return out


def box_text(boxes: dict[str, list] | None, name: str) -> str:
    b: list | None = (boxes or {}).get(name)
    if not b or len(b) != 2 or len(b[0]) != 3 or len(b[1]) != 3:
        return ""
    return "[" + " ".join(f"{float(v):.2f}" for v in list(b[0]) + list(b[1])) + "] "


def radar_marks(entry: dict | None, points: list[dict], map_name: str, arrows: bool = False, extra: list[str] | None = None, tags: dict[int, str] | None = None,
                near: int | None = None, boxes: dict[str, list] | None = None, box_ns: set[int] | None = None, all_boxes: dict[str, list] | None = None) -> str:
    lines: list[str] = list(extra or [])
    for p in on_map(points, map_name):
        x, y, z = (float(v) for v in p["pos"])
        label: str = clean_label(radar_label(p["name"]) + (tags or {}).get(int(p.get("n", 0)), ""))
        label = (">" + label if near is not None and int(p.get("n", 0)) == near else label)[:15]
        forced: bool = bool(box_ns and int(p.get("n", 0)) in box_ns)
        if arrows or boxes or forced:
            lines.append(f"P {x:.2f} {y:.2f} {z:.2f} {math.degrees(point_yaw(p)):.1f} {box_text(boxes if boxes else all_boxes if forced else None, p['name'])}{label}")
        else:
            lines.append(f"p {x:.2f} {y:.2f} {z:.2f} {label}")
    for e in (entry or {}).get("enemies", []):
        if not real_enemy(e["n"]):
            continue
        x, y, z = (float(v) for v in e["p"])
        lines.append(f"v {x:.2f} {y:.2f} {z:.2f} {radar_label(e['n'])}")
    return "\n".join(lines[:RADAR_MAX])


def radar_request(map_name: str, marks: str) -> dict:
    return {"cmd": "radar", "op": "set", "map": map_name, "marks": marks}


NO_DICTIONARY: tuple[str, ...] = ("Enemy_Lizalfos",)


def placeholder_spots(points: list[dict], map_name: str) -> str:
    lines: list[str] = []
    for p in on_map(points, map_name):
        name: str = p["name"]
        if not name.startswith("Enemy_") or name in NO_DICTIONARY:
            continue
        x, y, z = (float(v) for v in p["pos"])
        lines.append(f"{name} {x:.2f} {y:.2f} {z:.2f} {math.degrees(point_yaw(p)):.1f}")
    return chr(10).join(lines[:RADAR_MAX])


def placeholder_request(map_name: str, spots: str, on: bool) -> dict:
    return {"cmd": "placeholders", "op": "set", "map": map_name, "spots": spots, "on": bool(on)}
