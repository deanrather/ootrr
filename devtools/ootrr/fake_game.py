"""A stand-in for the game, for placer_server.py --fake: render checks and screenshots with no game and no Longshot.

Link stands where SESSION/notes/fake_status.json says ({"map", "pos", "facing"}), read on every status call, so a script can
move him between requests; without the file he stands in Dodongo's Cavern room 1. Spawned enemies are kept in memory, damage
kills them, captions are printed. teleport and warp move him in memory until the file changes.
"""
import json
import os
import time

DEFAULT: dict = {"map": "DodongoCavern", "pos": [4.0, 100.0, -6.0], "facing": 30.0, "age": "child"}


class FakeGame:
    def __init__(self, session: str = "") -> None:
        self.path: str = os.path.join(session, "notes", "fake_status.json") if session else ""
        self.live: list[dict] = [{"id": "1", "name": "Enemy_Beamos", "x": 17.0, "y": 100.0, "z": -9.0, "hp": 6},
                                 {"id": "2", "name": "Enemy_Keese_Fire", "x": 6.0, "y": 120.0, "z": -8.0, "hp": 1}]
        self.nid: int = 100
        self.captions: list[tuple] = []
        self.moved: dict = {}
        self.file_seen: str = ""
        self.debug: bool = False
        self.marker: bool = False
        self.raw: list[dict] = []

    def status(self) -> dict:
        st: dict = dict(DEFAULT)
        try:
            with open(self.path, encoding="utf-8") as f:
                text: str = f.read()
            if text != self.file_seen:
                self.file_seen, self.moved = text, {}
            st.update(json.loads(text))
        except (OSError, ValueError):
            pass
        st.update(self.moved)
        return st

    def sleep(self, s: float) -> None:
        time.sleep(min(float(s), 0.1))

    def enemies(self, scan: bool = False) -> tuple[list[float], list[dict]]:
        return self.status()["pos"], [dict(e) for e in self.live]

    def spawn(self, name: str, pos: list[float], yaw: float, quick: bool = False) -> tuple[bool, str]:
        self.nid += 1
        self.live.append({"id": str(self.nid), "name": name, "x": float(pos[0]), "y": float(pos[1]), "z": float(pos[2]), "hp": 2})
        return True, "fake"

    def damage(self, enemy_id: object) -> None:
        self.live = [e for e in self.live if e["id"] != enemy_id]

    def aim_cast(self) -> dict:
        st: dict = self.status()
        if st.get("aim") is not None:
            return dict(st["aim"])
        return {"hit": True, "pos": list(st["pos"]), "normal": [0.0, 1.0, 0.0], "surface": "floor", "yaw_out": None, "dist": 3.0}

    def debug_state(self) -> bool:
        return self.debug

    def aim_marker_state(self) -> bool:
        return self.marker

    def aim_marker(self, on: bool) -> bool:
        self.marker = bool(on)
        return True

    def aim_events(self, since: int) -> dict:
        try:
            with open(os.path.join(os.path.dirname(self.path), "fake_aim_events.json"), encoding="utf-8") as f:
                events: list[dict] = json.load(f)
        except (OSError, ValueError):
            events = []
        return {"events": [e for e in events if int(e["n"]) > int(since)], "next": max([int(e["n"]) for e in events] + [0]) + 1}

    def caption(self, test: str, expect: str, result: str) -> None:
        self.captions.append((test, expect, result))
        print("caption:", test, "|", expect, "|", result, flush=True)

    def ctl(self, *args: str, timeout: int = 30, retries: int = 6) -> str:
        if args and args[0] == "teleport" and len(args) > 1:
            self.moved = {"map": args[1], "pos": [0.0, 100.0, 0.0]}
            return f"teleport {args[1]}: ok"
        if args and args[0] == "warp" and len(args) > 3:
            self.moved = dict(self.moved, map=self.status()["map"], pos=[float(v) for v in args[1:4]])
            return "warp: ok"
        if args and args[0] == "raw" and len(args) > 1:
            try:
                req: dict = json.loads(args[1])
            except ValueError:
                req = {}
            self.raw.append(req)
            if req.get("cmd") == "debug" and req.get("state") in ("on", "off"):
                self.debug = req["state"] == "on"
        return '{"ok": true, "radar": {}}'
