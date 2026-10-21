# SPEC-FINDINGS — Satoshi Hike v2.1

The construction in §§7.4, 10, 11, and 12, Rule B2, and Rule B4 matches `spec/conformance/v2.1/` byte for byte. The C reference was not changed to chase a vector. The defect is the catalogue in §16, which still describes the historical v2.0 pack.

## 1. Line 2032 contradicts line 2030 and the v2.1 files

File: `pcoc/spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md`

Line 2030 says the v2.1 pack is the seed, the NL4 word, the terrain digests, the §11.1 fold, the leg, checkpoint, and waypoint results at the conformance-small sizes, the check that the seed equals `stages[0].waypoint`, and the rate and anchor comparisons, and that it does not republish a full hike-object encoding.

Line 2032 then says each verify vector ships `object_hex`, `sha256` of those object bytes, `header_reads`, `plant_reads`, `index_reads`, and `checkpoint-preimage.json`; that `kat-small.json` publishes the terrain bytes, one real leg, numeric bookends, a six-leg check, `documents`, and `draw_bytes_hex`; and that `test-3leg`, `anchor-window`, `anchor-w16-i17`, and `bind-threads` are verifier inputs. It says a producer replay of `kat-small` equals `object_hex`.

The files under `spec/conformance/v2.1/` are `anchors.json`, `kat-small.json`, `negatives.json`, `nl4-words.json`, `rate.json`, `seed.json`, `step-c1.json`, `terrain-small.json`, `test-3leg.json`, and `README.md`. They do not contain `object_hex`, `draw_bytes_hex`, `documents`, `header_reads`, `plant_reads`, `index_reads`, or a section 14 record. `anchor-window`, `anchor-w16-i17`, and `bind-threads` are not in the v2.1 manifest. `kat-small.json` in this pack is 18 stages of one leg each, with `legs_le_hex`, `checkpoints_hex`, and `next_waypoint_hex`. It is not a one-leg object encoding.

Proposed replacement for line 2032:

> Each v2.1 file is named in `spec/conformance/v2.1/manifest.json`. `seed.json` publishes the 64-byte seed preimage, the seed, the four lanes, and `waypoint_equals_seed`. `nl4-words.json` publishes NL4 base words, with parents off, and the SHA-256 of slot 0 as 2048 little-endian words. `terrain-small.json` publishes the SHA-256 of a full NL4 fill, parents and `slot_mix` included. `step-c1.json` publishes one §11.1 step on the 2048-word base slot (`S_folded`, the seal digest, and `S'`) and the §7.4 seal of zero. `kat-small.json` and `test-3leg.json` publish conformance-small producer results: the four profile integers, the seed, the opening waypoint, the terrain SHA-256, and for each stage `leg_count`, `next_beacon_hex` (JSON null on the last stage), `legs_le_hex`, `checkpoints_hex`, and `next_waypoint_hex` (JSON null on the last stage). They do not publish `object_hex`, the terrain bytes, `documents`, or `draw_bytes_hex`. `rate.json` publishes the placeholder floor `1/5` and the integer comparisons. `anchors.json` publishes index lists and whether `anchored_total` is at least 6. `negatives.json` publishes a withdrawn result and the conforming result for each withdrawn construction. This pack does not contain `anchor-window.json`, `anchor-w16-i17.json`, `bind-threads.json`, or `checkpoint-preimage.json`. Those names, and `object_hex`, `draw_bytes_hex`, `header_reads`, `plant_reads`, and `index_reads`, belong to the historical pack under `spec/conformance/v2.0/`. A v2.1 producer matches the published leg, checkpoint, and waypoint fields. It is not required to equal an `object_hex` this pack does not contain. Rule A4.4 stays specified in section 14.6. This pack does not publish draw bytes or a six-leg record, so it does not test that draw.

## 2. Line 2030, later sentences, still require a section 14 record inside the v2.1 pack

The same paragraph, after the accurate list, says: "The verify bytes in that pack are the section 14 record the reference CLI writes, keys in the order section 14 lists, with no insignificant whitespace and one trailing newline." It then requires `shape_item`, `headers`, `paths`, `anchors`, bookends, `N`, and `S` on a current object's record in that pack.

Those bytes are not in `spec/conformance/v2.1/`. A full verifier that is given a hike object still owes the section 14 record. This pack does not give it that object.

Proposed replacement for those sentences, leaving the earlier sentences of line 2030 in place:

> The v2.1 pack does not publish a section 14 verify record. A verifier that is given a hike object still outputs that record, keys in the order section 14 lists, with no insignificant whitespace and one trailing newline. Matching this pack does not require emitting that record, because the pack does not contain one. The historical v2.0 pack is not a substitute.

No other conflict turned up. The reference follows §§10–12 and the published vectors, and it does not invent `object_hex` to satisfy line 2032.
