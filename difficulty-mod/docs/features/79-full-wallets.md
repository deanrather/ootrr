# 79. Full Wallets

Setting `full_wallets` (number 610, "Full Wallets", Logic section), off by default. It makes upstream's greyed "Full Wallets" entry live, with upstream's label and tooltip unchanged. Code: `randomizer/core/src/full_wallet.rs` and `full_wallet/` (`fill.rs`, `sites.rs`), wired in `settings/list.rs`, `settings/menu.rs`, `slice/support.rs`, `slice/stages.rs`, `slice/edit.rs`.

## Requested

Upstream's menu shows "Full Wallets: Start with a full wallet. All wallet upgrades come filled with rupees." as a greyed stub. Make it work: with it on, each wallet the player gets, from a chest, a shop, a reward or the starting items, fills to that wallet's capacity (not above it, not to the old capacity), the rupee counter shows it, and no extra message appears. With it off, the build is unchanged. Data edits only.

## Interpretation

Behaviour follows Ship of Harkinian's "Full Wallets", which upstream's stub text is taken from (see Decisions). Matched from SoH:

- A new game starts with its wallet full, whatever that wallet is: 200 rupees, or 500 or 1000 when "Adult's Wallet" or "Giant's Wallet" is a starting item.
- Each upgrade (Adult's Wallet 500, Giant's Wallet 1000), wherever it is found, tops the count up to its new capacity: 5 rupees held and Adult's Wallet received ends at 500.

Not matched: SoH's child wallet (99) and Tycoon's Wallet (999) do not exist in the remake.

The user's failure list (Decisions, 26 Nov 10:35 and 10:36) is the acceptance list: the rupees show at once with the same counter popup as rupees picked up off the ground, every possible wallet source works, the count is topped up to the limit, and no stray message or dialog appears. Navi's "Your wallet is stuffed." is wanted, not stray (see Decisions, 26 Nov 18:11).

## Decisions

