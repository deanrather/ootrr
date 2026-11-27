# On-get hook: run code when Link gets an item

`on_get` is one shared hook for "when Link gets item X, do Y". A feature declares it once, and the hook adds the question and the call at every get path in the event flows. Use it instead of listing flow events in a feature's own module.

Status: a proposal for the other developers. It is not merged.

## The problem

An item reaches Link by different paths, and each path holds the item's name somewhere else: an event argument, `UI_MessageTargetActorName`, `Shop_SelectItemName`, or a fixed string in a script. A feature that runs code at a get has to find every path, ask for the item's name at each, and keep the order right against the other things that edit the same flow. A path left out shows only when someone gets the item that way.

## Declaring a hook

```rust
pub const WALLETS: [&str; 2] = ["Item_Wallet_Medium", "Item_Wallet_Large"];
pub const ON_GET: OnGet = OnGet { title: "Full Wallets", on: |p| p.full_wallet, tag: fill::PREFIX, items: &WALLETS, run: fill::fill, start: start_sets };
```

| Field | What it says |
|---|---|
| `title` | the name in the build report and in errors |
| `on` | `fn(&Plan) -> bool`: whether a build runs the hook |
| `tag` | the prefix of every event the hook adds, unique to the feature |
| `items` | item actor names. The hook also asks for the `Pouch_`, `Obj_` and progressive pickup names of each |
| `run` | `fn(&mut Flow, At, u16) -> Res<u16>`: add the events that do the work, ending at the given event, and return the first one |
| `start` | `fn(&Kit) -> Vec<Set>`: the new-game save writes for starting with the item. Return an empty list for none |

`run` is told where it is. `At::Gift` is a get with no menu open. `At::Sale` is a shop or Business Scrub sale, where the rupee counter is already open.

Add the hook to the list `on_get::ALL`. Nothing else in `slice/` changes.

## Where the hook runs

| Path | Flow and event | Name asked | `At` |
|---|---|---|---|
| chest, most people and rewards | `GetPouchContent` `Event0` | the event's `Actor` argument | `Gift` |
| story give | `Dm_SY_GetInEvent` `Event124` | the event's `Actor` argument | `Gift` |
| Business Scrub sale | `Dm_SY_GetInEvent` `Event430`, `Event442` | the event's `Actor` argument | `Sale` |
| ground pickup, pot, crate, drop | `Dm_SY_Get` `Event35`, `Dm_SY_Get_WithoutPlayer` `Event0` | `UI_MessageTargetActorName` | `Gift` |
| shop shelf | every `EventTriggerGetItemAtShop` in `DisplayShop` | `Shop_SelectItemName` | `Sale` |
| a script that adds a listed item by name | any other flow | none, the name is fixed | `Gift` |

On a shop shelf that sells a progressive tier, the shelf adds the tier with a script call after the `Event281` and `Event543` purchases (`shelf::hook`). The hook runs after that call, so the tier is owned when `run` starts, also when Ice Traps has put its own questions between the purchase and the call. The shelf adds no tier by script after `Event123`, so there the hook runs right after the purchase event.

Other literal adds inside `DisplayShop` are not hooked, so a shelf purchase runs the code once.

## Not covered

- Items that go into a bottle. These use other events.
- A repeat pickup that plays no get event, and every add the game's code makes without an event flow.
- Dungeon items placed as window-only gives. These write a dungeon credit and never add to the pouch.
- The item's look, and timing or display problems while the effect runs.

## The coverage check

`on_get::cover::run` takes a patch and walks these get paths in the game's event flows: each supported or gated flow location, both chest flows, the Archipelago delivery, both pickup flows and the three shop purchase events. `cargo test -p ootrr-core on_get -- --nocapture` runs it for every hook in `on_get::ALL`, with a plain and a tiered shop shelf, and fails on a path that is not reached.

It takes any patch, so it also checks a feature that does not use the hook. Results when run on 27 Nov:

| Patch | Missed |
|---|---|
| Full Wallets on this hook, plain and tiered shelf | nothing |
| Full Wallets before the hook, plain shelf | the 3 shop purchase events |
| Full Wallets before the hook, tiered shelf | `Event123` |
| Magic Bean Pack before its shop shelf fix | the 3 shop purchase events |
| Magic Bean Pack with that fix | nothing |
| Ice Traps, its fixed hooks only | the 2 script adds (Saria's gift, the Ocarina of Time). Ice Traps covers these with a mark after each placed location, which this run does not include |

Limits of the check:

- A path counts as reached when an event the patch added lies on it. For a pickup and a script add, an added event earlier in the same flow also counts.
- It does not show that the game takes that branch, which item names the added events ask for, or that the effect works.
- 5 flow locations whose flow the build makes (cows and the like) are listed as not checked and are not walked.

## What changed for Full Wallets

Full Wallets is the first user. Its own wiring (`full_wallet/sites.rs` and the build step in `full_wallet.rs`) is gone, and its start-with write comes from the hook's `start`.

One behaviour changes: in `DisplayShop` the fill now follows the purchase event itself instead of the script add of the tier. Every other event flow is built as before.

## Testing

- Unit and game-file tests: `cargo test -p ootrr-core on_get` and `cargo test -p ootrr-core full_wallet`.
- Byte comparison and game test: see the "Checked" lines of the commit message.
- In the game: with Full Wallets and `shuffle_shops` on, own no upgrade wallet, hold 200 rupees and buy a shelf wallet. The count ends at 500. With Full Wallets off, the same purchase ends at 200 minus the price.
- Not yet tested in the game: a shop wallet in a seed that also has a shop Ice Trap. A test covers the event order.

## Commit message

Add one shared on-get hook, and move Full Wallets onto it

Code that should run when Link gets an item had to find every get path in
the event flows itself. A path could be left out unnoticed: the Magic Bean
Pack at first gave no extra beans from a shop shelf (found by reading the
code).

## Cause

Each get path holds the item's name somewhere else, and each feature
listed the flow events in its own module.

## Change

- `on_get::OnGet` declares the items, the code to run and the start-with
  save writes. `on_get::ALL` lists the hooks.
- `on_get::cover` walks the get paths in the game's event flows, shop
  shelves included, and reports those that carry none of a patch's events.
- Full Wallets is declared as a hook. Its own flow wiring is removed.
- With Full Wallets on, `DisplayShop` fills after each purchase event
  rather than after the script add of a tier. Builds with it off are
  unchanged.

## Checked

- cargo test --workspace: 2450 passed, 0 failed, 13 ignored.
- Default output identical to nightly 4fa6e5a6 (286 files).
- With Full Wallets on, builds differ from the build before the hook only
  in `DisplayShop` and its size table entry. Compared with Full Wallets
  alone and with shop shuffle. With Full Wallets off and shop shuffle on,
  no file differs.
- Not played in the game.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
