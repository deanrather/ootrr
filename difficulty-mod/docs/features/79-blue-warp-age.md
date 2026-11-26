# Blue warps under dungeon shuffle

A blue warp keeps Link's age when `shuffle_entrances` is `dungeons`, and lands outside the entrance he used. Ship of Harkinian (SoH) does the same.

## Problem

The boss room's blue warp changes to a fixed `_Child` or `_Adult` map (event flow `Dm_SY_BancChange_EscapeDungeon`), and the map sets Link's age. Only Dodongo's Cavern picks its map by age. With the dungeons shuffled, either age can reach most bosses, so the warp can change Link's age. It also lands at the boss's own dungeon, not at the entrance Link used.

Seen in the game: an adult who beat Queen Gohma came out as a child in Kokiri Forest. The other bosses are read from the event flow with `ootrr flow-sim`.

## Change

`entr/blue_warp.rs` runs only when the seed has a dungeon layout. Every map change a boss's warp reaches becomes a switch on `EventQueryPlayerIsChild`, with one change per age.

Where Link lands depends on the entrance the boss's dungeon sits behind:

| Entrance | Landing |
|---|---|
| Deku Tree | child: the usual warp point. Adult: the Deku Tree's doorway |
| Dodongo's Cavern, Jabu-Jabu's Belly | the usual warp point |
| Forest Temple | the temple's doorway in Sacred Forest Meadow |
| Fire, Water, Shadow, Spirit Temple | the usual warp point |
| Bottom of the Well, Ice Cavern, Gerudo Training Ground | that entrance's doorway |

The game's own arrival scene plays only when the boss's dungeon is behind its own entrance and Link has the age the scene was made for (Dodongo's Cavern: either age). It never plays for the Forest Temple, whose own landing is in Kokiri Forest.

Anywhere else, the scene is left out, because it moves Link and the camera to fixed spots. What it writes (quest steps, game data, scene played flags) is written in the boss room before the map changes.

When one age has no way to arrive outside an entrance (the entrance catalogue's `start_ok`), that age cannot have used it, and its branch shares the other age's landing.

With `shuffle_entrances` off, the build stages nothing here. Without a shuffle a boss is only reached at the age its warp expects, except Dodongo, which the game handles.

## Differences from SoH

1. Forest Temple entrance: SoH lands on the meadow's blue warp point, this lands at the temple's doorway.
2. Adult at the Deku Tree entrance: the game has no adult warp point there, so Link arrives in the doorway. Whether an adult can walk out of it is not tested: the adult Deku Tree mouth is blocked by boulders.
3. SoH also opens the Deku Tree to adults and the Fire Temple, Bottom of the Well and Gerudo Training Ground to the other age under dungeon shuffle. This does not.
4. Boss shuffle and decoupled entrances do not exist here.

## Tests

`cargo test -p ootrr-core entr::` with `OOTRR_ROMFS` set:

1. Every boss and both ages land on a map of that age that has the named start position, with the dungeons in place and in every two-dungeon swap that fits.
2. A boss behind another entrance lands outside it, and the arrival scene's quest writes are present.

Not played in the game.

## Test it in the game

1. Roll a seed with `shuffle_entrances=dungeons`.
2. Enter a boss dungeon, set the boss's defeat flag and stand by the blue warp.
3. Take the warp. Link keeps his age and comes out at the entrance he used.

## Commit message

```
Blue warps keep Link's age and return him to the entrance he used

With "Shuffle Entrances" set to Dungeons, the blue warp after a boss
could turn an adult Link into a child or a child into an adult, and it
left him at the boss's own dungeon instead of the entrance he used.

## Cause

Dm_SY_BancChange_EscapeDungeon changes to a fixed _Child or _Adult map,
and the map sets Link's age. Only Dodongo's Cavern picks by age.

## Change

entr/blue_warp.rs runs only for seeds with a dungeon layout, so default
seeds are unchanged. Each warp's map change becomes a switch on
EventQueryPlayerIsChild and lands outside the entrance the dungeon sits
behind. Where the game's own arrival scene cannot play there, its quest
writes happen in the boss room.

## Checked

Game-file tests cover every boss and both ages in place and in every
two-dungeon swap that fits. Not played in the game. Seen in the game
before the fix for Queen Gohma only.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
```
