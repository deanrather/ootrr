"""The playtest panel: a page in the browser beside the game that a person at the controls clicks during a hands-on test
(docs/dev/reference/playtest-panel.md). It walks through a list of steps, goes to places (teleport, then warp), gives
items, saves notes with Link's position and a screenshot, records short clips, and shows and edits the saved flags live.
Every button calls the control bridge (scripts/longshot_bridge.py) at once: nothing else is in the loop. Standard library
only; it listens on 127.0.0.1.

    python -I scripts/playtest_panel/server.py --run RUN_DIR [--session DIR] [--steps FILE] [--places FILE]
           [--buttons FILE] [--extras DIR] [--flag-names FILE]... [--port 8765] [--lan [--key KEY]]

--run is a run folder (longshot_ctl.py launch makes one), --runs ROOT the newest run under ROOT each time the panel
connects (so a relaunched game is found again), --sock a bridge socket. --session is where notes, screenshots and clips
go (default ./playtest-session). --lan also listens on the local network (a phone on the same Wi-Fi), with an access
key in the address it prints. Everything the panel records is written to the session folder as it happens, in files
another program can follow (the layout is in docs/dev/reference/playtest-panel.md).
"""
import argparse
import ipaddress
import json
import os
import re
import secrets
import shutil
import socket
import ssl
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPTS = os.environ.get("LONGSHOT_SCRIPTS") or os.path.dirname(HERE)
if not os.path.isfile(os.path.join(SCRIPTS, "longshot_bridge.py")):
    sys.exit("the Mod UI needs the Longshot scripts folder (longshot_bridge.py): set LONGSHOT_SCRIPTS or use devtools/start.py --longshot-scripts")
sys.path.insert(0, SCRIPTS)
from longshot_bridge import Bridge, Failed, Session, list_runs, load_flag_names, murmur3, read_flags  # noqa: E402

BUSY = ("another edit is waiting", "not taken", "another spawn is still being checked")
STATIC_DIR = os.path.join(HERE, "static")
STATIC = {"/layout.js": ("layout.js", "text/javascript; charset=utf-8"),
          "/sw.js": ("sw.js", "text/javascript; charset=utf-8"),
          "/manifest.webmanifest": ("manifest.webmanifest", "application/manifest+json"),
          "/icon-192.png": ("icon-192.png", "image/png"),
          "/icon-512.png": ("icon-512.png", "image/png")}
CLIP_WINDOW = "Longshot"
CTL = os.path.join(SCRIPTS, "longshot_ctl.py")


def read_json(path, otherwise):
    try:
        with open(path, encoding="utf-8-sig") as f:
            return json.load(f)
    except (OSError, ValueError):
        return otherwise


class Game:
    """One bridge connection shared by the panel's threads, one request at a time, opened again after a failure"""

    def __init__(self, run=None, runs=None, sock=None):
        self.run, self.runs, self.sock = run, runs, sock
        self.bridge = None
        self.lock = threading.Lock()

    def folder(self):
        if not self.runs:
            return self.run
        runs = list_runs(self.runs)
        live = [r for r in runs if r.get("state") != "finished"] or runs
        if not live:
            raise Failed("no run under %s" % self.runs, 2)
        return max(live, key=lambda r: r.get("started_at") or "")["run"]

    def open(self):
        if self.sock:
            return Bridge(self.sock, 10)
        return Session.attach(self.folder()).connect(10)

    def ctl(self, args, timeout=120):
        """longshot_ctl.py with the same game, for a step's commands written as its command lines"""
        try:
            target = ["--sock", self.sock] if self.sock else ["--run", self.folder()]
        except Failed as e:
            return str(e)
        try:
            r = subprocess.run([sys.executable, "-I", CTL] + target + [str(x) for x in args], capture_output=True,
                               text=True, timeout=timeout)
        except (OSError, subprocess.TimeoutExpired) as e:
            return str(e)
        return (r.stdout + r.stderr).strip()

    def request(self, cmd, timeout=None, **args):
        with self.lock:
            try:
                if self.bridge is None:
                    self.bridge = self.open()
                return self.bridge.request(cmd, timeout=timeout, **args)
            except (Failed, OSError) as e:
                if self.bridge is not None:
                    try:
                        self.bridge.close()
                    except OSError:
                        pass
                self.bridge = None
                return {"ok": False, "error": "no_game", "message": str(e)}

    def ask(self, cmd, retries=20, delay=1.0, **args):
        """A request sent again while the game is busy with another edit (the bridge takes one at a time)"""
        r = self.request(cmd, **args)
        for _ in range(retries):
            if r.get("ok") or not any(t in r.get("message", "") for t in BUSY):
                break
            time.sleep(delay)
            r = self.request(cmd, **args)
        return r

    def status(self):
        r = self.request("status")
        return r.get("game") or {} if r.get("ok") else {"error": r.get("message", "no game")}


