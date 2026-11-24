# OOTRR nightly: changes since 0.8.3-nightly.20261122.3

Covers the nightly branch from `134d9d8` (the source of nightly 0.8.3-nightly.20261122.3) to the current head: PRs #98 to #111, plus the repair commits on top of them.

Items can now be placed anywhere, with no safety refusals. Dungeon items found on the ground, in pots or in shops are now their own dungeon's items. Own-dungeon small keys are placed by the logic's key counts. Shops are reorganised by shelf. New goal and time-saver options. A Dungeon Keys tracker in Threads of Time. Generation is about a third faster. Several entrance shuffle and quest fixes.

**Most of this is not tested in the game yet.** Back up your saves.

## Place anywhere (#104, #98)

- **No placement refusals.** Every item can go at every spot. Instead of refusing placements, the build now fixes the cases those refusals used to cover:
  - **Spots that exist once per age** (one copy for child, one for adult) now give their item once. All copies share one actor hash, so the save marks the spot taken for both ages, as the vanilla game does for Mido's chests and Impa's Heart Piece. Scrubs and cows already give once through their own flags.
  - **Treasure Chest Game:** the first win chest opens only once, so its item can't be won twice. Any item can go there. An upgrade gives the tier the spoiler names. The preview shows the item.
  - **Boss hearts:** a Gold Skulltula Token there stands on the ground as a token pickup.
  - **Recovery Hearts, Fairies and single Deku Sticks** in chests and gifts now work. A heart heals you, a fairy gives a Recovery Heart, and a single stick gives the vanilla bundle of 3.
- **All 100 Gold Skulltula Tokens can be reached.** The logic now counts Magic Beans bought from the salesman, so bean-soil spots are in logic. This fixes "only 97 Gold Skulltula Tokens can be reached".
- Spots that were marked unsure were checked against their actors and given proper logic. For example, KF West Ledge Chest is reached by the Lost Woods bean ride and ZF Scarecrow Ledge Chest by Pierre.

## Dungeon items and keys

