"""Puzzle card: the /puzzle page and /api/puzzle/* endpoints for timed silver rupee rooms, served by placer_server.py on port 8766.

placer_server calls get(placer, path) and post(placer, path, body); each returns (code, body, content type) or None when the
path is not a puzzle path. The marks are SESSION/notes/panel_puzzles.json and the export SESSION/notes/timed-gems.json
(see puzzle_logic.py).
"""
import json
import os
import threading
import time

import placer_logic as L
import puzzle_logic as P
import shipped_puzzles as SP
import warp_runner as W

HERE: str = os.path.dirname(os.path.abspath(__file__))
JSON: str = "application/json"


class Puzzles:
    def __init__(self, placer) -> None:
        self.placer = placer
        notes: str = os.path.dirname(placer.points_path)
        self.path: str = os.path.join(notes, P.FILE_NAME)
        self.export_path: str = os.path.join(notes, P.EXPORT_NAME)
        self.state: dict = P.load(self.path)
        self.shipped, self.source = SP.load(getattr(placer, "gems_rs", None))
        self.lock: threading.Lock = threading.Lock()
        self.message: str = ""
        if P.merge_shipped(self.state, self.shipped):
            self.save()

    def save(self) -> None:
        P.save(self.path, self.state)
        P.save(self.export_path, P.export(self.state, self.place))

    def warps(self) -> list[dict]:
        return P.warp_list(self.state)

    def go(self, body: dict) -> dict:
        try:
            pid: int = int(body.get("id"))
        except (TypeError, ValueError):
            return self.refuse("pick a puzzle")
        w: dict | None = next((x for x in self.warps() if x["id"] == pid), None)
        if w is None:
            return self.refuse(f"puzzle {pid} has no spot to warp to")
        r: dict = W.runner(self.placer).start(w["map"], w["pos"], f"Puzzle {pid}")
        self.message = r["message"]
        return r

    def place(self, map_name: str) -> str:
        return L.place_name(getattr(self.placer, "places", {}) or {}, map_name)

    def where(self, fresh: bool) -> tuple[str, list[float] | None, float]:
        st: dict = (self.placer.game.status() if fresh else {}) or self.placer.status
        pos = st.get("pos")
        if not st.get("map") or not pos or len(pos) != 3 or st.get("loading"):
            return "", None, 0.0
        return str(st["map"]), [float(v) for v in pos], float(st.get("facing") or 0.0)

    def row(self, p: dict, kind: str, m: dict, pos: list[float], near_n: int | None) -> dict:
        return {"n": m["n"], "kind": kind, "label": P.label_of(kind, p, m), "dist": round(P.dist(m["pos"], pos), 1), "near": m["n"] == near_n}

    def view(self) -> dict:
        with self.lock:
            map_name, pos, _ = self.where(False)
            if pos is None:
                return {"connected": bool(self.placer.status), "map": map_name, "current": None, "nearest": None, "message": self.message,
                        "overlay": W.shown(self.placer, "puzzles"), "travel": W.runner(self.placer).state(), "total": len(self.state["puzzles"])}
            near: tuple | None = P.nearest_mark(self.state, map_name, pos)
            near_n: int | None = near[2]["n"] if near else None
            cur: dict | None = P.active(self.state, map_name, pos)
            info: dict | None = None
            if cur is not None:
                marks: list[dict] = sorted((self.row(cur, k, m, pos, near_n) for k, m in P.marks_of(cur)), key=lambda r: r["dist"])
                info = {"id": cur["id"], "shipped": cur.get("shipped"), "name": cur.get("name", ""), "chest_hash": cur.get("chest_hash"), "seconds": cur.get("seconds", P.DEFAULT_SECONDS), "marks": marks, "gems": len(cur["gems"]),
                        "has": {k: bool(cur.get(k)) for k in P.SINGLE}, "problems": P.problems(cur), "hints": P.hints(cur),
                        "route": round(P.route(cur), 1)}
            return {"connected": True, "map": map_name, "place": self.place(map_name), "pos": pos, "message": self.message,
                    "current": info, "next_id": P.next_id(self.state), "on_map": len(P.on_map(self.state, map_name)),
                    "nearest": ({"n": near[2]["n"], "kind": near[1], "label": P.label_of(near[1], near[0], near[2]), "puzzle": near[0]["id"],
                                 "dist": round(P.dist(near[2]["pos"], pos), 1)} if near else None),
                    "limits": {"least": P.LEAST, "most": P.MOST, "gems": P.MOST_GEMS}, "export": os.path.basename(self.export_path),
                    "overlay": W.shown(self.placer, "puzzles"), "travel": W.runner(self.placer).state(), "total": len(self.state["puzzles"]),
                    "source": {"file": os.path.basename(self.source) if self.source else None, "count": len(self.shipped)}}

    def overlay(self, body: dict) -> dict:
        on: bool = W.toggle(self.placer, "puzzles", body.get("on"))
        self.message = "puzzle overlay " + ("on: switch, rupees and chest show in the game" if on else "off")
        return {"ok": True, "on": on, "message": self.message}

    def done(self, msg: str, test: str, expect: str) -> dict:
        self.message = msg
        self.save()
        self.placer.game.caption(test, expect, msg)
        return {"ok": True, "message": msg}

    def refuse(self, msg: str) -> dict:
        self.message = msg
        return {"ok": False, "message": msg}

    def mark(self, kind: str) -> dict:
        with self.lock:
            map_name, pos, facing = self.where(True)
            if pos is None:
                return self.refuse("game not ready")
            try:
                p, how, m = P.mark(self.state, map_name, pos, facing, kind, time.strftime("%H:%M:%S"))
            except ValueError as ex:
                return self.refuse(str(ex))
            lab: str = P.label_of(kind, p, m)
            return self.done(f"Puzzle {p['id']}: {lab} {how}", f"Puzzle {p['id']}: {lab} at Link",
                             f"{len(p['gems'])} rupee(s), {int(p['seconds'])} s" + ("; ready to export" if not P.problems(p) else ""))

    def seconds(self, body: dict) -> dict:
        with self.lock:
            map_name, pos, _ = self.where(False)
            if pos is None:
                return self.refuse("game not ready")
            p: dict | None = P.active(self.state, map_name, pos)
            if p is None:
                return self.refuse("no puzzle near: mark the switch first")
            try:
                want: float = float(body["seconds"]) if body.get("seconds") is not None else float(p.get("seconds", P.DEFAULT_SECONDS)) + float(body.get("delta", 0))
            except (TypeError, ValueError):
                return self.refuse("seconds must be a number")
            s: float = P.set_seconds(p, want)
            self.message = f"Puzzle {p['id']}: {int(s)} s"
            self.save()
            return {"ok": True, "message": self.message}

    def new(self) -> dict:
        with self.lock:
            map_name, pos, _ = self.where(False)
            if pos is None:
                return self.refuse("game not ready")
            self.state["current"] = None
            P.drop_empty(self.state)
            p: dict = P.new_puzzle(self.state, map_name, time.strftime("%H:%M:%S"))
            self.message = f"Puzzle {p['id']} started: mark its switch, rupees and chest"
            self.save()
            return {"ok": True, "message": self.message}

    def remove_nearest(self) -> dict:
        with self.lock:
            map_name, pos, _ = self.where(True)
            if pos is None:
                return self.refuse("game not ready")
            near: tuple | None = P.nearest_mark(self.state, map_name, pos)
            if near is None:
                return self.refuse("no puzzle marks on this map")
            lab: str = P.label_of(near[1], near[0], near[2])
            P.remove_mark(self.state, near[2]["n"])
            return self.done(f"Puzzle {near[0]['id']}: {lab} removed", f"Remove {lab}", f"{len(P.marks_of(near[0]))} mark(s) left in puzzle {near[0]['id']}")

    def export(self) -> dict:
        with self.lock:
            out: dict = P.export(self.state, self.place)
            P.save(self.export_path, out)
            bad: int = sum(1 for r in out["rooms"] if r["problems"])
            self.message = f"{len(out['rooms'])} puzzle(s) in notes/{P.EXPORT_NAME} (saved after every change)" + (f"; {bad} not ready" if bad else "")
            return {"ok": True, "message": self.message, "rust": out["rust"], "rooms": out["rooms"]}


