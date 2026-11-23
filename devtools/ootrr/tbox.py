r"""Press T while the game window is in front: a small text box opens over it; Enter appends the text to the session notes.

    python -I tbox.py SESSION [--key T] [--proc longshot.exe]

The text is appended to SESSION\notes\notes.jsonl, the panel's own notes file, as {"kind": "note", "source": "t-box", ...}
with the map and position from SESSION\notes\state.json (written by the panel; no bridge call here). The last line of
SESSION\notes\messages.jsonl is shown in the box. A Longshot build with its own in-game T box (HOS_NOTES_DIR set, it writes
notes\tbox-ingame.json with its pid) handles T itself: this box then leaves that game alone and stays the fallback for others. Longshot ignores the keyboard while another process owns the foreground
window (host/nn_hid.c, game_in_front), so typing in the box does not move Link; the box is a normal window, not a tool
window or a no-activate one, because Longshot counts those as part of the game and would keep reading the keys.
"""
import ctypes
import ctypes.wintypes as wt
import json
import os
import sys
import time

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
user32.GetForegroundWindow.restype = wt.HWND
user32.GetWindowThreadProcessId.argtypes = [wt.HWND, ctypes.POINTER(wt.DWORD)]
user32.SetForegroundWindow.argtypes = [wt.HWND]
user32.GetAsyncKeyState.argtypes = [ctypes.c_int]
user32.GetClientRect.argtypes = [wt.HWND, ctypes.POINTER(wt.RECT)]
user32.ClientToScreen.argtypes = [wt.HWND, ctypes.POINTER(wt.POINT)]
user32.GetParent.argtypes = [wt.HWND]
user32.GetParent.restype = wt.HWND
user32.FindWindowW.restype = wt.HWND
user32.FindWindowW.argtypes = [wt.LPCWSTR, wt.LPCWSTR]
kernel32.OpenProcess.restype = wt.HANDLE
kernel32.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
kernel32.QueryFullProcessImageNameW.argtypes = [wt.HANDLE, wt.DWORD, wt.LPWSTR, ctypes.POINTER(wt.DWORD)]
kernel32.CloseHandle.argtypes = [wt.HANDLE]
user32.IsWindowVisible.argtypes = [wt.HWND]
user32.GetWindowTextLengthW.argtypes = [wt.HWND]
user32.GetWindowTextW.argtypes = [wt.HWND, wt.LPWSTR, ctypes.c_int]
GAME_DIR = ""

VK_NAMES = {"ESC": 27, "TAB": 9, "ENTER": 13, "SPACE": 32}


def vk_of(name):
    name = name.upper()
    if name in VK_NAMES:
        return VK_NAMES[name]
    if len(name) == 1:
        return ord(name)
    if name.startswith("F") and name[1:].isdigit():
        return 0x6F + int(name[1:])
    raise ValueError("unknown key " + name)


def window_pid(hwnd):
    pid = wt.DWORD(0)
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    return pid.value


def process_name(hwnd):
    h = kernel32.OpenProcess(0x1000, False, window_pid(hwnd))
    if not h:
        return ""
    try:
        buf = ctypes.create_unicode_buffer(520)
        n = wt.DWORD(520)
        if kernel32.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(n)):
            if GAME_DIR and not buf.value.lower().startswith(GAME_DIR):
                return ""
            return os.path.basename(buf.value).lower()
        return ""
    finally:
        kernel32.CloseHandle(h)


def foreground(proc, own_pid=None):
    """(hwnd, (x, y, w, h) of the client area) when the foreground window's process is `proc`, else None. The box's own
    process never counts."""
    hwnd = user32.GetForegroundWindow()
    if not hwnd:
        return None
    if window_pid(hwnd) == (os.getpid() if own_pid is None else own_pid):
        return None
    if process_name(hwnd) != proc.lower():
        return None
    r = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(r))
    p = wt.POINT(0, 0)
    user32.ClientToScreen(hwnd, ctypes.byref(p))
    return hwnd, (p.x, p.y, r.right - r.left, r.bottom - r.top)


def key_down(vk):
    return bool(user32.GetAsyncKeyState(vk) & 0x8000)


