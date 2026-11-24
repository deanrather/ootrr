# Fall in pit penalty

Setting `fall_pit_penalty` (number 600; it was 162 until 24 Nov, when upstream nightly gave 162 to its own fishing setting, so a settings string made by an older fork build with this setting off its default is not read back correctly: its old number now means upstream's setting. The setting is fork-only, so nothing upstream changes), a slider from 0 to 3, default 1 (the game's own penalty). Difficulty page, section Player. Code: `randomizer/core/src/difficulty/falls.rs`, wired in `difficulty.rs`.

## The request
First: "near-death falls: falling down a hole takes you down to 1 heart" (22 Nov). After playing that version (hands-on 15), the user turned it into a slider, "Fall in pit penalty", 0 to 3:
- 0: no penalty (a fall costs nothing)
- 1: the default penalty
- 2: leaves you on 1 heart; at 1 heart or less, nothing more is lost (changed 23 Nov after the first test: it used to take the rest)
- 3: leaves you on zero hearts (every fall: a fairy, or game over)

## How it behaves
"A fall" is any void-out: a pit, lava, quicksand. Falls from a height onto solid ground, enemy hits and the damage options are unchanged. A bottled fairy revives Link as usual wherever a fall takes the last heart.

## Investigation
- The void-out is the event flow `Event/EventFlow/DmF_SY_FallDown.bfevfl.zs`. Its main entry fades out, runs the player's own `EventPlayerAbyss` action once, puts Link back (`ReturnFromAbyss`, `EventPlayerPostAbyss`) and gives the usual short invincibility. Lava, whirlpools, quicksand and pits share it; its `EventQueryCheckPlayerAbyssType` branches (types 3 to 9) only change the camera and fades.
- `EventPlayerAbyss` has one parameter, `InvalidDamage` (false in the base game), which feeds the engine node `ExecuteEventPlayerAbyss`. No parameter file holds the fall cost.
- **The game's own cost depends on what Link fell into.** Shadow Temple pits cost half a heart (2 quarter hearts, seen in hands-on 15); lava in the Fire Temple boss arena costs one heart (4, seen in the damage-options tests).
- The player's event graph `AI/Player.event.root.ainb` (in `AI.Global`) implements the life actions a flow can use:
  - `EventQueryCurrentLife` with `Value`: compares Link's life with `Value`, answering 0 for less, 1 for equal, 2 for more.
  - `EventTriggerHealLife` with `Value`: `OneShotLifeSetOwnLife` with set type 0 and that value. A negative value lowers life: hands-on 15 saw it bring Link from 20 hearts to 1.
  - There is no "set life to N" action, so the edit picks one fixed heal per life value.
- A void-out that kills runs the game over flow (`DmF_SY_GameOver`, entries `GameOverReturnFromAbyss` and `PlayerDead`). That flow is not changed. Death always comes from the game's own fall damage, so a fairy and the game over screen work as usual.

## The bug in the first version (on/off, hands-on 15)
The on/off version brought falls above 1 heart down to 1 heart, then ran the base game's event at 1 heart or less, assuming it costs one heart. In Shadow Temple pits it costs half a heart, so a fall at 1 heart left half a heart instead of ending the game. The scripted check had used lava, which does cost one heart, so it passed. The new levels never depend on the size of the game's own cost: to take the rest, they bring life down to a quarter heart first, and then any cost kills.

## Implementation
(Levels 0 and 2 were rebuilt on 23 Nov as an entry wrapper: see "Fixes after the tester runs". The descriptions of their low-life branches below are the old ones.)

In `DmF_SY_FallDown`, the one `EventPlayerAbyss` event is reached through new events (marked `OOTRRPitPenalty`):
- **0:** a copy of the event with `InvalidDamage` true replaces it. Nothing else.
- **1:** no edit; nothing is staged.
- **2:** an `EventQueryCurrentLife` check against 4.
  - More than 4: the no-damage copy runs, then a balanced tree of life checks (5 to 80, at most 7 deep) runs the one heal of 4 minus life. Link ends on exactly 1 heart.
  - 4 or less: a tree over 2 to 4 heals down to 1 quarter heart, then a 1-frame `EventWait`, then the vanilla event with its damage. That damage ends the life: fairy or game over.
- **3:** like the low branch of 2, but over every life from 2 to 80: a heal to 1 quarter heart, a 1-frame wait, then the vanilla event.

Every branch ends where the vanilla event ended, the end of its fork branch, so the return, fade and invincibility run as in the base game. Other entries are untouched. The edit is refused for level 1, for values outside 0 to 3, when the flow does not run `EventPlayerAbyss` exactly once, and when it is already edited. Life tops out at 80 (20 hearts).

## How I tested it
- Unit tests (`cargo test -p ootrr-core falls fall_pit`), on a small flow built like the real one, walking every path:
  - 0: every life keeps its value and the event runs with no damage.
  - 2: every life from 5 to 80 ends at exactly 4 with no damage; every life from 1 to 4 ends at 1 and then the damaging event.
  - 3: every life from 1 to 80 ends at 1, then a wait, then the damaging event.
  - Paths take at most 12 steps; each walks to at most one heal and one abyss event.
  - The other entry and the original event are untouched.
  - Level 1, out-of-range values, a second edit and a flow without the abyss event are refused.
  - The setting reads back from the settings string, and the default (1) leaves the default string unchanged.
- Game-file tests (with `OOTRR_ROMFS`):
  - The real flow takes levels 0, 2 and 3 and reads back.
  - Only the vanilla switch leads into the new events.
  - Each level stages that one flow file and nothing else.
  - `ootrr flow-loads` on built mods for 0, 2 and 3 reports no missing loads.
