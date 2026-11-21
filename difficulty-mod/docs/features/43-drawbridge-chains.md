# 43. Climbable drawbridge chains

Setting `climbable_drawbridge_chains` (number 158), toggle, default off. Difficulty page, section "Just for fun" (next to the Spawn enemies combo). Branch `feature/drawbridge-chains`, worktree `wt\chains`, module `difficulty/chains.rs`. **Status: built and unit tested; NOT confirmed in the game. The ramp and the lightning trigger are unproven (see Tested).**

## What was asked (user, 21 Nov)

A just-for-fun feature. The Hyrule Castle Town drawbridge (the field entrance to the Market) has two big chains from the bridge up to the gatehouse. Add an invisible solid surface along each chain so players can carefully walk up it to the top. At the top they are struck by lightning and knocked into the moat. The menu says only "Climbable drawbridge chains" and the tooltip does not mention the lightning: it is a surprise for players. This document and the pull request describe exactly what the setting does, so nothing is hidden from the maintainers.

## What the setting does (all of it)

When the setting is on, in the child Hyrule Field map only (`HyruleField_Child`):

1. **Invisible ramps.** On each of the two chains of the lowered drawbridge (x = -3.73 and +3.73 from the bridge's centre line) a row of 10 invisible solid boxes (the game's `FldObj_AirWall_Player`, a box that only the player collides with, 0.9 wide and 0.5 thick) follows the chain from the end of the bridge deck up to the top of the gatehouse. Slope 27 degrees at the bottom to 50 degrees at the top, the same line as the chain links. They are narrow on purpose: a player has to keep on the chain.
2. **Only while the bridge is down.** The boxes carry the same presence flag as the lowered bridge model (`IsHyruleFieldBridgeClose` false), so they exist only while the bridge is lowered (by day, and at night while the flag is off) and disappear when the bridge is raised.
3. **Lightning at the top.** At the top of each chain there is a trigger box (an `Area` on the player layer) and a Phantom Ganon lightning bolt actor (`PhantomGanon_Bullet_Thunder`) wired to it through a new logic graph. When Link stands in the box, the bolt is created. The bolt hurts like the Phantom Ganon's lightning (attack power 4, one heart at normal damage), so it cannot kill from full health, and its knock-back throws Link off the chain, outward, towards the moat.
4. **The moat.** Link lands in the water beside the bridge. The moat is deep enough to swim in; the game's own scene throws the Ocarina of Time into it.

Nothing else changes: not the bridge, its model or collision, not the gatehouse, the Market, the adult castle (the adult drawbridge is broken and has no chains, so nothing is added there), no events, flows or flags. Setting off writes nothing. Default settings string and seeds are unchanged.

## What it does not do

- No lightning anywhere else, no lightning when the setting is off, no damage other than the single bolt at the top.
- No change to the lowered or raised state of the bridge.

## How it is built

Data only, nothing runtime-specific (no code patch).

- Geometry from the game's own files. The lowered bridge is the actor `Land_HrlField_DrawBridge_A_01` at (0.0, 150.35, 34.02) in `HyruleField_Child_Static`. Its model `Model/Land_HrlField_DrawBridge_A.Land_HrlField_DrawBridge_A_01.bfres.zs` holds two chains as one vertex buffer of 37888 vertices; read at the model's own scale, the chain centre line is at x = +-3.73 and runs from local (z 11.2, y 0.45) at the end of the deck to (z -2.4, y 11.4) at the gatehouse, sagging a little (`PROFILE` in `chains.rs`, eleven points taken from that buffer). Map coordinates are the actor position plus the local ones.
- The ramp pieces are `FldObj_AirWall_Player` actors (a 4 x 4 x 4 box that only the player hits, scaled and rotated by the map actor), one per segment of the profile, pitched to the segment's slope, with the top face on the chain line.
- Each ramp piece copies the `Presence` property of the lowered bridge actor, so it appears and disappears with the bridge.
- The top: two `Area` actors and two bolt actors, grouped in a new `AiGroups` entry whose logic module is a graph built from the gates graph (`Logic/Gimmick_ChainTop_<tag>.logic.module.ainb`): Area, player sensor, AND ("inside"), and the bolt node with `Logic_Create` and `RespawnTrigger` linked to "inside". The bolt sits 1.6 units inward of the chain so that its knock-back pushes outward.
- Setting: `climbable_drawbridge_chains`, number 158 (152, 153, 155, 156 and 157 are taken, 154 is night lightning on its own branch), label "Climbable drawbridge chains", tooltip "The drawbridge chains can be climbed. Carefully." The tooltip is deliberately plain: the lightning is the surprise. It is described in the build report (`Opts::notes`), in this document and in the pull request.

## Tested

- `cargo test -p ootrr-core difficulty` (302) and `settings` (113) pass, plus six new tests in `chains.rs` (profile, piece geometry, names, graph audit, written files, presence copied).
- In the game (21 Nov, one child save, noon, 3 hearts): the mod loads and Hyrule Field (child) loads; the lowered bridge and chains are there. With the first build (`FldObj_AirWall_Common_Cube`) Link walked along the chain line at deck height and was not blocked: that box does not stop the player. The `FldObj_AirWall_Player` version, and the pitch sign of the pieces, were built but not played: the test copy had to be closed because more than two game copies were running.
- Not tested at all: climbing, the lightning at the top (note: the night lightning branch found that a bolt created from logic did not appear on cue, so this trigger may not work either), landing in the moat, swimming out.
- The adult castle has a broken bridge and no chains, so nothing was built for it.

## How to test it in the game

Turn the setting on. As a child, by day, go to the Hyrule Field entrance of the castle town (`warp 0 151 50` is the bridge's south end), walk along the bridge edge and up a chain. Expect: the ramp holds you if you stay on the chain, you reach the gatehouse top, a green bolt strikes, you lose one heart and are thrown into the moat.
