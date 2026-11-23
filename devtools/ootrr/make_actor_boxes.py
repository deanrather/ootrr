"""Writes actor_boxes.json: each placer kind's (and each vanilla enemy's) model box from the game's own RSDB/ActorInfo.Product.111.rstbl.byml.zs
(ModelAabbMin and ModelAabbMax, in the actor's own space: feet near y = 0, forward +z), as numbers only: {"Enemy_Stalfos": [[x0, y0, z0], [x1, y1, z1]]}.
Written to cache/ (git-ignored) by build_cache.py:

    python -I make_actor_boxes.py --romfs <extracted game romfs>

Reads the game with the Byml and zstd code of placements.py (needs the zstandard module). The romfs is never written.
"""
import argparse
import json
import os
import sys

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE]
import paths as PATHS  # noqa: E402
import placer_logic as L  # noqa: E402


def box_of(row: dict) -> list[list[float]] | None:
    lo, hi = row.get("ModelAabbMin"), row.get("ModelAabbMax")
    try:
        a: list[float] = [round(float(lo[k]), 2) for k in "XYZ"]
        b: list[float] = [round(float(hi[k]), 2) for k in "XYZ"]
    except (TypeError, KeyError, ValueError):
        return None
    if any(a[i] > b[i] for i in range(3)):
        return None
    return [a, b]


def wanted_names() -> list[str]:
    names: set[str] = {c[0] for c in L.CATALOG}
    for entry in L.load_vanilla().values():
        for e in entry.get("enemies", []):
            if L.real_enemy(e["n"]):
                names.add(e["n"])
    return sorted(names)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--romfs")
    ap.add_argument("--out", default=PATHS.cache("actor_boxes.json"))
    a = ap.parse_args(argv)
    import placements as P
    a.romfs = PATHS.need_romfs(a.romfs)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    z = P.Zs(a.romfs)
    rows: dict[str, dict] = {r["__RowId"]: r for r in P.byml(z.read(os.path.join(a.romfs, "RSDB", "ActorInfo.Product.111.rstbl.byml.zs")))}
    out: dict[str, list] = {}
    missing: list[str] = []
    for n in wanted_names():
        b: list | None = box_of(rows[n]) if n in rows else None
        if b is None:
            missing.append(n)
        else:
            out[n] = b
    with open(a.out, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=0, sort_keys=True)
    print("%d boxes written to %s; %d without one: %s" % (len(out), a.out, len(missing), ", ".join(missing)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