- By hand, the old on/off version (equivalent to level 2 above 1 heart): Shadow Temple pits, child, 20 hearts. Falls from full and from 5 hearts left 1 heart. A fall at half a heart ended the life (a fairy, then game over). A fall at 1 heart left half a heart: that was the bug fixed here.
- The new levels have not been played.

## How to test it in the game
1. Build with `fall_pit_penalty` set to 2, plus `skip_intro`, `skip_cutscenes` all, `faster_chests` and `starting_hearts` 10 or more. Carry a bottled fairy.
2. Shadow Temple pits (`teleport ShadowTemple`, `warp 56.82 56.37 69.16`): fall at full health, which should leave 1 heart. Fall again at 1 heart: the fairy revives you. Fall again at 1 heart without a fairy: game over.
3. Level 3: any fall uses the fairy, or ends the game.
4. Level 0: falls cost nothing.
5. Level 1: falls cost what they always did (half a heart in Shadow Temple pits).

## Notes for the next feature
- The game's own fall cost differs by surface. Never assume one number: test pits and lava both.
- A negative `EventTriggerHealLife` is a working way for a flow to lower Link's life; to kill, lower it to 1 and let the game's own damage do it, so the death runs through the normal paths.

## Fixes after the tester runs (23 Nov)
First run: level 0 from half a heart killed Link, and level 2 from exactly one heart went 4 to 1 to 0. The second was the old design (take the rest at 1 heart or less); the user's spec is "leaves you on 1 heart", so level 2 now leaves 4 or less unchanged.

Second run (L0b, L2b: a +4 lift and a -4 undo around the abyss event): lives 3, 4, 5, 8 and 80 passed, but lives 1 and 2 still died, about 0.5 to 1.5 s into the fall, and the vanilla control (level 1) shows why: its cost is 2, a fall at 2 kills at the abyss, while a fall at 4 only loses the 2 after Link is back. So the game seems to decide death from Link's life when the void-out starts, and a lift placed at the abyss event is too late for lives 1 and 2 (it worked at 3 and 4, where nothing would have killed anyway). The engine node behind the event is not readable here, so this is a reading of the samples, not a proof.

Third version (L0c, L2c): the lift moves to the very start of the flow.
- The flow's own entry now points at a new check, and the old main chain becomes a second entry, `OOTRRPitOriginal`, called as a subflow.
- Life 2 or less: heal +8, wait 4 frames, run the whole original flow (whose abyss event now has `InvalidDamage` true), then heal -8.
- Life 3 or 4: the same with +4.
- Level 0, life above 4: the original flow directly. Level 2, life above 4: the old ladder lowers life to 4, then the +4 path.
- Every path ends with life back where it was (level 0) or at 4 (level 2, from above 4). Level 3 is unchanged (the old design, which tester runs showed killing from every life).
- The undo runs after the whole original flow, once Link is back, so it cannot be mistaken for a fall.
Unit walks follow the subflow call and cover every life from 1 to 80 at each level.
If lives 1 and 2 still die, the death is decided before any flow can run, and only a code change could fix it; say so in the report.

## Known limit: half a heart or less (round 3, 23 Nov)
Round 3 of the tester runs (commit `3d27b69`, mods L0c and L2c) was correct from 3 quarter hearts up: level 0 takes no damage, level 2 ends at exactly 4 from above 4 and leaves 4 or less alone, level 3 ends the game (checked with the fairy-free save). At 1 and 2 quarter hearts both level 0 and level 2 still kill, about 1.2 to 1.75 s into the fall, before any flow step can lift life. The vanilla control (level 1) kills from 2 quarter hearts too, even with a bottled fairy in a bottle, and loses 2 quarter hearts (4 to 2) from 4. So from half a heart or less a pit fall still kills, as in the base game, and the slider cannot change that: the death is decided before the event flow can act. This is accepted as a known limit (default taken, pending the user). The tooltip says so.

## Test status (23 Nov)
Rebased state is on current `origin/main`. `cargo test -p ootrr-core difficulty` (312 passed), `settings` (114 passed) and `cargo test --workspace` pass; the audit has no failures. Not played yet: a tester plan and one built mod per level (0, 2, 3) are ready. Lava that burns over time is a different mechanism and is untouched; the void-out flow cannot tell a sinking-lava void-out from a pit, so both count as falls.

## MR Drafts

### Initial MR Draft
**Title:** New Difficulty > Fall in pit penalty Feature: Choose what falling into a pit costs

A new Difficulty slider, Fall in pit penalty, 0 to 3: 0 costs nothing, 1 (default) is the game's usual penalty, 2 leaves you on 1 heart (at 1 heart or less nothing more is lost), 3 leaves you on zero hearts (a bottled fairy still revives you). Data edits only; default off, settings strings and default seeds unchanged. Not yet played at levels 0, 2 and 3.

For details about implementation and testing, see difficulty-mod/docs/features/49-fall-pit-penalty.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

### 2nd MR Draft
**Title:** New Difficulty > Fall in pit penalty Feature: Choose what falling into a pit costs

A new Difficulty slider, Fall in pit penalty, 0 to 3: 0 costs nothing, 1 (default) is the game's usual penalty, 2 leaves you on 1 heart (at 1 heart or less nothing more is lost), 3 leaves you on zero hearts (a bottled fairy still revives you). Known limit: from half a heart or less, a fall still kills, as in the base game; this cannot be changed from the event flow. Data edits only; default off, settings strings and default seeds unchanged. Tester-run in the game at levels 0 to 3 from 3 quarter hearts up (Shadow Temple pits and Haunted Wasteland quicksand); not hand-played.

For details about implementation and testing, see difficulty-mod/docs/features/49-fall-pit-penalty.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
