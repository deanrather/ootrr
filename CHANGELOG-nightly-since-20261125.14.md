# OOTRR nightly: changes since 0.9.0-nightly.20261125.14

Archipelago fixes from the first rooms: generating with rewards at the end of dungeons, building a slot alone, checks sent for the wrong spot, and playing on Linux. Covers the nightly branch from `2746d25` (the source of nightly 0.9.0-nightly.20261125.14).

**Make your player file again and regenerate the room** to get these. The client part also works in rooms made before.

## Archipelago

- **Breaking one pot no longer sends another pot's check.** Some pots share their map hash with another pot: KF House of Twins Pot 1 with the Know it All House pots, and Barinade Pots 1 to 5 in Jabu-Jabu's Belly with each other. Breaking one counted as all of them. A flag or key that more than one location would use no longer counts for any of them, so these 7 pots (Full shuffle only) hold only your own items. Every other preset is unchanged.
- **The four Gerudo jail keys** (when hideout keys are shuffled) were seen through a flag that only the key itself sets, so a jail holding another item never sent its check. Those jails now hold only your own items, like the song spots.
- **Rooms with Dungeon Rewards at the end of dungeons generate again.** 0.9.0-nightly.20261125.14 made the boss and Rauru spots hold only your own items, but they only take Stones and Medallions, so the fill could leave them empty ("No more spots to place"). Those spots are now filled first, with your own rewards.
- **A slot no longer fails to build with "places key items at pickup spots out of upgrade tier order".** Some spots, such as Song from Royal Familys Tomb, hand out one fixed tier of an upgrade, and the room could put a Bomb Bag or other upgrade there in the wrong order. It showed up most in rooms with only OOTRR, but could happen in any room. Upgrades now stay off every spot the build gives a fixed tier.
- **The OOTRR Client can read the game on Linux.** It always said "game not running" there: only Windows was supported. It now finds Longshot natively or under Wine or Proton. Most Linux systems block one program from reading another's memory. If the client says so, run `sudo sysctl kernel.yama.ptrace_scope=0` once (it lasts until the next restart) and reconnect. *Not checked on a real Linux game yet.*
