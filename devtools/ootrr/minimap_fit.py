"""Line the debug radar up with the game's own minimap: fit world (x, z) to the picture from Link's red arrow on the minimap.

    python -I minimap_fit.py [--run RUN | --session S] sample          one sample: Link's place (status) and his arrow (a shot)
    python -I minimap_fit.py [--run RUN] solve [--map M] [--affine] [--band] [--apply]
    python -I minimap_fit.py [--run RUN] push [--map M]                 send the saved fit for the map (default Link's)
    python -I minimap_fit.py [--run RUN] samples [--clear]              list (or forget) this run's samples
    python -I minimap_fit.py detect SHOT.png [--debug OUT.png]          find the arrow in a saved picture
    python -I minimap_fit.py --from-shots LIST.json solve [--save]      offline: [{"shot": png, "pos": [x, y, z], "map": M}, ...]

u and v are shares of the picture's width and height (the bridge's radar fit takes them so). The fit is u = a x + c, v = b z + d
(north-up, axis flips allowed) or, with --affine, u = ux x + uz z + u0, v = vx x + vz z + v0. Samples: RUN/minimap_samples.json.
Fits: FITS (default the hands-on notes' minimap_fits.json), {map: [{"m": [ux, uz, u0, vx, vz, v0], "ylo", "yhi", "clip", "rms_px", "n"}]}.
The shot is the game's own picture; the radar is hidden for it anyway (show and labels3d off, then put back) so our red dots
never count. Needs PIL for shots (no other installs).
"""
import argparse
import json
import math
import os
import subprocess
import sys
import tempfile
import time

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import paths as PATHS  # noqa: E402

DEFAULT_SESSION: str = PATHS.session()
DEFAULT_CTL: str = PATHS.longshot_ctl()
DEFAULT_FITS: str = os.path.join(DEFAULT_SESSION, "notes", "minimap_fits.json")
SEARCH: tuple = (0.825, 0.685, 0.975, 0.955)
CLIP: tuple = (0.828, 0.690, 0.971, 0.949)
REF_W: int = 1920
REF_H: int = 1080


def last_json(text: str) -> dict:
    for line in reversed(text.strip().splitlines()):
        line = line.strip()
        if line.startswith("{"):
            try:
                return json.loads(line)
            except ValueError:
                pass
    return {}


def solve3(a: list[list[float]], b: list[float]) -> list[float] | None:
    m: list[list[float]] = [row[:] + [b[i]] for i, row in enumerate(a)]
    n: int = len(m)
    for col in range(n):
        piv: int = max(range(col, n), key=lambda r: abs(m[r][col]))
        if abs(m[piv][col]) < 1e-12:
            return None
        m[col], m[piv] = m[piv], m[col]
        for r in range(n):
            if r != col:
                f: float = m[r][col] / m[col][col]
                for k in range(col, n + 1):
                    m[r][k] -= f * m[col][k]
    return [m[i][n] / m[i][i] for i in range(n)]


def fit_line(xs: list[float], ys: list[float]) -> tuple[float, float] | None:
    n: int = len(xs)
    if n < 2:
        return None
    mx: float = sum(xs) / n
    my: float = sum(ys) / n
    sxx: float = sum((x - mx) ** 2 for x in xs)
    if sxx < 1e-9:
        return None
    k: float = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
    return k, my - k * mx


def fit_plane(xs: list[float], zs: list[float], ys: list[float]) -> list[float] | None:
    if len(xs) < 3:
        return None
    a: list[list[float]] = [[0.0] * 3 for _ in range(3)]
    b: list[float] = [0.0] * 3
    for x, z, y in zip(xs, zs, ys):
        row: list[float] = [x, z, 1.0]
        for i in range(3):
            b[i] += row[i] * y
            for j in range(3):
                a[i][j] += row[i] * row[j]
    return solve3(a, b)


