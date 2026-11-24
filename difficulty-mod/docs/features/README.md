# Difficulty features: index

One file per feature, each with your request, my interpretation, the investigation, the implementation, how it was tested, how to test it in the game, and notes. Start with [00-making-new-features.md](00-making-new-features.md).

This index lists every feature file in this folder. Which build carries a feature is in `../CHANGELOG.md`. Numbers 37, 39, 41 to 43, 45, 46, 50, 51, 53 and 56 are either features on other branches that are not in this folder yet or were never used.

| # | Feature | Setting | Played in the game? |
|---|---|---|---|
| 01 | [Enemy health](01-enemy-health.md) | `enemy_health` | Yes, works (rc 1) |
| 02 | [Enemy damage](02-enemy-damage.md) | `enemy_damage` | No |
| 03 | [Enemy speed](03-enemy-speed.md) | `enemy_speed` | No |
| 04 | [No heart drops](04-no-heart-drops.md) | `no_heart_drops` | Failed in 001, fix unplayed |
| 05 | [No ammo drops](05-no-ammo-drops.md) | `no_ammo_drops` | Failed in 001, fix unplayed |
| 06 | [No gossip fairies](06-no-gossip-fairies.md) | `no_gossip_fairies` | No |
| 07 | [Lose heart on death](07-lose-heart-on-death.md) | `lose_heart_on_death` | Failed in 001, fix unplayed |
| 08 | [No bottles during combat](08-no-bottles-in-combat.md) | `no_bottles_in_combat` | Failed in 001, fix unplayed |
| 09 | [Enemies regen health](09-enemy-regen.md) | `enemy_regen` | No |
| 10 | [More enemies](10-more-enemies.md) | `more_enemies` | No |
| 11 | [Bosses spawn minibosses](11-boss-minibosses.md) | `boss_minibosses` | No |
| 12 | [Bosses spawn minions](12-boss-minions.md) | `boss_minions` | No |
| 13 | [No dupe items](13-no-dupe-items.md) | `no_dupe_items` | No |
| 14 | [Enemies respawn sooner](14-enemies-respawn-sooner.md) | `enemies_respawn_sooner` | No |
| 15 | [No Z-targeting](15-no-z-targeting.md) | `no_z_targeting` | No |
| 16 | [Fairies help enemies too](16-fairies-help-enemies.md) | `fairy_helps_enemies` | No (respawn half not possible) |
| 17 | [Enemies call for backup](17-enemies-call-for-backup.md) | `enemy_backup` | No |
| 18 | [Zora's Domain ambush](18-zora-ambush.md) | `zora_ambush` | No |
| 19 | [Bean Seller's sales pitch](19-bean-sellers-pitch.md) | `bean_pitch` | Retired (setting kept off) |
| 20 | [Enemies block the way](20-enemies-block-the-way.md) | `enemy_gates` | Yes, by hand |
| 21 | [Volvagia roams Hyrule Field](21-volvagia-roams-hyrule-field.md) | `volvagia_roams` | Partly (field playtest 20 Nov; fixes after it not replayed) |
| 22 | [Special encounters](22-special-encounters.md) | part of `more_enemies` | Partly |
| 23 | [Grass and pot surprises](23-grass-and-pot-surprises.md) | part of `more_enemies` | Yes, by hand |
| 24 | [Short-Tempered Cuccos](24-cucco-revenge.md) | `cucco_revenge` | Yes |
| 25 | [Spawn enemies](25-spawn-enemies-combo.md) | `enemy_spawn_combo` | Yes, by hand |
| 26 | [Enemy randomizer](26-enemy-randomizer.md) | `enemy_randomizer` | Partly (see the file) |
| 27 | [Unbreakable Bombchu Bowling grass](27-unbreakable-bowling-grass.md) (idea by Sabriel) | `unbreakable_bowling_grass` | Yes, by hand |
| 28 | [Unequip sword and shield](28-unequip-gear.md) | none | Not possible with data edits |
| 29 | [Fireball storms](29-fireball-storms.md) | `fireball_storms` | Yes, by hand (two of four spots) |
| 30 | [Faster sliding blocks](30-faster-blocks.md) | `faster_blocks` | No |
| 31 | [Random fuses](31-random-fuses.md) | `random_fuses` | No (bowling fuse fix played) |
| 32 | [Shield-eating mega Like Likes](32-shield-eating-like-likes.md) | `mega_like_likes` | Yes, by hand (swallow, kill, Mirror sparkle) |
| 33 | [Random enemy sizes](33-random-enemy-sizes.md) | `enemy_sizes`, `enemy_sizes_health` | See the file |
| 34 | [Shorter timers](34-shorter-timers.md) | `shorter_timers` | Yes, by script (21 Nov) |
| 35 | [Damage options](35-damage-options.md) | `damage_taken` | Partly (see the file) |
| 36 | [Kaepora next-item hint](36-kaepora-next-item-hint.md) | `kaepora_hint` | Yes, by script (see the file) |
| 38 | [More owl rides](38-owl-warps.md) | `owl_warps` | See the file |
| 40 | [Harder cucco game](40-harder-cuccos.md) | `harder_talon_cuccos` | Yes, by hand (22 Nov) |
| 44 | [More boulders and Fire Keese](44-more-boulders.md) | `more_boulders` | Yes, by script (22 Nov) |
| 47 | [Skip first pickup text: every pickup](47-first-pickup-all.md) | `skip_first_pickup_text` (extended to every pickup) | Yes, by hand and by script (22 Nov) |
| 48 | [Enemy aggressiveness](48-enemy-aggressiveness.md) | `enemy_aggression` | Yes, by hand (hands-on 15); the Mad Scrub fix was not replayed |
| 49 | [Fall in pit penalty](49-fall-pit-penalty.md) | `fall_pit_penalty` | Partly (old on/off version, by hand) |
| 52 | [Enemy feints](52-enemy-feints.md) (Stalfos, Lizalfos, Dinolfos, Iron Knuckles, Deku Babas; held wind-ups) | `enemy_feints` | round 4 by script (boomerang hold; user picked it from clips 23 Nov); not yet played by hand |
| 54 | [Gerudo fighter](54-gerudo-fighter.md) | part of `more_enemies` | See the file |
| 55 | [AI graph attachments](55-ainb-attachments.md) | none (tooling) | Byte-identical round trip by test |
| 57 | [Owl ride to Lon Lon Ranch](57-owl-ranch.md) | part of `owl_warps` | See the file |
| 70 | [Kokiri Forest rock 11](70-kf-rock-11.md) (upstream issue 70; superseded by upstream retiring the rock, 24 Nov) | none | No (removed) |

Features 01 to 08 I built directly. Features 09 to 19 were built by sub-agents; what those files say about investigation and testing comes from the agents' reports, which I did not re-run except where noted.
