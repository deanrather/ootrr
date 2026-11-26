import typing
import hashlib
import json
import os
import subprocess
import sys
from typing import Any, Dict, List, Mapping, Optional, Set

from BaseClasses import CollectionState, Item, ItemClassification, Location, Region
from Fill import fill_restrictive
from worlds.AutoWorld import LogicMixin, WebWorld, World

import settings

from . import components as _components
from .logic import Bundle, Sweep
from .options import OOTRROptions
from .tables import FILLER, GAME, ITEM_NAME_TO_ID, LOCATION_NAME_TO_ID, bundle, presets

PATCH_FORMAT = "ootrr-ap-patch"
PATCH_VERSION = 1
PATCH_EXT = ".apootrr"
ITEMS_HANDLING = 0b101


def record_key(*parts: Any) -> str:
    return hashlib.sha256("\0".join(str(p) for p in parts).encode("utf-8")).hexdigest()[:12]


def foreign_label(item: str, player: str) -> str:
    return "AP: " + record_key("foreign", player, item)


class OOTRRSettings(settings.Group):
    class Exe(settings.UserFilePath):
        """The OOTRR app (ootrr.exe). The client runs it to read the game and hand over items."""
        is_exe = True
        description = "OOTRR app (ootrr.exe)"

    ootrr_exe: Exe = Exe("ootrr.exe")


def export_custom(exe: str, text: str, seed: str) -> Bundle:
    import Utils
    key: str = hashlib.sha256(f"{text}\0{seed}".encode("utf-8")).hexdigest()[:20]
    out: str = Utils.cache_path("ootrr", f"{key}.json")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if not os.path.isfile(out):
        flags: int = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
        try:
            r = subprocess.run([exe, "ap-export", "--settings", text, "--seed", seed, "--out", out], capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=600, creationflags=flags)
        except OSError as e:
            raise ValueError(f"OOTRR: could not run the OOTRR app at {exe}: {e}") from e
        if r.returncode != 0 or not os.path.isfile(out):
            lines = (r.stderr or r.stdout or "").strip().splitlines()
            said: str = lines[-1] if lines else f"exit code {r.returncode}"
            raise ValueError(f"OOTRR: the OOTRR app refused these settings: {said}")
    with open(out, encoding="utf-8") as f:
        b = Bundle(json.load(f))
    unknown = [n for n in b.free_names if n not in LOCATION_NAME_TO_ID] + [n for n in set(b.pool) if n not in ITEM_NAME_TO_ID]
    if unknown:
        raise ValueError(f"OOTRR: the OOTRR app and this apworld are different versions ({len(unknown)} unknown names, for example {unknown[0]!r}). Use the apworld that comes with your OOTRR app")
    return b


class OOTRRItem(Item):
    game = GAME


class OOTRRLocation(Location):
    game = GAME


class OOTRRState(LogicMixin):
    _ootrr_sweeps: Dict[int, Sweep]

    def init_mixin(self, multiworld) -> None:
        self._ootrr_sweeps = {}

    def copy_mixin(self, new: CollectionState) -> CollectionState:
        new._ootrr_sweeps = dict(self._ootrr_sweeps)
        return new


class OOTRRWeb(WebWorld):
    theme = "grass"


