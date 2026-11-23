"""Gold Skulltula card: the /gs page and /api/gs/* endpoints, served by placer_server.py on port 8766.

placer_server calls get(placer, path) and post(placer, path, body); each returns (code, body, content type) or None when the
path is not a Gold Skulltula path. Marks: SESSION/notes/panel_gs.json; export: SESSION/notes/gs_positions.json (see gs_logic.py).
"""
import json
import math
import os
import threading
import time

import gs_logic as S
import placer_logic as L
import warp_runner as W

HERE: str = os.path.dirname(os.path.abspath(__file__))
JSON: str = "application/json"


class Skulls:
    def __init__(self, placer, inventory: str | None = None, tour: str | None = None) -> None:
        self.placer = placer
        notes: str = os.path.dirname(placer.points_path)
        self.path: str = os.path.join(notes, S.FILE_NAME)
        self.export_path: str = os.path.join(notes, S.EXPORT_NAME)
        self.gs, self.source = S.load_list(inventory or getattr(placer, "gs_list", None))
        self.tour: dict = S.tour_warps(S.load_json(tour or getattr(placer, "gs_tour", None) or S.TOUR, []))
        self.state: dict = S.load(self.path)
        self.chosen: str | None = None
        self.lock: threading.Lock = threading.Lock()
        self.message: str = ""
        self.saved: str = ""
        self.last_spawn: str = ""

    def save(self) -> None:
        S.save(self.path, self.state)

    def where(self, fresh: bool) -> tuple[str, list[float] | None, float]:
        st: dict = (self.placer.game.status() if fresh else {}) or self.placer.status
        pos = st.get("pos")
        if not st.get("map") or not pos or len(pos) != 3 or st.get("loading"):
            return "", None, 0.0
        return str(st["map"]), [float(v) for v in pos], float(st.get("facing") or 0.0)

    def done(self, msg: str, test: str, expect: str) -> dict:
        self.message = msg
        self.save()
        self.autoexport()
        self.placer.game.caption(test, expect, msg)
        return {"ok": True, "message": msg}

    def autoexport(self) -> None:
        try:
            out, _ = S.export(self.state, self.gs)
            S.save(self.export_path, out)
            self.saved = f"saved {len(out['entries'])} moved, {len(out['skipped'])} left in place: notes/{S.EXPORT_NAME} (written on every change)"
        except Exception as ex:
            self.saved = f"could not save the export: {ex}"

    def refuse(self, msg: str) -> dict:
        self.message = msg
        return {"ok": False, "message": msg}

    def warps(self) -> list[dict]:
        return S.warp_list(self.state, self.gs, self.tour)

    def view(self) -> dict:
        with self.lock:
            map_name, pos, facing = self.where(False)
            common: dict = {"overlay": W.shown(self.placer, "gs"), "travel": W.runner(self.placer).state(), "inventory": len(self.gs),
                            "marked": len({m["hash"] for m in self.state["marks"]}), "skipped": len(self.state["skipped"]),
                            "export": S.EXPORT_NAME, "message": self.message, "saved": self.saved,
                            "aim_on": bool(self.placer.aim_on) and getattr(self.placer, "aim_for", "enemy") == "gs",
                            "aim_other": bool(self.placer.aim_on) and getattr(self.placer, "aim_for", "enemy") != "gs"}
            if pos is None:
                return dict(common, connected=bool(self.placer.status), map=map_name, target=None, here=[], marks=[], nearest=None)
            rec, chosen = S.target(self.gs, map_name, pos, self.chosen)
            here: list[dict] = sorted(({"hash": str(r["hash"]), "name": S.short(r["name"]), "dist": round(S.dist(S.at_on(r, map_name), pos), 1),
                                        "marked": S.mark_of(self.state, r["hash"], map_name) is not None, "skip": str(r["hash"]) in self.state["skipped"]}
                                       for r in S.on_map(self.gs, map_name)), key=lambda x: x["dist"])
            near: dict | None = S.nearest_mark(self.state, map_name, pos)
            tgt: dict | None = None
            if rec is not None:
                m: dict | None = S.mark_of(self.state, rec["hash"], map_name)
                tgt = {"hash": str(rec["hash"]), "name": rec["name"], "short": S.short(rec["name"]), "chosen": chosen,
                       "dist": round(S.dist(S.at_on(rec, map_name), pos), 1), "rule": rec.get("rule_plain") or rec.get("rule", ""),
                       "age": rec.get("age", ""), "night": bool(rec.get("night_only")), "room": rec.get("room"),
                       "appearance": rec.get("appearance", []), "marked": bool(m), "skip": str(rec["hash"]) in self.state["skipped"],
                       "moved": round(S.dist(m["to"], S.at_on(rec, map_name)), 1) if m else None,
                       "both_ages": len(rec.get("files", [])) > 1}
            marks: list[dict] = sorted(({"n": m["n"], "name": S.short(m["name"]), "surface": m["surface"], "dist": round(S.dist(m["to"], pos), 1),
                                         "near": near is not None and m["n"] == near["n"]} for m in S.marks_on(self.state, map_name)), key=lambda r: r["dist"])
            return dict(common, connected=True, map=map_name, place=L.place_name(getattr(self.placer, "places", {}) or {}, map_name), pos=pos,
                        facing=facing, target=tgt, here=here, marks=marks,
                        nearest={"n": near["n"], "name": S.short(near["name"]), "dist": round(S.dist(near["to"], pos), 1)} if near else None,
                        preview={s: S.spot(pos, facing, s) for s in S.SURFACES})

    def mark(self, surface: str) -> dict:
        with self.lock:
            map_name, pos, facing = self.where(True)
            if pos is None:
                return self.refuse("game not ready")
            rec, _ = S.target(self.gs, map_name, pos, self.chosen)
            if rec is None:
                return self.refuse("no Gold Skulltula in this map" if self.gs else "no Gold Skulltula inventory (cache/gs-list.json)")
            try:
                m, how = S.mark(self.state, rec, map_name, pos, facing, surface, time.strftime("%H:%M:%S"))
            except ValueError as ex:
                return self.refuse(str(ex))
            d: float = S.dist(m["to"], S.at_on(rec, map_name))
            return self.done(f"{S.short(rec['name'])}: new spot {how} ({surface}, {d:.1f} m from the original)",
                             f"Gold Skulltula {S.short(rec['name'])} on the {surface}", f"{'faces back at Link' if surface == 'wall' else 'looks where Link looks'}; mark {m['n']}")

    def aim_place(self, hit: dict | None = None, map_name: str = "") -> dict:
        with self.lock:
            st: dict = self.placer.game.status() or self.placer.status
            map_name = map_name or str(st.get("map") or "")
            if not map_name or st.get("loading"):
                return self.refuse("game not ready")
            if hit is None:
                hit = self.placer.game.aim_cast()
                if not hit.get("hit"):
                    return self.refuse("aim: nothing in range (is the aim build running?)")
            hpos: list[float] = [float(v) for v in hit["pos"]]
            rec, _ = S.target(self.gs, map_name, hpos, self.chosen)
            if rec is None:
                return self.refuse("no Gold Skulltula in this map" if self.gs else "no Gold Skulltula inventory (cache/gs-list.json)")
            link = hit.get("link") or st.get("pos")
            try:
                m, how = S.mark_aim(self.state, rec, map_name, hit, link, float(st.get("facing") or 0.0), time.strftime("%H:%M:%S"))
            except ValueError as ex:
                return self.refuse(str(ex))
            d: float = S.dist(m["to"], S.at_on(rec, map_name))
            return self.done(f"{S.short(rec['name'])}: new spot {how} at the crosshair ({m['surface']}, {d:.1f} m from the original)",
                             f"Gold Skulltula {S.short(rec['name'])} at aim", f"{m['surface']} spot; mark {m['n']}")

    def aim_remove(self, hit: dict | None = None, map_name: str = "") -> dict:
        with self.lock:
            st: dict = self.placer.game.status() or self.placer.status
            map_name = map_name or str(st.get("map") or "")
            if not map_name or st.get("loading"):
                return self.refuse("game not ready")
            if hit is None:
                hit = self.placer.game.aim_cast()
                if not hit.get("hit"):
                    return self.refuse("aim: nothing in range (is the aim build running?)")
            near: dict | None = S.nearest_mark_to(self.state, map_name, [float(v) for v in hit["pos"]])
            if near is None:
                return self.refuse("no new Gold Skulltula spot within 2 m of the crosshair")
            S.remove_mark(self.state, near["n"])
            return self.done(f"{S.short(near['name'])}: new spot removed (aimed at)", f"Remove mark {near['n']} at aim", f"{len(S.marks_on(self.state, map_name))} mark(s) left here")

    def aim_apply(self, e: dict) -> None:
        kind: str = e.get("kind", "")
        if kind == "place":
            self.aim_place(e, str(e.get("map") or ""))
        elif kind == "remove":
            self.aim_remove(e, str(e.get("map") or ""))
        elif kind == "undo":
            with self.lock:
                last: dict | None = max(self.state["marks"], key=lambda m: int(m.get("n", 0)), default=None)
                if last:
                    S.remove_mark(self.state, last["n"])
                    self.done(f"{S.short(last['name'])}: last new spot undone", f"Undo mark {last['n']}", "removed")

    def spawn(self) -> dict:
        with self.lock:
            map_name, pos, facing = self.where(True)
            if pos is None:
                return self.refuse("game not ready")
            near: dict | None = None
            rec, _ = S.target(self.gs, map_name, pos, self.chosen)
            if rec is not None:
                near = S.mark_of(self.state, rec["hash"], map_name)
            if near is None:
                near = S.nearest_mark(self.state, map_name, pos)
            if near is not None:
                at, yaw, where = list(near["to"]), float(near.get("facing") or 0.0) + (180.0 if near.get("surface") == "wall" else 0.0), f"new spot #{near['n']}"
            else:
                at, yaw, where = S.spot(pos, facing, "wall"), facing + 180.0, "in front of Link"
            ok, detail = self.placer.game.spawn("Enemy_Skulltula_Gold", at, math.radians(yaw), quick=True)
            self.message = f"Gold Skulltula spawned at {where}" if ok else f"Gold Skulltula did not appear ({where}): {detail or 'the game refused'}"
            self.placer.game.caption("Spawn Skulltula", f"a Gold Skulltula appears at {where}", "appeared" if ok else "did not appear")
            return {"ok": bool(ok), "message": self.message}

    def aim(self, body: dict) -> dict:
        return self.placer.set_aim(body.get("on"), "gs")

    def choose(self, body: dict) -> dict:
        h = body.get("hash")
        self.chosen = str(h) if h else None
        r: dict | None = S.by_hash(self.gs, self.chosen) if self.chosen else None
        self.message = f"marking {S.short(r['name'])}" if r else "marking the nearest Gold Skulltula"
        return {"ok": True, "message": self.message}

    def skip(self) -> dict:
        with self.lock:
            map_name, pos, _ = self.where(False)
            if pos is None:
                return self.refuse("game not ready")
            rec, _ = S.target(self.gs, map_name, pos, self.chosen)
            if rec is None:
                return self.refuse("no Gold Skulltula in this map")
            on: bool = S.toggle_skip(self.state, rec["hash"])
            return self.done(f"{S.short(rec['name'])}: " + ("left in place (its marks are cleared)" if on else "no longer left in place"),
                             f"Leave {S.short(rec['name'])}", "stays where the game has it" if on else "can be marked again")

    def remove_nearest(self) -> dict:
        with self.lock:
            map_name, pos, _ = self.where(True)
            if pos is None:
                return self.refuse("game not ready")
            near: dict | None = S.nearest_mark(self.state, map_name, pos)
            if near is None:
                return self.refuse("no Gold Skulltula marks on this map")
            S.remove_mark(self.state, near["n"])
            return self.done(f"{S.short(near['name'])}: new spot removed", f"Remove mark {near['n']}", f"{len(S.marks_on(self.state, map_name))} mark(s) left here")

    def go(self, body: dict) -> dict:
        w: dict | None = next((x for x in self.warps() if x["key"] == body.get("key")), None)
        if w is None:
            return self.refuse("pick a spot")
        r: dict = W.runner(self.placer).start(w["map"], w["pos"], w["label"])
        self.message = r["message"]
        return r

    def overlay(self, body: dict) -> dict:
        on: bool = W.toggle(self.placer, "gs", body.get("on"))
        self.message = "Gold Skulltula overlay " + ("on: new spots (GS) and the originals (old) show in the game" if on else "off")
        return {"ok": True, "on": on, "message": self.message}

    def export(self) -> dict:
        with self.lock:
            out, notes = S.export(self.state, self.gs)
            S.save(self.export_path, out)
            self.message = f"exported {len(out['entries'])} entr{'y' if len(out['entries']) == 1 else 'ies'}, {len(out['skipped'])} left in place, to notes/{S.EXPORT_NAME}"
            return {"ok": True, "message": self.message, "notes": notes, "json": json.dumps(out, indent=1)}


