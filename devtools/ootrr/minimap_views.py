"""The minimap shows one of several fixed views of a floor and switches as Link moves; within a view the map stands still
and only his arrow moves. Group a run's minimap samples by view (the minimap pictures compared), fit one scale for the whole
map and an offset per view, and save one fit per view with the sample points that chose it (the placer sends the view whose
points are nearest Link).

    python -I minimap_views.py [--run RUN] tour [--map M] [--grid 10]   warp Link round the map's markers and sample each
    python -I minimap_views.py [--run RUN] solve [--map M] [--save]      group, fit, report (and save to the fits file)
"""
import argparse
import json
import math
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import minimap_fit as MF

SAME_VIEW: float = 10.0
CROP: tuple = (0.83, 0.70, 0.97, 0.94)


def view_key(path: str):
    from PIL import Image
    im = Image.open(path).convert("L")
    w, h = im.size
    return im.crop((int(w * CROP[0]), int(h * CROP[1]), int(w * CROP[2]), int(h * CROP[3]))).resize((64, 64))


def picture_gap(a, b) -> float:
    from PIL import ImageChops, ImageStat
    return float(ImageStat.Stat(ImageChops.difference(a, b)).mean[0])


def group(samples: list[dict], gap=picture_gap, key=view_key, same: float = SAME_VIEW) -> list[list[int]]:
    keys: list = [key(s["shot"]) for s in samples]
    groups: list[list[int]] = []
    for i, k in enumerate(keys):
        for g in groups:
            if gap(keys[g[0]], k) < same:
                g.append(i)
                break
        else:
            groups.append([i])
    return groups


def fit_views(samples: list[dict], groups: list[list[int]]) -> dict:
    nx = dx = nz = dz = 0.0
    for g in groups:
        if len(g) < 2:
            continue
        mx: float = sum(samples[i]["pos"][0] for i in g) / len(g)
        mz: float = sum(samples[i]["pos"][2] for i in g) / len(g)
        mu: float = sum(samples[i]["u"] for i in g) / len(g)
        mv: float = sum(samples[i]["v"] for i in g) / len(g)
        for i in g:
            x, z = samples[i]["pos"][0] - mx, samples[i]["pos"][2] - mz
            nx += x * (samples[i]["u"] - mu)
            dx += x * x
            nz += z * (samples[i]["v"] - mv)
            dz += z * z
    if dx <= 0 or dz <= 0:
        raise SystemExit("need a view with 2+ samples apart in both x and z to find the scale")
    su: float = nx / dx
    sv: float = nz / dz
    views: list[dict] = []
    for g in groups:
        cu: float = sum(samples[i]["u"] - su * samples[i]["pos"][0] for i in g) / len(g)
        cv: float = sum(samples[i]["v"] - sv * samples[i]["pos"][2] for i in g) / len(g)
        res: list[float] = [math.hypot((su * samples[i]["pos"][0] + cu - samples[i]["u"]) * MF.REF_W,
                                       (sv * samples[i]["pos"][2] + cv - samples[i]["v"]) * MF.REF_H) for i in g]
        views.append({"m": [su, 0.0, cu, 0.0, sv, cv], "pts": [[round(v, 2) for v in samples[i]["pos"]] for i in g],
                      "n": len(g), "rms_px": round(math.sqrt(sum(r * r for r in res) / len(res)), 2), "max_px": round(max(res), 2)})
    return {"su": su, "sv": sv, "views": views}


