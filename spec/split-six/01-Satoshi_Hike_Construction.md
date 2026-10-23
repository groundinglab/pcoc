# Satoshi Hike Construction

**Responsibility.** Defines the object, the terrain, the leg, the stage and waypoint binding, the encodings, and the measurement procedure. Used by both a producer implementer and a verifier implementer to produce and measure conformant PCOC objects.

**Source.** Inherited without change of wording from the following sections of Satoshi Hike 2.3 (`spec/satoshi-hike-v2.3/SPEC.md`):

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
- §14 Verify

---

## 1. Status

The status prose in this section is **informative**. The rule index is the ID-to-heading map. The subsection “Part A rule classes” is the **normative** classification.

This document is Satoshi Hike 2.3. Part A is the construction: `SAMPLE_BYTES` 1024, `STEPS_PER_LEG` 2000000, `KIND` `v2.2-pcoc-object`, anchor-window offsets 2, 3, and 4, the NL4 terrain word, the §11.1 fold, the asynchronous producer (Rule A6), and the unknown-kind measurement in Rule A5. Section 13.6 is the Bari waypoint service. Part B defines the admission levers (section 15.4): for each, a name, a unit, an allowed range, and how it is measured or compared. A policy sets the values. This file states no lever value and gives no lever a default. The leg-rate floor must be measured under this file (section 15.5). The fake-leg ignore is normative (Rule B7). Rule B9, including the `unknown_kind` reason, is normative. Measurement stays in section 14. Judgment stays in section 15. Section 16.2 states the implementation requirements; the SDKs are the reference implementations.

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
| <a id="rule-a4-5-2"></a>A4.5.2 | INVARIANT | Anchored and fetched is the `anchors` token `anchored`: in the section 13.1 window, an interrupt header stored on the object, and plant `match`. `anchored_total` is that count. This row records it and does not judge it. Rule B4, settled and normative, compares it with 6. | §14.3, §14.5 |
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

`analysis` is an object with `whole_hike_legs`, `whole_hike_span`, `per_stage`, `legs_outside`, `hike_legs`, and `hike_span`, in that order. `hike_legs` and `hike_span` are Rule A4.10. `whole_hike_legs` is the number of checkpoints over stages 1–17, a JSON number in `0 … 2^53 − 1`. `whole_hike_span` is the JSON number `T(I_17) − T(I_0)` in ±(2^53−1), or `not read`. `per_stage` is an array of 18 objects, index `g` from 0 through 17, each with `g`, `checkpoint_count`, and `span`. `checkpoint_count` is the number of checkpoints in the stage. It is not a read of the object’s `leg_count`. `span` is the JSON number of the per-stage span in ±(2^53−1), or `not read`. `legs_outside` is the number of checkpoints over stages outside `a` … `b−1`, a JSON number in `0 … 2^53 − 1`. It is `not defined` when the bookend fields are `not defined`, and `not read` when those fields are `not read`. `whole_hike_legs` counts checkpoints, including a stage whose count is zero. `whole_hike_span` and each `per_stage` span stay `not read` when an endpoint `T` was not taken, including when the bookend fields are `not defined`. `analysis` has no rate key.

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

The verifier’s wall clock is not an input. Verify does not Observe, so it cannot recover an unstored header by assuming the next height. Block times are `T` from Rule A4.2, read as they are, and a `T` is recorded only when that header’s `index_comparison` is `match`. No clamp and no reordering is applied to a recorded `T`. Bookend `S` uses those raw `header.time` values. `hike_span` does not. Rule A4.10 sets `hike_span` from median-time-past at the first beacon and at the last beacon. No other adjustment is applied. `delta_h` (`Δh`) is recorded in this bookend block when both heights are index-confirmed. `delta_h` stays in the bookend block. Rule B3 does not read it. Rule B3 is not an admission input. It is a raw measurement. It is not analysis and it is not policy.

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

**First beacon.** The first beacon is `seed_beacon`, the object field `stages[0].beacon`. Its stored height is `stages[0].beacon_height`. In the verify record it is `headers[0]`.

**Last beacon.** The last beacon is `last_interrupt`, which stores `I_17`. Its stored height is `last_interrupt_height`. In the verify record it is `headers[18]`.

**How the verifier reads the times.** Rule A4.2 already issues `GET {index-url}/index/{stages[0].beacon_height}` and `GET {index-url}/index/{last_interrupt_height}`, each height in the shortest base-10 digits, under section 9. `T(first beacon)` is `headers[0].T`. `T(last beacon)` is `headers[18].T`. Each is the integer `header.time` of that index document only when that header’s `index_comparison` is `match` and `header.time` follows section 9. Otherwise that `T` is `not read`. The object stores no block time. Those two `T` values are not `hike_span`. Bookend `S` and the analysis spans still use `T`. The verifier does not subtract a time taken from a producer-stored height for those spans.

**Median-time-past.** `hike_span` uses median-time-past, not raw `nTime`, so a producer cannot shorten the span by choosing a beacon whose `nTime` sits ahead of real time. For a beacon at height `h`, let `n = min(11, h + 1)`. The verifier reads the `n` headers at heights `h − n + 1` through `h` inclusive. Each read is `GET {index-url}/index/{height}` under section 9, with the height in the shortest base-10 digits. The beacon’s own header is the Rule A4.2 read already required for `headers[0]` or `headers[18]`. The other heights in that window are additional reads. They are required for `hike_span` only. They are not a plant scan and they do not choose a different interrupt. Take `header.time` from the beacon’s document only when that header’s `index_comparison` is `match` and the time follows section 9. Take `header.time` from each other document in the window when the read returned a document, `header.height` equals the requested height, and `header.time` follows section 9. The portable index carries no previous-hash member (section 9), and this measurement does not link the window’s headers to each other. If any of the `n` times is missing, MTP for that beacon is `not read`. Otherwise sort the `n` times into non-decreasing order and take the value at index `n // 2`, counting the first as index 0. For the ordinary case `n = 11`, that index is 5, the middle of the 11. A height below 10 uses the shorter window. A beacon on the production profile is above height 10, so `n = 11`.

`MTP(first beacon)` is that value at `stages[0].beacon_height`. `MTP(last beacon)` is that value at `last_interrupt_height`. The verifier reads those windows from the portable index. It does not read a time the producer stored on the object.

- `hike_legs`: the number of checkpoints over stages 0 through 17, the sum of each stage’s checkpoint count, including stage 0 and including a stage whose count is zero. The count does not read `leg_count`. It is not `whole_hike_legs`. It is not bookend `N`.
- `hike_span`: `MTP(last beacon) − MTP(first beacon)`, a JSON number in ±(2^53−1) when both MTP values are integers, including zero or a negative value. If either MTP is `not read`, `hike_span` is `not read`. Do not invent an integer. `hike_span` ≤ 0 is still that integer. The rate that uses it is undefined, and Rule B2 refuses. `hike_span` is not `T(last beacon) − T(first beacon)`.
- Whole-hike, analysis only: the number of checkpoints over stages 1–17, and the span `T(I_17) − T(I_0)`. `I_0` is `stages[1].beacon`. `I_17` is `last_interrupt`. Stage 0’s checkpoints are not in `whole_hike_legs`. They are in `hike_legs`. The count does not read `leg_count`. This pair is not the admission rate.
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


<a id="sec-15"></a>