class OOTRRWorld(World):
    """
    OOTRR is a randomizer for the Ocarina of Time remake. In Archipelago, the items of every
    shuffled chest, NPC reward and pickup can come from other games, and yours go to them.
    """

    game = GAME
    web = OOTRRWeb()
    options_dataclass = OOTRROptions
    options: OOTRROptions
    topology_present = False
    item_name_to_id = ITEM_NAME_TO_ID
    location_name_to_id = LOCATION_NAME_TO_ID
    origin_region_name = "Menu"
    settings_key = "ootrr_options"
    settings: typing.ClassVar[OOTRRSettings]

    b: Bundle
    dead: List[str]
    pre_items: List[Item]
    own_only: Set[str]

    def generate_early(self) -> None:
        text: str = str(self.options.settings_string.value or "").strip()
        if text:
            try:
                exe: str = str(self.settings.ootrr_exe)
            except FileNotFoundError as e:
                raise ValueError(f"OOTRR: {self.player_name} uses an OOTRR settings string, which needs the OOTRR app on this computer. Set ootrr_options.ootrr_exe in host.yaml") from e
            pid: str = "custom settings"
            self.b = export_custom(exe, text, f"ap{self.multiworld.seed}p{self.player}")
        else:
            pid = presets()[self.options.preset.value]["id"]
            self.b = bundle(pid)
            pid += " preset"
        held: Dict[str, int] = {}
        for n in self.b.pool:
            held[n] = held.get(n, 0) + 1
        full: Sweep = self.b.sweep(held)
        if not full.won:
            raise ValueError(f"OOTRR: {self.player_name}'s {pid} cannot be beaten with its own items")
        self.dead = [n for n in self.b.free_names if n not in full.reach]
        local = {n for n, m in self.b.meta.items() if m.get("local")}
        self.options.local_items.value |= local
        self.pre_items = []

    def sweep(self, state: CollectionState) -> Sweep:
        s: Optional[Sweep] = state._ootrr_sweeps.get(self.player)
        if s is None:
            held: Dict[str, int] = {n: k for n, k in state.prog_items[self.player].items() if n in self.b.adv_names}
            s = self.b.sweep(held)
            state._ootrr_sweeps[self.player] = s
        return s

    def collect(self, state: CollectionState, item: Item) -> bool:
        changed: bool = super().collect(state, item)
        if changed:
            state._ootrr_sweeps.pop(self.player, None)
        return changed

    def remove(self, state: CollectionState, item: Item) -> bool:
        changed: bool = super().remove(state, item)
        if changed:
            state._ootrr_sweeps.pop(self.player, None)
        return changed

    def classify(self, name: str) -> ItemClassification:
        if name in self.b.adv_names:
            return ItemClassification.progression
        m: Optional[Dict[str, Any]] = self.b.meta.get(name)
        if m is None or m.get("junk"):
            return ItemClassification.filler
        return ItemClassification.useful

    def create_item(self, name: str) -> Item:
        return OOTRRItem(name, self.classify(name), self.item_name_to_id[name], self.player)

    def create_regions(self) -> None:
        menu: Region = Region("Menu", self.player, self.multiworld)
        self.multiworld.regions.append(menu)
        dead = set(self.dead)
        player: int = self.player
        self.own_only = set()
        for name in self.b.free_names:
            if name in dead:
                continue
            loc: OOTRRLocation = OOTRRLocation(player, name, self.location_name_to_id[name], menu)
            loc.access_rule = lambda state, n=name: n in self.sweep(state).reach
            deny: frozenset = self.b.deny[name]
            foreign: bool = bool(self.b.row[name]["foreign"]) and name not in self.b.blind
            if not foreign:
                self.own_only.add(name)
            loc.item_rule = lambda item, deny=deny, foreign=foreign: (item.name not in deny) if item.player == player else foreign
            menu.locations.append(loc)
        self.multiworld.completion_condition[player] = lambda state: self.sweep(state).won

    def create_items(self) -> None:
        pool: List[str] = list(self.b.pool)
        for n in self.dead:
            pool.remove(self.b.pool[self.b.free_names.index(n)])
        forced = dict(self.b.forced)
        for loc, item in forced.items():
            if loc in self.dead:
                continue
            self.get_location(loc).place_locked_item(self.create_item(item))
            pool.remove(item)
        self.plando(pool, forced)
        local_dungeon = {n for n, m in self.b.meta.items() if m.get("local") and m.get("dungeon")}
        for n in pool:
            it: Item = self.create_item(n)
            if n in local_dungeon:
                self.pre_items.append(it)
            else:
                self.multiworld.itempool.append(it)

    def plando(self, pool: List[str], forced: Dict[str, str]) -> None:
        who: str = self.player_name
        for loc, item in dict(self.options.ootrr_plando.value or {}).items():
            loc, item = str(loc).strip(), str(item).strip()
            if loc not in self.b.free_names or loc in self.dead or loc in forced:
                raise ValueError(f"OOTRR: {who}'s plandomizer names {loc!r}, which these settings do not shuffle")
            if item not in pool:
                raise ValueError(f"OOTRR: {who}'s plandomizer puts {item!r} at {loc!r}, but {item!r} is not in {who}'s item pool, or every copy of it is already placed")
            spot: Location = self.get_location(loc)
            it: Item = self.create_item(item)
            if not spot.item_rule(it):
                raise ValueError(f"OOTRR: {who}'s plandomizer puts {item!r} at {loc!r}, which OOTRR does not allow there")
            spot.place_locked_item(it)
            pool.remove(item)

    def get_pre_fill_items(self) -> List[Item]:
        return list(self.pre_items)

    def pre_fill(self) -> None:
        if self.pre_items:
            self.fill_dungeons()
        self.fill_own_only()

    def own_state(self, skip: Set[int]) -> CollectionState:
        state: CollectionState = CollectionState(self.multiworld)
        for it in self.multiworld.itempool:
            if it.player == self.player and id(it) not in skip:
                state.collect(it, True)
        for loc in self.multiworld.get_filled_locations():
            it = loc.item
            if it is not None and it.player == self.player and id(it) not in skip:
                state.collect(it, True)
        return state

    def fill_own_only(self) -> None:
        spots: List[Location] = [l for l in self.multiworld.get_unfilled_locations(self.player) if l.name in self.own_only]
        if not spots:
            return
        self.random.shuffle(spots)
        mine: List[Item] = [it for it in self.multiworld.itempool if it.player == self.player and not it.advancement]
        kinds: Dict[str, Item] = {}
        for it in mine:
            kinds.setdefault(it.name, it)
        picky: List[Location] = [l for l in spots if not any(l.item_rule(x) for x in kinds.values())]
        if picky:
            fit: List[Item] = [it for it in self.multiworld.itempool if it.player == self.player and it.advancement and any(l.item_rule(it) for l in picky)]
            self.random.shuffle(fit)
            skip: Set[int] = {id(x) for x in fit}
            self.multiworld.itempool = [it for it in self.multiworld.itempool if id(it) not in skip]
            fill_restrictive(self.multiworld, self.own_state(skip), picky, fit, single_player_placement=True, lock=True, allow_partial=True, allow_excluded=True, name="OOTRR reward spots")
            self.multiworld.itempool.extend(fit)
            left: List[str] = [l.name for l in picky if l.item is None]
            if left:
                raise ValueError(f"OOTRR: {self.player_name}'s {', '.join(left)} take only some of {self.player_name}'s own items, and none of them fit there")
            spots = [l for l in spots if l.item is None]
        need: int = len(spots) - len(mine)
        if need > 0:
            prog: List[Item] = [it for it in self.multiworld.itempool if it.player == self.player and it.advancement]
            self.random.shuffle(prog)
            while need > 0 and prog:
                take: List[Item] = prog[:need]
                prog = prog[need:]
                skip: Set[int] = {id(x) for x in take}
                self.multiworld.itempool = [it for it in self.multiworld.itempool if id(it) not in skip]
                free: List[Location] = [l for l in spots if l.item is None]
                fill_restrictive(self.multiworld, self.own_state(skip), free, take, single_player_placement=True, lock=True, allow_partial=True, allow_excluded=True, name="OOTRR own items")
                need -= len(skip) - len(take)
                self.multiworld.itempool.extend(take)
        self.random.shuffle(mine)
        mine.sort(key=lambda it: it.useful)
        placed: Set[int] = set()
        for loc in spots:
            if loc.item is not None:
                continue
            k: Optional[int] = next((k for k, x in enumerate(mine) if loc.item_rule(x)), None)
            if k is None:
                continue
            it: Item = mine.pop(k)
            placed.add(id(it))
            loc.place_locked_item(it)
        self.multiworld.itempool = [it for it in self.multiworld.itempool if id(it) not in placed]

    def fill_dungeons(self) -> None:
        held = set(id(i) for i in self.pre_items)
        state: CollectionState = self.own_state(held)
        items: List[Item] = sorted(self.pre_items, key=lambda i: (not i.advancement, i.name))
        locs: List[Location] = [l for l in self.multiworld.get_unfilled_locations(self.player)]
        self.random.shuffle(locs)
        fill_restrictive(self.multiworld, state, locs, items, single_player_placement=True, lock=True, allow_excluded=True, name="OOTRR dungeon items")
        if items:
            raise ValueError(f"OOTRR: {len(items)} dungeon items found no place in their dungeon")
        self.pre_items = []

    def get_filler_item_name(self) -> str:
        return FILLER

    def patch(self) -> Dict[str, Any]:
        mw = self.multiworld
        placements: Dict[str, str] = {}
        foreign: Dict[str, Dict[str, Any]] = {}
        for loc in mw.get_locations(self.player):
            it: Optional[Item] = loc.item
            if it is None:
                continue
            if it.player == self.player:
                placements[loc.name] = it.name
            else:
                who: str = mw.player_name[it.player]
                placements[loc.name] = foreign_label(it.name, who)
                foreign[loc.name] = {"item": it.name, "player": who, "game": it.game, "progression": bool(it.advancement)}
        for n in self.dead:
            placements[n] = self.b.pool[self.b.free_names.index(n)]
        remote: List[Dict[str, Any]] = []
        for loc in mw.get_locations():
            it = loc.item
            if it is None or it.player != self.player or loc.player == self.player or loc.address is None:
                continue
            remote.append({"key": record_key(loc.player, loc.address), "item": it.name, "player": mw.player_name[loc.player], "location": loc.name})
        remote.sort(key=lambda r: r["key"])
        start: List[Dict[str, Any]] = [{"key": record_key("start", k), "item": it.name} for k, it in enumerate(mw.precollected_items[self.player])]
        ids: Dict[str, int] = {l.name: l.address for l in mw.get_locations(self.player) if l.address is not None}
        return {
            "format": PATCH_FORMAT,
            "version": PATCH_VERSION,
            "game": GAME,
            "preset": self.b.preset,
            "settings": self.b.settings,
            "seed": self.b.seed,
            "seed_name": mw.seed_name,
            "player": self.player,
            "slot": mw.player_name[self.player],
            "seed_key": self.seed_key(),
            "placements": dict(sorted(placements.items())),
            "foreign": dict(sorted(foreign.items())),
            "remote": remote,
            "start": start,
            "location_ids": dict(sorted(ids.items())),
        }

    def seed_key(self) -> str:
        return record_key("archipelago", self.multiworld.seed_name, self.player)

    def generate_output(self, output_directory: str) -> None:
        path: str = os.path.join(output_directory, self.multiworld.get_out_file_name_base(self.player) + PATCH_EXT)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(self.patch(), f, ensure_ascii=False, indent=1)

    def fill_slot_data(self) -> Mapping[str, Any]:
        detect: Dict[str, List[List[str]]] = {}
        for loc in self.multiworld.get_locations(self.player):
            f, k = self.b.detect.get(loc.name, ([], []))
            if loc.address is not None and (f or k):
                detect[loc.name] = [list(f), list(k)]
        return {"patch_version": PATCH_VERSION, "preset": self.b.preset, "seed_key": self.seed_key(), "detect": detect}