def solve(samples: list[dict], affine: bool = False) -> dict:
    xs: list[float] = [float(s["pos"][0]) for s in samples]
    zs: list[float] = [float(s["pos"][2]) for s in samples]
    us: list[float] = [float(s["u"]) for s in samples]
    vs: list[float] = [float(s["v"]) for s in samples]
    if affine:
        pu: list[float] | None = fit_plane(xs, zs, us)
        pv: list[float] | None = fit_plane(xs, zs, vs)
        if pu is None or pv is None:
            raise ValueError("affine fit: needs 3 or more samples not on one line")
        m: list[float] = [pu[0], pu[1], pu[2], pv[0], pv[1], pv[2]]
    else:
        lu: tuple | None = fit_line(xs, us)
        lv: tuple | None = fit_line(zs, vs)
        if lu is None or lv is None:
            raise ValueError("axis fit: needs 2 or more samples apart in both x and z")
        m = [lu[0], 0.0, lu[1], 0.0, lv[0], lv[1]]
    res: list[float] = []
    for x, z, u, v in zip(xs, zs, us, vs):
        du: float = (m[0] * x + m[1] * z + m[2] - u) * REF_W
        dv: float = (m[3] * x + m[4] * z + m[5] - v) * REF_H
        res.append(math.hypot(du, dv))
    rms: float = math.sqrt(sum(r * r for r in res) / len(res)) if res else 0.0
    return {"m": m, "residuals_px": res, "rms_px": rms, "max_px": max(res) if res else 0.0, "n": len(samples), "affine": affine}


def red_mask_centroid(img, rect: tuple = SEARCH) -> dict:
    img = img.convert("RGB")
    w, h = img.size
    x0: int = max(0, int(rect[0] * w))
    y0: int = max(0, int(rect[1] * h))
    x1: int = min(w, int(math.ceil(rect[2] * w)))
    y1: int = min(h, int(math.ceil(rect[3] * h)))
    px = img.load()
    mask: set[tuple[int, int]] = set()
    for y in range(y0, y1):
        for x in range(x0, x1):
            r, g, b = px[x, y]
            if r >= 170 and r - max(g, b) >= 100 and g < 110 and b < 110:
                mask.add((x, y))
    best: list[tuple[int, int]] = []
    seen: set[tuple[int, int]] = set()
    for p in mask:
        if p in seen:
            continue
        comp: list[tuple[int, int]] = []
        stack: list[tuple[int, int]] = [p]
        seen.add(p)
        while stack:
            q = stack.pop()
            comp.append(q)
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    n = (q[0] + dx, q[1] + dy)
                    if n in mask and n not in seen:
                        seen.add(n)
                        stack.append(n)
        if len(comp) > len(best):
            best = comp
    if len(best) < 3:
        raise ValueError("no red arrow in the minimap square (%d red pixels there)" % len(mask))
    cx: float = sum(p[0] for p in best) / len(best) + 0.5
    cy: float = sum(p[1] for p in best) / len(best) + 0.5
    return {"u": cx / w, "v": cy / h, "px": [cx, cy], "pixels": len(best), "red": len(mask), "parts": len(best) != len(mask), "w": w, "h": h}


def detect(path: str, rect: tuple = SEARCH, debug: str | None = None) -> dict:
    from PIL import Image, ImageDraw
    with Image.open(path) as im:
        im.load()
        out: dict = red_mask_centroid(im, rect)
        if debug:
            w, h = im.size
            box: tuple = (int(rect[0] * w), int(rect[1] * h), int(rect[2] * w), int(rect[3] * h))
            crop = im.convert("RGB").crop(box).resize(((box[2] - box[0]) * 4, (box[3] - box[1]) * 4))
            d = ImageDraw.Draw(crop)
            cx: float = (out["px"][0] - box[0]) * 4
            cy: float = (out["px"][1] - box[1]) * 4
            d.line((cx - 12, cy, cx + 12, cy), fill=(0, 255, 255), width=2)
            d.line((cx, cy - 12, cx, cy + 12), fill=(0, 255, 255), width=2)
            crop.save(debug)
    return out


