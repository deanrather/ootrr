# 52 Enemy feints (Stalfos)

Setting `enemy_feints` (number 166), Difficulty > Enemies, "Enemy feints": Off (default) / 25% / 50%. Prototype, not played in the game yet.

## Request

The user approved a feint prototype on 23 Nov: enemies sometimes fake an attack, souls-like, so that a panic dodge at the first sight of a wind-up gets punished but an attentive player can still read the real swing. Start with the Stalfos. Design numbers come from the user's timing reference (a wind-up that tempts a roll, a hold of about 0.4 to 1.5 s, then a true tell of at least about 20 frames before the hit, delays on at most 20 to 30% of attacks).

## Interpretation

- Only the Stalfos for now, and only its basic sword attack.
- On each attack, one random draw decides whether this attack is a feint. A feint plays the start of the normal swing for 18 frames, then the Stalfos stands in its `Wait` pose for 30 frames plus a random 0 to 30 more (0.5 to 1 s at about 60 counter ticks a second), then the normal attack runs from the start, unchanged. So the real swing still shows its whole vanilla wind-up as the true tell, and its own recovery.
- At most one feint per attack: the draw is once per attack, and the real attack that follows is the vanilla one with no draw of its own.
- Levels: off, 25% and 50% (the user's defaults). The 18 and 30 + 30 frame numbers are constants in `difficulty/feint.rs` (`SWING_FRAMES`, `PAUSE_FRAMES`, `PAUSE_EXTRA`) to tune after the first playtest.

## How it works

Data edits only: one AI graph in the shared AI pack, no code patch, nothing runtime-specific, no new saved flag, no blackboard key, no new crates.

- The Stalfos attack is the module `AI/Stalfos.action.BasicAttack.module.ainb` (27 elements, in `Pack/AI.Global.Product.111.pack.zs`). Its state root (element 9, type 500) runs element 0 on Enter and Update: a Parallel of `ExecuteASPlay "WeaponAttack"` (the whole wind-up, swing and recovery are one animation command), a velocity damper, a hold-event-10 turn toward Link and the end check (when animation hold event 0 ends, `ExecuteGenericSetResultSuccess`).
- The feint adds 12 elements and points the root's Enter and Update at a new Sequential (type 2) of two children: a gate, then the untouched element 0.
  - Gate: a bool switch (type 7) whose input is `QueryGenericGetRandomBool` with `TrueProbability` P, `FalseProbability` 1 - P and `OutputOnlyOnEnter` true (one draw per attack). `IsNoSelectWhenChildBusy` is true, as in the Moblin attack's own switch, so the branch can't flip while a feint plays. False: `ExecuteGenericSetResultSuccess`, so the vanilla attack starts at once.
  - True: a Sequential of two timed stages, each copied from the Moblin Club attack's own "wait for 40 frames" stage (a Parallel with `EndPolicy 0`, `ResultPolicy 1` whose first child is an `ExecuteGenericCounter` with `ResultOnFinish 2`):
    1. counter 18 frames + `ExecuteASPlay "WeaponAttack"` (not busy until finished) + the same velocity damper as element 4;
    2. counter 30 frames with `RandomTimeMax` 30 + `ExecuteASPlay "Wait"`.
  - Then element 0 starts. Its `ExecuteASPlay` has `IsIgnoreSameCommand false`, so `WeaponAttack` restarts from frame 0.
- The draw uses `QueryGenericGetRandomBool` (seen in Dark Link's mirror module) instead of `QueryMathRandomF32` plus `QueryMathLessF32`: it needs no link between two queries, which our node builder has not had proven in the game. The only linked input is the switch's, the shape enemy regen has used since rc 1.
- Who runs it: every graph in the AI pack whose string pool names the module (`Enemy_Stalfos.action.root`, which our parser can't read because it has attachments, and `Stalfos.action.JumpAttack.module`), and every actor whose BSA or AI info names one of those: `Enemy_Stalfos`, `Enemy_Stalfos_AlwaysRespawn`, `Enemy_Stalfos_ForChild`, `Enemy_Stalfos_Weak`. Those four get 0x4000 bytes more actor heap (as regen does). No actor pack carries its own copy of the module. The AI pack and the module's own size-table entries grow through the late stage.
- The edit reads the previous step's output (`difficulty::step`), so it stacks with enemy regen, fairy help and backup, which edit other graphs in the same pack.

## Research results

- `ExecuteASPlay "WeaponAttack"` property `ASCommand` carries flags `0x80000000`: it is bound to the module's first string parameter. `Stalfos.action.JumpAttack.module` element 144 calls this module with `ASCommand = "WeaponAttackChance"` (a follow-up swing after the jump attack). The feint's own swing is hard-coded `WeaponAttack` (flags 0), so after a jump attack a feint shows the start of the plain swing and then the real follow-up runs. We can't tell the two callers apart inside the module without reading that bound value. Possible later: give the feint swing's `ASCommand` the same `0x80000000` binding (a guess about how bound parameters resolve, not tried).
- `Common.ExecuteStopAndPlayAS.module` (the Moblin's way to play `Wait`) is a module call; adding module calls needs the module-call table, which our builder does not write. Plain `ExecuteASPlay "Wait"` is used by 8 vanilla graphs (Deku Baba, Poe, Wolfos run, Lizalfos lock-on wait).
- `QueryMathLessF32`: `F32` is compared against `F32Base` (Armos: distance less than 1). Not used in the end.
- No vanilla `QueryMathRandomF32` in the enemy graphs sets `OutputOnlyOnEnter` true; the property itself is used on other queries (`QueryMathDeg2Rad`).
- The hit is not in the graph: no attack sensor node is in the module, so the sword's damaging collision comes from the animation's own events (`AtCollision` in `AnimationEvent/AsNode/Enemy_Stalfos.root.baev`, not parsed). Cutting `WeaponAttack` at frame 18 is meant to stop before that window; the frame where Link gets hurt has not been measured yet.

## How it was tested

- Unit and game-file tests (`cargo test -p ootrr-core feint`, with `OOTRR_ROMFS` set), all pass:
  - the levels read back, off is the default and keeps the default settings string;
  - the vanilla module rewrites byte for byte; the patched one has 27 + 12 elements, the 27 old element records, parameters, properties, inputs, outputs and query lists unchanged except the root's Enter and Update plugs; the vanilla attack is the second child of the new sequence and has one parent; each feint stage ends on its own counter; no blackboard key; a second patch is refused;
  - every new node has the shape of the game's own node of that kind (the module's own `ExecuteASPlay`, damper, result node, parallel and switch; the Moblin counter, sequence and timed parallel; Dark Link's random bool);
  - the stage changes only the module in the AI pack, keeps its compression dictionary, grows the four Stalfos heaps by 0x4000 and the module's own size-table entry;
  - with enemy regen on as well, both edits are in the pack and the heap growths add up.
- `cargo test -p ootrr-core difficulty` (310 passed), `settings` (113) and `cargo test --workspace` (core 1615 passed, 0 failed) pass.
- Parity: a default build (time savers, open forest, 20 hearts, feints off) from this branch and from main gives the same `.ootrr`, the same spoiler and the same 263 mod files byte for byte. Note for whoever repeats this: build each mod into an empty folder, because the size table a build writes depends on the other mods already in the output folder.
- Built test mods at 25% and 100% hold the 39-element module with `TrueProbability` 0.25 / 1 and `FalseProbability` 0.75 / 0; the off build writes no AI pack at all.
- Not played in the game. Not load-tested.

## How to test it in the game

1. Every map with Stalfos must load with the setting on and its Stalfos must fight (a malformed graph crashes when the enemy loads). Vanilla placements (from `enemy-table.md`): Forest Temple (5 weak), the two Stalfos stages (`StalfosStage`, `StalfosWeakStage`), Gerudo Training Ground (4), Shadow Temple (3), Inside Ganon Castle (2), Crumbling Ganon Castle (2 respawning), Spirit Temple (1 child Stalfos). Builds with enemy gates or more enemies also place Stalfos elsewhere (the Lost Woods and Lon Lon gates, Hyrule Field).
2. Child Temple of Time, `spawn Enemy_Stalfos --dist 5` in front of Link. First on a build with the setting off: note how long `WeaponAttack` lasts and at which `anim_frame` Link's life drops. Then on a build with every attack a feint: `WeaponAttack`, then `Wait` at about frame 18, then `WeaponAttack` again; Link loses no life before the second `WeaponAttack`; the Stalfos never stays in `Wait` and never T-poses.
3. By hand: does it read as a feint, is the pause long enough to bait a dodge, and is the real swing still fair?

The tester plan is `<playtest folder>\feint\PLAN.md` on the author's machine.

## Notes for the next feature

- Lizalfos (`Lizalfos.action.BasicAttack.module`, in the AI pack) and Dinolfos (inside its own pack) have the same shape, with hold event 10 around the turn. The same `build` should port with a different module name and swing command (`JumpWeaponAttack` or `WeaponAttack` for the Lizalfos), once the Stalfos version is seen working.
- A cut tied to the end of hold event 10 (`QueryASGetHoldEvent` gives `CurrentFrame` and `EndFrame`) would carry over between enemies better than a fixed 18 frames, but needs query-to-query links.
- If `Wait` blends badly out of a half-played swing, try `NoGuardWait` or the taunt `Intimidation` (both in the Stalfos AS), or a low `PlayRate` hold instead of a cut (a delayed attack rather than a feint).

## MR Drafts

### Initial MR Draft
**Title:** New Difficulty > Enemy feints Feature: Stalfos sometimes fake a swing

Adds an Enemy feints setting (off, 25%, 50%, default off) on the Difficulty page under Enemies. A Stalfos attack can start with a feint: it starts its swing, stops short, pauses half a second to a second, then attacks for real. Data edits only (one AI graph), not played in the game yet; setting number 166, check for clashes at merge.

For details about implementation and testing, see difficulty-mod/docs/features/52-enemy-feints.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)