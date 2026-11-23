"""Enemy placer: a phone page (http://<host>:8766/placer) to mark spawn points and spawn / kill enemies.

    python -I placer_server.py [SESSION] [--port 8766] [--lan] [--key KEY] [--fake]

SESSION defaults to $PANEL_SESSION, session in config.json, or devtools/session. Spawn points: SESSION/notes/spawnpoints.json.
Runs beside the playtest panel and needs no panel restart; it talks to the game through longshot_ctl.py like killkey.py.
Pages: /placer (enemies), /gate (Manage gate, gate_routes.py), /puzzle (timed rupees, puzzle_routes.py), /gs (Gold Skulltula
spots, gs_routes.py); shared look in ui.css and ui.js. Each card's overlay switch (warp_runner.py, notes/panel_overlay.json) says
whether its layer goes into the in-game radar. --fake runs against fake_game.py (no game, no Longshot): for screenshots and render
checks, use a scratch SESSION.
"""
import argparse
import ipaddress
import json
import math
import os
import secrets
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gate_routes as GR
import gs_routes as SR
import minimap_fit as MF
import placer_logic as L
import puzzle_routes as PR
import warp_runner as W

import paths as PATHS

CTL: list[str] = [sys.executable, "-I", PATHS.longshot_ctl()]
DEFAULT_SESSION: str = PATHS.session()


def last_json(text: str) -> dict:
    for line in reversed(text.strip().splitlines()):
        line = line.strip()
        if line.startswith("{"):
            try:
                return json.loads(line)
            except ValueError:
                pass
    return {}


class Game:
    def __init__(self, session: str, run: str | None = None, sock: str | None = None) -> None:
        self.session: str = session
        self.run: str | None = run
        self.sock: str | None = sock

    def target(self) -> list[str]:
        if self.sock:
            return ["--sock", self.sock]
        return ["--run", self.run or self.run_dir()]

    def run_dir(self) -> str:
        root: str = os.path.join(self.session, "runs")
        dirs: list[str] = [os.path.join(root, d) for d in os.listdir(root) if os.path.isdir(os.path.join(root, d))]
        return max(dirs, key=os.path.getmtime)

    def ctl(self, *args: str, timeout: int = 30, retries: int = 6) -> str:
        out: str = ""
        for _ in range(retries):
            r = subprocess.run(CTL + self.target() + list(args), capture_output=True, text=True, timeout=timeout)
            out = r.stdout + r.stderr
            if "not taken" not in out and "another edit is waiting" not in out:
                break
            time.sleep(0.4)
        return out

    def status(self) -> dict:
        try:
            data: dict = last_json(self.ctl("--json", "status", retries=2))
        except Exception:
            return {}
        return data.get("game") or {}

    def enemies(self, scan: bool = False) -> tuple[list[float], list[dict]]:
        args: list[str] = ["--json", "enemies"] if scan else ["--json", "enemies", "--no-scan"]
        data: dict = last_json(self.ctl(*args, timeout=90))
        return data.get("link", [0.0, 0.0, 0.0]), data.get("enemies", [])

    def spawn(self, name: str, pos: list[float], yaw: float, quick: bool = False) -> tuple[bool, str]:
        out: str = self.ctl("--json", "spawn", name, "--at", str(pos[0]), str(pos[1]), str(pos[2]), "--yaw", f"{yaw:.3f}", *(["--quick"] if quick else []))
        data: dict = last_json(out)
        if "appeared" in data:
            return bool(data["appeared"]), str(data.get("detail", ""))
        return ("appeared" in out and "not available" not in out), out.strip()[-120:]

    def damage(self, enemy_id: object) -> None:
        out: str = self.ctl("damage", str(enemy_id), "999")
        if "refused" in out or "no readable life" in out:
            raise RuntimeError(out.strip())

    def aim_cast(self) -> dict:
        data: dict = last_json(self.ctl("--json", "aim", retries=2))
        return data.get("aim") or {}

    def aim_marker(self, on: bool) -> bool:
        out: str = self.ctl("--json", "aim", "marker", "on" if on else "off", retries=2)
        return '"marker"' in out

    def aim_events(self, since: int) -> dict:
        return last_json(self.ctl("--json", "aim", "events", "--since", str(int(since)), retries=2))

    def debug_state(self) -> bool | None:
        data: dict = last_json(self.ctl("--json", "debug", retries=2))
        return bool(data["debug"]) if "debug" in data else None

    def aim_marker_state(self) -> bool | None:
        data: dict = last_json(self.ctl("--json", "aim", "events", "--since", str(10 ** 9), retries=2))
        m = data.get("marker")
        return bool(m.get("on")) if isinstance(m, dict) and "on" in m else None

    def caption(self, test: str, expect: str, result: str) -> None:
        try:
            self.ctl("caption", "--test", test, "--expect", expect, "--result", result, retries=2)
        except Exception:
            pass


