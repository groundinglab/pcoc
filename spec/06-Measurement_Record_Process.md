# Measurement Record Process

**Responsibility.** How to measure a finished PCOC against Construction and emit the fact-only measurement (verify) record, plus the verify CLI conventions. Constrained by Construction + Conformance. No thresholds, no admit/refuse.

**Source.** Satoshi Hike 2.3 (tag `spec-2.3.0`). Section numbers are the specification section numbers:

- §14 Verify (full procedure: Rules A4/A5, §§14.1–14.7, verify-record shape)
- §18 Verify CLI

**FRAMING.** This document is the measure procedure. Cites of “section 15” name gate-owned judgment (fast record, admit/refuse). A verifier does not apply section 15 and does not reject-before-measure. Runtime is **valid → measured → admitted**: record tokens, including `mismatch`, and do not admit or refuse. “Confirm still valid, then measure” is one pass, not a separate reject gate before measuring. Admission Process consumes the record. Do not say “verified” for admit.

---

## 14. Verify

**Informative. Summary.** Verify takes one hike object, the portable index, and the profile in force, and outputs one verify record (Rule A4). The record carries the shape (14.1), the header reads (14.2), the waypoint chain and paths (14.3), the bookends and leg rate (14.4), the anchored-and-fetched count (14.5), the six-leg check (14.6), and the unknown-kind measurement (14.7, Rule A5). Verify measures and does not judge. Section 15 is the only judgment.

**Normative.**

<a id="rule-a4"></a>

### Rule A4. What verify measures against the portable index

Input is one hike object, the portable index (`{index-url}`, or the documents those reads return), and the profile in force. A `conformance-small` run also takes the four integers in section 16.1. A rig may supply the Rule A4.4 draw bytes. There is no network-mode argument. The plant comparison accepts a returned document whose `network` is `main` or `synthetic` (P14.5).

The profile in force is the section 6 constants, unless the verifier is running `conformance-small`. That run may differ only in `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG`. The profile name and those four integers are verifier inputs. The hike object gains no new field. The verify record must state the profile it ran under, and, when that profile is `conformance-small`, `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG`. That statement is measurement provenance, not a producer claim. Injected draw bytes are rig-only, for a deterministic Rule A4.4 vector. They are never a production input. Production verify reads the operating-system CSPRNG.

Section 14 records measurements against the object and the portable index. It does not judge them. It does not read Bitcoin as its procedure. Outcome tokens are the section 4 set: `match`, `mismatch`, `present`, `skip`, `not defined`, `not read`, `not fetched by design`, `n/a`, `anchored`, `not anchored`, and `unknown kind`. There is no yes/no token. This section does not pass, fail, reject, deny, or admit.

**Evidence and convenience.** Evidence, the hash-bound values, is the seed preimage (`public_key`, `seed_beacon`), the checkpoints, the leg results, the waypoints `W_1` … `W_17`, the Bitcoin headers (`stages[g].beacon` for `g ≥ 1`, and `last_interrupt`), the path’s leaf, Merkle path and root, which bind the path to the waypoint, `stages[0].waypoint` (the seed, `SHA-256(public_key ‖ seed_beacon)`), and any calculated field whose hash dependencies enforce order and cost. Convenience signposts are the path locators (`txid`, `vout`, `block_hash`, `block_height`), checked against the index, `leg_count`, `beacon_height`, `last_interrupt_height`, the section 6 constants, and `stages[0].beacon` (= `seed_beacon`). Verify records `leg_count` under shape: `shape_item` includes `leg_count` when it differs from `len(checkpoints)` (Rule A4.1). Section 6 constants are recorded as `constant`. Stored heights are recorded as `index_comparison` (Rule A4.2). They are convenience. `d_g` and `delta_h` do not subtract a stored height unless the index comparison that confirms it is `match` (Rules A4.5 and A4.8). Verify checks a convenience signpost against the evidence or the index and never relies on it. A broken signpost is a recorded `mismatch`. Section 15 is where an admission service may judge that recorded mismatch. `N`, `S`, `delta_h`, `d_g`, `a`, `b`, spans, and rates are computed by verify from that evidence and the portable index, are never producer claims, and are fields of the verify record.

The measurements are recorded in section order: Rule A4.1 (section 14.1), Rule A4.2 (section 14.2), Rule A4.3 then Rule A4.5 (section 14.3), Rules A4.6 through A4.11 (section 14.4), Rule A4.4 (section 14.6). Later measurements are still recorded. Section 15 is not applied inside this section.

Verification does not GET an HTTP tip. Verification does not GET a Bari path. Verification does not Observe, so it cannot recover an unstored header by assuming the next height.