def load_json(path: str, default):
    try:
        with open(path, encoding="utf-8-sig") as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def save_json(path: str, data) -> None:
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    tmp: str = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1)
    os.replace(tmp, path)


def fit_for(fits: dict, map_name: str, y: float | None = None) -> dict | None:
    best: dict | None = None
    for f in fits.get(map_name, []) or []:
        lo: float = float(f.get("ylo", -1e30))
        hi: float = float(f.get("yhi", 1e30))
        if y is not None and not (lo <= y <= hi):
            continue
        if best is None or hi - lo < float(best.get("yhi", 1e30)) - float(best.get("ylo", -1e30)):
            best = f
    return best


def view_for(views: list[dict], pos: list[float] | None) -> dict | None:
    best: tuple[float, dict] | None = None
    for f in views:
        for p in f.get("pts") or []:
            d: float = math.dist([float(v) for v in p], [float(v) for v in pos]) if pos else 0.0
            if best is None or d < best[0]:
                best = (d, f)
    return best[1] if best else None


def fit_requests(map_name: str, fits: dict, pos: list[float] | None = None) -> list[dict]:
    out: list[dict] = []
    chosen: list[dict] = fits.get(map_name, []) or []
    views: list[dict] = [f for f in chosen if f.get("pts")]
    if views:
        one: dict | None = view_for(views, pos)
        chosen = [one] if one else []
        out.append({"cmd": "radar", "op": "fit", "map": map_name, "clear": True})
    for f in chosen:
        m: list[float] = [float(v) for v in f["m"]]
        req: dict = {"cmd": "radar", "op": "fit", "map": map_name, "ux": m[0], "uz": m[1], "u0": m[2], "vx": m[3], "vz": m[4], "v0": m[5]}
        if "ylo" in f:
            req["ylo"] = float(f["ylo"])
        if "yhi" in f:
            req["yhi"] = float(f["yhi"])
        clip = f.get("clip")
        if clip and len(clip) == 4:
            req["clip"] = " ".join("%.5f" % float(c) for c in clip)
        out.append(req)
    return out


def put_fit(fits: dict, map_name: str, fit: dict) -> dict:
    lst: list[dict] = [f for f in fits.get(map_name, []) or [] if (f.get("ylo"), f.get("yhi")) != (fit.get("ylo"), fit.get("yhi"))]
    lst.append(fit)
    fits[map_name] = lst
    return fits


class Bridge:
    def __init__(self, ctl: str, run: str) -> None:
        self.cmd: list[str] = [sys.executable, "-I", ctl, "--run", run]

    def call(self, *args: str, timeout: int = 40) -> str:
        r = subprocess.run(self.cmd + list(args), capture_output=True, text=True, timeout=timeout)
        return r.stdout + r.stderr

    def raw(self, req: dict) -> dict:
        return last_json(self.call("raw", json.dumps(req)))

    def status(self) -> dict:
        return last_json(self.call("--json", "status")).get("game") or {}

    def shot(self, path: str) -> dict:
        return last_json(self.call("--json", "shot", path, timeout=60))


def latest_run(session: str) -> str:
    root: str = os.path.join(session, "runs")
    dirs: list[str] = [os.path.join(root, d) for d in os.listdir(root) if os.path.isdir(os.path.join(root, d))]
    return max(dirs, key=os.path.getmtime)


