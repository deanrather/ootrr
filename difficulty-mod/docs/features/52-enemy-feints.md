# 52 Enemy feints

Setting `enemy_feints` (number 166), Difficulty > Enemies, "Enemy feints": Off (default) / 25% / 50% / 75%. Stalfos, Lizalfos, Dinolfos, Iron Knuckles and Deku Babas. The Stalfos single-swing feint was played by hand (hands-on 16); everything added in the second round is unit-tested and file-tested only.

## Request

- 23 Nov (first round): enemies sometimes fake an attack, souls-like, so that a panic dodge at the first sight of a wind-up gets punished but an attentive player can still read the real swing. Start with the Stalfos. Design numbers come from the user's timing reference (a wind-up that tempts a roll, a hold of about 0.4 to 1.5 s, then a true tell of at least about 20 frames before the hit, delays on at most 20 to 30% of attacks).
- 23 Nov (hands-on 16, after playing the prototype): "i think the feint feature functionally works. let's flesh it out a lot more. add it to every attack on every enemy that we can. come up w/ a nice way of testing it." Also "can we crank up the frequency to 75%?" and "i've only noticed it feint 1 of its 3 moves".
- Same session: "the attack where he swings left, then right. can we make it feint after the first swing but before the 2nd?"

## Interpretation

- One draw per attack decides whether it is a feint. A feint plays the start of that same attack, stops at a fixed frame before the hit (the cut), holds the `Wait` pose for 39 frames plus a random 0 to 39 more (about 0.65 to 1.3 s), then the vanilla attack runs from its first frame, unchanged: its whole wind-up is the true tell and its own recovery is kept.
- Levels: off, 25%, 50%, 75% (75% added in round two, last in the list, so settings strings made before it keep their meaning; off stays the default).
- Every attack that has one readable wind-up in a module our graph builder can edit gets a feint (list below). For attacks with two feint places (the Stalfos left-right-overhead combo, the Iron Knuckle double swing) the chance is split: half the level for a feint before the first swing, half for a stop after the first swing. The first swing's hit is real in the second kind; only the second swing is delayed.
- At most one feint per attack, except the Iron Knuckle double swing: its two draws are separate, so at 75% about 1 in 7 double swings has both.

## What each enemy does now

| Enemy, attack | Module (where) | Fake swing | Cut (frames) | Vanilla hit (frames) | Chance at level P |
|---|---|---|---|---|---|
| Stalfos single swing | `Stalfos.action.BasicAttack.module` (AI pack) | the caller's own command (`WeaponAttackChance`) | 20 | 36 to 52 | P |
| Stalfos combo (left, right, overhead) | `Stalfos.action.ConsecutiveAttack.module` (each Stalfos pack) | the caller's own command (`WeaponTripleAttackChance`) | 20 (before swing 1) or 60 (after swing 1) | 37 to 47, 79 to 91, 185 to 194 | P/2 each |
| Stalfos jump attack | `Stalfos.action.JumpAttack.module` (AI pack) | `JumpAttack_St` (the crouch before the leap) | 12 of its 22 | 6 to 15 frames into `JumpAttack`, the landing | P |
| Lizalfos swing | `Lizalfos.action.BasicAttack.module` (AI pack) | `WeaponAttack` | 15 | not measured | P |
| Dinolfos swing | `Dinolfos.action.BasicAttack.module` (Dinolfos pack) | `WeaponAttack` | 15 | not measured | P |
| Iron Knuckle overhead | `Enemy_IronKnuckle.action.LargeAttack.module` (AI pack) | `WeaponAttack` | 20 | not measured | P |
| Iron Knuckle double swing | `Enemy_IronKnuckle.action.BasicAttack.module` (each Iron Knuckle pack) | `WeaponDoubleAttackFirst` | 20, or a pause between the first and second swing | not measured | P/2 each |
| Deku Baba bite | `Dekubaba.action.basicattack.module` (both Deku Baba packs) | `Attack` | 10 | not measured | P |

Vanilla hit frames are the tester's hands-on-16 measurement (life drops by `anim_frame`, 35 single swings, 54 combo hits, 22 jump landings); the jump attack's hit comes 6 to 15 frames after the landing starts, too fast to cut, so the jump feint is in the crouch, which has no hit. The cuts for the other enemies are short guesses; the harness below measures their hit frames.