def key_rebinds(session, vk):
    """Names of the buttons any longshot_options.ini near the session binds to `vk` (a T-box key that is also a game key
    would be pressed in the game as well)."""
    found = []
    for root, dirs, files in os.walk(session):
        if root[len(session):].count(os.sep) > 2:
            dirs[:] = []
            continue
        for skip in ("runs", "notes"):
            if skip in dirs:
                dirs.remove(skip)
        if "longshot_options.ini" in files:
            try:
                with open(os.path.join(root, "longshot_options.ini"), encoding="utf-8", errors="replace") as f:
                    for line in f:
                        line = line.strip()
                        if line.startswith("key_") and "=" in line:
                            name, val = line.split("=", 1)
                            if str(vk) in [v.strip() for v in val.split(",")]:
                                found.append(name[4:])
            except OSError:
                pass
    return sorted(set(found))


def notes_dir(session):
    return os.path.join(session, "notes")


def read_context(session, max_age=15.0):
    """map, pos, life, event from the panel's state.json when written in the last `max_age` seconds, else {}."""
    try:
        with open(os.path.join(notes_dir(session), "state.json"), encoding="utf-8") as f:
            s = json.load(f)
        if time.time() - float(s.get("t", 0)) > max_age:
            return {}
        st = s.get("status") or {}
        return {k: st[k] for k in ("map", "pos", "life", "event") if k in st}
    except (OSError, ValueError, TypeError):
        return {}


sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import paths as PATHS  # noqa: E402

CTL = PATHS.longshot_ctl()
TELEMETRY_SECS = 10.0
TELEMETRY_EVERY = 0.5


def latest_run(session):
    root = os.path.join(session, "runs")
    try:
        dirs = [os.path.join(root, d) for d in os.listdir(root) if os.path.isdir(os.path.join(root, d))]
    except OSError:
        return None
    return max(dirs, key=os.path.getmtime) if dirs else None


def bridge_json(session, *args, timeout=8):
    import subprocess
    run = latest_run(session)
    if not run:
        return {}
    try:
        r = subprocess.run([sys.executable, "-I", CTL, "--run", run, "--json"] + list(args), capture_output=True, text=True,
                           timeout=timeout, creationflags=0x08000000)
        for line in reversed(r.stdout.splitlines()):
            line = line.strip()
            if line.startswith("{"):
                return json.loads(line)
    except Exception:
        pass
    return {}


class Telemetry:
    """Samples the game's status twice a second and keeps the last TELEMETRY_SECS seconds, so every note carries where
    Link was, which way he faced and what happened just before the user typed."""

    def __init__(self, session):
        import collections
        import threading
        self.session = session
        self.samples = collections.deque()
        self.lock = threading.Lock()
        threading.Thread(target=self.loop, daemon=True).start()

    def loop(self):
        while True:
            g = (bridge_json(self.session, "status") or {}).get("game") or {}
            if g:
                s = {k: g[k] for k in ("t", "frame", "map", "pos", "facing", "camera", "life", "max_life", "event", "loading",
                                       "age", "music", "prompt") if k in g}
                s["host_t"] = round(time.time(), 2)
                with self.lock:
                    self.samples.append(s)
                    while self.samples and time.time() - self.samples[0]["host_t"] > TELEMETRY_SECS:
                        self.samples.popleft()
            time.sleep(TELEMETRY_EVERY)

    def context(self):
        with self.lock:
            hist = list(self.samples)
        if not hist:
            return {}
        last = hist[-1]
        out = {k: last[k] for k in ("map", "pos", "facing", "camera", "life", "max_life", "event", "age", "music") if k in last}
        pos = last.get("pos")
        if pos and len(pos) == 3:
            near = bridge_json(self.session, "actors", "--near", *[str(v) for v in pos], "15", "--limit", "60")
            rows = near.get("actors")
            if rows is None:
                rows = [e for e in (bridge_json(self.session, "enemies", "--no-scan").get("enemies") or [])
                        if all(k in e for k in ("x", "y", "z")) and
                        ((e["x"] - pos[0]) ** 2 + (e["y"] - pos[1]) ** 2 + (e["z"] - pos[2]) ** 2) ** 0.5 <= 15]
            out["nearby"] = [{k: a.get(k) for k in ("name", "x", "y", "z", "hp", "anim", "state") if a.get(k) is not None}
                             for a in rows[:60]]
        out["telemetry"] = hist
        return out


TELEMETRY = None
RECORDER = None
SEG_SECS = 2
SEG_WRAP = 40
PRE_ROLL = 6.0
OPENED_AT = [0.0]