def words(r):
    if not r.get("ok"):
        return "%s: %s" % (r.get("error", "error"), r.get("message", ""))
    for key in ("done", "detail", "spawned", "item"):
        if key in r:
            return str(r[key])
    return "ok"


def stamp():
    t = time.time()
    return time.strftime("%H%M%S", time.localtime(t)) + "-%03d" % int(t * 1000 % 1000)


def write_atomic(path, value):
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(value, f)
    os.replace(tmp, path)


def write_atomic_text(path, text):
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline=chr(10)) as f:
        f.write(text)
    os.replace(tmp, path)


class Flags:
    """The saved flags read in a loop while a page looks at them: the values now, and every change seen since the panel
    started (newest last), numbered so that a page asks only for what it has not seen"""

    def __init__(self, game, name_files, learned, log=None):
        self.game, self.learned, self.log_path = game, learned, log
        self.names = {}
        for path in name_files:
            try:
                self.names.update(load_flag_names(path))
            except (OSError, ValueError) as e:
                print("flag names %s: %s" % (path, e), file=sys.stderr)
        if os.path.exists(learned):
            self.names.update(load_flag_names(learned))
        self.values = {}
        self.kinds = {}
        self.changes = []
        self.seq = 0
        self.wanted = 0.0
        self.error = ""
        self.read_at = 0.0
        self.mu = threading.Lock()
        threading.Thread(target=self.loop, daemon=True).start()

    def loop(self):
        while True:
            if time.monotonic() - self.wanted < 6:
                self.read()
            time.sleep(1.0)

    def read(self):
        try:
            r = read_flags(self.game, "all")
        except Failed as e:
            self.error = str(e)
            return
        now = time.time()
        with self.mu:
            first = not self.values
            for kind in ("bools", "ints"):
                for h, v in r[kind].items():
                    old = self.values.get(h)
                    if old is not None and old != v:
                        self.seq += 1
                        self.changes.append({"seq": self.seq, "t": now, "hash": "%08x" % h, "old": old, "new": v})
                    self.values[h] = v
                    self.kinds[h] = kind[0]
            if first:
                self.seq += 1
            fresh = [dict(c, name=self.names.get(int(c["hash"], 16))) for c in self.changes if c["t"] == now]
            del self.changes[:-2000]
            self.error, self.read_at = "", now
        if fresh and self.log_path:
            with open(self.log_path, "a", encoding="utf-8") as f:
                f.write("".join(json.dumps(c) + "\n" for c in fresh))

    def request(self, cmd, timeout=None, **args):
        return self.game.request(cmd, timeout=timeout, **args)

    def snapshot(self, since):
        self.wanted = time.monotonic()
        with self.mu:
            out = {"seq": self.seq, "error": self.error, "read_at": self.read_at,
                   "changes": [c for c in self.changes if c["seq"] > since][-500:]}
            if since <= 0 or (self.changes and since < self.changes[0]["seq"] - 1):
                out["all"] = {"%08x" % h: [self.kinds[h], v] for h, v in self.values.items()}
                out["names"] = {"%08x" % h: n for h, n in self.names.items() if h in self.values}
        return out

    def learn(self, name):
        h = murmur3(name)
        with self.mu:
            known = h in self.values
            if known and self.names.get(h) != name:
                self.names[h] = name
                with open(self.learned, "a", encoding="utf-8") as f:
                    f.write(name + "\n")
        return {"ok": True, "hash": "%08x" % h, "known": known, "name": name}

    def set(self, key, value):
        r = self.game.ask("edit", verb="flagset", arg=key, n=int(value), timeout=20)
        if r.get("ok"):
            self.wanted = time.monotonic()
            threading.Thread(target=self.read, daemon=True).start()
        return r