def take_sample(br: Bridge, run: str, rect: tuple, label: str) -> dict:
    st: dict = br.status()
    pos = st.get("pos")
    map_name: str = st.get("map") or ""
    if not map_name or not pos or len(pos) != 3 or st.get("loading"):
        raise SystemExit("no game state (map %r pos %r): is Link in play?" % (map_name, pos))
    before: dict = br.raw({"cmd": "radar"}).get("radar") or {}
    hide: bool = bool(before.get("show") or before.get("labels3d"))
    shots: str = os.path.join(run, "minimap-shots")
    os.makedirs(shots, exist_ok=True)
    path: str = os.path.join(shots, "mm_%s_%d.png" % (map_name, int(time.time() * 1000)))
    try:
        if hide:
            br.raw({"cmd": "radar", "op": "options", "show": False, "labels3d": False})
            br.raw({"cmd": "wait", "event": "present", "frames": 3, "timeout": 5})
        got: dict = br.shot(path)
    finally:
        if hide:
            br.raw({"cmd": "radar", "op": "options", "show": bool(before.get("show")), "labels3d": bool(before.get("labels3d"))})
    if not os.path.exists(path):
        raise SystemExit("the shot failed: %s" % json.dumps(got)[:300])
    after: dict = br.status()
    pos2 = after.get("pos") or pos
    if math.dist([float(v) for v in pos], [float(v) for v in pos2]) > 0.5:
        raise SystemExit("Link moved during the sample (%.2f m): stand still and sample again" % math.dist(pos, pos2))
    found: dict = detect(path, rect, os.path.splitext(path)[0] + "_found.png")
    return {"map": map_name, "pos": [float(v) for v in pos], "u": found["u"], "v": found["v"], "shot": path, "w": found["w"], "h": found["h"],
            "pixels": found["pixels"], "label": label, "t": time.strftime("%H:%M:%S")}


def parse_rect(text: str | None, default: tuple) -> tuple:
    if not text:
        return default
    v: list[float] = [float(x) for x in text.replace(",", " ").split()]
    if len(v) != 4:
        raise SystemExit("a rect is four numbers u0,v0,u1,v1")
    return tuple(v)


