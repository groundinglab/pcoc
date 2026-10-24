# Satoshi Hike Construction

**Responsibility.** Defines the object, the terrain, the leg, the stage and waypoint binding, the encodings, and the two-way Bitcoin commitment bind. Used with Conformance as the definitional pair. Producer and Measurement are process documents constrained by this definition.

**Source.** Inherited without change of wording from the following sections of Satoshi Hike 2.3 (tag `spec-2.3.0`; formerly the monolith at `spec/satoshi-hike-v2.3/SPEC.md`, not present on this branch’s public face):

- §1 Status
- §2 Scope
- §3 Interpretation
- §4 Glossary
- §6 Constants
- §7 Encodings
- §8 Hike object
- §9 Clock
- §10 Terrain
- §11 Leg
- §12 Stage and waypoint

**FRAMING (2026-10-24 relocation).** §14 Verify (measurement procedure and verify-record shape) moved to [06-Measurement_Record_Process.md](./06-Measurement_Record_Process.md). Rule-index IDs A4/A5 still name those rules; their bodies live in Measurement. Part B / admission lever chatter that formerly sat in Status is not owned here — see Admission Process and policy instances outside this pack. Construction owns the Bitcoin bind (headers into the hike; anchors out). Waypoint Publication is the scoped batching channel for the anchor side.

---

## 1. Status

The status prose in this section is **informative**. The rule index is the ID-to-heading map. The subsection “Part A rule classes” is the **normative** classification.

This document is Satoshi Hike 2.3 Construction (Part A object definition). Part A is the construction: `SAMPLE_BYTES` 1024, `STEPS_PER_LEG` 2000000, `KIND` `v2.2-pcoc-object`, anchor-window offsets 2, 3, and 4, the NL4 terrain word, the §11.1 fold, and the asynchronous producer event model (Rule A6 lives in Producer Process). **FRAMING.** Unknown-kind measurement (Rule A5) and the full §14 measure procedure live in Measurement Record. Section 13.6 Bari waypoint service lives in Waypoint Publication. Part B admission levers, judgment, Rule B7/B9/B10, and section 15.5 floor measuring method live in Admission Process (value-free process) and/or outside policy instances (`gl-admit-*`); this Construction file does not set lever values. Section 16.2 implementation requirements live in Conformance.

### Rule index

**Normative** for the ID-to-heading map. The heading text in the section named is the rule. A test named for an ID tests that heading. This table does not add a second rule. Classes for these headings, and for unnumbered Part A sentences, are in “Part A rule classes” below.

| ID | Heading | Section |
|---|---|---|
| A1 | Rule A1. Event order at every stage boundary | 13 |
| A1.1 | Rule A1.1 Legs, then the tip | 13 |
| A1.2 | Rule A1.2 The interrupt ends the stage | 13 |
| A1.3 | Rule A1.3 Waypoint byte order | 13 |
| A1.4 | Rule A1.4 Post (asynchronous) | 13 |
| A1.5 | Rule A1.5 Pivot | 13 |
| A6 | Rule A6. Nothing pauses a leg | 13 |
| A2 | Rule A2. Stages and where leg 0 starts | 12.1 |
| A2.1 | Rule A2.1 Eighteen stages | 12.1 |
| A2.2 | Rule A2.2 Leg 0 | 12.1 |
| A2.3 | Rule A2.3 Checkpoint | 12.1 |
| A2.4 | Rule A2.4 First post | 12.1 |
| A3 | Rule A3. What the object stores | 8 |
| A3.1 | Rule A3.1 Headers | 8 |
| A3.2 | Rule A3.2 Per stage | 8 |
| A3.3 | Rule A3.3 Paths | 8 |
| A3.4 | Rule A3.4 Produce stderr | 13.3 |
| A4 | Rule A4. What verify measures against the portable index | 14 |
| A4.1 | Rule A4.1 Shape | 14.1 |
| A4.2 | Rule A4.2 Headers | 14.2 |
| A4.3 | Rule A4.3 Chain | 14.3 |
| A4.4 | Rule A4.4 Six-leg check | 14.6 |
| A4.5 | Rule A4.5 Paths | 14.3 |
| A4.6 | Rule A4.6 Bookends | 14.4 |
| A4.7 | Rule A4.7 Bookend legs | 14.4 |
| A4.8 | Rule A4.8 Bookend span | 14.4 |
| A4.9 | Rule A4.9 Bookend leg rate | 14.4 |
| A4.10 | Rule A4.10 Analysis | 14.4 |
| A4.11 | Rule A4.11 Fewer than two anchored and fetched | 14.4 |
| A5 | Rule A5. Unknown kind | 14.7 |
| B0 | Rule B0. Which rate the admission service reads | 15 |
| B2 | Rule B2. Leg-rate floor | 15 |
| B3 | Rule B3. Span sanity | 15 |
| B4 | Rule B4. Paths minimum | 15 |
| B6 | Rule B6. Fixed start | 15 |
| B7 | Rule B7. Resubmits | 15 |
| B8 | Rule B8. Leg count | 15 |
| B9 | Rule B9. Invariant refuses | 15.2 |
| B10 | Rule B10. Submission deadline | 15 |
| BR1 | Rule BR1. Paid waypoint | 13.6 |
| BR2 | Rule BR2. Dedupe | 13.6 |
| BR3 | Rule BR3. Pool and tree | 13.6 |
| BR4 | Rule BR4. Path serve | 13.6 |
| I1 | Rule I1. SDKs are the reference implementations | 16.2 |
| I2 | Rule I2. Interfaces | 16.2 |
| I3 | Rule I3. Producer model | 16.2 |
| I4 | Rule I4. Tests | 16.2 |
| I5 | Rule I5. Versioning and pin | 16.2 |
| I6 | Rule I6. Cost | 16.2 |

### Part A rule classes

**Normative.**

This subsection is the normative classification of Part A. It does not add a second construction. Each row is one rule and one class. Informative prose, section 5, section 13.6 (Bari rules BR1–BR4, classed there), section 16.2 (Rules I1–I6), section 7.4, section 13.4, the expected-case list in section 13.1, and sections 17, 18, and 19 are not rows. A glossary sentence that only renames a row is not a second row. The **Normative** and **Informative** marks on the sections a row cites stay where those sections put them.

**Rule ID scheme.**

- Heading IDs in the rule index stay A1–A5 and their dotted children. A heading that states one rule of one class is that row.
- When one heading states more than one class, the rows are `ID.n` in SPEC order (`n` starts at 1). Those rows are the classification. The parent ID is the group and is not given a second class.
- A2 is a group heading. A2.1–A2.4 are the rows. A2 is not a separate class.
- A normative Part A sentence that has no heading ID gets `P<section>.<n>`, in reading order. The digits before the first dot are the SPEC section number. `P` marks a Part A construction paragraph. It does not renumber A1–A5 and it is not Part B. An ID that is not in this table is unassigned and is not used.
- Classes are `INVARIANT`, `CONSTANT`, `RANGE`, and `POLICY THRESHOLD`. An INVARIANT holds byte for byte or as an exact measurement. A CONSTANT is a fixed parameter of this version. A RANGE is an allowed value or format. A POLICY THRESHOLD is admission only.
- Part A has no POLICY THRESHOLD row. Sentences that can be misread as thresholds are listed after the table and stay in the class written here. Admission thresholds are named parameters in section 15. Rule B9 is the one section 15 row in this table. It classes those refuses. It is not a Part A measurement.

