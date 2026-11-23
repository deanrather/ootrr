"""One warp at a time for the placer's cards (gate, puzzle, Gold Skulltula): goto.go() on a background thread, so the phone
gets its answer at once and the card shows how the trip is going. Overlay flags (which layers the in-game radar shows) live
here too, in SESSION/notes/panel_overlay.json.
"""
import json
import os
import threading
import time

import goto

LAYERS: tuple[str, ...] = ("gates", "puzzles", "gs", "boxes", "models", "placer", "gate_models")
LAYER_DEFAULT: dict[str, bool] = {k: False for k in LAYERS}
SHOWING: tuple[str, ...] = ("gates", "puzzles", "gs", "placer", "models", "gate_models", "boxes")
OVERLAY_FILE: str = "panel_overlay.json"
FILE_VERSION: int = 2


class Runner:
    def __init__(self, placer) -> None:
        self.placer = placer
        self.lock: threading.Lock = threading.Lock()
        self.busy: bool = False
        self.message: str = ""
        self.label: str = ""
        self.ok: bool | None = None
        self.started: float = 0.0

    def state(self) -> dict:
        return {"busy": self.busy, "message": self.message, "label": self.label, "ok": self.ok}

    def start(self, map_name: str, pos: list[float], label: str, threaded: bool = True) -> dict:
        with self.lock:
            if self.busy:
                return {"ok": False, "message": f"still going to {self.label}: wait for it to finish"}
            self.busy, self.ok, self.label, self.started = True, None, label, time.time()
            self.message = f"going to {label}"
        if threaded:
            threading.Thread(target=self.run, args=(map_name, pos, label), daemon=True).start()
        else:
            self.run(map_name, pos, label)
        return {"ok": True, "message": self.message}

    def say(self, m: str) -> None:
        self.message = f"{self.label}: {m}"

    def run(self, map_name: str, pos: list[float], label: str) -> None:
        g = self.placer.game
        try:
            self.placer.game.caption(f"Warp to {label}", f"Link stands in {map_name} at {[round(v, 1) for v in pos]}", "going")
            ok, msg = goto.go(g.status, lambda *c: g.ctl(*c, timeout=40, retries=2), map_name, [float(v) for v in pos], say=self.say,
                              sleep=getattr(g, "sleep", time.sleep))
        except Exception as ex:
            ok, msg = False, f"error: {ex}"
        self.ok = ok
        self.message = f"{label}: {msg}"
        try:
            self.placer.game.caption(f"Warp to {label}", f"Link stands in {map_name}", msg)
        except Exception:
            pass
        self.busy = False


MAKE: threading.Lock = threading.Lock()


def runner(placer) -> Runner:
    with MAKE:
        r: Runner | None = getattr(placer, "warp_runner", None)
        if r is None:
            r = Runner(placer)
            placer.warp_runner = r
        return r


def overlay_path(placer) -> str:
    return os.path.join(os.path.dirname(placer.points_path), OVERLAY_FILE)


def overlay(placer) -> dict[str, bool]:
    flags: dict | None = getattr(placer, "overlay_flags", None)
    if flags is None:
        flags = dict(LAYER_DEFAULT)
        try:
            with open(overlay_path(placer), encoding="utf-8") as f:
                saved: dict = json.load(f)
            if saved.get("v") == FILE_VERSION:
                flags.update({k: bool(saved[k]) for k in LAYERS if k in saved})
        except (OSError, ValueError, TypeError):
            pass
        placer.overlay_flags = flags
    return flags


def any_on(placer) -> bool:
    flags: dict[str, bool] = overlay(placer)
    return any(flags.get(k, False) for k in SHOWING)


def shown(placer, layer: str) -> bool:
    return overlay(placer).get(layer, False)


def toggle(placer, layer: str, on: bool | None = None) -> bool:
    flags: dict[str, bool] = overlay(placer)
    flags[layer] = (not flags.get(layer, False)) if on is None else bool(on)
    try:
        tmp: str = overlay_path(placer) + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(dict(flags, v=FILE_VERSION), f)
        os.replace(tmp, overlay_path(placer))
    except OSError:
        pass
    sync = getattr(placer, "overlay_sync", None)
    if sync:
        sync(any(flags.get(k, False) for k in SHOWING))
    return flags[layer]