class Panel:
    def __init__(self, a):
        self.a = a
        self.game = Game(a.run, a.runs, a.sock)
        self.step = None
        self.session = os.path.abspath(a.session)
        self.notes = os.path.join(self.session, "notes")
        os.makedirs(os.path.join(self.notes, "shots"), exist_ok=True)
        os.makedirs(os.path.join(self.notes, "clips"), exist_ok=True)
        self.log_path = os.path.join(self.notes, "notes.jsonl")
        self.log_mu = threading.Lock()
        self.key = None
        self.messages_path = a.messages or os.path.join(self.notes, "messages.jsonl")
        self.flags = Flags(self.game, a.flag_names or [], os.path.join(self.session, "learned-flag-names.txt"),
                           os.path.join(self.notes, "flags.jsonl"))

    def log(self, entry):
        """One line of notes.jsonl, written whole and closed at once: a program following the file reads complete lines,
        and a screenshot or clip a line names is complete when the line is written"""
        entry["time"] = time.strftime("%H:%M:%S")
        entry["t"] = round(time.time(), 3)
        with self.log_mu, open(self.log_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(entry) + "\n")
        return entry

    def recent(self, n=10):
        try:
            with open(self.log_path, encoding="utf-8") as f:
                return [json.loads(x) for x in f.readlines()[-n:]]
        except (OSError, ValueError):
            return []

    def messages(self, n=3):
        """The last lines of the messages file: {"text", "time"} a line, written by whoever follows the session (a
        person, a script, an agent) for the tester to read in the page"""
        try:
            with open(self.messages_path, encoding="utf-8-sig") as f:
                return [json.loads(x) for x in f.readlines()[-n:] if x.strip()]
        except (OSError, ValueError):
            return []

    def steps_path(self):
        """The steps file in use: the path written in SESSION/notes/steps-file.txt when that file names one that exists
        (point the page at another steps file by writing the path there, no restart), else --steps"""
        try:
            with open(os.path.join(self.notes, "steps-file.txt"), encoding="utf-8-sig") as f:
                p = f.read().strip().strip('"')
            if p and os.path.isfile(p):
                return p
        except OSError:
            pass
        return self.a.steps

    def steps_info(self):
        p = self.steps_path()
        try:
            m = os.path.getmtime(p)
        except OSError:
            m = 0
        return {"file": os.path.basename(p), "path": p, "sig": "%s|%s" % (p, m)}

    def extras(self):
        """Each NAME.html is a card; its first line may be <!--{"title": "Card name", "order": 30, "open": false}--> (the
        card's name in its header, where it sits among the extras, whether it starts open)"""
        folder = self.a.extras
        if not folder or not os.path.isdir(folder):
            return []
        out = []
        for name in sorted(os.listdir(folder)):
            stem, ext = os.path.splitext(name)
            if ext.lower() == ".html":
                meta = {}
                try:
                    with open(os.path.join(folder, name), encoding="utf-8-sig") as f:
                        m = re.match(r"\s*<!--(\{.*?\})-->", f.readline())
                    if m:
                        meta = json.loads(m.group(1))
                except (OSError, ValueError):
                    pass
                out.append({"name": stem, "js": os.path.exists(os.path.join(folder, stem + ".js")),
                            "title": str(meta.get("title") or stem), "order": meta.get("order", 50), "open": meta.get("open", True)})
        return sorted(out, key=lambda e: (e["order"], e["name"]))

    def state(self):
        places = read_json(self.a.places, [])
        if isinstance(places, list):
            places = {"places": places}
        out = {"title": self.a.title, "steps": read_json(self.steps_path(), []), "steps_info": self.steps_info(), "places": places, "buttons": read_json(self.a.buttons, []),
               "extras": self.extras(), "recent": self.recent(), "status": self.game.status(), "session": self.session,
               "messages": self.messages()}
        try:
            write_atomic(os.path.join(self.notes, "state.json"), {"t": round(time.time(), 3), "step": self.step,
                                                                    "status": out["status"]})
        except OSError:
            pass
        return out

    def goto(self, body):
        """Teleport to the map when Link is not in it, warp as soon as it has loaded, then check he arrived: a warp sent
        while the map's arrival event plays is accepted and ignored, so it is sent again for about 10 s"""
        target, outs = body.get("map", ""), []
        if target and self.game.status().get("map") != target:
            r = {}
            end = time.monotonic() + 15
            while time.monotonic() < end:
                r = self.game.request("teleport", to=int(target) if str(target).isdigit() else target, timeout=30)
                if r.get("ok") or not any(k in words(r) for k in ("in progress", "event is playing")):
                    break
                time.sleep(1.0)
            outs.append(words(r) if not r.get("ok") else "teleport to %s" % r.get("map"))
            arrive = r.get("map") or target
            for _ in range(150 if r.get("ok") else 0):
                g = self.game.status()
                if g.get("map") == arrive and not g.get("loading") and not g.get("event"):
                    break
                time.sleep(0.2)
            if self.game.status().get("map") != arrive:
                outs.append("not moved: Link is not in %s yet, press again" % target)
                return outs
        for p in body.get("warps", []):
            arrived, r = False, {}
            end = time.monotonic() + 10
            while time.monotonic() < end and not arrived:
                r = self.game.request("warp", x=float(p[0]), y=float(p[1]), z=float(p[2]), timeout=10)
                for _ in range(4):
                    time.sleep(0.25)
                    pos = self.game.status().get("pos") or []
                    if len(pos) == 3 and abs(pos[0] - float(p[0])) < 4 and abs(pos[2] - float(p[2])) < 4:
                        arrived = True
                        break
            outs.append(("arrived at %s" % (p,)) if arrived else ("not arrived: " + words(r)))
        return outs

    def all_items(self):
        r = self.game.request("give")
        items = r.get("items", []) if r.get("ok") else []
        given = 0
        for it in items:
            if it.get("have") and not it.get("quantity"):
                continue
            args = {"item": it["item"], "count": 63 if it.get("quantity") else None, "timeout": 30}
            g = self.game.request("give", **args)
            tries = 0
            while not g.get("ok") and any(t in g.get("message", "") for t in BUSY) and tries < 15:
                self.game.request("press", buttons="A", frames=8)
                time.sleep(0.8)
                g = self.game.request("give", **args)
                tries += 1
            given += 1 if g.get("ok") else 0
            self.game.request("press", buttons="A", frames=8)
            time.sleep(0.3)
        for _ in range(6):
            self.game.request("press", buttons="A", frames=8)
            time.sleep(0.6)
        return "%d of %d given" % (given, len(items))

    def button(self, body):
        label = body.get("label", "")
        found = next((b for b in read_json(self.a.buttons, []) if b.get("label") == label), None)
        if not found:
            return {"ok": False, "error": "unknown button"}
        if found.get("action") == "all_items":
            out = [self.all_items()]
        else:
            out = []
            for req in found.get("do", []):
                req = dict(req)
                out.append(words(self.game.ask(req.pop("cmd"), **req)))
        self.log({"kind": "command", "name": label, "step": body.get("step"), "reply": " | ".join(o[-120:] for o in out)})
        return {"ok": True, "reply": out}

    def action(self, body):
        """One of a step's actions: "do", bridge requests as a button's, or "cmds", longshot_ctl.py command lines"""
        step = next((s for s in read_json(self.steps_path(), []) if s.get("id") == body.get("step")), None)
        found = next((a for a in (step or {}).get("actions", []) if a.get("label") == body.get("label")), None)
        if not found:
            return {"ok": False, "error": "unknown action"}
        out = []
        for req in found.get("do", []):
            req = dict(req)
            out.append(words(self.game.ask(req.pop("cmd"), **req)))
        cmds = found.get("cmds", [])
        if len(cmds) == 1 and cmds[0] and isinstance(cmds[0][0], list):
            cmds = cmds[0]
        for c in cmds:
            if c and isinstance(c[0], list):
                c = c[0]
            out.append(self.game.ctl(c)[-200:])
        self.log({"kind": "command", "name": found["label"], "step": body.get("step"), "reply": " | ".join(o[-120:] for o in out)})
        return {"ok": True, "reply": out}

    def note(self, kind, body):
        st = self.game.status()
        shot = os.path.join(self.notes, "shots", stamp() + ".png")
        threading.Thread(target=lambda: self.game.request("shot", path=shot), daemon=True).start()
        entry = {"kind": kind, "step": body.get("step"), "text": body.get("text", ""), "verdict": body.get("verdict"),
                 "map": st.get("map"), "pos": st.get("pos"), "life": st.get("life"), "event": st.get("event"),
                 "shot": shot}
        return self.log(entry)

    def record(self, body):
        secs = max(1, min(600, int(body.get("secs", 30))))
        out = os.path.join(self.notes, "clips", "clip-" + stamp() + ".mp4")
        if os.name != "nt":
            return {"ok": False, "error": "clips use ffmpeg's gdigrab (Windows); record the window with your own tool"}
        try:
            proc = subprocess.Popen(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "gdigrab", "-framerate", "30",
                                     "-t", str(secs), "-i", "title=" + CLIP_WINDOW, "-vf", "scale=960:-2", "-c:v", "libx264",
                                     "-preset", "ultrafast", "-pix_fmt", "yuv420p", "-f", "mp4", out + ".part"])
        except OSError as e:
            return {"ok": False, "error": "ffmpeg: %s" % e}
        step, text = body.get("step"), body.get("text", "")
        self.log({"kind": "record", "step": step, "text": text, "secs": secs, "clip": out})

        def finish():
            code = proc.wait()
            if code == 0 and os.path.exists(out + ".part"):
                os.replace(out + ".part", out)
                self.log({"kind": "clip", "step": step, "text": text, "secs": secs, "clip": out})
            else:
                self.log({"kind": "clip_failed", "step": step, "secs": secs, "clip": out, "exit": code})
        threading.Thread(target=finish, daemon=True).start()
        return {"ok": True, "clip": out}

    def post(self, path, body):
        if "step" in body:
            self.step = body.get("step")
        if path == "/button":
            return self.button(body)
        if path == "/action":
            return self.action(body)
        if path == "/goto":
            outs = self.goto(body)
            self.log({"kind": "command", "name": "go to " + body.get("label", body.get("map", "")), "step": body.get("step"),
                      "reply": " | ".join(o[-80:] for o in outs)})
            return {"ok": True, "reply": outs}
        if path == "/teleport":
            to = str(body.get("to", ""))
            r = self.game.request("teleport", to=int(to) if to.isdigit() else to, timeout=30)
            self.log({"kind": "command", "name": "teleport " + to, "step": body.get("step"), "reply": words(r)[-160:]})
            return {"ok": r.get("ok", False), "reply": words(r)}
        if path == "/warp":
            x, y, z = (float(body.get(k, 0)) for k in ("x", "y", "z"))
            r = self.game.request("warp", x=x, y=y, z=z, timeout=10)
            self.log({"kind": "command", "name": "warp %g %g %g" % (x, y, z), "step": body.get("step"), "reply": words(r)})
            return {"ok": r.get("ok", False), "reply": words(r)}
        if path == "/record":
            return self.record(body)
        if path in ("/note", "/next", "/verdict"):
            return self.note(path[1:], body)
        if path == "/bridge":
            body = dict(body)
            return self.game.request(str(body.pop("cmd", "")), **body)
        if path == "/flag":
            key = body.get("name") or "0x" + str(body.get("hash", ""))
            r = self.flags.set(key, body.get("value", 0))
            self.log({"kind": "command", "name": "flag %s = %s" % (key, body.get("value")), "step": body.get("step"),
                      "reply": words(r)})
            return r
        if path == "/extras_file":
            name = str(body.get("name", ""))
            if not re.match(r"^[A-Za-z0-9_.-]{1,64}$", name) or name.startswith("."):
                return {"ok": False, "error": "name: letters, digits, _ . - only"}
            folder = os.path.join(self.notes, "extras")
            os.makedirs(folder, exist_ok=True)
            full = os.path.join(folder, name)
            text = str(body.get("text", ""))
            if len(text) > 4 * 1024 * 1024:
                return {"ok": False, "error": "text over 4 MB"}
            write_atomic_text(full, text)
            return {"ok": True, "path": full}
        if path == "/flags/learn":
            return self.flags.learn(str(body.get("name", "")).strip())
        return None


