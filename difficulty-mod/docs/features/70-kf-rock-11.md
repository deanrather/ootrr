# Kokiri Forest rock 11 before the Deku Tree (upstream issue 70)

## Original request
Upstream issue 70: "unable to pick up or break rock in kokiri forest". The check "KF Child Rock 11" cannot be picked up or broken until after the Great Deku Tree. The reporter guessed it was tied to the Kokiri kid who is normally there, and that a time saver removing him leaves the rock stuck. The coordinator added the user's preference: remove the rock if the kid is gone, but only if no check is lost.

## Interpretation and findings
- Rock 11 is the actor `Obj_LiftRock_A_01` (hash 2661037609348760452) in `KokiriForest_Child_Static`, at (-45.8, 111.8, -27.8). The same spot holds `Obj_LiftRock_A_02` (hash 6257885313885825332), which is linked to the Kokiri kid `Npc_Kokiri_008_Child` (the one who teaches the sword, shield and rock lifting; his anchors include `ForPracticeArea`).
- The map's logic module `Logic/KokiriForest_Child_Level_All_02_6960.logic.module.ainb` decides which of the two stands there, from the game's saved flag `Get_StoneOfForest_IsCompleted_Exp`: the kid's lesson rock (A_02) before the Deku Tree is done, the check rock (A_01) only after. So in the vanilla game rock 11 does not exist before the Deku Tree, with or without any time saver. Nothing in the randomizer removes the kid (a search for him in the code finds nothing), so the time saver in the report is not the cause: the cause is the game's own flag, and it hits every seed in which rocks are shuffled, since the logic treats rock 11 as a child check from the start.
- Removing the rock was not chosen: with rock shuffle on it is a check and would lose its item. With rock shuffle off the rock is plain scenery and is left as the game has it.

## Implementation
`randomizer/core/src/pot_slice/kf_lesson.rs`, staged next to the Kokiri ring fix (`kf_ring.rs`) whenever rock shuffle is not off (`slice/stages.rs`, `pot_rows`). It forces the check rock's `Logic_Create` to true and `Logic_Delete` to false (the same constant-input edit other features use). The lesson rock and the kid are untouched, so the lesson works as before and the two rocks stand in the same spot before the Deku Tree. After the Deku Tree the lesson rock is deleted and the check rock stays, as in the game. Data edit only, no code patch, no new saved flag.

## Tests
- `cargo test -p ootrr-core kf_lesson`: the game's module has the check rock wired to the flag; after the edit its create and delete inputs are constants (true, false), its plugs are gone, the lesson rock's inputs and plugs are identical, the graph passes the AINB audit and reads back byte for byte.
- Rock shuffle off: the stage is not added, so default output is unchanged.
- Played in the game: not yet (see below).

## How to test in the game
Rock shuffle on (all or overworld), a fresh child save before the Deku Tree. Go to the rock behind the kid (the spot with the lesson rock, about -45.8, 111.8, -27.8) and lift or break the check rock (it may sit under or beside the lesson rock); the check is given. Control: rock shuffle off, vanilla. After the Deku Tree the lesson rock is gone and rock 11 still works.

## Kid interruption (user, 23 Nov)
The user decided a shuffled rock is never removed and asked whether the kid's walk-past interruption could be taken away instead. Findings: the kid (`Npc_Kokiri_008_Child`) has a `Near` entry that only shows a speech balloon (`Near_01`, `Near_03`), his own talk entries start when Link presses A, and a ring of `Area` triggers (the lesson's practice area) surrounds the lesson rock. No time saver in the randomizer edits him or his flow, so the interruption itself was not reproduced or isolated here; if it is confirmed in the game, the next step is to find which trigger starts his `Camera_ForPracticeArea` or `GameStarted_*` entry. The fix above does not depend on it: the check rock no longer waits for the Deku Tree, and the kid and his lesson rock stay as the game has them.

## Notes
- The two rocks overlap at one position before the Deku Tree; whether they look odd or the lesson rock hides the check rock needs a look in the game.

## MR Drafts

### Initial MR Draft
**Title:** Improved Shuffle > Small Rocks Fix: Kokiri Forest rock 11 can be lifted before the Deku Tree

With rock shuffle on, the check "KF Child Rock 11" now stands in Kokiri Forest from the start of the game. The game only placed it after the Great Deku Tree (a lesson rock stood there before), so the check could not be reached early. The kid and his lesson rock are unchanged, and nothing changes with rock shuffle off.

Fixes #70

For details about implementation and testing, see difficulty-mod/docs/features/70-kf-rock-11.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
