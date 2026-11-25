# 43. Climbable drawbridge chains

Setting `climbable_drawbridge_chains` (number 158), toggle, default off. Difficulty page, section "Environment". Branch `feature/drawbridge-chains`, module `difficulty/chains.rs`. **Status (24 Nov): round 6 built and unit tested (climbable wire net along each chain). NOT yet seen in the game.**

## What was asked

User, 21 Nov: a just-for-fun feature. The Hyrule Castle Town drawbridge has two big chains from the bridge up to the gatehouse; make them climbable so players can carefully go up to the top. The menu says only "Climbable drawbridge chains" (a surprise). The lightning strike at the top in the first request was dropped in round 5 (a bolt created by logic never fires; see feature 39).

Refined (24 Nov): find a prop whose own pack carries climbable collision and lay it along each chain so Link climbs from the ground to the gate top; arrival must be safe (no fall into a void, a way down); game-file tests; load-safe.

## Research: which props can be climbed and are solid when placed (24 Nov)

Added map actors get no baked collision, so only props whose own pack creates a physics body (dynamic or kinematic) are solid. Climbable means a shape with a climb material.

| Actor | Body | Climb material | Verdict |
|---|---|---|---|
| `Obj_SpiritTemple_MoveClimbWall_B_01` | kinematic (`Ptm_ObjKinematicLarge_01`), the actor's own | `Physical` polytope 5.48 x 3.0 x 0.3 (local x +-2.74, y 0 to 3, z +-0.15) with `Material_WireNet` and `Wall_Climb`; `Front`/`Left`/`Right` shapes `AirWallForClimb` | **Used.** Solid and climbable by its own data. Moved only by its logic (`ActorLogicMoveClimbWall`); with no links it stays put. |
| `Obj_InsideTheDekuTree_FallingLadder_A_01` | kinematic, own | mesh in a `.phsh` (material not readable from the pack params) | Second choice: a ladder 2.4 wide, 5.4 high; logic makes it fall. |
| `FldObj_AirWall_ClimbPlayer`, `Land_IvyClimbableAirWall_A_01` | static (`*_StaticBody`) | `AirWallForClimb` | Invisible and climbable, but static bodies are baked: a placed `FldObj_AirWall_*` was not solid in round 1. |
| `Land_*_Ladder_*`, `Land_IvyClimbable_*`, `Land_SpiritTemple_ClimbWall_*` | static | in `.phsh` | Same problem as above. |
| `Obj_MovableBox_A_01` (rounds 3 to 5) | dynamic, own | stone (no climb) | Solid and on the chain line (round 4), but a walking ramp: Link crept up at about 0.04 units per stick push mid-ramp and slid off at the foot. |

The game always places the Spirit Temple net as a back-to-back pair at one spot (two `Obj_SpiritTemple_MoveClimbWall_B_01` at (13.685, 135.99, -17.5), yaw +90 and -90 degrees; the same for `_B_02`), so one panel is climbable from one face. Round 6 copies that: each strip is laid twice, facing opposite ways.

## How it is built (round 6)

- Geometry from the game's own files. The lowered bridge is `Land_HrlField_DrawBridge_A_01` at (0.0, 150.35, 34.02) in `HyruleField_Child_Static`. The chain centre line is at x = +-3.73 and runs from local (z 15, y 0) on the deck to (z -2.4, y 11.4) at the gatehouse (`PROFILE`, thirteen points read from the bridge model's vertex buffer).
- One net strip per profile segment (12 per chain), placed in `HyruleField_Child_Dynamic` like the planks before it: rotated about x by (slope - 90 degrees) so the panel's long axis runs up the chain, its upper face on the chain line (centre 0.15 below it), scaled to 1.4 wide (x 1.4 / 5.48) and to the segment length plus 0.4 at each end (y), thickness unchanged. Each strip has a twin: the same strip turned half a turn about x and started from the far end, so both faces are covered. 48 actors in all.
- Every strip copies the `Presence` of the lowered bridge (`IsHyruleFieldBridgeClose` false): the nets exist only while the bridge is down and vanish with it.
- Slopes: 0 to 5 degrees on the deck, 27 at the foot of the chain, 50 at the top (`PROFILE`); Link walks on the shallow part and climbs the steep part, if the game lets him climb a net at that angle (only the game can tell).
- Arrival and the way down: the top of the chain is the gatehouse wall at 11.4 above the deck. From the net Link can climb back down it, or let go and drop beside the bridge into the moat (water) or onto the deck. There is no void near the bridge.
- Setting text: label "Climbable drawbridge chains", tip "The drawbridge chains can be climbed. Carefully." The build report line (`Opts::notes`) says exactly what is added. Setting off: nothing is written; default settings string and seeds unchanged. The adult castle has a broken bridge and no chains: nothing changes there.

## History

- Round 1 (21 Nov): invisible `FldObj_AirWall_Common_Cube`, then `FldObj_AirWall_Player` boxes: not solid when placed (no baked collision). A lightning trigger at the top was built but never fired.
- Rounds 3 to 5 (22 to 23 Nov): scaled `Obj_MovableBox_A_01` planks in the `_Dynamic` map: solid, continuous, on the chain line within 0.25 (drop tests), vanish with the bridge, maps load. Walking up failed: slides at the foot, very slow mid-ramp. Lightning trigger dropped.
- Round 6 (24 Nov, this build): climbable Spirit Temple wire net along each chain, doubled back to back.

## How it was tested

`cargo test -p ootrr-core chains` (with `OOTRR_ROMFS`): the profile rises steadily and stays under 60 degrees; every net strip runs along its segment (direction within 1e-4), both segment end points lie on its upper face (within 1e-3) and inside its length, consecutive strips overlap, the strips sit on x = +-3.73 and mirror each other; each twin covers the same strip facing the other way; every face looks up; only the child field `_Dynamic` map is written, with exactly 48 new nets, all carrying the bridge's `Presence`, hashes unique, the old actors unchanged. Then `difficulty`, `settings`, `cargo test --workspace`, the default build compared byte for byte with main, and `flow-loads` (results in the MR draft).

Not tested in the game yet.

## How to test it in the game

Child, by day (bridge down), Hyrule Field. Warp to the deck end of a chain (x +-3.73, y about 152, z 47.5) and push the stick toward the castle. Expect: Link walks onto the net and, where it steepens, climbs it, up to about y 161.7 at z about 32. Then climb down, or drop into the moat. At night (bridge raised) the nets are gone. The Market and the castle load as before. A tester script and plan are kept with the playtest scratch (see `TESTING.md`).

## Notes for the next feature

- Solid added props must create their own body: look for `Ptm_ObjKinematic*` or `Ptm_ObjDynamic*` controller sets in the pack; `*_StaticBody` ones are baked into the map and do nothing when placed.
- Climbable surfaces carry `Wall_Climb` or `AirWallForClimb` in their `ShapeParam` `MaterialPresets`; when the shape is a `.phsh` mesh the material is not in the params.
- The game lays one-sided climbing nets as back-to-back pairs.

## MR Drafts

### Initial MR Draft

**Title:** New Difficulty > Climbable drawbridge chains Feature: climb the castle drawbridge chains

As a child, while the Hyrule Castle drawbridge is down, each of its two chains carries a strip of the game's own climbable wire net (the Spirit Temple's), so Link can climb from the bridge to the gatehouse. The nets vanish with the bridge at night. Default off; data edits only (48 placed actors in the child field map).

For details about implementation and testing, see difficulty-mod/docs/features/43-drawbridge-chains.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