def handler(panel):
    class H(BaseHTTPRequestHandler):
        def log_message(self, fmt, *args):
            try:
                with open(os.path.join(panel.notes, "access.log"), "a", encoding="utf-8") as f:
                    f.write("%s %s %s\n" % (time.strftime("%H:%M:%S"), self.client_address[0], fmt % args))
            except OSError:
                pass

        def nearby(self):
            """--lan answers only this PC and the private networks (192.168.x, 10.x, ...), never a public address"""
            ip = ipaddress.ip_address(self.client_address[0].split("%")[0])
            return ip.is_loopback or ip.is_private or ip.is_link_local

        def send(self, code, body, kind="application/json"):
            data = body if isinstance(body, bytes) else body.encode("utf-8")
            self.send_response(code)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(data)

        def file(self, path, kind):
            try:
                with open(path, "rb") as f:
                    return self.send(200, f.read(), kind)
            except OSError:
                return self.send(404, "{}")

        def allowed(self, query):
            if not panel.key:
                return True
            if not self.nearby():
                return False
            given = self.headers.get("X-Panel-Key") or dict(p.split("=", 1) for p in query.split("&") if "=" in p).get("key", "")
            return secrets.compare_digest(given, panel.key)

        def do_GET(self):
            path, _, query = self.path.partition("?")
            shell = STATIC.get(path)
            if (shell or path in ("/", "/index.html")) and not self.nearby():
                return self.send(403, "{}")
            if shell:
                return self.file(os.path.join(STATIC_DIR, shell[0]), shell[1])
            if path in ("/", "/index.html"):
                return self.file(os.path.join(HERE, "index.html"), "text/html; charset=utf-8")
            if not self.allowed(query):
                return self.send(403, "<p>This panel needs its access key: open the address the panel printed when it "
                                      "started (it ends in ?key=...).</p>", "text/html; charset=utf-8")
            if path == "/state":
                return self.send(200, json.dumps(panel.state()))
            if path == "/destinations":
                r = panel.game.request("teleport")
                return self.send(200, json.dumps(r.get("destinations", []) if r.get("ok") else []))
            if path == "/flags":
                since = int(dict(p.split("=", 1) for p in query.split("&") if "=" in p).get("since", "0") or 0)
                return self.send(200, json.dumps(panel.flags.snapshot(since)))
            if path.startswith("/extras/"):
                name = os.path.basename(path[len("/extras/"):])
                stem, ext = os.path.splitext(name)
                kinds = {".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css"}
                if panel.a.extras and ext in kinds and stem in {e["name"] for e in panel.extras()}:
                    return self.file(os.path.join(panel.a.extras, name), kinds[ext])
            return self.send(404, "{}")

        def do_POST(self):
            if self.headers.get("X-Panel") != "1":   # (a page of another site cannot send this header without asking first)
                return self.send(403, json.dumps({"error": "the panel's own page only"}))
            if not self.allowed(self.path.partition("?")[2]):
                return self.send(403, json.dumps({"error": "the access key is missing or wrong"}))
            n = int(self.headers.get("Content-Length", "0"))
            try:
                body = json.loads(self.rfile.read(n) or b"{}")
            except ValueError:
                return self.send(400, json.dumps({"error": "bad json"}))
            r = panel.post(self.path, body if isinstance(body, dict) else {})
            return self.send(404 if r is None else 200, json.dumps(r if r is not None else {"error": "not found"}))

    return H