AIM_POLL: float = 0.5
RADAR_RESEND: float = 60.0
RADAR_RETRY: float = 15.0


def short(name: str) -> str:
    return name.replace("Enemy_", "")


class Placer:
    AUTOSTART: bool = True

    def __init__(self, session: str, game: Game) -> None:
        self.game: Game = game
        notes: str = os.path.join(session, "notes")
        os.makedirs(notes, exist_ok=True)
        self.points_path: str = os.path.join(notes, "spawnpoints.json")
        self.state_path: str = os.path.join(notes, "placer_state.json")
        self.fits_path: str = os.path.join(notes, "minimap_fits.json")
        self.portraits: str = os.path.join(HERE, "portraits")
        self.lock: threading.Lock = threading.Lock()
        self.points: list[dict] = self.load(self.points_path, [])
        saved: dict = self.load(self.state_path, {})
        self.unavailable: dict[str, list[str]] = saved.get("unavailable", {})
        self.selected: str | None = saved.get("selected")
        self.used: dict[str, int] = {str(k): int(v) for k, v in (saved.get("used") or {}).items()}
        self.sync_inline: bool = False
        self.aim_on: bool = False
        self.aim_for: str = "enemy"
        self.aim_wake: threading.Event = threading.Event()
        self.aim_since: int = 0
        self.placed: list[int] = []
        self.high: int = max([int(saved.get("high", 0))] + [int(p.get("n", 0)) for p in self.points])
        self.tracked: dict[int, object] = {}
        self.status: dict = {}
        self.message: str = ""
        self.live: dict[str, int] = {}
        self.live_list: tuple = ([0.0, 0.0, 0.0], [])
        self.live_ok: bool = False
        self.vanilla: dict[str, dict] = L.load_vanilla()
        self.hp: dict[str, float] = L.load_hp()
        self.catalog: list[tuple[str, str]] = L.by_hp(L.CATALOG, self.hp)
        self.arrows: bool = False
        self.ph_sent: tuple = ("", 0.0, True)
        self.ph_skipped: list[str] = []
        self.actor_boxes: dict[str, list] = L.load_boxes()
        self.places: dict[str, str] = L.load_places()
        if self.AUTOSTART and not os.environ.get("PLACER_NO_THREADS"):
            threading.Thread(target=self.poll, daemon=True).start()
            threading.Thread(target=self.aim_loop, daemon=True).start()

    @staticmethod
    def load(path: str, default):
        try:
            with open(path, encoding="utf-8-sig") as f:
                return json.load(f)
        except (OSError, ValueError):
            return default

    @staticmethod
    def save(path: str, data) -> None:
        tmp: str = path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=1)
        os.replace(tmp, path)

    def poll(self) -> None:
        tick: int = 0
        while True:
            try:
                self.status = self.game.status()
            except Exception:
                self.status = {}
            self.push_radar()
            if tick % 4 == 0:
                self.reconcile()
            if tick % 3 == 0:
                try:
                    link, enemies = self.game.enemies()
                    if enemies:
                        self.live_list = (link, enemies)
                        self.live_ok = True
                    else:
                        self.live_ok = False
                except Exception:
                    self.live_ok = False
            tick += 1
            time.sleep(1.0)

    def aim_loop(self) -> None:
        while True:
            if self.aim_on:
                try:
                    self.aim_poll()
                except Exception:
                    pass
            self.aim_wake.wait(AIM_POLL)

    def push_radar(self) -> None:
        self.push_marks()
        self.push_placeholders(self.status.get("map") or "")

    def push_marks(self) -> None:
        map_name: str = self.status.get("map") or ""
        if not map_name:
            return
        key: str | None = L.map_key(self.vanilla, map_name, self.status.get("age", ""))
        args: dict = GR.radar_args(self, map_name)
        args["extra"] = list(args.get("extra") or []) + PR.radar_lines(self, map_name) + SR.radar_lines(self, map_name)
        pos = self.status.get("pos")
        mine: bool = W.shown(self, "placer")
        near: dict | None = L.nearest_point(self.points, map_name, pos) if pos and len(pos) == 3 and mine else None
        marks: str = L.radar_marks(self.vanilla.get(key) if key and mine else None, list(self.points) if mine else [], map_name, self.arrows, near=near["n"] if near else None,
                              boxes=self.actor_boxes if W.shown(self, "boxes") else None, **args)
        fit_reqs: list[dict] = MF.fit_requests(map_name, self.load(self.fits_path, {}), self.status.get("pos"))
        sig: str = marks + "\n" + json.dumps(fit_reqs)
        sent: tuple = getattr(self, "radar_sent", ("", "", 0.0))
        if (map_name, sig) == sent[:2] and time.time() - sent[2] < RADAR_RESEND:
            return
        try:
            out: str = self.game.ctl("raw", json.dumps(L.radar_request(map_name, marks)), timeout=10, retries=1)
            for req in fit_reqs:
                self.game.ctl("raw", json.dumps(req), timeout=10, retries=1)
        except Exception:
            return
        self.radar_sent = (map_name, sig, time.time())
        if '"radar"' not in out:
            self.radar_sent = (map_name, sig, time.time() - RADAR_RESEND + RADAR_RETRY)

    def push_placeholders(self, map_name: str) -> None:
        if not map_name:
            return
        on: bool = W.shown(self, "models") or W.shown(self, "gate_models")
        lines: list[str] = []
        if W.shown(self, "models"):
            lines += L.placeholder_spots(list(self.points), map_name).split(chr(10))
        lines += GR.placeholder_lines(self, map_name)
        spots: str = chr(10).join(dict.fromkeys(x for x in lines if x))
        sig: str = map_name + chr(10) + spots + chr(10) + str(on)
        last_sig, last_at, ever_on = self.ph_sent
        if not on and not ever_on:
            return
        if sig == last_sig and time.time() - last_at < RADAR_RESEND:
            return
        try:
            out: str = self.game.ctl("raw", json.dumps(L.placeholder_request(map_name, spots, on)), timeout=10, retries=1)
        except Exception:
            return
        ok: bool = '"placeholders"' in out
        if ok:
            try:
                self.ph_skipped = list(last_json(out)["placeholders"].get("skipped_kinds", []))
            except Exception:
                pass
        self.ph_sent = (sig, time.time() if ok else time.time() - RADAR_RESEND + RADAR_RETRY, ever_on or on)

    def reconcile(self) -> None:
        if not self.status:
            return
        want: bool = W.any_on(self)
        try:
            have: bool | None = getattr(self.game, "debug_state", lambda: None)()
            self.game_debug = have
            if have is not None and have != want:
                self.overlay_sync(want)
            marker: bool | None = getattr(self.game, "aim_marker_state", lambda: None)()
            if marker is not None and marker != self.aim_on:
                if marker:
                    self.aim_since = int(self.game.aim_events(10 ** 9).get("next", 1)) - 1
                self.aim_on = marker
        except Exception:
            pass

    def overlay_sync(self, any_on: bool) -> None:
        self.debug_on = any_on
        self.game_debug = any_on
        req: str = json.dumps({"cmd": "debug", "state": "on" if any_on else "off"})
        self.radar_sent = ("", "", 0.0)
        self.ph_sent = ("", 0.0, self.ph_sent[2])

        def send() -> None:
            try:
                self.game.ctl("raw", req, timeout=10, retries=1)
            except Exception:
                pass
        if self.sync_inline:
            send()
        else:
            threading.Thread(target=send, daemon=True).start()

    def set_overlay(self, on) -> dict:
        shown: bool = W.toggle(self, "placer", on)
        self.message = "placer overlay " + ("on: spawn points and the vanilla enemies show in the game" if shown else "off")
        self.radar_sent = ("", "", 0.0)
        return {"ok": True, "message": self.message, "overlay": shown}

    def save_all(self) -> None:
        self.save(self.points_path, self.points)
        self.save(self.state_path, {"unavailable": self.unavailable, "high": self.high, "selected": self.selected, "used": self.used})

    def portrait_names(self) -> set[str]:
        try:
            return {os.path.splitext(f)[0] for f in os.listdir(self.portraits) if f.lower().endswith(".png")}
        except OSError:
            return set()

    def view(self) -> dict:
        st: dict = self.status
        map_name: str = st.get("map") or ""
        pos: list[float] = st.get("pos") or [0.0, 0.0, 0.0]
        counts: dict[str, int] = L.nearby_counts(self.points, map_name, pos) if st else {}
        hits: dict[str, dict] = L.hit_points(self.points, map_name, pos) if st else {}
        bad: list[str] = self.unavailable.get(map_name, [])
        have: set[str] = self.portrait_names()
        key: str | None = L.map_key(self.vanilla, map_name, st.get("age", ""))
        stats: dict = L.room_stats(self.vanilla.get(key) if key else None, self.points, map_name, pos) if st else {"kinds": {}}
        kinds: dict[str, int] = stats.get("kinds", {})
        entry: dict | None = self.vanilla.get(key) if key else None
        if stats.get("room") is not None:
            counts = stats.get("ours", {})
        live: dict[str, int] = L.live_in_room(entry, self.live_list[1], stats.get("room"), self.live_list[0], L.NEAR_RADIUS)
        buttons: list[dict] = []
        used: dict[str, float] = L.last_used(self.points, self.used)
        for n, lab in self.catalog:
            text, wip = L.button_label(lab)
            buttons.append({"name": n, "label": lab, "text": text, "wip": wip, "full": L.full_name(n), "hp": self.hp.get(n),
                            "count": counts.get(n, 0), "live": live.get(n, 0), "hit": False, "off": n in bad,
                            "img": short(n) in have, "van": kinds.get(n, 0), "hab": L.habitat(n),
                            "size": L.box_size(self.actor_boxes, n), "used": used.get(n, 0)})
        near: dict | None = L.nearest_point(self.points, map_name, pos) if st else None
        sel: str | None = self.selected or (L.last_name(self.points, map_name) if st else None)
        cur: int = stats.get("vanilla", 0) + stats.get("added", 0)
        tgt: int = int(L.MULT_CAP) * stats.get("vanilla", 0)
        alive: bool = bool(near and L.alive_for(near, self.live_list[1], self.tracked.get(near["n"])))
        return {"nearest": ({"n": near["n"], "name": near["name"], "label": short(near["name"]), "dist": round(L.dist(near["pos"], pos), 1),
                             "rise": round(float(near["pos"][1]) - float(pos[1]), 1), "alive": alive, "hab": L.habitat(near["name"]),
                             "img": short(near["name"]) in have} if near else None),
                "selected": ({"name": sel, "label": short(sel), "text": L.button_label(dict(L.CATALOG).get(sel, short(sel)))[0], "full": L.full_name(sel),
                      "img": short(sel) in have, "hab": L.habitat(sel), "chosen": bool(self.selected)} if sel else None),
                "aim_on": self.aim_on, "aim_for": self.aim_for, "debug": getattr(self, "game_debug", None), "current": cur, "target": tgt, "habitats": L.HABITAT_KINDS, "kill_radius": int(L.NEAR_RADIUS),
                "met": tgt > 0 and cur >= tgt,
                "connected": bool(st), "map": map_name, "pos": pos, "here": len(L.on_map(self.points, map_name)),
                "buttons": buttons, "message": self.message, "overlay": W.shown(self, "placer"), "boxes": W.shown(self, "boxes"), "models": W.shown(self, "models"),
                "place": L.place_name(self.places, map_name), "room": stats.get("room"), "mult": stats.get("mult", ""),
                "vanilla": stats.get("vanilla", 0), "header": L.header_text(L.place_name(self.places, map_name), stats.get("room"), stats.get("vanilla", 0), stats.get("added", 0)), "added": stats.get("added", 0), "known": key is not None, "live_ok": self.live_ok}

    def press(self, name: str) -> dict:
        with self.lock:
            st: dict = self.game.status() or self.status
            map_name: str = st.get("map") or ""
            pos = st.get("pos")
            if not map_name or not pos or len(pos) != 3 or st.get("loading"):
                self.message = "game not ready"
                return {"ok": False, "message": self.message}
            if name not in {n for n, _ in L.CATALOG}:
                return {"ok": False, "message": "unknown enemy"}
            self.selected = name
            self.used[name] = max(self.used.values(), default=0) + 1
            self.save_all()
            if self.aim_on:
                self.aim_for = "enemy"
                self.message = f"selected {short(name)}: the next aim placement uses it"
                return {"ok": True, "message": self.message}
            return self.add(name, map_name, pos, float(st.get("facing") or 0.0))

    def kind_for_aim(self, map_name: str) -> str | None:
        return self.selected or L.last_name(self.points, map_name)

    def place_at_aim(self, hit: dict | None = None, map_name: str = "") -> dict:
        with self.lock:
            return self.aim_place(hit, map_name)

    def aim_hit(self, hit: dict | None, label: str) -> dict | None:
        if hit is None:
            hit = self.game.aim_cast()
            if not hit.get("hit"):
                self.message = "aim: nothing in range (is the aim build running?)"
                self.game.caption(label, "acts where you aim", self.message)
                return None
        return hit

    def aim_place(self, hit: dict | None = None, map_name: str = "") -> dict:
        st: dict = self.game.status() or self.status
        map_name = map_name or st.get("map") or ""
        if not map_name or st.get("loading"):
            self.message = "game not ready"
            return {"ok": False, "message": self.message}
        name: str | None = self.kind_for_aim(map_name)
        if not name:
            self.message = "no enemy selected: tap an enemy on the phone first"
            self.game.caption("Place at aim", "places the selected enemy's marker where you aim", self.message)
            return {"ok": False, "message": self.message}
        hit = self.aim_hit(hit, "Place at aim")
        if hit is None:
            return {"ok": False, "message": self.message}
        return self.add(name, map_name, hit["pos"], float(st.get("facing") or 0.0), aim=hit, link=L.aim_link(hit, st.get("pos")))

    def remove_at_aim(self, hit: dict | None = None, map_name: str = "") -> dict:
        with self.lock:
            return self.aim_remove(hit, map_name)

    def aim_remove(self, hit: dict | None = None, map_name: str = "") -> dict:
        st: dict = self.game.status() or self.status
        map_name = map_name or st.get("map") or ""
        if not map_name or st.get("loading"):
            self.message = "game not ready"
            return {"ok": False, "message": self.message}
        hit = self.aim_hit(hit, "Remove aimed at")
        if hit is None:
            return {"ok": False, "message": self.message}
        point: dict | None = L.nearest_to_hit(self.points, map_name, [float(v) for v in hit["pos"]])
        if not point:
            self.message = f"no marker within {L.AIM_REMOVE_RADIUS} m of where you aim"
            self.game.caption("Remove aimed at", f"removes the marker nearest the crosshair (within {L.AIM_REMOVE_RADIUS} m)", self.message)
            return {"ok": False, "message": self.message}
        return self.remove(point)

    def aim_nudge(self, d: list[float]) -> dict:
        n: int | None = self.last_placed()
        p: dict | None = next((q for q in self.points if q.get("n") == n), None)
        if not p:
            self.message = "nothing placed to nudge"
            return {"ok": False, "message": self.message}
        new: dict = L.moved(p, d)
        self.points = [new if q.get("n") == n else q for q in self.points]
        self.save_all()
        self.message = f"nudged {short(p['name'])} #{n} by " + " ".join(f"{float(v):+.2f}" for v in d)
        return {"ok": True, "message": self.message}

    def last_placed(self) -> int | None:
        have: set[int] = {int(q.get("n", 0)) for q in self.points}
        while self.placed and self.placed[-1] not in have:
            self.placed.pop()
        return self.placed[-1] if self.placed else None

    def aim_undo(self) -> dict:
        n: int | None = self.last_placed()
        p: dict | None = next((q for q in self.points if q.get("n") == n), None)
        if not p:
            self.message = "nothing placed to undo"
            return {"ok": False, "message": self.message}
        self.placed.pop()
        return self.remove(p)

    def aim_apply(self, e: dict) -> None:
        kind: str = e.get("kind", "")
        if self.aim_for == "gs":
            SR.aim_apply(self, e)
            return
        if kind == "place":
            self.aim_place(e, str(e.get("map") or ""))
        elif kind == "nudge":
            self.aim_nudge([float(v) for v in (e.get("d") or [0, 0, 0])])
        elif kind == "undo":
            self.aim_undo()
        elif kind == "remove":
            self.aim_remove(e, str(e.get("map") or ""))

    def aim_poll(self) -> int:
        r: dict = self.game.aim_events(self.aim_since)
        if "next" in r and int(r["next"]) - 1 < self.aim_since:
            self.aim_since = 0
        done: int = 0
        for e in sorted(r.get("events") or [], key=lambda x: int(x.get("n", 0))):
            if int(e.get("n", 0)) <= self.aim_since:
                continue
            self.aim_since = int(e["n"])
            with self.lock:
                self.aim_apply(e)
            done += 1
        return done

    def set_aim(self, on, target: str = "enemy") -> dict:
        if on is None:
            want: bool = not (self.aim_on and self.aim_for == target)
        else:
            want = bool(on)
        ok: bool = self.game.aim_marker(want)
        if not ok:
            self.message = "aim mode: the game did not answer (needs the aim build of Longshot)"
            return {"ok": False, "message": self.message, "aim_on": self.aim_on}
        if want:
            r: dict = self.game.aim_events(10 ** 9)
            self.aim_since = int(r.get("next", 1)) - 1
            self.aim_for = target
        self.aim_on = want
        what: str = "Gold Skulltula spots" if target == "gs" else "enemy markers"
        self.message = f"aim mode on for {what}: tap B (put away) to place, hold B to remove the nearest" if want else "aim mode off"
        return {"ok": True, "message": self.message, "aim_on": want, "aim_for": self.aim_for}

    def remove(self, point: dict) -> dict:
        killed: bool = False
        tried: bool = False
        link, enemies = self.game.enemies()
        live = L.alive_for(point, enemies, self.tracked.get(point["n"]))
        if not live:
            link, enemies = self.game.enemies(scan=True)
            live = L.alive_for(point, enemies, self.tracked.get(point["n"]))
        if live:
            tried = True
            try:
                self.game.damage(live["id"])
                killed = True
            except Exception:
                killed = False
        self.tracked.pop(point["n"], None)
        self.points = L.remove_point(self.points, point["n"])
        self.save_all()
        self.message = f"removed {short(point['name'])} point" + (" and killed it" if killed else " (could not kill it: needs the new Longshot)" if tried else " (no live one near it)")
        left: int = len([p for p in self.points if p["map"] == point["map"] and p["name"] == point["name"]])
        self.game.caption(f"Removed spawn point: {short(point['name'])}", f"{left} {short(point['name'])} points left on this map",
                          "removed" + (" + killed" if killed else ""))
        return {"ok": True, "message": self.message}

    def add(self, name: str, map_name: str, pos: list[float], facing: float, face_link: bool = False, aim: dict | None = None, link: list[float] | None = None) -> dict:
        key: str | None = L.map_key(self.vanilla, map_name, self.status.get("age", ""))
        entry: dict | None = self.vanilla.get(key) if key else None
        refs: list = (entry or {}).get("refs", [])
        room: int | None = L.room_at(refs, pos) if refs else None
        ground: float = float(pos[1])
        spot: dict | None = L.aim_spot(aim, name, facing, entry, room, link) if aim else None
        pos = spot["pos"] if spot else L.spawn_spot(name, list(pos), entry, room)
        rise: float = round(float(pos[1]) - ground, 1)
        point: dict = L.make_point(self.points, map_name, name, pos, facing, time.strftime("%H:%M:%S"), face_link,
                                   *([spot["normal"], spot["surface"]] if spot else []))
        if spot:
            point["yaw"] = round(spot["yaw"], 1)
            point["via"] = "aim"
        point["n"] = max(int(point["n"]), self.high + 1)
        self.high = point["n"]
        self.points.append(point)
        self.placed.append(point["n"])
        self.save_all()
        total: int = len([p for p in self.points if p["map"] == map_name and p["name"] == name])
        self.message = f"marker {short(name)} #{point['n']}" + (f", {rise} up" if rise else "") + " (Spawn all to see it)"
        if spot:
            self.message = f"{spot['surface']} marker {short(name)} #{point['n']}" + (f", {rise} up" if rise else "") + (", faces out of the wall" if point["wall"] else ", faces you")
        self.game.caption(f"Marker placed: {short(name)}", f"{'faces out of the wall' if spot and point['wall'] else 'faces you' if face_link or spot else 'faces where you face'}; {total} on this map; no enemy spawned", f"marker {point['n']}")
        return {"ok": True, "message": self.message}

    def repeat(self, body: dict) -> dict:
        with self.lock:
            st: dict = self.game.status() or self.status
            map_name: str = str(body.get("map") or st.get("map") or "")
            pos = body.get("pos") or st.get("pos")
            facing: float = float(body.get("facing") if body.get("facing") is not None else st.get("facing") or 0.0)
            if not map_name or not pos or len(pos) != 3:
                return {"ok": False, "message": "game not ready"}
            name: str | None = str(body["name"]) if body.get("name") else L.last_name(self.points, map_name)
            if not name:
                self.message = "nothing placed yet: press an enemy on the phone first"
                self.game.caption("Nut throw", "places the previous enemy again", self.message)
                return {"ok": False, "message": self.message}
            return self.add(name, map_name, [float(v) for v in pos], facing, face_link=True)

    def spawn_all(self) -> dict:
        with self.lock:
            st: dict = self.game.status() or self.status
            map_name: str = st.get("map") or ""
            if not map_name or st.get("loading"):
                return {"ok": False, "message": "game not ready"}
            _, before = self.game.enemies(scan=True)
            todo: list[dict] = [p for p in L.on_map(self.points, map_name) if not L.alive_for(p, before, self.tracked.get(p["n"]))]
            skipped: int = len(L.on_map(self.points, map_name)) - len(todo)
            self.game.caption("Spawn all", f"spawning {len(todo)} enemies", "working")
            for p in todo:
                self.game.spawn(p["name"], p["pos"], L.point_yaw(p), quick=True)
            time.sleep(1.5)
            _, after = self.game.enemies(scan=True)
            done: int = 0
            for p in todo:
                fresh = L.new_enemy(before, after, p["name"], p["pos"])
                if fresh and L.dist(L.enemy_pos(fresh), p["pos"]) <= L.WANDER:
                    self.tracked[p["n"]] = fresh["id"]
                    before = before + [fresh]
                    done += 1
            self.message = f"spawned {done} of {len(todo)}, {skipped} already alive"
            self.game.caption("Spawn all", f"every spawn point on {map_name} has its enemy", self.message)
            return {"ok": True, "message": self.message}

    def set_boxes(self, on) -> dict:
        shown: bool = W.toggle(self, "boxes", on)
        self.message = "boxes " + ("on: each marker has its model's box in the game" if shown else "off")
        self.radar_sent = ("", "", 0.0)
        return {"ok": True, "message": self.message, "boxes": shown}

    def set_models(self, on) -> dict:
        shown: bool = W.toggle(self, "models", on)
        self.message = "display models " + ("on: each enemy marker near Link shows the enemy's model in the game" if shown else "off")
        self.ph_sent = ("", 0.0, self.ph_sent[2])
        return {"ok": True, "message": self.message, "models": shown}

    def remove_nearest(self) -> dict:
        st: dict = self.status
        near: dict | None = L.nearest_point(self.points, st.get("map") or "", st.get("pos") or [0.0, 0.0, 0.0])
        if not near:
            self.message = "no points on this map"
            return {"ok": False, "message": self.message}
        with self.lock:
            return self.remove(near)

    def kill_all(self) -> dict:
        with self.lock:
            link, enemies = self.game.enemies(scan=True)
            if not enemies:
                self.message = "game is paused or in a menu: close it and press Kill all again"
                return {"ok": False, "message": self.message}
            victims: list[dict] = L.to_kill(enemies, link)
            missed: list[str] = []
            for e in victims:
                try:
                    self.game.damage(e["id"])
                except Exception:
                    missed.append(short(e["name"]))
            self.message = f"killed {len(victims) - len(missed)} within {int(L.NEAR_RADIUS)} m"
            if missed:
                self.message += f"; could not kill {', '.join(missed)}"
            self.game.caption("Kill all", f"enemies within {int(L.NEAR_RADIUS)} m are gone", self.message)
            return {"ok": True, "message": self.message}


