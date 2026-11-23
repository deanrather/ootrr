"""Starts the Mod UI (the playtest panel with the OOTRR cards) and the managers' server against a Longshot game that is running
with the control bridge. It launches nothing itself unless you pass --launch.

    python devtools/start.py --game <Longshot game folder> --romfs <extracted game romfs> [--lan]

--game               the Longshot game folder (holds longshot.exe); only --launch and the printed hint use it
--romfs              your extracted game, read only; only build_cache.py needs it (start.py just checks the caches exist)
--longshot-scripts   the scripts folder of the Longshot fork checkout (holds longshot_bridge.py); or $LONGSHOT_SCRIPTS
--session DIR        notes, screenshots, runs; default devtools/session (runs go in SESSION/runs)
--ootrr DIR          the OOTRR checkout for data/ (default: the repository devtools lives in)
--run DIR | --sock PATH   attach to one run folder or one bridge socket instead of the newest run under SESSION/runs
--launch             start the game for you: longshot_ctl.py launch GAME --root SESSION/runs (the bridge is on)
--lan                also listen on the local network (a phone), with an access key
--port 8765 --placer-port 8766     the two servers' ports
--no-placer --no-tbox --tbox-key KEY --tbox-ingame-only
--demo               the placer cards answer from a fake game (look at the pages with no Longshot)
any other option goes to the Mod UI server (see: python devtools/panel/server.py --help), such as --https, --no-key, --title.

Settings can also come from the environment or devtools/config.json (config.example.json): see ootrr/paths.py.
"""
import atexit
import os
import secrets
import subprocess
import sys
import time

HERE: str = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "ootrr"))
import paths as PATHS  # noqa: E402


def pop_flag(args: list[str], name: str) -> bool:
    if name in args:
        args.remove(name)
        return True
    return False


def pop_value(args: list[str], name: str, default: str | None = None) -> str | None:
    if name in args:
        i: int = args.index(name)
        if i + 1 >= len(args):
            sys.exit("%s needs a value" % name)
        v: str = args[i + 1]
        del args[i:i + 2]
        return v
    return default


