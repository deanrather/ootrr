# 44. More boulders and Fire Keese on the Death Mountain Trail

## Requested (user, 21 Nov)

"More rolling boulders. They're way too easy to dodge. Let's put double as many. And fire bats."

## Setting

`more_boulders`, number 159, "More boulders and Fire Keese on the Trail", off by default. Difficulty page, section Environment, after Fireball storms. The number 159 was the next free one when it was made. Default settings string and seeds unchanged.

Tip: "Twice as many boulders roll down the adult Death Mountain Trail, and six Fire Keese flutter over the trail, as a child and as an adult."

## Where it lives (found in the game files)

- Only the adult Trail has rolling boulders. `Banc/DeathMountainTrail/DeathMountainTrail_Adult_Static.bcett.byml.zs` holds two spawners: `RollingRockSpawnActor_DeathMountainTrail` (top of the Trail, two spawn anchors, each linked with a rail) and `RollingRockSpawnActor_DeathMountainTrail_Large` (above Goron City, one anchor). Each carries `Dynamic` values `SpawnTime` 7.5 (seconds between boulders), `RollingRockMoveSpeed` 7.0 and `MinLimitSpeed` 4.0. The child Trail has none (its hazard is the fall of fireballs, see feature 21's neighbour, fireball storms).
- No Keese are placed on the Trail in the vanilla game.

## What the setting does

`difficulty/boulders.rs`, one step in the late stage, on the Static file of both ages:

1. **Boulders**: `SpawnTime` of both spawners is halved (7.5 s to 3.75 s), so twice as many boulders roll down. The boulders still come from the game's own spawners and run on the game's own rails, so they roll as the vanilla ones do.
2. **Fire Keese**: six `Enemy_Keese_Fire` are placed over the walkable stretches of the Trail (child and adult), a few units above the ground beside the boulder lanes and the fireball areas, at points taken from the game's own start positions and enemy placements. They are plain placed enemies with no room logic, like the Torch Slug the fireball storms add; they wake and attack on their own.

Everything is data (six floats halved, twelve placements). It works wherever OOTRR works. Fireball storms do not clash: they change the Trail's logic modules, not its Static file.

## Tested

- `cargo test -p ootrr-core boulders`: the spawners' times are exactly half, the child map has no spawner and the adult has two, each map gains exactly six `Enemy_Keese_Fire` and nothing else changes. `difficulty` and `settings` pass with the game files (see the changelog entry for the run).
- In the game: see "Game results" below.

## Game results

Played by script on 22 Nov, adult and child saves, one game copy at a time, controlled by a second launch with the same mods built from the same preset minus the setting (`boulders_on` against `boulders_off`).

| Measure | Setting off | Setting on |
|---|---|---|
| Boulders passing Link beside the Goron City lane (the `_Large` spawner), counted from window video (frame difference against a rolling background, one event per boulder) | 12 in 90 s: one every 7.5 s, 8 per minute | 16 in 60 s: one every 3.7 s, 16 per minute |
| Fire Keese on the child Trail | none | five listed in the map at the placed spots (the sixth lies at the top, not loaded from the start); one set Link on fire at once (life 12 to 8) |
| Fire Keese on the adult Trail | none | five listed at the placed spots |
| Frame rate (Trail, window 848x516, debug overlay on) | 60 | 51 to 61, dips to 4 fps for about 5 s the first time a boulder appeared (shader build), none after |
| Map loads (Kokiri Forest, both Trails, age switches both ways) | load | load, no errors in the log |

The enemy list names the Fire Keese `Enemy_Keese`; they are the fire kind (they ignite Link). The 3.75 s interval matches the halved `SpawnTime` exactly. The spawner above (top of the Trail, two anchors) was not counted separately; it uses the same halved value.

The two clips (mod on, 60 s; mod off, 90 s) are kept in the working notes, not in the repo.

Not proven: that Fire Keese attack every time they are met (one meeting seen), and the top spawner's rate.

## How to test it in the game

Adult Death Mountain Trail, setting on: stand beside a boulder lane (just below the top, or above Goron City) for 30 s and count the boulders; with the setting off the count is about half. Fire Keese flutter over the Trail as a child and as an adult and attack when you come near.
