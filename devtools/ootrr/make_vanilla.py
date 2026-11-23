"""Writes cache/vanilla_enemies.json: the game's own enemy placements per map, from its map files, with the room each
enemy belongs to and a few room reference points, as the enemy placer's room counts want them:

    {"maps": {"<Map>": {"enemies": [{"n": "Enemy_Stalfos", "p": [x, y, z], "r": room or null}], "refs": [[[x, y, z], room], ...]}}}

Room numbers are the placement's own DefaultDungeonRoomIndex; refs are every placement that carries one. Positions are
rounded to whole units. Needs the zstandard module and the extracted game; the game is never written.
"""
import json
import os
import re
import sys

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import paths as PATHS  # noqa: E402


def whole(t: list) -> list[int]:
    return [int(round(float(v))) for v in (t or [0, 0, 0])[:3]]


def build(romfs: str) -> dict:
    import placements as P
    zs = P.Zs(romfs)
    banc: str = os.path.join(romfs, "Banc")
    maps: dict = {}
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
            e: dict = maps.setdefault(m, {"enemies": [], "refs": []})
            for a in root.get("Actors") or []:
                if not isinstance(a, dict):
                    continue
                room = (a.get("Dynamic") or {}).get("DefaultDungeonRoomIndex")
                room = room if isinstance(room, int) else None
                if str(a.get("Gyaml", "")).startswith("Enemy_"):
                    e["enemies"].append({"n": a["Gyaml"], "p": whole(a.get("Translate")), "r": room})
                if room is not None:
                    e["refs"].append([whole(a.get("Translate")), room])
    return {"maps": {k: v for k, v in maps.items() if v["enemies"] or v["refs"]}}


def write(romfs: str, out: str | None = None) -> int:
    data: dict = build(romfs)
    p: str = out or PATHS.cache("vanilla_enemies.json")
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w", encoding="utf-8") as f:
        json.dump(data, f, separators=(",", ":"))
    return len(data["maps"])


if __name__ == "__main__":
    print("%d maps with enemies" % write(PATHS.need_romfs()))
