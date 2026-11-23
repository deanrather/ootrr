"""Index of every placed actor in the game's map files: placement hash (decimal) -> scene, map, room, x, y, z, type.
Used by the actor viewer so its objmap links name the right map and floor (objmap wants the floor, e.g. B1_State00, which the
bridge does not report: a placed object's floor follows from its own height, calibrated against the modder's location notes).

    python -I actor_index.py [--romfs PATH] [--rebuild]      (default $OOTRR_ROMFS or romfs in config.json)

Reads Banc/<Scene>/<Map>_Static|_Dynamic.bcett.byml.zs with the Byml and zstd code of placements.py (needs the zstandard
module). The index is cached in actor-index.json in cache/ (git-ignored: it is made from the game files and stays on this PC).
The calibration (objmap_floors.json: numbers and layer names only) is data/objmap_floors.json.
"""
import json
import os
import re
import sys

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import paths as PATHS  # noqa: E402

CACHE: str = PATHS.cache("actor-index.json")
FLOORS: str = os.path.join(PATHS.DATA, "objmap_floors.json")
OBJMAP: str = "https://objmap-oot.zeldamods.org/#/map/"
ZOOM: str = "7"


def romfs_path(arg: str | None = None) -> str:
    return PATHS.romfs(arg)


def signature(romfs: str) -> str:
    banc: str = os.path.join(romfs, "Banc")
    n: int = 0
    newest: float = 0.0
    for a in sorted(os.listdir(banc)):
        d: str = os.path.join(banc, a)
        if os.path.isdir(d):
            for f in os.listdir(d):
                n += 1
                newest = max(newest, os.path.getmtime(os.path.join(d, f)))
    return "%s|%d|%.0f" % (os.path.abspath(romfs), n, newest)


def build(romfs: str) -> dict:
    import placements as P
    zs = P.Zs(romfs)
    banc: str = os.path.join(romfs, "Banc")
    ids: dict = {}
    for scene in sorted(os.listdir(banc)):
        d: str = os.path.join(banc, scene)
        if not os.path.isdir(d):
            continue
        for f in sorted(os.listdir(d)):
            if not f.endswith(".bcett.byml.zs"):
                continue
            try:
                root: dict = P.byml(zs.read(os.path.join(d, f)))
            except Exception:
                continue
            m: str = re.sub(r"_(Static|Dynamic)$", "", f.split(".")[0])
            for a in root.get("Actors") or []:
                h = a.get("Hash") if isinstance(a, dict) else None
                if not isinstance(h, int):
                    continue
                t: list = a.get("Translate") or [0, 0, 0]
                room = (a.get("Dynamic") or {}).get("DefaultDungeonRoomIndex")
                row: list = [scene, m, room, round(float(t[0]), 2), round(float(t[1]), 2), round(float(t[2]), 2), a.get("Gyaml", "?")]
                if str(h) not in ids or (m.endswith("_Child") and ids[str(h)][1].endswith("_Adult")):
                    ids[str(h)] = row
    return ids


def load(romfs: str | None = None, rebuild: bool = False, cache: str = CACHE) -> dict:
    r: str = romfs_path(romfs)
    if not os.path.isdir(os.path.join(r, "Banc")):
        return {}
    sig: str = signature(r)
    if not rebuild and os.path.isfile(cache):
        try:
            with open(cache, encoding="utf-8") as f:
                c: dict = json.load(f)
            if c.get("sig") == sig:
                return c["ids"]
        except (OSError, ValueError, KeyError):
            pass
    ids: dict = build(r)
    os.makedirs(os.path.dirname(cache), exist_ok=True)
    with open(cache, "w", encoding="utf-8") as f:
        json.dump({"sig": sig, "ids": ids}, f, separators=(",", ":"))
    return ids


def load_floors(path: str = FLOORS) -> dict:
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {"scenes": {}, "overrides": {}}


def layer_for(floors: dict, scene: str, m: str, h: str, y: float) -> str | None:
    o: dict = floors.get("overrides", {})
    if h in o:
        return o[h]
    s: list | None = floors.get("scenes", {}).get(scene)
    if s:
        for top, layer in s:
            if top is None or y < top:
                return layer
    if m.endswith(("_Child", "_Adult")):
        return "00"
    return None


def objmap_url(scene: str, m: str, layer: str | None, x: float, z: float, h: str) -> str:
    hexh: str = "0x%016x" % int(h)
    return OBJMAP + scene + "/" + m + "/" + (layer + "/" if layer else "") + "z" + ZOOM + ",%.1f,%.1f" % (x, z) + "?id=" + scene + "," + m + "," + hexh


def url_for_hash(ids: dict, floors: dict, h: str) -> str | None:
    e: list | None = ids.get(h)
    if not e:
        return None
    return objmap_url(e[0], e[1], layer_for(floors, e[0], e[1], h, e[4]), e[3], e[5], h)


if __name__ == "__main__":
    a: list = sys.argv[1:]
    rf: str | None = a[a.index("--romfs") + 1] if "--romfs" in a else None
    ix: dict = load(rf, "--rebuild" in a)
    print("actor index: %d placed actors" % len(ix))
