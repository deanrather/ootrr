"""Manage Gate card: the /gate page and /api/gate/* endpoints, served by placer_server.py on port 8766.

placer_server calls get(placer, path) and post(placer, path, body); each returns (code, body, content type) or None when the path
is not a gate path. The gate list is the mod's own gates (gates.rs, shipped_gates.py) merged with SESSION/notes/panel_gates.json
(see gate_logic.py); guards are the placer's own points. Warp to gate uses gate_warps.json (measured spots, moved out of the panel's
places list on 23 Nov) and goto.go() through warp_runner.py; the overlay switch hides the gate layer of the in-game radar.
"""
import json
import os
import sys
import threading
import time

import gate_logic as G
import placer_logic as L
import shipped_gates as SG
import warp_runner as W

HERE: str = os.path.dirname(os.path.abspath(__file__))
JSON: str = "application/json"
sys.path.insert(0, HERE)
import paths as PATHS  # noqa: E402

WARPS: str = os.path.join(PATHS.DATA, "gate_warps.json")


class Gates:
    def __init__(self, placer, rs: str | None = None) -> None:
        self.placer = placer
        self.path: str = os.path.join(os.path.dirname(placer.points_path), G.FILE_NAME)
        self.shipped, self.source = SG.load(rs or getattr(placer, "gates_rs", None))
        self.gates: list[dict] = G.merge(G.load(self.path, []), self.shipped)
        self.measured: dict[int, list[float]] = G.measured_warps(G.load(WARPS, []))
        self.lock: threading.Lock = threading.Lock()
        self.message: str = ""
        G.VANILLA = getattr(placer, "vanilla", None) or {}
        self.pinned: int | None = None
        self.sticky: int | None = None

    def selected(self, map_name: str, pos: list[float]) -> tuple[dict | None, str]:
        g, how = G.select_gate(self.gates, map_name, pos, self.points(), self.sticky, self.pinned)
        self.sticky = g["id"] if g else None
        return g, how

    def select(self, body: dict) -> dict:
        raw = body.get("id")
        if raw in (None, "", "auto"):
            self.pinned = None
            self.message = "gate selection: automatic (the gate whose area Link is in)"
            return {"ok": True, "message": self.message}
        try:
            gid: int = int(raw)
        except (TypeError, ValueError):
            return self.refuse("pick a gate")
        if G.find(self.gates, gid) is None:
            return self.refuse(f"no gate {gid}")
        self.pinned = gid
        self.message = f"Gate {gid} stays selected until you pick another or Auto"
        return {"ok": True, "message": self.message}

    def models(self, body: dict) -> dict:
        on: bool = W.toggle(self.placer, "gate_models", body.get("on"))
        self.message = "gate display models " + ("on: each guard's real model shows at its spot" if on else "off")
        return {"ok": True, "on": on, "message": self.message}

    def save(self) -> None:
        G.save(self.path, G.to_save(self.gates, self.shipped))

    def warps(self) -> list[dict]:
        return G.warp_list(self.gates, self.measured)

    def go(self, body: dict) -> dict:
        try:
            gid: int = int(body.get("id"))
        except (TypeError, ValueError):
            return self.refuse("pick a gate")
        w: dict | None = next((x for x in self.warps() if x["id"] == gid), None)
        if w is None:
            return self.refuse(f"gate {gid} has no spot to warp to")
        r: dict = W.runner(self.placer).start(w["map"], w["pos"], f"Gate {gid}")
        self.message = r["message"]
        return r

    def overlay(self, body: dict) -> dict:
        on: bool = W.toggle(self.placer, "gates", body.get("on"))
        self.message = "gate overlay " + ("on: wall and guard wire boxes show in the game" if on else "off")
        return {"ok": True, "on": on, "message": self.message}

    def live(self) -> list[dict]:
        return list((getattr(self.placer, "live_list", None) or ([], []))[1])

    def points(self) -> list[dict]:
        return list(self.placer.points)

    def tidy(self) -> None:
        if G.prune(self.gates, self.points()):
            self.save()

    def where(self, fresh: bool) -> tuple[str, list[float] | None]:
        st: dict = (self.placer.game.status() if fresh else {}) or self.placer.status
        pos = st.get("pos")
        if not st.get("map") or not pos or len(pos) != 3 or st.get("loading"):
            return "", None
        return str(st["map"]), [float(v) for v in pos]

    def guard_list(self, g: dict, pos: list[float]) -> list[dict]:
        return [{k: e[k] for k in ("source", "n", "i", "label", "dist")} for e in G.guard_entries(g, self.points(), pos)]

    def guard_rows(self, g: dict, pos: list[float]) -> list[dict]:
        rows: list[dict] = []
        for n in g.get("guards", []):
            p: dict | None = G.point_on(self.points(), n)
            if p:
                rows.append({"n": n, "label": G.short(p["name"]), "dist": round(G.dist(p["pos"], pos), 1)})
        return sorted(rows, key=lambda r: r["dist"])

    def gate_info(self, g: dict, pos: list[float], how: str = "auto") -> dict:
        spots: list[dict] = [{"label": G.guard_label(g, True, self.live()), "dist": round(G.dist(s, pos), 1)} for s in g.get("spots", [])]
        return {"id": g["id"], "left": g.get("left"), "right": g.get("right"), "dist": round(G.gate_dist(g, pos), 1),
                "width": round(G.dist(g["left"], g["right"]), 1) if G.complete(g) else None, "guards": self.guard_rows(g, pos), "rows": self.guard_list(g, pos),
                "area": round(G.area_dist(g, pos, self.points()), 1), "how": how, "name": g.get("name", ""), "shipped": bool(g.get("shipped")), "edited": G.edited(g, self.shipped), "spots": spots}

    @staticmethod
    def target(p: dict | None, pos: list[float], gates: list[dict]) -> dict | None:
        if not p:
            return None
        other: dict | None = G.guard_of(gates, p["n"])
        return {"n": p["n"], "label": G.short(p["name"]), "dist": round(G.dist(p["pos"], pos), 1), "gate": other["id"] if other else None}

    @staticmethod
    def pick_info(e: dict | None) -> dict | None:
        return {k: e[k] for k in ("source", "n", "i", "label", "dist")} if e else None

    def view(self) -> dict:
        with self.lock:
            self.tidy()
            map_name, pos = self.where(False)
            common: dict = {"overlay": W.shown(self.placer, "gates"), "models": W.shown(self.placer, "gate_models"), "pinned": self.pinned, "travel": W.runner(self.placer).state(),
                            "source": {"file": os.path.basename(self.source) if self.source else None, "count": len(self.shipped)},
                            "total": len(self.gates)}
            if pos is None:
                return dict(common, connected=bool(self.placer.status), map=map_name, near=None, message=self.message)
            pts: list[dict] = self.points()
            near, how = self.selected(map_name, pos)
            here: list[dict] = [g for g in G.on_map(self.gates, map_name) if G.posts(g)]
            any_gate: dict | None = min(here, key=lambda g: G.area_dist(g, pos, pts)) if here else None
            gates_here: list[dict] = [{"id": g["id"], "label": G.gate_label(g)} for g in sorted(here, key=lambda x: int(x["id"]))]
            waiting: list[dict] = [g for g in G.on_map(self.gates, map_name) if not G.complete(g) and g is not near and G.gate_dist(g, pos) <= G.PAIR_REACH]
            return {**common, "connected": True, "map": map_name, "place": L.place_name(getattr(self.placer, "places", {}) or {}, map_name), "pos": pos, "message": self.message,
                    "near": self.gate_info(near, pos, how) if near else None,
                    "closest": {"id": any_gate["id"], "dist": round(G.area_dist(any_gate, pos, pts), 1)} if any_gate and not near else None,
                    "how": how, "gates_here": gates_here,
                    "waiting": [{"id": g["id"], "has": "left" if g.get("left") else "right", "dist": round(G.gate_dist(g, pos), 1)} for g in waiting],
                    "mark": self.target(G.mark_target(near, pts, map_name, pos), pos, self.gates),
                    "unmark": self.pick_info(G.guard_pick(near, pts, pos)), "model_note": self.model_note(near, map_name),
                    "on_map": len(G.on_map(self.gates, map_name)), "near_radius": G.GATE_NEAR,
                    "will": {s: G.place_preview(self.gates, map_name, pos, s) for s in G.SIDES}}

    def model_note(self, g: dict | None, map_name: str) -> str:
        if g is None or not W.shown(self.placer, "gate_models"):
            return ""
        kinds: list[str] = sorted({str(e["name"]) for e in G.guard_entries(g, self.points()) if str(e["name"]).startswith("Enemy_")})
        refused: set = set(getattr(self.placer, "ph_skipped", []) or [])
        bad: list[str] = [k for k in kinds if G.model_kind(k, L.NO_DICTIONARY, refused) is None]
        stood: list[str] = [k for k in kinds if k in L.NO_DICTIONARY and k not in bad]
        out: list[str] = []
        if bad:
            out.append("no display model for " + ", ".join(G.short(k) for k in bad) + ": wire box only")
        if stood:
            out.append(", ".join(G.short(k) for k in stood) + " guards shown with the miniboss Lizalfos model (the closest display model; the game has none for plain Lizalfos)")
        return "; ".join(out)

    def done(self, msg: str, test: str, expect: str) -> dict:
        self.message = msg
        self.save()
        self.placer.game.caption(test, expect, msg)
        return {"ok": True, "message": msg}

    def refuse(self, msg: str) -> dict:
        self.message = msg
        return {"ok": False, "message": msg}

    def place(self, side: str) -> dict:
        with self.lock:
            map_name, pos = self.where(True)
            if pos is None:
                return self.refuse("game not ready")
            g, how = G.place_post(self.gates, map_name, pos, side, time.strftime("%H:%M:%S"))
            tag: str = "L" if side == "left" else "R"
            if G.complete(g):
                expect: str = f"wall {G.dist(g['left'], g['right']):.1f} m wide; {len(g['guards'])} guard(s)"
            else:
                expect = f"now place the {'right' if side == 'left' else 'left'} post"
            word: str = {"new": "new gate, left post" if side == "left" else "new gate, right post", "placed": f"{side} post placed", "moved": f"{side} post moved"}[how]
            return self.done(f"Gate {g['id']}: {word}", f"Gate {g['id']} {tag} at Link", expect)

    def remove(self) -> dict:
        with self.lock:
            map_name, pos = self.where(True)
            if pos is None:
                return self.refuse("game not ready")
            g, _ = self.selected(map_name, pos)
            if g is None:
                return self.refuse(f"no gate within {int(G.GATE_NEAR)} m")
            freed: int = len(g.get("guards", []))
            if g.get("shipped"):
                if not G.edited(g, self.shipped):
                    return self.refuse(f"Gate {g['id']} ships with the mod (gates.rs): move its posts here, remove it in gates.rs")
                G.revert(self.gates, self.shipped, g["id"])
                return self.done(f"Gate {g['id']} put back as gates.rs has it (posts and guard spots)", f"Revert gate {g['id']}",
                                 "posts back where the mod has them")
            self.gates = G.remove_gate(self.gates, g["id"])
            return self.done(f"Gate {g['id']} removed, {freed} guard mark(s) cleared", f"Remove gate {g['id']}",
                             f"posts gone; the {freed} spawn point(s) stay as plain points")

    def mark(self, on: bool) -> dict:
        with self.lock:
            self.tidy()
            map_name, pos = self.where(True)
            if pos is None:
                return self.refuse("game not ready")
            g, _ = self.selected(map_name, pos)
            if g is None:
                return self.refuse(f"no gate within {int(G.GATE_NEAR)} m")
            pts: list[dict] = self.points()
            if on:
                p: dict | None = G.mark_target(g, pts, map_name, pos)
                if p is None:
                    return self.refuse(f"no unmarked spawn point within {int(G.GUARD_REACH)} m")
                old: dict | None = G.mark_guard(self.gates, g["id"], p["n"])
                extra: str = f" (taken from gate {old['id']})" if old else ""
                return self.done(f"Gate {g['id']}: {G.short(p['name'])} #{p['n']} is a guard{extra}", f"Mark guard for gate {g['id']}",
                                 f"{len(g['guards'])} guard(s) now")
            rows: list[dict] = G.guard_entries(g, pts, pos)
            if not rows:
                return self.refuse(f"gate {g['id']} has no guards")
            e: dict | None = G.guard_pick(g, pts, pos)
            if e is None:
                return self.refuse(f"gate {g['id']}'s nearest guard is {rows[0]['label']} {rows[0]['dist']} m away: walk within {int(G.GUARD_DELETE_REACH)} m or press its x in the list")
            return self.drop(g, e)

    def drop(self, g: dict, e: dict) -> dict:
        if e["source"] == "marked":
            G.unmark_guard(self.gates, g["id"], e["n"])
            return self.done(f"Gate {g['id']}: {e['label']} #{e['n']} is removed from the gate's guards", f"Unmark guard for gate {g['id']}", f"{len(g['guards'])} guard(s) left")
        G.remove_spot(self.gates, g["id"], e["i"])
        return self.done(f"Gate {g['id']}: removed the {e['label']} guard spot from the gate design ({len(g.get('spots', []))} left; Undo edits puts it back; the export to gates.rs carries the edit)",
                         f"Unmark guard spot of gate {g['id']}", f"{len(g.get('spots', []))} guard spot(s) left")

    def delete_guard(self, body: dict) -> dict:
        with self.lock:
            self.tidy()
            g: dict | None = G.find(self.gates, int(body.get("id") or 0))
            if g is None:
                return self.refuse("no such gate")
            rows: list[dict] = G.guard_entries(g, self.points())
            src: str = str(body.get("source", ""))
            key = body.get("n") if src == "marked" else body.get("i")
            e: dict | None = next((x for x in rows if x["source"] == src and x["n" if src == "marked" else "i"] == key), None)
            if e is None:
                return self.refuse("that guard is already gone: the list is refreshing")
            return self.drop(g, e)


