# 82. No Music

A fourth choice in the music setting that silences the background music. Cosmetic: the seed and settings string do not change.

## Requested

"Right now it lists 3 options in the top down, add a new one that's 'No Music'."

## Interpretation

The drop-down is **Music shuffle** on the Cosmetics page. It gains **No Music** after `off`, `grouped` and `anywhere`. On the command line it is `--music none`. The browser version builds from the same page files, so it has the same choice.

Choices made where the request does not say (each is a short change to reverse):

- Cutscene music tracks (`BGM_Dm_`) are silenced too.
- Fanfares and jingles are still heard: item, heart, medal and Spiritual Stone fanfares, the riddle chimes, game over, sleeping in a bed, and the Temple of Time cutscene jingle (`BGM_Dm_TimeTemple_0030_Jingle`). `cosmetic::is_jingle` is the one list, shared with the shuffle.
- `BGM_MiniGameOutro` is silenced. It is the closing bars of the minigame theme (about 4 seconds, in the theme's own event and in the background music group), not a fanfare. Not heard.
- Characters who perform are still heard (Malon singing), as are the ocarina, song example melodies, sound effects, voices and ambient sound.

## Decisions

- User, 26 Nov: "'no music' should only mute the background music".
- The report that music shuffle "just straight up mutes the music" could not be tied to a version or seed (user: "no can do. it was ages ago"). It is not reproduced, and the shuffle fix below does not claim to answer it.

## How it works

The shuffle choices point each music call in `SLink2/slink2.Product.111.bslnk` at another track. No Music leaves every call on its own track and turns its volume down, so each track keeps its length, loop points, transitions and events.

Every `BGM_` call names a sound group, and `Sound/Group/GroupSetting.Product.111.byml` gives each group a volume.

1. `GroupParam/<group>/Default/Volume` becomes 0 for `BgmDefault`, `BgmEndReverb`, `BgmEnemyBattle` and `BgmInsideWorld`. These four groups hold only `BGM_` tracks.
2. 45 music calls are in `DemoBgmDefault`, a group that also holds song melodies and cutscene sounds. Their group name changes to `BgmDefault`, in place. They are cutscene tracks (`BGM_Dm_`), the Wasteland carpet merchant (`BGM_Shop_Desert`) and Business Scrub (`BGM_Shop_ScrubBusiness`) themes, the Song of Time lesson's `BGM_ShrineOfTime_Child` and the owl's `BGM_Kaepora` and `BGM_Kaepora_Outro`. The jingles in that group (`BGM_ItemGet_FromZelda`, `BGM_SleepInBed`, `BGM_SleepInBedToEnemyMuseum`, `BGM_Dm_TimeTemple_0030_Jingle`) stay.
3. The group file grows by 576 bytes (its groups no longer share one block), so its size table entry grows too.

The mod gets three changed files: the sound link, the group file and the size table. No code patch.

New file: `randomizer/cosmetics/src/silence.rs`. `slink.rs` gains `group` and `regroup`. `required::unpack` lists the group file, so a build from a game package works. No Music does not unpack the sound streams and does not use the cosmetic seed.

## What is silent

By track name in the game's files, not by ear:

- Field, town, village, house, dungeon, temple and Ganon's Castle music, including the dungeons' ambient music layers (chimes, drones, Jabu-Jabu's roars).
- Enemy battle, miniboss, boss and fishing battle music.
- Minigame, shop and speaker music, horse riding, Great Fairy Fountain, Lost Woods.
- Cutscene tracks named `BGM_Dm_`.

## What is not silenced

- Cutscene soundtracks (`Sound/Resource/Stream/Dm_*`) are separate whole-scene mixes, not `BGM_` tracks, and are not changed. The title screen (`Dm_Title_*`), the ending and the staff credits are among them, so any music mixed into them is still heard.
- The file select has no track of its own name. If it plays a `BGM_` theme, that theme is silent. Not checked.
- The Lost Woods music that grows louder toward the right way is silent, so it no longer helps there.

## Scenes that wait on music

No file, length, loop or call changes, so there is no shorter or missing track for a scene to wait on. This covers every kind of silenced track and every kept one.

Unproven, because they are engine behaviour that data does not show:

- The game applies the group volume from this file.
- A track at volume 0 keeps playing and still reports its name, which **No bottles during combat** reads.
- The volume stays at 0 after a loading zone or a reload.

## Shuffle fix: tracks that never changed

Players reported that the castle sneaking area and Lon Lon Ranch kept their own music with the music shuffled. With nightly `240af8b2`, 37 of 274 music calls could never change with `grouped` and 29 with `anywhere`, over 60 cosmetic seeds each. With the fix, 8 and 0.

### Cause

1. A music event's calls in the sound link belong to the user named by its `SLinkUserName`, and the shuffle looked for a user with the event's own name. `BGM_LonLonRanch_Child` and `BGM_LonLonRanch_Adult` both use `BGM_LonLonRanch`, so neither ever changed. `BGM_LostWood_LeadOcarina` uses `BGM_LostWood`.
2. Several users' calls share one parameter block, and the shuffle left a block alone unless every caller wanted the same track.

### Change

1. `moves` keys each wanted track by `SLinkUserName`. Where an event has the user's own name, its keys win over another event that shares the user.
2. On a shared block, the caller the track is named after decides: the user whose name equals the key, else the user whose name is the longest `_` prefix of it. With no such caller, the callers still have to agree. The file keeps its size.
3. Events that share a user which no event is named after (`BGM_LonLonRanch_Child` and `_Adult`) trade as one single track, so both ages hear the same new one. The child theme is built from layers and some of its calls are the adult's too, so it no longer trades as a layer set.

### Tracks that now move

Lon Lon Ranch (child and adult, which get the same new track), the castle grounds (`BGM_HyruleGarden`), the windmill as adult, King Dodongo (`BGM_DungeonBoss_Dinasour`), inside the Deku Tree, the Lost Woods ocarina layers and Ganon (`anywhere` only).

### Calls that follow another user

A shared block holds one track, so these play whatever the deciding user got:

| Follows | Shared call | Deciding user |
|---|---|---|
| `BGM_SurfaceFair`, one layer | `BGM_HyruleGarden` | `BGM_HyruleGarden` |
| `BGM_WindmillHouse_Child`, its adult track | `BGM_WindmillHouse_Adult` | `BGM_WindmillHouse_Adult` |
| `BGM_DungeonBoss_Volvagia`, its Dinosaur track | `BGM_DungeonBoss_Dinasour` | `BGM_DungeonBoss_Dinasour` |
| `BGM_DungeonBoss_KotakeKoume` | `BGM_DungeonBoss` | `BGM_DungeonBoss` |
| `BGM_EnemyMuseum` and `BGM_Common_DekuTree` | the Deku Tree low and melody tracks | `BGM_InsideTheDekuTree` |

`BGM_SurfaceFair`'s other layers have their own calls and are shuffled apart from the Deku Tree. So one layer of that theme can come from a different set than the others. Not heard.

### Left unchanged on purpose

- `BGM_DungeonBoss_Ganon` with `grouped`: it is the only boss theme built from layers, so it has nothing of its kind to trade with.
- Cutscene tracks (`BGM_Dm_`) and intro, outro, pad, roar and demo parts.
- Music that is not in the music table, played by an object in the world: the shop and minigame shop speakers, Malon's singing and the owl's theme. The shuffle has never touched it.

No Music is not affected: it mutes by sound group, so shared blocks and aliases are silent too.

A cosmetic seed now gives different `grouped` and `anywhere` music than before. The seed, the settings string, `off` and `none` do not change.

## Tested

- Unit tests: the four groups are muted and the others are not, a missing group is refused, `none` parses and does not use the cosmetic seed, the web request accepts `none`.
- Game-file test (`silence::the_real_files_leave_no_music_call_in_an_audible_music_group`): after the edit every `BGM_` call is in a muted group or is a fanfare, a kept jingle or a performer, no other sound is in a muted group, 45 calls change group and nothing else in the sound link changes.
- `cargo test --workspace` with the game files: 2431 passed, 0 failed, 13 ignored.
- Same seed, nightly `240af8b` against this change, Longshot host, one cosmetic seed: the mods with music `off` (283 files), `grouped` and `anywhere` (284 files) are byte-identical, and so is the seed file.
- A `none` mod differs from an `off` mod in three files only, and `flow-loads` finds nothing missing.
- Shuffle fix: `every_real_track_can_move_except_the_listed_ones` fails on the unfixed code (37 and 29 calls stuck) and passes with the fix. Unit tests cover the shared user and the shared block.
- After the shuffle fix, same seed against nightly: the `off` mod is byte-identical, `grouped` and `anywhere` differ in the sound link only. The `none` mod is byte-identical to the one built before the fix.
- `cargo test --workspace` on the shuffle fix (code commit `5e80d4d8`): 2435 passed, 0 failed, 13 ignored.
- Not played, not heard. No game test has been run.

## Test in the game

For the shuffle, build with `--music anywhere` and a fixed `--cosmetic-seed`. Listen on the castle grounds, at Lon Lon Ranch as child and adult, in the windmill and at King Dodongo for music that is not the place's own, and walk the world for places with no music. For No Music:

1. Build the same seed twice, with `--music none` and with `--music off`.
2. Stand in a place with music (the Temple of Time). With No Music only effects and ambient sound are heard.
3. Swing the sword, open a chest (the fanfare is heard), play a song on the ocarina and see it take effect.
4. Cross a loading zone and reload the save. The music stays silent.
5. Watch a cutscene that has music through to its end.
6. Start a fight. No battle music is heard.

## Commit message

```
Music shuffle has a No Music choice that silences the background music

Cosmetics > Music shuffle gains No Music (`--music none`). Area, battle,
boss, minigame, shop and cutscene music tracks are silent. Fanfares,
jingles, sound effects, the ocarina and performing characters are still
heard. Cutscene soundtracks, including the title screen's, keep their
own sound.

## Cause

The setting could only keep or shuffle the music.

## Change

Sound/Group/GroupSetting sets the volume of the four background music
groups to 0, and the music calls in the scene music group move to one of
them in SLink2. No track, length or loop changes. The group file grows,
so its size table entry grows. The other choices run no new code, and
required::unpack lists the new file.

## Checked

Unit and game-file tests: every BGM_ call ends in a muted group or is a
fanfare, kept jingle or performer. Workspace tests 2431 passed, 0 failed,
13 ignored. Music off, grouped and anywhere build byte-identical to
nightly for one seed. Not played in the game, so that the game applies
the group volume is unproven.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
```

The shuffle fix is its own commit:

```
Lon Lon Ranch, castle grounds and windmill music now shuffle

With Cosmetics > Music shuffle on, Lon Lon Ranch, the castle grounds, the
windmill as adult, King Dodongo and the Deku Tree always kept their own
music.

## Cause

Wanted tracks were looked up by event name, not by SLinkUserName, and a
parameter block shared by several users was left alone unless they all
wanted the same track.

## Change

moves() keys by SLinkUserName, and the two Lon Lon events trade as one
track. On a shared block the user the track is
named after decides. Calls sharing its block follow it. Ganon's theme
stays with grouped, as the only layered boss theme. A cosmetic seed now
gives different grouped and anywhere music. The off and none choices and
the seed are unchanged.

## Checked

A test over the real files counts calls that never move over 60 seeds:
grouped 37 before and 8 after, anywhere 29 before and 0 after. Workspace
tests 2435 passed, 0 failed, 13 ignored. Off builds byte-identical to nightly for one seed. Not
played or heard in the game.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
```