def main(argv: list[str]) -> int:
    args: list[str] = list(argv)
    if "-h" in args or "--help" in args:
        print(__doc__)
        return 0
    game: str = PATHS.setting("game", pop_value(args, "--game"))
    romfs: str = PATHS.romfs(pop_value(args, "--romfs"))
    scripts: str = PATHS.longshot_scripts(pop_value(args, "--longshot-scripts"))
    session: str = os.path.abspath(PATHS.session(pop_value(args, "--session")))
    ootrr: str = PATHS.ootrr_checkout(pop_value(args, "--ootrr"))
    run: str | None = pop_value(args, "--run")
    sock: str | None = pop_value(args, "--sock")
    port: str = pop_value(args, "--port", "8765") or "8765"
    pport: str = pop_value(args, "--placer-port", "8766") or "8766"
    steps: str | None = pop_value(args, "--steps")
    launch: bool = pop_flag(args, "--launch")
    no_placer: bool = pop_flag(args, "--no-placer")
    demo: bool = pop_flag(args, "--demo")
    no_tbox: bool = pop_flag(args, "--no-tbox")
    ingame_only: bool = pop_flag(args, "--tbox-ingame-only")
    tbox_key: str = pop_value(args, "--tbox-key", "T") or "T"
    lan: bool = "--lan" in args
    no_key: bool = "--no-key" in args
    key: str | None = pop_value(args, "--key") or (secrets.token_urlsafe(12) if lan and not no_key else None)

    if not scripts or not os.path.isfile(os.path.join(scripts, "longshot_bridge.py")):
        print("devtools needs the Longshot fork's scripts folder (it holds longshot_bridge.py and longshot_ctl.py).\n"
              "Give --longshot-scripts <Longshot checkout>/scripts, or set LONGSHOT_SCRIPTS. See devtools/README.md.", file=sys.stderr)
        return 2
    os.environ["LONGSHOT_SCRIPTS"] = scripts
    os.environ["OOTRR_CHECKOUT"] = ootrr
    if romfs:
        os.environ["OOTRR_ROMFS"] = romfs
    os.makedirs(os.path.join(session, "runs"), exist_ok=True)
    os.makedirs(os.path.join(session, "notes"), exist_ok=True)
    os.environ.setdefault("HOS_NOTES_DIR", os.path.join(session, "notes"))
    os.environ["PANEL_SESSION"] = session

    if not os.path.isdir(PATHS.EXTRAS) or not os.path.isfile(os.path.join(PATHS.EXTRAS, "actors.js")):
        import shutil
        os.makedirs(PATHS.EXTRAS, exist_ok=True)
        for f in os.listdir(os.path.join(HERE, "cards")):
            shutil.copyfile(os.path.join(HERE, "cards", f), os.path.join(PATHS.EXTRAS, f))
        print("note: no cache yet, so the Actor Viewer has no names and the placer has no vanilla counts.\n"
              "      run: python devtools/build_cache.py --romfs <your extracted game romfs>", file=sys.stderr)

    procs: list[subprocess.Popen] = []

    def stop() -> None:
        for p in procs:
            if p.poll() is None:
                p.terminate()

    atexit.register(stop)
    ctl: str = os.path.join(scripts, "longshot_ctl.py")
    if launch:
        if not game:
            print("--launch needs --game <Longshot game folder>", file=sys.stderr)
            return 2
        r = subprocess.run([sys.executable, "-I", ctl, "launch", game, "--root", os.path.join(session, "runs")], capture_output=True, text=True)
        print((r.stdout + r.stderr).strip())
        if r.returncode:
            return r.returncode
    else:
        print("start the game with the bridge on, in this runs folder (the panel finds the newest run by itself):\n"
              "  python -I %s launch %s --root %s" % (ctl, game or "<Longshot game folder>", os.path.join(session, "runs")))

    target: list[str] = ["--run", run] if run else ["--sock", sock] if sock else ["--runs", os.path.join(session, "runs")]
    cmd: list[str] = [sys.executable, "-I", os.path.join(HERE, "panel", "server.py")] + target + [
        "--session", session, "--extras", PATHS.EXTRAS, "--title", "Mod UI", "--port", port,
        "--steps", steps or os.path.join(HERE, "panel", "examples", "steps.json"),
        "--places", PATHS.cache("places-all.json") if os.path.isfile(PATHS.cache("places-all.json")) else os.path.join(PATHS.DATA, "places.json"),
        "--buttons", os.path.join(PATHS.DATA, "buttons.json")]
    if os.path.isfile(PATHS.cache("flag-names.json")):
        cmd += ["--flag-names", PATHS.cache("flag-names.json")]
    if key and lan:
        cmd += ["--key", key]

    if not no_tbox and not ingame_only and os.name == "nt":
        procs.append(subprocess.Popen([sys.executable, "-I", os.path.join(HERE, "ootrr", "tbox.py"), session, "--key", tbox_key,
                                       "--game-dir", game or ""]))
    if not no_placer:
        pc: list[str] = [sys.executable, "-I", os.path.join(HERE, "ootrr", "placer_server.py"), session, "--arrows", "--port", pport]
        if demo:
            pc.append("--fake")
        if lan:
            pc += ["--lan"] + (["--key", key] if key else [])
        procs.append(subprocess.Popen(pc))
    suffix: str = ("?key=" + key if key and lan else "") + (("&" if key and lan else "?") + "placer=" + pport if pport != "8766" else "")
    print("Mod UI: http://127.0.0.1:%s/%s" % (port, suffix), flush=True)
    panel = subprocess.Popen(cmd + args)
    procs.append(panel)
    try:
        return panel.wait()
    except KeyboardInterrupt:
        return 0
    finally:
        stop()
        time.sleep(0.2)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