def tour_spots(points: list[dict], map_name: str, grid: float) -> list[list[float]]:
    seen: set = set()
    out: list[list[float]] = []
    for p in points:
        if p.get("map") != map_name:
            continue
        x, y, z = (float(v) for v in p["pos"])
        cell: tuple = (round(x / grid), round(y / 4.0), round(z / grid))
        if cell in seen:
            continue
        seen.add(cell)
        out.append([x, y + 0.3, z])
    return out


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--run")
    p.add_argument("--session", default=MF.DEFAULT_SESSION)
    p.add_argument("--ctl", default=MF.DEFAULT_CTL)
    p.add_argument("--fits", default=MF.DEFAULT_FITS)
    sub = p.add_subparsers(dest="command", required=True)
    q = sub.add_parser("tour")
    q.add_argument("--map")
    q.add_argument("--grid", type=float, default=10.0)
    q = sub.add_parser("solve")
    q.add_argument("--map")
    q.add_argument("--save", action="store_true")
    q.add_argument("--same", type=float, default=SAME_VIEW, help="pictures closer than this (mean grey difference) are one view")
    q.add_argument("--reject", type=float, default=15.0, help="leave out samples further off than this (px at 1920x1080) and fit again")
    a = p.parse_args(argv)
    run: str = a.run or MF.latest_run(a.session)
    spath: str = os.path.join(run, "minimap_samples.json")
    samples: list[dict] = MF.load_json(spath, [])
    br = MF.Bridge(a.ctl, run)
    if a.command == "tour":
        map_name: str = a.map or br.status().get("map") or ""
        pts: list[dict] = MF.load_json(os.path.join(a.session, "notes", "spawnpoints.json"), [])
        pts = pts["points"] if isinstance(pts, dict) else pts
        spots: list[list[float]] = tour_spots(pts, map_name, a.grid)
        print("touring %d spots on %s" % (len(spots), map_name), flush=True)
        for i, spot in enumerate(spots):
            br.call("warp", *["%.2f" % v for v in spot])
            time.sleep(3.0)
            st: dict = br.status()
            if st.get("map") != map_name:
                print("left the map at spot %d: stop" % i, flush=True)
                break
            try:
                s: dict = MF.take_sample(br, run, MF.SEARCH, "tour%d" % i)
            except SystemExit as ex:
                print("spot %d skipped: %s" % (i, ex), flush=True)
                continue
            samples.append(s)
            MF.save_json(spath, samples)
            print("spot %d: Link %.1f %.1f %.1f -> u %.4f v %.4f (%d px)" % (i, *s["pos"], s["u"], s["v"], s["pixels"]), flush=True)
        return 0
    map_name = a.map or (samples[-1]["map"] if samples else "")
    mine: list[dict] = [s for s in samples if s["map"] == map_name and 3 <= s.get("pixels", 0) <= 30]
    groups: list[list[int]] = group(mine, same=a.same)
    r: dict = fit_views(mine, groups)
    for _ in range(3):
        bad: set = set()
        for g in groups:
            for i in g:
                v: dict = next(x for x in r["views"] if [round(c, 2) for c in mine[i]["pos"]] in x["pts"])
                m: list[float] = v["m"]
                err: float = math.hypot((m[0] * mine[i]["pos"][0] + m[2] - mine[i]["u"]) * MF.REF_W, (m[4] * mine[i]["pos"][2] + m[5] - mine[i]["v"]) * MF.REF_H)
                if err > a.reject:
                    bad.add(i)
        if not bad:
            break
        print("left out %d samples more than %.0f px off: %s" % (len(bad), a.reject, ", ".join(mine[i].get("label", str(i)) for i in sorted(bad))))
        groups = [[i for i in g if i not in bad] for g in groups]
        groups = [g for g in groups if g]
        r = fit_views(mine, groups)
    print("%s: %d samples, %d views; scale %.2f px/unit across, %.2f px/unit down (1920x1080)" % (
        map_name, len(mine), len(groups), r["su"] * MF.REF_W, r["sv"] * MF.REF_H))
    for k, v in enumerate(r["views"]):
        print("  view %d: %d samples, rms %.2f px, max %.2f px" % (k, v["n"], v["rms_px"], v["max_px"]))
    if a.save:
        fits: dict = MF.load_json(a.fits, {})
        fits[map_name] = [dict(v, clip=list(MF.CLIP), t=time.strftime("%H:%M:%S")) for v in r["views"]]
        MF.save_json(a.fits, fits)
        print("saved %d view fits for %s to %s" % (len(r["views"]), map_name, a.fits))
    return 0


if __name__ == "__main__":
    sys.exit(main())
