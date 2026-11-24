# Contributing to OOTRR

Thanks for helping. OOTRR is a randomizer for the Ocarina of Time Switch 2 remake. It reads the game files you extracted yourself and writes a mod folder. This page is the short version of how we work. If something here is unclear, ask in the OOTRR Discord before you start.

## Ground rules

- **No game data in the repo.** No game files, keys, dumps, extracted textures, saves or anything derived from them. The shipped tool never decrypts anything and never touches keys. Identifiers (flag names, actor names, hashes) are fine.
- **License.** The project is GPL-3.0-or-later (releases up to 0.4.0 were MIT). Code you contribute is under the GPL. Credits for ported work stay in `NOTICE`.
- **One concern per pull request.** Small, focused changes are reviewed fast. Base your branch on `nightly` and open the pull request against `nightly`. Merged work ships in the next nightly build, and `nightly` is merged into `main` for each release.
- **Maintainers decide releases.** Tags and releases are made by the owner. Do not push tags.
- **No new crate without asking first.** The only dependency is `ruzstd`.

## Build and test

```
cargo build --release
cargo test --workspace
```

`cargo test` builds with the `test` profile: our code at opt level 1 and the dependencies at opt level 3, so the slow seed and game file tests run several times faster than plain debug and the build stays quick. CI runs `cargo test --workspace --profile ci`, a release build without LTO. Avoid `cargo test --release`, it compiles with LTO and one codegen unit and takes minutes before the first test runs.

Seed pins live in `data/pins/*.json`, one file per pin test (`old_seeds`, `rewards_off_and_stones`, `beehives`). A change that moves the item pool moves them. When that is intended, run the pin tests with `OOTRR_UPDATE_PINS=1` set, check that only the pins you expected changed, and commit the json. Two branches that both move a pin conflict only in that file: take either side and run the update again after the merge.

A test that loops over many independent cases (every catalog pack, every combination) runs on one thread. Split it with the `parts!` macro from `lib.rs`: write a function that takes a part number and checks only the cases whose index matches, and let `parts!` make one test per part.

Many tests read the extracted game. Set `OOTRR_ROMFS` to your extracted game folder (the one with Pack, Banc, Event and System) so they run. Without it they skip. A few tracker tests read `OOTRR_TEST_ROMFS` and `OOTRR_TEST_SAVES`. The release build must have no warnings.

Never run `ootrr` with no arguments. It starts the local web server and may open a browser. Use a subcommand. The usage list is in `randomizer/cli/src/main.rs`.

## Layout

- `randomizer/game`: reading and writing the game's files and tables (byml, sarc, zs, msbt, bfevfl, bfres, textures, the RomFS reader, size tables, flags, the location catalog, saves). Nothing in it knows about seeds or settings.
- `randomizer/logic`: the logic engine (rules, world, search, fill, spoiler). It depends on `game` only for errors and the random generator.
- `randomizer/cosmetics`: music, sound effect and colour randomizers.
- `randomizer/core`: the generator and every feature that turns a seed into edits. It re-exports the three crates above, so `crate::byml` and `ootrr_core::byml` keep working.
- `randomizer/cli`: the `ootrr` command line tool
- `web`: the local web page and the tracker. Everything that is the page lives here, the randomizer lives in `randomizer/`
- `data/`: location catalogs, logic data (`data/logic`: one file per dungeon in `World`, and one file per overworld area in `World/Overworld`, loaded in the order listed in `randomizer/core/src/slice/world_files.rs`), flag names (`data/flags.json`), start presets
- `tools/audit.py`: the check that no game data slipped into the repo. Run it before a release.
- `tools/repath_patch.py`: rewrites a patch made before the move to `randomizer/` and `web/` so it applies to the new paths (`python tools/repath_patch.py old.patch new.patch`). Hunks for the old `data/logic/World/Overworld.json` need moving by hand into its area file.
- `packaging/`: installers

## Adding a feature

- A rule about which item may sit at which spot goes in `randomizer/core/src/slice/place_rules.rs`: write a function that answers for the spots it knows (Some) and passes on the rest (None), and add it to `RULES`. The first rule with an answer wins, so its place in the list matters.
- A file the build adds or edits after the chests goes in `randomizer/core/src/slice/stages.rs`: write a stage function that pushes its files, and add it to `STAGES`. Stages run in list order. If a stage needs something new from the build, add it to `Ctx`.
- Settings go in the feature's own module where it has one (see `shops/plan.rs`), and on an In Dev menu page.