MAKE: threading.Lock = threading.Lock()


def manager(placer) -> Gates:
    with MAKE:
        m: Gates | None = getattr(placer, "gate_manager", None)
        if m is None:
            m = Gates(placer)
            placer.gate_manager = m
        return m


def radar_args(placer, map_name: str) -> dict:
    m: Gates = manager(placer)
    if not W.shown(placer, "gates"):
        return {"extra": [], "tags": {}}
    boxes: dict = getattr(placer, "actor_boxes", {}) or {}
    return {"extra": G.radar_lines(m.gates, map_name, False) + G.box_lines(m.gates, map_name, boxes), "tags": G.guard_tags(m.gates, map_name),
            "box_ns": G.guard_ns(m.gates, map_name), "all_boxes": boxes}


def placeholder_lines(placer, map_name: str) -> list[str]:
    m: Gates = manager(placer)
    if not W.shown(placer, "gate_models"):
        return []
    return G.placeholder_lines(m.gates, m.points(), map_name, getattr(L, "NO_DICTIONARY", ()), set(getattr(placer, "ph_skipped", []) or []))


def get(placer, path: str) -> tuple[int, bytes | str, str] | None:
    if path in ("/gate", "/gate/"):
        with open(os.path.join(HERE, "gate.html"), "rb") as f:
            return 200, f.read(), "text/html; charset=utf-8"
    if path == "/api/gate/state":
        return 200, json.dumps(manager(placer).view()), JSON
    if path == "/api/gate/warps":
        return 200, json.dumps({"gates": manager(placer).warps()}), JSON
    return None


def post(placer, path: str, body: dict) -> tuple[int, bytes | str, str] | None:
    m: Gates = manager(placer)
    acts: dict = {"/api/gate/markguard": lambda: m.mark(True), "/api/gate/unmarkguard": lambda: m.mark(False), "/api/gate/delguard": lambda: m.delete_guard(body),
                  "/api/gate/left": lambda: m.place("left"), "/api/gate/right": lambda: m.place("right"), "/api/gate/remove": m.remove,
                  "/api/gate/goto": lambda: m.go(body), "/api/gate/overlay": lambda: m.overlay(body),
                  "/api/gate/select": lambda: m.select(body), "/api/gate/models": lambda: m.models(body)}
    if path not in acts:
        return None
    return 200, json.dumps(acts[path]()), JSON
