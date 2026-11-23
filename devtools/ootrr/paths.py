"""Where devtools finds things. One place, so no module hard-codes a path.

Order for every setting: the command line (where the tool has one), the environment, config.json beside start.py (or the file
DEVTOOLS_CONFIG names), then the default. The OOTRR checkout defaults to the repository devtools lives in.
"""
import json
import os
import sys

HERE: str = os.path.dirname(os.path.abspath(__file__))
ROOT: str = os.path.dirname(HERE)
DATA: str = os.path.join(ROOT, "data")
CACHE: str = os.environ.get("DEVTOOLS_CACHE") or os.path.join(ROOT, "cache")
EXTRAS: str = os.path.join(CACHE, "extras")
KEYS: dict = {"ootrr_checkout": "OOTRR_CHECKOUT", "romfs": "OOTRR_ROMFS", "longshot_scripts": "LONGSHOT_SCRIPTS",
              "session": "PANEL_SESSION", "game": "LONGSHOT_GAME"}


def config() -> dict:
    p: str = os.environ.get("DEVTOOLS_CONFIG") or os.path.join(ROOT, "config.json")
    try:
        with open(p, encoding="utf-8-sig") as f:
            v = json.load(f)
    except (OSError, ValueError):
        return {}
    return v if isinstance(v, dict) else {}


def setting(name: str, arg: str | None = None, default: str = "") -> str:
    if arg:
        return arg
    e: str | None = os.environ.get(KEYS.get(name, ""))
    if e:
        return e
    v = config().get(name)
    return str(v) if v else default


def ootrr_checkout(arg: str | None = None) -> str:
    return setting("ootrr_checkout", arg, os.path.dirname(ROOT))


def romfs(arg: str | None = None) -> str:
    return setting("romfs", arg)


def session(arg: str | None = None) -> str:
    return setting("session", arg, os.path.join(ROOT, "session"))


def longshot_scripts(arg: str | None = None) -> str:
    return setting("longshot_scripts", arg)


def longshot_ctl() -> str:
    d: str = os.environ.get("LONGSHOT_CTL", "")
    if d:
        return d
    return os.path.join(longshot_scripts(), "longshot_ctl.py") if longshot_scripts() else ""


def cache(name: str) -> str:
    return os.path.join(CACHE, name)


def need_romfs(arg: str | None = None) -> str:
    r: str = romfs(arg)
    if not r or not os.path.isdir(os.path.join(r, "Banc")):
        sys.exit("devtools needs the extracted game: give --romfs <folder holding Banc/>, or set OOTRR_ROMFS, or romfs in config.json")
    return r
