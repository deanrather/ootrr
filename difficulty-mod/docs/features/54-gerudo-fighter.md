# 54. Gerudo fighter: the fortress thief as an ordinary enemy

## Requested

User, 23 Nov: "make gerudo fighter a spawnable. we need to make a modified copy which doesn't have the dialog and warping behaviors." The fortress thief (`Enemy_MiddleBoss_GerudoThief`) talks and sends you to jail when she wins, and plays a scene when beaten, so she cannot be placed like another enemy.

## Interpretation

A copy of her under its own actor name, `Enemy_Gerudo_Fighter`, with the same body, moves and swords but none of the scenes: visible, notices Link, attacks, and when she lands a hit that would have caught Link, it only damages him. She dies like an ordinary enemy (no freeze) and drops something ordinary. Ordinary enemy music. The real thieves in the Gerudo Fortress are not touched. Available to the features that add enemies and for spawning in tests.

## What was found (files, 23 Nov; research in the working notes, `reference-gerudo-fighter`)

- Four thief actors share one set of graphs; the plain one is the base. The scenes are not in her graphs: her brain asks the shared boss module to start two events by key, `Arrest` and `Death`, whose names come from her `DungeonBossCommonParam` file (`GerudoFortress_MiddleBossLose`, `GerudoFortress_MiddleBossDefeat_01`). The jail destination is map data linked to the fortress placements.
- The jail request starts in her spin attack module `Enemy_MiddleBoss_GerudoThief.action.LargeAttack.module`: when the spin hits and Link's life is above 0, a switch (element 64, `QueryLifeIsDamagedFromActorLink`) sends its `True` plug to element 3, `ExecuteBlackboardSetBool IsRequestArrestEvent`. Nothing else in her graphs sets it.
- Her death table (`Enemy_MiddleBoss_GerudoThief_game__death__...`) has one cell, `LifeDepleted`, with `IsDropItem` false and no `IsWaitForSignal`, so it inherits `EnemyCommon`, where the wait is true. The signal normally comes from the defeat scene, which the clone never plays (the Volvagia freeze, feature 21).
- Her own drop reference is `DropTable_None`.
- The death tables of the whole game (203 of them) fit one rule: the size table entry is the file length plus 5,040 to 5,400 bytes plus 240 bytes per cell (feature 21). A test checks the rule against every table and our entry against the rule.

## What was built (`randomizer/core/src/difficulty/gerudo.rs`)

(Superseded 23 Nov, see Update below: the setting was removed.) Setting `gerudo_fighters`, number 167, "Gerudo fighters (experimental)", Difficulty page, Enemies section (last), default off. Number 163 is taken by `club_moblin_all_round` on its branch and 165 and 166 by timed gems and enemy feints; 167 was free on every open branch on 23 Nov (renumber at merge if that changed). It is a setting of its own and not folded into More enemies because the clone has never run in the game: a shipped slider should not start adding an untested miniboss. With the setting off nothing is staged, so the default output is the same as main by construction (the horde and sizes edits are inert without it).

With it on, the build adds:

1. A pack `Pack/Actor/Enemy_Gerudo_Fighter.pack.zs`: the thief pack as staged (so enemy health, damage and speed scaling apply to her) plus seven files of its own:
   - ActorParam, child of the thief's, with four own references: `GameParameterTableRef`, `DeathRef`, `DropRef`, and `BattlePresentationRef` pointing at the pack's `EnemyCommon` file (ordinary enemy music, no boss presentation).
   - GameParameterTable pointing at an own `DungeonBossCommonParam` file. In it both event keys (`Arrest`, `Death`) and both start conditions (541910881, 2204074072) are kept and both `EventName` values are empty; the Dream World name is removed. The brain preloads and starts both keys, so the keys must exist; with empty names nothing plays (the same as the Volvagia clone's `Die`).
   - DeathParam pointing at an own death table.
   - Own DropParam, DropTable and DropSubTable: one red rupee, 100 %, `IsReplace` (the route that worked for Volvagia in round 5). The one donor file, the empty sub table, comes from the Floormaster pack.
2. A copy of the spin attack module inside that pack, with one plug changed: the `True` plug of the switch (element 64) now goes to the idle element 65 instead of the set-bool element 3. Same node types, same file size, nothing added. After this the arrest request can never be set, so the brain never takes the Arrest path. The same-name file sits inside the clone pack, as the Mega Like Like and regen edits do; the vanilla pack keeps its own copy. Every build makes this edit (the constant `KEEP_AI` is false). The build-time switch `OOTRR_GERUDO_KEEP_AI`, which built the clone without it (the fallback the research proposed, to tell an AI problem from a parameter problem), was removed on 24 Nov: no environment variable changes what a build writes. The unit tests still build that variant by passing the flag as a parameter.
3. A loose death table `Enemy_Gerudo_Fighter_game__death__DeathReasonProcedures...`: a copy of the thief's with each of the eleven enabled reasons (`AncientWeapon`, `ChemicalCriticalHitFire`, `Crushed`, `Drowned`, `FallIntoAbyss`, `Frozen`, `LifeCondition`, `LifeDepleted`, `PhysicsKinematicCollision`, `SideCrushed`, `UpperCrushed`; checked against the merged `Default`, `EnemyCommon` and thief tables by a test) set to `IsDropItem` true, `IsWaitForSignal` false, `IsRequestDestruction` true and both respawn flags false. Her own `LifeDepleted` procedure (3265626599) is kept. Cells the thief file lacks are added as partial cells and the parents fill the rest. Size table entry: the thief table's entry plus the aligned growth plus 256 bytes per added cell plus 1 KB (the feature 21 rule; the test shows at least 512 bytes over what the rule needs).
4. `RSDB/ActorInfo`, `GameActorInfo` and `Tag` rows copied from the thief, and size table entries for the pack and for every new file (each at least its parent's entry plus its own length).

Placement: an ordinary placed actor with no links and one `Dynamic` key, `IsSetBoolGameDataRecordingDeath` false (the same value the vanilla third thief carries), so no saved flag is written and nothing is recorded when she dies. She has no `Drop__DropTable` key, so her own drop table applies.

More enemies: with this setting on and More enemies at 500 % or more, she is one more miniboss kind in the horde (`horde::FIGHTERS`, `Class::Mini`, fun 2, one per group), under the existing miniboss cap and the existing rules (never in the skip maps, which include the Gerudo Fortress, never in child Kokiri Forest, never in the Water Temple, 25 units from other big enemies: she is on the `BIG` list). With the setting off the pool is the old one and a test compares the plans. She is on the never-resize list of random enemy sizes.

Test spot (unit tests only; the constant `TEST_SPOT` is false in every build, and the build-time switch `OOTRR_GERUDO_STAGE` was removed on 24 Nov): one fighter in the child Temple of Time at (92.0, 104.1, -34.0), on the temple's axis, 7.5 units from the warp point `99.5 105.2 -34`, floor level read from the map's own anchors.

Music: ordinary enemy music (default taken by the research; the user has not objected).

## Tested

Unit and game-file tests (`OOTRR_ROMFS` set; `cargo test -p ootrr-core difficulty` and `settings` and `--workspace`, see the report for the numbers):

- Names and paths; the clone pack equals the thief pack except the spin module and adds exactly seven files with the right references (four ActorParam components, own table, own death file whose parent is the thief's); a second build is refused.
- Boss parameters: both keys and conditions kept, names empty, no fortress text, same parent.
- Spin module: one set-bool before and one after; before it has one parent and after none; same length; exactly one node changed; the graph audit is clean and reads back; a second pass is refused.
- Death table: each enabled cell has the five values; every other cell is the game's, byte for byte; her own procedure kept; the table round-trips; only field names and value kinds used by the game's tables.
- The enabled list equals the merge of the three inherited tables.
- Death table size entry against the per-cell rule over every death table in the game.
- Drop files: one red rupee, 100 %; table and param point at each other; a donor without the sub table is refused.
- Rows and tags copy the thief; a second insert is refused.
- Staging: files, sizes for all nine new entries (seven own plus pack plus table), every own file's entry covers its parent's, the loose table is unknown to the game and gets an entry, the test placement adds one actor with no links, the fortress map has none.
- Scaling: with health 50 % and damage 200 % the clone has 9 life (the thief 18) and a different attack file.
- The test spot is inside the temple (the hall prop on its axis, floor level of the temple's anchor, nothing within 5 units).
- Horde: off, no fighter and the plans equal the old ones; on at 10x some map gets one; only as a miniboss; never in the skip maps; its placed actor carries only the one `Dynamic` key and no links.
- Settings: read, the settings string round-trips, notes count; the default settings string is unchanged.

Not run in the game. The test plan is `playtest\gerudo-fighter\PLAN.md` (working folder).

## Update, 23 Nov: part of More enemies

Hands-on 16 verdict (user): "gerudo fighter is great"; beating her and being beaten both "worked perfect". Decision (user): "she's part of more enemies". The separate `gerudo_fighters` setting (number 167), its menu entry and its settings text are gone. When More enemies is on (above 2x, where the horde is staged) the clone is staged and joins the pool as a Mini kind with the same placement rules as before: BIG list spacing, never in the skip maps (Gerudo Fortress included), not in child Kokiri Forest or the Water Temple, never resized. `Opts.gerudo` is now derived from the More enemies value in `difficulty::read`. With More enemies off nothing is staged, so the default output is the same as main. The More enemies tooltip mentions her; the build report line says she is part of More enemies. Tests: the horde test (fighter only as a miniboss, never in skip maps) and the Opts round trip were updated (the note count is 30 with More enemies at 150).

## How to test it in the game

Use the time savers (`skip_intro`, `skip_cutscenes` all, `faster_chests`, `starting_hearts` 20) and the arsenal save. Child Temple of Time (`teleport Market_Child`, `warp 99.5 105.2 -34`). Build with More enemies at 500 % or more (or, for the first run, the old `gerudo_fighters` on), then spawn `Enemy_Gerudo_Fighter` with the bridge (no build places one in the temple any more). Pass: she is visible and attacks with her swords; a spin hit takes hearts and sends Link nowhere and starts no dialog; killed with the sword she plays her death, vanishes, leaves a red rupee and the game keeps running; she is back when the map reloads (nothing is recorded). Control: the same build with the setting off has no such actor. Then load the adult Gerudo Fortress with the setting on and check the vanilla thieves still fight, talk and jail as before.

## Notes for the next feature

- To make any boss or miniboss spawnable: blank its `EventStartParameters` names and keep the keys; clear `IsWaitForSignal` and set `IsDropItem` on every enabled death cell; give it its own drop table; size the death table by cells.
- What C++ does when the jail request is never set is not proven; the fallback variant (the clone without the AI edit, now built only by the unit tests) would separate an AI edit problem from a parameter problem.
- If a vanilla thief and a clone are ever loaded together, the same-name spin module in two packs shares one size entry; this is why the horde never places her in the fortress.

## MR Drafts

### Initial MR Draft

**Title:** New Difficulty > Enemies Feature: Gerudo fighters, a fortress thief that fights like any enemy

A new experimental toggle, "Gerudo fighters" (`gerudo_fighters`, number 167, default off, Difficulty page, Enemies section) adds a copy of the Gerudo Fortress thief that can be placed or spawned like any enemy: no dialog, no defeat scene, no jail (a spin hit only hurts), ordinary enemy music, her own drop (a red rupee). With More enemies at 500 % or more she joins the minibosses. The fortress thieves are unchanged. Untested in the game: see the test plan in the doc. Check the setting number (167) against other open PRs at merge.

For details about implementation and testing, see difficulty-mod/docs/features/54-gerudo-fighter.md.


### 2nd MR Draft

**Title:** Improved Difficulty > More enemies Feature: Gerudo fighters join the enemy pool

The Gerudo fighter (a copy of the fortress thief that fights like any enemy: no dialog, no defeat scene, no jail, a red rupee drop) is now part of More enemies instead of a setting of its own. The `gerudo_fighters` setting is removed. She appears as a miniboss in the pool, with the usual spacing, never in the fortress or the peaceful places. Played by hand in the Temple of Time: beating her and being beaten both worked.

For details about implementation and testing, see difficulty-mod/docs/features/54-gerudo-fighter.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
