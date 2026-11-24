# Kokiri Forest rock 11 before the Deku Tree (upstream issue 70)

**Superseded on 24 Nov.** Upstream nightly (commit 7146d21, 22 Nov) took rock 11 out of the shuffle itself: `data/rocks_catalog.json` lists "KF Child Rock 11" under `retire`, so it is no longer a check. When nightly was merged into the fork, the logic rule described below (`AFTER_DEKU_TREE` in `randomizer/core/src/pot_slice/rocks.rs`) and its two tests were removed: the location they named no longer exists, and both tests failed on the merged tree. `rocks.rs` on the fork is now upstream's file unchanged. The rest of this page is the record of the fork's own fix.

## Original request
Upstream issue 70: "unable to pick up or break rock in kokiri forest". The check "KF Child Rock 11" cannot be picked up or broken until after the Great Deku Tree. The reporter guessed it was tied to the Kokiri kid who is normally there, and that a time saver removing him leaves the rock stuck. The user's preferences, in order: remove the rock if no check is lost; then keep the kid and drop his interruption; finally (23 Nov) "fix the logic. the logic is probably copied from the n64 game that behaved differently, so it's likely an issue in this version's logic."

## Findings
- Rock 11 is the actor `Obj_LiftRock_A_01` (hash 2661037609348760452) in `KokiriForest_Child_Static`, at (-45.8, 111.8, -27.8). The same spot holds `Obj_LiftRock_A_02` (hash 6257885313885825332), linked to the Kokiri kid `Npc_Kokiri_008_Child` (the one who teaches the sword, shield and rock lifting).
- The map's logic module `Logic/KokiriForest_Child_Level_All_02_6960.logic.module.ainb` decides which of the two stands there, from the saved flag `Get_StoneOfForest_IsCompleted_Exp`: the kid's lesson rock (A_02) until the Deku Tree is done, the check rock (A_01) only after. So in this game rock 11 does not exist before the Deku Tree, with or without any time saver; the N64 game had it from the start, and the randomizer's logic still treated it as a child check from the start.
- No time saver in the randomizer edits the kid; the kid's `Near` entry only shows a balloon and his talks start on A. His interruption was not isolated and is left alone.
- Removing the rock, or making it exist early (tried and reverted, commit 5c9cd37), was dropped: the game data stays as the game has it.

## Implementation (logic only)
`pot_slice/rocks.rs`: the rule of "KF Child Rock 11" is `is_child and 'Defeat Queen Gohma'` (the logic's own event for the Deku Tree being done, the step that sets `Get_StoneOfForest_IsCompleted`). Every other rock keeps its rule. Nothing in the game files changes; the rule is added only when Small Rocks (or grass) shuffle is on, so default seeds and settings strings are unchanged. Seeds with Small Rocks on can place differently than before, because rock 11 is no longer open from the start.

## Tests
- `the_rock_that_exists_only_after_the_deku_tree_needs_it_in_the_logic`: only that rock carries the new condition; with no items it is unreachable while "KF Child Rock 1" is reachable; with the vanilla placement and full kit it is reachable.
- `no_seed_needs_the_deku_tree_rock_to_finish_the_deku_tree`: for rocks "all" and "overworld", four generated seeds each; with rock 11's item withheld, the Queen Gohma check is still reachable.
- Existing rock tests (counts, beatability, default untouched) still run.
- Game data is unchanged against main: the branch no longer touches any game file stage (the earlier stage was removed).
- Played in the game: not needed for a logic-only change; not played.

## How to test
Generate seeds with Small Rocks on (see the tester plan in the playtest folder) and check the spoiler never lists, at "KF Child Rock 11", an item needed to reach the Deku Tree's end. In the game, before the Deku Tree a lesson rock stands there (vanilla); after the Deku Tree the check rock appears and gives its item.

## Notes
- Other rocks and grass with a similar story-flag dependency were not searched (out of scope). The module `Level_All_02_6960` is the only one examined.
- The first attempt (a data change in `kf_lesson.rs`) is in the history and reverted.

## MR Drafts

### Initial MR Draft
**Title:** Improved Shuffle > Small Rocks Fix: Kokiri Forest rock 11 can be lifted before the Deku Tree

(Superseded: this version changed the game data. See the 2nd draft.)

### 2nd MR Draft
**Title:** Improved Logic > Small Rocks Fix: Kokiri Forest rock 11 needs the Deku Tree

With Small Rocks on, the check "KF Child Rock 11" is now only in logic after the Great Deku Tree is done, since the game only places that rock then (a lesson rock stands there before). No game data changes.

Fixes #70

For details about implementation and testing, see difficulty-mod/docs/features/70-kf-rock-11.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
