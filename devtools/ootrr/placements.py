"""Lists the placed actors of one map from the game's own map files, offline (no game).

For comparing with the Longshot bridge's `actors` reply in a live game: each row is the placement's
`Gyaml` (the actor type, what the bridge reads as the actor's name), its `Hash` (the placement id OOTRR's
randomizer and tracker use; OOTRR writes it in decimal, e.g. "chest:10905582354954996284"), and `Translate`.

    python -I placements.py --romfs <extracted game romfs> --map Market_Child [--prefix Obj_] [--json out.json]
        [--near X Y Z R]

Reads Banc/<Area>/<map>_Static.bcett.byml.zs and _Dynamic.bcett.byml.zs (zstd, then Byml v7/v8). Needs the
`zstandard` module. Read-only; the romfs is never written.
"""
import argparse
import json
import math
import os
import struct
import sys

try:
    import zstandard
except ImportError:  # pragma: no cover
    zstandard = None


def byml(b):
    if b[:2] not in (b"YB", b"BY"):
        raise ValueError("not a Byml file")
    e = "<" if b[:2] == b"YB" else ">"
    kt, st, rt = struct.unpack(e + "III", b[4:16])

    def u32(o):
        return struct.unpack(e + "I", b[o:o + 4])[0]

    def strtab(o):
        if not o:
            return []
        n = u32(o) >> 8
        offs = struct.unpack(e + "%dI" % (n + 1), b[o + 4:o + 8 + 4 * n])
        return [b[o + offs[i]:b.index(b"\0", o + offs[i])].decode("utf-8", "replace") for i in range(n)]

    keys, strs = strtab(kt), strtab(st)

    def node(t, v):
        if t == 0xA0:
            return strs[v]
        if t == 0xC0:
            return arr(v)
        if t == 0xC1:
            return dct(v)
        if t == 0xD0:
            return bool(v)
        if t == 0xD1:
            return struct.unpack(e + "i", struct.pack(e + "I", v))[0]
        if t == 0xD2:
            return struct.unpack(e + "f", struct.pack(e + "I", v))[0]
        if t == 0xD3:
            return v
        if t == 0xD4:
            return struct.unpack(e + "q", b[v:v + 8])[0]
        if t == 0xD5:
            return struct.unpack(e + "Q", b[v:v + 8])[0]
        if t == 0xD6:
            return struct.unpack(e + "d", b[v:v + 8])[0]
        if t == 0xFF:
            return None
        return None

    def arr(o):
        n = u32(o) >> 8
        ts = b[o + 4:o + 4 + n]
        vo = (o + 4 + n + 3) & ~3
        return [node(ts[i], u32(vo + 4 * i)) for i in range(n)]

    def dct(o):
        n = u32(o) >> 8
        d = {}
        for i in range(n):
            q = o + 4 + 8 * i
            ki = struct.unpack(e + "I", b[q:q + 3] + b"\0")[0] if e == "<" else struct.unpack(e + "I", b"\0" + b[q:q + 3])[0]
            d[keys[ki]] = node(b[q + 3], u32(q + 4))
        return d

    if not rt:
        return {}
    return dct(rt) if b[rt] == 0xC1 else arr(rt)


def sarc_files(b):
    e = "<" if b[6:8] == b"\xff\xfe" else ">"
    dof = struct.unpack(e + "I", b[12:16])[0]
    n = struct.unpack(e + "H", b[0x1A:0x1C])[0]
    nt = 0x20 + 16 * n + 8
    out = {}
    for i in range(n):
        _, fl, s, en = struct.unpack(e + "IIII", b[0x20 + i * 16:0x30 + i * 16])
        o = (fl & 0xFFFFFF) * 4
        out[b[nt + o:b.index(b"\0", nt + o)].decode()] = b[dof + s:dof + en]
    return out


class Zs:
    """zstd with the game's dictionaries (Pack/ZsDic.pack.zs), each frame decoded with the one its header names."""

    def __init__(self, romfs):
        plain = zstandard.ZstdDecompressor()
        with open(os.path.join(romfs, "Pack", "ZsDic.pack.zs"), "rb") as f:
            pack = plain.stream_reader(f).read()
        self.dicts = {}
        for name, data in sarc_files(pack).items():
            if name.endswith(".zsdic"):
                d = zstandard.ZstdCompressionDict(data)
                self.dicts[d.dict_id()] = d

    def read(self, path):
        with open(path, "rb") as f:
            raw = f.read()
        did = zstandard.get_frame_parameters(raw).dict_id
        dec = zstandard.ZstdDecompressor(dict_data=self.dicts[did]) if did else zstandard.ZstdDecompressor()
        return dec.stream_reader(raw).read()