MAKE: threading.Lock = threading.Lock()


def manager(placer) -> Puzzles:
    with MAKE:
        m: Puzzles | None = getattr(placer, "puzzle_manager", None)
        if m is None:
            m = Puzzles(placer)
            placer.puzzle_manager = m
        return m


def radar_lines(placer, map_name: str) -> list[str]:
    if not W.shown(placer, "puzzles"):
        return []
    m: Puzzles = manager(placer)
    pos = (placer.status or {}).get("pos")
    near: tuple | None = P.nearest_mark(m.state, map_name, pos) if pos and len(pos) == 3 else None
    return P.radar_lines(m.state, map_name, near[2]["n"] if near else None)


def get(placer, path: str) -> tuple[int, bytes | str, str] | None:
    if path in ("/puzzle", "/puzzle/"):
        with open(os.path.join(HERE, "puzzle.html"), "rb") as f:
            return 200, f.read(), "text/html; charset=utf-8"
    if path == "/api/puzzle/state":
        return 200, json.dumps(manager(placer).view()), JSON
    if path == "/api/puzzle/warps":
        return 200, json.dumps({"puzzles": manager(placer).warps()}), JSON
    if path == "/api/puzzle/export":
        return 200, json.dumps(P.export(manager(placer).state, manager(placer).place), indent=1), JSON
    return None


def post(placer, path: str, body: dict) -> tuple[int, bytes | str, str] | None:
    m: Puzzles = manager(placer)
    acts: dict = {f"/api/puzzle/{k}": (lambda k=k: m.mark(k)) for k in P.KINDS}
    acts.update({"/api/puzzle/seconds": lambda: m.seconds(body), "/api/puzzle/new": m.new,
                 "/api/puzzle/removenearest": m.remove_nearest, "/api/puzzle/export": m.export, "/api/puzzle/overlay": lambda: m.overlay(body),
                 "/api/puzzle/goto": lambda: m.go(body)})
    if path not in acts:
        return None
    return 200, json.dumps(acts[path]()), JSON
