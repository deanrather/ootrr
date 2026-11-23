# 52 Enemy feints

Setting `enemy_feints` (number 166), Difficulty > Enemies, "Enemy feints": Off (default) / 25% / 50% / 75%. Stalfos, Lizalfos, Dinolfos, Iron Knuckles and Deku Babas. Round 3 (this version) replaces the round 1 and 2 "cut the swing, idle, start again" feint with a held wind-up: the enemy winds up, rocks back and forth around its ready pose (a "boomerang" of the wind-up), then strikes from there. A build-time switch gives a plain freeze instead, for comparison. Round 3 is unit-tested and file-tested only; nothing of it has been played yet. Round 1's Stalfos single-swing feint was played by hand (hands-on 16).

## Request

- 23 Nov (first round): enemies sometimes fake an attack, souls-like, so that a panic dodge at the first sight of a wind-up gets punished but an attentive player can still read the real swing. Start with the Stalfos. Design numbers come from the user's timing reference (a wind-up that tempts a roll, a hold of about 0.4 to 1.5 s, then a true tell of at least about 20 frames before the hit, delays on at most 20 to 30% of attacks).
- 23 Nov (hands-on 16, after playing the prototype): "i think the feint feature functionally works. let's flesh it out a lot more. add it to every attack on every enemy that we can. come up w/ a nice way of testing it." Also "can we crank up the frequency to 75%?" and "i've only noticed it feint 1 of its 3 moves".
- Same session: "the attack where he swings left, then right. can we make it feint after the first swing but before the 2nd?"
- 23 Nov (round 3, after playing round 1): "the current behavior where it cancels the swing animation is ok, it's certainly better than nothing and passable. but ideally, it would: perform the 'pull-back' part of the attack; hold in the 'ready to attack' stance for some time; perform the forward-part of the attack. that would flow much nicer, so see if we can make it behave like that."
- Same day, a clarification: "we want the wind-up part of the animation (eg, the arms and upper body) to pause or slow down, but we don't want the entire model (legs, movement, etc) to pause or slow down. so I'm not sure if it's possible. but try."
- Same day, another idea: "rather than pausing the animation then resuming, maybe it would make more sense to 'boomerang' the animation a bit, so it still looks like they're stumbling in-place."

## Interpretation

- One draw per attack decides whether it is delayed. A delayed attack plays its own animation from the start; at a fixed hold point (the end of the wind-up, before the hit) it holds for 39 frames plus a random 0 to 39 more (about 0.65 to 1.3 s), then the same animation plays on to the strike and its own recovery. Nothing restarts and no other animation is played.
- **The hold is a boomerang by default:** the animation plays backwards at half speed for 8 frames (4 animation frames back into the wind-up), then forwards at half speed for 8, and repeats until the hold ends, so the enemy rocks in its ready pose instead of turning to stone. It only ever goes back from the hold point, never forward past it, so it never comes closer to the hit. Built with the environment variable `OOTRR_FEINT_FREEZE=1`, the hold is a plain freeze (rate 0) instead, for comparison; the shipped default is the boomerang.
- **The whole body takes part, legs included.** The upper-body-only hold the user asked for is not possible as a data edit for these enemies (see "Upper body only" below). What does keep moving during the hold: the enemy's physics and turning. The attack's own tracking keeps running, so if the hold point falls in the attack's turn-to-target window, the frozen enemy keeps turning to face Link.
- Levels: off, 25%, 50%, 75% (75% is last in the list, so settings strings made before it keep their meaning; off stays the default).
- For attacks with two hold points the chance is split: the Stalfos left-right-overhead combo holds either at the end of the first wind-up or after the first swing, before the second (P/2 each); the Iron Knuckle double swing holds before its first swing (P/2) and, by a separate draw, at the start of its second swing (P/2), so at 75% about 1 in 7 double swings has both.

## What each enemy does now

