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

1. Wallet upgrades. The pouch table `RSDB/PouchActorInfo.Product.111.rstbl.byml.zs` already marks every bomb bag, bullet bag, nut bag, stick bag and quiver with `IsItemStockMaxIfGet = true`, which fills that bag on get. No wallet row has it. With the setting on, a last build step adds `IsItemStockMaxIfGet = true` to every row whose `MaxNumGameData` is `Rupee` and whose `AfterExpansionMaxNum` is above 200. In the vanilla table that is `Item_Wallet_Medium`, `Item_Wallet_Large`, `Pouch_Wallet_Medium` and `Pouch_Wallet_Large`; in a seed it also covers the rows the build clones from them (the `Obj_Wallet_*` pickup packs, shop shelf actors). Ice-trap rows (`OOTRR_*_Cold`) are left alone. The step runs after the shop and ice-trap steps, so it reads their output; it grows the resource size table entry when the table grows.
2. The starting wallet. With the setting on, the new-game save defaults set `PlayerStatus.CurrentRupee` to the capacity of the best wallet the new game holds: 200, or 500/1000 with the starting-item wallets.

Logic is unchanged: placement and shop price rules count `Progressive_Wallet` items, never rupees, so the spoiler and item placement are the same with the setting on or off.

Off: neither step runs, so nothing is written.

## Unproven (game test needed)

- Whether the engine honours `IsItemStockMaxIfGet` for rupees (`MaxNumGameData = Rupee`). It does for bags. If it does not, the fallback is a flow splice after the get (`EventTriggerIncreaseRupee`, as in `progressive.rs`), planned for a second round.
- Whether it fills to the new capacity on the second upgrade (500 to 1000).
- Whether the start-module push of the vanilla wallet, which runs after the save defaults, clamps or resets the start count.
- Archipelago grants arrive through the bridge, not the pouch add, and are not expected to fill.

## Tested

- Unit tests (`cargo test -p ootrr-core full_wallet`, 3 of 3 passed with `OOTRR_ROMFS` set): only wallet rows above 200 rupees gain the column and no other column or row changes; the step is idempotent; a new game's count is 200, 500 or 1000; against the real game table the constants 200/500/1000 match `AfterExpansionMaxNum`, the bomb bag carries `IsItemStockMaxIfGet = true` as a boolean, exactly 4 rows change, and the table round-trips.
- `cargo test -p ootrr-core settings`: 113 passed, 0 failed.
- Not played. No game test has run.

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
