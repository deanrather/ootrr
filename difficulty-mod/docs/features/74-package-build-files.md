# 74. Builds from a game package: unpack the files the Difficulty settings read

Fix branch `fix/package-build-difficulty-files`. No setting changes; default output, settings strings and default seeds are unchanged. Code: `randomizer/core/src/required.rs`.

## Requested (24 Nov)

A review found that Difficulty settings merged in #52 fail when the game is given as a `.nup` or `.nx` package. A package build first unpacks only the files the build reads into a temporary folder (`required::unpack`); files a setting reads that were never added to that list are missing.

## What failed

Reproduced with upstream main (`c139d4e`) building from a game package:

| Setting | Missing file |
|---|---|
| Enemies regen health, Fairy helps, Mega Like Likes | `ELink2/elink2.Product.110.belnk.zs` |
| No heart drops, Limited ammo | `GameParameter/DropGlobalParam/Default.game__drop__DropGlobalParam.bgyml` |
| More enemies, any value | `AoC1` (listed by `extras::files`, os error 3) |
| More enemies (horde placement) | `Phive/NavMesh`, `Phive/StitchedNavMesh`: read for ground checks |

The folder build works because every file is there; only a package build hits it.

Origin (history): 5bdcd62 (18 Nov, drop lottery), e387d73 (18 Nov, AoC1 in More enemies), 7ac9a13 (20 Nov, ELink2 edit), c456321 (19 Nov, nav meshes); all reached upstream in #52. None added its file to `required.rs`; the history is silent on why, so this was an oversight (our inference).

## Fix

`required::DIFFICULTY_FILES` (the lottery file and the ELink2 file) and `DIFFICULTY_TREES` (`AoC1` 83 KB, `Phive/NavMesh` 22.8 MB, `Phive/StitchedNavMesh` 9.3 MB) are added to the unpack set. About 33 MB more is unpacked. The default build's unpack set grows too, since the unpack set does not depend on settings. After merging nightly dda8087 the same test failed for two more reads that nightly's changes brought: the Darunia Jr. adult schedule file (`open_world::rolling_goron::OWN`, added to the unpack set) and the pose file of a pickup's model (`Model/Item_*_Animation.anim.bfres.zs`, 75 files, about 1 MB, matched by `required::POSE_HEAD` and `POSE_TAIL` through `Unpack::shaped`; without it the Obj_Ocarina packs, ActorInfo and size table differed from the folder build). `a_package_build_unpacks_the_files_the_difficulty_settings_read` checks both.

## Tested

- `a_package_build_unpacks_the_files_the_difficulty_settings_read`: the unpack set wants each file above and still excludes unrelated `Phive` and `ELink2` files.
- `every_difficulty_setting_builds_the_same_mod_from_the_unpack_set_alone`: cuts the game folder down to the unpack set, builds with every Difficulty setting at its maximum (plus first-pickup, area-intro, blocks and owl settings) from the full folder and from the cut, and requires the same file list and identical bytes. It failed on the old unpack list and passes now. `cargo test -p ootrr-core --lib required::`: 5 passed, 564 s (the machine was shared).
- Real package: the same all-Difficulty seed built from the `.nup` dump. Old exe (main `c139d4e`): fails after 27 s with the missing drop file. Fixed exe: finishes in 86 s, 772 files, and `diff -r` against the folder build of the same seed (51 s) shows no difference. The unpack is about 35 s of the 86 s now (the folder build's 51 s is the build alone).
- `cargo test --workspace`: 1611 passed, 0 failed, 6 ignored. `tools/audit.py randomizer` and `difficulty-mod`: FAIL 0.

## How to test

Build any seed with several Difficulty settings on (More enemies, Enemies regen health, No heart drops) from a `.nup` or `.nx` instead of an extracted folder. It must finish; the result must match a build from the extracted folder.

## Notes for the next feature

A setting that reads a game file outside `required::TREES` must add it to `required::unpack` (or `DIFFICULTY_FILES`/`DIFFICULTY_TREES`); the second test above catches it for any setting the Difficulty page or `OURS` lists. A new setting that is not listed there must be added to `OURS`.

## MR Drafts

### Initial MR Draft
**Title:** Improved Difficulty > Package Builds Fix: unpack the files the Difficulty settings read

Building from a `.nup` or `.nx` failed for Enemies regen health, Fairy helps, Mega Like Likes, No heart drops, Limited ammo and every More enemies value, because the files they read were not in the unpack list. They are now (about 33 MB more), and a game-file test builds with every Difficulty setting from the unpack set alone and requires the same bytes as the folder build.

Merged with nightly dda8087, which fixed nightly's own tests. For details about implementation and testing, see difficulty-mod/docs/features/74-package-build-files.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