| Enemy, attack | Module (where) | Held animation | Hold point (frames from the attack's start) | Vanilla hit (anim frames) | Chance at level P |
|---|---|---|---|---|---|
| Stalfos single swing | `Stalfos.action.BasicAttack.module` (AI pack) | the caller's command (`WeaponAttackChance` and others) | 28 | 36 to 52 | P |
| Stalfos combo (left, right, overhead) | `Stalfos.action.ConsecutiveAttack.module` (each Stalfos pack) | `WeaponTripleAttackChance` | 28 (before swing 1) or 70 (after swing 1, before swing 2) | 37 to 47, 79 to 91, 185 to 194 | P/2 each |
| Stalfos jump attack | `Stalfos.action.JumpAttack.module` (AI pack) | `JumpAttack_St` (the crouch before the leap) | 12 of its 22 | 6 to 15 frames into `JumpAttack`, the landing | P |
| Lizalfos swing | `Lizalfos.action.BasicAttack.module` (AI pack) | `WeaponAttack` or `JumpWeaponAttack` | 28 | 41 to 43 | P |
| Dinolfos swing | `Dinolfos.action.BasicAttack.module` (Dinolfos pack) | `WeaponAttack` | 26 | 36 to 53 | P |
| Iron Knuckle overhead | `Enemy_IronKnuckle.action.LargeAttack.module` (AI pack) | `WeaponAttack` | 20 | not measured | P |
| Iron Knuckle double swing | `Enemy_IronKnuckle.action.BasicAttack.module` (each Iron Knuckle pack) | `WeaponDoubleAttackFirst`, and `WeaponDoubleAttackSecond`/`Miss` | 20 into the first swing; 10 into the second | not measured | P/2 each, separate draws |
| Deku Baba bite | `Dekubaba.action.basicattack.module` (both Deku Baba packs) | `Attack` | 10 | not measured | P |

Hold points count AI frames (1/60 s) from the moment the attack body starts; the animation lags that count by up to about 7 frames (round 1: an 18-frame counter ended at `anim_frame` 11 to 17.5). The Stalfos points are set from the tester's hands-on-16 hit frames: 28 is at least 8 frames before the first hit (36), 70 is after swing 1's last hit (47) and at least 9 before swing 2's first (79). The Lizalfos (hits at 41 to 43) and Dinolfos (36 to 53) points come from the round 2 tester run: 13 and 10 frames before the first hit. The Iron Knuckle and Deku Baba keep round 2's short, safe guesses (their hits are not measured yet); the harness reports their real hold frames and hit frames, and the constants are one line each in `feint.rs` (`*_HOLD`).

Actors that run an edited module and get 0x8000 more actor heap: every Stalfos (`Enemy_Stalfos`, `_AlwaysRespawn`, `_ForChild`, `_Weak`), `Enemy_Lizalfos`, `Enemy_MiddleBoss_Lizalfos`, `Enemy_Dinolfos`, the Iron Knuckles (`_Black`, `_Silver`, `_White`, `Enemy_MiddleBoss_IronKnuckle_Nabooru`), `Enemy_Dekubaba`, `Enemy_Dekubaba_Withered`, and any other actor whose graphs or BSA name one of the edited modules (found by search at build time).

## Not done, and why

- **Upper body only (the 23 Nov clarification).** Investigated; not possible as a data edit for these attacks. Findings:
  - The game does have upper-body layers: the AS files of the Stalfos, Lizalfos and Dinolfos define a partial named `Upper` (built from `Spine_1`/`Spine_2`), and the game plays upper-body commands on it while the legs keep moving (`Stalfos.action.InterruptGuard.module` plays `MoveShock`/`MoveShockQuick` with `Partial "Upper"`; `Lizalfos.action.Cower.module` plays `BeforeEscapeUpper` on `Upper` over a whole-body `BeforeEscape`; `QueryASSearchPartialSlot "Upper"` finds the layer's slot, `OneShotASRemove` removes it). `ExecuteSetASRate` takes a slot (`ASSlot`), so a partial layer's rate can be set on its own. The Iron Knuckle and Deku Baba AS files have no upper partial (`Material_*` and `Body` only).
  - The blocker: the attack itself is one whole-body command in slot 0, and the attack module ends on that command's own animation events (the Stalfos swing ends when hold event 0 ends; the hit is the command's `AtCollision` event). An upper-only hold needs the lower body to do something else while the upper body stays at the hold frame. If slot 0 keeps playing the attack, the strike, its root motion and its hit happen during the "hold". If slot 0 switches to `Wait` or walking, the attack command is gone and the module ends (no strike, the round 2 behaviour). If slot 0 is frozen or rocked, the legs freeze or rock with it, which is what we ship (the boomerang, or the freeze build). No enemy AS file here has a `Lower` partial that could overlay idle legs on a frozen slot 0 (the game's 13 `Lower` uses are NPCs and others). The one untried route: overlay `Wait` on the `WholeBody` partial (in the Stalfos AS, used by no graph) over a frozen slot 0, then the attack on `Upper` pinned at the hold frame (`ExecuteASPlaySetASFrameEachFrameByPlug` with `ASFrame` and `Partial`). Whether an attack command plays on `Upper` at all, and how the layers stack, can only be seen in the game; it is a test-build experiment for the Stalfos, not something to ship blind.
- **Graphs with attachments.** 77 graphs in the enemy packs and the AI pack don't parse ("attachments are not supported"), among them every enemy's action root and land-battle action sequence (`Enemy_Stalfos.action.root`, `Enemy_Lizalfos_LandBattle.actionseq.root`, `Enemy_Wolfos.action.root`, `Enemy_Moblin_Club_LandBattle.actionseq.root`, `Enemy_Skulltula.action.root`, the Stalchild sequences). None of the feints needs them, but any attack chosen inside one of those roots can't be touched. Adding attachment support to the AINB reader and writer is a separate item (about a day for read/write plus a day of tests).
- **Wolfos** (`Wolfos.action.BasicAttack.module`): a run-up then a lunge (`NoWeaponAttack`) inside a two-state machine; next step is a hold wrapped around the lunge state, like the Iron Knuckle's second swing.
- **Club Moblin** (`MoblinClub.action.basicattack.module`): plays `Attack` through `Common.ExecuteStopAndPlayAS.module` calls deep in a sequence; same next step.
- **Spear Moblin** (`Moblin.action.RushAttack.module`): a charge, not a swing. Not tried.
- **Stalchild, Freezard, Gibdo, ReDead, Like Like, Octorok, Keese, Guay, Skulltula, Tektite, Floormaster, Wallmaster, Dodongo, Baby Dodongo, Torch Slug, Shell Blade, Dead Hand, Stinger, Bubble, Biri, Bari, Gohma Larva, Peahat, Beamos**: no one readable wind-up in a parseable module. Not tried.
- **Bosses and Dark Link, Gerudo fighters**: left out on purpose.

## How it works

Data edits only: AI graph edits, no code patch, nothing runtime-specific, no new saved flag, no blackboard key, no new crates. Code: `randomizer/core/src/difficulty/feint.rs` (the setting, the target table, the build stage) and `difficulty/feint/graph.rs` (the node building).

- Each target names its module, the vanilla element count, the state element whose `Update` (and `Enter`, if it runs the same body) starts the attack body, the body, the hold points and, for the Iron Knuckle, the sequence, the position of its second swing and that swing's hold point. The patch refuses a module that doesn't have exactly that shape.
- The state's Enter/Update plugs that start the body now point at a bool switch (type 7, `IsNoSelectWhenChildBusy` true) on `QueryGenericGetRandomBool` (`TrueProbability` the chance, `OutputOnlyOnEnter` true: one draw per attack). False: the untouched vanilla body. True: the body wrapped with a hold (with two hold points, a second 50/50 switch picks the wrapper).
- **The wrapper** is a Parallel (`EndPolicy 0`, `ResultPolicy 1`: the first child decides when it ends and what it returns, as in the Club Moblin's own timed wait) of [the vanilla body, a timing branch]. The timing branch is a Sequential of: an `ExecuteGenericCounter` up to the hold point; a hold stage (a Parallel `EndPolicy 0`, `ResultPolicy 1` of a counter of 39 + random 39 frames and the pose); then `ExecuteGenericSetResultBusy`, so the branch idles and never ends or restarts while the attack plays out.
  - Boomerang pose (default): a looping Sequential (`IsLoop` true, `IsUpdateNextInFrame` true, copied from `Redead.MinimumPerceptionSetTarget.module`) of two timed steps, each a Parallel (`ResultPolicy 1`) of an 8-frame counter and `ExecuteSetASRate` on slot 0 with `IsUndoOnLeave` true: `ASRate -0.5`, then `ASRate 0.5`.
  - Freeze pose (`OOTRR_FEINT_FREEZE=1` at build time): one `ExecuteSetASRate` with `ASRate 0`.
  - When the hold counter ends, the pose leaves and the game restores the play rate itself (`IsUndoOnLeave`), so the attack carries on from where the hold left it. The body runs the whole time, so its own hit raycast, turning, damping and end condition (its animation events) all stay vanilla; with the animation frozen, those events simply wait.
- **A negative rate plays backwards:** the game's own doors close by playing their opening animation at `ASRate -1` (`_LocalModule_e4e2d47532fa.module`, called from `Door.root`, `FieldDoor.event.root` and `PrivateDoorForNpc_Single.root`: `ExecuteSetASRate` fed by `QuerySelectF32ByBool` with `A = -1`). That is the boomerang's backward step.
- **Where the freeze comes from:** the Stalfos's own paralysis (`Stalfos.action.InterruptParalyze.module`, element 32): `ExecuteSetASRate` with `IsUndoOnLeave` true, `ASSlot` 0 (default), `ASRate` 0, beside `ExecuteGenericSetResultBusy`. That is how the game freezes a stunned Stalfos mid-animation; the same node with rate 0 appears in 12 other stun, frozen and light-arrow modules (`EnemyCommon.action.interruptboomerang`, `interrupthookshot`, `interruptlightarrow`, `Bokoblin.action.interruptfrozen` and others). The feint copies its shape exactly (tested).
- **Iron Knuckle double swing:** its body is a Sequential of [the first swing (`Common.ExecuteAnimeDrivenASPlay.module` `WeaponDoubleAttackFirst`), a switch choosing the second swing or the miss]. The second position is replaced by a gated wrapper around that same switch, so the hold falls 10 frames into whichever second part it plays. The first swing's hit is real.
- **No fake command any more.** Round 2 played the attack's command itself to show a wind-up (copying the caller's bound `ASCommand`, flag word `0x80000000`). Round 3 plays nothing new: it holds whatever the vanilla body plays, so the Stalfos swing module, called with `WeaponAttackChance` or another command, always holds the right swing.
- Every copy of a module in every pack gets the same edit (the tests check every copy is the same vanilla file). The edit reads the previous step's output (`difficulty::step`), so it stacks with regen, fairy help and backup in the shared AI pack. The packs' and modules' own size-table entries grow through the late stage (tested for every module).

## Research results

- **Animation rate nodes in the game's node definitions** (`AI/NodeDefinition/Node.Product.111.aidefn.byml.zs`): `ExecuteSetASRate` (`ASRate` f32 default 1, `ASSlot` s32 default 0; the graphs add `IsUndoOnLeave`), `ExecuteASPlayByPlug` / `ExecuteASPlaySetASFrameEachFrameByPlug` (`PlayRate`, `RestorePlayRateOnLeave`, `ASFrame`, `Partial`), `ExecuteASSetAndSetFrame` (`SetFrame`), `OneShotASSetFrame` (`ASFrame`, `Partial`), `QueryASGetCurrentFrame`, `QueryASSearchPartialSlot`. Uses of `ExecuteSetASRate` in the enemy packs and the AI pack: 16 with slot 0, 1 with slot 1 (constant); 12 with rate 0; 15 with `IsUndoOnLeave` true.
- **Round 2 tester run (hands-off, `feint100`/`feint75`/`feintoff`, trace `trace2.jsonl`):** Stalfos PASS on every attack (swing 8/8 at 100%, 10/13 at 75%; combo 8/8 and 13/15 with 4 of 8 stops after the first swing; jump 5/5 and 4/4; no hit during a cut; pauses 0.69 to 1.32 s). The Lizalfos (5/110), Dinolfos (30/55) and Deku Baba (20/31) rates the report printed at 100% were the classifier, not the edit: counting the cut episodes directly in the same trace gives 58 cuts for 57 full swings (Lizalfos), 35 for 41 (Dinolfos) and 25 for 26 (Deku Baba) at 100%, 76/96, 58/80 and 25/40 at 75%, and 0, 2 and 0 with feints off. The Lizalfos `Wait` animation is about 38 frames, shorter than the pause, so it looped and split the pause into two episodes; the Dinolfos had `Run` and the Deku Baba `Turn`/`AttackSuccess` between the parts. The Dinolfos's "freeze" (2.28 s) and 1.62 s pause were such splits. The spawned Iron Knuckles sat (`Sit`) and never attacked. Measured vanilla hits: Lizalfos 41 to 43, Dinolfos 36 to 53.
- **Partials in the enemy AS files** (string pools): Stalfos `Upper`, `WholeBody` (bones `Spine_1`, `Spine_2`); Lizalfos and Dinolfos `Upper`, `WhileAttacking`; Iron Knuckle `Material_*` only; Deku Baba `Body`. Partial names used by graphs in these packs: `Lower` 13, `Damage` 10, `Upper` 8, `Material` 6, `Body_Only` 5, `AnimeDriven` 5, others 1 to 4.
- **The jump attack body** (`Stalfos.action.JumpAttack.module` element 69) is a Sequential whose first step is `Common.ExecuteStopAndPlayAS.module "JumpAttack_St"` (the crouch); the leap and its timers come after it, so a hold 12 frames in falls inside the crouch and the AI's own jump timers (a 30-frame counter in the air) have not started yet.
- AI counters count 1/60 s steps whatever the frame rate (hands-on 16: the game ran at about 39 fps, yet an 18-frame cut ended at `anim_frame` 11 to 17.5, and a 30 + random 30 pause lasted 0.52 to 0.89 s). `anim_frame` also counts in 1/60 s. `RandomTimeMax` adds 0 to the maximum.
- Vanilla animation lengths (anim frames, hands-on 16 samples): `JumpAttack_St` 21 to 24, `JumpAttack_Lp` 31 to 36, `JumpAttack` 64 to 71, `WeaponAttackChance` 99 to 104, `WeaponTripleAttackChance` 249 to 253.
- The hit is in the animation events (`AtCollision` in `AnimationEvent/AsNode/*.root.baev`, not parsed), not in the graph. The Stalfos combo cancels itself when Link guards (hold event 6).
- Survey: 2734 graphs in the AI pack and every `Enemy_*` pack; 77 don't parse (attachments).

## How it was tested

- Unit and game-file tests (`OOTRR_ROMFS` set), in `difficulty/feint.rs` and `difficulty/feint/graph.rs`:
  - levels read back; 75% is the last choice; off is the default and keeps the default settings string;
  - for each of the 8 modules: every copy in every pack is the same file (1, 4, 1, 1, 1, 1, 4, 2 copies); the vanilla module rewrites byte for byte and sets no play rate; the patched one has the expected number of new elements; old element records, parameters (except the state's and the Iron Knuckle sequence's plugs), properties, inputs, outputs and query lists unchanged; no new animation play; the state enters the draw; False runs the vanilla body; True runs the body inside the wrapper as its first child, beside a counter to the hold point, the hold (counter 39 + 39, rate 0 on slot 0, undo on leave) and a busy end; the Iron Knuckle's second part is wrapped the same way behind its own draw; draw chances as listed; no blackboard key; a second patch is refused;
  - every new node has the shape of a game node of that kind (the rate node and busy result from the paralysed Stalfos; the Club Moblin counter, sequence and timed parallel; Dark Link's random bool; the Stalfos module's switch); the paralysis module holds its frozen pose beside a busy result;
  - the Stalfos hold points sit before the measured hits (with a 7-frame lag allowance) and the mid-combo point between the first and second hits;
  - the stage edits only attack modules, keeps each pack's compression dictionary, writes the AI pack, the 4 Stalfos packs, the Dinolfos pack, the 4 Iron Knuckle packs and the 2 Deku Baba packs, grows the listed heaps by 0x8000 and every module's own size-table entry; with enemy regen on, both edits are in the AI pack and the heap growths add up.
- Results (round 3, boomerang default; every module test also runs the freeze build): `cargo test -p ootrr-core difficulty` 312 passed, `settings` 113 passed, `cargo test --workspace` core 1617 passed, 0 failed, every other crate passed. Audit: FAIL 0 (randomizer: 8 WARN, all in files this branch doesn't touch; difficulty-mod: 0 WARN).
- Parity: a default build (time savers, open forest, 20 hearts) from this branch and from upstream main (c139d4e) gives the same `.ootrr`, spoiler and 257 mod files byte for byte, each built into an empty folder.
- The round 3 harness classifier, run on the round 2 trace, finds 0 holds in every control and round-2 run (no false positives on vanilla swings).
- In the game: nothing from round 3 yet. Round 1's Stalfos single-swing feint was played by hand (hands-on 16).

## How to test it in the game

The tester harness is `<playtest folder>\feint\feint3.py` with `PLAN.md` beside it. Per enemy kind it puts the enemy in front of Link in the child Temple of Time, samples its `anim`, `anim_frame` and `anim_rate` about 12 times a second with `hitguard` on, then with `hitguard` off to read hit frames, and classifies every attack:

- **hold:** the attack command from frame 0 to 8, `anim_frame` staying inside a 6.5-frame window (rocking back and forth for the boomerang, static for the freeze; `anim_rate` at or below 0 at some point) for 0.45 to 1.6 s near the hold point, then the same command playing on past it;
- **mid (Stalfos combo):** the same, held near frame 70; **gap (Iron Knuckle):** a hold 10 frames into `WeaponDoubleAttackSecond`/`Miss`;
- **normal:** no hold; **freeze:** held over 2 s and never moving on; **cut:** round 2's pattern (means a wrong mod).

The Iron Knuckle a script spawns doesn't attack, so the Iron Knuckle runs use a test build that places the aggressive special-encounter Iron Knuckle in the child Temple of Time (see `PLAN.md`). Mods: `feintoff` (control: same time savers, feints off), `feint75`, `feint100` (a local test build where 75% means every attack; never shipped), and for the Iron Knuckle only `feint100ik` and its control `feintoffik` (the same test build with the temple Iron Knuckle placed by a build-time switch, `OOTRR_FEINT_KNUCKLE=1`, which exists only in that local test commit).

By hand (capture-playbook rows): does the boomerang read as "stumbling in place" and the strike after it feel fair, compared with the freeze build; the legs taking part (the user's question: is a whole-body hold acceptable); the Stalfos hold between the first and second combo swings; the Iron Knuckle's held second swing; and a load test of every map with these enemies (Forest Temple, Gerudo Training Ground, Shadow Temple, Inside Ganon's Castle, Spirit Temple, Dodongo's Cavern, Deku Tree).

## Notes for the next feature

- To hold any animation mid-play: wrap the node that plays it in a Parallel (`ResultPolicy 1`, it first) beside a Sequential [counter to the frame, Parallel(`ResultPolicy 1`) [counter for the hold, the pose: `ExecuteSetASRate` rate 0 undo-on-leave, or a looping back-and-forth of two rate steps], `ExecuteGenericSetResultBusy`]. Nothing about the played node changes.
- When a harness classifies by animation episodes, an idle animation shorter than the pause loops and splits the pause; count the attack's own episodes directly as a cross-check.
- Upper-body layers exist (`Partial "Upper"`, `QueryASSearchPartialSlot`, `OneShotASRemove`), but only for commands made for them; see "Upper body only" above.
- To add an enemy: find its attack module (parses? one state whose Update runs the body?), pick the hold point from measured hit frames, and add a `Target`. For a hold inside a later part of an attack, use `gap: Some((sequence, position, hold))`.

## MR Drafts

### Initial MR Draft
**Title:** New Difficulty > Enemy feints Feature: Stalfos sometimes fake a swing

Adds an Enemy feints setting (off, 25%, 50%, default off) on the Difficulty page under Enemies. A Stalfos attack can start with a feint: it starts its swing, stops short, pauses half a second to a second, then attacks for real. Data edits only (one AI graph), not played in the game yet; setting number 166, check for clashes at merge.

For details about implementation and testing, see difficulty-mod/docs/features/52-enemy-feints.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

### 2nd MR Draft
**Title:** New Difficulty > Enemy feints Feature: enemies sometimes fake an attack

Adds an Enemy feints setting (off, 25%, 50%, 75%, default off) on the Difficulty page under Enemies. Stalfos (single swing, combo, jump attack), Lizalfos, Dinolfos, Iron Knuckles and Deku Babas sometimes start an attack, stop short, pause for about two thirds of a second to a second and a third, then attack for real; a Stalfos combo or an Iron Knuckle double swing can instead pause after its first swing. Data edits only (8 AI modules). The Stalfos single-swing feint has been played; the rest is tested by unit and game-file tests only. Setting number 166: check for clashes at merge.

For details about implementation and testing, see difficulty-mod/docs/features/52-enemy-feints.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

### 3rd MR Draft
**Title:** New Difficulty > Enemy feints Feature: enemies sometimes hold their wind-up before striking

Adds an Enemy feints setting (off, 25%, 50%, 75%, default off) on the Difficulty page under Enemies. Stalfos (single swing, combo, jump attack), Lizalfos, Dinolfos, Iron Knuckles and Deku Babas sometimes wind up an attack, rock back and forth in the ready pose for about two thirds of a second to a second and a third, then strike from there; a Stalfos combo can hold before its second swing, and an Iron Knuckle before its second swing. The hold sets the animation's play rate with the game's own node (`ExecuteSetASRate`, as its paralysis and doors do), so the whole body takes part. Data edits only (8 AI modules); unit- and file-tested, not yet played in this form. Setting number 166: check for clashes at merge.

For details about implementation and testing, see difficulty-mod/docs/features/52-enemy-feints.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