**Informative.** By design, any verifier may instead read a Bitcoin node directly (block headers with nTime, and the waypoint plants' OP_RETURNs) and build its own index (P14.1). This specification does not require it.

#### Verify record

**Normative.** The verify record is one JSON object. It is not the hike object. The hike object gains no new field. The profile name and, under `conformance-small`, the four integers `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG` are verifier inputs. They appear on this record as measurement provenance. `profile` is always on the record: `conformance-small`, or `production` for a run under the section 6 constants. The profile is not absent from the record. When `profile` is `conformance-small`, the record also has `sample_bytes`, `slot_bytes`, `terrain_bytes`, and `steps_per_leg`. Those four integer keys are present only for `conformance-small`. When `profile` is `production`, those four integer keys are absent. They are not a producer claim. The keys of this full record, in this order, are `profile`, and when `profile` is `conformance-small` the four integers `sample_bytes`, `slot_bytes`, `terrain_bytes`, and `steps_per_leg`, then `shape`, `shape_item`, `headers`, `seed_value`, `checkpoints`, `waypoints`, `paths`, `anchors`, `anchored_total`, `skip_list`, `a`, `b`, `N`, `S`, `delta_h`, `fixed_start`, `P_b`, `height_P_b`, `index_comparison_P_b`, `T_P_b`, `height_I_a_minus_1`, `T_I_a_minus_1`, `six_leg`, `six_leg_runs`, and `analysis`. On this full record, every key listed above is present. The fast record in section 15.3 includes `checkpoints[*].legs[*].result` from the Rule A4.3 digest and omits `six_leg` and `six_leg_runs`. The Rule A4.3 checkpoint digest is a cheap check on every checkpoint. It hashes the stored sample. It does not recompute a leg. The admission service runs it with the other cheap checks. It is not thorough-only. The fast checkpoint-binding checks are the fold and the waypoint rehash under section 12.2 from stored checkpoints. Every other section 14 key stays, including `checkpoints[*].g`, `checkpoints[*].legs[*].j`, `waypoints[*].result`, and `seed_value`. It has no placeholders. It does not fill an omitted key with `not defined`. There is no JSON null. Tokens are JSON strings. Top-level key order is not significant for the values. The bytes hashed as `verify_record_sha256` are this object encoded with RFC 8785 (JSON Canonicalization Scheme). A conforming verify CLI writes this one object with no insignificant whitespace, keys in the order listed above, and one trailing newline (U+000A). The pack does not publish a section 14 record. That byte order is not a second record. The compact line that named only `profile`, `shape`, `checkpoints`, `waypoints`, `leg_rate`, `six_leg`, and `anchors` is not a conformance output and a conforming verify CLI does not write it. That encoding is the default for this cut. If a future profile stores the record as CBOR instead, that profile must name deterministic CBOR (RFC 8949 §4.2.1) explicitly. This file does not define a third encoding. Arrays are JSON arrays. `n/a` is used only as `plant` when `path_state` is `skip`: the plant comparison does not apply. `n/a` is not `not defined`. A field Rule A4.11 leaves undefined stores `not defined`. Display rounding is not under test. The rate quotient is not a key of the verify record.

Nested objects in that CLI file use the key order this section names. A `headers` element is `stored_hash`, `stored_height`, `index_comparison`, `T`. A `paths` element for `g` in 1…16 is `g`, `path_state`, `plant`, `P_g`, `height_P_g`, `d_g`, `claimed_block`. The `g = 17` element is `g`, `w17_path`, `claimed_block`. `P_g`, `P_b`, and `claimed_block`, when objects, are `block_hash`, `block_height`. A checkpoint stage is `g`, `legs`. A leg is `j`, `result`. A waypoint element is `g`, `result`. `fixed_start` is `N`, `S`, `delta_h`. `six_leg`, when an object, is `indices`, `results`. `analysis` is `whole_hike_legs`, `whole_hike_span`, `per_stage`, `legs_outside`, `hike_legs`, `hike_span`. A `per_stage` element is `g`, `checkpoint_count`, `span`. The trailing newline is part of the CLI file. It is not an input to the RFC 8785 hash. The hash input is the record without that newline.

**Normative.** Part A. The JSON type of each numeric key on this verify record is fixed. It does not depend on the value. Heights, counts, Unix seconds, and leg indices are JSON numbers. Their stated bound is ±(2^53−1). A non-negative key is `0 … 2^53 − 1`. Signed differences of those values, including `S`, `delta_h`, `d_g`, and the analysis spans, stay inside ±(2^53−1) and are JSON numbers. Decimal-string keys: none. Integers are plain decimal JSON numbers. No exponent. No leading zeros. No spelling `-0`. A negative integer uses a leading `-` and then digits, and the value is not zero. A measurement token is a JSON string. `six_leg_runs` is a JSON array of draw objects when the draws are recorded, and the JSON string `not read` when the six-leg draw is `not read`. That string is the token. It is not an array and it is not `[]`.

When a shape `mismatch` leaves a key or an array element without a usable value from the object, that value is `not defined`, and an index read or `T` that cannot be made is `not read`. Array lengths stay as stated: `headers` has 19 elements, `checkpoints` has 18, `waypoints` has 17, `paths` has 17, and `analysis.per_stage` has 18 elements. When that stage’s checkpoints are unusable, that stage’s `legs` is `not defined`. The length of `legs` is the number of checkpoints. It does not read `leg_count`. The one exception is a checkpoint with no leg result: its `result` is `mismatch` (Rule A4.3). An empty checkpoint array is usable. Its `legs` is `[]`. The waypoint recompute still runs, and a differing digest is `mismatch`, not `not defined`.

`shape` is `match` or `mismatch`.

`shape_item` is a JSON array of strings from this closed set, and from no other string:

| Order | String | Violated when |
|---|---|---|
| 1 | `malformed cbor` | The input is not one definite-length CBOR data item under section 7.3 |
| 2 | `field set` | A map that is present has the wrong keys or value types. The length of `stages` is reported only by `stages length`. An absent stage map is reported only through the per-field conditions (`leg_count` and the others) |
| 3 | `stage-0 path key` | Stage 0 has a `path` key |
| 4 | `W_17 path` | `stages[17].path` is not empty |
| 5 | `constant` | `kind`, `fill`, `slot_bytes`, `n_slots`, `sample_bytes`, or `steps_per_leg` differs from the profile in force |
| 6 | `stages length` | `stages` does not have length `STAGES` |
| 7 | `leg_count` | `leg_count` differs from `len(checkpoints)`, `leg_count` < 1, or an array length differs |
| 8 | `checkpoint width` | A checkpoint is not 32 bytes |
| 9 | `leg result width` | A leg result is not 8 bytes |
| 10 | `seed beacon` | `stages[0].beacon` is not `seed_beacon` |
| 11 | `last_interrupt` | `last_interrupt` is not 32 bytes, or it does not differ from `stages[17].beacon`, or its height is not strictly greater |
| 12 | `header height order` | A stored height is not strictly greater than the header before it |
| 13 | `header hash differ` | A stored header hash does not differ from the header before it |
| 14 | `height_range` | An integer above `2^53 − 1` is in the object's own bytes, including `stored_height` and `P_b` when those bytes are the object's. A height inside a hash-verified path body is not this string |

On `match`, `shape_item` is `[]`. On `mismatch`, the array contains every string whose condition is violated, each string once, in the order 1…14. The table order is normative for `shape_item`. A condition that is violated is not dropped because another condition is also violated. A condition that tests a field’s value is violated when that field is absent. `stage-0 path key` is violated only when the key is present.

A malformed CBOR value is the one exception to that absent-field rule. `shape` is `mismatch` and `shape_item` is exactly `["malformed cbor"]`. No other string is added.

When the object’s `kind` is `KIND` and the object decodes, and `last_interrupt` and `last_interrupt_height` are both absent, and every other Rule A3 condition that uses only present fields is met, `shape_item` is exactly `["field set", "last_interrupt", "header height order", "header hash differ"]`. That is a shape measurement. It is not Rule A5, which applies only when `kind` is not `KIND`. `headers` still has 19 elements. `headers[18].stored_hash` and `headers[18].stored_height` are `not defined`. `headers[18].index_comparison` is `not read`, and `headers[18].T` is `not read`. If only the `last_interrupt` hash is absent and `last_interrupt_height` is present and strictly greater than `stages[17].beacon_height`, `header height order` is not violated.

`headers` is an array of 19 objects, index 0 through 18, in stored-header order. Index 0 is `stages[0].beacon` (`seed_beacon`). Index `k` for `k` from 1 through 17 is `stages[k].beacon` (`I_{k−1}`). Index 18 is `last_interrupt` (`I_17`). Each object has exactly `stored_hash`, `stored_height`, `index_comparison`, and `T`. `stored_hash` is 64 lowercase hexadecimal digits, natural byte order, byte 0 first, the hash stored for that header, or `not defined` when that hash is absent. `stored_height` is the unsigned height stored for that header, or `not defined` when that height is absent. An integer above `2^53 − 1` in that object byte is not recorded as that unsigned value: `shape` is `mismatch`, `shape_item` includes `height_range`, and the value is `not defined`. It is not `not read`. `index_comparison` is `match`, `mismatch`, or `not read`. When `stored_hash` is `not defined`, `index_comparison` is `not read`. `T` is an integer number of Unix seconds only when `index_comparison` is `match` and `header.time` follows section 9, and `not read` otherwise.

`seed_value` is `match` if `stages[0].waypoint` equals `SHA-256(public_key ‖ seed_beacon)`, and `mismatch` otherwise.

`checkpoints` is an array of 18 objects, index `g` from 0 through 17, one per stage. Each object has `g` and `legs`. `legs` is an array in leg order, `j` from 0 through one less than the number of checkpoints in that stage. `legs` runs over the checkpoints. Each leg object has `j` and `result`. `result` is `match` or `mismatch`. A checkpoint with no leg result has `result` `mismatch`.

`waypoints` is an array of 17 objects for the Rule A4.3 recomputes of `W_1` … `W_17`, in ascending `g` from 1 through 17. Each object has `g` and `result` (`match` or `mismatch`). When the stage bytes needed for that recompute are absent or unusable, including malformed CBOR, `result` is `not defined`. It is not `mismatch`. `checkpoints[g].legs` is `not defined` in that same case. An empty checkpoint array is usable: `legs` is `[]`, the waypoint is still recomputed, and a differing digest is `mismatch`. A present checkpoint with no leg result still has `result` `mismatch`.

`paths` is an array of 17 objects in ascending `g` from 1 through 17. Array position `g − 1` is `W_g`.

For `g` from 1 through 16, each object has `g`, `path_state`, `plant`, `P_g`, `height_P_g`, `d_g`, and `claimed_block`. `path_state` is `present` or `skip`. `plant` is `match`, `mismatch`, `not read`, or `n/a`. `plant` is `n/a` only when `path_state` is `skip`. Then `P_g`, `height_P_g`, `d_g`, and `claimed_block` are `not defined`. On a malformed CBOR value, `paths[g].path_state` and `paths[g].plant` for g in 1…16 are `not defined`. That is the one exception to the closed set in this paragraph. `skip_list` is `[]`. `analysis.per_stage[g].checkpoint_count` for g in 0…17, `analysis.whole_hike_legs`, and `analysis.hike_legs` are `not defined`. A missing stage is not a checkpoint count of zero, and `skip` is only an empty path byte string (Rule A4.5), which this input is not.

For `g = 17`, the object has `g`, `w17_path`, and `claimed_block`. When `stages[17].path` is empty, `w17_path` is `not fetched by design` and `claimed_block` is `not defined`. When `stages[17].path` is not empty, `w17_path` is `not defined`, `shape_item` includes `W_17 path`, and `claimed_block` is the block encoding below when both fields can be read, otherwise `not defined`. Those bytes are not `present` and are not anchored and fetched.

`P_g`, `P_b`, and `claimed_block`, when they are objects, have exactly `block_hash` and `block_height`. `block_hash` is 64 lowercase hexadecimal digits in Bitcoin display order: the section 9 reversal of the path body’s natural-order `block_hash` bytes. It is not a byte-0-first spelling of those bytes. `block_height` is an integer. `root` in an index plant comparison stays natural-order hex. That comparison is not these keys. The interrupt-header check uses the path body’s natural bytes and does not use this display spelling.

When `plant` is `match`, `P_g` is that object and `height_P_g` is the index-confirmed `block_height` (the plant the index recorded). Otherwise `P_g` and `height_P_g` are `not defined`. `d_g` is the signed integer `height_P_g − height(I_{g−1})` only when `plant` is `match` and the index comparison of `I_{g−1}` (`headers` index `g`) is `match`. `height(I_{g−1})` in that subtraction is `header.height` from that matching index document. Otherwise `d_g` is `not defined`. `match` for that header requires the stored hash to agree and the stored height to equal `header.height`. Do not subtract `stages[g].beacon_height` when that comparison is not `match`. On a `present` path, `claimed_block` is that object when both fields can be read from the path body, and `not defined` when they cannot. That gap is not an index read.

`anchored_total` is the count of `g` in 1…16 whose `anchors` token is `anchored`. A present path outside the window is not in this count. `skip_list` is a JSON array of integers, ascending, each a `g` in 1…16 whose `path_state` is `skip`. It is `[]` when there is none.

`a` and `b` are JSON numbers in `0 … 2^53 − 1`, or `not defined`, or `not read`. `N` is a JSON number in `0 … 2^53 − 1`, or `not defined`, or `not read`. `S` is a JSON number of seconds in ±(2^53−1), or `not read`, or `not defined`. `delta_h` is a JSON number in ±(2^53−1), or `not defined`, or `not read`. `P_b` is the block encoding for `W_b` when bookends exist. Which of these fields are integers and which are tokens is Rule A4.11.

The raw endpoints are present as their own keys. The read for `P_b` is `GET {index-url}/index/{P_b.block_height}`. `index_comparison_P_b` is `match`, `mismatch`, or `not read`. Decode `header.hash` and `P_b.block_hash` each as 32 display-order bytes under section 7.2. Do not reverse either side. Letter case does not change the bytes. `index_comparison_P_b` is `match` when those 32 bytes are equal and `header.height` equals `P_b.block_height`. It is `mismatch` when the read is a successful header read under section 9 and either the 32 bytes differ or the height differs. It is `not read` when the read does not return a document, or returns a document that is an unsuccessful header read under section 9, including a plant document with no usable `header` that Rule A4.8 reuses. `height_P_b` is that document’s `header.height` when `index_comparison_P_b` is `match`, and `not defined` otherwise. `T_P_b` is the integer `header.time` only when `index_comparison_P_b` is `match` and `header.time` follows section 9. A time that does not follow section 9 is `not read`. Otherwise `T_P_b` is `not read`. `height_I_a_minus_1` is `header.height` from the index document for `I_{a−1}` when that header’s `index_comparison` is `match`, and `not defined` otherwise. It is not the stored `stages[a].beacon_height` taken alone. `match` for that header requires the stored hash to agree and the stored height to equal `header.height`. `T_I_a_minus_1` is `T(I_{a−1})` under Rule A4.2, an integer only when that `index_comparison` is `match`, and `not read` otherwise. Which of `a`, `b`, `N`, `S`, `delta_h`, `P_b`, `height_P_b`, `index_comparison_P_b`, `T_P_b`, `height_I_a_minus_1`, `T_I_a_minus_1`, `fixed_start.N`, `fixed_start.S`, `fixed_start.delta_h`, and `analysis.legs_outside` are integers, `not defined`, or `not read` is Rule A4.11, in the order that rule states. A returned header `mismatch` is not the step that sets every bookend field to `not read`.

`fixed_start` is a verify-computed key. Verify computes it from keys this record already holds. The producer does not claim it. The hike object does not carry `fixed_start`. When bookends are defined, `fixed_start` is an object with `N`, `S`, and `delta_h`. `fixed_start.N` is the checkpoint count over stages 1 through `b−1`. It does not read `leg_count`. `fixed_start.S` is `T_P_b − headers[1].T`. If either time is `not read`, `fixed_start.S` is `not read`. `fixed_start.delta_h` is the index-confirmed `height(P_b)` minus the index-confirmed `header.height` of the header at `headers[1]` when `headers[1].index_comparison` is `match` and the `P_b` height read is `match`. `headers[1].stored_height` is not that height. `fixed_start.delta_h` is `not read` when `headers[1].index_comparison` is `not read` or the `P_b` height read is `not read`. A `not read` on either of those reads stays `not read`. `fixed_start.delta_h` is `not defined` only when the object's own bytes leave no bookends, when two or more paths are valid (Rule A4.11 step 1), every read returned, and fewer than two `anchors` tokens are `anchored`, or when both of those reads returned and `headers[1].index_comparison` is `mismatch` or the `P_b` height read is `mismatch`. The two-or-more valid paths case is index-settled. When the object's own bytes leave no bookends, `fixed_start.N`, `fixed_start.S`, and `fixed_start.delta_h` are `not defined`.

`six_leg` is an object `{ "indices": [q0, q1, q2, q3, q4, q5], "results": [token, token, token, token, token, token] }` in draw order when six indices are drawn. Each result is `match` or `mismatch`. When fewer than six legs exist, `six_leg` is `not defined`. `six_leg_runs` is `[]` in that case. The draw is not made. That array is `[]` only in this case. When the draw is `not read`, `six_leg_runs` is the string `not read` and `six_leg` is `not read`. That count is settled by the object's own bytes. When the byte source cannot supply the next 8 bytes, `six_leg` is `not read`. A short-byte six-leg draw stays re-readable. It is not `not defined`. Short path bytes for a six-leg run are that `not read`. `six_leg_runs` is an array of those objects, one Rule A4.4 draw per element, in the order the runs were requested. The recorded order is that request order. Whether the runs execute together is Rule A4.4.4. `six_leg` is `six_leg_runs[0]` when `six_leg_runs` is a non-empty array. When the six-leg draw is `not read`, `six_leg_runs` is `not read` and `six_leg` is `not read`. That array is not `[]`.

`analysis` is an object with `whole_hike_legs`, `whole_hike_span`, `per_stage`, `legs_outside`, `hike_legs`, and `hike_span`, in that order. `hike_legs` and `hike_span` are Rule A4.10. `hike_legs` is the number of checkpoints over stages 1–17. Stage 0 is not in that count. `whole_hike_legs` is that same count, a JSON number in `0 … 2^53 − 1`. `whole_hike_span` is the JSON number `T(I_17) − T(I_0)` in ±(2^53−1), or `not read`. `hike_span` is median-time-past from `I_0` to `I_17`, not `whole_hike_span`. `per_stage` is an array of 18 objects, index `g` from 0 through 17, each with `g`, `checkpoint_count`, and `span`. `checkpoint_count` is the number of checkpoints in the stage. It is not a read of the object’s `leg_count`. `span` is the JSON number of the per-stage span in ±(2^53−1), or `not read`. `legs_outside` is the number of checkpoints over stages outside `a` … `b−1`, a JSON number in `0 … 2^53 − 1`. It is `not defined` when the bookend fields are `not defined`, and `not read` when those fields are `not read`. `whole_hike_legs` counts checkpoints, including a stage whose count is zero. `whole_hike_span` and each `per_stage` span stay `not read` when an endpoint `T` was not taken, including when the bookend fields are `not defined`. `analysis` has no rate key.

<a id="sec-14-1"></a>

### 14.1 Shape

**Normative.**

<a id="rule-a4-1"></a>

#### Rule A4.1 Shape

Shape compares the object with Rule A3. `shape` is `match` when every condition below is met. Otherwise `shape` is `mismatch`, and `shape_item` names each condition that is violated. Later measurements are still recorded. A malformed CBOR value is `shape` `mismatch` and `shape_item` is exactly `["malformed cbor"]`. A condition that tests a field’s value is violated when that field is absent. `stage-0 path key` is violated only when the key is present. When a shape `mismatch` leaves a key or an array element without a usable value from the object, that value is `not defined`, and an index read or `T` that cannot be made is `not read`. Array lengths stay as stated in the verify record.

- The input is one definite-length CBOR data item of the shape in section 8, under the decoding rules in section 7.3. Key order and shortest integer encodings are not a shape `mismatch`.
- A map that is present contains exactly the keys and value types listed in section 8. Otherwise `shape_item` includes `field set`. The length of `stages` is reported only by `stages length`. An absent stage map is reported only through the per-field conditions (`leg_count` and the others). Stage 0 has no `path` key. Stages 1–17 each have a `path` key.
- A `path` key on stage 0, including a key whose value is empty, is shape `mismatch`. `shape_item` includes `stage-0 path key`. Those bytes are not a path. This is a measurement only. The object records no stage-0 path.
- `stages[17].path` is the empty byte string. Any other value is shape `mismatch`. `shape_item` includes `W_17 path`. When stage 17 or its `path` field is absent, including when `stages` is shorter than `STAGES`, this condition is violated under the field-absent rule above and `shape_item` includes `W_17 path` (the `test-3leg` vector records this).
- `kind`, `fill`, `slot_bytes`, `n_slots`, `sample_bytes`, and `steps_per_leg` equal the profile in force. Otherwise `shape_item` includes `constant`. The profile in force is the section 6 constants, unless the verifier is running `conformance-small`, which may differ only in `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG`. `n_slots` is still `TERRAIN_BYTES / SLOT_BYTES` for that profile. The hike object has no `network` field. The profile is the verifier’s choice.
- `last_interrupt` is 32 bytes, differs from `stages[17].beacon`, and `last_interrupt_height` is strictly greater than `stages[17].beacon_height`. Otherwise `shape_item` includes `last_interrupt`.
- `stages` has length `STAGES`. Otherwise `shape_item` includes `stages length`.
- Each `leg_count` ≥ 1 and equals `len(checkpoints)`, and both arrays of that stage have that length. Otherwise `shape_item` includes `leg_count`.
- Each checkpoint is 32 bytes. Otherwise `shape_item` includes `checkpoint width`. Each leg result is 8 bytes. Otherwise `shape_item` includes `leg result width`.
- `stages[0].beacon = seed_beacon`. Otherwise `shape_item` includes `seed beacon`.
- `stages[0].waypoint` is 32 bytes. Rule A4.3 records `match` if it equals `SHA-256(public_key ‖ seed_beacon)`, and `mismatch` otherwise.
- Each stored header’s hash differs from the one before it (`shape_item` `header hash differ` when one does not), and its height is strictly greater (`shape_item` `header height order` when one is not). The stored headers, in order, are `stages[0].beacon`, `stages[1].beacon` … `stages[17].beacon`, and `last_interrupt`.
- An integer above `2^53 − 1` in the object's own bytes, including `stored_height` and `P_b` when those bytes are the object's and that height is not inside a hash-verified path body, is shape `mismatch`. `shape_item` includes `height_range`. The value is `not defined`. It is not `not read`. A height that is `not defined` this way is treated as absent by every other shape condition: `header height order` is violated for each comparison that height takes part in, and `last_interrupt` is violated when that height is `last_interrupt_height`. `block_height` in a body that decodes is in `0 … 2^32 − 1`, which is inside `0 … 2^53 − 1`. A hash-verified canonical path body has no height above `2^53 − 1`. A body that does not decode, or whose `validate_for` does not succeed, has `plant` `mismatch` and `claimed_block` `not defined`. That is not `shape_item` `height_range`.

Shape does not decode a path on stages 1–17 and does not record **skip** or **present**. Rule A4.5 does. This measurement does not GET a path.

<a id="sec-14-2"></a>

### 14.2 Headers

**Normative.**

<a id="rule-a4-2"></a>

#### Rule A4.2 Headers

`{index-url}` is the absolute URL from section 9. For each stage, in order from stage 0, GET `{index-url}/index/{beacon_height}`. Then GET `{index-url}/index/{last_interrupt_height}`. Each height is the shortest base-10 digits of the integer.

Each GET is a clock read under section 9. For each stored header, record its stored height, its stored hash, its index comparison, and its `T`.

- The read is not successful: `index_comparison` is `not read`, and `T` is `not read`.
- The read is successful and the natural beacon differs from the stored hash, or `header.height` differs from the stored height: `index_comparison` is `mismatch`. Time is not part of `index_comparison`.
- The read is successful and the hash and the height agree: `index_comparison` is `match`.
- `T` is the integer `header.time` only when `index_comparison` is `match` and `time` follows section 9. Otherwise `T` is `not read`. A successful read whose stored hash does not agree records `mismatch` and does not take `T`. Do not record that document’s time as `S` or as any other span. A missing or unusable `time` does not by itself make `index_comparison` `mismatch`. A height disagreement is `mismatch` and also leaves `T` as `not read`, because `T` is taken only on `match`.

This rule does not take `T` from a successful read when the stored hash does not agree.

On the mainnet path, a successful read’s `header.hash`, `header.height`, and `header.time` are the portable index’s record of that Bitcoin block. `index_comparison` uses hash and height only. `T` is recorded separately.

Read every stored header, including `last_interrupt`. A `mismatch` or a `not read` does not skip a later header. Later measurements are still recorded.

The header measurement does not read `network`. The same hash and height comparison applies to every returned document. A header that matches `header.hash` and `header.height` is not a `mismatch` because the document’s `network` is `synthetic`. A `match` against a synthetic document is not a mainnet fact.

This header read does not run a leg and does not recompute terrain. It does not GET an HTTP tip. It is not Observe.

<a id="sec-14-3"></a>

### 14.3 Waypoints and paths

**Normative.**

<a id="rule-a4-3"></a>

#### Rule A4.3 Chain

Recompute terrain as in section 10.

Checkpoint comparisons, for each stage in order and each leg `j` in order:

- If `j = 0`, `previous` is that stage’s leg-0 start (Rule A2.2). On stage 0, `previous` is `SHA-256(public_key ‖ seed_beacon)` computed from the object. On stage `g ≥ 1`, `previous` is that stage’s stored `waypoint`. Otherwise `previous = checkpoints[j − 1]` of that stage.
- Recompute the terrain sample at `previous`.
- Let `limb` be the stored `leg_results[j]` byte string when that element is present. If there is no `leg_results[j]` for that checkpoint, record `result` `mismatch` and do not treat a missing leg result as a limb.
- The stage limb is the seed on stage 0 and `W_g` on stage `g ≥ 1`. On stage 0 the stage limb in this comparison is the computed `SHA-256(public_key ‖ seed_beacon)`. On stage `g ≥ 1` it is the stored `waypoint`. It is not `previous` when `j ≥ 1`.
- When `leg_results[j]` is present, record `match` on that leg if `checkpoints[j]` equals `SHA-256(previous ‖ stage limb ‖ terrain_sample ‖ limb)`. Record `mismatch` otherwise. `legs` runs over the checkpoints.

This digest record does not run a leg. It checks every stored checkpoint that has a leg result against that limb and the recomputed sample. A checkpoint with no leg result is `mismatch`. A `mismatch` does not skip a later leg.

Record `match` if `stages[0].waypoint` equals `SHA-256(public_key ‖ seed_beacon)`. Record `mismatch` otherwise. That comparison does not call the field a Rule A1 waypoint.

**Informative.** Stage 0’s checkpoint comparisons use the computed seed. The `W_1` recompute uses the stored `stages[0].waypoint`. So the stored field is measured by this seed-value comparison and through `W_1`.

For `g` from 0 through 16, recompute `W_{g+1}` under section 12.2. Use as `B` the interrupt `I_g`, which is the stored `stages[g+1].beacon`. Use the stored `stages[g].waypoint` and the stored checkpoints of stage `g`. On `g = 0` that stored field is the seed, not a member of `W_1` … `W_17`. Record `match` if the digest equals `stages[g+1].waypoint`. Record `mismatch` otherwise. This uses the stored checkpoint bytes, including an empty list: the fold is `0^32` and the waypoint is still recomputed. A disagreement is `mismatch`. It is not `not defined`. `legs` for that stage is `[]`. `not defined` is only when the stage pair is absent or those bytes are unusable. This recompute does not run a leg. No waypoint is recomputed from stage 17 or from `last_interrupt`. The waypoints recomputed here are `W_1` … `W_17`.

<a id="rule-a4-5"></a>

#### Rule A4.5 Paths

**Anchoring.** H for `W_g` is `stages[g].beacon_height` and the window is `ANCHOR_WINDOW_OFFSETS` (section 13.1); verify uses that same H. The path carries the height of the block that includes the waypoint. Verify records `anchored` only when that inclusion height is in the window, that header is an interrupt stored on the object, and the plant comparison against that one header is `match`. Otherwise it records `not anchored`. The stored interrupt headers are `stages[1].beacon` through `stages[17].beacon` and `last_interrupt` (`I_17`). The interrupt-header check is the same height and the same 32 natural-order bytes (section 9): the path body’s `block_hash` compared with that stored beacon, with no reversal. Verify reads `GET {index-url}/index/{body.block_height}` only when the height is in the window and that header is one of those stored interrupts. It does not read that height otherwise, and it does not read any other index height for that waypoint. A missing path, a path outside the window, or a path whose inclusion header is not one of those stored interrupt headers is `not anchored`. A path the producer stores can still be `not anchored` when its inclusion block is inside the window and is not one of the stored interrupt headers. A path included in `I_17` is inside the window for `W_16` in the expected one-header steps, and it is `anchored` when the plant comparison against that header is `match`.

For each of `W_1` … `W_16` (`stages[1].path` … `stages[16].path`):

- Empty path bytes: `path_state` is `skip`, and `plant` is `n/a`.
- Any other path bytes: `path_state` is `present`.

When `stages[17].path` is the empty byte string, record `not fetched by design`. It is not `skip` and it is not `present`. When those bytes are not empty, `shape_item` includes `W_17 path`. Do not record those bytes as `present`. Do not authenticate a plant. They are not anchored and fetched. If those bytes still yield `block_hash` and `block_height`, record them as `claimed_block`. If they do not, `claimed_block` for that waypoint is `not defined`.

A `present` path is measured by the procedure below.

- `plant` `match`: record `P_g` and `height_P_g` from the index-confirmed plant. Record `d_g = height_P_g − height(I_{g−1})` as a JSON number in ±(2^53−1) only when the index comparison of `I_{g−1}` is also `match`. `height(I_{g−1})` in that subtraction is `header.height` from that matching index document, not `stages[g].beacon_height` taken alone. If that comparison is not `match`, `d_g` is `not defined`. `I_{g−1}` is `stages[g].beacon`. Record `claimed_block` from the path body. `plant` `match` is recorded only when the inclusion height is in the section 13.1 window, that header is an interrupt stored on the object, and the plant comparison against that one header is `match`. That `plant` `match` is the `anchors` token `anchored`. A path outside the window, or whose inclusion header is not a stored interrupt, is `not anchored`. No plant comparison is performed for it.
- `plant` `mismatch`: the document was read and it has no plant for that block, or a plant it records does not match. `P_g` is `not defined`. `d_g` is `not defined`. Record `claimed_block` from the path body when `block_hash` and `block_height` can be read; otherwise `claimed_block` is `not defined`. The path is not anchored and fetched and is not a bookend.
- `plant` `not read`: the read does not return a document. `d_g` is `not defined`. The path is not anchored and fetched. Record `claimed_block` the same way.

A path that does not decode as a canonical path body, or whose `validate_for` does not succeed for that stage’s `waypoint`, has `plant` `mismatch` and `claimed_block` `not defined` when the bytes were read, because the path facts disagree with a canonical body. That is not an unsuccessful index read. `block_height` in a body that decodes is in `0 … 2^32 − 1`, which is inside `0 … 2^53 − 1`. A hash-verified canonical path body has no height above `2^53 − 1`. That is not `shape_item` `height_range` (P14.6). The hike object still decodes. Do not clamp that height and do not record it.

`anchored_total` is the count of `W_1` … `W_16` whose `anchors` token is `anchored`. Only that token is anchored and fetched. A present path outside the window is not in this count. `skip_list` is the JSON array of skipped waypoint indices among `W_1` … `W_16`, ascending.

`anchors` is an array of 16 tokens, `W_1` through `W_16`. The token is `anchored` only when section 13.1’s window check and interrupt-header check both hold and the plant comparison against that one header is `match`. Every other case in that rule, including a missing path, a path outside the window, and a path whose inclusion header is not stored on the object, is `not anchored`. The comparison that is made reads one index height, the height the path names. It does not scan the window.

A `present` path whose `plant` is `mismatch` or `not read` is in neither `anchored_total` nor `skip_list`. It is recorded, and it is never a bookend.

#### Present path plant measured against the index

**Normative.**

#### Purpose and authority

This measurement compares each present path and that stage’s waypoint with the plant facts the portable index records. It is the measurement “present path plant measured against the index,” placed here with the waypoint measurement.

The measurement records `plant` `match`, `mismatch`, or `not read` for the claimed path and its waypoint anchor. Fake or invented locator metadata, once read, is `plant` `mismatch`.

Bitcoin is the ground truth for those facts. The portable index stands in for it faithfully. This measurement reads them from the portable index. A returned document’s `network` is the string `main` or the string `synthetic`. Any other string, or a missing `network`, is `plant` `mismatch`. There is no verifier input that selects one of those two strings. A `match` on a document whose `network` is `synthetic` is not a mainnet fact.

Bari is the produce-side path-fetch and plant helper. Verify does not re-GET Bari for this measurement.

When the read returns a document and that document records no plant for that block, `plant` is `mismatch`. When plants are present and none matches, `plant` is `mismatch`. When the read does not return a document, `plant` is `not read`. There is no silent fallback from a missing document.

This measurement is separate from the header index comparison in section 14.2. Section 14.2 records, for each stored header, `match`, `mismatch`, or `not read`, and `T` or `not read`. This measurement records whether a present path’s locators and Merkle-committed root match the plant the portable index records. A header index comparison of `match` does not mean the plant comparison is `match`. A plant comparison of `match` does not mean the header index comparison is `match`.

#### What the proof already carries

For a present path that decodes as a canonical path body, the hike object stores `path` as the canonical path-body bytes defined below (the same layout section 13.1 cites). The hike SPEC inlines the field list and roles below for this measurement. A present path’s bytes include, when this measurement has decoded them:

| Field | Role in this measurement |
|---|---|
| `leaf` | Equal to that stage’s `waypoint` when this measurement’s `validate_for` succeeds |
| Merkle path (`leaf_index`, `tree_size`, `siblings`) | Commits `leaf` to `root` when this measurement’s `validate_for` succeeds |
| `root` | The Merkle root the on-chain waypoint anchor must commit |
| `txid` | Transaction locator |
| `vout` | Output index locator |
| `block_hash` | Block locator |
| `block_height` | Height locator |

A canonical path body is one byte string. Version is the first byte and is `0x01`. Then, in order: `leaf` 32 bytes, `leaf_index` as a 4-byte big-endian integer, `tree_size` as a 4-byte big-endian integer, `sibling_count` as one byte, `sibling_count` siblings of 32 bytes each, `root` 32 bytes, `txid` 32 bytes, `vout` as a 4-byte big-endian integer, `block_hash` 32 bytes, and `block_height` as a 4-byte big-endian integer. `leaf`, `root`, `txid`, and `block_hash` are natural-order bytes. The length is `146 + 32 × sibling_count` and no other length decodes. `sibling_count` is at most 32. `tree_size` is at least 1. `leaf_index` is in `0 … tree_size − 1`.

`canonical_sibling_count(tree_size)` starts from `width = tree_size` and counts one level while `width > 1`, setting `width = floor(width / 2) + (width mod 2)` after each level. The sibling count in the body equals that count.

`validate_for(waypoint)` succeeds only when `leaf` equals that stage’s `waypoint` and the Merkle path below resolves to `root`. Otherwise the body does not validate.

Leaf and parent hashes are SHA-256. A leaf node is `SHA-256(0x1A ‖ leaf)`. A parent node is `SHA-256(0x1B ‖ left ‖ right)`. Resolution starts from the leaf node, `index = leaf_index`, and `width = tree_size`. For each sibling in order: if `width` is odd and `index = width − 1`, the sibling must equal the current node and the next node is the parent of that node with itself; otherwise if `index` is even the next node is the parent of `(current, sibling)`, and if `index` is odd the next node is the parent of `(sibling, current)`. Then `index = floor(index / 2)` and `width = floor(width / 2) + (width mod 2)`. The final node is the path root.

`block_height` in a body that decodes is in `0 … 2^32 − 1`, which is inside `0 … 2^53 − 1`. A hash-verified canonical path body has no height above `2^53 − 1`. A body that does not decode, or whose `validate_for` does not succeed, has `plant` `mismatch` and `claimed_block` `not defined`. That is not `shape_item` `height_range`.

This measurement does not add a new hike-object field and does not change the path wire version. It binds the locators and root already carried in `path`.

Shape, section 14.1, does not decode `path`. For a `present` path on `W_1` … `W_16`, this measurement decodes `CanonicalPathBody` and runs `validate_for(waypoint)` (leaf + Merkle to `root` only). A `present` path for which that decode or `validate_for` does not succeed has `plant` `mismatch`. A `skip` is not decoded here. Stage 0 has no path. `W_17` is not fetched by design. When `stages[17].path` is not empty, decode it only far enough to read `block_hash` and `block_height` for `claimed_block`, using the canonical path layout in this section. Do not run `validate_for`. Do not record `plant`. Do not GET the index for those bytes. If either field cannot be read, `claimed_block` is `not defined`. `w17_path` is `not defined`, and `shape_item` includes `W_17 path`.

#### Normative verify procedure

**Inputs.** One hike object, and `{index-url}` (or the documents the reads of that URL return). The profile in force, and under `conformance-small` the four integers, are the section 14 inputs. A rig may supply draw bytes. There is no network-mode argument. Plant facts are the plant records that the index document holds for the block: `root`, `txid`, `vout`, `block_hash`, and `block_height`. They are not a new hike-object field. The index document is one JSON object. `network` is a top-level string. On every verify the plant comparison requires `network` equal to `main` or equal to `synthetic`. Any other string, or a missing `network`, is `plant` `mismatch` when the document was read.

A recorded plant is every object in the top-level array `plants`, and the top-level member `plant` when that member is a JSON object. JSON `null` and an absent member add no plant. If `plants` is absent, `plant` alone is the set. Zero recorded plants, on a document that was read, is `plant` `mismatch`.

Each plant object has `root`, `txid`, `vout`, `block_hash`, and `block_height`. `root` is 64 hexadecimal digits in natural order. `txid` and `block_hash` are 64 hexadecimal digits in Bitcoin display order. Reverse each of those two once to natural order before comparing it with the path body. Do not reverse `root`. `vout` and `block_height` are JSON integers in `0 … 2^32 − 1`, not strings, and not numbers with a fractional part or an exponent. `tree_size` may be present and is not compared. Extra members are ignored. `plant` is `match` when the path decodes and `validate_for` succeeds, the document was read, `network` is the network this paragraph requires, and any one recorded plant equals the body on `root`, `txid`, `vout`, `block_hash`, and `block_height` after that reversal rule. Otherwise, on a document that was read, `plant` is `mismatch`. **Errors** on this measurement, not section 15 decisions: a read that does not return a document is `plant` `not read`; a document that was read and has no plant for that block, or no matching plant, is `plant` `mismatch`; a path body that does not yield `claimed_block` is `not defined`.

This bind is recorded for every `present` path among `W_1` … `W_16`, in order, inside this waypoint measurement, after section 14.1 and section 14.2 have been recorded. It does not apply section 15, and it does not stop a later stage or a later measurement. If the read does not return a document, `plant` is `not read`. If the document was read and it has no plant for that block, or the plants do not match, `plant` is `mismatch`. `claimed_block` is always recorded (Rule A4.5).

For each present path that decodes as `CanonicalPathBody` and whose `validate_for` succeeds for that stage’s `waypoint`, let `body` be that decoded body and let `waypoint` be that stage’s stored `waypoint`. If `body.block_height` is not in the section 13.1 window for that stage, or that height and the path body’s natural-order `block_hash` are not an interrupt header stored on the object, record `anchors` `not anchored` and `plant` `not read`, and record `claimed_block` from the path body when `block_hash` and `block_height` can be read. Do not GET the index for that path. This `plant` `not read` means the read was not issued. It is not an unsuccessful read. It is not a dependent read under Rule A4.11. The steps below run only when both of those checks hold.

1. **Leaf already bound.** `body.leaf` equals `waypoint` and the Merkle path verifies to `body.root`, the decode and `validate_for` just above. This measurement does not repeat a weaker check. A `present` path that did not succeed there has `plant` `mismatch`.

2. **Obtain plant facts for the locators.** This step runs only after the window check and the stored-interrupt check above. The verifier reads `GET {index-url}/index/{body.block_height}` once. That GET is the plant read. It is not one of the Rule A4.2 header reads, and a Rule A4.2 read of the same height is not a substitute for it. The plant facts for `(body.txid, body.vout, body.block_hash, body.block_height, body.root)` come from that portable-index document. The plant read does not require a `header` member. Bari is not queried on this measurement. If that read does not return a document, `plant` is `not read` and steps 4–5 are not a `mismatch`. A document that was read and records no plant for that block is `mismatch`. A path outside the window, and a path whose inclusion header is not a stored interrupt, do not reach this GET. Their `plant` is `not read` because the read was not issued.

3. **What the index attests.** **Informative.** The portable index records a plant for a block only after checking against Bitcoin that `txid` is in `block_hash` at `block_height` on the best chain, that output `vout` has value zero, and that its `scriptPubKey` is exactly Bari `commitment_script(root)`. Verify does not repeat those checks. It compares the body with the plant the index records (step 4).

4. **Portable index.** This is the same one GET as step 2, not a second height. The document’s `network` must be `main` or `synthetic`. Any other string, or a missing `network`, is `plant` `mismatch`. The index must be able to record two plants in the same block. A plant the document records for that block must equal this `body` on `root`, `txid`, `vout`, `block_hash`, and `block_height`. `root` is natural-order hex. `txid` and `block_hash` are Bitcoin display hex, the same presentation as `IndexSnapshot::to_stored`. A plant’s `tree_size` is not part of the comparison. This step compares plant facts that were read. A read that does not return a document is `plant` `not read`. A document that was read and records no plant for that block, a read document whose `network` is neither `main` nor `synthetic`, or a read plant that disagrees on those locator and root fields is `plant` `mismatch`. There is no silent fallback. A read that does not return a document is not recorded as `mismatch`. A missing `header` on that document does not make this step `not read`.

5. **No invented metadata.** Locators that do not equal a plant the index records (step 4) are `plant` `mismatch`.

`plant` is `match` only when the path decodes and validates (step 1), the index document was read (step 2), its `network` is `main` or `synthetic`, and a plant it records for that block equals `body` on `root`, `txid`, `vout`, `block_hash` and `block_height` (step 4). That `match` is what authenticates `P_g`: `body.block_hash` at `body.block_height`. Rule A4.5 then records `height_P_g`. It records `d_g` only when the index comparison of `I_{g−1}` is also `match`. Section 15 is where an admission service may judge a plant `mismatch`. This section only records it.

#### Network string

The plant comparison uses that one rule on every verify. There is no second mode that rejects `synthetic` or that rejects `main`. A document with `network` `synthetic` can record `plant` `match`. That match is not a mainnet fact. A direct read of Bitcoin is outside this procedure (P14.1). It does not add a section 14 input, and that direct read still requires the chain name `main`. A header that matches the document’s `header.hash` and `header.height` is not a `mismatch` for the document’s `network`. Header measurement does not read `network`.

#### Explicit separation from the header index comparison

| | Rule A4.2, section 14.2 | Rule A4.5, this section |
|---|---|---|
| Object | Each stored header’s hash and height, including `last_interrupt` | Path locators and `root` on a `present` `W_1` … `W_16` |
| Read | `GET {index-url}/index/{height}` → `header`, including `header.time` | One `GET {index-url}/index/{block_height}` only when that height is in the window and that header is a stored interrupt |
| Recorded | `index_comparison` `match`, `mismatch`, or `not read`; `T` or `not read` | `plant` `match`, `mismatch`, or `not read`; on `match`, `P_g`, `height_P_g`, and `d_g` |
| Does not | Decode `path` | Re-check the stage header against its stored height |

#### What this measurement does not claim

- It does not pace tips or re-run produce Observe.
- It does not live-refetch Bari: verification still does not `GET {bari-url}/path/{leaf}` (section 14). Produce remains the only path fetch (section 13.1). Verify does not Observe.
- It does not replace shape. Merkle/`validate_for` for a `present` path is this measurement, not section 14.1.
- It does not replace Rule A4.2.
- It does not require the path’s block to be the same header the stage started from (`seed_beacon` or `I_{g−1}`). Those heights answer different questions: the header the stage started from, and the block that includes the plant. Section 13.1 states the path’s block: one height in the anchor window for the post at H, and, for verify, an interrupt header stored on the object.
- It does not add bari-url to verify. Plant facts are read from `{index-url}`.
- It does not impose cross-stage uniqueness on path locators or plants; the bind is per waypoint.
- An index GET, for a stored header and for a plant, is one request under section 9. It does not follow a redirect. It is not retried. A status other than 200, a redirect, a body that breaks section 9, or a request that receives no HTTP status is an unsuccessful read. A header read then records `index_comparison` `not read` and `T` `not read`. A plant read then records `plant` `not read`. The verifier’s wall clock is not an input, and this file sets no timeout.

<a id="sec-14-4"></a>

### 14.4 Bookends

**Normative.**

The verifier’s wall clock is not an input. Verify does not Observe, so it cannot recover an unstored header by assuming the next height. Block times are `T` from Rule A4.2, read as they are, and a `T` is recorded only when that header’s `index_comparison` is `match`. No clamp and no reordering is applied to a recorded `T`. Bookend `S` uses those raw `header.time` values. `hike_span` does not. Rule A4.10 sets `hike_span` from median-time-past at the first interrupt (`I_0`, `stages[1].beacon`, `headers[1]`) and at the last interrupt (`I_17`, `last_interrupt`, `headers[18]`). The seed beacon is not an endpoint. No other adjustment is applied. `delta_h` (`Δh`) is recorded in this bookend block when both heights are index-confirmed. `delta_h` stays in the bookend block. Rule B3 does not read it. Rule B3 is not an admission input. It is a raw measurement. It is not analysis and it is not policy.

<a id="rule-a4-6"></a>

#### Rule A4.6 Bookends

`W_a` and `W_b` are the anchored-and-fetched waypoints with the lowest index and the highest index, wherever they sit. “First” and “last” mean those indices, not the order in which asks completed. Anchored and fetched is the `anchors` token `anchored` from section 14.3: in the section 13.1 window, an interrupt header stored on the object, and plant `match`. A present path outside that window is never a bookend. A present path whose plant comparison is **mismatch** is never a bookend.

`a` and `b` are those indices. Both are in `1 … 16`. `W_17` is not fetched by design, so it is not a bookend. Stage 0 has no path, so `a` is not 0.

<a id="rule-a4-7"></a>

#### Rule A4.7 Bookend legs

`N` counts the checkpoints in stages `a` through `b−1`. Each stage’s checkpoints are folded into the next waypoint. `N` does not read `leg_count`. `N` is a JSON number in `0 … 2^53 − 1`.

<a id="rule-a4-8"></a>

#### Rule A4.8 Bookend span

Record the endpoints from the index, not from stored heights alone.

`P_b` is the authenticated block named in `W_b`’s path. The header read for that block is `GET {index-url}/index/{P_b.block_height}`. `index_comparison_P_b` and `T_P_b` are that read, as the verify record states. `height_P_b` is `header.height` from that document only when `index_comparison_P_b` is `match`. Otherwise `height_P_b` is **not defined**. When section 14.3 already read a document for `{index-url}/index/{P_b.block_height}` in this verification, that document is the Rule A4.8 read. Verify does not send a second GET for the same height.

`I_{a−1}` is the interrupt `W_a` was built from, stored as `stages[a].beacon`. The stored height `stages[a].beacon_height` is convenience. `height_I_a_minus_1` is `header.height` from the index read of that header only when that header’s `index_comparison` is `match`. Otherwise `height_I_a_minus_1` is **not defined**. `T(I_{a−1})` is `T` from Rule A4.2, so it is an integer only on that same `match`, and **not read** otherwise.

```text
S = T(P_b) − T(I_{a−1})
Δh = height_P_b − height_I_a_minus_1
```

`S` is seconds, a JSON number in ±(2^53−1), recorded when both `T` values are integers, including zero or a negative value. If either `T` is **not read**, record `S` as **not read**. Do not invent an integer `S`. Do not subtract a time taken from a header whose `index_comparison` is not `match`.

`delta_h` is a JSON number in ±(2^53−1) only when both `height_P_b` and `height_I_a_minus_1` are index-confirmed integers, including zero or a negative value. Otherwise `delta_h` is **not defined**. Do not compute it from `stages[a].beacon_height` or from any other producer-stored height. `match` for `index_comparison_P_b` requires the header hash to agree and `header.height` to equal `P_b.block_height`. `match` for `I_{a−1}` requires the stored hash to agree and the stored height to equal `header.height`. The subtraction uses those `header.height` values.

`Δh` does not use `stages[a].beacon_height` in place of an index-confirmed height. `Δh` does not require `T`. When `T` is **not read** and both heights are index-confirmed, `S` is **not read** and `delta_h` is still that integer. `N` is still the integer from Rule A4.7 when bookends exist. Both `S` and `delta_h` sit in the bookend block. `delta_h` stays in the bookend block. Rule B3 does not read it. Rule B3 is not an admission input. It is not analysis and it is not policy.

<a id="rule-a4-9"></a>

#### Rule A4.9 Bookend leg rate

The verify record holds the integer `N` and the integer `S` only. It does not hold a floating-point rate. It does not hold any other encoding of the quotient.

The bookend display `N÷S` is not the admission rate. Rule B0 reads `(hike_legs, hike_span)`. Rule A4.11 records `N` and `S` as `not defined` in steps 1 and 3 of that rule, and as `not read` in steps 2 and 4. This rule does not then store integers. A bookend field of `not defined` does not, by itself, refuse admission.

The rate display is `N÷S` when `S > 0`. Otherwise the rate display is `not defined`. `N` and `S` are still shown. `not defined` here is a display token. It is not a floating-point value, and it is not a judgment. Display rounding is not under test. The quotient is not in the verify record. The display is informative. It is not an admission input.

When `S` is **not read**, the rate display is **not defined**, and there is no numeric pair.

<a id="rule-a4-10"></a>

#### Rule A4.10 Analysis

Record these beside the bookend block. `hike_legs` and `hike_span` are the admission inputs (Rule B0). The other lines in this rule are analysis. Rule B0 does not read `whole_hike_legs`, `whole_hike_span`, a per-stage span, `legs_outside`, or the bookend pair for the floor. Each span below uses `T` from Rule A4.2, so each endpoint time exists only when that header’s `index_comparison` is `match`. The span is a signed integer of seconds when both endpoint times were read, and `not read` when either endpoint `T` is `not read`. Each rate below is display only, on the same rule as Rule A4.9: the quotient when the span is an integer greater than 0, otherwise `not defined`. Display rounding is not under test. The verify record’s analysis holds the integers. It does not hold a float. `delta_h` is not an analysis value.

**First beacon.** The first beacon is the first interrupt `I_0`, the object field `stages[1].beacon`. Its stored height is `stages[1].beacon_height`. In the verify record it is `headers[1]`. It is not `seed_beacon` and it is not `headers[0]`. The seed beacon can sit before produce starts. That wait is not part of the admit clock.

**Last beacon.** The last beacon is `last_interrupt`, which stores `I_17`. Its stored height is `last_interrupt_height`. In the verify record it is `headers[18]`.

**How the verifier reads the times.** Rule A4.2 already issues `GET {index-url}/index/{stages[1].beacon_height}` and `GET {index-url}/index/{last_interrupt_height}`, each height in the shortest base-10 digits, under section 9. `T(first beacon)` is `headers[1].T`. `T(last beacon)` is `headers[18].T`. Each is the integer `header.time` of that index document only when that header’s `index_comparison` is `match` and `header.time` follows section 9. Otherwise that `T` is `not read`. The object stores no block time. Those two `T` values are not `hike_span`. Bookend `S` and the analysis spans still use `T`. The verifier does not subtract a time taken from a producer-stored height for those spans.

**Median-time-past.** `hike_span` uses median-time-past, not raw `nTime`, so a producer cannot shorten the span by choosing a beacon whose `nTime` sits ahead of real time. For a beacon at height `h`, let `n = min(11, h + 1)`. The verifier reads the `n` headers at heights `h − n + 1` through `h` inclusive. Each read is `GET {index-url}/index/{height}` under section 9, with the height in the shortest base-10 digits. The beacon’s own header is the Rule A4.2 read already required for `headers[1]` or `headers[18]`. The other heights in that window are additional reads. They are required for `hike_span` only. They are not a plant scan and they do not choose a different interrupt. Take `header.time` from the beacon’s document only when that header’s `index_comparison` is `match` and the time follows section 9. Take `header.time` from each other document in the window when the read returned a document, `header.height` equals the requested height, and `header.time` follows section 9. The portable index carries no previous-hash member (section 9), and this measurement does not link the window’s headers to each other. If any of the `n` times is missing, MTP for that beacon is `not read`. Otherwise sort the `n` times into non-decreasing order and take the value at index `n // 2`, counting the first as index 0. For the ordinary case `n = 11`, that index is 5, the middle of the 11. A height below 10 uses the shorter window. A beacon on the production profile is above height 10, so `n = 11`.

`MTP(first beacon)` is that value at `stages[1].beacon_height`. `MTP(last beacon)` is that value at `last_interrupt_height`. The verifier reads those windows from the portable index. It does not read a time the producer stored on the object. It does not read `MTP(seed_beacon)` for this span.

- `hike_legs`: the number of checkpoints over stages 1 through 17, the sum of each stage’s checkpoint count, including a stage whose count is zero. Stage 0’s checkpoints are not in this count. The count does not read `leg_count`. It is the same checkpoints `whole_hike_legs` counts. It is not bookend `N`.
- `hike_span`: `MTP(last beacon) − MTP(first beacon)`, which is `MTP(I_17) − MTP(I_0)`, a JSON number in ±(2^53−1) when both MTP values are integers, including zero or a negative value. If either MTP is `not read`, `hike_span` is `not read`. Do not invent an integer. `hike_span` ≤ 0 is still that integer. The rate that uses it is undefined, and Rule B2 refuses. `hike_span` is not `T(last beacon) − T(first beacon)`. It is not `MTP(last_interrupt) − MTP(seed_beacon)`.
- Whole-hike, analysis only: the number of checkpoints over stages 1–17, and the span `T(I_17) − T(I_0)`. `I_0` is `stages[1].beacon`. `I_17` is `last_interrupt`. That checkpoint count matches `hike_legs`. The span does not: `whole_hike_span` is raw `header.time`, and `hike_span` is median-time-past. Stage 0’s checkpoints are in neither count. The count does not read `leg_count`. This pair is not the admission rate.
- Per-stage span: `T(I_0) − T(seed_beacon)` for stage 0; `T(I_g) − T(I_{g−1})` for stages 1–16; `T(I_17) − T(I_16)` for stage 17. The per-stage count is the number of checkpoints in that stage. The per-stage rate display divides that count by that span. It does not read `leg_count`. A per-stage rate is not an admission test. There is no per-stage leg minimum.
- Legs outside the bookends: the number of checkpoints over the stages that are not in `a` … `b−1`. The count does not read `leg_count`. When bookends are **not defined**, this count is **not defined**. This count is not an admission input.

`delta_h` is not in this list. It stays in the bookend block (Rule A4.8).

<a id="rule-a4-11"></a>

#### Rule A4.11 Fewer than two anchored and fetched

Apply these steps in order. Stop at the first step that records the bookend fields. `fixed_start` is a JSON object with keys `N`, `S`, and `delta_h` in every step. `fixed_start` itself is not the string `not defined` or `not read`. The bookend fields are `a`, `b`, `N`, `S`, `delta_h`, `P_b`, `height_P_b`, `index_comparison_P_b`, `T_P_b`, `height_I_a_minus_1`, `T_I_a_minus_1`, `fixed_start.N`, `fixed_start.S`, `fixed_start.delta_h`, and `analysis.legs_outside`.

1. A valid path is a path on `W_1` … `W_16` whose `path_state` is `present` (Rule A4.5), that decodes as `CanonicalPathBody`, and whose `validate_for` succeeds. A `present` path that does not validate is not a valid path. When fewer than two paths among `W_1` … `W_16` are valid, record every bookend field as `not defined`.

2. An issued plant GET is a plant read for a valid path (step 1) whose inclusion height is in the section 13.1 window and whose inclusion header is a stored interrupt. When any issued plant GET does not return a document, record every bookend field as `not read`. This step runs before the anchored count. A `plant` `not read` whose GET was not issued, because the path is outside the window or its inclusion header is not a stored interrupt, does not satisfy this step. That path is `not anchored`. The nineteen Rule A4.2 header reads are not this step.

3. When fewer than two `anchors` tokens among `W_1` … `W_16` are `anchored`, record every bookend field as `not defined`. Unread Rule A4.2 headers do not change this step. The anchor-window vector is this step: three paths are present, one is `anchored`, the issued plant read returned a document, and the header reads returned no document. The bookend fields are `not defined`. The header tokens on that vector are `not read`.

4. Otherwise `a` is the lowest anchored index and `b` is the highest, both in `1 … 16`, and `N` is the checkpoint count over stages `a` … `b−1` (Rule A4.7), including a stage whose checkpoint count is zero. Then read the endpoints. If `headers[a]` (`I_{a−1}`) has `index_comparison` `not read`, or the path at `W_b` does not yield a block, or the header read of `P_b.block_height` is `not read`, record every bookend field as `not read`. A returned document whose comparison is `mismatch` is not this case: `a`, `b`, and `N` stay the integers, `S` is `not read` when a `T` was not taken, and `delta_h` is `not defined` when a height is not index-confirmed (Rule A4.8). An unread header that is not `headers[a]` and is not the `P_b` read does not, by itself, change `a`, `b`, or `N`.

`analysis.whole_hike_span` and each `per_stage` span stay the Rule A4.10 values. They are `not read` when an endpoint `T` was not taken, including when step 1 or step 3 records the bookend fields as `not defined`. When two or more waypoints are `anchored` and step 4 does not apply, Rules A4.6 through A4.8 record the integers.

**Informative.** The legs in `N` start from `W_a`, which hashes `I_{a−1}`, so they could not begin before `I_{a−1}` existed. They are folded into `W_b`, whose plant is in `P_b`, so they existed by then. Naming an older interrupt only makes `S` longer. The interrupts in between, and any skipped waypoints, do not change `N÷S`, apart from `nTime` effects at the two ends of `S`. An honest `S` includes the gap from `I_{b−1}` to `P_b`, which is two, three, or four blocks when headers arrive one at a time. Miner `nTime` can move either end. That remark is about the bookend display. It is not the admission rate. Rule B3 does not adjust `hike_span`. That remark is not a Part A number. A stored interrupt shows that the header exists. It does not show that produce ran live. Only anchored and fetched shows that the fold existed by the plant’s block.

<a id="sec-14-5"></a>

### 14.5 Anchored and fetched

**Normative.**

`anchored_total` is Rule A4.5: the count of the `anchors` token `anchored` among `W_1` … `W_16`. A `skip` is not in that count. A present path outside the window is not in that count. A `present` path with `plant` `mismatch` or `not read` is not in that count. `W_17` is not in that count. This section records the count and does not judge it. Rule B4 compares the count with the lever `min_anchored_paths` (section 15.4). That comparison is not a Part A measurement. How many waypoints are fetchable is Rule A3.3.

<a id="sec-14-6"></a>

### 14.6 Six-leg check

**Normative.**

<a id="rule-a4-4"></a>

#### Rule A4.4 Six-leg check

Six legs are drawn. The draw count is 6. It is not `STAGES`. `N` in this file is bookend legs (Rule A4.7). This section does not use `N` for the hike’s leg total.

A global leg index addresses one leg of the hike. Let `L_g` be the number of checkpoints in stage `g`, `len(stages[g].checkpoints)`. The draw does not read `leg_count`. `start_0 = 0`, and `start_{g+1} = start_g + L_g`. Let `leg_total = start_{STAGES}`. The index `q` with `start_g ≤ q < start_{g+1}` is leg `j = q − start_g` of stage `g`. `leg_count ≥ 1` per stage is the section 13.2 loop, recorded under shape. A shape `mismatch` is Rule B9. It is not this draw. The draw rule is stated on `leg_total`, including `6 ≤ leg_total < 18`. This section does not turn that range into a shape `mismatch`, and it does not delete it. There is no `AUDIT_K`.

Before the draw, if there are fewer than 6 legs, `six_leg` is `not defined`, `six_leg_runs` is `[]`, and the draw is not made. That array is `[]` in this case. It is not `[]` when the draw is `not read`.

Each verification draws its own 6 distinct global indices, uniformly, from the operating-system CSPRNG. The `getrandom` interface is one conforming way to read that CSPRNG. The bytes are fresh for that verification. Another verification draws again. The two draws are not required to be the same indices.

One unbiased draw from `0 … leg_total−1` uses mathematical integers:

```text
M = 2^64
limit = M − (M mod leg_total)
repeat:
    read 8 bytes from the CSPRNG
    u = u64_le(those bytes)          # 0 ≤ u < M
    if u < limit:
        return u mod leg_total
```

When `leg_total` is a power of two, `limit = M` and the first draw is used.

The measurement repeats that draw, discarding an index already chosen, until it holds 6 distinct indices.

An 8-byte draw with `u >= limit` is consumed. Those bytes are not read again. If the byte source cannot supply the next 8 bytes before 6 distinct indices have been obtained, record the draw as **not read**. A short-byte six-leg draw is that `not read`. It is not `not defined`. Short path bytes for that run are the same `not read`. When the six-leg draw is `not read`, `six_leg_runs` is `not read` and `six_leg` is `not read`. That array is not `[]`. A later verification may draw again. Indices drawn before that are not the six leg results.

The index draw in this section runs on the calling thread. It finishes before any leg result is recomputed. A verifier may recompute the six leg results concurrently, and may run Rule A4.3 checkpoint comparisons concurrently. How many Rule A4.3 comparisons run at once is a verifier setting. The verify record does not depend on that number. One at a time is conforming. An SDK verifier may default to six concurrent Rule A4.3 comparisons. That default is not a conformance constant. Elements of `six_leg_runs` may execute in parallel. This file does not require a thread pool. That one-at-a-time permission is the section 14 measurement record. It does not apply to the `thorough_rounds` of the policy in force (section 15.5).

For each chosen index, recompute that leg from section 11.2. The previous checkpoint is that stage’s leg-0 start when `j = 0` (Rule A2.2): `SHA-256(public_key ‖ seed_beacon)` on stage 0, and the stored `waypoint` on stage `g ≥ 1`. Otherwise it is the stored `checkpoints[j − 1]` of that stage. The draw count is 6 either way.

The order of the chosen indices is the order drawn. The order in which the leg computations finish does not change that order. For each chosen index, record `match` if the recomputed leg result equals `u64_le(leg_results[j])`. Record `mismatch` otherwise.

<a id="sec-14-7"></a>

### 14.7 Unknown kind

**Normative.**

<a id="rule-a5"></a>

#### Rule A5. Unknown kind

An object whose `kind` text is not exactly `KIND` (`v2.2-pcoc-object`) is an unknown kind. No other label is recognised, and no label names another construction. The check is the kind text. It runs before any terrain fill. Verify records:

```json
{"kind":"unknown kind","kind_text":"<the kind text>"}
```

`kind` is the measurement `unknown kind`. `kind_text` is the kind text the object carried, as a JSON string. The record is that two-key object, keys in that order, with no insignificant whitespace, and `kind_text` spelled as RFC 8785 spells a JSON string; that is also the RFC 8785 encoding of the object. A conforming verify CLI writes it as it writes the section 14 record, with one trailing newline (U+000A). The newline is not part of the record. The pack’s unknown-kind vector is the record bytes without the newline. The record does not apply sections 14.1–14.6. It is not a decode failure. It is not `shape` `mismatch`. It does not say the object could not be read. An unknown kind is not a fake leg. The admission service refuses it with reason `unknown_kind` (Rule B9) and does not add its key to the ignore record. One well-formed data item that is not a map, or a map that does not yield a `kind` text, is still a decode failure (section 7.3). A malformed CBOR value is not a decode failure and is not Rule A5.

## 18. Verify CLI

**Normative.** The verify CLI emits only section 14 record tokens. Stdout for a current object is the one section 14 record, keys in the order section 14 lists. The compact line is not a conformance output and this CLI does not write it. Exit code `0` means the record was produced, whatever it measured, including all `not read` or `mismatch`. Exit code `1` means the object file cannot be read, or its bytes are a decode failure under section 7.3. A malformed CBOR value is a record, exit code `0`. That is the object file, not the `not read` token. Exit code `2` means a usage or invocation error. No other exit code. Stdout and stderr carry no pass, fail, accept, refuse, or admission wording. A named `--profile` is `production` or `conformance-small`. An omitted `--profile`, `test`, and `live` still measure with the section 6 constants. `--profile live` selects the live index only. Admission never runs inside verify. `--profile` is not a Gate policy profile. Profile judgment lives only in the policy library and the Gate. When the profile name is `conformance-small`, all four integers are required inputs. The verifier does not substitute the section 6 values for an omitted integer. It does not change `KIND`. A verify CLI that passes `--profile conformance-small` without all four integers is a usage error, exit code 2. This file assigns no default integers to that name. The record states the four integers it was given.

<a id="sec-19"></a>
