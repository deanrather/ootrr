"""Builds devtools' local caches from your own extracted game and the OOTRR checkout. Nothing here is committed
(devtools/.gitignore): every file is made from the game or from data/ in the checkout and stays on this PC.

    python devtools/build_cache.py [--romfs <extracted game romfs>] [--ootrr <OOTRR checkout>] [--ootrr-exe <ootrr binary>]
                                   [--only index|names|vanilla|boxes|places|flags]

Needs the zstandard module (pip install zstandard) to read the game's map files. The extracted game is read only.
Writes to devtools/cache (or $DEVTOOLS_CACHE):
  actor-index.json         every placed actor: hash -> scene, map, room, position, type
  actors.js + extras/      the Actor Viewer's names and objmap links (cache/extras is the Mod UI's cards folder)
  actor-labels*.tsv        name files for the game's `labels` command
  vanilla_enemies.json     the game's enemy placements per map, for the Enemy Placer
  actor_boxes.json         model boxes of the Enemy Placer's kinds, from the game's ActorInfo table
  places-all.json          one Go to place per randomizer check
  flag-names.json          saved flag names (needs the ootrr binary: cargo build --release -p ootrr)
"""
import argparse
import os
import shutil
import subprocess
import sys

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "ootrr"))
import paths as PATHS  # noqa: E402

STEPS: tuple[str, ...] = ("index", "names", "vanilla", "boxes", "places", "flags")


def find_exe(given: str | None, checkout: str) -> str | None:
    names: list[str] = ["ootrr.exe", "ootrr"]
    c: list[str] = [given or os.environ.get("OOTRR_EXE", "")]
    for d in ("release", "debug"):
        c += [os.path.join(checkout, "target", d, n) for n in names]
    c += [w for w in (shutil.which("ootrr"),) if w]
    return next((p for p in c if p and os.path.isfile(p)), None)


def copy_cards() -> int:
    os.makedirs(PATHS.EXTRAS, exist_ok=True)
    src: str = os.path.join(HERE, "cards")
    n: int = 0
    for f in sorted(os.listdir(src)):
        shutil.copyfile(os.path.join(src, f), os.path.join(PATHS.EXTRAS, f))
        n += 1
    return n


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--romfs")
    ap.add_argument("--ootrr", help="the OOTRR checkout (default: the repository devtools lives in)")
    ap.add_argument("--ootrr-exe")
    ap.add_argument("--only", action="append", choices=STEPS)
    a = ap.parse_args(argv)
    romfs: str = PATHS.need_romfs(a.romfs)
    checkout: str = PATHS.ootrr_checkout(a.ootrr)
    todo: tuple[str, ...] = tuple(a.only or STEPS)
    os.makedirs(PATHS.CACHE, exist_ok=True)
    print("cards: %d copied to %s" % (copy_cards(), PATHS.EXTRAS))
    import actor_index as AI
    ix: dict = {}
    if "index" in todo or "names" in todo or "places" in todo:
        ix = AI.load(romfs, rebuild="index" in todo)
        print("index: %d placed actors" % len(ix))
    if "names" in todo:
        import actor_names as AN
        r: dict = AN.write(checkout, romfs=romfs, ids=ix)
        print("names: %d ids (%s), %d linked to objmap" % (len(r["ids"]), "from " + checkout if r["ok"] else "NO data/locations.json or tracker_targets.json under " + checkout,
                                                            len(r["links"]["at"])))
    if "vanilla" in todo:
        import make_vanilla as MV
        print("vanilla: %d maps with enemies" % MV.write(romfs))
    if "boxes" in todo:
        import make_actor_boxes as MB
        if MB.main(["--romfs", romfs]) != 0:
            return 1
    if "places" in todo:
        import make_places as MP
        print("places: %d places in cache/places-all.json" % MP.write(checkout, ix))
    if "flags" in todo:
        exe: str | None = find_exe(a.ootrr_exe, checkout)
        out: str = PATHS.cache("flag-names.json")
        if not exe:
            print("flags: skipped, no ootrr binary found (cargo build --release -p ootrr, or --ootrr-exe PATH); the saved-flags card then shows ids only")
        else:
            r2 = subprocess.run([exe, "flags-build", romfs, out], capture_output=True, text=True)
            print("flags: " + ("written to " + out if r2.returncode == 0 else "failed: " + (r2.stdout + r2.stderr).strip()[-300:]))
    print("gs-list: not built (the Gold Skulltula card needs cache/gs-list.json from the gs-relocate work; without it the card says so)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