- **26 Nov (user):** Full Wallets behaves like SoH's Full Wallets (the stub is copied from SoH): the starting wallet starts full too, and each wallet upgrade fills to its new capacity. Settle any other behaviour question by SoH's behaviour (ideas only, no copied code).
- **26 Nov 10:35 (user):** The user on what counts as failure for Full Wallets, even if it works: (1) the rupees "should show immediately, and the same on-screen status popup should occur as what happens when you pickup rupees off the ground"; (2) "a wallet from any possible source should be tested" (chest, gift, shop, new-game start, Skulltula reward, and any other), and "if the playtester doesn't know how to use a shop, I can teach"; (3) the wallet is topped up to its limit, not added to: "if the player has 5 rupees and they get a wallet that lets them hold 500, they should get 495 rupees"; (4) "there should be no stray/errant messages or dialog. this is never acceptable." This replaces the session's own list.
- **26 Nov 10:36 (user):** The user, on the first side-by-side video (`<playtest folder>\side-by-side-demo\results\before-after.mp4`, Full Wallets off against on, the roof man's gift): "that particular example illustrates how you can't even tell you got money, 'cause there's no on-screen indication". So the build of commit 4fe247ee fails the first point of the failure list as seen by the user: a wallet pickup that fills the wallet shows nothing on screen. The fix needs the ground-pickup rupee popup; the same pair, rerecorded past the gift, is its before-and-after.
- **26 Nov 18:11 (user):** Navi's "Your wallet is stuffed." stays, with Full Wallets on too: "navi's 'stuffed' message is lovely. leave it". The session's default (turned off everywhere) is void.

## How it works

### Where a wallet is given

Every wallet give in the game ends in one of four pouch adds. The step hooks each one once, so one get gives one fill:

| Give point | Used by | Hook |
|---|---|---|
| `GetPouchContent` event `Event0` (`EventTriggerAddOrRemovePouchContent`, `Actor=arg:ActorName`) | chests (both chest flows, every tier and fixed contents), people's gifts and handovers (`Dm_SY_GetInEvent:GetInEvent*`, `NpcCommon_HandOver_*`), Gold Skulltula rewards (`TakenReward_Common_Event*`), Great Fairies, minigame prizes, frogs, Dampé, cows and the potion shop (their bottle routine is swapped for `GetInEvent_NotTurn_NotNpc` / `GetInEvent_NotTurn` for non-bottle items), Archipelago mailbox gives (`Dm_SY_GetInEvent:GetInEvent_NotTurn_NotNpc`) | name check, then popup fill |
| `Dm_SY_GetInEvent` `Event124` (same add) | `GetInEvent_Goron`, `GetInEvent_Zeldaletter` | name check, then popup fill |
| `Dm_SY_GetInEvent` `Event442`, `Event430` (same add) | scrub sales (`GetInEvent_NotTurn_2DShop`, `DekuShield_GetAndEquipCheck`), with the shop counter already open | name check, then shop fill |
| `Dm_SY_Get` `Event35`, `Dm_SY_Get_WithoutPlayer` `Event0` (`PouchContentGuestPerformer.EventTriggerAddOwnToPouch`) | pickups the game hands over with a get scene: items on the ground, in pots and crates, Gold Skulltula token spots, freestanding Pieces of Heart, boss hearts, keys, every `Obj_Wallet_*` pickup; with `crate::pickup_tiers` the end of every tier path too | name check, then popup fill |
| a pouch add with a literal `Actor` of `Item_Wallet_Medium` / `Item_Wallet_Large` and `Num` above 0, in any other flow the build writes | cutscene gifts with a bare add, the shop shelf's tier give (`DisplayShop`, `crate::shelf::hook`) | popup fill (shop fill in `DisplayShop`) |

The name check writes the add's actor argument into `UI_MessageTargetActorName` (`GameDataActor.EventTriggerSetGameDataString64`, the write the pickup tier code already uses), waits 1 frame so the write lands, and asks `EventQueryIsGameDataString64` for each wallet name: `Item_Wallet_*`, `Pouch_Wallet_*`, `Obj_Wallet_*` and the pickup tier names (`crate::pickup_tiers::names`). Any other item goes straight on to the add's old next event. In the two pickup flows the name is the one the game's own `EventTriggerSetOwnNameToMessage` (or the tier path's named add) wrote, so there is no write.

### The fill

The popup fill is the sequence the game uses to show rupees going in or out during an event (`Get_Epona` events 132, 131, 496, 133, and `GetPouchContent:OpenRupeeUIIfNeed` / `CloseRupeeUIIfNeed` for rupees from a chest):

1. `UISystemActor.EventOpenRupeeDisplay` (`IsWaitEnd`, `IsWaitOpenAnim` true, `IsNeedMaxNum`, `IsWaitChoiceBtnIn` false, as in `GetPouchContent`).
2. `PouchSystemActor.EventTriggerIncreaseRupee` with `Value` 1000.
3. `UISystemActor.EventWaitStableNumDisplay` (the counter finishes counting).
4. `EventSystemActor.EventWait` 60 frames (`GetPouchContent` holds the counter 60 frames after a rupee that does not fill the wallet).
5. `UISystemActor.EventCloseRupeeDisplay`.

The shop fill is steps 2 and 3 only: in a shop or a scrub sale the shop's own rupee counter is open (the game waits on it with `EventWaitStableNumDisplay` after paying), so opening a second one is avoided.

The flow has no "set to full" action. The add is 1000, the largest capacity, and the game caps the count at the wallet's capacity (game result: with the cap at 500, 37 + 500 read 500, never 537). So 5 rupees and Adult's Wallet end at 500: 495 go in.

The counter layout is `NumDisplay_00` in `UI/Layout2Archive/Common` (texts `T_Rupee_00`, `T_RupeeTotal_00`), the one rupee counter in the layout archive. That the flow action and a rupee picked up off the ground draw it the same way is the likely reading, not proven from data: the ground pickup draws it from code. Only a game test shows it.