def report(map_name: str, r: dict) -> str:
    m: list[float] = r["m"]
    lines: list[str] = ["%s: %d samples, %s fit" % (map_name, r["n"], "affine" if r["affine"] else "axis")]
    if r["affine"]:
        lines.append("  u = %.6g x + %.6g z + %.6g;  v = %.6g x + %.6g z + %.6g" % tuple(m))
    else:
        lines.append("  u = %.6g x + %.6g;  v = %.6g z + %.6g   (a b c d = %.6g %.6g %.6g %.6g)" % (m[0], m[2], m[4], m[5], m[0], m[4], m[2], m[5]))
    lines.append("  residuals (px at %dx%d): %s; rms %.2f, max %.2f" % (REF_W, REF_H, " ".join("%.1f" % x for x in r["residuals_px"]), r["rms_px"], r["max_px"]))
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--run", help="the game's run folder (default the session's newest)")
    p.add_argument("--session", default=os.environ.get("PANEL_SESSION", DEFAULT_SESSION))
    p.add_argument("--ctl", default=DEFAULT_CTL, help="longshot_ctl.py (the radar build's, which knows radar fit)")
    p.add_argument("--fits", default=DEFAULT_FITS)
    p.add_argument("--rect", help="where to look for the arrow, u0,v0,u1,v1 shares of the picture (default %s)" % ",".join(map(str, SEARCH)))
    p.add_argument("--clip", help="the minimap's square sent with the fit (default %s)" % ",".join(map(str, CLIP)))
    p.add_argument("--from-shots", dest="from_shots", help="a JSON list of {shot, pos, map}: fit offline")
    sub = p.add_subparsers(dest="command", required=True)
    q = sub.add_parser("sample")
    q.add_argument("--label", default="")
    q = sub.add_parser("solve")
    q.add_argument("--map")
    q.add_argument("--affine", action="store_true", help="a whole affine (rotation allowed), 3+ samples off one line")
    q.add_argument("--band", action="store_true", help="the fit holds only for Link's height band of the samples (+-3 m): one floor")
    q.add_argument("--apply", action="store_true", help="save the fit and send it to the game (radar fit)")
    q.add_argument("--save", action="store_true", help="save the fit only")
    q.add_argument("--drop", type=int, action="append", default=[], help="leave out sample N (as samples lists them)")
    q = sub.add_parser("push")
    q.add_argument("--map")
    q = sub.add_parser("samples")
    q.add_argument("--clear", action="store_true")
    q = sub.add_parser("detect")
    q.add_argument("shot")
    q.add_argument("--debug", help="write a 4x crop with the found point marked")
    a = p.parse_args(argv)
    rect: tuple = parse_rect(a.rect, SEARCH)
    clip: tuple = parse_rect(a.clip, CLIP)
    if a.command == "detect":
        print(json.dumps(detect(a.shot, rect, a.debug)))
        return 0
    run: str | None = None
    samples: list[dict]
    if a.from_shots:
        samples = []
        for one in load_json(a.from_shots, []):
            found: dict = detect(one["shot"], rect)
            samples.append({"map": one.get("map", ""), "pos": one["pos"], "u": found["u"], "v": found["v"], "shot": one["shot"]})
    else:
        run = a.run or latest_run(a.session)
        samples = load_json(os.path.join(run, "minimap_samples.json"), [])
    spath: str | None = os.path.join(run, "minimap_samples.json") if run else None
    br: Bridge | None = Bridge(a.ctl, run) if run else None
    if a.command == "sample":
        if not br or not spath:
            raise SystemExit("sample needs a run")
        s: dict = take_sample(br, run, rect, a.label)
        samples.append(s)
        save_json(spath, samples)
        same: int = sum(1 for x in samples if x["map"] == s["map"])
        print("sample %d (%s, %d for this map): Link %.1f %.1f %.1f -> arrow u %.4f v %.4f (%d px); %s" % (len(samples) - 1, s["map"], same, *s["pos"], s["u"], s["v"], s["pixels"], s["shot"]))
        return 0
    if a.command == "samples":
        if a.clear and spath:
            save_json(spath, [])
            print("samples cleared")
            return 0
        for i, s in enumerate(samples):
            print("%d  %s  %.1f %.1f %.1f  u %.4f v %.4f  %s" % (i, s["map"], *s["pos"], s["u"], s["v"], s.get("label", "")))
        return 0
    fits: dict = load_json(a.fits, {})
    if a.command == "push":
        map_name: str = a.map or (br.status().get("map") if br else "") or ""
        reqs: list[dict] = fit_requests(map_name, fits)
        if not reqs:
            print("no saved fit for %r" % map_name)
            return 1
        for req in reqs:
            out: dict = br.raw(req) if br else {}
            print("sent %s: %s" % (json.dumps(req), "ok" if out.get("ok") else json.dumps(out)[:300]))
        return 0
    if a.command == "solve":
        map_name = a.map or (samples[-1]["map"] if samples else "")
        use: list[dict] = [s for i, s in enumerate(samples) if s.get("map", "") == map_name and i not in a.drop]
        r: dict = solve(use, a.affine)
        print(report(map_name, r))
        if not a.affine and len(use) >= 3:
            try:
                ra: dict = solve(use, True)
                print("  (affine would give rms %.2f px, cross terms uz %.3g vx %.3g)" % (ra["rms_px"], ra["m"][1], ra["m"][3]))
            except ValueError:
                pass
        fit: dict = {"m": r["m"], "clip": list(clip), "rms_px": round(r["rms_px"], 2), "n": r["n"], "t": time.strftime("%Y-%m-%d %H:%M")}
        if a.band:
            ys: list[float] = [float(s["pos"][1]) for s in use]
            fit["ylo"] = round(min(ys) - 3.0, 2)
            fit["yhi"] = round(max(ys) + 3.0, 2)
        if a.apply or a.save:
            save_json(a.fits, put_fit(fits, map_name, fit))
            print("saved to %s" % a.fits)
        if a.apply:
            if not br:
                raise SystemExit("--apply needs a run")
            for req in fit_requests(map_name, {map_name: [fit]}):
                out = br.raw(req)
                print("sent: %s" % ("ok, " + ("map mode on" if (out.get("radar") or {}).get("fit") else "map mode not on now (Link elsewhere, or mode disc)") if out.get("ok") else json.dumps(out)[:300]))
        return 0
    return 2


if __name__ == "__main__":
    sys.exit(main())
