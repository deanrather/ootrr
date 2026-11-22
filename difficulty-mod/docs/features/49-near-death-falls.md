# Near-death falls

Setting `near_death_falls` (number 162), checkbox, default off. Difficulty page, section Player. Code: `randomizer/core/src/difficulty/falls.rs`, wired in `difficulty.rs`.

## The request
"Near-death falls: falling down a hole takes you down to 1 heart", instead of the usual cost of one heart.

## How it behaves
- Any void-out (falling into a pit, into lava, or sinking in quicksand) with more than 1 heart left brings Link back at the safe spot with exactly 1 heart (4 quarter hearts). He is not hurt any further and there is no game over.
- With 1 heart or less, the fall runs exactly as in the base game: it costs a heart, so at 1 heart it ends the game. A bottled fairy still revives him as usual.
- Falls from a height onto solid ground, enemy hits and the damage options are unchanged.

## Investigation
- The void-out is the event flow `Event/EventFlow/DmF_SY_FallDown.bfevfl.zs`. Its main entry fades out, runs the player's own `EventPlayerAbyss` action once, puts Link back (`ReturnFromAbyss`, `EventPlayerPostAbyss`) and gives the usual short invincibility. Lava, whirlpools, quicksand and pits appear to share it: its `EventQueryCheckPlayerAbyssType` branches (types 3 to 9) only change the camera and fades, and the lava respawn in the Fire Temple boss arena was measured at the same one heart.
- `EventPlayerAbyss` has one parameter, `InvalidDamage` (false in the base game). By its name and the engine node it feeds (`ExecuteEventPlayerAbyss`), it switches off the one-heart cost; not yet seen in the game. No damage number exists in any parameter file (the player's `PlayerGameParam` and damage files have none, as damage options found).
- The player's event graph `AI/Player.event.root.ainb` (in `AI.Global`) implements the life actions the flow can use:
  - `EventQueryCurrentLife` with `Value`: compares Link's life with `Value` and answers 0 for less, 1 for equal, 2 for more.
  - `EventTriggerHealLife` with `Value`: `OneShotLifeSetOwnLife` with set type 0 and that value (the Great Fairy uses 32, the diving game 4).
  - There is no "set life to N" action, so the edit chooses one fixed heal per life value.
- When a void-out is a death, the game runs its game over flow (`DmF_SY_GameOver`, entries `GameOverReturnFromAbyss` and `PlayerDead`) instead. That flow is not changed.

## Implementation
In `DmF_SY_FallDown`, the one event that runs `EventPlayerAbyss` is reached through a new check:
1. `EventQueryCurrentLife` against 4. Life 4 or less goes on to the untouched vanilla event, so nothing changes there.
2. More than 4: a copy of the event with `InvalidDamage` true runs instead, then a balanced tree of `EventQueryCurrentLife` checks (5 to 80, at most 7 checks deep) picks the one `EventTriggerHealLife` with `Value` = 4 minus the current life, so life ends at exactly 4. Every branch then goes where the vanilla event went (the end of its fork branch), so the rest of the flow (return, fade in, invincibility) runs as in the base game.

Everything else in the flow and its other entries is unchanged. The edit is refused if the flow does not run `EventPlayerAbyss` exactly once, or was already edited. Life tops out at 80 (20 hearts); above that nothing is changed.

## Not known yet (game test needed)
- That a negative `EventTriggerHealLife` lowers life. If the game ignores it, the setting would remove the fall cost altogether above 1 heart. The first test checks exactly this. Fallback if it fails: lower and restore the maximum with `EventTriggerSetMaxLife`, which `lose_heart_on_death` already uses.
- Whether the base game takes the heart inside `EventPlayerAbyss` or before the flow starts. Either way the result is 1 heart: the copy that runs above 1 heart has the damage switched off, and the tree reads life after it.

## How I tested it
- Unit tests (`cargo test -p ootrr-core falls`, `near_death`): on a small flow built like the real one, every life from 5 to 80 walks to exactly one heal that leaves 4, through the no-damage copy, in at most 12 steps; life 0 to 4 reaches the vanilla event with its damage and no heal; the other entry and the original event are untouched; a second edit and a flow without the abyss event are refused; the setting reads back from the settings string and off leaves the default string unchanged.
- Game-file tests (with `OOTRR_ROMFS`): the real `DmF_SY_FallDown` is edited and reads back, has the vanilla event plus the no-damage copy, and only the vanilla switch leads to the new check; with the setting on, the build stages that one flow file and nothing else.
- Not played in the game.

## How to test it in the game
1. Build with `near_death_falls` on, `skip_intro`, `skip_cutscenes` all, `faster_chests`, and several hearts (e.g. `starting_hearts` 10).
2. With more than one heart, fall into a pit (Deku Tree basement, Dodongo's Cavern) or lava (Fire Temple boss arena edge). Pass: Link comes back at the edge with exactly 1 heart, can move, no game over.
3. With 2 hearts and with 1.25 hearts: back with 1 heart.
4. With exactly 1 heart: game over, as in the base game. With a bottled fairy: revived.
5. With the setting off: a fall costs one heart, as before.