## Code style

1. No comments of any kind. No `//`, `///`, `//!` or `/* */`. Explanations go in the pull request or the docs.
2. snake_case for variables, parameters, functions and modules. CamelCase types and SCREAMING_SNAKE constants.
3. Short locals. Use `p` for a path, `f` for a file, `b` for bytes, `d` for data, `n` for a count, `k` and `v` for key and value, `i` and `j` for indexes, `out` for a result being built.
4. Short aliases on imports, such as `use std::path::{Path as P, PathBuf as PB}`.
5. Explicit type annotations on local bindings, such as `let n: usize = v.len()`. Skip them for closures and long iterator chains.
6. One concern per file. Small modules. No `utils` or `helpers` dumping ground.
7. Public names stay stable. Never rename vendored or generated code.
8. `unsafe` is forbidden.
9. Fallible functions return `Res<T>` from `error`. `unwrap` and `expect` are for tests only.
10. Tests sit in a `#[cfg(test)] mod tests` block at the bottom of the file they cover.
11. In prose, docs, commit messages and chat: no em-dashes and no semicolons.

## Settings

- Every new setting defaults to the old behaviour. With it off, seed files and spoilers must stay byte identical to `nightly`. Show that in your pull request with a few seeds per mode (default, chests, pots and crates and so on).
- Old settings strings must keep their meaning. The settings model has an optional baseline for this, see the commit that made story and NPC rewards on by default.
- Number your settings in a range the maintainer gives you, so two branches never collide.
- Write a plain tip for each setting and say when it is experimental or untested in the game.
- Every new setting goes under the **In Dev** heading of the settings menu first, never straight onto the Randomizer or Gameplay pages. Add it to an In Dev page in `randomizer/core/src/settings/menu.rs` (an existing one, or a new page named after the feature with `heading: IN_DEV`). A maintainer moves it to its proper page once it is reviewed. This keeps the main pages to settings that are known to work. A setting with no place in the menu also lands under In Dev, on the Unsorted page, but the menu test still asks you to place it.

## Editing the game's files

- Use the existing writers (`bfevfl` flows, `ainb` graphs, `byml`, SARC packs, the resource size table). Edit only what you mean to change and check that the file still rewrites byte for byte.
- **Size table rule.** If a file inside a pack grows, grow the resource size table entry of that inner file too, not only the pack's entry. If you do not, the game crashes while loading. One mod has exactly one size table.
- Derive new files from the player's own game at build time. Never commit one.
- **Flow load rule.** An event that calls into a flow its event row does not load is cancelled in the game (the cow ocarina bug). After a flow edit, run `ootrr flow-loads <romfs_root> <mod_romfs_folder>` on the built mod. It lists the events of the mod that call flows their row does not load, and it should list none.

## Finding flags and state

The game keeps its state in named and unnamed save flags. To find the flag behind something:
1. Make two saves, one before and one after the thing happens.
2. `ootrr save-diff <a> <b> --flags data/flags.json` lists what changed.
3. `ootrr save-get <progress.sav> <name_or_hash>... --flags data/flags.json` reads values, and `save-set` and `save-unlock` make test saves.

Unnamed flags show as a hash such as `0xef10b80f` (the world clock in seconds).

## Testing in the game

- Hookshot is what we test on. Put one randomizer mod at a time in its `mods` folder (a leading underscore disables a mod), because each mod carries its own size table.
- Keep test saves under the wallet cap, and place the player with `ootrr save-unlock ... --start Map/StartPos --romfs <game>`. Pick open field spots, not the chicken room in Lon Lon Ranch.
- Say in your pull request what you tested in the game and what you did not. Experimental features stay marked experimental until someone has played them.

## Pull request checklist

- Builds with no warnings and the full tests pass (say whether you ran the game tests)
- No comments in the code, style rules followed
- New settings default off, parity shown, old settings strings still parse
- Anything that grows a file in a pack grows its size table entry
- No game data, no saves, no generated files committed
- A line on what you tested in the game
