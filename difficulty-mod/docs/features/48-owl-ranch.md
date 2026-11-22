# Owl ride to Lon Lon Ranch

## The request (23 Nov)

"Another owl warp from Jabu's fountain to Lon Lon Ranch." The owl stands at Zora's Fountain beside Jabu-Jabu; talking to him flies Link to Lon Lon Ranch, one way, child only.

Measured by the user in the game: owl spot in `ZoraFountain_Child` (17.1, 105.2, 15.4), facing -174; landing spot in `HyruleField_Child` (-97.3, 150.5, 198.4), facing 170 (Lon Lon Ranch is part of the child Hyrule Field map).

## Interpretation

One more row in the ride table of `38-owl-warps.md` (`RIDES` in `randomizer/core/src/qol/owl.rs`), behind its own setting so it can be used without the other owl changes.

## How it works

- Setting `owl_ranch`, number 204, "Owl ride to Lon Lon Ranch", default off, World page, section Travel (next to More owl rides).
- Nothing in the game files has an owl at Zora's Fountain, so the Death Mountain Trail owl actor is placed there, like the Kakariko owl. The map check in the Death Mountain owl's event flow (`Sequence_CurrentBanc`) picks the ride by map; a new question is added to the owl's message file in every language archive.
- The landing has no start point in the field, so the map change action is given coordinates: `NextStartPos` empty, `NextPosition` and `NextStartRot` set (the coordinates mode of the teleport work, `teleport_flow.rs`). The rotation unit is degrees (the same as `tp-build` defaults).
- The ride uses the same yes talk, boarding line, refusal path and map change action as the other owl rides, so the flow ends the way the game's own owl paths end.
- With only this setting on, the Death Mountain Trail owl keeps its vanilla behaviour: the chain of map checks falls back to the original next event of its mask talk, and the owls are not made permanent. With More owl rides also on, both work.

## Tests

Unit and game-file tests (`cargo test -p ootrr-core owl`): the flow walked for the fountain owl (asks, yes goes to `HyruleField_Child` with empty start name), refusal ends in the game's line; the maps gain exactly one owl each in Kakariko and Zora's Fountain; staging with this setting alone adds the flow, the fountain map and the language archives only; a second patch is refused. Not tested in the game by the author (see the test plan).

## How to test in the game

Child Link. `teleport ZoraFountain_Child` then `warp 17.1 105.2 15.4`; talk to the owl, answer yes; watch the flight; check Link arrives near (-97.3, 150.5, 198.4) in `HyruleField_Child`, facing 170, with control. Answer no: refusal line, control back. Check both maps load.

## Notes for the next feature

A new ride is one row in `RIDES`; `coords: Some((pos, yaw_degrees))` lands at measured coordinates when the destination has no start point. Open question: whether the yaw is read in degrees by the game (the teleport work says so).
