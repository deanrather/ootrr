# 55 AI graph attachments (tooling)

No setting. This teaches the AI graph (AINB) reader and writer the attachment table, so graphs that carry attachments can be read, edited and written back. Nothing in the game changes, and the default build is byte-identical to main.

## Request

- 23 Nov: "improve the tooling". The reader refused any graph with attachments ("attachments are not supported"). That blocked every enemy's action root and land-battle sequence (`Enemy_Stalfos.action.root`, `Enemy_Wolfos.action.root`, `Enemy_Lizalfos_LandBattle.actionseq.root`, `Enemy_Moblin_Club_LandBattle.actionseq.root` and more), and with them feints on Wolfos and Moblins, the club Moblin turn and the Bongo hand.
- Requirements: parse attachments (their table, the per-node index ranges, their parameters) and write them back; every game graph parses and rewrites byte for byte; existing edits keep passing and the default build stays byte-identical; the builder can add nodes to graphs with attachments without breaking attachment indices; this doc.

## What an attachment is, as found in the files

Measured over every AI graph in the 1.1.1 game files (11,707 graphs in all packs and loose files, 5,036 distinct; 68 distinct graphs carry 92 attachments).

- Header word `0x18` counts the attachments, `0x40` points at the attachment index list and `0x3c` at the attachment entries. With no attachments both equal the property table offset, which is what the old reader insisted on.
- Layout, in file order: element parameters, then the index list (one u32 per slot: an attachment number), then the entries (0x10 bytes each), then one parameter block per attachment (0x64 bytes each), then the property table.
- An element names its attachments by a range into the index list: u16 count at element byte `0x04`, u32 first slot at `0x20`. Ranges run in element order. Several elements can name the same attachment (20 graphs do), and 22 attachments are named by no element at all (kept as they are).
- Entry: name (pool offset), offset of its parameter block, two u16 expression counts (always 0 in this game), and the murmur3 hash of the name (checked for all 92).
- Parameter block: a u32 flag word (0 or 1), six (first, count) ranges into the shared property arrays (one per value type, same as an element's), then six pairs of (u32, offset) whose first word is always 0 and whose offset always points just past the block. The reader refuses a block whose offsets point elsewhere (none do), so data we don't understand is never written back wrong.
- Attachment properties sit in the same six property arrays as the elements', always after all element properties, in attachment order. Attachments have no inputs, outputs or plugs. Names seen: `LookIKControllerArbiter*` (most), `HorseSlipBehavior`.

## How it works

- `ainb::Att` (`randomizer/core/src/ainb/attach.rs`) holds an attachment: name, hash, expression counts, flag word, property ranges and the tail words. `Ainb` gains `atts` and `att_ids` (the index list). `Ainb::parse` reads them and `Ainb::write` lays them out between the element parameters and the property table, recomputing every offset.
- `put_props(k, records)` inserts property records at the end of the element properties (before the attachments' own) and shifts the attachments' ranges, so a node added later sits where the game's own tool would put it. `add_node`, `new_node`, `graft` and the start-module clone now use it; for a graph without attachments it is the same append as before.
- `graft` copies a source node's attachments with it (names and string values re-pooled), and the start-module clone gives the copy its own index slots pointing at the same attachments, the way the game shares them.
- `el_atts(i)`, `att_name(j)`, `find_atts(name)`, `new_att`, `copy_att` and `set_el_atts(i, ids)` are the API for later features. `set_el_atts` only gives attachments to a node with none, and only when no later node has any, so index ranges stay in element order.
- The layout audit checks that element and attachment property ranges tile each array, that index slots name existing attachments and that element ranges stay inside the list.
- `ootrr check-ainb <romfs> <folder>` now also walks the graphs inside `.pack.zs` files and counts those with attachments; `ootrr ainb-dump <romfs> <pack> <member>` dumps a graph inside a pack, attachments included.

## How it was tested

- `every_game_graph_parses_and_rewrites_byte_identically_part_1` to `_8` (`ainb.rs`, needs `OOTRR_ROMFS`): every `.ainb` in the romfs, loose or in any pack, parses and rewrites byte for byte; graphs with attachments also pass the layout audit and the name hash check; graphs without attachments have no element naming one. Result: 11,707 graphs (124 of them with attachments, 68 distinct), 0 parse failures, 0 rewrite differences, 0 audit problems. Before this change the 124 refused to parse.
- `nodes_added_to_a_game_graph_with_attachments_keep_every_attachment` (`ainb/build.rs`): in `Enemy_Wolfos_White.action.root` (three attachments, one unnamed, two shared) and `Enemy_Stalfos.action.root`, add a node with properties, a composite, and graft a node that has attachments; the graph audits clean, rewrites byte for byte after a reparse, every original node still names the same attachments with the same property names and values, and the grafted node names copies of its source's.
- `an_attachment_round_trips_and_new_node_properties_go_before_it` (`ainb/attach.rs`): a synthetic attachment with a string property round-trips; a later property insert lands before it and shifts its range.
- All existing AINB edit tests (regen, feints, backup, hook, start module, doors, bridge and the rest) pass unchanged; full results in the report below.
- Default build compared byte for byte with main (`main-c139d4e`), each built into an empty folder: the default preset (seed 4242) gives the same seed file and 255 identical output files; a preset with every difficulty toggle on (sliders at 150 or 200, sizes wild, timers half) gives 757 identical files. No feature's output changes, because no feature edits an attachment-bearing graph yet.

## How to test it in the game

Nothing to test in the game: no setting, no change to the output. The first feature that edits an attachment-bearing graph (a Wolfos or Moblin feint, the club Moblin turn) gets its own in-game test.

## Notes for the next feature

- The roots that were closed are now open to `add_node`, `graft` and the input and plug edits. List them with `ootrr check-ainb <romfs> Pack/Actor`.
- Nodes are still only appended; nothing reorders elements. If a later feature needs to give a middle node a new attachment, `set_el_atts` must be extended to insert index slots and shift the later nodes' first slot (it refuses today).
- An attachment's own properties can be read with the same property helpers by its range (`atts[j].pe`); no helper writes them yet.

## MR Drafts

### Initial MR Draft
**Title:** New Tooling > AI graphs Feature: read and write graphs with attachments

The AI graph reader and writer now handle the attachment table, so the 68 game graphs that carry attachments (every enemy action root and land-battle sequence among them) can be edited. No setting, and the build output is unchanged.

For details about implementation and testing, see difficulty-mod/docs/features/55-ainb-attachments.md.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
