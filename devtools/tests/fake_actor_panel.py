"""The Longshot playtest panel with the OOTRR extras over a fake game, for the actor viewer's render check and screenshot.

    python fake_actor_panel.py [--port 8790] [--labels old|new] [--session DIR]

--labels new: the fake `labels` command answers like a build with the decimal format and a names file (the reply echoes
format and names); old: like today's tools build (unknown arguments ignored, a hex-only reply). Needs no game and no Longshot.
"""
import argparse
import json
import os
import shutil
import sys
import tempfile
import threading
from http.server import ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import _paths  # noqa: E402
import actor_names  # noqa: E402
import paths as PATHS  # noqa: E402
import server  # noqa: E402

PLACED: list = [
    ("Obj_TBox_A_01", "8579518566955184384", 1.0, 0.0, 2.0),
    ("Obj_Pot_A_01", "12786193094758345086", 2.5, 0.0, -1.0),
    ("Obj_Plant_Grass_A_01", "2527478394367885385", -3.0, 0.0, 4.0),
    ("Obj_Pot_A_02", "13609975487234437568", 6.0, 0.0, 5.0),
    ("Obj_BrokenBox_A_01", "100000000000000001", 8.0, 0.0, -2.0),
    ("Tree_Kokiri_BillboardTree_A_01", "100000000000000002", -6.0, 0.0, -4.0),
    ("Obj_Tree_A_01", "100000000000000003", -9.0, 0.0, 2.0),
    ("Animal_Kokiri_Insect_A", "100000000000000004", 3.0, 1.0, 7.0),
    ("Animal_Butterfly", "100000000000000005", 4.0, 2.0, -7.0),
    ("Obj_Rupee_Green", "100000000000000006", 0.5, 0.0, 3.5),
    ("Obj_Rupee_Blue", "100000000000000007", -1.5, 0.0, 3.0),
    ("Obj_LiftRock_A_01", "100000000000000008", 10.0, 0.0, 6.0),
    ("Obj_Heart_Piece", "100000000000000009", -4.0, 0.0, -9.0),
    ("Enemy_Tektite_Blue", "100000000000000010", 12.0, 0.0, -3.0),
    ("Enemy_Skulltula_Gold", "100000000000000011", -11.0, 3.0, 7.0),
    ("Npc_Navi", "100000000000000012", 5.0, 1.0, 1.0),
    ("Obj_SwitchShutter_A_02", "100000000000000013", 14.0, 0.0, 0.0),
    ("Animal_Cucco_CatchCucco", "100000000000000014", -7.0, 0.0, -8.0),
    ("Land_Kokiri_Ground_A_01", "100000000000000015", 1.0, -1.0, 1.0),
    ("AreaBox_Water", "100000000000000016", 2.0, 0.0, 2.0),
    ("Obj_SignBoard_A_01", "100000000000000017", -2.0, 0.0, 6.5),
    ("Obj_Fairy", "100000000000000018", 7.5, 2.0, 3.0),
    ("Obj_Tombstone_A_01", "100000000000000019", 9.0, 0.0, 9.0),
]


