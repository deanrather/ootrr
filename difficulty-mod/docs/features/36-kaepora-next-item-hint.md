# 36 Kaepora next-item hint: feasibility (read-only study, 21 Nov)

Request: a Hints option. Kaepora Gaebora, standing in Kokiri Forest, names where ONE of the next items the player needs for progression lies. Built and played on 21 Nov, see the end of this file.

## 1. Does OOTRR already have it? No.

Source read: `randomizer/core/src/hints.rs`, `npc_hints.rs`, `settings/list.rs`, README (worktree `wt\next`).

- **Gossip stone hints** (setting 41 `gossip_hints`, toggle): every stone tells where an important item lies. `hints::candidates` and `hints::pick` choose from placements, skipping keys, maps, compasses and tokens (`SKIP`).
- **Character hints** (setting 80 `misc_hints`, multi): Temple of Time altar, Dampe's diary, House of Skulltula, Zora's River frogs, Great Fairies, Sheik (Light Arrows), Malon (Weird Egg), Mido (Kokiri Sword), Saria (Magic Meter). Text only, no flow or actor edits. Each hint names one fixed item and its place, via `item_phrase`, `where_is` and `place`.
- Close but not the same: all of these are fixed, per-NPC, per-item. Nothing is progression-aware, no "way of the hero" or path hint, nothing picks "the next required item", and no owl appears in the hint code. The only owl code is `story_skip.rs` (skips owl cutscenes, `OWL_FLAGS`, `cut_owls`).

## 2. Ship of Harkinian: no.

SoH hint sources (from `soh/Enhancements/randomizer/hint.cpp` and `3drando/hints.cpp` on the `develop` branch): gossip stone hints (item, area, way of the hero, foolish, trial, junk), and static hints for the Temple of Time altars, Ganondorf, Sheik/OoT, Biggoron, Skulltula rewards, frogs, Malon, Big Poes, mask shop, Greg, warp songs. No file mentions an owl or Kaepora, and nothing hints "the next required item". Greg is only a hint-key item. A web search found no SoH owl or next-item hint either.
Sources: https://github.com/HarbourMasters/Shipwright/blob/develop/soh/soh/Enhancements/randomizer/hint.cpp and https://github.com/HarbourMasters/Shipwright/blob/develop/soh/soh/Enhancements/randomizer/3drando/hints.cpp

## 3. Is it possible here?

### Owl actor
- Vanilla owls in the game files: `Npc_Kaepora` pack, `Npc_Kaepora_DeathMountainTrail` pack plus flow `Npc_Kaepora_DeathMountainTrail.bfevfl`, `Npc_Kaepora_LakeHylia` pack plus flow, `Npc_Invisible_Kaepora`, and many cutscene-only owls (`Dm_HrlField_*`, `Dm_LostWood_*`, `Dm_ZoraRiver_*`, `Dm_TimeTemple_0086`). I did not find an owl placed in Kokiri Forest (the Lost Woods owls are scene-driven: `story_skip.rs` cuts them).
- Best base: `Npc_Kaepora_DeathMountainTrail`. Its flow has a plain `Talk` entry (EventTalk lines `Talk_00` and so on), a `Near` balloon entry and a mask-talk entry. Its flight to Kakariko is a separate branch (`EventReserveBancChange` to `KakarikoVillage_Child`, after the yes/no choice), so a rewritten Talk entry can leave it out.
- Placing it in `KokiriForest_Child`: a new actor in `Banc/KokiriForest/KokiriForest_Child_Dynamic.bcett.byml.zs`. Precedent: `difficulty/gates.rs` and `encounters.rs` already add actors to that map. Open point to prove by playing: whether the actor's pack is loaded in a map that never had it (the Dodongo black-screen lesson). The first test is a load test of Kokiri Forest, and of the Deku Tree and Lost Woods entrances.
- Dialog text: yes. Messages live in `EventFlowMsg/Npc_Kaepora_DeathMountainTrail.msbt` in the language archives, and `npc_hints.rs` already rewrites lines in all languages (`Op::Set`, `Op::Append`, `layout`). A gossip-stone style approach works.

