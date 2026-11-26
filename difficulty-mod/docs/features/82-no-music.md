# 82. No Music

A fourth choice in the music setting that silences the background music. Cosmetic: the seed and settings string do not change.

## Requested

"Right now it lists 3 options in the top down, add a new one that's 'No Music'."

## Interpretation

The drop-down is **Music shuffle** on the Cosmetics page. It gains **No Music** after `off`, `grouped` and `anywhere`. On the command line it is `--music none`. The browser version builds from the same page files, so it has the same choice.

Choices made where the request does not say (each is a short change to reverse):

- Cutscene music tracks (`BGM_Dm_`) are silenced too.
- Fanfares and jingles are still heard: item, heart, medal and Spiritual Stone fanfares, the riddle chimes, game over, sleeping in a bed.
- Characters who perform are still heard (Malon singing), as are the ocarina, song example melodies, sound effects, voices and ambient sound.

## Decisions

None from the user beyond the request.

## How it works

The shuffle choices point each music call in `SLink2/slink2.Product.111.bslnk` at another track. No Music leaves every call on its own track and turns its volume down, so each track keeps its length, loop points, transitions and events.

Every `BGM_` call names a sound group, and `Sound/Group/GroupSetting.Product.111.byml` gives each group a volume.

1. `GroupParam/<group>/Default/Volume` becomes 0 for `BgmDefault`, `BgmEndReverb`, `BgmEnemyBattle` and `BgmInsideWorld`. These four groups hold only `BGM_` tracks.
2. 46 music calls (cutscene tracks, the Wasteland carpet merchant and Business Scrub shop themes) are in `DemoBgmDefault`, a group that also holds song melodies and cutscene sounds. Their group name changes to `BgmDefault`, in place. The three jingles in that group (`BGM_ItemGet_FromZelda`, `BGM_SleepInBed`, `BGM_SleepInBedToEnemyMuseum`) stay.
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

## Tested

- Unit tests: the four groups are muted and the others are not, a missing group is refused, `none` parses and does not use the cosmetic seed, the web request accepts `none`.
- Game-file test (`silence::the_real_files_leave_no_music_call_in_an_audible_music_group`): after the edit every `BGM_` call is in a muted group or is a fanfare, a kept jingle or a performer, no other sound is in a muted group, 46 calls change group and nothing else in the sound link changes.
- `cargo test --workspace` with the game files: 2431 passed, 0 failed, 13 ignored.
- Same seed, nightly `240af8b` against this change, Longshot host, one cosmetic seed: the mods with music `off` (283 files), `grouped` and `anywhere` (284 files) are byte-identical, and so is the seed file.
- A `none` mod differs from an `off` mod in three files only, and `flow-loads` finds nothing missing.
- Not played, not heard. No game test has been run.

## Test in the game

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
