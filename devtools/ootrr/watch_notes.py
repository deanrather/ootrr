import json
import os
import sys
import time

KINDS: tuple[str, ...] = ("note", "record", "clip", "verdict")


def compact(o: dict) -> str:
    pos = o.get("pos")
    where: str = "%s %s" % (o.get("map", "?"), "(%.1f, %.1f, %.1f)" % tuple(pos) if pos and len(pos) == 3 else "(no pos)")
    parts: list[str] = [o.get("time", ""), o.get("kind", ""), repr(o.get("text", "")), where]
    if o.get("facing") is not None:
        parts.append("facing %.0f" % float(o["facing"]))
    if o.get("life") is not None:
        parts.append("life %s/%s" % (o.get("life"), o.get("max_life", "?")))
    if o.get("nearby") is not None:
        parts.append("%d nearby" % len(o["nearby"]))
    if o.get("telemetry") is not None:
        parts.append("%d telemetry samples" % len(o["telemetry"]))
    if o.get("clip"):
        parts.append("clip " + str(o["clip"]))
    if o.get("verdict"):
        parts.append("verdict " + str(o["verdict"]))
    return " | ".join(parts)


def main() -> int:
    path: str = os.path.join(sys.argv[1], "notes", "notes.jsonl")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "a", encoding="utf-8").close()
    with open(path, encoding="utf-8") as f:
        f.seek(0, 2)
        while True:
            line: str = f.readline()
            if not line:
                time.sleep(0.5)
                continue
            try:
                o: dict = json.loads(line)
            except ValueError:
                continue
            if o.get("kind") in KINDS:
                print(compact(o), flush=True)


if __name__ == "__main__":
    sys.exit(main())
