# 79. Full Wallets

Setting `full_wallets` (number 610, "Full Wallets", Logic section), off by default. It makes upstream's greyed "Full Wallets" entry live, with upstream's label and tooltip unchanged. Code: `randomizer/core/src/full_wallet.rs`, wired in `settings/list.rs`, `settings/menu.rs`, `slice/support.rs`, `slice/stages.rs`, `slice/edit.rs`.

## Requested

Upstream's menu shows "Full Wallets: Start with a full wallet. All wallet upgrades come filled with rupees." as a greyed stub. Make it work: with it on, each wallet the player gets, from a chest, a shop, a reward or the starting items, fills to that wallet's capacity (not above it, not to the old capacity), the rupee counter shows it, and no extra message appears. With it off, the build is unchanged. Data edits only.

## Interpretation

Behaviour follows Ship of Harkinian's "Full Wallets", which upstream's stub text is taken from (see Decisions). Matched from SoH:

- A new game starts with its wallet full, whatever that wallet is: 200 rupees, or 500 or 1000 when "Adult's Wallet" or "Giant's Wallet" is a starting item. SoH gives an over-large amount at new game that the wallet caps.
- Each upgrade (Adult's Wallet 500, Giant's Wallet 1000), wherever it is found, fills to its new capacity. SoH adds the capacity gained, which from a full wallet gives the same result; here the game's own refill column sets the count to the new capacity.

Not matched: SoH's child wallet (99) and Tycoon's Wallet (999) do not exist in the remake.

## Decisions

- **26 Nov (user):** Full Wallets behaves like SoH's Full Wallets (the stub is copied from SoH): the starting wallet starts full too, and each wallet upgrade fills to its new capacity. Settle any other behaviour question by SoH's behaviour (ideas only, no copied code).

## How it works

1. Wallet upgrades. A last build step (after the shop and Ice Trap steps, so it reads their output) goes through every event flow the mod writes, plus the Gold Skulltula reward flow `GatheringTokenGold`. After each event that gives `Item_Wallet_Medium` or `Item_Wallet_Large` (an action or sub-flow call naming the wallet in `ContentName`, `ActorName`, `ReveiveItem`, `ReceiveItem`, `TakenRewardActor` or `Actor`), it inserts `PouchSystemActor.EventTriggerIncreaseRupee` with `Value` 500 or 1000, then continues to the give's own next event. The flow has no "set to full" action, so the step adds the new capacity and relies on the game capping the count at the wallet's capacity. No message is added. Questions and display-only names (`DisplayItemActor`) are left alone, and the step skips a give that already has its rupees, so running it twice changes nothing.
2. The starting wallet. With the setting on, the new-game save defaults set `PlayerStatus.CurrentRupee` to the capacity of the best wallet the new game holds: 200, or 500/1000 with the starting-item wallets.

Give paths covered: chests (each tier's give in `Dm_SY_Treasurebox` and `Dm_SY_TreasureboxSmall` from the progressive wrap), flow rewards whose grant the build rewrites (people, Gold Skulltula rewards, other event rewards), the vanilla Kakariko Gold Skulltula rewards, and shop deals sold tier by tier (the shelf hook adds each tier with `EventTriggerAddOrRemovePouchContent`).
Not covered: a wallet lying on the ground or in a pot (`Obj_Wallet_*` pickups, given by the pickup routine without a named give in a flow), and Archipelago grants, which come through the bridge.

Logic is unchanged: placement and shop price rules count `Progressive_Wallet` items, never rupees, so the spoiler and item placement are the same with the setting on or off.

Off: neither step runs, so nothing is written.

## History

The first version (commit ae95b3da) set `IsItemStockMaxIfGet = true` on the wallet rows of `RSDB/PouchActorInfo.Product.111.rstbl.byml.zs`, the column that fills every bag on get. Game test on that build (26 Nov, scripted: Mido's top left chest and the man on the Kakariko roof, Adult's and Giant's Wallet): the count stayed 37 -> 37 in all four, as in the off control. The game ignores the column for rupees, so it was dropped.

## Unproven (game test needed)

- That `EventTriggerIncreaseRupee` caps at the new capacity (37 + 500 should read 500, not 537), and that the counter shows the new count straight away.
- Whether the start-module push of the vanilla wallet, which runs after the save defaults, clamps or resets the start count.

## Tested

- Unit tests (`cargo test -p ootrr-core full_wallet`, 4 of 4 passed with `OOTRR_ROMFS` set): only upgrade-wallet gives count (not the small wallet, not questions, not `DisplayItemActor`); in the real `GatheringTokenGold` flow exactly 2 gives get 500 and 1000, each rupee action leads to the give's old next event, every other event is unchanged, a second run changes nothing and the flow round-trips; in both real chest flows the progressive wallet wrap gets exactly 2 rupee actions; a new game's count is 200, 500 or 1000.
- `cargo test --workspace` on 4fe247ee with `OOTRR_ROMFS` set: 2416 passed, 0 failed.
- Built mods (commit 4fe247ee, same seed text, time savers, plando: both Progressive Wallets at "KF Midos Top Left Chest" and "Kak Man on Roof"): `ootrr flow-loads` finds no missing loads on four mods (on, off, Adult's Wallet start, Giant's Wallet start). On differs from off in exactly 4 files: the small chest flow and the roof man's flow (`Npc_Hylia_001_Adult`), each with 2 rupee actions, `GameData/GameDataList.Product.110.byml.zs` (the start count) and the resource size table.
- Setting off: the seed file and every mod file are byte-identical to a build of nightly 2746d256 with the same preset.
- Not played. The first version's game test is under History.

## How to test in the game

The tester plan is `playtest/full-wallet/PLAN.md` in the local test folder. By hand:

1. Build a seed with "Full Wallets" on and "Giant's Wallet" not a starting item.
2. New game: the counter reads 200.
3. Get Adult's Wallet (chest, shop or the Kakariko Gold Skulltula reward): the counter rises to 500, with only the usual "You got" box.
4. Get Giant's Wallet: the counter rises to 1000.
5. Repeat with the setting off: a new game reads 0 and the wallets add no rupees.

## MR Drafts

### Initial MR Draft
**Title:** New Logic > Full Wallets Feature: wallets come filled with rupees

The "Full Wallets" option on the settings page was greyed out and did nothing.

A player who wanted to start rich, or skip grinding rupees after each wallet upgrade, had no way to do it.

This PR makes the option work: a new game starts with a full wallet, and Adult's Wallet and Giant's Wallet come filled when you get them.

Not yet played in the game; the tester plan is in the feature doc.

For details about implementation and testing, see difficulty-mod/docs/features/79-full-wallets.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