### "Next required item"
- Text is fixed at generation, which is fine for a static choice. For example, the first sphere-1 required item the player does not hold at the start.
- It can also change as the player progresses, with no new saved flag. Event flows can read the game's own item flags (`EventQueryIsGameDataBool` on `IsGet.*`, seen in the DMT owl flow with `IsGet.Item_DinFire`). So the generator can write an ordered chain of N hints (the required items in logic order, with where each lies). The owl flow runs one switch per item, "do I own item k? yes: go on, no: say hint k", and says the first item the player still lacks. Stateless, as the rules require.
- Limits: items are only checkable if the game has an `IsGet` flag per item (swords, tools, songs, dungeon rewards: yes; progressive items and counts need a check on level or count; key items for dungeons, events and the Zelda's Letter and trade chain need checking one by one). Keep the chain short (about 6 to 10 items), each hint text laid out with `npc_hints::layout`.

### Risks
- **Flow ending:** every path must end like the vanilla Talk paths (EventTalk with `IsCloseDialog`, then `next=65535`), or Link freezes. Build the new chain as a new Talk entry reusing vanilla line nodes and test every branch, including "no hint left" (say a vanilla line).
- **Existing flows:** do not touch the DMT owl in Death Mountain Trail. A copy of the flow would need a new name and a new actor binding, which means adding a flow file, a flow-name registration (and size table entries) in the actor pack. Easier: edit the shared flow with a map-condition, or add a second Talk entry. Test both owls.
- **Logic:** none. Text and placement only. The hint only reads `seed.placements`, like `npc_hints`.
- **Quality of the choice:** "required" must come from the generator's required-item list (`required.rs` exists in core: check what it exposes), not from "important item". Items that are progression only on some paths need a rule.
- **Compat:** data edits only, works on every runtime. Settings: new setting default off, default settings string unchanged.

## 4. Verdict: feasible with changes

Changes from the ask: (a) the owl is a new actor placed in Kokiri Forest (its pack must load there: unproven); (b) "next item" is a short ordered chain gated on `IsGet.*` flags, not a single static pick. If the pack will not load, fall back to a static one-item text on an existing Kokiri Forest NPC, or a text-only hint on the signpost.

## Build plan
1. Branch `feature/kaepora-hint`, a small PR off upstream `main` (user rule, 21 Nov). New toggle in the Hints section of `settings/list.rs` and `menu.rs`, default off.
2. In `required.rs`/`hints.rs`: compute the ordered list of required items from the spoiler (first sphere items first), each with its place via `where_is` and `place`, and an `IsGet` flag name per item (map table; skip items without one).
3. Placement: add the owl to `KokiriForest_Child_Dynamic` (use the Longshot "position the object" tool to pick the spot). Load test the map first, before any flow work.
4. Flow: a new Talk chain (switches on `EventQueryIsGameDataBool`) ending in vanilla Talk-style nodes. Text lines via an `npc_hints`-style rewrite in all languages.
5. Tests: unit tests for the chain and layout; `flow-loads` check; load every map touched; play: take items one at a time and watch the owl change its hint. Write the PR draft in `docs\pr-drafts\`.

## Built (21 Nov)

Setting `kaepora_hint`, number 152 (151 was the highest used; nothing in any branch or worktree uses 152), toggle, default off, Hints section of the Hints page, after Character hints. Module `randomizer/core/src/kaepora.rs`; wiring is one stage ("kaepora hint", after "hints" so the language archives the other hints write are edited on top), a `Plan` field and a note in `slice/support.rs`, the setting in `settings/list.rs` and `menu.rs`, and a search alias in `web/assets/js/search.js`.

### How it works
- **Owl actor.** A new actor class `Npc_Kaepora_KokiriForest`, a copy of the Death Mountain Trail owl (`kaepora_pack.rs`), appended to `Banc/KokiriForest/KokiriForest_Child_Dynamic` (no params, no links, no presence flag). The copy has its own pack (the Trail owl's pack with its actor file renamed and pointing at its own schedule), its own AI schedule file, its own talk and near event rows in `EventSetting` and `GameAnalyzedEventInfo`, its own `ActorInfo`, `GameActorInfo` and tag rows, and its own event flow `Npc_Kaepora_KokiriForest` (the Trail flow with the flow and its first actor renamed). The new loose files get size-table entries sized from the game's own entries for the same file types. The Trail owl's pack, flow and schedule are not written by this setting (a test checks the build outputs; the flow file differs from the game's only by the settings that already edit it, such as the cutscene skips, and is identical with the setting off). The spot is two constants, `SPOT` and `FACING`, in `kaepora.rs`; the temporary spot is just east of the Lost Woods bridge, on the navmesh.
- **Which items.** At build the seed is rebuilt (`slice::starts::rebuild`) and `playthrough` run: the minimal required placements in sphere order. Each required item that has an `IsGet` flag (table `FLAGS`, one flag per tier for the progressive items, every name checked against `data/flags.json` by a test) becomes a link of the chain, the nth copy of a progressive item taking the nth tier flag. Up to 8 links (`MAX_LINKS`). Items without a flag (keys, trade items, tokens) are skipped. The place comes from `npc_hints::place` of the location the playthrough used, so a second copy of an item is hinted where that copy lies.
- **Talk flow.** The owl's own `Talk` entry now starts a chain of switches `IsGet.<item>`: owned goes on to the next one, not owned says that item's line (`OOTRRHint_NN`) and ends, all owned says the closing line (`OOTRRHint_Done`). Each line is a copy of one of the owl's own `EventTalk` events (`Event29`, `next=65535`, `IsCloseDialog`), so every path ends the way the game's own talk paths end. The switch is a copy of the owl's `Event4` (`EventQueryIsGameDataBool`); a true answer is case 0, as in the other flows. No saved flag is added.
- **Text.** New labels in `EventFlowMsg/Npc_Kaepora_DeathMountainTrail.msbt` of every language archive (the hint itself is English), laid out with `npc_hints::layout`, with the owl's lead codes copied from `Talk_03`.
- **The Death Mountain Trail owl.** Untouched: it keeps its own pack, flow and talk, including the flight to Kakariko. The new lines are added to its message file as new labels only; no existing label changes.

### Tested
- **Unit and game-file tests** (`kaepora::tests`, 9 of them, with `OOTRR_ROMFS`): the setting is off by default and the default settings string is unchanged; every item of `FLAGS` is a real item, has one flag per tier and every flag exists in `data/flags.json`; the chain of generated seeds has 1 to 8 links, no flag twice, laid-out text that parses, at most 3 lines per box, and is stable; each hint names the place the item was placed; every path of the edited `Talk` (for 0, 1, 5 and 8 links, with and without the base-location gate) is a chain of switches ending in `EventTalk` events whose `next` is 65535, one line per link plus the closing line, the vanilla events unchanged, a second edit refused; the language archive gains the lines and keeps `Talk_00`, `Talk_07`, `Near_00`; the map gains exactly one owl and refuses a second; a build stages the map, the flow and every language archive; the temporary spot lies on the navmesh floor. `cargo test -p ootrr-core` (1597 passed, 6 ignored) and the other crates pass.
- **Played in the game** (Longshot source build, one copy, a shadow game folder, fresh save, seed `kaepora1`, default settings plus `skip_intro`, `skip_cutscenes` all, `faster_chests`, `kaepora_hint`; by script, with screenshots read by eye):
  - Kokiri Forest loads with the owl in it. The owl is drawn east of the Lost Woods bridge. It disappears while Link stands right against it (near fade) and comes back when he steps away.
  - Talk at about 2 units: the box says "Kaepora Gaebora", "Hoo hoo! Heed my words, Link!", then "the Kokiri Sword awaits you in Kokiri Forest.", then "You will need it on your journey. Hoo hoo!". After the last A the talk ends (`event` false) and Link walks.
  - `give Kokiri Sword` (flag `IsGet.Weapon_Sword_Kokiri` 1): the next talk names the Boomerang in the Deku Tree. Setting the flag back to 0: it names the sword again.
  - All 73 flags of the table set to 1: the owl says only the closing line ("I have nothing more to tell you, Link"), the talk ends and Link walks.
  - The Deku Tree, the Lost Woods and Kokiri Forest again all loaded by teleport.
- **Re-check after the clone (21 Nov, same folder, fresh save):** the Kokiri Forest owl (class `Npc_Kaepora_KokiriForest`) loads and gives the hint (box "Know this: the Kokiri Sword awaits you in Kokiri Forest."), the talk ends and Link walks. The Death Mountain Trail owl, visited by teleport and warp, keeps its own talk ("you have grown ever stronger", "come near, and grab hold of my talons"), flew Link to Kakariko Village, and Link walked there. A build with the setting off writes the same Trail owl flow file as a build with it on.
- **Not tested:** the 3 to 7 items in between, one by one; adult Kokiri Forest (it has no owl by design); the Death Mountain Trail owl (it was not visited: with the base-location gate off, it gives the same hints); other languages; any runtime but Longshot; a seed with shuffled item pools beyond chests; the final spot, which the user will move by hand.
- The text first read "the Kokiri Sword awaits you" in lower case at the start of a box; the line now begins "Know this:".

### How to test in the game
Build a mod with `kaepora_hint` on and a fresh save. Warp to the east end of the Lost Woods bridge path in Kokiri Forest (about x -102, y 112.2, z -40.6 in `status` units), face west, press A about 2 units from the owl. Read the boxes. `give` an item or `flag set IsGet.<name> 1` and talk again. Change `SPOT` and `FACING` in `kaepora.rs` to move him.