### The Navi remark

Navi says "Your wallet is stuffed." when the rupee count reaches the cap. It is the game's own reaction (balloon entry `MaxRupee000`, condition `MaxRupeeSucessParam`), so it can also follow a wallet fill with the setting on. The setting does not change the Navi list, and the remark stays (see Decisions).

### The starting wallet

The new-game save defaults set `PlayerStatus.CurrentRupee` to the capacity of the best wallet the new game holds: 200, or 500/1000 with the starting-item wallets. A new game's rupee cap `PouchContentMaxNum.Rupee` defaults to 0 (`GameDataList`, hash 0xe0010a50, `ResetTypeValue` 32), and the start module pushes `Item_Wallet_Small`, then any starting wallet, after the defaults are set. Whether any of those pushes clamps the count to the cap at that moment is code, so only a new game shows it. No popup is added at a new game: nothing is received on screen there.

### Logic and off

Logic is unchanged: placement and shop price rules count `Progressive_Wallet` items, never rupees, so the spoiler and item placement are the same with the setting on or off. Off: no step runs and nothing is written.

## History

- First version (commit 33595f55): `IsItemStockMaxIfGet = true` on the wallet rows of `RSDB/PouchActorInfo.Product.111.rstbl.byml.zs`. Game test (26 Nov, scripted, Mido's top left chest and the man on the Kakariko roof): 37 -> 37 in all four, as in the off control. The game ignores the column for rupees.
- Second version (commit 4fe247ee): a bare `EventTriggerIncreaseRupee` of 500 or 1000 after each give that named a wallet in a flow the build wrote, plus `GatheringTokenGold`. Scripted 6 of 6 PASS on the count (chest and roof man, Adult's and Giant's, 37 -> 500 and 37 -> 1000, off stays 37). The user's review of the side-by-side video failed it: no on-screen sign of the rupees. It also missed wallets on the ground, in pots, at Gold Skulltula token spots and in a chest given by fixed contents (the name was an argument, not in the flow). The tester saw Navi's full-wallet remark after the Giant's Wallet chest. The third version (this one) replaces it.

## Hands-on

26 Nov, by the user:

- Chest (Mido's house), commit 19989b10, which also turned the Navi remark off: PASS. "mido's chest gave me a giant wallet and filled it and was perfect."
- A wallet given by a person, commit 02d4f5c2: PASS. "it was perfect."
- A wallet bought in a shop, commit 02d4f5c2: PASS. "looked perfect."
- The Gold Skulltula House reward, commit 02d4f5c2: PASS. "skultulla house was perfect."
- A wallet lying on the ground, commit 02d4f5c2: PASS. "ground perfect."
- A new game with the Adult's Wallet as a starting item, commit 02d4f5c2: PASS. "new-game success".

The scrub, the Great Fairy, a pot and a new game without a starting wallet are not yet played by hand (for the pot and the Great Fairy see the scripted test below).

## Scripted game test (commit 02d4f5c2)

26 Nov, 11 of 12 planned tests run, rupees set to 5 before each get. The test was partial: its own faults left the chest scenes without screenshots.

| Source | Setting | Rupees | Counter seen in screenshots | Navi's "Your wallet is stuffed." |
|---|---|---|---|---|
| Person's gift (Adult's, Giant's) | on | 5 -> 500, 5 -> 1000 | yes | not seen |
| Ground pickup | on | 5 -> 500 | yes, counting up | seen after the get |
| Gold Skulltula reward | on | 5 -> 500 | yes | not seen |
| Chest (Adult's, Giant's) | on | 5 -> 500, 5 -> 1000 | not tested | seen after the get |
| Person's gift, ground pickup, chest | off | stays 5 | ground: none; gift and chest: not judged | not seen |

No text other than the normal get boxes and the Navi remark was seen. A video of the Giant's Wallet chest with the setting on, made afterwards, shows the "You got a giant's wallet!" box, then the counter going from 5 to 1000, then the Navi remark. The Gold Skulltula reward with the setting off was not run.

## Scripted game test after the nightly merge

26 Nov, built from the merged code. A pot in the Know-It-All Brothers' house, child, no wallet upgrade, 5 rupees: the Adult's Wallet filled it to 500, the counter counted up inside the get scene (+495), and Navi said "Your wallet is stuffed." afterwards. No setting-off run of the pot.

The Hyrule Castle Great Fairy, child: the Adult's Wallet filled it from 60 to 500 and the counter read 500 in the reward scene. This was the 50 rupee offering path (the test save counted as already rewarded); the first-visit reward was not run. No setting-off run.

The scrub sale was not tested: the Hyrule Field grotto scrub asked for a password, and the Lost Woods bridge scrub attacked and was not defeated by the script, so nothing was bought.

## Unproven (game test needed)

- The counter popup in a chest scene with the setting off, and in a person's gift scene with the setting off (no counter is expected).
- The scrub sale: that the shop fill shows on its open counter. Not played.
- The Great Fairy's first-visit reward: not run (only the offering path).
- The starting count with no starting wallet, and with "Giant's Wallet": not played (see "The starting wallet").
- Archipelago: covered offline only (the mailbox gives through `GetInEvent_NotTurn_NotNpc`); no Archipelago build was made.

## Tested

Merged with nightly 52db1a16 (merge commit fa14bb97, conflicts in `slice/edit.rs` and `slice/support.rs` resolved by keeping both sides), all runs with `OOTRR_ROMFS` set: `cargo test --workspace` 2445 passed, 0 failed, 13 ignored; audits clean; the default preset builds byte-identical to nightly 52db1a16 (286 files). No game run after this merge.

Earlier, merged with nightly 240af8b2 (merge commit ea264d4a): `cargo test --workspace` 2437 passed, 0 failed, 13 ignored; audits clean; the default preset and the three setting-off plandos build byte-identical to nightly 240af8b2. The game results below are from commit 02d4f5c2, before that merge. The merge brought a nightly change that makes upgrade pickups replay their get event; wallet pickups have not been played since.

Before the merge, code commit 02d4f5c2:

- `cargo test -p ootrr-core full_wallet`: 9 of 9 passed.
  - each of the 4 give-point adds gets: the name write with its own argument, a 1-frame wait, one question per wallet name, all hits to one fill, the last miss to the old next event, the fill in the right shape (popup or shop) with `Value` 1000. Every other event is unchanged and a second run changes nothing.
  - both pickup flows, alone and after `crate::pickup_tiers::patch` for the wallet family: 1 and 5 events led to the add's next event before, all 5 lead to the check after, none bypasses it. The flows round-trip.
  - a literal wallet add fills (shop fill in `DisplayShop`, popup elsewhere), a removal (`Num` -1) and the small wallet do not.
  - every one of the 915 game flows: only the 4 give-point files change (6 sites in all), each round-trips.
  - every grant of every supported or gated flow location (data/flow_locations.json) reaches a give point, by category: chest (both chest flows), npc_gift, trade, minigame, gs_reward, great_fairy, scrub_sale, shop, bottle, cutscene_gift, dungeon_reward, cow and potion_shop (with the build's bottle swap). The 5 cow clone flows are made by the build from the cow template. The Archipelago mailbox give reaches `GetPouchContent` too.
  - a build with the setting on writes no Navi list, and the list is not in the game-package unpack list (this test fails on commit 19989b10).
  - the capacities 200, 500, 1000 match `AfterExpansionMaxNum` in the pouch table, the fill (1000) is at least every capacity, and a new game's count is 200, 500 or 1000.
- `cargo test --workspace` on commit 02d4f5c2: 2421 passed, 0 failed.
- Mods built from commit 02d4f5c2, one per give category, two Progressive Wallets placed by plando where the category allows: chest and person (`on`), shop shelf (`shuffle_shops` deals_4) and freestanding Piece of Heart spot, pot (`shuffle_pots` overworld) and the Kakariko 10 Gold Skulltula reward, scrub (`shuffle_scrubs_a`, one wallet), Great Fairy (one wallet), minigame and cow (`shuffle_cows`), token spot (`shuffle_skulltulas` matched) and crate (`shuffle_crates` overworld), and both starting wallets. `ootrr flow-loads` finds no missing loads on all 12 mods, the 3 setting-off ones included. On against off: 6 files differ (the 4 give-point flows, `GameDataList`, the size table), 7 with the shop (`DisplayShop`, 2 shelf tier adds).
- Setting off: the seed file and every mod file are byte-identical to nightly 2746d256, with the chest and person plando, the shop and freestanding plando and the pot and Gold Skulltula reward plando.

## How to test in the game

The tester plan is `playtest/full-wallet/PLAN.md` in the local test folder. By hand:

1. Build a seed with "Full Wallets" on, no wallet a starting item. New game: the counter (pause menu) reads 200.
2. Hold 5 rupees. Get Adult's Wallet anywhere (chest, person, shop, a pot, the ground, the Gold Skulltula House). Expected: after the "You got" box the rupee counter opens, counts up to 500 and closes. Navi may then say "Your wallet is stuffed."; no other text appears.
3. Get Giant's Wallet. Expected: the counter counts up to 1000.
4. Repeat with the setting off: a new game reads 0, the wallets add no rupees and no counter opens.

## Commit message

Full Wallets is now a setting (Logic > Full Wallets), off by default

With Full Wallets on, a new game starts with its wallet full, and each Adult's Wallet or Giant's Wallet fills to its capacity with the rupee counter on screen. With it off, nothing changes. Before this change the setting was a greyed entry on the settings page.

## Cause

The setting was a stub. The pouch table column that fills bags on get (`IsItemStockMaxIfGet`) does nothing for rupees, and no flow action sets the count to the cap.

## Change

- `full_wallet::sites` hooks the pouch adds every wallet give ends in: `GetPouchContent` `Event0`, `Dm_SY_GetInEvent` `Event124`, `Event442` and `Event430`, the pickup adds of `Dm_SY_Get` and `Dm_SY_Get_WithoutPlayer` (and the ends of the pickup tier paths), and literal wallet adds in the flows the build writes. A question on `UI_MessageTargetActorName` lets only wallet names through.
- `full_wallet::fill` opens the rupee counter, adds 1000 rupees, waits for the counter, holds it 60 frames and closes it, as the game does around rupee gives in events. The game caps the count at the wallet's capacity. In shops and scrub sales the shop's open counter is used.
- The new-game save defaults set `PlayerStatus.CurrentRupee` to the best starting wallet's capacity.
- Navi's "Your wallet is stuffed." remark is unchanged, so it can follow a wallet fill.
- The settings hunks add the toggle last and off by default, so the default settings string and seeds are unchanged.

## Checked

- `cargo test --workspace`: 2445 passed, 0 failed, with 9 `full_wallet` tests among them.
- Default settings: every file built is byte-identical to nightly 52db1a16. Setting off: byte-identical to nightly 240af8b2 with three more (chest and person plando, shop deals and freestanding plando, pot and Gold Skulltula reward plando).
- On: `ootrr flow-loads` finds no missing loads on 9 mods with the setting on.
- Game: a person's gift, a shop purchase, a ground pickup, the Gold Skulltula reward, a new game with the Adult's Wallet and, on an earlier version, a chest were played by hand and passed. Scripted after the nightly merge: a pot and the Great Fairy's offering reward filled the wallet. The scrub sale is not yet seen.

Setting number 610.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