MAKE: threading.Lock = threading.Lock()


def manager(placer) -> Skulls:
    with MAKE:
        m: Skulls | None = getattr(placer, "gs_manager", None)
        if m is None:
            m = Skulls(placer)
            placer.gs_manager = m
        return m


def radar_lines(placer, map_name: str) -> list[str]:
    if not W.shown(placer, "gs"):
        return []
    m: Skulls = manager(placer)
    pos = (placer.status or {}).get("pos")
    near: dict | None = S.nearest_mark(m.state, map_name, pos) if pos and len(pos) == 3 else None
    return S.radar_lines(m.state, m.gs, map_name, near["n"] if near else None)


def aim_apply(placer, e: dict) -> None:
    manager(placer).aim_apply(e)


def get(placer, path: str) -> tuple[int, bytes | str, str] | None:
    if path in ("/gs", "/gs/"):
        with open(os.path.join(HERE, "gs.html"), "rb") as f:
            return 200, f.read(), "text/html; charset=utf-8"
    if path == "/api/gs/state":
        return 200, json.dumps(manager(placer).view()), JSON
    if path == "/api/gs/warps":
        return 200, json.dumps({"spots": manager(placer).warps()}), JSON
    if path == "/api/gs/export":
        return 200, json.dumps(S.export(manager(placer).state, manager(placer).gs)[0], indent=1), JSON
    return None


def post(placer, path: str, body: dict) -> tuple[int, bytes | str, str] | None:
    m: Skulls = manager(placer)
    acts: dict = {"/api/gs/wall": lambda: m.mark("wall"), "/api/gs/ground": lambda: m.mark("ground"), "/api/gs/target": lambda: m.choose(body),
                  "/api/gs/skip": m.skip, "/api/gs/removenearest": m.remove_nearest, "/api/gs/goto": lambda: m.go(body),
                  "/api/gs/overlay": lambda: m.overlay(body), "/api/gs/export": m.export,
                  "/api/gs/aimplace": m.aim_place, "/api/gs/aimremove": m.aim_remove, "/api/gs/aim": lambda: m.aim(body), "/api/gs/spawn": m.spawn}
    if path not in acts:
        return None
    return 200, json.dumps(acts[path]()), JSON
