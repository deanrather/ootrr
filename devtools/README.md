# OOTRR devtools

Browser tools for people who mod the Ocarina of Time remake with OOTRR. They sit beside a running game (the Longshot
emulator's tools build) and read it through Longshot's control bridge. The first use is **location naming**: stand next to
a pot, a patch of grass, a tree or a chest, and the tools show the placement id, the objmap link and the name OOTRR
knows for it, and can draw the same text above the actors in the game itself.

Everything lives in this folder. Nothing here runs inside the randomizer, nothing ships to players, and the randomizer
does not depend on it. The mod OOTRR writes stays a plain data edit that works wherever OOTRR works; devtools only
help you look at the game while you build one.

| Part | What it is |
|---|---|
| `panel/` | The Mod UI: a page with cards (steps, Give, Go To, Notes, saved flags, drag-to-reorder, installable on a phone). A copy of the Longshot panel server, standard library only |
| `cards/`, `ootrr/` | The OOTRR cards (Actor Viewer, Enemy Placer, Gate, Skulltula and Puzzle managers) and the server behind the last four |
| `build_cache.py` | Builds the local data (actor index, name files, enemy table, boxes, places) from **your** extracted game |
| `start.py` | The one entry point |
| `tests/` | Unit tests; they need no game and no Longshot |

## What you need

1. **Python 3.10 or later.** Everything here uses only the standard library, except one step: `build_cache.py` needs
   `pip install zstandard` to read the game's map files.
2. **Longshot, our fork, tools branch.** The tools use bridge commands that upstream Longshot does not have yet (the
   list is at the end). Clone `https://git.magicbean.dev/BeanSeller/Longshot.git`, check out the branch `feat/playtest-panel` (the open Longshot MR that adds the extra control-bridge capabilities), and build it from your
   own game dump by following that checkout's `docs/dev/reference/build.md`. devtools need two things from it: its
   `scripts` folder (`longshot_bridge.py`, `longshot_ctl.py`) and the game folder you launch (the one holding
   `longshot.exe`). Longshot is a separate project; devtools never copy anything from it except through that folder.
3. **Your extracted game** (the `romfs` folder with `Banc`, `RSDB`, `Pack` inside). It is only read.
4. **This repository** (the OOTRR fork), for `data/locations.json` and `data/tracker_targets.json`, which give the names.
   devtools read them from the checkout they live in. Optional: the `ootrr` binary (`cargo build --release -p ootrr`)
   for the saved-flags names.
5. Optional: `node` only for the install-as-an-app test.

## Set up (once)

```
pip install zstandard
python devtools/build_cache.py --romfs <extracted game romfs>
```

This writes `devtools/cache/` (git-ignored; about 15 MB; it is made from your game and stays on your PC): the placed-actor
index, `actors.js` and the label files (names and objmap links), the enemy table and boxes for the placer, `places-all.json`
(one Go To place per randomizer check) and, if it finds the `ootrr` binary, `flag-names.json`. Run it again after the game
or the OOTRR data changes. `--only names` (or `index`, `vanilla`, `boxes`, `places`, `flags`) redoes one part.

Settings can be given as arguments, as environment variables or in `devtools/config.json` (copy `config.example.json`;
the file is git-ignored). Nothing has a built-in path.

| Setting | Argument | Environment | Default |
|---|---|---|---|
| Extracted game | `--romfs` | `OOTRR_ROMFS` | none (needed by `build_cache.py`) |
| Longshot scripts folder | `--longshot-scripts` | `LONGSHOT_SCRIPTS` | none (needed by `start.py`) |
| Longshot game folder | `--game` | `LONGSHOT_GAME` | none (only for `--launch` and the printed hint) |
| OOTRR checkout | `--ootrr` | `OOTRR_CHECKOUT` | the repository this folder is in |
| Session folder (notes, shots, runs) | `--session` | `PANEL_SESSION` | `devtools/session` |

## Launch the game with the bridge on

devtools attach to a game that is already running with Longshot's control bridge. Start it in the session's `runs` folder:

```
python <Longshot checkout>/scripts/longshot_ctl.py launch <Longshot game folder> --root <session>/runs
```

(`start.py` prints this line with your folders filled in, and `start.py --launch` runs it for you.) The panel finds the
newest run in that folder by itself, so a game you start again is found without restarting devtools. To use a game you
started another way, pass its bridge socket with `--sock` or its run folder with `--run`. Load a save before you use
the cards; the Mod UI's Go To card and Give card help.

## Start devtools

```
python devtools/start.py --game <Longshot game folder> --romfs <extracted game romfs> --longshot-scripts <Longshot checkout>/scripts
```

then open <http://127.0.0.1:8765/>. It starts the Mod UI on port 8765 and the managers' server on port 8766 (the four
manager cards are pages from it, shown inside the Mod UI). It launches no game unless you add `--launch`. Useful options:

- `--lan` also listens on your network, with an access key in the address it prints, so a phone or tablet beside the
  monitor can run the cards (see `docs/PWA.md` for installing it as an app).
- `--session DIR`, `--port`, `--placer-port`, `--no-placer`, `--no-tbox`, `--run`, `--sock`; every other option goes to
  the Mod UI server (`python devtools/panel/server.py --help`).