Actors that run an edited module and get 0x8000 more actor heap: every Stalfos (`Enemy_Stalfos`, `_AlwaysRespawn`, `_ForChild`, `_Weak`), `Enemy_Lizalfos`, `Enemy_MiddleBoss_Lizalfos`, `Enemy_Dinolfos`, the Iron Knuckles (`_Black`, `_Silver`, `_White`, `Enemy_MiddleBoss_IronKnuckle_Nabooru`), `Enemy_Dekubaba`, `Enemy_Dekubaba_Withered`, and any other actor whose graphs or BSA name one of the edited modules (found by search at build time).

## Not done, and why

- **Graphs with attachments.** 77 graphs in the enemy packs and the AI pack don't parse ("attachments are not supported"), among them every enemy's action root and land-battle action sequence (`Enemy_Stalfos.action.root`, `Enemy_Lizalfos_LandBattle.actionseq.root`, `Enemy_Wolfos.action.root`, `Enemy_Moblin_Club_LandBattle.actionseq.root`, `Enemy_Skulltula.action.root`, the Stalchild sequences). None of the feints needs them (every attack above lives in a module), but any attack chosen inside one of those roots can't be touched. Adding attachment support to the AINB reader and writer is a separate item: the attachment table sits between the parameter and property sections (the header word at 0x18 counts it), so it needs a parse and write of that table, offsets for every section after it, and the byte-for-byte rewrite test over every graph in the game. Estimate: a day for read/write plus a day of tests, before any feature uses it.
- **Wolfos** (`Wolfos.action.BasicAttack.module`, AI pack): parses, but its attack is a run-up then a lunge (`NoWeaponAttack`) inside a two-state machine; a feint at the module start would play the lunge pose from far away. Next step: insert the gated pause in front of the lunge state like the Iron Knuckle gap, with `AttackWait` (its own crouch) as the fake.
- **Club Moblin** (`MoblinClub.action.basicattack.module`, own pack): plays `Attack` through `Common.ExecuteStopAndPlayAS.module` calls deep in a sequence; same next step as the Wolfos.
- **Spear Moblin** (`Moblin.action.RushAttack.module`): a charge, not a swing; a feint would be a stop in the charge. Not tried.
- **Stalchild, Freezard, Gibdo, ReDead, Like Like, Octorok, Keese, Guay, Skulltula, Tektite, Floormaster, Wallmaster, Dodongo, Baby Dodongo, Torch Slug, Shell Blade, Dead Hand, Stinger, Bubble, Biri, Bari, Gohma Larva, Peahat, Beamos**: their attack modules parse (except where noted above) but play through module calls, multi-state machines (grab, dive, spit, roll, charge) or ranged attacks without one readable wind-up. Each needs its own pick of fake command and place. Not tried this round.
- **Bosses and Dark Link, Gerudo fighters**: left out on purpose (bosses' fights are scripted; Dark Link mirrors Link). The Gerudo fighter's `DoubleAttack` module (`Enemy_MiddleBoss_GerudoThief.action.BasicAttack.module`) would take the Iron Knuckle shape if wanted.

## How it works

Data edits only: AI graph edits, no code patch, nothing runtime-specific, no new saved flag, no blackboard key, no new crates. Code: `randomizer/core/src/difficulty/feint.rs` (the setting, the target table, the build stage) and `difficulty/feint/graph.rs` (the node building).

- Each target names its module, the vanilla element count, the state element whose `Update` (and `Enter`, if it runs the same body) starts the attack body, the body, the fake swing, the cut frames and, for the Iron Knuckle, the sequence and position of its gap. The patch refuses a module that doesn't have exactly that shape.
- The state's Enter/Update plugs that start the body now point at a new Sequential: a gate, then the untouched body.
  - Gate: a bool switch (type 7) on `QueryGenericGetRandomBool` (`TrueProbability` the chance, `OutputOnlyOnEnter` true: one draw per attack), `IsNoSelectWhenChildBusy` true. False: `ExecuteGenericSetResultSuccess`.
  - With two cut points, the True branch is a second switch on a 50/50 draw choosing between the two feints.
  - A feint is a Sequential of two timed stages (each a Parallel with `EndPolicy 0`, `ResultPolicy 1` and an `ExecuteGenericCounter` first, copied from the Club Moblin's own wait): the fake swing (counter at the cut, `ExecuteASPlay` not busy until finished, the velocity damper) and the pause (counter 39 + random 39, `ExecuteASPlay "Wait"`).
- **The fake swing plays the real attack's command.** The Stalfos swing and combo modules play `ASCommand` bound to the module's one string parameter: the property's flag word is `0x80000000` (blackboard string 0, `ASCommand`), and the caller fills it (`WeaponAttackChance` for the single swing, `WeaponTripleAttackChance` for the combo). The prototype's fake played a hard-coded `WeaponAttack`, a different swing; that is why the hands-on showed `WeaponAttack -> Wait -> WeaponAttackChance` and only one of the three moves seemed to feint. Now the fake copies the vanilla play's command and its flag word, so it shows the start of the same swing.
- **Mid-combo stop (Stalfos):** the second feint cuts the combo at frame 60, after the first swing's hit (37 to 47) and before the second (79 to 91), pauses, then the full combo plays from its start. The animation set has no command for the second swing alone (`Attack_1`..`Attack_3` are clips inside the one `WeaponTripleAttackChance` state), so "resume at the second swing" isn't possible as a data edit; the player sees swing 1, a pause, then the whole combo.
- **Gap (Iron Knuckle):** the double swing is a Sequential of the first swing (through `Common.ExecuteAnimeDrivenASPlay.module`) and the second swing or miss. A gated pause (the same switch and pause stage) is inserted between them, so the second swing comes late. The first swing's hit is real.
- Every copy of a module in every pack gets the same edit (the Stalfos combo is in all four Stalfos packs, the Iron Knuckle double swing in all four Iron Knuckle packs, the Deku Baba bite in both Deku Baba packs; the tests check every copy is the same vanilla file). The edit reads the previous step's output (`difficulty::step`), so it stacks with regen, fairy help and backup in the shared AI pack. The packs' and the modules' own size-table entries grow through the late stage (tested for every module).

## Research results

- AI counters count 1/60 s steps whatever the frame rate: in hands-on 16 the game ran at about 39 fps, yet the prototype's 18-frame cut ended at `anim_frame` 11 to 17.5 (sampled, 10 cuts), and the 30 + random 30 pause lasted 0.52 to 0.89 s (10 pauses). `anim_frame` also counts in 1/60 s: `WeaponAttackChance` runs to frame 100 in 1.68 s. So the reference numbers (frames at 60 per second) can be used directly. `RandomTimeMax` adds 0 to the maximum (the measured pauses all lie in 0.5 to 1.0 s). No seconds-based timer node exists in the enemy graphs (only frame counters, `QueryGetBaseFramePerSecond` and similar). The tester's earlier 0.3 to 0.77 s reading was from a sparser sampler; the new pause, 39 to 78 frames, matches the reference's 650 ms "elite knight" delay at its short end and stays under 1.5 s.
- Vanilla animation lengths (anim frames, hands-on 16 samples): `JumpAttack_St` 21 to 24, `JumpAttack_Lp` 31 to 36, `JumpAttack` 64 to 71, `WeaponAttackChance` 99 to 104, `WeaponTripleAttackChance` 249 to 253.
- Survey: 2734 graphs in the AI pack and every `Enemy_*` pack; 77 don't parse (attachments). Every enemy has `Wait` in its animation set (checked in the AS string pools of the Stalfos, Lizalfos, Dinolfos, Iron Knuckle, Deku Baba, Moblins and Wolfos).
- The Stalfos combo module already cancels itself when Link guards: while the hit is guarded it ends at the end of hold event 6 (`ExecuteGenericBoolForceResult` on `QueryASGetHoldEvent 6`). Hold 6 is probably the window between swings; a cut tied to it would carry over better than frame 60, but needs a query feeding an action node, not tried.
- The first-round notes still hold: the hit is in the animation events (`AtCollision` in `AnimationEvent/AsNode/*.root.baev`, not parsed), not in the graph; `Common.ExecuteStopAndPlayAS.module` calls need the module-call table, which the builder doesn't write, so fakes are plain `ExecuteASPlay`.

## How it was tested

- Unit and game-file tests (`OOTRR_ROMFS` set), in `difficulty/feint.rs` and `difficulty/feint/graph.rs`:
  - levels read back; 75% is the last choice; off is the default and keeps the default settings string;
  - for each of the 8 modules: every copy in every pack is the same file (1, 4, 1, 1, 1, 1, 4, 2 copies); the vanilla module rewrites byte for byte; the patched one has the expected number of new elements; old element records, parameters (except the state's and the gap sequence's plugs), properties, inputs, outputs and query lists unchanged; the state runs the feint first and keeps its other plugs; the fake swing has the vanilla command and flag word; the cut and pause counters and the draw chances are as listed; each new stage ends on its own counter; the gap sits between the first and second swing; no blackboard key; a second patch is refused;
  - every new node has the shape of a game node of that kind (the Stalfos module's play, damper, result node and switch; the Club Moblin counter, sequence and timed parallel; Dark Link's random bool);
  - the stage edits only attack modules, keeps each pack's compression dictionary, writes the AI pack, the 4 Stalfos packs, the Dinolfos pack, the 4 Iron Knuckle packs and the 2 Deku Baba packs, grows the listed heaps by 0x8000 and every module's own size-table entry; with enemy regen on, both edits are in the AI pack and the heap growths add up.
- `cargo test -p ootrr-core difficulty` (312 passed), `settings` (113 passed) and `cargo test --workspace` (core 1617 passed, 0 failed; every other crate passed).
- Parity: a default build (time savers, open forest, 20 hearts, feints off) from this branch and from upstream main (c139d4e) gives the same `.ootrr`, spoiler and mod files byte for byte (each built into an empty folder).
- In the game: only the first round's Stalfos single swing (hands-on 16: the user saw it feint; scripted runs showed the feint pattern on the single swing and the three load tests of Forest Temple, Gerudo Training Ground and Shadow Temple loaded with Stalfos fighting). Nothing from round two has been played yet.

## How to test it in the game

The tester harness is `<playtest folder>\feint\feint2.py` with `PLAN.md` beside it. Per enemy kind it spawns the enemy 5 units in front of Link in the child Temple of Time, samples its `anim` and `anim_frame` about 12 times a second with `hitguard` on (Link warped back after each attack), then 40 s with `hitguard` off to read hit frames, and classifies every attack episode:

- **pre feint:** attack command, last frame near a cut, then `Wait`, then the attack command again from frame 0 to 6;
- **mid feint (Stalfos combo):** `WeaponTripleAttackChance` cut near frame 60, `Wait`, the combo again;
- **gap (Iron Knuckle):** `WeaponDoubleAttackFirst` played out, `Wait`, then `WeaponDoubleAttackSecond` or `WeaponDoubleAttackMiss`;
- **normal:** the attack with no `Wait` inside; **freeze:** a `Wait` over 2 s after a cut.

It reports per attack kind the feint rate against the level, the cut frames, the pause lengths, any life drop during a cut swing, the vanilla hit frames, and freezes. Mods: `feintoff` (control: same time savers, feints off), `feint75`, `feint100` (a local test build where 75% means every attack; never shipped).

By hand (capture-playbook rows): does each feint read as a feint and does the real attack stay fair; the Stalfos combo stop after the first swing; the Iron Knuckle's late second swing; and a load test of every map with these enemies (Forest Temple, the Stalfos stages, Gerudo Training Ground, Shadow Temple, Inside Ganon's Castle, Spirit Temple for the Iron Knuckles, Dodongo's Cavern for the Lizalfos, Deku Tree for the Deku Babas, Inside Ganon's Castle for the Dinolfos).

## Notes for the next feature

- To add an enemy: find its attack module (parses? one state whose Update runs the body?), pick the vanilla `ExecuteASPlay` of the wind-up (`Swing::Copy`) or name the command (`Swing::Named`) after checking the AS string pool, and add a `Target`. For a pause between two parts of an attack use `gap: Some((sequence, position))`.
- A bound command (flag word `0x80000000` on a string property) is how modules take parameters from their caller; copy the flag word to play what the caller asked for.
- If `Wait` blends badly out of a half-played swing, try `NoGuardWait` or the taunt `Intimidation` (both in the Stalfos AS).

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
