import asyncio
import hashlib
import json
import os
import subprocess
import sys
import time
from typing import Any, Dict, List, Optional, Sequence, Set, Tuple

from CommonClient import ClientCommandProcessor, CommonContext, get_base_parser, gui_enabled, handle_url_arg, logger, server_loop
from NetUtils import ClientStatus

from .tables import GAME, LOCATION_NAME_TO_ID, bundle

ITEM_FLAGS = ("HavePlayedGetEvent.", "IsGet_SmallKey_GerudoFortress.")

ITEMS_HANDLING = 0b101
PENDING = "OOTRR.AP.Pending"
POLL = 1.0
RETRY = 8.0
TRIES = 5
ASIDE = 60.0


def record_key(*parts: Any) -> str:
    return hashlib.sha256("\0".join(str(p) for p in parts).encode("utf-8")).hexdigest()[:12]


def recv(key: str) -> str:
    return f"OOTRR.AP.Recv.{key}"


def got(key: str) -> str:
    return f"OOTRR.AP.Got.{key}"


def seed_flag(key: str) -> str:
    return f"OOTRR.Seed.{key}"


def goal_flag(key: str) -> str:
    return f"OOTRR.Goal.{key}"


def received_keys(items: Sequence[Any]) -> List[Tuple[str, str]]:
    out: List[Tuple[str, str]] = []
    start = 0
    for n in items:
        if n.location >= 0:
            out.append((record_key(n.player, n.location), f"item {n.item} from player {n.player}"))
        elif n.location == -2:
            out.append((record_key("start", start), f"starting item {n.item}"))
            start += 1
    return out


class Deliverer:
    def __init__(self) -> None:
        self.sent: Dict[str, Tuple[float, int]] = {}
        self.aside: Dict[str, float] = {}

    def next(self, keys: Sequence[str], state: Dict[str, Optional[bool]], now: float) -> Optional[str]:
        if state.get(PENDING):
            return None
        if any(state.get(recv(k)) for k in keys):
            return None
        for k in keys:
            if state.get(got(k)) is not False:
                continue
            if self.aside.get(k, 0.0) > now:
                continue
            last = self.sent.get(k)
            if last is not None and now - last[0] < RETRY:
                return None
            tries = 0 if last is None else last[1]
            if tries >= TRIES:
                self.aside[k] = now + ASIDE
                self.sent.pop(k, None)
                continue
            self.sent[k] = (now, tries + 1)
            return k
        return None


class Bridge:
    def __init__(self, exe: str):
        self.exe = exe
        self.p: Optional[subprocess.Popen] = None
        self.n = 0

    def _start(self) -> None:
        flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
        cmd = [sys.executable, self.exe, "ap-bridge"] if self.exe.endswith(".py") else [self.exe, "ap-bridge"]
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, encoding="utf-8", bufsize=1, creationflags=flags)

    def call(self, req: Dict[str, Any]) -> Dict[str, Any]:
        if self.p is None or self.p.poll() is not None:
            self._start()
        assert self.p is not None and self.p.stdin is not None and self.p.stdout is not None
        self.n += 1
        req = dict(req, id=self.n)
        self.p.stdin.write(json.dumps(req) + "\n")
        self.p.stdin.flush()
        line = self.p.stdout.readline()
        if not line:
            self.p = None
            return {"ok": False, "error": "the OOTRR bridge stopped"}
        return json.loads(line)

    def close(self) -> None:
        if self.p is not None:
            try:
                self.p.stdin.close()
                self.p.wait(timeout=2)
            except Exception:
                self.p.kill()
            self.p = None


class OOTRRCommands(ClientCommandProcessor):
    def _cmd_ootrr(self) -> bool:
        """Show the state of the link to the game."""
        ctx: OOTRRContext = self.ctx
        logger.info(ctx.status)
        return True


