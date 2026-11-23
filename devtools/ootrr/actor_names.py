"""The actor viewer's name lookup: OOTRR's data/locations.json and data/tracker_targets.json keyed by placement id
(the decimal form of the game's placement hash, as the randomizer writes it), written as extras/actors.js (the card's own
script: the panel serves it beside actors.html) and as a label text file for the game's `labels` command.

    python -I actor_names.py [OOTRR_CHECKOUT]      (default $OOTRR_CHECKOUT, else the repository devtools lives in)

locations.json: locations[].name (the randomizer's name) with refs[].hash (decimal), a location may have several refs.
tracker_targets.json: targets[] with id "<kind>:<decimal>" (some ids are names, no number), name (tracker's own name),
ootr (the randomizer's name; absent on some rows), scene, map, room, and keys[] (more decimal ids of the same location).
"""
import json
import os
import re
import sys
from collections import Counter

HERE0 = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE0)
import actor_index as AI  # noqa: E402
import actor_kinds as AK

import paths as PATHS  # noqa: E402

JS_PATH = os.path.join(PATHS.EXTRAS, "actors.js")
LABELS_PATH = PATHS.cache("actor-labels.tsv")
LABELS_COLOUR_PATH = PATHS.cache("actor-labels-colour.tsv")
PALETTE: dict = {"chest": "#f5c542", "pot": "#c98a4b", "grass": "#5cc46a", "gs": "#e0463c", "heart": "#ff6fa8", "item": "#4fc3f7",
                 "enemy": "#ff8a3d", "npc": "#b48cf2", "object": "#cfd8dc", "scenery": "#78909c",
                 "trees": "#2e9e4f", "rocks": "#9e9e9e", "bugs": "#ffe94d", "animals": "#a1887f", "fairies": "#f48fb1", "signs": "#bcaaa4",
                 "doors": "#b0bec5"}
KIND_COLOUR: dict = {"chest": "chest", "trap_chest": "chest", "pot": "pot", "flying_pot": "pot", "crate": "pot", "beehive": "pot",
                     "grass": "grass", "gs": "gs", "skulltula": "gs", "heart": "heart", "bossheart": "heart", "heart_piece": "heart",
                     "heart_container": "heart", "rupee": "item", "silver_rupee": "item", "key": "item", "pickup": "item",
                     "freestanding": "item", "scrub": "npc", "gift": "npc", "shop": "npc", "cow": "npc", "rock": "object"}


def colour_kind(e: dict) -> str:
    k: str = ""
    if e["l"]:
        k = e["l"][0][1]
    elif e["t"]:
        k = e["t"][0][2]
    return KIND_COLOUR.get(k, "object")


NUM = re.compile(r"^\d{1,20}$")


def read(path: str) -> dict:
    with open(path, encoding="utf-8-sig") as f:
        return json.load(f)


def add(ids: dict, k: str, side: str, v: object) -> None:
    if not NUM.match(str(k)):
        return
    e: dict = ids.setdefault(str(k), {"l": [], "t": []})
    if v not in e[side]:
        e[side].append(v)


def build(checkout: str) -> dict:
    d: str = os.path.join(checkout, "data")
    lp: str = os.path.join(d, "locations.json")
    tp: str = os.path.join(d, "tracker_targets.json")
    out: dict = {"ok": False, "locations_path": lp, "tracker_path": tp, "loc_found": os.path.isfile(lp),
                 "trk_found": os.path.isfile(tp), "ids": {}, "n_loc": 0, "n_trk": 0, "labels_file": LABELS_PATH}
    ids: dict = out["ids"]
    if out["loc_found"]:
        locs: list = read(lp).get("locations", [])
        out["n_loc"] = len(locs)
        for x in locs:
            for r in x.get("refs", []):
                add(ids, r.get("hash", ""), "l", [x.get("name", ""), x.get("kind", "")])
    if out["trk_found"]:
        trs: list = read(tp).get("targets", [])
        out["n_trk"] = len(trs)
        for x in trs:
            row: list = [x.get("name", ""), x.get("ootr") or "", x.get("kind", ""), x.get("scene") or "", x.get("map") or "", x.get("room")]
            ks: list = [x.get("id", "").split(":", 1)[-1]] + list(x.get("keys", []))
            for k in dict.fromkeys(ks):
                add(ids, k, "t", row)
    out["ok"] = out["loc_found"] or out["trk_found"]
    return out


