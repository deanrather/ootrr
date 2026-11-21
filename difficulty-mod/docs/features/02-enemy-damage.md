# Enemy damage multiplier

Setting `enemy_damage` (number 111), slider 0.5x to 10x. Code: `crates/core/src/difficulty/stats.rs` (`attack`, `is_enemy_attack`).

## Request
"enemy damage multiplier (from 0.5x to 5x)", later raised: "the top to 10x" (21 Nov)

## Interpretation
Scale the damage Link takes from enemy attacks and enemy projectiles. Not traps, lava or falls.

## Investigation
- Each attacker has `GameBalance/AttackParam/<Name>.game__gamebalance__AttackParam.bgyml` with `BaseAttackPower` (a whole number) and a list of attacks, each with a `DamageMultiplier` (a decimal). A Stalfos is base 2 with attacks at 2x and 4x.
- 180 such files exist. Many are not enemies: the player, items, pots, rolling rocks, fire walls.
- Projectiles are separate actors with their own file (`Twinrova_Bullet_FireBeam`, `BeamosBeam`, `Octorok_Bullet`).
- Some enemies have base 0 (ReDead, Wallmaster, Like Like): their grab damage is not in this file.

## Implementation
- **Which files:** names starting `Enemy_` or `DungeonBoss`, names containing `_Bullet`, and `Keese`, `BeamosBeam`, `ScrubDeku_DekuNut`. Names containing `MirrorShield` are excluded, because those are Link's reflected beams.
- **How:** scale the whole-number base and round it, then put the rounding remainder into the decimal multipliers, so the product is exact. I did this because I do not know whether the game rounds the product; this way nothing is lost when the result is a whole number.

## How I tested it
- Unit tests for the name filter and the exact-product rule.
- The all-packs diff after a real build: 187 base values and 52 multipliers changed, all in enemy and projectile files.

## In the game
Never played. To test:
- Set 5x, take one hit from a Keese or Deku Baba at full health, and compare hearts lost with vanilla.
- Check a projectile separately (Octorok rock, Deku Scrub nut).
- Set 0.5x and check a weak hit still does some damage. If weak enemies do zero damage at 0.5x, the game truncates and I need to floor at the vanilla value.
- Check that a ReDead or Like Like grab is unchanged (expected, and a known gap).

## Range change (21 Nov)
- The top of the slider is now 10x (`DAMAGE_RANGE` 50 to 1000, step and default unchanged). Enemy health keeps `STAT_RANGE` (0.5x to 5x).
- Settings strings store the value as a variable-length number, so the range only gates validation: every old string (50 to 500) decodes to the same value. A value above 500 in a new string is out of range for older builds, which fall back to the default for that setting.
- No cap is needed: attack values are 32-bit whole numbers and a 10x base stays small (a base of 12 gives 120). The remainder rule is unchanged, so the product stays exact.
- Tests: `ten_times_damage_scales_by_ten` (stats.rs), `damage_goes_to_ten_times_and_old_strings_still_decode` (difficulty.rs).
- Not played at 10x. To test: set 10x and take one hit from a Keese at full health; expect 10 times the vanilla loss, so a hit kills at once.

## Notes
- Grabs, falls and the rest are covered by the separate `damage_taken` setting: see `35-damage-options.md`.
- Name-based filters are fragile. If a new attacker is missed, add it to `EXTRA_ATTACKERS` or widen the rule in `is_enemy_attack`.
- Grab and trap damage would need a different file per enemy; not attempted.


## Moved from STATUS.md (20 Nov): feature table row

- **Feature (old table row 02):** Enemy damage
- **State:** Done
- **What was seen in the game:** 3x: a bite took 6 quarter hearts instead of 2 (script, rc 5)
- **Next step:** Grabs (ReDead, Like Like) not covered; parked