| Rule ID | Class | One-line rule | SPEC locus |
|---|---|---|---|
| <a id="rule-p2-2"></a>P2.2 | INVARIANT | Produce and verify perform no signature check. The admission service checks the signature (section 15). | §2, §15 |
| <a id="rule-p3-1"></a>P3.1 | INVARIANT | Ranges are half-open; `0^n` is `n` zero bytes; `‖` is raw concatenation; `H` is FIPS 180-4 SHA-256. | §3 |
| <a id="rule-p3-2"></a>P3.2 | RANGE | `le64` and `u64_le` are 8 little-endian bytes of an unsigned integer below `2^64`. | §3 |
| <a id="rule-p3-3"></a>P3.3 | INVARIANT | `mod` is the non-negative remainder; `mod 2^64` keeps the low 64 bits. | §3 |
| <a id="rule-p4-1"></a>P4.1 | RANGE | Outcome tokens are the closed lowercase set. There is no token `not obtained`. | §4 |
| <a id="rule-p4-2"></a>P4.2 | INVARIANT | Part A, the construction named in [section 1](#sec-1), does not pass, fail, reject, deny, or admit. | §4 |
| <a id="rule-p6-1"></a>P6.1 | CONSTANT | `SLOT_BYTES` is 16384. | §6 |
| <a id="rule-p6-2"></a>P6.2 | CONSTANT | `WORDS_PER_SLOT` is 2048. | §6 |
| <a id="rule-p6-3"></a>P6.3 | CONSTANT | `TERRAIN_BYTES` is 268435456. | §6 |
| <a id="rule-p6-4"></a>P6.4 | CONSTANT | `N_SLOTS` is 16384. | §6 |
| <a id="rule-p6-5"></a>P6.5 | CONSTANT | `SAMPLE_BYTES` is 1024. | §6 |
| <a id="rule-p6-6"></a>P6.6 | CONSTANT | `OFFSET_MODULUS` is 268434433. | §6 |
| <a id="rule-p6-7"></a>P6.7 | CONSTANT | `STEPS_PER_LEG` is 2000000. | §6 |
| <a id="rule-p6-8"></a>P6.8 | CONSTANT | `STAGES` is 18. | §6 |
| <a id="rule-p6-9"></a>P6.9 | CONSTANT | `HASH_BYTES` is 32. | §6 |
| <a id="rule-p6-10"></a>P6.10 | CONSTANT | `LEG_RESULT_BYTES` is 8. | §6 |
| <a id="rule-p6-11"></a>P6.11 | CONSTANT | `PUBLIC_KEY_BYTES` is 32. | §6 |
| <a id="rule-p6-12"></a>P6.12 | CONSTANT | `BEACON_BYTES` is 32. | §6 |
| <a id="rule-p6-13"></a>P6.13 | CONSTANT | `SEAL_DOMAIN` is the twelve bytes `pcoc-walk-v1`. | §6 |
| <a id="rule-p6-15"></a>P6.15 | CONSTANT | `KIND` is `v2.2-pcoc-object`. | §6 |
| <a id="rule-p6-16"></a>P6.16 | CONSTANT | `FILL` is `dag2`. | §6 |
| <a id="rule-p6-17"></a>P6.17 | CONSTANT | `MIX_ADD` is `0x9E3779B97F4A7C15`. | §6 |
| <a id="rule-p6-18"></a>P6.18 | CONSTANT | `MIX_MUL1` is `0xBF58476D1CE4E5B9`. | §6 |
| <a id="rule-p6-19"></a>P6.19 | CONSTANT | `MIX_MUL2` is `0x94D049BB133111EB`. | §6 |
| <a id="rule-p6-20"></a>P6.20 | CONSTANT | `WORD_XOR` is `0xC0FFEE`. | §6 |
| <a id="rule-p6-21"></a>P6.21 | CONSTANT | `PARENT_MUL` is `0xD1B54A32D192ED03`. | §6 |
| <a id="rule-p6-22"></a>P6.22 | CONSTANT | `SLOT_MUL` is `0xA24BAED4963EE407`. | §6 |
| <a id="rule-p6-23"></a>P6.23 | INVARIANT | `WORDS_PER_SLOT = SLOT_BYTES / 8`, and the division is exact. | §6 |
| <a id="rule-p6-24"></a>P6.24 | INVARIANT | `N_SLOTS = TERRAIN_BYTES / SLOT_BYTES`, and the division is exact. | §6 |
| <a id="rule-p6-25"></a>P6.25 | INVARIANT | `OFFSET_MODULUS = TERRAIN_BYTES − SAMPLE_BYTES + 1`. | §6 |
| <a id="rule-p6-26"></a>P6.26 | INVARIANT | This version has no `AUDIT_K`. The six-leg draw count is not a section 6 constant. | §6 |
| <a id="rule-p7-1"></a>P7.1 | INVARIANT | The first eight bytes of a digest are `b[0:8]`. `u64_le` does not reverse a digest a second time. | §7.1 |
| <a id="rule-p7-2"></a>P7.2 | RANGE | Hex uses `0-9`, `a-f`, and `A-F` with no spaces. A 64-digit spelling is 32 bytes, byte 0 first. | §7.2 |
| <a id="rule-p7-3"></a>P7.3 | INVARIANT | Produce emits one CBOR map in core deterministic encoding. | §7.3 |
| <a id="rule-p7-4"></a>P7.4 | INVARIANT | Top-level and per-stage key orders are the orders section 7.3 lists. Stage 0 has no `path` key. | §7.3 |
| <a id="rule-p7-5"></a>P7.5 | RANGE | Text is definite-length UTF-8. Byte fields, unsigned integers, and definite arrays are as section 7.3 states. | §7.3 |
| <a id="rule-p7-6"></a>P7.6 | RANGE | Verify decodes any definite-length encoding of the same values, including a non-shortest unsigned integer. Key order is not a shape `mismatch`. | §7.3 |
| <a id="rule-p7-7"></a>P7.7 | INVARIANT | Malformed CBOR is shape `mismatch`, and `shape_item` includes `malformed cbor`. Later measurements are still recorded. | §7.3 |
| <a id="class-a3"></a>A3 | INVARIANT | The hike object has exactly the top-level fields, types, and lengths in the section 8 table. | §8 Rule A3 |
| <a id="rule-p8-1"></a>P8.1 | INVARIANT | Evidence is the hash-bound set in section 8. Convenience signposts are checked against the evidence or the index and never relied on. | §8, §14 |
| <a id="rule-p8-2"></a>P8.2 | INVARIANT | Stored heights are convenience. `d_g` and `delta_h` subtract index-confirmed heights only. | §8, §14.3, §14.4 |
| <a id="class-a3-1"></a>A3.1 | INVARIANT | The object stores `seed_beacon` and all 18 interrupts, with strictly later heights and differing hashes, and stores no block time. | §8 Rule A3.1 |
| <a id="class-a3-2"></a>A3.2 | INVARIANT | Each stage map has exactly the listed fields. Array lengths equal `leg_count`. `leg_count` is at least 1. Stage 0’s `waypoint` is the seed. | §8 Rule A3.2 |
| <a id="rule-a3-3-1"></a>A3.3.1 | INVARIANT | Stage 0 omits `path`. An empty byte string is not a stage-0 path. “not planted by design” is not a token. | §8 Rule A3.3 |
| <a id="rule-a3-3-2"></a>A3.3.2 | INVARIANT | `stages[17].path` is always empty. `W_17` is not asked, including on a jump. The record is `not fetched by design`. | §8 Rule A3.3 |
| <a id="rule-a3-3-3"></a>A3.3.3 | CONSTANT | Sixteen waypoints, `W_1` through `W_16`, are fetchable. `W_17` is not. | §8 Rule A3.3 |
| <a id="rule-a3-3-4"></a>A3.3.4 | INVARIANT | The object has no terrain field. A stored leg result is `le64` of the section 11 `u64`. | §8 |
| <a id="class-a3-4"></a>A3.4 | INVARIANT | `path_skip <g>` and `not fetched by design 17` are the exact stderr lines, in that order, at the close. That step does not delay the object. Empty bytes are not a silent skip. | §13.3 Rule A3.4 |
| <a id="rule-p9-1"></a>P9.1 | RANGE | `{tip-url}`, `{index-url}`, and `{bari-url}` are absolute `http` or `https` URLs with no trailing slash. | §9, §13 |
| <a id="rule-p9-2"></a>P9.2 | INVARIANT | A clock read is one GET, does not follow a redirect, and is successful only on status 200. | §9 |
| <a id="rule-p9-3"></a>P9.3 | RANGE | A successful body is one UTF-8 JSON object, with no BOM, no second value, and no duplicate member name. | §9 |
| <a id="rule-p9-4"></a>P9.4 | RANGE | `header.hash` is 64 hex digits in Bitcoin display order. The natural beacon reverses those 32 bytes. | §9 |
| <a id="rule-p9-5"></a>P9.5 | RANGE | `header.height` is a JSON number in `0 … 2^53 − 1`, not a string and not a non-integer JSON number. An index height above that bound is an unsuccessful read: `not read`. An integer above that bound in `beacon_height` or `last_interrupt_height` is section 14 `shape_item` `height_range`. | §9, §14 |
| <a id="rule-p9-6"></a>P9.6 | RANGE | `header.time` is a JSON integer in `0 … 2^32 − 1` Unix seconds, not a string and not a non-integer JSON number. | §9 |
| <a id="rule-p9-7"></a>P9.7 | INVARIANT | On mainnet, `header.time` equals the `nTime` of the block whose hash is `header.hash`. Section 14 measures the portable index. A direct Bitcoin read is outside that procedure. | §9, §14 |
| <a id="rule-p9-8"></a>P9.8 | CONSTANT | With no named gossip peer, the starting list is the copied Bitcoin Core mainnet DNS seeds, in the listed order, port 8333. | §9 |
| <a id="rule-p9-10"></a>P9.10 | INVARIANT | Produce Observe reads the plane tip sequence and does not GET an HTTP tip or the portable index. | §9 |
| <a id="rule-p9-11"></a>P9.11 | INVARIANT | The interrupt is the first post-checkpoint tip read whose hash differs and whose height is strictly greater. The tip read returns at once the latest header the header source holds and never waits. There is no rebinding. | §9, Rule A1 |
| <a id="rule-p9-12"></a>P9.12 | INVARIANT | No clock-read failure or other transitory network condition stops a run or withholds the object. Produce logs it (section 13.5) and the walk continues. An unsuccessful Hashcash, POST, or path-ask read is a miss, and produce continues. A run ends only at `I_17`, on a producer failure, or on explicit cancellation. | §9, Rule A6 |
| <a id="rule-p9-13"></a>P9.13 | INVARIANT | Verify does not Observe, does not GET an HTTP tip, and does not GET a Bari path. It reads the portable index at each stored height, and it reads one path inclusion height only when that height is in the anchor window and that header is a stored interrupt. | §9, §14 |
| <a id="rule-p9-14"></a>P9.14 | INVARIANT | The seed beacon is the natural beacon from the first successful Observe, before any leg. That Observe is the latest header already on the tip sequence. It does not wait for a header mined after produce starts. Until a seed read succeeds the run has not started, so no leg exists to pause. | §9 |
| <a id="rule-p9-15"></a>P9.15 | INVARIANT | A height is not an input to `SHA-256` in this construction. | §9 |
| <a id="rule-p9-16"></a>P9.16 | CONSTANT | On the live plane the header source keeps at least `HEADER_PEERS_MIN` 2 gossip peers. A connected peer that has not announced a header within `HEADER_PEER_SILENCE_SECONDS` 120 of another connected peer announcing it is replaced. Below 2 peers the source reconnects asynchronously with backoff and writes an `alert` `header_peers_low`; the walk and the object are unaffected. | §9 |
| <a id="rule-p9-17"></a>P9.17 | INVARIANT | Every Rule BR1 block-hash comparison (current, before, after) is natural order on both sides: `SHA-256(SHA-256(header))` and the header’s previous-block field in wire order, the same bytes as `header_hash`. A display-hex tip hash is reversed once before comparing. A match only under reversal is a 402. | §9, §13.6 |
| <a id="rule-p10-1"></a>P10.1 | INVARIANT | The seed preimage is `public_key ‖ seed_beacon`, 64 bytes. The seed is `SHA-256` of that preimage, 32 bytes, no label, read as four little-endian lanes of 8 bytes each, `P[0]` … `P[3]`. | §10 |
| <a id="rule-p10-2"></a>P10.2 | INVARIANT | `mix64`, the NL4 base word, the parent indices, and the dag2 fill are the section 10 construction. Every terrain word depends on all 256 bits of the seed. Terrain is filled once per terrain. The step does not write it. | §10 |
| <a id="rule-p10-3"></a>P10.3 | INVARIANT | The seed equals `stages[0].waypoint` by design. | §10, §12.1 |
| <a id="rule-p11-1"></a>P11.1 | INVARIANT | A step folds every word of the selected slot by §11.1 (`acc` starts at 0 and grows by wrapping addition; `mix64` includes `MIX_ADD`) and sets the next state from `SHA-256(SEAL_DOMAIN ‖ le64(S_folded))`. | §11.1 |
| <a id="rule-p11-2"></a>P11.2 | INVARIANT | The leg result is `STEPS_PER_LEG` steps from the first eight bytes of the previous checkpoint, stored as `le64`. | §11.2 |
| <a id="rule-p11-3"></a>P11.3 | INVARIANT | The terrain sample is `SAMPLE_BYTES` at the offset taken from the previous checkpoint. It is not the waypoint slot sample. | §11.3 |
| <a id="class-a2-1"></a>A2.1 | CONSTANT | `seed_beacon` plus the 18 interrupts gives stages numbered 0–17. | §12.1 Rule A2.1 |
| <a id="class-a2-2"></a>A2.2 | INVARIANT | The seed is `SHA-256(public_key ‖ seed_beacon)`, stored at `stages[0].waypoint`, never posted, and not one of `W_1` … `W_17`. The seed equals that field by design. Leg 0 starts from it or from `W_g`. | §12.1 Rule A2.2 |
| <a id="class-a2-3"></a>A2.3 | INVARIANT | The checkpoint preimage is the previous checkpoint, the stage limb, the terrain sample, and `le64(leg_result)`, in that order. | §11.4, §12.1 Rule A2.3 |
| <a id="class-a2-4"></a>A2.4 | INVARIANT | The first Bari post is `W_1`, started after `I_0`, alongside stage 1’s legs. Stage 0 has no plant and no path. | §12.1 Rule A2.4 |
| <a id="class-a1-3"></a>A1.3 | INVARIANT | `W_{g+1}` is `SHA-256` of `I_g`, its slot sample, `stages[g].waypoint`, and the fold of that stage’s checkpoints. `slot_index` is `u64_le` of the first eight bytes of `I_g`, modulo `N_SLOTS`. No waypoint is built from `I_17`. | §12.2, §13 Rule A1.3 |
| <a id="class-a1"></a>A1 | INVARIANT | At each stage boundary the order is legs, then the tip read, then the interrupt, then the waypoint bytes, then the pivot. Stage `g+1`’s leg 0 starts at once from the pivot. The post is an asynchronous event started at the pivot and is not ordered before any leg. There is no rebinding. | §13 Rule A1 |
| <a id="class-a1-1"></a>A1.1 | INVARIANT | Legs in a stage run back to back. The tip is read after each checkpoint; that read returns at once and never waits. A leg already in progress finishes. | §13 Rule A1.1 |
| <a id="class-a1-2"></a>A1.2 | INVARIANT | When the tip read shows interrupt `I_g`, leg production in stage `g` stops. | §13 Rule A1.2 |
| <a id="class-a1-4"></a>A1.4 | INVARIANT | When `W_{g+1}` is known the producer starts its post to Bari with the Hashcash fee as an asynchronous event. The post does not delay, order, or pause any leg, and its outcome is not an input to any leg, checkpoint, leg result, or waypoint. | §13 Rule A1.4 |
| <a id="class-a1-5"></a>A1.5 | INVARIANT | `W_{g+1}` is the pivot. Stage `g+1` leg 0 starts from it at once. | §13 Rule A1.5 |
| <a id="class-a6"></a>A6 | INVARIANT | No post, Hashcash solve, ask, peer or header-source read, Bari exchange, or log write pauses, orders, delays, or interrupts a leg. Each is an asynchronous event. The post-checkpoint tip read stays on the walk path and never waits. | §13 Rule A6 |
| <a id="rule-p13-1"></a>P13.1 | INVARIANT | Produce sets no wall-clock bound, no steps-per-second quota, and no leg-count quota. `leg_count ≥ 1` is the drive loop. | §13, §13.3 |
| <a id="rule-p13-2"></a>P13.2 | CONSTANT | `ANCHOR_WINDOW_OFFSETS` is 2, 3, 4. That list is written once in §13.1. A plant posted at height H is asked only at H plus each offset. The path is gone one header after the last offset. | §13.1 |
| <a id="rule-p13-3"></a>P13.3 | INVARIANT | Produce never pauses a leg: not before the first leg of a stage and not between legs (Rule A6). A stage that ends at one leg, when the next header is the interrupt, is a world event the producer recorded. | §13.1 |
| <a id="rule-p13-4"></a>P13.4 | INVARIANT | The Hashcash challenge is `SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)` and the digest is `SHA-256(challenge ‖ waypoint ‖ le64(nonce))`, as section 13.1 defines. Leading zero bits are counted from the first digest byte. | §13.1 |
| <a id="rule-p13-5"></a>P13.5 | CONSTANT | The Hashcash difficulty is 21 leading zero bits, the one value for produce and for Bari. | §13.1 |
| <a id="rule-p13-6"></a>P13.6 | INVARIANT | The nonce search starts at 0. Produce sends one waypoint POST as an asynchronous event. Status 204 means Bari holds the waypoint. The only retry is one background resend 30 seconds after a transitory send failure (P13.13). Any other outcome is logged and does not withhold the object. H is the stage beacon height in §13.1. There is no fee bump. | §13.1 |
| <a id="rule-p13-7"></a>P13.7 | INVARIANT | The leaf is 64 lowercase hex digits, byte 0 first. A due ask is sent, as an asynchronous event, when the producer’s header source learns the due header. There is no wait before an ask. | §13.1 |
| <a id="rule-p13-8"></a>P13.8 | INVARIANT | A checked path is a status 200 canonical path body that validates for that waypoint and whose block is one height in the anchor window. Every other ask result is a miss. | §13.1 |
| <a id="rule-p13-9"></a>P13.9 | INVARIANT | The cut-off is `last_interrupt_height`. Production ends when the tip read shows `I_17`, and the object closes at once with the checked paths that have arrived by then. There is no collection window. An ask in flight at the close is abandoned, and no ask is made after it. `W_17` is not asked. | §13.1 |
| <a id="rule-p13-10"></a>P13.10 | INVARIANT | A stage’s `leg_count` is a convenience copy of the length of that stage’s `checkpoints`. Produce does not invent path bytes. No metric reads `leg_count`. | §13.3 |
| <a id="rule-p13-11"></a>P13.11 | INVARIANT | The live event log is not the hike object and is not an input to a leg, a checkpoint, a leg result, or a waypoint. | §13.5 |
| <a id="rule-p13-12"></a>P13.12 | RANGE | A public-key input is 32 bytes, or 64 hex digits with an optional leading `0x`. The object stores the 32 bytes. | §13 |
| <a id="rule-p13-13"></a>P13.13 | INVARIANT | After a transitory send failure (a transport failure, section 13.1) the producer resends the identical waypoint POST exactly once, in the background, 30 seconds later, then drops it if the resend fails. Both attempts are logged. This is the only retry of a waypoint POST. | §13.1 |
| <a id="rule-p13-14"></a>P13.14 | INVARIANT | Plant height H is `stages[g].beacon_height` for `W_g`, written once in §13.1. The fee header on the POST is not H. | §13.1 |
| <a id="rule-p13-15"></a>P13.15 | INVARIANT | The live event log uses the six record types in section 13.5, in the key order that section lists. | §13.5 |
| <a id="rule-p13-17"></a>P13.17 | INVARIANT | Event-log records are appended asynchronously. Produce does not wait for a record to be written, flushed, or synced. | §13.5 |
| <a id="rule-p13-16"></a>P13.16 | INVARIANT | The waypoint POST is `POST {bari-url}/waypoint`. The body is the 32 waypoint bytes. `Content-Type` is `application/octet-stream`. The headers are `X-Bari-Hashcash-Nonce` and `X-Bari-Hashcash-Header` (the 80-byte fee header). | §13.1 |
| <a id="class-a4"></a>A4 | INVARIANT | Verify records measurements against the object and the portable index and does not judge. Section 15 is not applied in section 14. | §14 Rule A4 |
| <a id="rule-p14-1"></a>P14.1 | INVARIANT | Anyone may read Bitcoin directly. That read is outside the section 14 procedure. Section 14 measures the portable index (P9.7). P9.7 is not a conformance-vector check. A LAN match is not a mainnet fact. | §14 |
| <a id="rule-p14-2"></a>P14.2 | RANGE | The verify record is the JSON object section 14 defines: required keys, no JSON null, decimal integers, and the stated array lengths. | §14 verify record |
| <a id="rule-p14-3"></a>P14.3 | RANGE | `shape_item` uses only the 14 strings in that table, in that order. Malformed CBOR is exactly `malformed cbor`. `height_range` is an object-byte height above `2^53 − 1`. | §14 verify record |
| <a id="rule-a4-1-1"></a>A4.1.1 | INVARIANT | `shape` is `match` only when every shape condition holds. Otherwise it is `mismatch`, and later measurements are still recorded. | §14.1 Rule A4.1 |
| <a id="rule-a4-1-2"></a>A4.1.2 | INVARIANT | `fill` and the size fields equal the profile in force (a `kind` other than `KIND` is Rule A5), or `shape_item` includes `constant`. The profile in force is section 6, unless the verifier runs `conformance-small`. | §14.1, §16.1 |
| <a id="rule-a4-1-3"></a>A4.1.3 | INVARIANT | Shape records the field set, widths, stage-0 `path` key, empty `W_17` path, `stages` length, `leg_count`, `seed_beacon`, and header order. | §14.1 |
| <a id="rule-a4-2-1"></a>A4.2.1 | INVARIANT | Verify reads every stored header from the portable index, in order, including `last_interrupt`. One header does not skip a later header. | §14.2 Rule A4.2 |
| <a id="rule-a4-2-2"></a>A4.2.2 | INVARIANT | `index_comparison` is `match`, `mismatch`, or `not read`, from hash and height together. Time is not part of that token. | §14.2 |
| <a id="rule-a4-2-3"></a>A4.2.3 | INVARIANT | `T` is `header.time` only when `index_comparison` is `match`. On `mismatch`, `T` is `not read`. | §9, §14.2 |
| <a id="rule-a4-3-1"></a>A4.3.1 | INVARIANT | Verify recomputes each checkpoint digest and the seed and records `match` or `mismatch`. A missing leg result is `mismatch`. This check does not run a leg. | §14.3 Rule A4.3 |
| <a id="rule-a4-3-2"></a>A4.3.2 | INVARIANT | Verify recomputes `W_1` … `W_17` from stored bytes under section 12.2. No waypoint is recomputed from `last_interrupt`. | §14.3 |
| <a id="rule-a4-5-1"></a>A4.5.1 | INVARIANT | Empty path bytes on `W_1` … `W_16` are `skip` and plant `n/a`. Any other bytes are `present`. Empty `W_17` is `not fetched by design`. | §14.3 Rule A4.5 |
| <a id="rule-a4-5-2"></a>A4.5.2 | INVARIANT | Anchored and fetched is the `anchors` token `anchored`: in the section 13.1 window, an interrupt header stored on the object, and plant `match`. `anchored_total` is that count. This row records it and does not judge it. Rule B4, settled and normative, compares it with 6. **FRAMING (nit):** inherited “6”; Rule B4’s normative comparison is `anchored_total ≥ min_anchored_paths` (lever, policy). Leave this sentence; policy owns the value. | §14.3, §14.5 |
| <a id="rule-a4-5-3"></a>A4.5.3 | INVARIANT | `d_g` subtracts index-confirmed heights only, and only when plant is `match` and the index comparison of `I_{g−1}` is `match`. Otherwise `d_g` is `not defined`. | §4, §14.3 |
| <a id="rule-p14-4"></a>P14.4 | INVARIANT | A present path’s plant is compared with the portable index. A read that does not return a document is `not read`. A document that was read with no plant, or with no matching plant, is `mismatch`. Verify does not GET Bari. | §14.3 |
| <a id="rule-p14-5"></a>P14.5 | INVARIANT | A returned plant document’s `network` is the string `main` or the string `synthetic`. Any other string, or a missing `network`, is `plant` `mismatch`. There is no verifier input that selects one of those two strings. Accepting `synthetic` does not make the measurement a mainnet fact. Two plants may share a block. `tree_size` is not compared. | §14.3 |
| <a id="rule-p14-6"></a>P14.6 | INVARIANT | A present path that does not decode, or whose `validate_for` does not succeed, has `plant` `mismatch` and `claimed_block` `not defined`. `block_height` in a body that decodes is in `0 … 2^32 − 1`, which is inside `0 … 2^53 − 1`. A hash-verified canonical path body has no height above `2^53 − 1`. That is not `shape_item` `height_range`. The hike object still decodes. | §14.3 |
| <a id="rule-p14-7"></a>P14.7 | INVARIANT | An index GET spells the height in the shortest base-10 digits. | §14.2 |
| <a id="rule-p14-8"></a>P14.8 | INVARIANT | A path on `W_1` … `W_16` is `anchored` only when its inclusion height is in the section 13.1 anchor window, that header is an interrupt header stored on the object, and the plant comparison against that one header is `match`. Otherwise the window record is `not anchored`. Verify does not scan other index heights for that path. | §13.1, §14.3 |
| <a id="class-a4-6"></a>A4.6 | INVARIANT | Bookends are the lowest and highest anchored-and-fetched indices among `W_1` … `W_16`. | §14.4 Rule A4.6 |
| <a id="class-a4-7"></a>A4.7 | INVARIANT | `N` is the checkpoint count over stages `a` … `b−1`. It does not read `leg_count`. | §14.4 Rule A4.7 |
| <a id="rule-a4-8-1"></a>A4.8.1 | INVARIANT | `S` is `T(P_b) − T(I_{a−1})` only when both `T` values were taken on `match`. Otherwise `S` is `not read`. | §14.4 Rule A4.8 |
| <a id="rule-a4-8-2"></a>A4.8.2 | INVARIANT | `delta_h` subtracts index-confirmed heights only. Otherwise `delta_h` is `not defined`. | §14.4 Rule A4.8 |
| <a id="class-a4-9"></a>A4.9 | INVARIANT | The verify record holds integers `N` and `S` and does not hold a floating-point rate. The bookend display is not a judgment and is not the admission rate. | §14.4 Rule A4.9 |
| <a id="class-a4-10"></a>A4.10 | INVARIANT | Analysis records the whole-hike count and span, each per-stage span, checkpoints outside the bookends, `hike_legs`, and `hike_span`. The bookend pair is not `hike_legs` or `hike_span`. | §14.4 Rule A4.10 |
| <a id="class-a4-11"></a>A4.11 | INVARIANT | Fewer than two valid paths (Rule A4.11 step 1) among `W_1` … `W_16` records the bookend fields `not defined`. An issued plant GET that returns no document records them `not read`, before the anchored count. A plant `not read` whose GET was not issued does not. Fewer than two `anchored` then records `not defined`. With two `anchored`, an unread `I_{a−1}` header or an unread `P_b` header records every bookend field `not read`. A returned `mismatch` does not. `fixed_start` stays an object. | §14.4 Rule A4.11 |
| <a id="rule-a4-4-1"></a>A4.4.1 | CONSTANT | The six-leg draw count is 6. It is not `STAGES` and it is not `AUDIT_K`. | §14.6 Rule A4.4 |
| <a id="rule-a4-4-2"></a>A4.4.2 | INVARIANT | The draw is six distinct global checkpoint indices by the unbiased draw in section 14.6. Live verify reads the operating-system CSPRNG. A `conformance-small` vector may supply draw bytes (section 16.1). Fewer than six legs is `not defined`. A short byte source is `not read`. When that draw is `not read`, `six_leg_runs` is `not read`. A short-byte six-leg draw stays re-readable. It is not `not defined`. An 8-byte draw at or above the limit is consumed. | §14.6, §16.1 |
| <a id="rule-a4-4-3"></a>A4.4.3 | INVARIANT | Each chosen index is recorded `match` or `mismatch` in draw order. | §14.6 |
| <a id="rule-a4-4-4"></a>A4.4.4 | INVARIANT | The index draw finishes before any leg result is recomputed. A verifier may recompute the six leg results concurrently, and may run Rule A4.3 checkpoint comparisons concurrently. How many Rule A4.3 comparisons run at once is a verifier setting. The verify record does not depend on that number. One at a time is conforming. Elements of `six_leg_runs` may execute in parallel. This file does not require a thread pool and does not set a pool size. One at a time is for this measurement record. It does not apply to `thorough_rounds`. | §14.6 |
| <a id="class-a5"></a>A5 | INVARIANT | An object whose `kind` text is not exactly `KIND` is recorded `kind` `unknown kind`. No other label is recognised. That record is not a decode failure and is not a shape `mismatch`. Sections 14.1–14.6 are not applied. A malformed CBOR value is section 7.3 `malformed cbor`, not this record. | §14.7 Rule A5 |
| <a id="class-b9"></a>B9 | INVARIANT | Rule B9 names the section 15.2 refuses that do not wait on a configured threshold: decode, waypoint rehash, shape `mismatch` including `height_range`, seed-value `mismatch`, a confirmed plant or header `mismatch`, a checkpoint-digest `mismatch`, a six-leg `mismatch`, and reason `unknown_kind` when section 14 records `kind` `unknown kind`. A cap hit is Rule B7. This row is a section 15 judgment. It is not a Part A measurement. | §15.2 Rule B9 |

**Sentences reviewed for POLICY THRESHOLD.** None of the rows above is a POLICY THRESHOLD. The sentences a reader might take for one are:

1. `leg_count ≥ 1` (A3.2, A4.1.3, P13.1). It is the section 13.2 loop, recorded under shape. It is not an admission floor. Rule B8 sets no leg-count threshold.
2. Hashcash difficulty 21 (P13.5). It is a produce and Bari constant. It is not an admission threshold.
3. Section 9, “mean about 60 seconds.” That phrase describes the synth sequencer. It is not a section 6 constant, not a `conformance-small` parameter, and not an admission threshold.
4. Section 13.1, a stage that ends at `leg_count = 1` “is conforming.” That sentence means the producer followed section 13. Judgment of the record is section 14.
5. Rule A4.9, the bookend pair `(N, S)`. Section 14 records the integers. It does not apply `leg_rate_floor`. The admission rate is Rule B2 on `(hike_legs, hike_span)`, not on `(N, S)`.
6. The informative note under Rule A4.11 about the two, three, or four blocks from `I_{b−1}` to `P_b`. Those sentences are not a Part A number. Rule B3 is not an admission input and does not adjust `hike_span`.

No Part A sentence is moved into section 15 by this classification. Where section 14 says an admission service may judge a recorded token, the judgment stays in section 15.

<a id="sec-2"></a>

## 2. Scope

**Normative.**

This document specifies Satoshi Hike 2.3.

A hike has exactly eighteen stages (Rule A2.1).

Section 13 produces the object. Section 14 measures it. Section 15 is the admission service. Produce and verify perform no signature check. The admission service checks the signature (section 15.1).

<a id="sec-3"></a>

## 3. Interpretation

**Normative.**

“Must” and “must not” apply to a conforming producer, a conforming verifier, and a conforming admission service.

In this file, “must”, “must not”, and “may”, in lowercase or in capitals, carry the meanings that BCP 14 (RFC 2119, RFC 8174) gives to MUST, MUST NOT, and MAY. They state requirements only in text marked **Normative**. In normative text, a plain statement (“is”, “does”, “uses”, “is not”) is a requirement as well. Text marked **Informative** states no requirement. Where informative text uses one of these words, it restates a rule of the normative section it names, and that section governs.

Byte ranges are half-open. `b[i:j]` is bytes `i` through `j-1` of `b`, starting at index 0.

`0^n` is the byte string of `n` zero bytes.

`‖` is raw concatenation of byte strings, in the written order. A concatenation has no length prefix, no tag, and no separator.

`le64(x)` is the 8-byte little-endian encoding of the unsigned integer `x` with `0 ≤ x < 2^64`.

`u64_le(b)` is the unsigned integer represented by the 8-byte string `b` in little-endian order.

`mod` is the non-negative remainder. For `a ≥ 0` and `m > 0`, `a mod m` is the integer `r` with `0 ≤ r < m` and `a = qm + r` for some integer `q`.

`mod 2^64` on a `u64` quantity is ordinary wrapping arithmetic on a 64-bit unsigned integer. The result is the low 64 bits, and overflow is discarded. That is this `mod` with modulus `2^64`. It is the rule for the NL4 word, the parent indices, the slot mix, and the step’s `u64` state. A `mod` with any other modulus stays the remainder above.

`SHA-256` is the function specified by FIPS 180-4. Its output is 32 bytes. In this document, `H` means `SHA-256`.

<a id="sec-4"></a>

## 4. Glossary

**Normative.**

| Term | Meaning |
|---|---|
| Satoshi Hike 2.3 | This construction. `KIND` is `v2.2-pcoc-object`. Satoshi Hike 2.2 is the same construction. |
| Individual | The hiker. The individual is identified by a 32-byte BIP-340 x-only public key on secp256k1. |
| Seed | `D = SHA-256(public_key ‖ seed_beacon)`. It is 32 bytes. It has no domain label. Section 10 reads it as four little-endian lanes. Stage 0’s leg 0 starts from it. The object stores it at the wire key `stages[0].waypoint`. The seed equals `stages[0].waypoint` by design. That field is not a Rule A1 waypoint. It is never built from an interrupt and never posted. Stage 0 has no waypoint of its own. |
| Seed preimage | `public_key ‖ seed_beacon`. It is 64 bytes: the 32-byte public key, then the 32-byte `seed_beacon` in natural byte order. It is the SHA-256 input that produces the seed. It is not stored as its own field. |
| Seed beacon | The tip read at the start of produce. The object stores it as `seed_beacon`. It is the first beacon (section 14.4). |
| Interrupt | `I_g`, `g = 0 … 17`. The tip as read at the first post-checkpoint tip check in stage `g` whose hash differs from `I_{g−1}` (from `seed_beacon`, on stage 0) and whose height is strictly greater. If the tip moved by more than one block before that check, `I_g` is the tip as read. There are 18 interrupts. |
| Waypoint | `W_{g+1}`, `g = 0 … 16`: the 32-byte value Rule A1.3 builds after `I_g` and produce posts to Bari. The waypoints are `W_1` … `W_17`. No waypoint is built after `I_17`. Stage 0 has no waypoint of its own. |
| Pivot | What a waypoint does. `W_{g+1}` ends stage `g`’s record and starts stage `g+1`. |
| Path | The canonical path body of a waypoint, showing that waypoint’s plant in a block (section 13.1). Only `W_1` … `W_17` have paths. Stage 0 has no path. |
| Anchored and fetched | The `anchors` token `anchored`: the inclusion height is in the section 13.1 window, that header is an interrupt header stored on the object, and `plant` is `match`. Anything else among `W_1` … `W_16` is not anchored and fetched. |
| Skip | Empty path bytes on `W_1` … `W_16`. A skip is a normal produce outcome and is recorded. How skips count is admission-service policy (Rule B4). |
| `P_g` | The block named in `W_g`’s path, where its plant was mined. Verify records `P_g` as authenticated only when that path is **present** and the plant comparison is **match**. |
| `d_g` | A signed integer only when `plant` is `match` and the index comparison of `I_{g−1}` is `match`: index-confirmed `height(P_g)` minus index-confirmed `height(I_{g−1})`. Otherwise `not defined`. Not taken from a producer-stored height alone. Computed by verify; a verify-record field, not object content. |
| Bookends | `W_a` and `W_b`: the anchored-and-fetched waypoints with the lowest index and the highest index. |
| Bookend legs | `N`: the number of checkpoints over stages `a` … `b−1`. An integer when bookends exist. Computed by verify; a verify-record field, not object content. |
| Bookend span | `S = T(P_b) − T(I_{a−1})` when both `T` values are integers under Rule A4.2, so each `index_comparison` is `match`. Otherwise `S` is `not read`. Seconds, a signed integer when recorded. Computed by verify; a verify-record field, not object content. |
| Bookend leg rate | Display only. `N÷S` when `S > 0`. Otherwise the display is **not defined**. The verify record holds the integers `N` and `S`. It does not hold a floating-point rate. Section 15 does not read `(N, S)` as the admission rate. |
| `Δh` | Record key `delta_h`: index-confirmed `height(P_b)` minus index-confirmed `height(I_{a−1})`, only when both header comparisons are `match`. Otherwise `not defined`. Not taken from a producer-stored height alone. A signed integer in the bookend block when it is defined. `delta_h` stays in the bookend block. Rule B3 does not read it. Rule B3 is not an admission input. It is not an analysis value and it is not policy. Computed by verify; a verify-record field, not object content. |
| Analysis | The whole-hike checkpoint count and span, each per-stage span and its rate display, checkpoints outside the bookends, `hike_legs`, and `hike_span`. Rule B0 reads `hike_legs` and `hike_span` for the floor. It does not read the bookend pair. Computed by verify; a verify-record field, not object content. |
| First beacon | `seed_beacon`, stored as `stages[0].beacon`. Verify record `headers[0]`. |
| Last beacon | `last_interrupt` (`I_17`). Verify record `headers[18]`. |
| Hike legs | `hike_legs`: the number of checkpoints over stages 0 through 17. Not bookend `N`. Not `whole_hike_legs`. |
| Hike span | `hike_span`: `MTP(last beacon) − MTP(first beacon)`. MTP is Rule A4.10. Not the raw `header.time` of those two blocks. Not bookend `S`. |
| Median-time-past | MTP of a beacon block at height `h`. BIP113: sort the `header.time` values of the headers from height `h − n + 1` through `h`, with `n = min(11, h + 1)`, and take the value at index `n // 2` (the first is index 0). Rule A4.10 says which index reads supply those times. |
| Portable index | What verify measures against. At each height it holds the header hash and the height, and optionally waypoint anchoring. A row that carries Bitcoin `nTime` stores it as `header.time`. A row with no `header.time` is measured as `T` `not read`. This file does not name a cut height and does not backfill a row. The index is faithful to Bitcoin. Anyone may bypass the index and read Bitcoin directly (see §14). That direct read is not the section 14 procedure. |
| `T(x)` | The Bitcoin `nTime` of block `x`, from the portable index member `header.time`, only when the index comparison for that header is `match`. Otherwise the measurement is `not read`. Unix seconds when it is an integer. |
| Block time | That `nTime`: an unsigned 32-bit count of Unix seconds, read as it is. |
| Clock | The plane's tip sequence. Produce Observe reads that sequence and returns the natural beacon and its height. The tip sets the order of events. It is not money, not a confirmation count, and not an archive of the chain. Block time is a measured input to section 14. It does not replace tip advances as that order. Wall-clock time is not an input. The verifier's wall clock is not an input. |
| Terrain | The deterministic room derived from the individual’s public key and the seed beacon. |
| Hike | The whole proof and produce object: one individual’s recorded journey over terrain under the clock. |
| Stage | Legs from the seed (stage 0) or from waypoint `W_g` (stage `g ≥ 1`) until interrupt `I_g`. Stage 17 ends at `I_17`. No waypoint is built from `I_17`. |
| Leg | One leg of exactly `STEPS_PER_LEG` steps, from a previous checkpoint to a leg result and a checkpoint. |
| Step | One fold+seal over one slot of `SLOT_BYTES` bytes. |
| Leg result | The `u64` end state of a leg. |
| Checkpoint | The 32-byte `SHA-256` digest defined in section 11. |
| `not fetched by design` | The path record for `W_17` only, lowercase, with spaces. `stages[17].path` is the empty byte string. Verify identifies it by position. |
| Leg-rate floor | Rule B2 policy parameter, not a measurement. A policy sets its value, in checkpoints per second (`hike_legs` over `hike_span`, with `hike_span` in Unix seconds). Admission-gate policy setting, configurable. This file gives it no default. One checkpoint is one leg. |
| Index-confirmed height | `header.height` from an index document whose comparison is `match` (stored hash agrees and stored height agrees). For `P_b` that comparison is `index_comparison_P_b`: the header hash agrees and `header.height` equals `P_b.block_height`. A stored height alone is not this height. |
| Policy profile | See section 15. Not a hike-object field. |
| Re-verify | See section 15. |
| Marks as fast | See section 15. |
| Friendly list | See section 15. |
| Paths minimum | Rule B4. `anchored_total ≥ min_anchored_paths` among `W_1` … `W_16`, wherever those waypoints sit. Not a measurement. |
| Outcome token | Lowercase, with spaces: `match`, `mismatch`, `present`, `skip`, `not defined`, `not read`, `not fetched by design`, `n/a`, `anchored`, `not anchored`, and `unknown kind`. Not `not_defined`. Not title case. There is no measurement token `not obtained`; that wording is not used (see below). `unknown_kind` is not in this set. It is a section 15 reason. `n/a` means a plant comparison does not apply because `path_state` is `skip`. `n/a` is not `not defined`. `not read` is an index document that is missing or unreadable, a header whose comparison is not `match` so `T` is not taken, a `T` that is missing or unusable on a `match`, an unreadable header, or anything short or missing from a network read. Short path bytes for a six-leg run, and a short-byte six-leg draw, are `not read`. A document that was read and has no plant for that block is `plant` `mismatch`. A path body that does not yield `claimed_block` is `not defined`. Fewer than six legs is `not defined`. Those `not defined` values are settled by the object's own bytes. |
| Comparison result | The result of a Part B integer comparison (Rules B2, B4, and B10). It is `meets` when the comparison holds. When it does not hold, the rule states the outcome: `refuse`, or `not read` and `held` where that rule says so. `meets` is not an outcome token of this table, not an admission outcome, and not a section 15.2 state. It means only that this comparison does not refuse; every other section 15 check still runs. Part A does not use these words. |
| Fee header | The 80-byte Bitcoin block header the producer’s header source holds as its best tip when a Hashcash solve starts. `header_hash` is `SHA-256(SHA-256(header))` in natural byte order. It is not a stored header and it is not `H` (section 13.1). |
| `bari_origin` | The ASCII origin of `{bari-url}` as section 13.1 serializes it. Bari uses its own public origin, serialized the same way (section 13.6). |
| Asynchronous event | Work the producer starts and does not wait for on the walk path: a post, a solve, an ask, a header-source or peer read, a Bari exchange, or a log write (Rule A6). |
| Close | The moment the tip read shows `I_17`. Production ends and the object is encoded at once with the checked paths that have arrived (section 13.1). |
| Transitory send failure | A waypoint POST that received no HTTP status line, or received status 502, 503, or 504 (section 13.1). A transport or proxy condition, not a Bari answer. |

The public key is the 32-byte string used below. Produce and verify do not test a signature on it.

**Wording not used.** Use the term in the table.

| Not used | Use instead |
|---|---|
| opening header; the header that opened stage `g`; open header; close header | `seed_beacon` on stage 0; `I_{g−1}` on stage `g ≥ 1` |
| a header arrives during the waypoint post | there is no rebinding; the next interrupt is the later tip check (Rule A1) |
| deciding legs; deciding span; deciding leg rate | `hike_legs`; `hike_span`; the Rule B2 rate. Bookend `N` and `S` are not that rate |
| opening value | seed, or `W_g` |
| seed value | seed |
| production_seed; the 64-byte “seed” | seed preimage |
| a squeeze of the seed to one 64-bit fill pin; a domain label on the seed | not this construction. Every terrain word depends on all 256 bits of the seed (section 10) |
| stage span | bookend span `S`; a per-stage span is analysis (Rule A4.10) and is not an admission input |
| leg floor | leg-rate floor |
| seed block | `seed_beacon` |
| anchored and included | anchored and fetched |
| planted | anchored and fetched |
| not planted by design; any path field on stage 0 | stage 0 has no path key (Rule A3.3) |
| walk or re-walk, for a leg | leg, or recomputed leg result |
| not obtained, for an unsuccessful index read or a missing `T` | **not read** |
| roll-forward cut | A row with no `header.time` records `T` as `not read`. This file does not name a cut height. |

`SEAL_DOMAIN` `pcoc-walk-v1` is an exempt wire constant. It is not wording from that table. The verify-record key `seed_value` and the token `seed-value mismatch` are exempt wire names. Do not rename them.

Part A (sections 8–14) does not say that verify or the object passes, fails, rejects, denies, or admits, and it does not say `accept`, `refuse`, `revoke`, or `held`. Those outcomes are section 15. Do not say “minimum legs per stage.” A per-stage rate is analysis. It is not an admission test.

<a id="sec-5"></a>

## 6. Constants

**Normative.**

| Name | Value |
|---|---|
| `SLOT_BYTES` | 16384 (16 KiB) |
| `WORDS_PER_SLOT` | 2048 |
| `TERRAIN_BYTES` | 268435456 (256 MiB) |
| `N_SLOTS` | 16384 |
| `SAMPLE_BYTES` | 1024 (1 KiB) |
| `OFFSET_MODULUS` | 268434433 |
| `STEPS_PER_LEG` | 2000000 (`2_000_000`) |
| `STAGES` | 18 |
| `HASH_BYTES` | 32 |
| `LEG_RESULT_BYTES` | 8 |
| `PUBLIC_KEY_BYTES` | 32 |
| `BEACON_BYTES` | 32 |
| `SEAL_DOMAIN` | the 12 bytes `pcoc-walk-v1` |
| `KIND` | `v2.2-pcoc-object` |
| `FILL` | `dag2` |
| `MIX_ADD` | `0x9E3779B97F4A7C15` |
| `MIX_MUL1` | `0xBF58476D1CE4E5B9` |
| `MIX_MUL2` | `0x94D049BB133111EB` |
| `WORD_XOR` | `0xC0FFEE` |
| `PARENT_MUL` | `0xD1B54A32D192ED03` |
| `SLOT_MUL` | `0xA24BAED4963EE407` |

The six-leg check draws 6 legs. That count is not a constant in this table.

`WORDS_PER_SLOT = SLOT_BYTES / 8`. The divisor is the width of one little-endian `u64`.

`N_SLOTS = TERRAIN_BYTES / SLOT_BYTES`, and the division is exact: `TERRAIN_BYTES = N_SLOTS × SLOT_BYTES`.

`OFFSET_MODULUS = TERRAIN_BYTES − SAMPLE_BYTES + 1`.

The section 6 numbers for `WORDS_PER_SLOT`, `N_SLOTS`, and `OFFSET_MODULUS` are that production evaluation. They are not a second free constant. Under `conformance-small` the same formulas apply to that profile’s `SLOT_BYTES`, `TERRAIN_BYTES`, and `SAMPLE_BYTES`.

`SEAL_DOMAIN` is the twelve ASCII bytes `70 63 6f 63 2d 77 61 6c 6b 2d 76 31`. It is exactly those twelve bytes.

**Normative. Domain identifiers.** The byte strings `pcoc-walk-v1` (`SEAL_DOMAIN`), `bari-hc-v2` (the Hashcash challenge prefix), `pcoc-gate-signin-v1` (the sign-in challenge domain), and the `/v1` HTTP route prefix are wire-format domain identifiers. Their digits are not specification versions and do not change with the version of this file. The only specification-version marker in an object is the kind label `v2.2-pcoc-object` (`KIND`).

There is no seed domain label. A domain label on the seed, or a squeeze of the seed to fewer than 256 bits, is not this construction.

`STAGES` is 18. That count, and the beacon count that goes with it, is a profile constant. It is not a security number. The description of this arbitrary profile size is 18 stages and 19 beacons (`seed_beacon` plus `I_0` … `I_17`). The object stores 18 stage maps, numbered 0–17. Admission does not treat 18, 17, or 19 as a threshold.

<a id="sec-7"></a>

## 7. Encodings

**Informative. Summary.** How integers, hexadecimal strings, and the hike object are written as bytes. Section 7.1 covers little-endian integers, 7.2 hexadecimal, and 7.3 the deterministic CBOR of the object and the order of the decode checks (malformed, then decode failure, then Rule A5). Section 7.4 gives worked checks.

**Normative.**

<a id="sec-7-1"></a>

### 7.1 Little-endian integers

`le64` and `u64_le` are defined in section 3.

The first eight bytes of a 32-byte string `b` are `b[0:8]`. Those bytes are SHA-256 output order when `b` is a digest: byte 0 is the first byte of the digest. `u64_le` does not reverse a digest a second time.

<a id="sec-7-2"></a>

### 7.2 Hexadecimal

A hexadecimal spelling uses the digits `0-9`, `a-f`, and `A-F`. Letter case does not change the value. The spelling contains no spaces.

A 64-digit spelling denotes 32 bytes. The first two digits are byte 0. Natural order is that spelling: byte 0 first. Bitcoin display order is the one reversal in section 9. A digest is not reversed a second time.

<a id="sec-7-3"></a>

### 7.3 CBOR

The hike object is one CBOR data item, as defined by RFC 8949. The item is a map.

Produce emits the core deterministic encoding:

- definite lengths only
- unsigned integers in the shortest encoding
- no floating-point values, no tags, and no big-integer tags
- map keys sorted by the bytewise lexicographic order of their definite-length encodings

Every field name in this object is at most 23 bytes, so the first byte of an encoded key is `0x60` plus the name length. For the fields in section 8, that order is:

Top-level map. Keys sort by encoded length, then bytewise. `steps_per_leg` is 13 bytes. `last_interrupt` is 14 bytes. `last_interrupt_height` is 21 bytes.

1. `fill`
2. `kind`
3. `stages`
4. `n_slots`
5. `public_key`
6. `slot_bytes`
7. `seed_beacon`
8. `sample_bytes`
9. `steps_per_leg`
10. `last_interrupt`
11. `last_interrupt_height`

Stage 0 has no `path` key (Rule A3.3). Its map:

1. `beacon`
2. `waypoint`
3. `leg_count`
4. `checkpoints`
5. `leg_results`
6. `beacon_height`

Each stage map for stages 1–17:

1. `path`
2. `beacon`
3. `waypoint`
4. `leg_count`
5. `checkpoints`
6. `leg_results`
7. `beacon_height`

Text values and map keys are definite-length UTF-8 text strings (CBOR major type 3). A length from 0 through 23 is the additional information in the first byte. A longer string uses additional information 24, 25, 26, or 27 and the length bytes that follow, then the UTF-8 bytes. Indefinite-length text is not a text string in this object. `KIND` is 16 bytes, so the `kind` value is the head byte `0x70` and those 16 bytes. Byte fields are byte strings (major type 2). Counts and heights are unsigned integers (major type 0). Arrays are definite arrays in the order defined in section 8.

Verification decodes one definite-length CBOR data item and records section 14. These are decoding rules. A definite-length encoding of the section 8 values decodes. A non-shortest definite-length unsigned integer decodes when the numeric value is the same. Map key order is not a shape `mismatch`. Indefinite lengths, tags, floating-point values, simple values, negative integers, a map key that is not a text string, a duplicate key, and any byte after that one data item are a malformed CBOR value. A text string whose bytes are not valid UTF-8 (RFC 3629) is also a malformed CBOR value, including a map key and a `kind` value. It is not Rule A5 and it is not a decode failure. A malformed CBOR value is shape `mismatch`, and `shape_item` includes `malformed cbor`. Later measurements are still recorded. Produce is what fixes the canonical bytes. Verification sorts the input bytes in this order. First, input that is not one definite-length CBOR data item under these rules is a malformed CBOR value, whatever its first data item holds. That includes an empty input, bytes that are not CBOR, bytes that end inside an item, and a malformed item that is not a map. Rule A5 is not applied to a malformed CBOR value, even when a `kind` text can be read from it. Second, one well-formed data item that is not a map, or a map with no `kind` key whose value is a text string, is a decode failure: there is no section 14 record, and the verify CLI exits with code `1` (section 18). Third, a map whose `kind` text is not exactly `KIND` is Rule A5. Every other input is measured by section 14.

<a id="sec-7-4"></a>

### 7.4 Checks

**Informative.**

`le64(1) = 01 00 00 00 00 00 00 00`.

`le64(0x0102030405060708) = 08 07 06 05 04 03 02 01`.

FIPS 180-4’s SHA-256 example for the three bytes `abc` is `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`.

`SHA-256(SEAL_DOMAIN ‖ le64(0))` is the 32-byte digest whose hexadecimal spelling, byte 0 first, is `4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533`. The first eight digest bytes, in that index order, are `48 16 bd af 23 24 04 76`. Their spelling in index order is `4816bdaf23240476`. When `S_folded = 0`, section 11 sets `S' = u64_le(E[0:8])`, the little-endian integer of those eight bytes. That integer is `0x76042423afbd1648`. `E` is that digest. `D` is the seed, not this digest. The spelling `4816bdaf23240476` is the digest bytes in index order. It is not the hexadecimal spelling of the integer `S'`.

A seed preimage of 32 bytes of `0x11` followed by 32 bytes of `0x22` has seed `5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c` (byte 0 first). The four lanes are `P[0] = 0x545dfe297dc78951`, `P[1] = 0x27858669c45e046a`, `P[2] = 0x9cdac73ac1a5fe85`, `P[3] = 0x7c81df6efbf55f11`. The NL4 base word at `i = 0`, `j = 0` is `0xc7582082a632c2c8`. There is no fill pin.

<a id="sec-8"></a>

## 8. Hike object

**Informative. Summary.** The hike object is one CBOR map. It holds the top-level fields, 18 stage maps numbered 0–17 with their beacons, heights, checkpoints, leg count, leg results, and waypoint (and, on stages 1–17, path), and `last_interrupt` (Rule A3). Verify (section 14) measures it, and section 15 judges it.

**Normative.**

<a id="rule-a3"></a>

### Rule A3. What the object stores

The object has exactly these top-level fields:

| Field | CBOR type | Value |
|---|---|---|
| `kind` | text | `KIND` |
| `fill` | text | `FILL` |
| `slot_bytes` | unsigned | `SLOT_BYTES` |
| `n_slots` | unsigned | `N_SLOTS` |
| `sample_bytes` | unsigned | `SAMPLE_BYTES` |
| `steps_per_leg` | unsigned | `STEPS_PER_LEG` |
| `public_key` | byte string, length 32 | the individual’s public key |
| `seed_beacon` | byte string, length 32 | the seed beacon in natural order |
| `stages` | array, length `STAGES` | stage maps, stage 0 first |
| `last_interrupt` | byte string, length 32 | `I_17`, the interrupt that ended stage 17, in natural order |
| `last_interrupt_height` | unsigned | the height of `I_17` |

**Evidence and convenience.** Evidence, the hash-bound values, is the seed preimage (`public_key`, `seed_beacon`), the checkpoints, the leg results, the waypoints `W_1` … `W_17`, the Bitcoin headers (`stages[g].beacon` for `g ≥ 1`, and `last_interrupt`), the path’s leaf, Merkle path and root, which bind the path to the waypoint, `stages[0].waypoint` (the seed, `SHA-256(public_key ‖ seed_beacon)`), and any calculated field whose hash dependencies enforce order and cost. Convenience signposts are the path locators (`txid`, `vout`, `block_hash`, `block_height`), checked against the index, `leg_count`, `beacon_height`, `last_interrupt_height`, the section 6 constants, and `stages[0].beacon` (= `seed_beacon`). Verify records `leg_count` under shape: `shape_item` includes `leg_count` when it differs from `len(checkpoints)` (Rule A4.1). Section 6 constants are recorded as `constant`. Stored heights are recorded as `index_comparison` (Rule A4.2). They are convenience. `d_g` and `delta_h` do not subtract a stored height unless the index comparison that confirms it is `match` (Rules A4.5 and A4.8). Verify checks a convenience signpost against the evidence or the index and never relies on it. A broken signpost is a recorded `mismatch`. Section 15 is where an admission service may judge that recorded mismatch.

<a id="rule-a3-1"></a>

#### Rule A3.1 Headers

The field name `beacon` is kept.

- `stages[0].beacon` = `seed_beacon`, and `stages[0].beacon_height` is that header’s height.
- For `g = 0 … 16`, `I_g` = `stages[g+1].beacon`, and `stages[g+1].beacon_height` is the height of `I_g`.
- `I_17` = `last_interrupt`, and `last_interrupt_height` is its height.

`stages[g].beacon` for `g ≥ 1` is `I_{g−1}`, the interrupt stage `g` started from. It is not a second header read after the waypoint post.

For `0 ≤ g < 17`, `stages[g+1].beacon_height > stages[g].beacon_height`, and `stages[g+1].beacon` differs from `stages[g].beacon`.

`last_interrupt_height > stages[17].beacon_height`, and `last_interrupt` differs from `stages[17].beacon`.

The object therefore carries `seed_beacon` and all 18 interrupts. The object carries no block time. Section 14 reads each block time from the portable index (sections 9 and 14.2).

<a id="rule-a3-2"></a>

#### Rule A3.2 Per stage

Stage 0’s map has exactly these fields. It has no `path` key.

| Field | CBOR type | Value |
|---|---|---|
| `beacon` | byte string, length 32 | `seed_beacon` in natural order |
| `beacon_height` | unsigned | the height of `seed_beacon` |
| `leg_count` | unsigned, at least 1 | number of legs in the stage |
| `waypoint` | byte string, length 32 | the seed, `SHA-256(public_key ‖ seed_beacon)`. Wire key `waypoint`. Prose name: seed. Not a Rule A1 waypoint. Never posted. |
| `checkpoints` | array of byte strings | one 32-byte checkpoint per leg, leg 0 first |
| `leg_results` | array of byte strings | one 8-byte little-endian leg result per leg, leg 0 first |

Stages 1–17 each have exactly these fields:

| Field | CBOR type | Value |
|---|---|---|
| `beacon` | byte string, length 32 | `I_{g−1}` in natural order |
| `beacon_height` | unsigned | the height of `I_{g−1}` |
| `leg_count` | unsigned, at least 1 | number of legs in the stage |
| `waypoint` | byte string, length 32 | `W_g`, the waypoint stage `g` started from |
| `path` | byte string | the canonical path body for `W_g`, or the empty byte string section 13.3 stores for a skip or for `W_17` |
| `checkpoints` | array of byte strings | one 32-byte checkpoint per leg, leg 0 first |
| `leg_results` | array of byte strings | one 8-byte little-endian leg result per leg, leg 0 first |

`len(checkpoints) = len(leg_results) = leg_count`.

An object-byte height above `2^53 − 1`, including `beacon_height` and `last_interrupt_height`, is not a usable unsigned value on the verify record. Section 14 records `shape` `mismatch`, `shape_item` includes `height_range`, and the value is `not defined`. It is not `not read`. Do not clamp the value and do not record it.

`checkpoints[j]` and `leg_results[j]` belong to the same leg.

`stages[0].beacon = seed_beacon`.

`stages[0].waypoint = SHA-256(public_key ‖ seed_beacon)`.

<a id="rule-a3-3"></a>

#### Rule A3.3 Paths

Stage 0 has no path. The stage-0 map omits the `path` key. Omission is the encoding. An empty byte string is not a stage-0 path. The words “not planted by design” are not a token in this file.

Stages 1–17 each carry the path of their waypoint `W_g`, or empty bytes for a skip.

`stages[17].path` (`W_17`) is always the empty byte string. `W_17` is not asked, including when a jump would put an offset at or below the cut-off (section 13.1). The record is **not fetched by design**.

Sixteen waypoints, `W_1` … `W_16`, are fetchable. `W_17` is not one of them.

A `path` key on stage 0 is a shape **mismatch** (Rule A4.1). Those bytes are not a path. Verify does not give them path semantics.

The object has no terrain field. A verifier recomputes terrain from `public_key` and `seed_beacon`.

A leg result on the object is `le64` of the `u64` defined in section 11, stored as an 8-byte byte string.

<a id="sec-9"></a>

## 9. Clock

**Informative. Summary.** The clock is Bitcoin headers. Produce reads its plane’s tip sequence (Observe) for the seed beacon and each stage’s interrupt. Verify reads the stored headers back from the portable index `{index-url}`. This section states the read and body rules, the byte order of the header hashes, and the live-plane header source.

**Normative.**

`{tip-url}` and `{index-url}` are absolute URLs with scheme `http` or `https` (ASCII case-insensitive) and no trailing slash.

A clock read is one `GET` of a URL below. The reader does not follow a redirect. A status other than 200 is not a successful read. The reader does not require a particular `Content-Type`.

A successful body is UTF-8 JSON: one JSON object and no second value. A byte order mark (U+FEFF, the bytes `EF BB BF`) is not a successful read and is not stripped. Bytes that are not UTF-8 are not a successful read.

A body is one JSON object and no second value. If the same member name occurs more than once in any JSON object in the document, including a nested object, the read is not successful. The reader does not keep one of the repeated values.

Header measurement and plant measurement are separate reads of `GET {index-url}/index/{height}`. Each read parses the body on its own. The header measurement uses only `header.hash`, `header.height`, and `header.time`. It is a successful header read only when `header` is a JSON object and `header.hash` and `header.height` follow the rules below. A body with no `header` member, or a `header` that is not that object, is an unsuccessful header read: section 14.2 records `index_comparison` `not read` and `T` `not read`. `network`, `plant`, `plants`, and every member other than `header.hash`, `header.height`, and `header.time` do not change `index_comparison` or `T`.

The plant measurement does not require `header`. A plant read that returns a JSON object is a returned document. Section 14.3 then records `plant` `match` or `mismatch` from `network` and the plant records in that object. A plant read that does not return a document is `plant` `not read`. One returned document can be a header `not read` and a plant `match`: the document has `network` and a matching plant and has no usable `header`. The anchor-window vector’s height 4 is that document. The header reads for that vector return no document. The one issued plant read returns a document with no `header` member. A returned document with a usable `header` and no matching plant is a successful header read and plant `mismatch`.

`header.hash` is a JSON string of exactly 64 hexadecimal digits under section 7.2, so the digits are `0-9`, `a-f`, and `A-F`, and letter case does not change the value. The string has no `0x` prefix. The digits are Bitcoin display order. Let `display` be the 32 bytes of that spelling under section 7.2. The natural beacon is the 32-byte string `natural` with

```text
natural[i] = display[31 − i]    for i = 0 … 31
```

That formula is the single reversal rule. Bitcoin display order is the natural 32 bytes reversed: `display[i] = natural[31 − i]`. Apply it only when reading a Bitcoin display hex string into natural bytes, or when writing natural bytes as a Bitcoin display hex string. Do not reverse between two values that are already natural order. Do not reverse a digest a second time.

Where each hash is written, and the order it uses:

- A SHA-256 digest in this file (the seed, a checkpoint, a waypoint, a fold, a Merkle leaf node, sibling, or root) is natural order. Byte 0 is the first byte of the digest. The object stores those 32 raw bytes. A hex spelling of them is byte 0 first.
- `seed_beacon`, `stages[g].beacon`, and `last_interrupt` are natural-order raw bytes.
- An index document’s `header.hash` is Bitcoin display hex. Reverse it once to natural order before comparing it with a stored beacon.
- A canonical path body stores `block_hash` and `txid` as 32 raw bytes in natural order, the same order as the object beacon. The interrupt-header check is the same height and those same 32 bytes, compared with the stored beacon, with no reversal.
- A path body’s `root`, `leaf`, and Merkle siblings are natural-order digest bytes. No reversal.
- An index plant’s `root` is hex in natural order. Compare it with the path root with no reversal.
- An index plant’s `txid` and `block_hash` are Bitcoin display hex. Reverse each once to natural order before comparing it with the path body.
- The verify record’s `stored_hash` is natural-order hex, byte 0 first.
- The verify record’s `block_hash`, on `P_g`, `P_b`, and `claimed_block`, is Bitcoin display hex: the reversal of the path body’s natural `block_hash` bytes. It is not a byte-0-first spelling of those bytes.
- A produce event-log `tip` or `waypoint` is natural-order hex, byte 0 first. An event-log `block` is the path body’s `block_hash`, byte 0 first. It is not display order.
- The Hashcash `header_hash` is natural order: `SHA-256(SHA-256(header))` of the 80-byte fee header, not reversed. The challenge does not use display bytes. `X-Bari-Hashcash-Header` spells the 80 header bytes in wire order, byte 0 first.
- Every block-hash comparison in Rule BR1 (section 13.6) is natural order on both sides, with no reversal. For **current**, the hash of Bari’s tip is `SHA-256(SHA-256(header))` of Bari’s tip header, natural order, the same bytes as `header_hash`. For **before**, the previous-block hash of Bari’s tip is the 32 bytes at offset 4 of Bari’s tip header, in wire order, which is natural order: `SHA-256(SHA-256(header))` of the block before the tip, the same bytes as that block’s `header_hash`. For **after**, the fee header’s previous-block field is the 32 bytes at offset 4 of the fee header, in wire order, compared with the natural-order hash of Bari’s tip. The **after** proof-of-work check reads the natural-order `SHA-256(SHA-256(header))` as a little-endian 256-bit integer. A tip hash that Bari learns as Bitcoin display hex, from an RPC, an index document, or a log, is reversed once to natural order before any comparison. Neither side is ever compared in display order. A fee header that matches Bari’s tip only when one side is reversed is not current, before, or after, and the POST is answered 402.
- The path-ask leaf is the waypoint’s hex, byte 0 first.

`header.height` is a JSON number in the range `0 … 2^53 − 1`. It is not a JSON string. A JSON number written with a fractional part or an exponent is not a height. That bound is what this file states for a height. A height above `2^53 − 1` that an index document reports makes that read unsuccessful. Record `not read`. Do not clamp the value and do not record the oversized value. An integer above `2^53 − 1` in `beacon_height` or `last_interrupt_height` is recorded in section 14: `shape` `mismatch`, `shape_item` includes `height_range`, and the value is `not defined`. It is not `not read`. `block_height` in a body that decodes is in `0 … 2^32 − 1`, which is inside `0 … 2^53 − 1`. A hash-verified canonical path body has no height above `2^53 − 1`. A body that does not decode, or whose `validate_for` does not succeed, has `plant` `mismatch` and `claimed_block` `not defined`. That is not `shape_item` `height_range` (P14.6). The hike object still decodes. `not read` is only for a read that is not hash-bound or whose hash does not agree.

`header.time` is the Bitcoin header `nTime` of the header at that height. It is a JSON integer in the range `0 … 2^32 − 1`, in Unix seconds. It is not a JSON string. A JSON number written with a fractional part or an exponent is not a time. On the mainnet path, `header.time` equals the `nTime` of the block whose hash is `header.hash`. The portable index is faithful to Bitcoin: header hash, height, and `nTime`. Anyone may bypass the index and read Bitcoin directly (see §14). Section 14 measures the portable index. It does not read Bitcoin as its procedure. `T(x)` reads `header.time` from the portable index for block `x` only when Rule A4.2 records `index_comparison` `match`. Produce does not read `header.time`. Bari’s observed time (`header.timestamp_unix_ms`, when a document carries it) is a different member. Section 14 does not use it as `T`.

A read whose `header` has no `time`, or whose `time` breaks this rule, is still a successful read for `hash` and `height` when those members follow the rules above. `T` for that header is then **not read**. Section 14.2 records that token. A read that is not successful leaves both the index comparison and `T` as **not read**. `T` is taken only when section 14.2 records `index_comparison` `match` for that header. `match` requires the stored hash to agree with the index. A successful read whose hash does not agree is `mismatch`, and `T` is **not read**. Do not take that document’s time. A height disagreement is also `mismatch`, and `T` is **not read** on any `mismatch`. A successful read with a well-formed `time` does not record `T` when the hash does not agree.

The body rules above are the portable index. `header.hash` and `header.height` follow those rules for every header read. `header.time` follows them on a row that carries it. A plant read does not require them. A row with no `header.time` records `T` as `not read`. This file does not name a cut height and does not backfill a row. Produce Observe does not use that GET.

Produce Observe returns `(natural, height)` from the plane's one tip sequence. The default profile is the test plane. Its sequence is the synth sequencer (the synth tip sequence). Wire magic is regtest headers (header network magic for gossip). That magic is not a Bitcoin Core plant and not a tip clock. One cadence, mean about 60 seconds. “Mean about 60 seconds” describes the synth sequencer. It is not a section 6 constant, not a `conformance-small` parameter, and not an admission threshold. The natural beacon is that header's hash in natural byte order. Test Observe is that gossip peer. It does not GET an HTTP tip.

**Synth headers.** Every synth header carries the fixed `nBits` `0x207fffff`. A synth header has valid proof of work when `SHA-256(SHA-256(header))`, read as a little-endian 256-bit integer, is at most the target that `nBits` encodes. On the test plane Bari applies that fixed `nBits` and that check for the Rule BR1 “after” case. The test plane keeps the same ±1 block contract as the live plane; only the block interval is shorter. Synth proof of work is cheap by design. A forged “after” header is valid only while Bari’s tip is unchanged, and it does not lower the 21-bit fee.

The live profile is explicit. Its sequence is the Bitcoin best chain, the chain with the most work, read by gossip. Live Observe does not GET an HTTP tip.

When no gossip peer is named, the starting list is a copy of Bitcoin Core's current mainnet DNS seeds (`CMainParams::vSeeds` in `src/kernel/chainparams.cpp`). The list is Core's own bootstrap. This specification does not invent a host. The copy is what lets a producer join gossip without naming one peer and without depending on any groundinglab node. A lookup of one seed returns node addresses. The producer connects to those nodes and asks them for more peers. An address that does not answer is dropped. That is how a producer stays independent of any single gossip peer.

**Header source.** On the live plane the producer’s header source keeps sessions with at least `HEADER_PEERS_MIN` gossip peers at the same time, for the whole run. `HEADER_PEERS_MIN` is 2. A peer announces a header when it relays that header or a header that builds on it. When one connected peer has announced a header and another connected peer has not announced it within `HEADER_PEER_SILENCE_SECONDS` of the first announcement, the silent peer is disconnected and replaced by a new peer from the starting list or from addresses that peers have given. `HEADER_PEER_SILENCE_SECONDS` is 120. Both values are constants of this file. They are not levers and not SDK options. While no peer announces a new header, no peer is replaced: a natural gap between blocks is not silence. When fewer than `HEADER_PEERS_MIN` peers are connected, the header source keeps connecting new peers asynchronously, backing off between attempts, and writes an `alert` record (section 13.5) named `header_peers_low` each time the count falls below `HEADER_PEERS_MIN`. The walk continues, the object is unaffected, and the run still follows this section. A header source built to use one peer does not follow this section. Keeping, replacing, and reconnecting peers is asynchronous work and never pauses a leg (Rule A6). The copied list is checked again against Bitcoin Core's mainnet chainparams when a producer release is cut.

The copy, in Core's order, is `dnsseed.bluematt.me`, `seed.bitcoin.jonasschnelli.ch`, `seed.btc.petertodd.net`, `seed.bitcoin.sprovoost.nl`, `dnsseed.emzy.de`, `seed.bitcoin.wiz.biz`, `seed.mainnet.achownodes.xyz`. The port is 8333. A named peer is still `host:port` when the producer supplies one. This specification does not name that host or port. The test profile does not use this list. Its Observe remains the synth peer.

Produce does not read `{index-url}`. Produce does not read an HTTP tip, including `{tip-url}/index` and `{tip-url}/index/{height}`.

`Observe` is that plane sequence and returns `(natural, height)`.

The seed beacon is the natural beacon from the first successful Observe, taken at the start of produce, before any leg. It fixes the terrain. It is the header stage 0 starts from. That header is the latest one already on the tip sequence. Produce does not wait for a header mined after produce starts. A header that is minutes old is the seed when no later header has been read. After the terrain fill, stage 0’s legs begin from the seed. There is no Bari post before those legs. A later header is the interrupt `I_0`, read at the Observe after a checkpoint. It does not replace the seed. If the header source holds no header yet, the run has not started: produce keeps reading asynchronously until a seed read succeeds or the run is cancelled. No leg exists yet, so no leg is paused.

A stage ends at its interrupt. The interrupt is the first successful Observe, taken after a checkpoint has been recorded, that returns a natural beacon different from the header the stage started from and a height strictly greater than that header’s height. The header stage 0 started from is `seed_beacon`. The header stage `g ≥ 1` started from is `I_{g−1}`. Both the hash condition and the height condition are required. If the tip moved by more than one block before that check, the interrupt is the tip as read. It is not an earlier header the producer did not read. The interrupt is a Bitcoin header interrupting the drive. The post-checkpoint tip read returns at once the latest header the producer’s header source already holds. It never waits for a header, a peer, or the network. A read that finds no newer header, or whose header source is unavailable, is not an interrupt: the stage continues with its next leg, and an unavailable source is logged (section 13.5).

The interrupt that ends stage `g`, for `g` from 0 through 16, is stored as `stages[g+1].beacon`. The waypoint `W_{g+1}` is built from that interrupt after it is read (Rule A1.3, section 12.2). The interrupt that ends stage 17 is `last_interrupt`. No second Observe replaces `stages[g].beacon` after the waypoint post. There is no rebinding.

A header newer than `I_g` is the next interrupt candidate. Produce reads it at the Observe after stage `g+1`’s first checkpoint, the same as any other header.

An Observe taken for the Hashcash fee header, a waypoint POST, or a path ask does not end a stage and is not `I_g`. Those reads, the POST, and the ask are asynchronous events (Rule A6). None of them pauses a leg.

Heights are stored on stages. A height is not an input to `SHA-256` in this construction.

A clock read that does not succeed never stops the run and never withholds the object. A post-checkpoint tip read that does not succeed is not an interrupt: the walk continues with its next leg, and produce logs the transitory condition (section 13.5). An Observe for the fee header, a waypoint POST, or a path ask that does not succeed is a miss. Rule A3.4 records it and produce continues.

Verification does not Observe the tip sequence and does not GET an HTTP tip. Section 14.2 reads `GET {index-url}/index/{height}` under the body rules in this section, once for each stored header: each stage’s `beacon_height`, then `last_interrupt_height`. It records `index_comparison` and `T` for each. That read is part of the header measurement in section 14.2. It is not the only request verify may send: section 14.3 may read the same index for a present path’s plant facts. Verify does not GET a Bari path. The index is that plane's index. The default profile does not fill `https://index.groundinglab.io`.

<a id="sec-10"></a>

## 10. Terrain

**Normative.**

```text
seed_preimage = public_key ‖ seed_beacon          # 64 bytes, no separator, no label
D = SHA-256(seed_preimage)                        # the seed, 32 bytes
P[k] = u64_le(D[8k : 8k+8])                       # k = 0 … 3
```

The seed preimage is 64 bytes. It has no separator and no domain label. The seed is the 32-byte digest `D`. It is not a 64-bit pin. Lane `P[k]`, for `k = 0 … 3`, is eight little-endian bytes of `D`. All 256 bits of `D` are the lanes. There is no fifth lane and no domain label on the seed. **Informative.** Leg 0 of stage 0 starts at `S_0 = u64_le(D[0:8]) = P[0]`, and the first terrain-sample offset is `P[0] mod OFFSET_MODULUS`. The same 64 bits are lane 0, the first leg’s opening state, and that first sample offset. That reuse is the seed, by design. It is not a separate exploit.

**Normative.** Every terrain word depends on all 256 bits of the seed. A word map in which some word is unchanged when any one lane changes is not this construction. The NL4 base word below is that rule. Parent mixing and `slot_mix` do not remove it.

The seed equals `stages[0].waypoint` by design. Both are `D`. Nothing new is stored on the object.

Every integer step below is a `u64`. A sum or product written `mod 2^64` wraps as section 3 defines: the low 64 bits are kept. The terrain is `TERRAIN_BYTES` bytes, filled below, and then left unchanged. Later beacons do not change it. Slot `i`, for `0 ≤ i < N_SLOTS`, is

```text
terrain[i × SLOT_BYTES : (i+1) × SLOT_BYTES]
```

Every load of slot `i` returns those same `SLOT_BYTES` bytes. The step does not write terrain.

<a id="sec-10-1"></a>

### 10.1 `mix64`

```text
mix64(x):
    y = (x + MIX_ADD) mod 2^64
    y = ((y XOR (y >> 30)) × MIX_MUL1) mod 2^64
    y = ((y XOR (y >> 27)) × MIX_MUL2) mod 2^64
    return y XOR (y >> 31)
```

`>>` is a logical right shift of a `u64`: vacated high bits are zero. It does not sign-extend. `<<` is a left shift of a `u64`. The shift count is not reduced modulo 32. `i << 32` is the slot index multiplied by `2^32` in that `u64`, and for every legal `i` the product fits in 64 bits.

<a id="sec-10-2"></a>

### 10.2 Lanes

The four lanes are `P[0]`, `P[1]`, `P[2]`, and `P[3]` from the seed, as section 10 states. A squeeze of the seed to one 64-bit fill pin is not this construction. Do not compute one. Do not use one.

**Informative.** Conformance does not require computing such a squeeze. The pack may publish a terrain digest made from a squeezed seed as a negative. An implementation that emits that digest does not conform.

<a id="sec-10-3"></a>

### 10.3 Word rotation and parent indices

`rotl(w, r)` is `w` rotated left by `r` bits on a 64-bit word. Bits that leave the top re-enter at the bottom. `rotl(w, 0) = w`. In this section `r` is in `0 … 63`.

`floor_log2(n)`, for an integer `n ≥ 1`, is the greatest integer `b` with `2^b ≤ n`.

`reverse32(v)` treats `v` as 32 bits, high bits zero when `v < 2^32`. Bit `k` of the result, with bit 0 the least significant bit, equals bit `31 − k` of `v`.

For a slot index `i` with `1 ≤ i < N_SLOTS`:

```text
bits = max(floor_log2(i), 1)
bitrev_parent(i) = (reverse32(i) >> (32 − bits)) mod i

h = (i × MIX_ADD) mod 2^64
mul_parent(i) = floor(h / 2^32) mod i
```

Both parent values are integers in `0 … i − 1`.

<a id="sec-10-4"></a>

### 10.4 dag2 fill

Start from `TERRAIN_BYTES` zero bytes. For `i` from 0 through `N_SLOTS − 1`, build `WORDS_PER_SLOT` words and then store the slot.

The base word is NL4. For each `j` in `0 … WORDS_PER_SLOT − 1`:

```text
c = (i << 32) XOR j XOR WORD_XOR
t = mix64( P[j mod 4] XOR c )
t = mix64( t XOR rotl(P[(j + 1) mod 4], 17) )
t = mix64( t XOR rotl(P[(j + 2) mod 4], 31) )
t = mix64( t XOR rotl(P[(j + 3) mod 4], 47) )
w[j] = t
```

`mix64` is §10.1, including the initial `+ MIX_ADD`. A finalizer that omits `MIX_ADD` is not this function. `i << 32` is the slot index multiplied by `2^32` in a `u64`, as under `mix64` above: a left shift of a `u64`, and the shift count is not reduced modulo 32. `j` is the word index. `j mod 4` selects a lane, and the next three steps fold the other three lanes in, so the word depends on `P[0]`, `P[1]`, `P[2]`, and `P[3]`.

A base word that uses one lane, two lanes, or a linear mix of the four lanes (including a map that is unchanged when two lanes are each replaced by their bitwise complement) is not NL4. Those maps are not conforming.

**Informative.** The one-lane map is `mix64(P[j mod 4] XOR c)`. The linear map is one `mix64` of `P[j mod 4] XOR rotl(P[(j + 1) mod 4], 17) XOR rotl(P[(j + 2) mod 4], 31) XOR rotl(P[(j + 3) mod 4], 47) XOR c`, with `c` as in the NL4 chain. Neither map is NL4.

Slot 0 keeps those words. It has no parent lanes and no slot mix.

When `i > 0`, take the three parents in this order, even if two of them are the same index:

1. `i − 1`
2. `bitrev_parent(i)`
3. `mul_parent(i)`

For parent number `k` in `1, 2, 3`, and for each `j` in `0 … WORDS_PER_SLOT − 1`, let `p` be that parent index and let `pw` be the little-endian word already stored at word `j` of slot `p`:

```text
rot = (j + k) mod 64
w[j] = mix64( (w[j] XOR rotl(pw, rot)) XOR ((k × PARENT_MUL) mod 2^64) )
```

After those three parents:

```text
slot_mix = (i × SLOT_MUL) mod 2^64
w[j] = mix64(w[j] XOR slot_mix)    for j = 0 … WORDS_PER_SLOT − 1
```

For every slot, including slot 0, write `le64(w[j])` into word `j` of slot `i`.

Terrain is computed once per terrain, from the seed, before the first leg. That is once per PCOC for the producer and once per gate check for a verifier that rebuilds the terrain. It is not once per leg. The public key and the seed beacon enter only through the seed preimage.

<a id="sec-11"></a>

## 11. Leg

**Normative.**

A leg is `STEPS_PER_LEG` steps. It starts from the previous checkpoint defined in section 12. The leg result is the `u64` end state of the leg. The checkpoint is the digest below. Every leg in a stage uses the same 32-byte second limb: the seed on stage 0, and waypoint `W_g` on stage `g ≥ 1`.

<a id="sec-11-1"></a>

### 11.1 Step

Let `S` be the current `u64` state. One step computes the next state `S'`.

```text
i = S mod N_SLOTS
slot = terrain[i × SLOT_BYTES : (i+1) × SLOT_BYTES]
```

`slot` is `SLOT_BYTES` long and contains `WORDS_PER_SLOT` little-endian words. Word `w`, for `w = 0 … WORDS_PER_SLOT − 1`, is `u64_le(slot[8w : 8w+8])`. The word index is the integer `w`.

```text
acc = 0
for w = 0 … WORDS_PER_SLOT − 1:
    acc = (acc + mix64(word(w) XOR S)) mod 2^64
S_folded = acc
E = SHA-256(SEAL_DOMAIN ‖ le64(S_folded))
S' = u64_le(E[0:8])
```

`mix64` is exactly §10.1, including the initial `+ MIX_ADD`. A function that omits `MIX_ADD` is not `mix64`. A port that used that shorter function produced a different step. `acc` starts at 0, not at `S`. Each step of the loop adds the `mix64` output with wrapping addition in `mod 2^64`. Equal words do not cancel under addition. An XOR of those outputs is not this fold. The per-word input is `word(w) XOR S`. There is no `w` term. The loop is every word in the slot. Under section 6, `WORDS_PER_SLOT` is 2048, so the production bound is `w = 0 … 2047`. Profile `conformance-small` (section 16.1) uses that profile’s `WORDS_PER_SLOT` in this same sentence. It does not change the production constant.

**Normative.** Every input to `mix64` is `word(w) XOR S`. No per-word term is independent of `S`, so nothing can be pre-folded into the stored terrain. A transformed terrain array is not the §10.4 terrain and is non-conforming. Equal words do not cancel under addition.

**Informative.** The last stage of `mix64` is `y XOR (y >> 31)`, which is linear over GF(2). An accumulator that XORs `mix64` outputs lets a producer apply that stage once to the XOR of the pre-final values. Wrapping addition is not that linear map, so that hoist does not compute this fold. `mix64` itself is unchanged.

The fold `S_folded = S XOR (XOR over w of (word(w) XOR w))` is not conforming. The parenthesised XOR does not depend on `S`, so one 8-byte value per slot can stand in for the slot. It is the negative `slot_summary_fold`. The XOR accumulator `acc = acc XOR mix64(word(w) XOR w XOR S)` is also not conforming. It is the negative `xor_accumulator_fold`. The wrapping-add fold `acc = (acc + mix64(word(w) XOR w XOR S)) mod 2^64` is also not conforming. `word(w) XOR w` does not depend on `S`. It is the negative `xor_w_fold`. The wrapping-add fold `acc = (acc + mix64(word(w) XOR ((S + w) mod 2^64))) mod 2^64` is also not conforming. It is the negative `s_plus_w_fold`. At `S = 0` those two folds match each other, because `(S + w)` equals `w`, and both differ from this section: this section mixes `word(w) XOR S`.

**Normative.** Christian's requirement, in his words:

> a deliberately inefficient algorithm on the hash principle in reverse; a hash can't rehydrate the 16 KB, so each step must be driven by the full 16 KB slot, and no digest, lookup table or smaller data can stand in for it. No efficiencies, caching, tables or partial reads anywhere in hike/fold/terrain. A producer speed-up is a suspected break: flag it, never merge it.

**Normative.** A conforming producer and a conforming verifier, on every step of every leg, load all `SLOT_BYTES` bytes of the selected slot from the terrain of this PCOC and mix each of the `WORDS_PER_SLOT` words with the current state `S` by the §11.1 loop, including `mix64` with `MIX_ADD`. The load is `SLOT_BYTES` bytes per step. A value computed on an earlier step does not satisfy the load on this step. A table, a digest, a per-slot summary, or any value shorter than the slot does not satisfy the load. Terrain is filled once for this PCOC from this seed, by §10.4, before the first leg, and is not shared with another PCOC and not reused from an earlier PCOC. The same rule applies to a gate recomputing a leg: one terrain for that check, then one full slot load mixed with `S` on every step of every recomputed leg. A producer or a verifier that emits the same leg, checkpoint, or waypoint bytes by a shorter load is non-conforming. A producer speed-up is a suspected break: flag it, and do not merge it. One terrain fill for this PCOC, shared by every leg of that run, is required. It is not a cache of an earlier step and it is not a cache shared with another PCOC. Running the legs of one round concurrently is not an efficiency. Each step still reads the full slot and mixes every word with `S`.

`SEAL_DOMAIN ‖ le64(S_folded)` is 20 bytes. That is one SHA-256 block.

**Informative.** The baseline that sets the floor and the gate policy is commodity hardware with SHA-256 instructions, x86 SHA-NI or Armv8 SHA2, of the commodity laptop with SHA instructions (for example an AMD Ryzen 5 5600H).

**Normative.** Where the host provides SHA-256 instructions (x86 SHA-NI or Armv8 SHA2), an implementation uses those instructions for every SHA-256 in this construction: the seed, the seal, the checkpoint, the waypoint fold, and the waypoint. A software SHA-256 is permitted only where the host has no such instructions. This requirement does not change a byte. A vector match does not excuse a software SHA-256 on a host that has the instructions. Hardware SHA-256 and software SHA-256 give identical bytes. A vector does not test which one ran. The rule is there so a measured rate is the rate of the baseline host.

`E[0:8]` is the first eight bytes of the seal digest `E` in index order, byte 0 first. `S' = u64_le(E[0:8])` is the little-endian integer of those bytes. It is not the integer whose big-endian hexadecimal spelling is the same eight bytes. `S` and `S'` are 64-bit unsigned integers under the wrapping rule in section 3. The step does not keep a wider integer. Section 7.4 records the `S_folded = 0` check: those eight digest bytes spell `4816bdaf23240476` in index order, and the integer `S'` is `0x76042423afbd1648`.

The step reads terrain and does not write it.

<a id="sec-11-2"></a>

### 11.2 Leg result

```text
S_0 = u64_le(previous_checkpoint[0:8])
S_{t+1} = Step(S_t)    for t = 0 … STEPS_PER_LEG − 1
leg_result = S_{STEPS_PER_LEG}
```

`leg_result` is a `u64`. On the hike object it is the byte string `le64(leg_result)`.

<a id="sec-11-3"></a>

### 11.3 Terrain sample

The terrain sample at a previous checkpoint is a contiguous window of `SAMPLE_BYTES` bytes. It is not aligned to a slot boundary.

```text
offset = u64_le(previous_checkpoint[0:8]) mod OFFSET_MODULUS
terrain_sample = terrain[offset : offset + SAMPLE_BYTES]
```

The offset is taken from the first eight bytes of the previous checkpoint. The window itself is the sample.

This window is not the waypoint’s slot sample. The slot sample is defined in section 12.

<a id="sec-11-4"></a>

### 11.4 Checkpoint

Rule A2.3. The checkpoint bind, in order, is

```text
H(previous checkpoint ‖ stage limb ‖ terrain sample(at previous checkpoint) ‖ leg result)
```

`H` is `SHA-256`. The stage limb is the seed on stage 0 and `W_g` on stage `g ≥ 1`. The four limbs are raw bytes:

| Order | Limb | Bytes |
|---|---|---|
| 1 | previous checkpoint | 32 |
| 2 | seed on stage 0; `W_g` on stage `g ≥ 1` | 32 |
| 3 | terrain sample at the previous checkpoint | `SAMPLE_BYTES` |
| 4 | leg result | `le64(leg_result)`, 8 bytes |

The preimage is `32 + 32 + SAMPLE_BYTES + 8` bytes. Under section 6 that is `32 + 32 + 1024 + 8 = 1096` bytes. On stage 0 the second limb is the seed.

The stage `path` is not a limb of this preimage. Stage 0 has no path.

The checkpoint is the 32-byte digest. The hike object stores that digest and the matching `leg_results` limb. The first limb is the full 32-byte previous checkpoint. The eight bytes that select the terrain sample are not a substitute for that limb. The leg result appears only as the fourth limb.

<a id="sec-12"></a>

## 12. Stage and waypoint

**Normative.**

Stages are `stages[0]` through `stages[17]`.

A stage contains one or more legs. The count is how many legs the drive in section 13 recorded before interrupt `I_g`. Every leg in stage 0 uses the seed as its second checkpoint limb. Every leg in stage `g ≥ 1` uses waypoint `W_g`.

<a id="sec-12-1"></a>

### 12.1 Previous checkpoint

<a id="rule-a2"></a>

#### Rule A2. Stages and where leg 0 starts

<a id="rule-a2-1"></a>

##### Rule A2.1 Eighteen stages

`seed_beacon` plus the 18 interrupts gives 18 stages, numbered 0–17.

<a id="rule-a2-2"></a>

##### Rule A2.2 Leg 0

The seed is

```text
stages[0].waypoint = SHA-256(public_key ‖ seed_beacon)   # the seed; 64-byte seed preimage; no label
```

The seed equals `stages[0].waypoint` by design. That field is the seed. It is not a Rule A1 waypoint. It is stored on the object. It is the stage limb of stage 0’s checkpoint preimages (section 11.4). It is the third limb of `W_1`’s preimage (section 12.2). It is never posted to Bari. Stage 0 has no waypoint of its own. The seed is not one of `W_1` … `W_17`.

Leg 0 of stage 0 starts from the seed. Leg 0 of stage `g ≥ 1` starts from `W_g`:

```text
stage 0:           previous = stages[0].waypoint    # the seed
stage g, g ≥ 1:    previous = stages[g].waypoint    # W_g
```

Leg `j` for `j ≥ 1` uses previous checkpoint `checkpoints[j − 1]` of that same stage.

The public key reaches every later checkpoint, fold, and waypoint through the seed.

A stage does not carry the last checkpoint of the previous stage forward. The previous stage’s checkpoints reach the next stage through the fold in its waypoint (section 12.2).

<a id="rule-a2-3"></a>

##### Rule A2.3 Checkpoint

The checkpoint preimage is section 11.4. The stage limb is the seed on stage 0 and `W_g` on stage `g ≥ 1`. Leg 0’s previous checkpoint is that same value. Leg `j ≥ 1` uses `checkpoints[j − 1]` of that stage.

<a id="rule-a2-4"></a>

##### Rule A2.4 First post

The first Bari post is `W_1`, after `I_0`. Stage 0 has no plant and no path.

<a id="sec-12-2"></a>

### 12.2 Waypoint

#### Waypoint preimage

Rule A1.3. For `g` from 0 through 16, after stage `g` has ended at interrupt `I_g`, produce computes `W_{g+1}` as follows. `B` is `I_g`, in natural order. It is `stages[g+1].beacon`. `B` is not `stages[g].beacon`. `W` is `stages[g].waypoint`: the seed when `g = 0`, and `W_g` when `g ≥ 1`. `C[0] … C[n−1]` are `stages[g].checkpoints` in leg order. `n ≥ 1`. `fold_g` is the fold below.

```text
F_0 = 0^32
F_{j+1} = SHA-256(F_j ‖ C[j])    for j = 0 … n − 1
fold = F_n                        # fold_g

slot_index = u64_le(B[0:8]) mod N_SLOTS
slot_sample = terrain[slot_index × SLOT_BYTES : (slot_index + 1) × SLOT_BYTES]

W_{g+1} = SHA-256(B ‖ slot_sample ‖ W ‖ fold)
```

That concatenation order is normative. `B` is the interrupt `I_g`, in natural order, stored as `stages[g+1].beacon`. `slot_sample` is the terrain slot indexed by `I_g`’s hash. The preimage is `32 + SLOT_BYTES + 32 + 32` bytes. Under section 6 that is `32 + 16384 + 32 + 32 = 16480` bytes.

The stage `path` is not an input to this preimage.

`F_0` is the start value of the fold. It is not itself hashed. When `n ≥ 1`, the first hash in the fold is `SHA-256(0^32 ‖ C[0])`.

Produce requires `n ≥ 1`. An empty checkpoint list is a shape `mismatch` (`leg_count` when that count differs from `len(checkpoints)`), not a skipped waypoint.

Verify still applies this fold when both stages of the pair are present and the checkpoint bytes are usable. `n = 0` leaves `fold = F_0 = 0^32` and performs no hash. Verify recomputes `W_{g+1}` from that fold and records `mismatch` when the digest differs from the stored waypoint. That stage’s `legs` is `[]`. `not defined` is recorded only when the stage pair is absent or the bytes needed for the recompute are absent or unusable. An empty checkpoint list is usable. `whole_hike_legs` adds each stage’s checkpoint count, including zero, when every stage from 1 through 17 has a usable count. A missing stage leaves `whole_hike_legs` `not defined`. `six_leg` is `not defined` when `leg_total < 6`.

`W_{g+1}` is the pivot. It ends stage `g` and starts stage `g+1`. Produce posts it to Bari with the Hashcash fee (section 13.1). Leg 0 of stage `g+1` starts from it.

`W_1` is built after `I_0` from that interrupt, its slot sample, the seed, and the fold of stage 0’s checkpoints. It is the first waypoint posted to Bari.

Stage 17 ends at `I_17`. No waypoint is computed from stage 17 or from `last_interrupt`. Stage 17’s checkpoints are not folded into a waypoint.

<a id="sec-12-3"></a>

### 12.3 What a stage stores

When stage `g` ends at `I_g`, the object records the header the stage started from (`beacon` and `beacon_height`), the seed or the waypoint the stage’s legs used, `leg_count`, `checkpoints`, and `leg_results`.

For `g` from 0 through 16, `I_g` is stored as `stages[g+1].beacon` and `stages[g+1].beacon_height`. `I_17` is stored as `last_interrupt` and `last_interrupt_height`.

Stage 0 has no plant and no path key. No ask is made for stage 0.

The path ask window is section 13.1. No ask is made above the cut-off. A waypoint `W_1` … `W_16` with no checked path stores empty bytes and produce writes `path_skip <g>` (Rule A3.4). `W_17` stores empty bytes and produce writes `not fetched by design 17`.

<a id="sec-13"></a>

---

## 14. Verify — relocated

**FRAMING (2026-10-24).** The measurement procedure formerly inlined here as §14 Verify (Rule A4, §§14.1–14.7, Rule A5) was relocated without wording change to [06-Measurement_Record_Process.md](./06-Measurement_Record_Process.md). Construction defines the object; Measurement reads a finished object against Construction and emits the fact record. Rule-index rows A4/A5 above still identify those rules.