def label_text(e: dict) -> str:
    if e["l"]:
        return e["l"][0][0]
    if e["t"]:
        return e["t"][0][1] or e["t"][0][0]
    return ""


MAX_KINDS: int = 8
ALSO_LINKED: tuple = ("Obj_TBox_", "Obj_Pot_", "Obj_Plant_", "Obj_LiftRock_", "Obj_Rupee", "Obj_Heart", "Obj_BrokenBox_")


def two_tokens(g: str) -> str:
    t: list = g.split("_")
    return "_".join(t[:2]) + "_" if len(t) > 2 else g


def location_kinds(ids: dict, named: dict) -> list:
    c: Counter = Counter(two_tokens(e[6]) for h, e in ids.items() if h in named)
    return [k for k, _ in c.most_common(MAX_KINDS)]


def link_table(ids: dict, floors: dict, named: dict, kinds: list) -> dict:
    maps: list = []
    layers: list = []
    at: dict = {}
    for h, e in ids.items():
        if h not in named and not e[6].startswith(tuple(kinds) + ALSO_LINKED):
            continue
        mk: list = [e[0], e[1]]
        if mk not in maps:
            maps.append(mk)
        la: str | None = AI.layer_for(floors, e[0], e[1], h, e[4])
        li: int = -1
        if la is not None:
            if la not in layers:
                layers.append(la)
            li = layers.index(la)
        at[h] = [maps.index(mk), li, round(e[3], 1), round(e[5], 1)]
    return {"maps": maps, "layers": layers, "at": at}


def write(checkout: str, js_path: str = JS_PATH, labels_path: str = LABELS_PATH, romfs: str | None = None, ids: dict | None = None,
          floors: dict | None = None) -> dict:
    data: dict = build(checkout)
    ix: dict = AI.load(romfs) if ids is None else ids
    fl: dict = AI.load_floors() if floors is None else floors
    data["index_n"] = len(ix)
    data["palette"] = PALETTE
    data["labels_file_colour"] = os.path.join(os.path.dirname(labels_path), "actor-labels-colour.tsv")
    data["colour_of"] = {k: colour_kind(e) for k, e in data["ids"].items()}
    data["kinds_table"] = AK.table()
    data["floors"] = fl
    data["kinds"] = location_kinds(ix, data["ids"]) if ix else []
    data["links"] = link_table(ix, fl, data["ids"], data["kinds"]) if ix else {"maps": [], "layers": [], "at": {}}
    data["labels_file"] = labels_path
    os.makedirs(os.path.dirname(js_path), exist_ok=True)
    with open(js_path, "w", encoding="utf-8") as f:
        f.write("window.OOTRR_NAMES=" + json.dumps(data, separators=(",", ":")) + ";\nif(window.actNamesSet)window.actNamesSet(window.OOTRR_NAMES);\n")
    os.makedirs(os.path.dirname(labels_path), exist_ok=True)
    with open(labels_path, "w", encoding="utf-8", newline="\n") as f:
        for k, e in data["ids"].items():
            t: str = label_text(e)
            if t:
                f.write(k + "\t" + k + " " + t + "\n")
    with open(data["labels_file_colour"], "w", encoding="utf-8", newline="\n") as f:
        for k, e in data["ids"].items():
            t = label_text(e)
            if t:
                f.write(k + "\t" + k + " " + t + "\t" + PALETTE[colour_kind(e)] + "\n")
    return data


if __name__ == "__main__":
    co: str = sys.argv[1] if len(sys.argv) > 1 else PATHS.ootrr_checkout()
    r: dict = write(co)
    print("actor names: %d ids from %d locations and %d tracker targets (%s); %d placed actors indexed, %d linked" % (
        len(r["ids"]), r["n_loc"], r["n_trk"], co, r["index_n"], len(r["links"]["at"])))
