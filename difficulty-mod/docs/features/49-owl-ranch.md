# Owl ride to Lon Lon Ranch (an improvement to More owl rides)

## The request (23 Nov)

"Another owl warp from Jabu's fountain to Lon Lon Ranch." The owl stands at Zora's Fountain beside Jabu-Jabu; talking to him flies Link to Lon Lon Ranch, one way, child only.

Measured by the user in the game: owl spot in `ZoraFountain_Child` (17.1, 105.2, 15.4), facing -174; landing spot in `HyruleField_Child` (-97.3, 150.5, 198.4), facing 170 (Lon Lon Ranch is part of the child Hyrule Field map).

## Interpretation

One more row in the ride table of `38-owl-warps.md` (`RIDES` in `randomizer/core/src/qol/owl.rs`). It is not a new setting: the user's call (23 Nov) was that it improves the existing More owl rides setting (`owl_warps`, number 153), so turning that on adds this ride too. The first version had its own setting (`owl_ranch`, 204); it was removed again, so the settings list, default settings string and default seeds are unchanged.

## How it works

- No new setting; the ride belongs to More owl rides (`owl_warps`), and its tooltip and build note name the new owl.
- Nothing in the game files has an owl at Zora's Fountain, so the Death Mountain Trail owl actor is placed there, like the Kakariko owl. The map check in the Death Mountain owl's event flow (`Sequence_CurrentBanc`) picks the ride by map; a new question is added to the owl's message file in every language archive.
- The landing has no start point in the field, so the map change action is given coordinates: `NextStartPos` empty, `NextPosition` and `NextStartRot` set (the coordinates mode of the teleport work, `teleport_flow.rs`). The rotation unit is degrees (the same as `tp-build` defaults).
- The ride uses the same yes talk, boarding line, refusal path and map change action as the other owl rides, so the flow ends the way the game's own owl paths end.

## Tests

Unit and game-file tests (`cargo test -p ootrr-core owl`): the flow walked for the fountain owl (asks, yes goes to `HyruleField_Child` with empty start name), refusal ends in the game's line; the maps gain exactly one owl each in Kakariko and Zora's Fountain; staging with More owl rides adds the flow, the logic file, four maps and the language archives; a second patch is refused. Played by hand on 23 Nov, with the earlier separate setting and the same ride data: refusing once and riding both behave like the other owls, and Link lands at the ranch with control.

## How to test in the game

Child Link. `teleport ZoraFountain_Child` then `warp 17.1 105.2 15.4`; talk to the owl, answer yes; watch the flight; check Link arrives near (-97.3, 150.5, 198.4) in `HyruleField_Child`, facing 170, with control. Answer no: refusal line, control back. Check both maps load.

## Notes for the next feature

A new ride is one row in `RIDES`; `coords: Some((pos, yaw_degrees))` lands at measured coordinates when the destination has no start point. The yaw is read in degrees (confirmed in the game on 23 Nov).