class FakeGame:
    def __init__(self, labels: str, checkout: str) -> None:
        self.labels_mode: str = labels
        self.calls: list = []
        self.on: bool = False
        self.rows: list = []
        for n, h, x, y, z in PLACED:
            self.rows.append({"name": n, "id": "0x1", "x": 40.0 + x, "y": 100.0 + y, "z": 20.0 + z, "state": 4, "hash": "0x%016x" % int(h), "hash_id": h})
        for r in self.rows:
            if r["name"] == "Enemy_Tektite_Blue":
                r["state"] = 8
        self.rows.append({"name": "Enemy_Keese", "id": "0x2", "x": 44.0, "y": 105.0, "z": 20.0, "state": 4, "hash": "0xffffffffffffffff", "hash_id": None})
        self.rows.append({"name": "Player", "id": "0x3", "x": 40.0, "y": 100.0, "z": 20.0, "state": 4, "hash": None, "hash_id": None})

    def status(self) -> dict:
        return {"map": "InsideTheDekuTree", "pos": [40.0, 100.0, 20.0], "facing": 0.0, "life": 12, "max_life": 12, "event": False, "loading": False}

    def request(self, cmd: str, timeout: object = None, **args: object) -> dict:
        self.calls.append((cmd, args))
        if cmd == "status":
            return {"ok": True, "game": self.status()}
        if cmd == "actors":
            rows: list = [dict(r, dist=((r["x"] - 40) ** 2 + (r["y"] - 100) ** 2 + (r["z"] - 20) ** 2) ** 0.5) for r in self.rows]
            rows.sort(key=lambda r: r["dist"])
            return {"ok": True, "actors": rows, "count": len(rows), "walk_us": 60.0}
        if cmd == "calls":
            return {"ok": True, "calls": [c for c in self.calls if c[0] == "labels"]}
        if cmd == "labels":
            if args.get("op") in ("on", "off"):
                self.on = args.get("op") == "on"
            if args.get("names"):
                self.table = self.read_names(str(args["names"]))
            self.last_labels = dict(args)
            l: dict = {"on": self.on, "shown": self.shown(args)}
            if self.labels_mode == "new" and self.on:
                l["format"] = args.get("format")
                l["names"] = 1490
            return {"ok": True, "labels": l}
        return {"ok": True}

    def read_names(self, path: str) -> dict:
        t: dict = {}
        try:
            with open(path, encoding="utf-8") as f:
                for line in f:
                    p: list = line.rstrip(chr(10)).split(chr(9))
                    if len(p) >= 2:
                        t[p[0]] = p[1:]
        except OSError:
            pass
        return t

    def shown(self, args: dict) -> int:
        table: dict = getattr(self, "table", {})
        rng: float = float(args.get("range", 25))
        mx: int = int(args.get("max", 40))
        hit: list = [r for r in self.rows if r["hash_id"] and (not args.get("named_only") or r["hash_id"] in table)
                     and ((r["x"] - 40) ** 2 + (r["y"] - 100) ** 2 + (r["z"] - 20) ** 2) ** 0.5 <= rng]
        self.hit_names = [r["name"] for r in hit][:mx]
        return min(mx, len(hit))

    def ask(self, cmd: str, **args: object) -> dict:
        return self.request(cmd, **args)

    def ctl(self, args: object, timeout: int = 0) -> str:
        return ""


def extras_dir(session: str) -> str:
    d: str = os.path.join(session, "extras")
    os.makedirs(d, exist_ok=True)
    cards: str = os.path.join(_paths.ROOT, "cards")
    for f in os.listdir(cards):
        shutil.copyfile(os.path.join(cards, f), os.path.join(d, f))
    return d


def make(session: str, labels: str, checkout: str, extras: str = "") -> tuple:
    a = argparse.Namespace(run="x", runs=None, sock=None, session=session, steps=os.path.join(_paths.PANEL, "examples", "steps.json"),
                           places=os.path.join(_paths.DATA, "places.json"), buttons=os.path.join(_paths.DATA, "buttons.json"),
                           extras=extras or extras_dir(session), flag_names=None, messages=None, port=0, lan=False, title="Mod UI")
    panel = server.Panel(a)
    g = FakeGame(labels, checkout)
    panel.game = g
    panel.flags.game = g
    return panel, g


def serve(panel: object, port: int) -> ThreadingHTTPServer:
    srv = ThreadingHTTPServer(("127.0.0.1", port), server.handler(panel))
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8790)
    ap.add_argument("--labels", default="old")
    ap.add_argument("--session", default=os.path.join(tempfile.gettempdir(), "actor-viewer-fake"))
    ap.add_argument("--ootrr", default=PATHS.ootrr_checkout())
    a = ap.parse_args()
    ex: str = extras_dir(a.session)
    actor_names.write(a.ootrr, os.path.join(ex, "actors.js"), os.path.join(a.session, "labels.tsv"))
    p, g = make(a.session, a.labels, a.ootrr, ex)
    print("fake panel on http://127.0.0.1:%d/" % a.port, flush=True)
    server.ThreadingHTTPServer(("127.0.0.1", a.port), server.handler(p)).serve_forever()