def game_hwnd():
    """The visible window of this session's game (GAME_DIR), else of any longshot.exe."""
    found = []
    PROC = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)

    def cb(hwnd, _):
        if user32.IsWindowVisible(hwnd) and process_name(hwnd) == "longshot.exe":
            n = user32.GetWindowTextLengthW(hwnd)
            buf = ctypes.create_unicode_buffer(n + 1)
            user32.GetWindowTextW(hwnd, buf, n + 1)
            if "Longshot" in buf.value:
                found.append(hwnd)
        return True
    user32.EnumWindows(PROC(cb), 0)
    return found[0] if found else None


class Recorder:
    """A dashcam of the game window: ffmpeg writes SEG_SECS-second segments in a ring of SEG_WRAP files, so a note can
    take the seconds just before it was sent."""

    def __init__(self, session):
        import threading
        self.session = session
        self.dir = os.path.join(notes_dir(session), "clips", "ring")
        os.makedirs(self.dir, exist_ok=True)
        threading.Thread(target=self.loop, daemon=True).start()

    def loop(self):
        import subprocess
        while True:
            hwnd = game_hwnd()
            if not hwnd:
                time.sleep(5)
                continue
            cmd = ["ffmpeg", "-v", "error", "-y", "-f", "gdigrab", "-framerate", "15", "-i", "hwnd=0x%x" % int(hwnd),
                   "-vf", "scale=960:-2", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", "-g", "15",
                   "-f", "segment", "-segment_time", str(SEG_SECS), "-segment_wrap", str(SEG_WRAP), "-reset_timestamps", "1",
                   os.path.join(self.dir, "seg%d.mp4")]
            try:
                subprocess.run(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               creationflags=0x08000000)
            except Exception:
                pass
            time.sleep(3)

    def save(self, out_path, after=2.0, since=None):
        """After `after` seconds, join the finished segments from `since` (default: PRE_ROLL s before the box opened) to now."""
        import subprocess
        import threading
        start = (since if since is not None else (OPENED_AT[0] or time.time()) - PRE_ROLL) - SEG_SECS

        def work():
            time.sleep(after + SEG_SECS)
            segs = sorted((os.path.join(self.dir, f) for f in os.listdir(self.dir) if f.startswith("seg") and f.endswith(".mp4")),
                          key=os.path.getmtime)
            segs = [s for s in segs if os.path.getmtime(s) >= start and os.path.getsize(s) > 0][:-1]
            if not segs:
                return
            lst = out_path + ".txt"
            with open(lst, "w", encoding="utf-8") as f:
                for s in segs:
                    f.write("file '%s'\n" % s.replace("\\", "/"))
            subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", lst, "-c", "copy", out_path],
                           stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=0x08000000)
            try:
                os.remove(lst)
            except OSError:
                pass
        threading.Thread(target=work, daemon=True).start()


def append_note(session, text, source="t-box", context=None):
    text = text.strip()
    if not text:
        return None
    entry = {"kind": "note", "step": None, "text": text, "verdict": None, "source": source}
    if context is None:
        context = (TELEMETRY.context() if TELEMETRY else {}) or read_context(session)
    entry.update(context)
    if RECORDER is not None:
        clip = os.path.join(notes_dir(session), "clips", "tbox-%s.mp4" % time.strftime("%H%M%S"))
        RECORDER.save(clip)
        entry["clip"] = clip
    entry["shot"] = None
    entry["time"] = time.strftime("%H:%M:%S")
    entry["t"] = round(time.time(), 3)
    os.makedirs(notes_dir(session), exist_ok=True)
    with open(os.path.join(notes_dir(session), "notes.jsonl"), "a", encoding="utf-8") as f:
        f.write(json.dumps(entry) + "\n")
    return entry


def ingame_pid(session):
    """The process id of a game that draws its own T box (it writes notes\tbox-ingame.json at start), else 0."""
    try:
        with open(os.path.join(notes_dir(session), "tbox-ingame.json"), encoding="utf-8") as f:
            return int(json.load(f).get("pid", 0))
    except (OSError, ValueError, TypeError):
        return 0


def last_reply(session):
    try:
        with open(os.path.join(notes_dir(session), "messages.jsonl"), encoding="utf-8-sig") as f:
            lines = [x for x in f.read().splitlines() if x.strip()]
        o = json.loads(lines[-1])
        return "%s  %s" % (o.get("time", ""), o.get("text", "")) if isinstance(o, dict) else str(o)
    except (OSError, ValueError, IndexError):
        return ""


def force_foreground(hwnd):
    """Windows refuses SetForegroundWindow from a background process unless it recently had input: a synthetic Alt tap
    makes the call allowed."""
    user32.keybd_event(0x12, 0, 0, 0)
    user32.keybd_event(0x12, 0, 2, 0)
    user32.SetForegroundWindow(hwnd)