def main(argv=None):
    p = argparse.ArgumentParser(description="the playtest panel: a browser page that drives the game through the bridge")
    g = p.add_mutually_exclusive_group(required=True)
    g.add_argument("--run", help="a run folder (longshot_ctl.py launch)")
    g.add_argument("--runs", help="a runs root: the newest run under it, found again after a relaunch")
    g.add_argument("--sock", help="a bridge socket or pipe (HOS_CONTROL)")
    p.add_argument("--session", default="playtest-session", help="where notes, screenshots and clips go")
    p.add_argument("--steps", default=os.path.join(HERE, "examples", "steps.json"), help="the steps to walk through")
    p.add_argument("--places", default=os.path.join(HERE, "examples", "places.json"), help="the places to go to")
    p.add_argument("--buttons", default=os.path.join(HERE, "examples", "buttons.json"), help="the Give me buttons")
    p.add_argument("--extras", help="a folder of extra panels: each NAME.html (and NAME.js) is shown as a card")
    p.add_argument("--flag-names", action="append", help="a flag names file (more than one may be given)")
    p.add_argument("--messages", help="a file of messages for the tester, one JSON object a line with text and time "
                                      "(default: the session's notes/messages.jsonl)")
    p.add_argument("--title", default="Playtest Panel", help="the page's title and header")
    p.add_argument("--port", type=int, default=8765)
    p.add_argument("--lan", action="store_true", help="also listen on the local network (a phone on the same Wi-Fi), with an access key")
    p.add_argument("--key", help="with --lan: the access key (default: a new random one each start)")
    p.add_argument("--no-key", action="store_true", help="with --lan: no access key (anyone on the local network can use it)")
    p.add_argument("--https", action="store_true", help="serve https (a phone's browser installs the app only from a secure page): "
                                                          "makes a self-signed certificate in the session folder with openssl, or uses --cert and --cert-key")
    p.add_argument("--cert", help="with --https: a certificate file (PEM)")
    p.add_argument("--cert-key", help="with --https: its private key file (PEM)")
    a = p.parse_args(argv)
    panel = Panel(a)
    panel.key = (a.key or secrets.token_urlsafe(12)) if a.lan and not a.no_key else None
    scheme = "https" if a.https else "http"
    print("playtest panel on %s://127.0.0.1:%d/%s  session %s" % (scheme, a.port, "?key=" + panel.key if panel.key else "",
                                                                    panel.session), flush=True)
    if a.lan:
        for ip in lan_addresses():
            print("  on the local network: %s://%s:%d/%s" % (scheme, ip, a.port, "?key=" + panel.key if panel.key else ""), flush=True)
        print("  (Windows may ask once to let Python accept connections: allow private networks only)", flush=True)
    server = ThreadingHTTPServer(("0.0.0.0" if a.lan else "127.0.0.1", a.port), handler(panel))
    if a.https:
        cert, key = a.cert, a.cert_key
        if not cert or not key:
            cert, key = make_cert(panel.notes, lan_addresses() if a.lan else [])
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.load_cert_chain(cert, key)
        server.socket = ctx.wrap_socket(server.socket, server_side=True)
    server.serve_forever()


