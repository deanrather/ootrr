# Enemy aggressiveness

Setting `enemy_aggression` (number 160), slider 0x to 3x in steps of 0.5x, default 1x (off). Code: `randomizer/core/src/difficulty/aggression.rs`, wired in `difficulty::edit_entry`.

## What was requested

"Enemy Aggressiveness. 0x = peaceful: enemies just sit there. 0.5x: they attack half as often. 1.5x: 50% more often. 2x: double as often, and so on."

## How enemies decide to attack (what the game files say)

Most enemies use the game's shared attack-permission system:

- **`GameBalance/AttackInterval/<Name>`**: three pairs of waits in frames between attacks (`ShortIntervalMin/Max`, `MediumIntervalMin/Max`, `LongIntervalMin/Max`). 32 files. `Default` (40-60, 120-150, 200-240) is shared by most enemies; Stalfos (15-60, 60-150, 15-240), Poe, Floormaster, Dinolfos, Stalchild, Stinger, Torch Slug, Spike, Business Scrub, Tentacle, Guay flock, Tektite and the bosses Ganon, Gohma, Dark Link and the Gerudo thieves have their own. Keese, Deku Baba and Dodongo have all zeros (they attack as soon as they can), ReDead has 1 frame.
- **`Component/GameBalanceClientParam/<Name>`**: names the interval file (`AttackIntervalRef`) and holds `GlobalPlayerNoAttackTime` (80 frames in `Default`, 90 in the `Play` variant; 0 for Stalfos, Wolfos, Lizalfos), a pause after an attack before the next one on Link.
- **AI graphs** ask for permission with `SelectorGameBalanceAttackPermission` (36 nodes across enemy and global packs) before an attack. Its `AttackIntervalRatio` input scales the interval; the per-sequence value sits in `Game/BSA/ActionSeqContainer/<Name>_LandBattle` (`ActionSeqs[n]/AttackIntervalRatio`). The game itself uses 0.7 there for Ganon's second phase, so scaling these waits is how the game makes a fight more aggressive.
- **`Component/PerceptionReceiverParam/<Name>`**: how an enemy notices Link. Two kinds: sight and intuition with ranges (`UseSight`, `UseIntuition`, Deku Baba, Keese, Like Like, Moblin, ReDead, Octorok ...) or "system range" (`IsForceSystemRange` with `ForceSystemRange`: Stalfos 31, Wolfos 51, Lizalfos 70, Iron Knuckle 30, Leever 30, Stalchild 6). All these files are flat (no `$parent`).

## How it works

- **0.5x to 3x:** every `AttackInterval` file except bosses' and minibosses' own has all six waits divided by the multiplier, and `GlobalPlayerNoAttackTime` and `GlobalPlayerNoAttackTimePlay` in `Default`, `EnemyCommon` and the enemies' own client files are divided too. Zero waits stay zero. Stacks with Enemy speed, which already divided the enemies' own interval files: the two multiply (`aggression::pace_pct`), so speed 1.5x and aggressiveness 2x make the waits a third.
- **0x, peaceful:** every enemy perception receiver gets `IsEnabledByDefault`, `IsForceSystemRange` and all `Use*` senses set to false, so the enemy never notices Link. Attack timing is left alone. Kept awake, because a fight, a check or a puzzle needs them: bosses and minibosses (`DungeonBoss`, `MiddleBoss`, battle managers), Business Scrubs, the Deku Brothers and the forest stage scrubs with their Leaf and Seed shots (sales, the Deku Tree 2-3-1, the stage check). Mad Scrubs and ordinary Deku Scrubs are calmed like any other enemy (the first hands-on showed Mad Scrubs still shooting when every Enemy_Scrub name was exempt), Poe sisters and Meg, Big Poes, Armos, Iron Knuckles, Dead Hand, Skull Kid, Gold Skulltulas, the Guay boss.
- Only boolean and float values change, so every file keeps its size (checked; an edit that grew a file is refused).

## Which enemies change (from the files, not yet seen in the game)

| Enemy | 0.5x to 3x | 0x |
|---|---|---|
| Stalfos | own interval file, scaled | system range off |
| Keese | interval all zero: no change | sight off |
| Lizalfos | its client file names no interval and has pause 0: no change | system range off |
| Dinolfos | own interval file: scaled | sight off |
| Moblin (spear, club) | no client file and no permission node found: no change | sight and intuition off |
| Deku Baba | interval all zero: no change | sight off |
| Wolfos | inherits `Default` timing, own pause 0: scaled | system range off |
| Like Like | no client file and no permission node found: no change | sight off |
| ReDead, Gibdo | own 1-frame interval: barely changes; inherited pause scaled | sight and intuition off |
| Iron Knuckle | inherits `Default` timing (permission node in its battle graph): scaled | kept awake |
| Dodongo | own interval all zero: no change (Baby Dodongos use `Default` and scale) | sight and intuition off |
| Floormaster, Poe, Stalchild, Stinger, Torch Slug, Spike, Tektite | own interval files: scaled | senses off |

## Risks

- Peaceful enemies that a room needs killed still die to your attacks, so a room that opens when its enemies are gone still opens. Whether a peaceful enemy fights back once hit is unknown (it may react to damage through another path).
- Enemies started by room logic ("force battle") may still turn to fight without perception.
- `Default` timing is shared: a boss or miniboss that uses `Default` rather than its own file (for example Barinade or Bongo Bongo, if they use the permission system) also attacks more often. Bosses with their own file (Ganon, Gohma, Dark Link, Gerudo thieves) keep theirs.
- An interval is a wait between attacks, not the whole cycle: approach, wind-up and recovery do not shrink, so 2x gives "up to twice as often", most visible on enemies that circle and wait (Stalfos, Wolfos, Iron Knuckle).

## How it was tested

- Unit tests in `aggression.rs`: which names scale and which stay awake, the speed-times-aggressiveness rule, pauses divided with the file size kept, every sense turned off, the report line.
- With the game files (`OOTRR_ROMFS`): the real Stalfos pack at 2x has its waits halved and its pause 40; at 0x its receiver is off and its waits untouched; Keese sight off; Business Scrub, Iron Knuckle, Joelle, Gohma and Dark Link receivers unchanged; Gohma's own interval file unchanged at 2x.
- Hands-on 15 (23 Nov, played by the user, Temple of Time): at 0x, 60+ enemy kinds spawned in batches; all ordinary enemies stayed peaceful even when hit, except Mad Scrubs (fixed afterwards by narrowing the exempt scrub names; not re-played). The exempt ones still fought as designed: Iron Knuckles after being bumped, Dead Hand, the field Big Poe. At 3x Stalfos, Lizalfos, Wolfos, ReDead, Iron Knuckle and Like Like were judged "perfect, exactly what you'd want".
- Not tested: whether a kill-to-open room still opens at 0x when its enemies hide until they notice Link (Leevers, Peahat larvae). Known risk.

## How to test it in the game

- 2x and 3x: stand still near a Stalfos or Wolfos (Hyrule Field spawn, or Forest Temple Stalfos room) and count hits per minute against 1x. Expect clearly more for Stalfos, Wolfos, Iron Knuckle, Floormaster; about the same for Keese and Deku Baba.
- 0.5x: Stalfos should circle longer between attacks.
- 0x: walk up to Keese, a Deku Baba, a Stalfos and a Wolfos: none should react. Hit one and note whether it fights back. Check that Deku Tree scrubs still pop up and that a room with enemies that opens a door still opens when they are killed.