WM_KEYDOWN = 0x100
WM_SYSKEYDOWN = 0x104
MODIFIERS = (0x10, 0x11, 0x12, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x5B, 0x5C, 0x14)


class KbdLL(ctypes.Structure):
    _fields_ = [("vk", wt.DWORD), ("scan", wt.DWORD), ("flags", wt.DWORD), ("time", wt.DWORD),
                ("extra", ctypes.c_size_t)]


def key_char(vk, scan):
    """The character a key press makes with the current Shift and Caps Lock state, else ''."""
    state = (ctypes.c_ubyte * 256)()
    if user32.GetAsyncKeyState(0x10) & 0x8000:
        state[0x10] = 0x80
    if user32.GetKeyState(0x14) & 1:
        state[0x14] = 1
    buf = ctypes.create_unicode_buffer(8)
    n = user32.ToUnicode(vk, scan, state, buf, 8, 0)
    return buf.value[:n] if n > 0 else ""


def pending_key(st, vk, trigger_vk, scan=0, char=None):
    """Bookkeeping for a key pressed after T but before the box has focus: typed text goes to st['buf'], Enter/Esc to
    st['early']. The key itself is always kept from the game while pending."""
    if vk in MODIFIERS:
        return
    if vk == trigger_vk and st["theld"]:
        return
    if vk == 13:
        st["early"] = "send"
    elif vk == 27:
        st["early"] = "cancel"
    elif vk == 8:
        st["buf"] = st["buf"][:-1]
    else:
        st["buf"] += key_char(vk, scan) if char is None else char


def draft_path(session):
    return os.path.join(notes_dir(session), "tbox-draft.txt")