def hash_hex(h):
    """The bridge's form: '0x' and lower-case hex without leading zeros (C's %#llx)."""
    return "%#x" % h if h else "0"


def map_files(romfs, map_name):
    area = map_name.rsplit("_", 1)[0] if map_name.endswith(("_Child", "_Adult")) else map_name
    folder = os.path.join(romfs, "Banc", area)
    out = []
    for part in ("Static", "Dynamic"):
        p = os.path.join(folder, "%s_%s.bcett.byml.zs" % (map_name, part))
        if os.path.exists(p):
            out.append((part, p))
    return out


def placements(romfs, map_name):
    if zstandard is None:
        raise SystemExit("needs the zstandard module")
    files = map_files(romfs, map_name)
    if not files:
        raise SystemExit("no Banc files for %s under %s" % (map_name, romfs))
    rows = []
    zs = Zs(romfs)
    for part, p in files:
        root = byml(zs.read(p))
        for a in root.get("Actors") or []:
            if not isinstance(a, dict):
                continue
            h = a.get("Hash")
            t = a.get("Translate") or [0, 0, 0]
            rows.append({
                "name": a.get("Gyaml", "?"),
                "hash_id": str(h) if isinstance(h, int) else None,
                "hash": hash_hex(h) if isinstance(h, int) else None,
                "x": round(float(t[0]), 3), "y": round(float(t[1]), 3), "z": round(float(t[2]), 3),
                "file": part,
            })
    return rows


def find(romfs, hashes):
    """Every Banc file's actors whose Hash is one of hashes: prints map, name, hash, position. Returns the number not found."""
    zs = Zs(romfs)
    want = set(hashes)
    seen = set()
    banc = os.path.join(romfs, "Banc")
    for area in sorted(os.listdir(banc)):
        folder = os.path.join(banc, area)
        if not os.path.isdir(folder):
            continue
        for f in sorted(os.listdir(folder)):
            if not f.endswith(".bcett.byml.zs"):
                continue
            try:
                root = byml(zs.read(os.path.join(folder, f)))
            except Exception as ex:  # (a file this reader does not take: say so, go on)
                print("skip %s/%s: %s" % (area, f, ex), file=sys.stderr)
                continue
            for a in root.get("Actors") or []:
                h = a.get("Hash") if isinstance(a, dict) else None
                if h in want:
                    seen.add(h)
                    t = a.get("Translate") or [0, 0, 0]
                    print("%-48s %-34s %-20s %s %.2f %.2f %.2f" % (f.replace(".bcett.byml.zs", ""), a.get("Gyaml", "?"), hash_hex(h), h,
                                                                  t[0], t[1], t[2]))
    for h in hashes:
        if h not in seen:
            print("not found: %s (%d)" % (hash_hex(h), h))
    return len(want - seen)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--romfs", default=os.environ.get("OOTRR_ROMFS"))
    ap.add_argument("--map", help="a map name as status's game.map gives it, e.g. Market_Child")
    ap.add_argument("--prefix", action="append", default=[], help="keep names starting with this (repeatable)")
    ap.add_argument("--near", nargs=4, type=float, metavar=("X", "Y", "Z", "R"))
    ap.add_argument("--json", help="write the rows to this file")
    ap.add_argument("--find", action="append", default=[], metavar="HASH",
                    help="instead: search every map file for this placement hash (0x hex or decimal; repeatable)")
    a = ap.parse_args(argv)
    if a.find:
        return find(a.romfs, [int(h, 0) for h in a.find])
    if not a.map:
        ap.error("--map MAP (or --find HASH)")
    rows = placements(a.romfs, a.map)
    if a.prefix:
        rows = [r for r in rows if any(r["name"].lower().startswith(p.lower()) for p in a.prefix)]
    if a.near:
        x, y, z, r = a.near
        for row in rows:
            row["dist"] = round(math.dist((x, y, z), (row["x"], row["y"], row["z"])), 2)
        rows = sorted((row for row in rows if row["dist"] <= r), key=lambda row: row["dist"])
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump({"map": a.map, "count": len(rows), "placements": rows}, f, indent=1)
    for row in rows:
        print("%-40s %-20s %-22s %9.2f %9.2f %9.2f %s" % (row["name"], row["hash"], row["hash_id"], row["x"], row["y"], row["z"],
                                                          ("d=%.1f" % row["dist"]) if "dist" in row else row["file"]))
    print("%d placements" % len(rows), file=sys.stderr)


if __name__ == "__main__":
    sys.exit(1 if main() else 0)