def make_cert(folder, ips):
    """A self-signed certificate for 127.0.0.1, localhost and this PC's addresses, kept in the notes folder (made with openssl,
    which Git for Windows ships; the standard library cannot make one)"""
    cert, key = os.path.join(folder, "panel-cert.pem"), os.path.join(folder, "panel-key.pem")
    if os.path.isfile(cert) and os.path.isfile(key):
        return cert, key
    exe = shutil.which("openssl") or next((q for q in (r"C:\Program Files\Git\usr\bin\openssl.exe",) if os.path.isfile(q)), None)
    if not exe:
        sys.exit("--https needs openssl on the PATH (Git for Windows has one), or give --cert and --cert-key")
    san = ",".join(["DNS:localhost", "IP:127.0.0.1"] + ["IP:" + ip for ip in ips])
    os.makedirs(folder, exist_ok=True)
    subprocess.check_call([exe, "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "825", "-subj", "/CN=Mod UI panel",
                           "-addext", "subjectAltName=" + san, "-keyout", key, "-out", cert],
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return cert, key


def lan_addresses():
    found = set()
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.connect(("192.0.2.1", 9))   # (nothing is sent: it picks the interface a route off this PC would use)
            found.add(s.getsockname()[0])
        found = {ip for ip in found if not ip.startswith("169.254.")}
    except OSError:
        pass
    try:
        found.update(ip for ip in socket.gethostbyname_ex(socket.gethostname())[2] if not ip.startswith(("127.", "169.254.")))
    except OSError:
        pass
    return sorted(found)


if __name__ == "__main__":
    main()