STATIC: dict[str, str] = {"/ui.css": "text/css; charset=utf-8", "/ui.js": "text/javascript; charset=utf-8"}


def handler(placer: Placer, key):
    class H(BaseHTTPRequestHandler):
        def log_message(self, fmt, *args) -> None:
            pass

        def nearby(self) -> bool:
            ip = ipaddress.ip_address(self.client_address[0].split("%")[0])
            return ip.is_loopback or ip.is_private or ip.is_link_local

        def allowed(self, query: str) -> bool:
            if not self.nearby():
                return False
            if not key:
                return True
            given: str = self.headers.get("X-Panel-Key") or dict(p.split("=", 1) for p in query.split("&") if "=" in p).get("key", "")
            return secrets.compare_digest(given, key)

        def send(self, code: int, body, kind: str = "application/json") -> None:
            data: bytes = body if isinstance(body, bytes) else body.encode("utf-8")
            self.send_response(code)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self) -> None:
            path, _, query = self.path.partition("?")
            if path in STATIC and self.nearby():
                with open(os.path.join(HERE, path.lstrip("/")), "rb") as f:
                    return self.send(200, f.read(), STATIC[path])
            if not self.allowed(query):
                return self.send(403, "<p>Open the address with ?key=...</p>", "text/html; charset=utf-8")
            if path in ("/", "/placer", "/placer/"):
                with open(os.path.join(HERE, "placer.html"), "rb") as f:
                    return self.send(200, f.read(), "text/html; charset=utf-8")
            if path == "/api/state":
                return self.send(200, json.dumps(placer.view()))
            if path.startswith("/portraits/"):
                name: str = os.path.basename(path)
                full: str = os.path.join(placer.portraits, name)
                if name.lower().endswith(".png") and os.path.isfile(full):
                    with open(full, "rb") as f:
                        return self.send(200, f.read(), "image/png")
            reply = GR.get(placer, path) or PR.get(placer, path) or SR.get(placer, path)
            if reply:
                return self.send(*reply)
            return self.send(404, "{}")

        def do_POST(self) -> None:
            path, _, query = self.path.partition("?")
            if self.headers.get("X-Placer") != "1" or not self.allowed(query):
                return self.send(403, json.dumps({"error": "forbidden"}))
            n: int = int(self.headers.get("Content-Length", "0"))
            try:
                body: dict = json.loads(self.rfile.read(n) or b"{}")
            except ValueError:
                return self.send(400, "{}")
            try:
                if path == "/api/press":
                    return self.send(200, json.dumps(placer.press(str(body.get("name", "")))))
                if path == "/api/repeat":
                    return self.send(200, json.dumps(placer.repeat(body)))
                if path == "/api/spawnall":
                    return self.send(200, json.dumps(placer.spawn_all()))
                if path == "/api/removenearest":
                    return self.send(200, json.dumps(placer.remove_nearest()))
                if path == "/api/killall":
                    return self.send(200, json.dumps(placer.kill_all()))
                if path == "/api/models":
                    return self.send(200, json.dumps(placer.set_models(body.get("on"))))
                if path == "/api/aimplace":
                    return self.send(200, json.dumps(placer.place_at_aim()))
                if path == "/api/aimremove":
                    return self.send(200, json.dumps(placer.remove_at_aim()))
                if path == "/api/aim":
                    return self.send(200, json.dumps(placer.set_aim(body.get("on"))))
                if path == "/api/overlay":
                    return self.send(200, json.dumps(placer.set_overlay(body.get("on"))))
                if path == "/api/boxes":
                    return self.send(200, json.dumps(placer.set_boxes(body.get("on"))))
                reply = GR.post(placer, path, body) or PR.post(placer, path, body) or SR.post(placer, path, body)
                if reply:
                    return self.send(*reply)
            except Exception as ex:
                return self.send(200, json.dumps({"ok": False, "message": f"error: {ex}"}))
            return self.send(404, "{}")

    return H


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("session", nargs="?", default=os.environ.get("PANEL_SESSION", DEFAULT_SESSION))
    ap.add_argument("--port", type=int, default=8766)
    ap.add_argument("--lan", action="store_true")
    ap.add_argument("--key")
    ap.add_argument("--arrows", action="store_true", help="send placed markers' facing (Longshot radar API 41 and later)")
    ap.add_argument("--run", help="one run folder to talk to (default: the newest under SESSION/runs)")
    ap.add_argument("--sock", help="one bridge socket to talk to (instead of --run)")
    ap.add_argument("--fake", action="store_true", help="no game: answer from fake_game.py (render checks and screenshots)")
    a = ap.parse_args()
    if a.fake:
        import fake_game
        game = fake_game.FakeGame(a.session)
    else:
        game = Game(a.session, a.run, a.sock)
    placer: Placer = Placer(a.session, game)
    placer.arrows = a.arrows
    print(f"enemy placer on http://127.0.0.1:{a.port}/placer  session {a.session}", flush=True)
    ThreadingHTTPServer(("0.0.0.0" if a.lan else "127.0.0.1", a.port), handler(placer, a.key)).serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main())