def run(session, key="T", proc="longshot.exe"):
    global TELEMETRY, RECORDER
    import tkinter as tk
    TELEMETRY = Telemetry(session)
    RECORDER = Recorder(session)
    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(2)
    except Exception:
        pass
    vk = vk_of(key)
    clash = key_rebinds(session, vk)
    if clash:
        print("tbox: warning: key %s is bound in the game to %s: choose another with --tbox-key" % (key, ", ".join(clash)),
              flush=True)
    root = tk.Tk()
    root.withdraw()
    box = tk.Toplevel(root)
    box.withdraw()
    box.title("Note to the notes file  (Enter sends, Shift+Enter new line, Esc cancels)")
    box.attributes("-topmost", True)
    box.configure(bg="#1e1e1e")
    last = tk.Label(box, text="", anchor="w", justify="left", bg="#1e1e1e", fg="#9cdcfe", font=("Segoe UI", 10))
    last.pack(fill="x", padx=8, pady=(6, 2))
    entry = tk.Text(box, height=3, wrap="word", font=("Segoe UI", 12), bg="#2d2d2d", fg="#ffffff",
                    insertbackground="#ffffff")
    entry.pack(fill="both", expand=True, padx=8, pady=(2, 8))
    toast = tk.Toplevel(root)
    toast.withdraw()
    toast.overrideredirect(True)
    toast.attributes("-topmost", True)
    tlabel = tk.Label(toast, text="", bg="#2e7d32", fg="white", font=("Segoe UI", 11), padx=14, pady=6)
    tlabel.pack()
    st = {"game": None, "rect": (0, 0, 800, 600), "open": 0.0, "was": False, "pending": False, "buf": "", "early": None,
          "theld": False, "restore": None, "restore_t": 0.0}

    def show_toast(msg, rect):
        tlabel.config(text=msg)
        toast.update_idletasks()
        toast.geometry("+%d+%d" % (rect[0] + (rect[2] - toast.winfo_reqwidth()) // 2, rect[1] + rect[3] - 120))
        toast.deiconify()
        toast.after(1400, toast.withdraw)

    def save_draft():
        try:
            os.makedirs(notes_dir(session), exist_ok=True)
            with open(draft_path(session), "w", encoding="utf-8") as f:
                f.write(entry.get("1.0", "end").rstrip("\n"))
        except OSError:
            pass

    def close(send):
        text = entry.get("1.0", "end").strip()
        box.withdraw()
        st["restore"], st["restore_t"] = st["game"], time.time()
        if send and text:
            append_note(session, text)
            show_toast("note saved", st["rect"])
            entry.delete("1.0", "end")
        elif not send:
            save_draft()
            entry.delete("1.0", "end")
        st["game"] = None
        st["pending"] = False

    def on_key(e):
        if e.keysym == "Escape":
            close(False)
            return "break"
        if e.keysym in ("Return", "KP_Enter") and not (e.state & 0x1):
            close(True)
            return "break"
        if st["theld"] and e.keycode == vk:
            return "break"
        return None

    entry.bind("<Key>", on_key)
    entry.bind("<KeyRelease>", lambda e: save_draft())

    HOOK = ctypes.WINFUNCTYPE(ctypes.c_ssize_t, ctypes.c_int, ctypes.c_size_t, ctypes.c_size_t)
    user32.CallNextHookEx.restype = ctypes.c_ssize_t
    user32.CallNextHookEx.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_size_t, ctypes.c_size_t]
    user32.SetWindowsHookExW.restype = ctypes.c_void_p
    user32.SetWindowsHookExW.argtypes = [ctypes.c_int, HOOK, ctypes.c_void_p, wt.DWORD]
    kernel32.GetModuleHandleW.restype = ctypes.c_void_p
    kernel32.GetModuleHandleW.argtypes = [wt.LPCWSTR]
    user32.ToUnicode.argtypes = [wt.UINT, wt.UINT, ctypes.c_void_p, wt.LPWSTR, ctypes.c_int, wt.UINT]

    def hook(n, w, l):
        try:
            if n >= 0 and st["pending"]:
                k = ctypes.cast(l, ctypes.POINTER(KbdLL)).contents
                if k.vk not in MODIFIERS:
                    if w in (WM_KEYDOWN, WM_SYSKEYDOWN):
                        pending_key(st, k.vk, vk, k.scan)
                    return 1
        except Exception:
            pass
        return user32.CallNextHookEx(None, n, w, l)

    st["hook_ref"] = HOOK(hook)
    st["hook"] = user32.SetWindowsHookExW(13, st["hook_ref"], kernel32.GetModuleHandleW(None), 0)

    def ready():
        st["pending"] = False
        entry.focus_force()
        if st["buf"]:
            entry.insert("end", st["buf"])
        st["buf"] = ""
        early, st["early"] = st["early"], None
        if early:
            close(early == "send")

    def open_box(hwnd, rect):
        st["game"], st["rect"], st["open"] = hwnd, rect, time.time()
        st["buf"], st["early"], st["pending"], st["theld"] = "", None, True, True
        OPENED_AT[0] = time.time()
        w = max(360, min(rect[2] - 40, 760))
        box.geometry("%dx%d+%d+%d" % (w, 140, rect[0] + (rect[2] - w) // 2, rect[1] + rect[3] - 160))
        c = last_reply(session)
        last.config(text=("Reply: " + c[:300]) if c else "Reply: (nothing yet)", wraplength=w - 20)
        box.deiconify()
        box.update_idletasks()
        h = box.winfo_id()
        force_foreground(user32.GetParent(h) or h)
        entry.focus_force()

    def poll():
        t_down = key_down(vk)
        if not t_down:
            st["theld"] = False
        down = t_down and not key_down(0x11) and not key_down(0x12)
        if down and not st["was"] and st["game"] is None and st["restore"] is None:
            fg = foreground(proc)
            if fg and window_pid(fg[0]) != ingame_pid(session):
                open_box(*fg)
        st["was"] = down
        if st["pending"]:
            fg = user32.GetForegroundWindow()
            if (fg and window_pid(fg) == os.getpid()) or time.time() - st["open"] > 1.5:
                ready()
        elif st["game"] is not None:
            fg = user32.GetForegroundWindow()
            if fg and fg == st["game"] and time.time() - st["open"] > 1.0:
                h = box.winfo_id()
                force_foreground(user32.GetParent(h) or h)
                entry.focus_force()
        if st["restore"] is not None:
            held = key_down(13) or key_down(27) or key_down(0x10)
            if not held or time.time() - st["restore_t"] > 3.0:
                force_foreground(st["restore"])
                st["restore"] = None
        root.after(15, poll)

    print("tbox: watching for %s while %s is in front (session %s)" % (key, proc, session), flush=True)
    poll()
    root.mainloop()


if __name__ == "__main__":
    args = sys.argv[1:]
    session = args.pop(0) if args and not args[0].startswith("-") else PATHS.session()
    opts = {"--key": "T", "--proc": "longshot.exe", "--game-dir": ""}
    for o in opts:
        if o in args:
            opts[o] = args[args.index(o) + 1]
    GAME_DIR = os.path.normpath(opts["--game-dir"]).lower() if opts["--game-dir"] else ""
    run(session, opts["--key"], opts["--proc"])