- **Dungeon item packs (#104):** a dungeon item on the ground, in a pot, crate, rock, grass or beehive, or for sale in a shop now stands as its own dungeon's item, for example "Small Key (Fire Temple)" in every language. Picking it up or buying it credits the dungeon the same way a chest does. Gold Skulltula drops keep the old system.
- **Packs no longer clip into the ground (#104):** dungeon item packs are lifted and resized like other pickups.
- **Keys from pots, rocks and grass are credited (#99):** a small key dropped by a pot in its own dungeon (reported with two Forest Temple lobby pots) never reached the key count. Boss keys in grass, pots or rocks weren't credited either. Both are fixed in every key mode.
- **Own dungeon small keys follow the logic (#104):** small keys no longer stay in their vanilla chests in the Fire, Water and Spirit Temples, and the Forest Temple's first key is no longer forced into the First Room Chest. The logic's key counts place every key inside its dungeon, whatever order the doors are opened in.
  - **Fire Temple, Own dungeon:** as in N64 OoTR, the Boss Key loop door starts unlocked, so the Hammer alone opens it. One Fire key (Highest Goron Chest) is replaced by a Rupee. Keysanity and Any dungeon keep the lock and all eight keys. *The unlocked door is not tested in the game.*
- **Shadow Temple jar key (Ontico):** Keysanity now shuffles it. Another item placed there skips the key pop.
- **Fire Temple Boss Key chest (#102):** a new test makes sure full builds leave the chest unlocked (the 0.8.2 bug).

## Threads of Time: Dungeon Keys tracker (#108)

- The tapestry menu has a **Dungeon Keys** quest with one entry per key dungeon: Forest, Fire, Water, Shadow and Spirit Temple, Bottom of the Well, Gerudo Training Ground and Ganon's Castle.
- Each entry reads **"Small Keys: 1 (3/5 Found)"** (keys held now, then keys found out of the dungeon's total in this seed) and shows the Boss Key as 0/1 or 1/1.
- The game's own quest panels are hidden from the menu only. Their quests still advance as usual.

## Shops (#105)

- Shop locations are named by shelf position, for example **KF Shop Right Side Top Left**.
- Deals take the right side first. A deal shelf's own goods join the item pool, so the Deku and Hylian Shields now count in the logic. The other shelves only swap goods with the same shelf position in other shops.
- A child can buy tunics in every build, not only with shopsanity. A tunic keeps its own 600-rupee price in any shop.
- A plando on any shop slot makes it a special deal.

## New settings (#105)

- **Rainbow Bridge: Medallions** is now available.
- **Ganon's Boss Key: LACS - Medallions:** Zelda gives the key in the Light Arrows Cutscene once you hold the set number of Medallions. *Experimental, untested in the game.*
- **Night Gold Skulltulas expect Sun's Song** (World): the logic expects night Gold Skulltulas only once you can play Sun's Song.
- **Free Scarecrow's Song** (Time savers → Side quests): start with Scarecrow's Song. Take out the Ocarina at one of Pierre's spots and he appears, with no song to record.
- **Skip the tower escape** (Time savers → Dungeons): after Ganondorf, go straight to the Ganon fight. Without this setting, the logic now needs the Longshot to finish.
- **Bunny Hood** can be a starting item.

## New checks and settings

- **Goron City spinning pot:** the Heart Piece check (GC Pot Freestanding PoH) is shuffled with the other Story and NPC rewards. The winning spin gives the placed item with the usual item get instead of the Heart Piece scene, and later wins give nothing more. *Untested in the game.*
- **Gerudo Membership Card setting (In Dev):** the card was already shuffled with Story and NPC rewards. It now has its own toggle, on by default, so existing seeds don't change. Turn it off to keep the card at the guard. "Rescue one" still keeps it vanilla.

## Minigames and side quests (#105)

- **Fishing:** any fish wins the fishing prizes (always on).
- **Skull Kid Ocarina minigame:** one round of 6 notes, and winning gives the check (always on).
- **Dampé's race:** one race gives both rewards. The Heart Piece lies on the ground across from Dampé, and a Song of Time block in the course works as a platform.
- **Gravedigging tour:** the first dig goes to the Heart Piece dig, with no second fee. The other digs keep the lottery.
- **Ruto's Letter** is always shuffled.
- **Windmill Heart Piece:** the logic no longer uses the Boomerang route, and Dampé's windmill access needs no Song of Time.
- **Lake Hylia Sun:** shooting the sun gives the placed item, and the Fire Arrows pickup stays hidden.

## Entrance shuffle and quests

- **Jabu-Jabu's swallow (#101):** with dungeon entrances shuffled and Jabu's Belly moved, being swallowed now takes you to the dungeon behind Jabu's mouth instead of always to Jabu's Belly. *Untested in the game.*
- **Jabu cleared through another door (#101):** the fish can still be fed and King Zora still moves, so the way behind Jabu's mouth stays open.
- **Open Jabu (#103):** a test now pins the quest step that keeps Jabu's mouth area loaded (the v0.6.2 report).

## Navi warps (#100)

- Navi warps are off while a minigame timer is running: Dampé's race, Zora diving, the horse race, the cucco search, the running man, the Biggoron trade timer and boss fight timers. Before, warping out of Dampé's race left its timer running.

## Presets and generation

- **Every built-in preset generates a seed (#110).** Easy, Full shuffle and Open world used settings this build refuses. Each now uses the closest settings that work.
- **Generation is about a third faster (#111):** a default seed goes from about 175 ms to about 117 ms, and a heavy seed from about 770 ms to about 505 ms. Seeds are the same as before.

## Web and files (#105)

- **Configuration page:** a new "Delete all other seeds" option, which also clears old seeds from before the per-seed folders.
- Longshot mods go under the title ID folder.

## Removed

- The Fire Arrow flame and Heart Piece glow on seed pickups were dropped (#104). The spin, lift and resize stay.

## Fixes on top of the merged PRs

- **The nightly branch did not compile.** The merge of #98 doubled code that #104 already held and brought back the comments #104 had removed. Those 13 files are back to how they were before the merge. No behaviour changes.
- **Upgrades on the ground, in pots, rocks, grass, at Gold Skulltulas and boss hearts** now give the next tier you don't have yet, decided when you pick them up, as chests already did. Before, the logic picked one tier for each spot, so finding copies in another order could give a second Goron Bracelet or jump to Golden Gauntlets, and Deku Stick or Deku Nut capacity always jumped to the top tier. These spots now show the first tier's model, and the get window names the tier you get. *Untested in the game.*
- **Shops:** with shop deals on, Blue Fire, Fish, Bugs, Fairies or the Red Potion could vanish from every shop when a deal took their shelf. They now move to another shelf.
- **Keysanity:** a dungeon item at LH Sun stopped the mod build. It now builds and credits its dungeon.
- **Longshot:** a rebuilt seed no longer carries the size table of its own previous build.
- The Night Gold Skulltulas setting appeared twice in the settings menu.
- 33 tests that still expected the state from before #105 now match it, and the seed pins are refreshed. Seeds differ from 0.8.3-nightly.20261122.3 because Ruto's Letter is always shuffled and shops changed.