- `--demo` runs the manager cards against a fake game so you can look at the pages with no Longshot at all.

On Windows a small note box (`docs/TBOX.md`) also starts: press T with the game in front and type a note that is
appended to the session's notes with Link's position. `--no-tbox` leaves it out.

## Location naming: the workflow

1. Build the cache, start the game with the bridge, start devtools, open the Mod UI. Load a save and use **Go To** to land
   near the area you are naming (the Go To list has one place per randomizer check, plus a few measured spots).
2. Open the **Actor Viewer** card. Tick the kinds you care about (chests, pots, grass, trees, rocks, rupees...; **Defaults**
   is a sensible set). Set `within` (metres from Link) and `max labels`.
3. Press **Show overlay**. The game draws the placement id and OOTRR's name above each ticked actor near Link. Walk up
   to the one you are naming. An actor with no id (drops, spawned enemies, effects) cannot be labelled, only listed.
4. Press **Read now** (or tick `every 3 s`). The table lists the nearest actors, each with:
   the placement hash in hex (what the game and objmap show) and in decimal (what OOTRR's files use), the name from
   `data/locations.json` and `data/tracker_targets.json`, the distance, a **check** column that says when the two OOTRR
   tables differ or an id is repeated, a **link / copy** column and the position and state.
5. In **link / copy**: `objmap` opens the same actor on the object map in its own scene, map and floor; `copy id` copies
   the decimal id; `copy line` copies one markdown line, `[id](objmap link) >> name`, ready to paste into your notes.
   **Copy rows** copies the whole table as tab-separated text.
6. Name what is missing in your notes with those lines, then add the names to OOTRR's data the way its own docs say
   (`data/locations.json` is the randomizer's, so open a pull request there). Run `build_cache.py --only names` and the
   overlay shows the new names.

Colours in the overlay come from the kind of check (chest, pot, grass...); the checkbox `coloured labels` needs a Longshot
build that reads a third column. The cache's `actor-labels.tsv` and `actor-labels-colour.tsv` are the files the game's
`labels names` command reads; the card passes them for you.

## What each card does

| Card | Use |
|---|---|
| Actor Viewer | Location naming, above. Live actor list and in-game labels, with objmap links |
| Playtest Guide | Steps with Do, Expect and pass or fail, from the steps file (`--steps`; `panel/examples/steps.json` is a sample) |
| Give / Take | Buttons that hand items (`data/buttons.json`; edit it) |
| Go To | Teleport and warp to a place, then check you arrived (`data/places.json` has measured spots; the cache adds one per check) |
| Notes, Recent | Notes with Link's position and a screenshot, short clips (needs ffmpeg), the session log |
| Story Flags | Live saved-flag list and editor (names from `cache/flag-names.json`) |
| Enemy Placer | Mark enemy spawn points in the game, spawn and kill enemies, radar and display-model layers, counts against the game's own enemies per room |
| Enemy-Gate Manager | Edit the difficulty mod's enemy gates (reads `randomizer/core/src/difficulty/gates.rs` in the checkout) |
| Skulltula Manager | Custom Gold Skulltula spots. Needs `cache/gs-list.json`, which the Gold Skulltula work makes; without it the card says so |
| Timed Rupee Puzzle Manager | Timed silver-rupee rooms (reads `gems.rs` when that file exists in your checkout) |

Cards can be dragged by the grip at the right of the header; the order is remembered in the browser. A card that fails to
load does not hide the others.

## What still depends on Longshot

devtools talk to the game only through the control bridge (`longshot_bridge.py`, `longshot_ctl.py`). The tools build of
Longshot has these commands, and the cards need them: `status`, `teleport`, `warp`, `press`, `shot`, `give` (and `take`),
`edit` (`flagset`, `flagget`, `foehurt`), `flags`, `subscribe`, `caption`, `spawn`, `enemies`, `actors` (with the
placement hash in hex and decimal), `labels` (`on`, `off`, `names`, `format`, `colors`, `named_only`, `range`, `max`),
`radar` (`set`, `clear`, `options`, `fit`), `placeholders` (`set`, `on`, `off`, `clear`), `aim` (`marker`, `events`, place and
remove), `debug` and `damage`. If a command is missing from your build the card that uses it reports the bridge error; the
others keep working. The Actor Viewer list and names work with `actors` alone; the in-game overlay needs `labels`.

## Tests

```
python devtools/tests/run_all.py
```

runs every test file with a stub of Longshot's bridge module, so no Longshot and no game are needed (set `LONGSHOT_SCRIPTS`
to a real `scripts` folder to use the real one; `test_pwa.py` needs `node`). Tests that read the cache skip themselves
when you have not built it.

## What is not here

No game data of any kind: the cache is built on your PC from your own files and is git-ignored. No community spreadsheet
or other project's data: the enemy placer can sort by hit points and show place names if you point
`DEVTOOLS_MONSTERS_CSV` and `DEVTOOLS_LOCATIONS_CSV` at your own CSV files; without them it does without. The
objmap calibration in `data/objmap_floors.json` is numbers and layer names only.

## Licence

GPL-3.0-or-later, like OOTRR (see `LICENSE` at the top of the repository).