class OOTRRContext(CommonContext):
    game = GAME
    items_handling = ITEMS_HANDLING
    command_processor = OOTRRCommands
    tags = set()

    def __init__(self, server_address: Optional[str], password: Optional[str], exe: str):
        super().__init__(server_address, password)
        self.bridge = Bridge(exe)
        self.seed_key: Optional[str] = None
        self.detect: Dict[str, Tuple[List[str], List[str]]] = {}
        self.deliver = Deliverer()
        self.status = "Not connected to a room yet."
        self.unknown: Set[str] = set()

    async def server_auth(self, password_requested: bool = False) -> None:
        if password_requested and not self.password:
            await super().server_auth(password_requested)
        await self.get_username()
        await self.send_connect(game=self.game)

    def on_package(self, cmd: str, args: Dict[str, Any]) -> None:
        if cmd == "Connected":
            sd = args.get("slot_data") or {}
            self.seed_key = sd.get("seed_key")
            if "detect" in sd:
                raw = {n: (list(v[0]), list(v[1])) for n, v in sd["detect"].items()}
            else:
                raw = bundle(sd.get("preset", "default")).detect
            uses: Dict[str, int] = {}
            for f, k in raw.values():
                for x in set(f) | set(k):
                    uses[x] = uses.get(x, 0) + 1
            self.detect = {}
            for n, (f, k) in raw.items():
                f = [x for x in f if not x.startswith(ITEM_FLAGS) and uses[x] == 1]
                k = [x for x in k if uses[x] == 1]
                if f or k:
                    self.detect[n] = (f, k)
            self.status = "Connected. Waiting for the game."

    async def disconnect(self, *args: Any, **kwargs: Any) -> None:
        self.seed_key = None
        await super().disconnect(*args, **kwargs)

    def make_gui(self):
        from kvui import GameManager

        class OOTRRManager(GameManager):
            base_title = "OOTRR Client"

        return OOTRRManager

    def mine(self) -> List[str]:
        ids = set(self.missing_locations) | set(self.checked_locations)
        names = {v: k for k, v in LOCATION_NAME_TO_ID.items()}
        return [names[i] for i in ids if i in names]

    def step(self) -> List[Dict[str, Any]]:
        msgs: List[Dict[str, Any]] = []
        if self.seed_key is None or self.slot is None:
            return msgs
        locs = self.mine()
        keys = [k for k, _ in received_keys(self.items_received)]
        bools: List[str] = [seed_flag(self.seed_key), goal_flag(self.seed_key), PENDING]
        objs: List[str] = []
        for n in locs:
            f, k = self.detect.get(n, ([], []))
            bools.extend(f)
            objs.extend(k)
        for k in keys:
            bools.extend((recv(k), got(k)))
        r = self.bridge.call({"op": "read", "bools": bools, "keys": objs})
        if not r.get("ok"):
            self.status = f"Waiting for the game: {r.get('error')}"
            return msgs
        state: Dict[str, Optional[bool]] = r["bools"]
        objects: Dict[str, Optional[bool]] = r["keys"]
        if not state.get(seed_flag(self.seed_key)):
            self.status = "The running game is not this slot's seed, or its save has not started yet. Load the save made with this slot's mod."
            return msgs
        self.status = "Linked to the game."
        found: List[int] = []
        for n in locs:
            lid = LOCATION_NAME_TO_ID[n]
            if lid in self.checked_locations or lid in self.locations_checked:
                continue
            f, k = self.detect.get(n, ([], []))
            if any(state.get(x) for x in f) or any(objects.get(x) for x in k):
                found.append(lid)
        if found:
            self.locations_checked.update(found)
            msgs.append({"cmd": "LocationChecks", "locations": found})
        if state.get(goal_flag(self.seed_key)) and not self.finished_game:
            self.finished_game = True
            msgs.append({"cmd": "StatusUpdate", "status": ClientStatus.CLIENT_GOAL})
        known = [k for k in keys if state.get(got(k)) is not None]
        for k, what in received_keys(self.items_received):
            if state.get(got(k)) is None and k not in self.unknown:
                self.unknown.add(k)
                logger.warning(f"OOTRR cannot hand over {what}: this slot's mod has no record for it.")
        k = self.deliver.next(known, state, time.monotonic())
        if k is not None:
            a = self.bridge.call({"op": "write", "name": recv(k), "value": True})
            b = self.bridge.call({"op": "write", "name": PENDING, "value": True}) if a.get("ok") else a
            if not b.get("ok"):
                self.status = f"The game refused an item: {b.get('error')}"
        return msgs


async def game_loop(ctx: OOTRRContext) -> None:
    while not ctx.exit_event.is_set():
        try:
            msgs = await asyncio.to_thread(ctx.step)
            if msgs:
                await ctx.send_msgs(msgs)
        except Exception as e:
            logger.exception(e)
            ctx.status = f"Error: {e}"
        await asyncio.sleep(POLL)


def find_exe(given: Optional[str]) -> str:
    if given:
        return given
    try:
        from settings import get_settings
        return str(get_settings().ootrr_options.ootrr_exe)
    except Exception:
        return "ootrr.exe"


async def main(args) -> None:
    ctx = OOTRRContext(args.connect, args.password, find_exe(args.ootrr))
    ctx.auth = args.name
    ctx.server_task = asyncio.create_task(server_loop(ctx), name="server loop")
    if gui_enabled:
        ctx.run_gui()
    ctx.run_cli()
    ctx.game_task = asyncio.create_task(game_loop(ctx), name="game loop")
    await ctx.exit_event.wait()
    ctx.bridge.close()
    await ctx.shutdown()


def launch(*args: str) -> None:
    import colorama
    parser = get_base_parser(description="OOTRR Archipelago client")
    parser.add_argument("--name", default=None, help="Slot name to connect as.")
    parser.add_argument("--ootrr", default=None, help="Path to ootrr.exe.")
    parser.add_argument("url", nargs="?", help="Archipelago connection url")
    a = handle_url_arg(parser.parse_args(args))
    colorama.just_fix_windows_console()
    asyncio.run(main(a))
    colorama.deinit()
