"""The timed silver rupee rooms the mod ships, read from the ROOMS table of randomizer/core/src/difficulty/gems.rs (no Rust build).

Each room becomes a panel puzzle dict like the ones the Puzzle card marks: switch (facing from switch_rotate), rupees (the Rust
spots minus puzzle_logic.GEM_RISE, so they are Link's feet again), counter, seconds, the chest's placement hash and, when the
room has one, the measured spot beside the switch to warp to (WARPS). The file is the first that exists of $OOTRR_GEMS_RS, the
feature worktree, the main checkout and wt/next.

    python -I shipped_puzzles.py [gems.rs]   prints what it read
"""
import math
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import puzzle_logic as P  # noqa: E402

import paths as PATHS  # noqa: E402

CANDIDATES: tuple[str, ...] = (os.path.join(PATHS.ootrr_checkout(), "randomizer", "core", "src", "difficulty", "gems.rs"),)
WARPS: dict[tuple[str, int], list[float]] = {("KokiriForest_Child", 1): [-81.44, 119.3, 21.8]}
NUM: str = r"-?\d+(?:\.\d+)?"
TRIPLE: str = r"\[\s*(%s)\s*,\s*(%s)\s*,\s*(%s)\s*\]" % (NUM, NUM, NUM)


def path_of(given: str | None = None) -> str | None:
    if given:
        return given if os.path.isfile(given) else None
    for p in [os.environ.get("OOTRR_GEMS_RS", "")] + list(CANDIDATES):
        if p and os.path.isfile(p):
            return p
    return None


def triples(text: str) -> list[list[float]]:
    return [[float(a), float(b), float(c)] for a, b, c in re.findall(TRIPLE, text)]


def consts(text: str) -> dict[str, str]:
    return dict(re.findall(r"pub const ([A-Z_]+): u64 = (\d+);", text))


def field(body: str, name: str) -> str:
    m = re.search(r"\b%s:\s*" % name, body)
    if not m:
        return ""
    i: int = m.end()
    depth: int = 0
    j: int = i
    while j < len(body):
        ch: str = body[j]
        if ch in "[(":
            depth += 1
        elif ch in "])":
            if depth == 0:
                break
            depth -= 1
        elif ch == '"':
            j = body.index('"', j + 1)
        elif ch == "," and depth == 0:
            break
        j += 1
    return body[i:j].strip()


def table(text: str) -> str:
    m = re.search(r"pub const ROOMS: \[Room; \d+\] = \[", text)
    if not m:
        return ""
    end: int = text.find("\n];", m.end())
    end = text.find("];", m.end()) if end < 0 else end
    return text[m.end():end]


def rows(text: str) -> list[str]:
    out: list[str] = []
    body: str = table(text)
    i: int = 0
    while True:
        s: int = body.find("Room {", i)
        if s < 0:
            return out
        depth: int = 0
        j: int = s + len("Room ")
        while j < len(body):
            if body[j] == "{":
                depth += 1
            elif body[j] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        out.append(body[s + len("Room {"):j])
        i = j + 1


def chest_of(token: str, names: dict[str, str]) -> str | None:
    m = re.match(r"Some\(\s*(\w+)\s*\)", token.strip())
    if not m:
        return None
    v: str = m.group(1)
    if v.isdigit():
        return v
    return names.get(v)


def mark(pos: list[float], facing: float = 0.0) -> dict:
    return {"n": 0, "pos": [round(v, 2) for v in pos], "facing": round(facing, 1), "time": "mod"}


def parse(text: str) -> list[dict]:
    names: dict[str, str] = consts(text)
    out: list[dict] = []
    for r in rows(text):
        sw: list[list[float]] = triples(field(r, "switch"))
        rot: list[list[float]] = triples(field(r, "switch_rotate"))
        gems: list[list[float]] = triples(field(r, "gems"))
        counter: list[list[float]] = triples(field(r, "counter"))
        if not sw or not gems:
            continue
        rid: int = int(field(r, "id"))
        mp: str = field(r, "map").strip('"')
        p: dict = {"map": mp, "name": field(r, "name").strip('"'), "seconds": float(field(r, "seconds") or P.DEFAULT_SECONDS),
                   "switch": mark(sw[0], math.degrees(rot[0][1]) if rot else 0.0),
                   "gems": [mark([g[0], round(g[1] - P.GEM_RISE, 2), g[2]]) for g in gems],
                   "counter": mark(counter[0]) if counter else None, "chest": None, "wall_a": None, "wall_b": None,
                   "chest_hash": chest_of(field(r, "chest"), names), "shipped": rid, "warp": WARPS.get((mp, rid))}
        wall: list[list[float]] = triples(field(r, "guard")) if "Wall" in field(r, "guard") else []
        if len(wall) == 2:
            p["wall_a"], p["wall_b"] = mark(wall[0]), mark(wall[1])
        out.append(p)
    return out


def load(given: str | None = None) -> tuple[list[dict], str | None]:
    p: str | None = path_of(given)
    if p is None:
        return [], None
    try:
        with open(p, encoding="utf-8") as f:
            return parse(f.read()), p
    except OSError:
        return [], None


if __name__ == "__main__":
    rooms, where = load(sys.argv[1] if len(sys.argv) > 1 else None)
    print(f"{len(rooms)} room(s) from {where}")
    for x in rooms:
        print(x["shipped"], x["map"], len(x["gems"]), "rupees", x["seconds"], "s, chest", x["chest_hash"], ":", x["name"])
