"""The gates the mod ships, read from the GATES table of randomizer/core/src/difficulty/gates.rs (no Rust build needed).

Each gate becomes a panel gate dict: {id, map, name, left, right, spots, kind, also, shipped: True, guards: []}; left and right
are the Rust a and b, spots the guard spot plus its `more` spots. The file is the first that exists of $OOTRR_GATES_RS, the
main checkout and wt/next (the main checkout is on `difficulty`, which every merged gate reaches).

    python -I shipped_gates.py [gates.rs]   prints what it read
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import paths as PATHS  # noqa: E402

CANDIDATES: tuple[str, ...] = (os.path.join(PATHS.ootrr_checkout(), "randomizer", "core", "src", "difficulty", "gates.rs"),)
KNOWN: dict[str, str] = {"HAND": "Enemy_DeadHand", "KNIGHT": "Enemy_IronKnuckle_Silver"}
NUM: str = r"-?\d+(?:\.\d+)?"
TRIPLE: str = r"\[\s*(%s)\s*,\s*(%s)\s*,\s*(%s)\s*\]" % (NUM, NUM, NUM)


def path_of(given: str | None = None) -> str | None:
    if given:
        return given if os.path.isfile(given) else None
    for p in [os.environ.get("OOTRR_GATES_RS", "")] + list(CANDIDATES):
        if p and os.path.isfile(p):
            return p
    return None


def triples(text: str) -> list[list[float]]:
    return [[float(a), float(b), float(c)] for a, b, c in re.findall(TRIPLE, text)]


def field(row: str, name: str) -> str:
    m = re.search(r"\b%s:\s*" % name, row)
    if not m:
        return ""
    i: int = m.end()
    depth: int = 0
    j: int = i
    while j < len(row):
        ch: str = row[j]
        if ch in "[(":
            depth += 1
        elif ch in "])":
            depth -= 1
        elif ch == '"':
            j = row.index('"', j + 1)
        elif ch == "," and depth == 0:
            break
        j += 1
    return row[i:j].strip()


def consts(text: str) -> dict[str, str]:
    out: dict[str, str] = dict(KNOWN)
    for k, v in re.findall(r'pub const ([A-Z_]+): &str = "([^"]*)";', text):
        out[k] = v
    for k, v in re.findall(r"pub const ([A-Z_]+): &str = ([A-Z_]+);", text):
        if v in out:
            out[k] = out[v]
    return out


def kind_of(token: str, names: dict[str, str]) -> str:
    token = token.strip()
    if token.startswith('"'):
        return token.strip('"')
    return names.get(token, token)


def parse(text: str) -> list[dict]:
    names: dict[str, str] = consts(text)
    out: list[dict] = []
    for row in re.findall(r"^\s*Gate \{ id: .*\},?\s*$", text, re.M):
        if "id: u8" in row:
            continue
        spot: list[list[float]] = triples(field(row, "spot"))
        a: list[list[float]] = triples(field(row, "a"))
        b: list[list[float]] = triples(field(row, "b"))
        if not (a and b and spot):
            continue
        also: list[list[float]] = triples(field(row, "also"))
        out.append({"id": int(field(row, "id")), "map": field(row, "map").strip('"'), "name": field(row, "name").strip('"'),
                    "left": a[0], "right": b[0], "spots": spot + triples(field(row, "more")),
                    "also": [[also[i], also[i + 1]] for i in range(0, len(also) - 1, 2)],
                    "kind": kind_of(field(row, "new_kind"), names), "guards": [], "shipped": True})
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
    gates, where = load(sys.argv[1] if len(sys.argv) > 1 else None)
    print(f"{len(gates)} gates from {where}")
    for g in gates:
        print(g["id"], g["map"], g["kind"] or "(vanilla guards)", len(g["spots"]), "spot(s):", g["name"])
