# Satoshi Hike v2.1

> **Correction, 22 Oct 2026.** This file is the live construction. It corrects Satoshi Hike v2.0. Satoshi Hike v1.35 is historical. One lineage. The path of this file stays `spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md`. The text at that path is v2.1. The wire `KIND` is `satoshi-hike-v2.1-object`. An object whose `kind` is `satoshi-hike-v2.0-object` or `satoshi-hike-v1.35-object` is an earlier object (Rule A5). The admission service refuses it with reason `earlier_version` before any terrain fill. That refuse is not a fake leg and does not add the key to the ignore record. SDKs and the gate move together with this kind.

**Status.** The parameters in the list below are the 2026-10-08 lock, as corrected on 22 Oct 2026. Sign-off of this file is not recorded here.

### v2.1 correction (22 Oct 2026)

**Normative** for the rules this list names. Each line is old → new. The sections below are the construction. This list does not add a second one.

1. **Seed (P10.1, §10, glossary).** Old: the 64 bytes `public_key ‖ seed_beacon` were called the seed (`production_seed`) and were split into eight lanes. New: the **seed** is `D = SHA-256(public_key ‖ seed_beacon)`, 32 bytes, no label. The 64 bytes are the **seed preimage**. `D` is read as four little-endian lanes `P[0] … P[3]`.
2. **`SEED_DOMAIN` / fill pin (P6.14, §6, §10.2).** Old: `SEED_DOMAIN` is `PSEED64` and a NUL, and `fill_pin` squeezes the preimage to one `u64`. New: both are deleted. P6.14 is `RETIRED` and is not reused. No terrain word is a function of a 64-bit pin.
3. **Base word (§10.4).** Old: `w[j] = mix64(fill_pin XOR (i << 32) XOR j XOR WORD_XOR)`. New: the NL4 chain in §10.4. Parent mixing and `slot_mix` are unchanged. Normative: every terrain word depends on all 256 bits of the seed. The fill runs once per terrain (once per PCOC, once per gate check), not once per leg.
4. **Seed and stage 0 (A2.2).** Old: `stages[0].waypoint` was called the seed, defined as `SHA-256` of the 64-byte seed, with no sentence that the reuse is intended. New: that field **is** the seed. The seed equals `stages[0].waypoint` by design. The bytes of `stages[0].waypoint` do not change. The old name of those bytes was seed value. The 64-byte `production_seed` is the seed preimage, not this field.
5. **Step fold (§11.1, P11.1).** Old: `S_folded = S XOR (XOR over w of (word(w) XOR w))`, the v2.0 fold. New: `acc = 0`; for each word, `acc = (acc + mix64(word(w) XOR w XOR S)) mod 2^64`; `S_folded = acc`; then the same seal. The accumulator is wrapping addition. `mix64` is §10.1, including `+ MIX_ADD`. `mix64` without `MIX_ADD` is not `mix64`. An XOR accumulator is not this fold.
6. **Full-slot principle (§11.1).** Old: the step text did not forbid a per-slot summary. New: Christian's requirement, quoted in §11.1, is normative. No efficiency, caching, table, or partial read is permitted in the hike, the fold, or the terrain. A producer speed-up is a suspected break.
7. **Hardware SHA (§11.1).** Old: the file said nothing about the host. New: informative baseline is commodity hardware with SHA-256 instructions (x86 SHA-NI or Armv8 SHA2), commodity laptop with SHA instructions (for example an AMD Ryzen 5 5600H). Where the host provides those instructions, an implementation uses them for every SHA-256 in this construction. A software SHA-256 is permitted only where the host has none. A vector match does not excuse a software SHA-256 on a host that has the instructions.
8. **Admission rate (B0, B2, A4.9).** Old: the admission service reads the bookend pair `(N, S)`. New: `leg_rate = hike_legs / hike_span`, where `hike_legs` is the checkpoint count over stages 0–17 and `hike_span = MTP(last beacon) − MTP(first beacon)`. MTP is the median-time-past of that beacon block, the BIP113 median of the `nTime` values of the headers ending at that block, read from the portable index as Rule A4.10 states. The comparison is `≥ leg_rate_floor` and requires `hike_span > 0`. `hike_span` ≤ 0 means the rate is undefined. The outcome is `refuse`, not `held`. Bookend `(N, S)` stays a section 14 measurement and is not the admission rate. Bookend `S` still uses the raw endpoint `header.time` values.
9. **Anchors (B4).** Old: candidate `min_anchored_paths` of 8, 7, or 7, and candidate `min_last_bookend`. New: `anchored_total ≥ 6`, anywhere among `W_1` … `W_16`. Settled. Normative. Not a candidate. No first-planted rule. No last-planted rule. This comparison is the SPEC exception to the older sentence that this file never sets a threshold.
10. **Leg counts (B8, §4).** Old and new: there is no minimum or maximum leg count as policy. `leg_count ≥ 1` per stage stays the section 13.2 loop, recorded under shape. It is not an admission floor. Not new: no per-stage admission minimum and no whole-hike leg-count minimum.
11. **Profile size (§6, §15).** Old: a reader could take 18 stages as a security parameter. New: `STAGES` = 18 is a profile constant, not a security number. The settled description of this arbitrary profile size is 18 stages and 19 beacons (`seed_beacon` plus `I_0` … `I_17`). The phrase “18 beacons / 17 stages” is the old name of that arbitrary size. It is not a second count. Admission does not treat 18, 17, or 19 as a threshold. v2.1 does not change the count.
12. **Gate sample (§15.5, B7).** Old: candidate `thorough_six_leg_runs` of 15, 8, and 8 (90, 48, and 48 leg results). New: the default policy is cheap checks on every checkpoint and every waypoint, then 18 distinct legs drawn once and split into 3 rounds of 6, in that draw order. Each round’s 6 legs run in parallel. A `mismatch` refuses that submission and does not start the next round. `leg_total` < 6 makes the section 14.6 draw `not defined`, and the admission service refuses. When `6 ≤ leg_total < 18`, an index is not repeated while an unused index remains. After every index has been used, a later draw may repeat an index, and the 6 indices of the current round stay distinct. The round count is policy. The default is 3.
13. **Fake leg (§15.5, B7).** Old: a six-leg `mismatch` refuses that submission (Rule B9) and does not ignore later submissions of the key. New: a fake leg, a section 14.6 recompute `mismatch`, refuses that submission, and that gate ignores every later submission from that public key. The ignore is per key and that gate only. This file sets no expiry. A gate policy may set a duration. Later submissions get the same refusal. A cheap-check failure is a plain refusal and does not add the key. The ignore is that gate’s persistent per-key record. It survives a policy-version bump and a gate restart. A re-verify does not give a free redraw. Cross-gate sharing of that record is out of scope. Settled. Normative. Not a candidate.
14. **Floor (B2, §15.5).** Old: informative examples `7/20`, `1/4`, and `1/8`. New: the floor is TBD. The measuring method is in §15.5. A test placeholder of `0.2` checkpoints per second (`1/5`) is marked placeholder, not ratified. The old examples are withdrawn as the floor.
15. **v2.0 fold (§11.1, conformance prose).** Old: the §11.1 fold was the v2.0 fold, and §15.5 published the 48-leg and 90-leg candidates. New: that fold and those candidates are deleted. The historical pack `spec/conformance/v2.0/` is not the v2.1 target.

16. **Kind (P6.15, Rule A5, §15.1).** Old: `KIND` is `satoshi-hike-v2.0-object`. New: `KIND` is `satoshi-hike-v2.1-object`. Kind `satoshi-hike-v2.0-object` is an earlier object, recorded like kind `satoshi-hike-v1.35-object`. Section 15 refuses it with reason `earlier_version` before any terrain fill. That refuse is not a fake leg. SDKs and the gate move together with this kind. A deployment does not cut this kind over while an SDK or the gate still emits the earlier kind.
17. **Seal digest (§7.4, §11.1).** Old: the seal digest was also named `D`. New: the seal digest is `E`. `E = SHA-256(SEAL_DOMAIN ‖ le64(S_folded))` and `S' = u64_le(E[0:8])`. `D` names only the seed.
18. **Fold accumulator (§11.1, 22 Oct 2026 00:01 ACDT).** The last stage of `mix64` is linear over GF(2). An XOR of `mix64` outputs lets a producer apply that stage once. New: the §11.1 accumulator is wrapping addition, `(acc + mix64(word(w) XOR w XOR S)) mod 2^64`. That hoist does not compute this fold. `mix64` itself is unchanged. The XOR accumulator is a conformance negative, `xor_accumulator_fold`. It is not the v2.0 fold.
19. **Submission deadline (§15, 22 Oct 2026 00:01 ACDT).** Old: a hike could be submitted any number of blocks after the last beacon. New: the gate refuses when `tip_height − last_beacon_height > K` at receipt. `tip_height` is the tip the gate observes at receipt. `K` is policy. A placeholder of 3 is placeholder, not ratified. A gap of 3 does not refuse. A gap of 4 refuses when `K` is 3. When `tip_height` is below `last_beacon_height`, the state is `held`. The reason token for the refuse is `submission_deadline`. That refuse is not a fake leg.
20. **Per-word mix (Christian's decision, 22 Oct 2026 02:21 ACDT).** Old: a per-word `w` term, `mix64(word(w) XOR w XOR S)` and `mix64(word(w) XOR ((S + w) mod 2^64))`. The term `word(w) XOR w` does not depend on `S`. New: `acc = 0`; for `w` in `0 … 2047`, `acc = (acc + mix64(word(w) XOR S)) mod 2^64`; then the existing SHA-256 seal. There is no `w` term. `mix64` is unchanged, including `+ MIX_ADD`. Every input to `mix64` is `word(w) XOR S`. No per-word term is independent of `S`. A transformed terrain array is not the §10.4 terrain and is non-conforming. Equal words do not cancel under addition. The negatives are `xor_w_fold`, `s_plus_w_fold`, `xor_accumulator_fold`, and the v2.0 fold. This line replaces the per-word input in items 5 and 18.
21. **Deadline clock and once per submission (D1, D2, Rule B10, 22 Oct 2026).** `tip_height` is the section 15.3 Gate `{index-url}` tip clock, read fresh at receipt, bypassing the local mirror. A failed or stale read is `not read` and the state is `held`. With several index sources configured, the gate uses the highest tip among those read. The deadline is Rule B10, a fast-phase check decided once per client submission, at the first successful tip read for that submission. Held retries and service-started re-verifies reuse that decision. A policy-version bump never revokes on this rule. A client resubmission is a new receipt. The parameter is `submission_deadline_blocks`, placeholder 3, not ratified. A lower tip after a reorg does not reopen a deadline refuse. Where an MTP window header’s index body carries a previous-hash, that previous-hash links to the next header toward the beacon.

**Changes from v1.35.**

- `KIND` is `satoshi-hike-v2.1-object`. Kind `satoshi-hike-v2.0-object` is an earlier object.
- `SAMPLE_BYTES` is 1024. `OFFSET_MODULUS` is 268434433. The checkpoint preimage is `32 + 32 + 1024 + 8 = 1096` bytes.
- `STEPS_PER_LEG` is 2000000. One step still reads one slot of `SLOT_BYTES` 16384.
- Amended 2026-10-17 (proposed `spec-v2.0.5`): conformance `constants.json` labels the §15.5 leg-rate figures as informative examples. No normative change.
- Amended 2026-10-16 (proposed `spec-v2.0.4`), policy boundary: `leg_rate_floor` is a profile parameter, and this file states no value for it; section 15.5 lists example values only, and other profile figures there stay candidates. this file defines measurements and may discuss thresholds. Apart from `anchored_total ≥ 6` (Rule B4, settled 21 Oct 2026), it sets no threshold value. The sentence “it never sets a threshold value” is superseded for that comparison. Values belong to a policy, which a relying party chooses. The earlier lock of the `leg_rate_floor` values strict `7/20`, default `1/4`, lenient `1/8` is withdrawn. Those figures were kept in section 15.5 as informative examples by that amendment. v2.1 withdraws them (changelog item 14). v2.1 also settles the anchored-waypoint comparison at 6 (changelog item 9). That comparison is Rule B4. It is not a Part A measurement. One argued default policy ships beside the admission gate service in `services/admission-gate/POLICY.md`. It is a recommendation, not part of this file.
- The anchor window is three blocks. `ANCHOR_WINDOW_OFFSETS` is 2, 3, 4, written once in section 13.1. The producer asks `W_1` … `W_16` after each header in that window. `W_17` is posted and is not asked, including when a jump would put an offset at or below the cut-off. A path carries the height of the block that includes it. Verify records `anchored` only when that inclusion height is in the window, that header is an interrupt stored on the object, and the plant comparison against that one header is `match`. Otherwise it records `not anchored`. It reads that one inclusion height only in that case. It does not scan several index heights. A missing path, a path outside the window, or a path whose inclusion header is not stored on the object is `not anchored`. The hash comparison is the same 32 natural-order bytes (section 9).
- Rule A5 records `version` `earlier version` for kind `satoshi-hike-v1.35-object` and for kind `satoshi-hike-v2.0-object`. That is a measurement. It is not a decode failure. Section 15 refuses that record with reason `earlier_version`. That reason is policy. Section 14 does not record it.
- Rule A4.3 checkpoint comparisons may run concurrently. How many run at once is a verifier setting. The verify record does not depend on that number. One at a time is conforming.

The Bari fee cap is not a rule of this file.

`SEAL_DOMAIN` stays the twelve bytes named in section 6. That spelling is the wire constant. It is not a name for a leg.

**Informative.** One picture of the object, before the machinery. The normative shape is section 8. One public key, one seed beacon, and a terrain that is recomputed and not stored. Eighteen stages. Each leg is `STEPS_PER_LEG` steps and ends in a leg result and a checkpoint. Waypoints are `W_1` … `W_17`. `last_interrupt` ends stage 17, and no waypoint is built from it.

The producer holds `TERRAIN_BYTES` 268435456 bytes and takes 2,000,000 steps per leg. Each step reads one slot of `SLOT_BYTES` 16384. That occupancy is the cost that does not divide. Separately, each post of `W_1` … `W_17` pays Hashcash of 21 leading zero bits to Bari. That fee buys the plant. It does not buy the leg. The Bari fee cap is not a rule of this file.

A verifier checks four different things. Every checkpoint is the Rule A4.3 digest over the 1024-byte sample, and that digest is not a leg. The waypoints are recomputed from stored checkpoints. Each stored header is read from the portable index. A present path among `W_1` … `W_16` is read at the one inclusion height only when that height is in the window and that header is a stored interrupt. The default policy then recomputes 18 random legs as 3 rounds of 6, each round’s 6 legs in parallel, at 2,000,000 steps each. The Rule A4.3 comparisons are cheap checks and may run at the same time as each other. The anchor window is the offsets 2, 3, and 4. A missing path, a path outside the window, or a path whose header is not a stored interrupt is `not anchored`.

This file is the specification. A hike, a checkpoint, a waypoint, a seed, and a leg result are defined here.

Sections marked **Normative** are the construction. Sections marked **Informative** explain that construction and do not add a second one.

## 1. Status

The status prose in this section is **informative**. The rule index is the ID-to-heading map. The subsection “Part A rule classes” is the **normative** classification.

This document is Satoshi Hike v2.1, the 22 Oct 2026 correction of v2.0. Part A applies the 2026-10-08 lock as corrected above: `SAMPLE_BYTES` 1024, `STEPS_PER_LEG` 2000000, `KIND` `satoshi-hike-v2.1-object`, anchor-window offsets 2, 3, and 4, the NL4 terrain word, the §11.1 fold, and the earlier-kind measurement in Rule A5. Part B states no ratified value for `leg_rate_floor`. That value is TBD and is chosen by a policy outside this file. The anchored-waypoint minimum of 6 is settled and normative (Rule B4). The fake-leg ignore is settled and normative (Rule B7). Rule B9, including the `earlier_version` reason, is normative. Measurement stays in section 14. Judgment stays in section 15. evidence status orders the later evidence.

### Rule index

**Normative** for the ID-to-heading map. The heading text in the section named is the rule. A test named for an ID tests that heading. This table does not add a second rule. Classes for these headings, and for unnumbered Part A sentences, are in “Part A rule classes” below.

| ID | Heading | Section |
|---|---|---|
| A1 | Rule A1. Event order at every stage boundary | 13 |
| A1.1 | Rule A1.1 Legs, then the tip | 13 |
| A1.2 | Rule A1.2 The interrupt ends the stage | 13 |
| A1.3 | Rule A1.3 Waypoint byte order | 13 |
| A1.4 | Rule A1.4 Post | 13 |
| A1.5 | Rule A1.5 Pivot | 13 |
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
| A5 | Rule A5. Earlier objects | 14.7 |
| B0 | Rule B0. Which rate the admission service reads | 15 |
| B1 | Rule B1. Integrity | 15 |
| B2 | Rule B2. Leg-rate floor | 15 |
| B3 | Rule B3. Span sanity | 15 |
| B4 | Rule B4. Paths minimum | 15 |
| B5 | Rule B5. Plant gap | 15 |
| B6 | Rule B6. Fixed start | 15 |
| B7 | Rule B7. Resubmits | 15 |
| B8 | Rule B8. Leg count | 15 |
| B9 | Rule B9. Invariant refuses | 15.2 |
| B10 | Rule B10. Submission deadline | 15 |

### Part A rule classes

**Normative.**

This subsection is the normative classification of Part A. It does not add a second construction. Each row is one rule and one class. Informative prose, section 5, section 7.4, section 13.4, the expected-case list in section 13.1, and sections 17 and 18 are not rows. A glossary sentence that only renames a row is not a second row. The **Normative** and **Informative** marks on the sections a row cites stay where those sections put them.

**Rule ID scheme.**

- Heading IDs in the rule index stay A1–A5 and their dotted children. A heading that states one rule of one class is that row.
- When one heading states more than one class, the rows are `ID.n` in SPEC order (`n` starts at 1). Those rows are the classification. The parent ID is the group and is not given a second class.
- A2 is a group heading. A2.1–A2.4 are the rows. A2 is not a separate class.
- A normative Part A sentence that has no heading ID gets `P<section>.<n>`, in reading order. The digits before the first dot are the SPEC section number. `P` marks a Part A construction paragraph. It does not renumber A1–A5 and it is not Part B. An ID in this table is never renumbered. A retired ID stays, marked retired, and is not reused.
- Classes are `INVARIANT`, `CONSTANT`, `RANGE`, `POLICY THRESHOLD`, and `RETIRED`. An INVARIANT holds byte for byte or as an exact measurement. A CONSTANT is a fixed parameter of this version. A RANGE is an allowed value or format. A POLICY THRESHOLD is admission only. `RETIRED` marks an ID that stays in this table and is not reused. An ID in this table is never renumbered.
- Part A has no POLICY THRESHOLD row. Sentences that can be misread as thresholds are listed after the table and stay in the class written here. Admission thresholds are named parameters in section 15. Rule B9 is the one section 15 row in this table. It classes those refuses. It is not a Part A measurement.

| Rule ID | Class | One-line rule | SPEC locus |
|---|---|---|---|
| P2.1 | RETIRED | Eighteen stages. The live rule is A2.1. This ID is not reused. | §2, §12.1 |
| P2.2 | INVARIANT | Produce and verify perform no signature check. The admission service checks the signature (section 15). | §2, §15 |
| P3.1 | INVARIANT | Ranges are half-open; `0^n` is `n` zero bytes; `‖` is raw concatenation; `H` is FIPS 180-4 SHA-256. | §3 |
| P3.2 | RANGE | `le64` and `u64_le` are 8 little-endian bytes of an unsigned integer below `2^64`. | §3 |
| P3.3 | INVARIANT | `mod` is the non-negative remainder; `mod 2^64` keeps the low 64 bits. | §3 |
| P4.1 | RANGE | Outcome tokens are the closed lowercase set. There is no token `not obtained`. | §4 |
| P4.2 | INVARIANT | Part A does not pass, fail, reject, deny, or admit. | §4 |
| P6.1 | CONSTANT | `SLOT_BYTES` is 16384. | §6 |
| P6.2 | CONSTANT | `WORDS_PER_SLOT` is 2048. | §6 |
| P6.3 | CONSTANT | `TERRAIN_BYTES` is 268435456. | §6 |
| P6.4 | CONSTANT | `N_SLOTS` is 16384. | §6 |
| P6.5 | CONSTANT | `SAMPLE_BYTES` is 1024. | §6 |
| P6.6 | CONSTANT | `OFFSET_MODULUS` is 268434433. | §6 |
| P6.7 | CONSTANT | `STEPS_PER_LEG` is 2000000. | §6 |
| P6.8 | CONSTANT | `STAGES` is 18. | §6 |
| P6.9 | CONSTANT | `HASH_BYTES` is 32. | §6 |
| P6.10 | CONSTANT | `LEG_RESULT_BYTES` is 8. | §6 |
| P6.11 | CONSTANT | `PUBLIC_KEY_BYTES` is 32. | §6 |
| P6.12 | CONSTANT | `BEACON_BYTES` is 32. | §6 |
| P6.13 | CONSTANT | `SEAL_DOMAIN` is the twelve bytes `pcoc-walk-v1`. | §6 |
| P6.14 | RETIRED | `SEED_DOMAIN` / `PSEED64` is deleted. This ID is not reused. The seed has no domain label. | §6 |
| P6.15 | CONSTANT | `KIND` is `satoshi-hike-v2.1-object`. | §6 |
| P6.16 | CONSTANT | `FILL` is `dag2`. | §6 |
| P6.17 | CONSTANT | `MIX_ADD` is `0x9E3779B97F4A7C15`. | §6 |
| P6.18 | CONSTANT | `MIX_MUL1` is `0xBF58476D1CE4E5B9`. | §6 |
| P6.19 | CONSTANT | `MIX_MUL2` is `0x94D049BB133111EB`. | §6 |
| P6.20 | CONSTANT | `WORD_XOR` is `0xC0FFEE`. | §6 |
| P6.21 | CONSTANT | `PARENT_MUL` is `0xD1B54A32D192ED03`. | §6 |
| P6.22 | CONSTANT | `SLOT_MUL` is `0xA24BAED4963EE407`. | §6 |
| P6.23 | INVARIANT | `WORDS_PER_SLOT = SLOT_BYTES / 8`, and the division is exact. | §6 |
| P6.24 | INVARIANT | `N_SLOTS = TERRAIN_BYTES / SLOT_BYTES`, and the division is exact. | §6 |
| P6.25 | INVARIANT | `OFFSET_MODULUS = TERRAIN_BYTES − SAMPLE_BYTES + 1`. | §6 |
| P6.26 | INVARIANT | This version has no `AUDIT_K`. The six-leg draw count is not a section 6 constant. | §6 |
| P7.1 | INVARIANT | The first eight bytes of a digest are `b[0:8]`. `u64_le` does not reverse a digest a second time. | §7.1 |
| P7.2 | RANGE | Hex uses `0-9`, `a-f`, and `A-F` with no spaces. A 64-digit spelling is 32 bytes, byte 0 first. | §7.2 |
| P7.3 | INVARIANT | Produce emits one CBOR map in core deterministic encoding. | §7.3 |
| P7.4 | INVARIANT | Top-level and per-stage key orders are the orders section 7.3 lists. Stage 0 has no `path` key. | §7.3 |
| P7.5 | RANGE | Text is definite-length UTF-8. Byte fields, unsigned integers, and definite arrays are as section 7.3 states. | §7.3 |
| P7.6 | RANGE | Verify decodes any definite-length encoding of the same values, including a non-shortest unsigned integer. Key order is not a shape `mismatch`. | §7.3 |
| P7.7 | INVARIANT | Malformed CBOR is shape `mismatch`, and `shape_item` includes `malformed cbor`. Later measurements are still recorded. | §7.3 |
| A3 | INVARIANT | The hike object has exactly the top-level fields, types, and lengths in the section 8 table. | §8 Rule A3 |
| P8.1 | INVARIANT | Evidence is the hash-bound set in section 8. Convenience signposts are checked against the evidence or the index and never relied on. | §8, §14 |
| P8.2 | INVARIANT | Stored heights are convenience. `d_g` and `delta_h` subtract index-confirmed heights only. | §8, §14.3, §14.4 |
| A3.1 | INVARIANT | The object stores `seed_beacon` and all 18 interrupts, with strictly later heights and differing hashes, and stores no block time. | §8 Rule A3.1 |
| A3.2 | INVARIANT | Each stage map has exactly the listed fields. Array lengths equal `leg_count`. `leg_count` is at least 1. Stage 0’s `waypoint` is the seed. | §8 Rule A3.2 |
| A3.3.1 | INVARIANT | Stage 0 omits `path`. An empty byte string is not a stage-0 path. “not planted by design” is not a token. | §8 Rule A3.3 |
| A3.3.2 | INVARIANT | `stages[17].path` is always empty. `W_17` is not asked, including on a jump. The record is `not fetched by design`. | §8 Rule A3.3 |
| A3.3.3 | CONSTANT | Sixteen waypoints, `W_1` through `W_16`, are fetchable. `W_17` is not. | §8 Rule A3.3 |
| A3.3.4 | INVARIANT | The object has no terrain field. A stored leg result is `le64` of the section 11 `u64`. | §8 |
| A3.4 | INVARIANT | `path_skip <g>` and `not fetched by design 17` are the exact stderr lines, in that order, after the cut-off. Empty bytes are not a silent skip. | §13.3 Rule A3.4 |
| P9.1 | RANGE | `{tip-url}`, `{index-url}`, and `{bari-url}` are absolute `http` or `https` URLs with no trailing slash. | §9, §13 |
| P9.2 | INVARIANT | A clock read is one GET, does not follow a redirect, and is successful only on status 200. | §9 |
| P9.3 | RANGE | A successful body is one UTF-8 JSON object, with no BOM, no second value, and no duplicate member name. | §9 |
| P9.4 | RANGE | `header.hash` is 64 hex digits in Bitcoin display order. The natural beacon reverses those 32 bytes. | §9 |
| P9.5 | RANGE | `header.height` is a JSON number in `0 … 2^53 − 1`, not a string and not a non-integer JSON number. An index height above that bound is an unsuccessful read: `not read`. An integer above that bound in `beacon_height` or `last_interrupt_height` is section 14 `shape_item` `height_range`. | §9, §14 |
| P9.6 | RANGE | `header.time` is a JSON integer in `0 … 2^32 − 1` Unix seconds, not a string and not a non-integer JSON number. | §9 |
| P9.7 | INVARIANT | On mainnet, `header.time` equals the `nTime` of the block whose hash is `header.hash`. Section 14 measures the portable index. A direct Bitcoin read is outside that procedure. | §9, §14 |
| P9.8 | CONSTANT | With no named gossip peer, the starting list is the copied Bitcoin Core mainnet DNS seeds, in the listed order, port 8333. | §9 |
| P9.9 | RETIRED | Former retired-host rule. This ID is not reused. | §9 |
| P9.10 | INVARIANT | Produce Observe reads the plane tip sequence and does not GET an HTTP tip or the portable index. | §9 |
| P9.11 | INVARIANT | The interrupt is the first successful post-checkpoint Observe whose hash differs and whose height is strictly greater. There is no rebinding. | §9, Rule A1 |
| P9.12 | INVARIANT | An unsuccessful seed Observe, or an unsuccessful post-checkpoint Observe that looks for the interrupt, emits no object. An unsuccessful Hashcash, POST, or path-ask Observe is a miss, and produce continues. | §9 |
| P9.13 | INVARIANT | Verify does not Observe, does not GET an HTTP tip, and does not GET a Bari path. It reads the portable index at each stored height, and it reads one path inclusion height only when that height is in the anchor window and that header is a stored interrupt. | §9, §14 |
| P9.14 | INVARIANT | The seed beacon is the natural beacon from the first successful Observe, before any leg. That Observe is the latest header already on the tip sequence. It does not wait for a header mined after produce starts. | §9 |
| P9.15 | INVARIANT | A height is not an input to `SHA-256` in this construction. | §9 |
| P10.1 | INVARIANT | The seed preimage is `public_key ‖ seed_beacon`, 64 bytes. The seed is `SHA-256` of that preimage, 32 bytes, no label, read as four little-endian lanes. | §10 |
| P10.2 | INVARIANT | `mix64`, the NL4 base word, the parent indices, and the dag2 fill are the section 10 construction. Every terrain word depends on all 256 bits of the seed. Terrain is filled once per terrain. The step does not write it. | §10 |
| P10.3 | INVARIANT | The seed equals `stages[0].waypoint` by design. | §10, §12.1 |
| P11.1 | INVARIANT | A step folds every word of the selected slot by §11.1 (`acc` starts at 0 and grows by wrapping addition; `mix64` includes `MIX_ADD`) and sets the next state from `SHA-256(SEAL_DOMAIN ‖ le64(S_folded))`. | §11.1 |
| P11.2 | INVARIANT | The leg result is `STEPS_PER_LEG` steps from the first eight bytes of the previous checkpoint, stored as `le64`. | §11.2 |
| P11.3 | INVARIANT | The terrain sample is `SAMPLE_BYTES` at the offset taken from the previous checkpoint. It is not the waypoint slot sample. | §11.3 |
| A2.1 | CONSTANT | `seed_beacon` plus the 18 interrupts gives stages numbered 0–17. | §12.1 Rule A2.1 |
| A2.2 | INVARIANT | The seed is `SHA-256(public_key ‖ seed_beacon)`, stored at `stages[0].waypoint`, never posted, and not one of `W_1` … `W_17`. The seed equals that field by design. Leg 0 starts from it or from `W_g`. | §12.1 Rule A2.2 |
| A2.3 | INVARIANT | The checkpoint preimage is the previous checkpoint, the stage limb, the terrain sample, and `le64(leg_result)`, in that order. | §11.4, §12.1 Rule A2.3 |
| A2.4 | INVARIANT | The first Bari post is `W_1`, after `I_0`. Stage 0 has no plant and no path. | §12.1 Rule A2.4 |
| A1.3 | INVARIANT | `W_{g+1}` is `SHA-256` of `I_g`, its slot sample, `stages[g].waypoint`, and the fold of that stage’s checkpoints. `slot_index` is `u64_le` of the first eight bytes of `I_g`, modulo `N_SLOTS`. No waypoint is built from `I_17`. | §12.2, §13 Rule A1.3 |
| A1 | INVARIANT | At each stage boundary the order is legs, then the tip, then the interrupt, then the waypoint bytes, then the post, then the pivot. There is no rebinding. | §13 Rule A1 |
| A1.1 | INVARIANT | Legs in a stage run back to back. The tip is read after each checkpoint. A leg already in progress finishes. | §13 Rule A1.1 |
| A1.2 | INVARIANT | When the tip read shows interrupt `I_g`, leg production in stage `g` stops. | §13 Rule A1.2 |
| A1.4 | INVARIANT | The producer posts `W_{g+1}` to Bari with the Hashcash fee. | §13 Rule A1.4 |
| A1.5 | INVARIANT | `W_{g+1}` is the pivot. Stage `g+1` leg 0 starts from it. | §13 Rule A1.5 |
| P13.1 | INVARIANT | Produce sets no wall-clock bound, no steps-per-second quota, and no leg-count quota. `leg_count ≥ 1` is the drive loop, not an admission floor. | §13, §13.3 |
| P13.2 | CONSTANT | `ANCHOR_WINDOW_OFFSETS` is 2, 3, 4. That list is written once in §13.1. A plant posted at height H is asked only at H plus each offset. The path is gone one header after the last offset. | §13.1 |
| P13.3 | INVARIANT | Produce does not pause the drive before the first leg of a stage. A stage that ends at one leg, when the next header is the interrupt, is a world event the producer recorded. | §13.1 |
| P13.4 | INVARIANT | The Hashcash challenge and digest are the section 13.1 byte strings. Leading zero bits are counted from the first digest byte. | §13.1 |
| P13.5 | CONSTANT | The Hashcash difficulty is 21 leading zero bits, the one value for produce and for Bari. | §13.1 |
| P13.6 | INVARIANT | The nonce search starts at 0. Produce sends one waypoint POST and retries only when the tips differ by a header or the request did not connect. Status 204 means the waypoint was appended. H is the stage beacon height in §13.1. Any other miss does not withhold the object. There is no fee bump. | §13.1 |
| P13.7 | RANGE | The leaf is 64 lowercase hex digits, byte 0 first. Before each due ask, produce waits an integer number of milliseconds drawn uniformly from 15000 through 45000, and both ends are included. | §13.1 |
| P13.8 | INVARIANT | A checked path is a status 200 canonical path body that validates for that waypoint and whose block is one height in the anchor window. Every other ask result is a miss. | §13.1 |
| P13.9 | INVARIANT | The cut-off is `last_interrupt_height`. It selects asks for `W_1` … `W_16`. No ask is made above the cut-off. `W_17` is not asked. | §13.1 |
| P13.10 | INVARIANT | `leg_count` is a convenience copy of `len(checkpoints)`. Produce does not invent path bytes. No metric reads `leg_count`. | §13.3 |
| P13.11 | INVARIANT | The live event log is not the hike object and is not an input to a leg, a checkpoint, a leg result, or a waypoint. | §13.5 |
| P13.12 | RANGE | A public-key input is 32 bytes, or 64 hex digits with an optional leading `0x`. The object stores the 32 bytes. | §13 |
| P13.13 | INVARIANT | A waypoint POST is retried only when the tips differ by a header or the request did not connect. | §13.1 |
| P13.14 | INVARIANT | Plant height H is `stages[g].beacon_height` for `W_g`, written once in §13.1. The Hashcash tip on the POST is not H. | §13.1 |
| P13.15 | INVARIANT | The live event log uses the five record types in section 13.5, in the key order that section lists. | §13.5 |
| P13.16 | INVARIANT | The waypoint POST is `POST {bari-url}/waypoint`. The body is the 32 waypoint bytes. `Content-Type` is `application/octet-stream`. The header is `X-Bari-Hashcash-Nonce`. | §13.1 |
| A4 | INVARIANT | Verify records measurements against the object and the portable index and does not judge. Section 15 is not applied in section 14. | §14 Rule A4 |
| P14.1 | INVARIANT | Anyone may read Bitcoin directly. That read is outside the section 14 procedure. Section 14 measures the portable index (P9.7). P9.7 is not a conformance-vector check. A LAN match is not a mainnet fact. | §14 |
| P14.2 | RANGE | The verify record is the JSON object section 14 defines: required keys, no JSON null, decimal integers, and the stated array lengths. | §14 verify record |
| P14.3 | RANGE | `shape_item` uses only the 14 strings in that table, in that order. Malformed CBOR is exactly `malformed cbor`. `height_range` is an object-byte height above `2^53 − 1`. | §14 verify record |
| A4.1.1 | INVARIANT | `shape` is `match` only when every shape condition holds. Otherwise it is `mismatch`, and later measurements are still recorded. | §14.1 Rule A4.1 |
| A4.1.2 | INVARIANT | `kind`, `fill`, and the size fields equal the profile in force, or `shape_item` includes `constant`. The profile in force is section 6, unless the verifier runs `conformance-small`. | §14.1, §16.1 |
| A4.1.3 | INVARIANT | Shape records the field set, widths, stage-0 `path` key, empty `W_17` path, `stages` length, `leg_count`, `seed_beacon`, and header order. | §14.1 |
| A4.2.1 | INVARIANT | Verify reads every stored header from the portable index, in order, including `last_interrupt`. One header does not skip a later header. | §14.2 Rule A4.2 |
| A4.2.2 | INVARIANT | `index_comparison` is `match`, `mismatch`, or `not read`, from hash and height together. Time is not part of that token. | §14.2 |
| A4.2.3 | INVARIANT | `T` is `header.time` only when `index_comparison` is `match`. On `mismatch`, `T` is `not read`. | §9, §14.2 |
| A4.3.1 | INVARIANT | Verify recomputes each checkpoint digest and the seed and records `match` or `mismatch`. A missing leg result is `mismatch`. This check does not run a leg. | §14.3 Rule A4.3 |
| A4.3.2 | INVARIANT | Verify recomputes `W_1` … `W_17` from stored bytes under section 12.2. No waypoint is recomputed from `last_interrupt`. | §14.3 |
| A4.5.1 | INVARIANT | Empty path bytes on `W_1` … `W_16` are `skip` and plant `n/a`. Any other bytes are `present`. Empty `W_17` is `not fetched by design`. | §14.3 Rule A4.5 |
| A4.5.2 | INVARIANT | Anchored and fetched is the `anchors` token `anchored`: in the section 13.1 window, an interrupt header stored on the object, and plant `match`. `anchored_total` is that count. This row records it and does not judge it. Rule B4, settled and normative, compares it with 6. | §14.3, §14.5 |
| A4.5.3 | INVARIANT | `d_g` subtracts index-confirmed heights only, and only when plant is `match` and the index comparison of `I_{g−1}` is `match`. Otherwise `d_g` is `not defined`. | §4, §14.3 |
| P14.4 | INVARIANT | A present path’s plant is compared with the portable index. A read that does not return a document is `not read`. A document that was read with no plant, or with no matching plant, is `mismatch`. Verify does not GET Bari. | §14.3 |
| P14.5 | INVARIANT | A returned plant document’s `network` is the string `main` or the string `synthetic`. Any other string, or a missing `network`, is `plant` `mismatch`. There is no verifier input that selects one of those two strings. Accepting `synthetic` does not make the measurement a mainnet fact. Two plants may share a block. `tree_size` is not compared. | §14.3 |
| P14.6 | INVARIANT | A present path that does not decode, or whose `validate_for` does not succeed, has `plant` `mismatch` and `claimed_block` `not defined`. `block_height` in a body that decodes is in `0 … 2^32 − 1`, which is inside `0 … 2^53 − 1`. A hash-verified canonical path body has no height above `2^53 − 1`. That is not `shape_item` `height_range`. The hike object still decodes. | §14.3 |
| P14.7 | INVARIANT | An index GET spells the height in the shortest base-10 digits. | §14.2 |
| P14.8 | INVARIANT | A path on `W_1` … `W_16` is `anchored` only when its inclusion height is in the section 13.1 anchor window, that header is an interrupt header stored on the object, and the plant comparison against that one header is `match`. Otherwise the window record is `not anchored`. Verify does not scan other index heights for that path. | §13.1, §14.3 |
| A4.6 | INVARIANT | Bookends are the lowest and highest anchored-and-fetched indices among `W_1` … `W_16`. | §14.4 Rule A4.6 |
| A4.7 | INVARIANT | `N` is the checkpoint count over stages `a` … `b−1`. It does not read `leg_count`. | §14.4 Rule A4.7 |
| A4.8.1 | INVARIANT | `S` is `T(P_b) − T(I_{a−1})` only when both `T` values were taken on `match`. Otherwise `S` is `not read`. | §14.4 Rule A4.8 |
| A4.8.2 | INVARIANT | `delta_h` subtracts index-confirmed heights only. Otherwise `delta_h` is `not defined`. | §14.4 Rule A4.8 |
| A4.9 | INVARIANT | The verify record holds integers `N` and `S` and does not hold a floating-point rate. The bookend display is not a judgment and is not the admission rate. | §14.4 Rule A4.9 |
| A4.10 | INVARIANT | Analysis records the whole-hike count and span, each per-stage span, checkpoints outside the bookends, `hike_legs`, and `hike_span`. The bookend pair is not `hike_legs` or `hike_span`. | §14.4 Rule A4.10 |
| A4.11 | INVARIANT | Fewer than two present paths among `W_1` … `W_16` records the bookend fields `not defined`. An issued plant GET that returns no document records them `not read`, before the anchored count. A plant `not read` whose GET was not issued does not. Fewer than two `anchored` then records `not defined`. With two `anchored`, an unread `I_{a−1}` header or an unread `P_b` header records every bookend field `not read`. A returned `mismatch` does not. `fixed_start` stays an object. | §14.4 Rule A4.11 |
| A4.4.1 | CONSTANT | The six-leg draw count is 6. It is not `STAGES` and it is not `AUDIT_K`. | §14.6 Rule A4.4 |
| A4.4.2 | INVARIANT | The draw is six distinct global checkpoint indices by the unbiased draw in section 14.6. Live verify reads the operating-system CSPRNG. A `conformance-small` vector may supply draw bytes (section 16.1). Fewer than six legs is `not defined`. A short byte source is `not read`. When that draw is `not read`, `six_leg_runs` is `not read`. A short-byte six-leg draw stays re-readable. It is not `not defined`. An 8-byte draw at or above the limit is consumed. | §14.6, §16.1 |
| A4.4.3 | INVARIANT | Each chosen index is recorded `match` or `mismatch` in draw order. | §14.6 |
| A4.4.4 | INVARIANT | The index draw finishes before any leg result is recomputed. A verifier may recompute the six leg results concurrently, and may run Rule A4.3 checkpoint comparisons concurrently. How many Rule A4.3 comparisons run at once is a verifier setting. The verify record does not depend on that number. One at a time is conforming. Elements of `six_leg_runs` may execute in parallel. This file does not require a thread pool and does not set a pool size. One at a time is for this measurement record. It does not apply to `thorough_rounds`. | §14.6 |
| A5 | INVARIANT | An object whose `kind` is `satoshi-hike-v1.35-object` or `satoshi-hike-v2.0-object` is recorded `version` `earlier version`. That record is not a decode failure and is not a shape `mismatch`. Sections 14.1–14.6 are not applied. | §14.7 Rule A5 |
| B9 | INVARIANT | Rule B9 names the section 15.2 refuses that do not wait on a configured threshold: decode, waypoint rehash, shape `mismatch` including `height_range`, seed-value `mismatch`, a confirmed plant or header `mismatch`, a checkpoint-digest `mismatch`, a six-leg `mismatch`, and reason `earlier_version` when section 14 records `version` `earlier version`. A cap hit is Rule B7. This row is a section 15 judgment. It is not a Part A measurement. | §15.2 Rule B9 |

**Sentences reviewed for POLICY THRESHOLD.** None of the rows above is a POLICY THRESHOLD. The sentences a reader might take for one are:

1. `leg_count ≥ 1` (A3.2, A4.1.3, P13.1). It is the section 13.2 loop, recorded under shape. It is not an admission floor. Rule B8 sets no leg-count threshold.
2. Hashcash difficulty 21 (P13.5). It is a produce and Bari constant. It is not an admission threshold.
3. The wait from 15 seconds through 45 seconds (P13.7). It is a produce range. It is not an admission threshold.
4. Section 9, “mean about 60 seconds.” That phrase describes the synth sequencer. It is not a section 6 constant, not a `conformance-small` parameter, and not an admission threshold.
5. Section 13.1, a stage that ends at `leg_count = 1` “is conforming.” That sentence means the producer followed section 13. Judgment of the record is section 15.
6. Rule A4.9, the bookend pair `(N, S)`. Section 14 records the integers. It does not apply `leg_rate_floor`. The admission rate is Rule B2 on `(hike_legs, hike_span)`, not on `(N, S)`.
7. The informative note under Rule A4.11 about the two, three, or four blocks from `I_{b−1}` to `P_b`. Those sentences are not a Part A number. Rule B3 is not an admission input and does not adjust `hike_span`.

No Part A sentence is moved into section 15 by this classification. Where section 14 says an admission service may judge a recorded token, the judgment stays in section 15.

## 2. Scope

**Normative.**

This document specifies Satoshi Hike v2.1.

A hike has exactly eighteen stages (Rule A2.1).

Section 13 produces the object. Section 14 measures it. Section 15 is the admission service. Produce and verify perform no signature check. The admission service checks the signature (section 15.1).

## 3. Interpretation

**Normative.**

“Must” and “must not” apply to a conforming producer, a conforming verifier, and a conforming admission service.

Byte ranges are half-open. `b[i:j]` is bytes `i` through `j-1` of `b`, starting at index 0.

`0^n` is the byte string of `n` zero bytes.

`‖` is raw concatenation of byte strings, in the written order. A concatenation has no length prefix, no tag, and no separator.

`le64(x)` is the 8-byte little-endian encoding of the unsigned integer `x` with `0 ≤ x < 2^64`.

`u64_le(b)` is the unsigned integer represented by the 8-byte string `b` in little-endian order.

`mod` is the non-negative remainder. For `a ≥ 0` and `m > 0`, `a mod m` is the integer `r` with `0 ≤ r < m` and `a = qm + r` for some integer `q`.

`mod 2^64` on a `u64` quantity is ordinary wrapping arithmetic on a 64-bit unsigned integer. The result is the low 64 bits, and overflow is discarded. That is this `mod` with modulus `2^64`. It is the rule for the NL4 word, the parent indices, the slot mix, and the step’s `u64` state. A `mod` with any other modulus stays the remainder above.

`SHA-256` is the function specified by FIPS 180-4. Its output is 32 bytes. In this document, `H` means `SHA-256`.

## 4. Glossary

**Normative.**

| Term | Meaning |
|---|---|
| Satoshi Hike v2.1 | This construction. `KIND` is `satoshi-hike-v2.1-object`. v2.0, as built before this correction, and v1.35 are historical. |
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
| Leg-rate floor | Rule B2 policy parameter, not a measurement. A policy sets its value, in checkpoints per second (`hike_legs` over `hike_span`, with `hike_span` in Unix seconds). This file sets no ratified value. The value is TBD. One checkpoint is one leg. |
| Index-confirmed height | `header.height` from an index document whose comparison is `match` (stored hash agrees and stored height agrees). For `P_b` that comparison is `index_comparison_P_b`: the header hash agrees and `header.height` equals `P_b.block_height`. A stored height alone is not this height. |
| Policy profile | See section 15. Not a hike-object field. |
| Re-verify | See section 15. |
| Marks as fast | See section 15. |
| Friendly list | See section 15. |
| Paths minimum | Rule B4. `anchored_total ≥ 6` among `W_1` … `W_16`, wherever those waypoints sit. Not a measurement. |
| Outcome token | Lowercase, with spaces: `match`, `mismatch`, `present`, `skip`, `not defined`, `not read`, `not fetched by design`, `n/a`, `anchored`, `not anchored`, and `earlier version`. Not `not_defined`. Not title case. There is no measurement token `not obtained`; that wording is retired below. `earlier_version` is not in this set. It is a section 15 reason. `n/a` means a plant comparison does not apply because `path_state` is `skip`. `n/a` is not `not defined`. `not read` is an index document that is missing or unreadable, a header whose comparison is not `match` so `T` is not taken, a `T` that is missing or unusable on a `match`, an unreadable header, or anything short or missing from a network read. Short path bytes for a six-leg run, and a short-byte six-leg draw, are `not read`. A document that was read and has no plant for that block is `plant` `mismatch`. A path body that does not yield `claimed_block` is `not defined`. Fewer than six legs is `not defined`. Those `not defined` values are settled by the object's own bytes. |

The public key is the 32-byte string used below. Produce and verify do not test a signature on it.

**Retired wording.** Use the term in the table.

| Retired | Use instead |
|---|---|
| opening header; the header that opened stage `g`; open header; close header | `seed_beacon` on stage 0; `I_{g−1}` on stage `g ≥ 1` |
| a header arrives during the waypoint post | there is no rebinding; the next interrupt is the later tip check (Rule A1) |
| deciding legs; deciding span; deciding leg rate | `hike_legs`; `hike_span`; the Rule B2 rate. Bookend `N` and `S` are not that rate |
| opening value | seed, or `W_g` |
| seed value | seed |
| production_seed; the 64-byte “seed” | seed preimage |
| fill pin; `fill_pin`; `SEED_DOMAIN`; `PSEED64` | deleted. Do not squeeze the seed to 64 bits |
| stage span | bookend span `S`; a per-stage span is analysis (Rule A4.10) and is not an admission input |
| leg floor | leg-rate floor |
| seed block | `seed_beacon` |
| anchored and included | anchored and fetched |
| planted | anchored and fetched |
| not planted by design; any path field on stage 0 | stage 0 has no path key (Rule A3.3) |
| walk or re-walk, for a leg | leg, or recomputed leg result |
| not obtained, for an unsuccessful index read or a missing `T` | **not read** |
| roll-forward cut | A row with no `header.time` records `T` as `not read`. This file does not name a cut height. |

`SEAL_DOMAIN` `pcoc-walk-v1` is an exempt wire constant. It is not retired wording. The verify-record key `seed_value` and the token `seed-value mismatch` are exempt wire names. Do not rename them.

Part A (sections 8–14) does not say that verify or the object passes, fails, rejects, denies, or admits, and it does not say `accept`, `refuse`, `revoke`, or `held`. Those outcomes are section 15. Do not say “minimum legs per stage.” A per-stage rate is analysis. It is not an admission test.

## 5. Overview

**Informative.**

One individual contributes a public key. The clock contributes a seed beacon and, later, eighteen interrupts. The public key and the seed beacon determine the terrain. The hike then records eighteen stages, numbered 0–17. Stage 0 stores the seed, `seed_beacon`, and one or more legs. It has no waypoint of its own and no path. Stages 1–17 each store a waypoint, the interrupt the stage started from, the path of that waypoint, and one or more legs. The interrupt that ends stage 17 is stored as `last_interrupt`. Each leg is `STEPS_PER_LEG` steps and ends in a leg result and a checkpoint.

```text
public key  +  seed beacon
        │
        ▼
     terrain          (recomputed by a verifier; not stored)
     seed       SHA-256(public key ‖ seed beacon); stored at stages[0].waypoint

hike
  stage 0
    seed, seed beacon, beacon height
    leg 0 … leg (leg_count − 1)
      STEPS_PER_LEG steps → leg result
      checkpoint
  stage 1 … stage 17
    waypoint, interrupt it started from, beacon height, path
    leg 0 … leg (leg_count − 1)
      STEPS_PER_LEG steps → leg result
      checkpoint
  last_interrupt, last_interrupt_height
```

The procedures in the normative sections are the construction.

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
| `KIND` | `satoshi-hike-v2.1-object` |
| `FILL` | `dag2` |
| `MIX_ADD` | `0x9E3779B97F4A7C15` |
| `MIX_MUL1` | `0xBF58476D1CE4E5B9` |
| `MIX_MUL2` | `0x94D049BB133111EB` |
| `WORD_XOR` | `0xC0FFEE` |
| `PARENT_MUL` | `0xD1B54A32D192ED03` |
| `SLOT_MUL` | `0xA24BAED4963EE407` |

There is no `AUDIT_K`. This draw does not use `AUDIT_K = 18` and does not use `AUDIT_K = STAGES`. The six-leg check draws 6 legs. That count is not a constant in this table.

`WORDS_PER_SLOT = SLOT_BYTES / 8`. The divisor is the width of one little-endian `u64`.

`N_SLOTS = TERRAIN_BYTES / SLOT_BYTES`, and the division is exact: `TERRAIN_BYTES = N_SLOTS × SLOT_BYTES`.

`OFFSET_MODULUS = TERRAIN_BYTES − SAMPLE_BYTES + 1`.

The section 6 numbers for `WORDS_PER_SLOT`, `N_SLOTS`, and `OFFSET_MODULUS` are that production evaluation. They are not a second free constant. Under `conformance-small` the same formulas apply to that profile’s `SLOT_BYTES`, `TERRAIN_BYTES`, and `SAMPLE_BYTES`.

`SEAL_DOMAIN` is the twelve ASCII bytes `70 63 6f 63 2d 77 61 6c 6b 2d 76 31`. It is exactly those twelve bytes.

There is no `SEED_DOMAIN`. v2.0’s eight bytes `50 53 45 45 44 36 34 00` (`PSEED64` and a NUL) are deleted. The seed has no domain label. P6.14 is retired and is not reused.

`STAGES` is 18. That count, and the beacon count that goes with it, is a profile constant. It is not a security number. The settled description of this arbitrary profile size is 18 stages and 19 beacons (`seed_beacon` plus `I_0` … `I_17`). The phrase “18 beacons / 17 stages” is the old name of that arbitrary size. It is not a second count. The object stores 18 stage maps, numbered 0–17. v2.1 does not change either count. Admission does not treat 18, 17, or 19 as a threshold.

## 7. Encodings

**Normative.**

### 7.1 Little-endian integers

`le64` and `u64_le` are defined in section 3.

The first eight bytes of a 32-byte string `b` are `b[0:8]`. Those bytes are SHA-256 output order when `b` is a digest: byte 0 is the first byte of the digest. `u64_le` does not reverse a digest a second time.

### 7.2 Hexadecimal

A hexadecimal spelling uses the digits `0-9`, `a-f`, and `A-F`. Letter case does not change the value. The spelling contains no spaces.

A 64-digit spelling denotes 32 bytes. The first two digits are byte 0. Natural order is that spelling: byte 0 first. Bitcoin display order is the one reversal in section 9. A digest is not reversed a second time.

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

Text values and map keys are definite-length UTF-8 text strings (CBOR major type 3). A length from 0 through 23 is the additional information in the first byte. A longer string uses additional information 24, 25, 26, or 27 and the length bytes that follow, then the UTF-8 bytes. Indefinite-length text is not a text string in this object. `KIND` is longer than 23 bytes, and the `kind` value is still one definite-length text string. Byte fields are byte strings (major type 2). Counts and heights are unsigned integers (major type 0). Arrays are definite arrays in the order defined in section 8.

Verification decodes one definite-length CBOR data item and records section 14. These are decoding rules. A definite-length encoding of the section 8 values decodes. A non-shortest definite-length unsigned integer decodes when the numeric value is the same. Map key order is not a shape `mismatch`. Indefinite lengths, tags, floating-point values, simple values, negative integers, a map key that is not a text string, a duplicate key, and any byte after that one data item are a malformed CBOR value. A malformed CBOR value is shape `mismatch`, and `shape_item` includes `malformed cbor`. Later measurements are still recorded. Produce is what fixes the canonical bytes.

### 7.4 Checks

**Informative.**

`le64(1) = 01 00 00 00 00 00 00 00`.

`le64(0x0102030405060708) = 08 07 06 05 04 03 02 01`.

FIPS 180-4’s SHA-256 example for the three bytes `abc` is `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`.

`SHA-256(SEAL_DOMAIN ‖ le64(0))` is the 32-byte digest whose hexadecimal spelling, byte 0 first, is `4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533`. The first eight digest bytes, in that index order, are `48 16 bd af 23 24 04 76`. Their spelling in index order is `4816bdaf23240476`. When `S_folded = 0`, section 11 sets `S' = u64_le(E[0:8])`, the little-endian integer of those eight bytes. That integer is `0x76042423afbd1648`. `E` is that digest. `D` is the seed, not this digest. The spelling `4816bdaf23240476` is the digest bytes in index order. It is not the hexadecimal spelling of the integer `S'`.

A seed preimage of 32 bytes of `0x11` followed by 32 bytes of `0x22` has seed `5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c` (byte 0 first). The four lanes are `P[0] = 0x545dfe297dc78951`, `P[1] = 0x27858669c45e046a`, `P[2] = 0x9cdac73ac1a5fe85`, `P[3] = 0x7c81df6efbf55f11`. The NL4 base word at `i = 0`, `j = 0` is `0xc7582082a632c2c8`. There is no fill pin.

## 8. Hike object

**Normative.**

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

#### Rule A3.1 Headers

The field name `beacon` is kept.

- `stages[0].beacon` = `seed_beacon`, and `stages[0].beacon_height` is that header’s height.
- For `g = 0 … 16`, `I_g` = `stages[g+1].beacon`, and `stages[g+1].beacon_height` is the height of `I_g`.
- `I_17` = `last_interrupt`, and `last_interrupt_height` is its height.

`stages[g].beacon` for `g ≥ 1` is `I_{g−1}`, the interrupt stage `g` started from. It is not a second header read after the waypoint post.

For `0 ≤ g < 17`, `stages[g+1].beacon_height > stages[g].beacon_height`, and `stages[g+1].beacon` differs from `stages[g].beacon`.

`last_interrupt_height > stages[17].beacon_height`, and `last_interrupt` differs from `stages[17].beacon`.

The object therefore carries `seed_beacon` and all 18 interrupts. The object carries no block time. Section 14 reads each block time from the portable index (sections 9 and 14.2).

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

#### Rule A3.3 Paths

Stage 0 has no path. The stage-0 map omits the `path` key. Omission is the encoding. An empty byte string is not a stage-0 path. The words “not planted by design” are not a token in this file.

Stages 1–17 each carry the path of their waypoint `W_g`, or empty bytes for a skip.

`stages[17].path` (`W_17`) is always the empty byte string. `W_17` is not asked, including when a jump would put an offset at or below the cut-off (section 13.1). The record is **not fetched by design**.

Sixteen waypoints, `W_1` … `W_16`, are fetchable. `W_17` is not one of them.

A `path` key on stage 0 is a shape **mismatch** (Rule A4.1). Those bytes are not a path. Verify does not give them path semantics.

The object has no terrain field. A verifier recomputes terrain from `public_key` and `seed_beacon`.

A leg result on the object is `le64` of the `u64` defined in section 11, stored as an 8-byte byte string.

## 9. Clock

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
- Hashcash `tip_hash` is natural order. The challenge does not use display bytes.
- The path-ask leaf is the waypoint’s hex, byte 0 first.

`header.height` is a JSON number in the range `0 … 2^53 − 1`. It is not a JSON string. A JSON number written with a fractional part or an exponent is not a height. That bound is what this file states for a height. A height above `2^53 − 1` that an index document reports makes that read unsuccessful. Record `not read`. Do not clamp the value and do not record the oversized value. An integer above `2^53 − 1` in `beacon_height` or `last_interrupt_height` is recorded in section 14: `shape` `mismatch`, `shape_item` includes `height_range`, and the value is `not defined`. It is not `not read`. `block_height` in a body that decodes is in `0 … 2^32 − 1`, which is inside `0 … 2^53 − 1`. A hash-verified canonical path body has no height above `2^53 − 1`. A body that does not decode, or whose `validate_for` does not succeed, has `plant` `mismatch` and `claimed_block` `not defined`. That is not `shape_item` `height_range` (P14.6). The hike object still decodes. `not read` is only for a read that is not hash-bound or whose hash does not agree.

`header.time` is the Bitcoin header `nTime` of the header at that height. It is a JSON integer in the range `0 … 2^32 − 1`, in Unix seconds. It is not a JSON string. A JSON number written with a fractional part or an exponent is not a time. On the mainnet path, `header.time` equals the `nTime` of the block whose hash is `header.hash`. The portable index is faithful to Bitcoin: header hash, height, and `nTime`. Anyone may bypass the index and read Bitcoin directly (see §14). Section 14 measures the portable index. It does not read Bitcoin as its procedure. `T(x)` reads `header.time` from the portable index for block `x` only when Rule A4.2 records `index_comparison` `match`. Produce does not read `header.time`. Bari’s observed time (`header.timestamp_unix_ms`, when a document carries it) is a different member. Section 14 does not use it as `T`.

A read whose `header` has no `time`, or whose `time` breaks this rule, is still a successful read for `hash` and `height` when those members follow the rules above. `T` for that header is then **not read**. Section 14.2 records that token. A read that is not successful leaves both the index comparison and `T` as **not read**. `T` is taken only when section 14.2 records `index_comparison` `match` for that header. `match` requires the stored hash to agree with the index. A successful read whose hash does not agree is `mismatch`, and `T` is **not read**. Do not take that document’s time. A height disagreement is also `mismatch`, and `T` is **not read** on any `mismatch`. A successful read with a well-formed `time` does not record `T` when the hash does not agree.

The body rules above are the portable index. `header.hash` and `header.height` follow those rules for every header read. `header.time` follows them on a row that carries it. A plant read does not require them. A row with no `header.time` records `T` as `not read`. This file does not name a cut height and does not backfill a row. Produce Observe does not use that GET.

Produce Observe returns `(natural, height)` from the plane's one tip sequence. The default profile is the test plane. Its sequence is the synth sequencer (the synth tip sequence). Wire magic is regtest headers (header network magic for gossip). That magic is not a Bitcoin Core plant and not a tip clock. One cadence, mean about 60 seconds. “Mean about 60 seconds” describes the synth sequencer. It is not a section 6 constant, not a `conformance-small` parameter, and not an admission threshold. The natural beacon is that header's hash in natural byte order. Test Observe is that gossip peer. It does not GET an HTTP tip.

The live profile is explicit. Its sequence is the Bitcoin best chain, the chain with the most work, read by gossip. Live Observe does not GET an HTTP tip.

When no gossip peer is named, the starting list is a copy of Bitcoin Core's current mainnet DNS seeds (`CMainParams::vSeeds` in `src/kernel/chainparams.cpp`). The list is Core's own bootstrap. This specification does not invent a host. The copy is what lets a producer join gossip without naming one peer and without depending on any groundinglab node. A lookup of one seed returns node addresses. The producer connects to those nodes and asks them for more peers. An address that does not answer is dropped. That is how a producer stays independent of any single gossip peer. The copied list is checked again against Bitcoin Core's mainnet chainparams when a producer release is cut.

The copy, in Core's order, is `dnsseed.bluematt.me`, `seed.bitcoin.jonasschnelli.ch`, `seed.btc.petertodd.net`, `seed.bitcoin.sprovoost.nl`, `dnsseed.emzy.de`, `seed.bitcoin.wiz.biz`, `seed.mainnet.achownodes.xyz`. The port is 8333. A named peer is still `host:port` when the producer supplies one. This specification does not name that host or port. The test profile does not use this list. Its Observe remains the synth peer.

Produce does not read `{index-url}`. Produce does not read an HTTP tip, including `{tip-url}/index` and `{tip-url}/index/{height}`.

`Observe` is that plane sequence and returns `(natural, height)`.

The seed beacon is the natural beacon from the first successful Observe, taken at the start of produce, before any leg. It fixes the terrain. It is the header stage 0 starts from. That header is the latest one already on the tip sequence. Produce does not wait for a header mined after produce starts. A header that is minutes old is the seed when no later header has been read. After the terrain fill, stage 0’s legs begin from the seed. There is no Bari post before those legs. A later header is the interrupt `I_0`, read at the Observe after a checkpoint. It does not replace the seed. The historical vector `spec/conformance/v2.0/stage0-start.json` is the withdrawn construction. It is not a v2.1 positive. The stage-0 timing in this paragraph is unchanged: the first tip is observed again after the first leg, stage 0 stores two legs, and the seed height is that first tip’s height.

A stage ends at its interrupt. The interrupt is the first successful Observe, taken after a checkpoint has been recorded, that returns a natural beacon different from the header the stage started from and a height strictly greater than that header’s height. The header stage 0 started from is `seed_beacon`. The header stage `g ≥ 1` started from is `I_{g−1}`. Both the hash condition and the height condition are required. If the tip moved by more than one block before that check, the interrupt is the tip as read. It is not an earlier header the producer did not read. The interrupt is a Bitcoin header interrupting the drive.

The interrupt that ends stage `g`, for `g` from 0 through 16, is stored as `stages[g+1].beacon`. The waypoint `W_{g+1}` is built from that interrupt after it is read (Rule A1.3, section 12.2). The interrupt that ends stage 17 is `last_interrupt`. No second Observe replaces `stages[g].beacon` after the waypoint post. There is no rebinding.

A header newer than `I_g` is the next interrupt candidate. Produce reads it at the Observe after stage `g+1`’s first checkpoint, the same as any other header.

An Observe taken for Hashcash, a waypoint POST, or a path ask does not end a stage and is not `I_g`. After a post, produce returns to the drive. It does not pause the drive to wait for a further header before the first leg, and it does not pause on a wall clock before the first leg.

Heights are stored on stages. A height is not an input to `SHA-256` in this construction.

If the seed Observe, or a post-checkpoint Observe that looks for the interrupt, is not successful, produce emits no object. An Observe for Hashcash, a waypoint POST, or a path ask that does not succeed is a miss. Rule A3.4 records it and produce continues.

Verification does not Observe the tip sequence and does not GET an HTTP tip. Section 14.2 reads `GET {index-url}/index/{height}` under the body rules in this section, once for each stored header: each stage’s `beacon_height`, then `last_interrupt_height`. It records `index_comparison` and `T` for each. That read is part of the header measurement in section 14.2. It is not the only request verify may send: section 14.3 may read the same index for a present path’s plant facts. Verify does not GET a Bari path. The index is that plane's index. The default profile does not fill `https://index.groundinglab.io`.

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

### 10.1 `mix64`

```text
mix64(x):
    y = (x + MIX_ADD) mod 2^64
    y = ((y XOR (y >> 30)) × MIX_MUL1) mod 2^64
    y = ((y XOR (y >> 27)) × MIX_MUL2) mod 2^64
    return y XOR (y >> 31)
```

`>>` is a logical right shift of a `u64`: vacated high bits are zero. It does not sign-extend. `<<` is a left shift of a `u64`. The shift count is not reduced modulo 32. `i << 32` is the slot index multiplied by `2^32` in that `u64`, and for every legal `i` the product fits in 64 bits.

### 10.2 Lanes

The four lanes are `P[0]`, `P[1]`, `P[2]`, and `P[3]` from the seed, as section 10 states. v2.0’s §10.2 fill pin is deleted. Do not compute it. Do not use it.

**Informative.** Conformance does not require recomputing the deleted squeeze. A negative vector may publish the withdrawn terrain digest. An implementation that emits that digest does not conform. This file does not restate the squeeze.

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

## 11. Leg

**Normative.**

A leg is `STEPS_PER_LEG` steps. It starts from the previous checkpoint defined in section 12. The leg result is the `u64` end state of the leg. The checkpoint is the digest below. Every leg in a stage uses the same 32-byte second limb: the seed on stage 0, and waypoint `W_g` on stage `g ≥ 1`.

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

The v2.0 fold `S_folded = S XOR (XOR over w of (word(w) XOR w))` is withdrawn. The parenthesised XOR does not depend on `S`, so one 8-byte value per slot can stand in for the slot. That fold is not conforming. The XOR accumulator `acc = acc XOR mix64(word(w) XOR w XOR S)` is also not conforming. It is the negative `xor_accumulator_fold`. It is not the v2.0 fold. The wrapping-add fold `acc = (acc + mix64(word(w) XOR w XOR S)) mod 2^64` is also not conforming. `word(w) XOR w` does not depend on `S`. It is the negative `xor_w_fold`. The wrapping-add fold `acc = (acc + mix64(word(w) XOR ((S + w) mod 2^64))) mod 2^64` is also not conforming. It is the negative `s_plus_w_fold`. At `S = 0` those two folds match each other, because `(S + w)` equals `w`, and both differ from this section: this section mixes `word(w) XOR S`.

**Normative.** Christian's requirement, in his words:

> a deliberately inefficient algorithm on the hash principle in reverse; a hash can't rehydrate the 16 KB, so each step must be driven by the full 16 KB slot, and no digest, lookup table or smaller data can stand in for it. No efficiencies, caching, tables or partial reads anywhere in hike/fold/terrain. A producer speed-up is a suspected break: flag it, never merge it.

**Normative.** A conforming producer and a conforming verifier, on every step of every leg, load all `SLOT_BYTES` bytes of the selected slot from the terrain of this PCOC and mix each of the `WORDS_PER_SLOT` words with the current state `S` by the §11.1 loop, including `mix64` with `MIX_ADD`. The load is `SLOT_BYTES` bytes per step. A value computed on an earlier step does not satisfy the load on this step. A table, a digest, a per-slot summary, or any value shorter than the slot does not satisfy the load. Terrain is filled once for this PCOC from this seed, by §10.4, before the first leg, and is not shared with another PCOC and not reused from an earlier PCOC. The same rule applies to a gate recomputing a leg: one terrain for that check, then one full slot load mixed with `S` on every step of every recomputed leg. A producer or a verifier that emits the same leg, checkpoint, or waypoint bytes by a shorter load is non-conforming. A producer speed-up is a suspected break: flag it, and do not merge it. One terrain fill for this PCOC, shared by every leg of that run, is required. It is not a cache of an earlier step and it is not a cache shared with another PCOC. Running the legs of one round concurrently is not an efficiency. Each step still reads the full slot and mixes every word with `S`.

`SEAL_DOMAIN ‖ le64(S_folded)` is 20 bytes. That is one SHA-256 block.

**Informative.** The baseline that sets the floor and the gate policy is commodity hardware with SHA-256 instructions, x86 SHA-NI or Armv8 SHA2, of the commodity laptop with SHA instructions (for example an AMD Ryzen 5 5600H).

**Normative.** Where the host provides SHA-256 instructions (x86 SHA-NI or Armv8 SHA2), an implementation uses those instructions for every SHA-256 in this construction: the seed, the seal, the checkpoint, the waypoint fold, and the waypoint. A software SHA-256 is permitted only where the host has no such instructions. This requirement does not change a byte. A vector match does not excuse a software SHA-256 on a host that has the instructions. Hardware SHA-256 and software SHA-256 give identical bytes. A vector does not test which one ran. The rule is there so a measured rate is the rate of the baseline host.

`E[0:8]` is the first eight bytes of the seal digest `E` in index order, byte 0 first. `S' = u64_le(E[0:8])` is the little-endian integer of those bytes. It is not the integer whose big-endian hexadecimal spelling is the same eight bytes. `S` and `S'` are 64-bit unsigned integers under the wrapping rule in section 3. The step does not keep a wider integer. Section 7.4 records the `S_folded = 0` check: those eight digest bytes spell `4816bdaf23240476` in index order, and the integer `S'` is `0x76042423afbd1648`.

The step reads terrain and does not write it.

### 11.2 Leg result

```text
S_0 = u64_le(previous_checkpoint[0:8])
S_{t+1} = Step(S_t)    for t = 0 … STEPS_PER_LEG − 1
leg_result = S_{STEPS_PER_LEG}
```

`leg_result` is a `u64`. On the hike object it is the byte string `le64(leg_result)`.

### 11.3 Terrain sample

The terrain sample at a previous checkpoint is a contiguous window of `SAMPLE_BYTES` bytes. It is not aligned to a slot boundary.

```text
offset = u64_le(previous_checkpoint[0:8]) mod OFFSET_MODULUS
terrain_sample = terrain[offset : offset + SAMPLE_BYTES]
```

The offset is taken from the first eight bytes of the previous checkpoint. The window itself is the sample.

This window is not the waypoint’s slot sample. The slot sample is defined in section 12.

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

## 12. Stage and waypoint

**Normative.**

Stages are `stages[0]` through `stages[17]`.

A stage contains one or more legs. The count is how many legs the drive in section 13 recorded before interrupt `I_g`. Every leg in stage 0 uses the seed as its second checkpoint limb. Every leg in stage `g ≥ 1` uses waypoint `W_g`.

### 12.1 Previous checkpoint

#### Rule A2. Stages and where leg 0 starts

##### Rule A2.1 Eighteen stages

`seed_beacon` plus the 18 interrupts gives 18 stages, numbered 0–17.

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

##### Rule A2.3 Checkpoint

The checkpoint preimage is section 11.4. The stage limb is the seed on stage 0 and `W_g` on stage `g ≥ 1`. Leg 0’s previous checkpoint is that same value. Leg `j ≥ 1` uses `checkpoints[j − 1]` of that stage.

##### Rule A2.4 First post

The first Bari post is `W_1`, after `I_0`. Stage 0 has no plant and no path.

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

### 12.3 What a stage stores

When stage `g` ends at `I_g`, the object records the header the stage started from (`beacon` and `beacon_height`), the seed or the waypoint the stage’s legs used, `leg_count`, `checkpoints`, and `leg_results`.

For `g` from 0 through 16, `I_g` is stored as `stages[g+1].beacon` and `stages[g+1].beacon_height`. `I_17` is stored as `last_interrupt` and `last_interrupt_height`.

Stage 0 has no plant and no path key. No ask is made for stage 0.

The path ask window is section 13.1. No ask is made above the cut-off. A waypoint `W_1` … `W_16` with no checked path stores empty bytes and produce writes `path_skip <g>` (Rule A3.4). `W_17` stores empty bytes and produce writes `not fetched by design 17`.

## 13. Produce

**Normative.**

Inputs are the 32-byte public key and one plane profile: the tip sequence and `{bari-url}`. The default profile is the test plane. The live profile is explicit. A missing parameter is not filled from the other plane.

`{bari-url}` is an absolute URL with scheme `http` or `https` (ASCII case-insensitive) and no trailing slash, the same rule section 9 states for a URL. Produce does not read `{index-url}`.

A public-key input may be those 32 bytes, or a hexadecimal spelling of them under section 7.2 with an optional leading `0x` and exactly 64 digits. The object stores the 32 bytes.

Output is one hike object in the encoding of section 7.3, or no object if the drive’s clock read does not succeed. A waypoint POST that does not succeed, and an ask that does not complete, do not withhold the object. Rule A3.4 stores empty path bytes and produce continues. On the live profile, produce also appends the event log in section 13.5 while the drive runs. That log is not the hike object. Only the live profile writes it.

### Rule A1. Event order at every stage boundary

Produce is a continuous drive from the seed beacon through `STAGES`. A Bitcoin header interrupts that drive. The interrupt is a world event, unknown in time and value. Each stage boundary is one event, in this order. There is no rebinding. A later header is the next interrupt, read after the next stage’s next checkpoint. Never say a header arrives during the waypoint post.

#### Rule A1.1 Legs, then the tip

Stage `g`’s legs run back to back. After each checkpoint the producer reads the tip. A leg already in progress always finishes.

#### Rule A1.2 The interrupt ends the stage

When that read shows a new interrupt `I_g`, leg production in stage `g` stops.

#### Rule A1.3 Waypoint byte order

The producer builds `W_{g+1}` by section 12.2:

```text
W_{g+1} = SHA-256(I_g ‖ slot_sample(I_g) ‖ stages[g].waypoint ‖ fold_g)
```

The third limb is the seed on `g = 0` and `W_g` on `g ≥ 1`. The seed is not called `W_g`.

#### Rule A1.4 Post

The producer posts `W_{g+1}` to Bari with the Hashcash fee (section 13.1).

#### Rule A1.5 Pivot

`W_{g+1}` is the pivot. Stage `g+1`’s leg 0 starts from it.

Stage 0 has no boundary before it. After the seed Observe and the terrain fill, produce enters stage 0’s legs from the seed. There is no Bari post. The first Bari post is `W_1`, after `I_0`. The interrupt that ends stage 17 is `last_interrupt`. Produce stores it and builds no waypoint from it.

Between interrupts the drive runs legs of `STEPS_PER_LEG` steps and records a checkpoint after each leg. The checkpoint binds the leg result, the seed or the stage waypoint, the previous checkpoint, and the terrain sample. That record is what the drive did. `leg_count` is how many of those legs were recorded before the interrupt. Several legs in one stage are the consequence of an uninterrupted stretch. This specification does not bound the wall-clock time of a leg. It does not set a steps-per-second quota. It does not set a leg quota. Principle 9: there is no minimum or maximum leg count. `leg_count ≥ 1` follows from Rule A1.1, because the repeat in section 13.2 runs at least once, and section 14.1 records it under shape.

`Observe` performs the read in section 9 and returns `(natural, height)`.

### 13.1 Stage path

For a stage `g ≥ 1`, when its waypoint `W_g` is known, and before any leg of that stage, produce posts that waypoint. It is the pivot built after `I_{g−1}`. The seed is never posted. Stage 0 has no plant and no path ask.

**Anchor window.** This is the only list of offsets.

```text
ANCHOR_WINDOW_OFFSETS = 2, 3, 4
```

A waypoint posted at height H is asked after each header whose height is H plus one offset in that list, in list order. If the path is served, it is stored and later headers in the window are not asked. If it is not served, the producer asks at the next offset, and then at the last offset. After the last offset a miss stores an empty path and the stage does not keep waiting. Nothing is asked before the first offset or after the last. One header after the last offset, the path is gone. The ask is not made before the post. Test and live use this one rule. An operator setting does not change the list.

For `W_g` with `g` from 1 through 16, H is `stages[g].beacon_height`. That is the height of the interrupt header the object already stores for that stage: `I_{g−1}` (`stages[g].beacon`). The producer asks at each H plus one offset. The verifier uses that same H. The Hashcash tip on the POST is not H. The object does not store a second post height. One function in the reference, `anchor_window_origin`, returns this H. Replacing that function is how a later choice would use a stored post height instead.

The path carries the height of the block that includes the waypoint. Verify records `anchored` only when that inclusion height is in the window, that header is an interrupt stored on the object, and the plant comparison against that one header is `match`. Otherwise it records `not anchored`. The stored interrupt headers are `stages[1].beacon` through `stages[17].beacon` and `last_interrupt` (`I_17`). The interrupt-header check is the same height and the same 32 natural-order bytes (section 9): the path body’s `block_hash` compared with that stored beacon, with no reversal. Verify reads `GET {index-url}/index/{body.block_height}` only when the height is in the window and that header is one of those stored interrupts. It does not read that height otherwise, and it does not read any other index height for that waypoint. A missing path, a path outside the window, or a path whose inclusion header is not one of those stored interrupt headers is `not anchored`. A path the producer stores can still be `not anchored` when its inclusion block is inside the window and is not one of the stored interrupt headers. A path included in `I_17` is inside the window for `W_16` in the expected one-header steps, and it is `anchored` when the plant comparison against that header is `match`.

The live tip for Hashcash is one Observe of that same tip sequence, taken at this moment. Its natural beacon is `tip_hash`. This Observe is not the interrupt `I_g`, and it does not end a stage. If it does not succeed, the POST does not succeed. Produce continues. The miss is kept until the stderr step after the cut-off. No `path_skip` line is written at this moment. Hashcash is that Observe. It is not a second read and it is not an HTTP tip.

After the post, produce returns to the drive. It must not pause the drive to wait for further headers before the first leg. It must not pause on a wall clock before the first leg. There is no one-minute stagger. A header newer than `I_{g−1}` is read at the Observe after this stage’s first checkpoint. If it meets the interrupt rule of section 9, it ends this stage at `leg_count = 1`. A stage that ends at one leg is the interrupt the producer recorded. Waiting for such a header before the first leg is the non-conforming pause.

Hashcash is the rule in `services/bari/src/hashcash.rs`. Produce does not use a second puzzle.

The challenge is `SHA-256` of the ten bytes `bari-hc-v1` followed by `tip_hash`. A nonce is a `u64`. Its digest is `SHA-256(challenge ‖ waypoint ‖ le64(nonce))`. Leading zero bits are counted from the first digest byte. Inside a byte the high bit is first. A zero byte contributes eight bits and the count continues. The first non-zero byte contributes that byte’s leading zeros and the count stops.

The difficulty is 21 leading zero bits. That is the one Hashcash difficulty value in this specification. Produce and Bari both use it. Two environment variables that can disagree are not allowed.

The nonce is the first `u64` in the order 0, 1, 2, … whose digest meets the difficulty. If that search exhausts every `u64`, the POST does not succeed. Produce continues. The miss is kept until the stderr step after the cut-off. No `path_skip` line is written at this moment. The request header is `X-Bari-Hashcash-Nonce`. Its value is that nonce in decimal.

Produce sends one `POST {bari-url}/waypoint` and does not follow a redirect. The body is the 32 waypoint bytes. `Content-Type` is `application/octet-stream`. The request carries the Hashcash header. Status 204 means the waypoint was appended. If Bari’s tip and the producer’s tip differ by a header, or the request did not connect, produce tries again. If the POST does not succeed, including when a status is not 204 and is not one of those two retries, and including when those retries end, produce continues. The miss is kept until the stderr step after the cut-off (Rule A3.4). No `path_skip` line is written at this moment. Produce does not send a second POST for that stage, except to try again in those two cases. There is no fee bump, RBF, replacement, or repair of a missed plant. A missed POST does not stop the hike and does not withhold the object.

The tips differ by a header only when the response status is 400 and the body is exactly the ASCII bytes `Hashcash proof invalid for live tip challenge` and no other bytes. Produce then Observes once. If that Observe succeeds, produce builds a new challenge from that natural beacon and searches for a nonce from 0. If that Observe does not succeed, the POST does not succeed and produce sends no further POST for that stage. The request did not connect only when no HTTP status line was received. Produce may send the same nonce again in that case. HTTP 5xx, HTTP 408, HTTP 429, a timeout after a status line, and HTTP 400 with any other body are not retries. The nonce header value is the shortest decimal digits of the `u64`, with no sign, no leading zeros, and no surrounding space. Zero is `0`.

The POST that returned 204 is the plant. H stays the stage beacon height defined above. The Hashcash tip on that POST is not H.

The serve window is the anchor window above. A plant posted at height H can be included at a height in that window. The path is served through the last offset. It is gone one header after that offset.

On the live profile, Bari's plant is a Bitcoin transaction. The anchor is the block that includes that transaction. Bari serves the waypoint path for the anchor window. The producer posts the waypoint to Bari and asks Bari for the path. This specification does not define a path that anchors by talking to Bitcoin and skipping Bari.

The leaf is the waypoint’s 64 lowercase hexadecimal digits, byte 0 first. An ask sends `GET {bari-url}/path/{leaf}` and does not follow a redirect. After each ask header in the anchor window, the producer waits an integer number of milliseconds drawn uniformly from 15000 through 45000, and both ends are included, before the GET, so the producers do not ask Bari at the same moment. Produce must make the ask at every due header among `W_1` … `W_16`. Watching only the newest tip is not enough. `W_17` is not asked.

- Status 200: a checked path is a 200 response whose bytes are a path for that waypoint. The bytes are a path for that waypoint when they decode as a canonical path body as defined in section 14.3 and its `validate_for` succeeds for that waypoint, including the Merkle checks in section 14.3. Produce checks that before it stores the bytes. A 200 that does not check is a miss. Produce does not require a `Content-Type`. The path’s block must be one height in the anchor window for the stage that posted at H. It does not have to be the same header as `I_{g−1}`. A path the producer stores can still be `not anchored` when that block is inside the window and is not one of the stored interrupt headers.
- Status 404 names which of these it is: still in the pool, sealed but not in a block yet, not included, already archived after it was served, or unknown. Any of those is a miss. Produce makes the ask at every header where it is due.
- Any other status, or an ask that does not complete, is a miss. Produce continues.

An ask that does not complete, or returns anything other than a checked 200, is a miss. After the last ask due at or below the cut-off, a waypoint `W_1` … `W_16` with no checked path stores empty bytes. The `path_skip <g>` line is written in the stderr step after the cut-off (Rule A3.4), not when the ask returns. Produce must make the ask at every due header. A miss does not stop the hike and does not withhold the object.

There is no silent skip. A silent skip stores path bytes that were not fetched and leaves no record. That is forbidden. “No silent skip” is not a reason to pause the drive for further headers. That pause does not satisfy it. Section 13.3 separates that rule from `path_skip`.

The `path` bytes are not an input to section 11.4 or section 12.2.

**Cut-off.** The cut-off is `last_interrupt_height`, the height of `I_17`. An ask for `W_1` … `W_16` due at a height at or below the cut-off is made under the rule above, including an ask due at the cut-off header itself, after its wait of 15000 through 45000 milliseconds. An ask due at a height above the cut-off is not made. `W_17` is not asked. A jump that puts `H+2`, `H+3`, or `H+4` for `W_17` at or below the cut-off does not create an ask, and produce does not send `GET {bari-url}/path/{leaf}` for `W_17`. Produce stores empty path bytes for stage 17. Produce does not wait for a header above the cut-off. When every ask for `W_1` … `W_16` due at or below the cut-off has happened, produce encodes the object.

**Informative.** Plants are `W_1` … `W_17`. There is no plant for the seed. When each interrupt is one header above the last, and the plant for `W_g` (`g ≥ 1`) is posted at the height of `I_{g−1}`:

- `W_14` is the last waypoint whose full anchor window is at or below the cut-off.
- `W_15` is asked at the first two offsets. If both miss, produce stores empty path bytes and writes `path_skip 15`.
- `W_16`, built after `I_15`, is asked once, at the first offset, which is the cut-off header. It is the last waypoint asked. A miss writes `path_skip 16`.
- `W_17` is not asked. Produce writes `not fetched by design 17` and stores empty path bytes.

Rule A3.3: sixteen waypoints, `W_1` … `W_16`, are fetchable. `W_17` is not. When heights jump or a plant is posted above that height, the cut-off rule determines which asks for `W_1` … `W_16` are made. The indices above are the expected one-header case, not a second rule, and they do not add an ask for `W_17`. Under this expected case the last bookend is usually `W_14`, `W_15`, or `W_16` (Rule A4.6).

### 13.2 Procedure

```text
(seed_beacon, height_0) ← Observe()
seed_preimage ← public_key ‖ seed_beacon        # 64 bytes, section 10; no label
seed ← SHA-256(seed_preimage)                   # stages[0].waypoint; equals the seed by design
terrain ← section 10.4                          # NL4, once per terrain; not per leg

waypoint ← seed
(beacon, beacon_height) ← (seed_beacon, height_0)

for g in 0 … 17:
    if g ≥ 1:
        post waypoint                # W_g; section 13.1; Hashcash; does not end a stage
    previous ← waypoint              # leg 0: seed on stage 0, W_g on stage g ≥ 1
    checkpoints ← []
    leg_results ← []

    repeat:
        leg_result ← LegResult(terrain, previous)     # section 11.2, STEPS_PER_LEG steps
        sample ← terrain sample at previous           # section 11.3, SAMPLE_BYTES
        checkpoint ← SHA-256(
            previous ‖ waypoint ‖ sample ‖ le64(leg_result))
        append checkpoint to checkpoints
        append le64(leg_result) to leg_results
        previous ← checkpoint
        (tip, tip_height) ← Observe()                 # Rule A1.1; after the checkpoint
        if tip ≠ beacon and tip_height > beacon_height:
            (interrupt, interrupt_height) ← (tip, tip_height)    # I_g; Rule A1.2
            stop the repeat

    record stages[g] with
        beacon, beacon_height, waypoint,
        leg_count = len(checkpoints),
        checkpoints, leg_results
        # stage 0: no path key

    if g < 17:
        waypoint ← SHA-256(interrupt ‖ slot_sample(interrupt) ‖ stages[g].waypoint ‖ fold(checkpoints))
                                                      # Rule A1.3; the pivot W_{g+1}
        (beacon, beacon_height) ← (interrupt, interrupt_height)  # I_g stored as stages[g+1].beacon
    else:
        (last_interrupt, last_interrupt_height) ← (interrupt, interrupt_height)
                                                      # I_17; no waypoint is built

cut_off ← last_interrupt_height
make every path ask for W_1 … W_16 due at a height ≤ cut_off that has not happened yet   # section 13.1; W_17 is not asked
for g in 1 … 16:
    stages[g].path ← stage g's checked path body, or empty bytes
stages[17].path ← empty bytes                         # not asked, including when a jump would put an offset at or below the cut-off
                                                      # final stderr step: path_skip <g> ascending g, then not fetched by design 17
encode the object                                     # section 7.3; stage 0 has no path key
```

The repeat body always runs at least once, so `leg_count ≥ 1`. Each iteration is one leg of the drive, exactly `STEPS_PER_LEG` steps, then one checkpoint record. The checkpoint preimage is the previous checkpoint, the seed or the stage waypoint, the terrain sample at that previous checkpoint (`SAMPLE_BYTES`), and `le64(leg_result)`, as section 11.4 defines. Observe runs after the checkpoint is recorded, not in the middle of a leg. That Observe is how the drive notices the interrupt. The section 13.1 Hashcash Observe is not an interrupt read.

Stage 0 starts with no post. Its legs begin after the terrain fill, from the seed. The interrupt comes first. The waypoint of the next stage is built from it and posted after it. That waypoint is the pivot. `I_g` is stored as the next stage’s `beacon`. It is not replaced.

Stage 0 has no plant and no ask. For `W_1` … `W_16`, the ask window is section 13.1. Those asks happen during whichever stage the drive is in by then, or at the cut-off header. No ask is made above the cut-off. `W_17` is not asked. An ask does not end a stage. A miss does not stop the hike.

When stage 17’s repeat stops at `I_17`, produce records stage 17, stores `last_interrupt` and its height, and builds no waypoint. It makes the asks due at or below the cut-off and then encodes the object. It does not wait for a header above the cut-off.

### 13.3 Drive, path_skip, and not fetched by design

**Normative.**

`leg_count` is how many legs the section 13.2 drive recorded before the interrupt. The count is that stretch. It is a convenience copy of `len(checkpoints)`. Verify records it under shape: `shape_item` includes `leg_count` when it differs from `len(checkpoints)` (Rule A4.1). No metric reads `leg_count`. Section 14.1 records `leg_count ≥ 1` as shape, because the repeat runs at least once. Principle 9: there is no minimum or maximum leg count beyond that. Produce does not enforce a rate.

A stage-loop test rig checks one production stage of this drive. A result there checks that one stage. It does not check two headers.

Produce does not invent path bytes. A checked 200 is stored. Anything else is a miss, and the miss is recorded. That is “no silent skip”. A miss does not stop the hike.

If the POST for `W_g` does not succeed, no path ask is due for that waypoint, because there is no plant height. Produce continues. The miss is kept for the final stderr step. If the POST returns 204, produce makes every ask that is due at or below the cut-off. After the cut-off, produce writes every stderr line in one step: `path_skip <g>` for each empty path among `W_1` … `W_16`, in ascending `g`, then `not fetched by design 17`. No stderr line for those paths is written before that step.

#### Rule A3.4 Produce stderr

The stderr line for an empty path on `W_g`, with `g` in `1 … 16`, is exactly one line:

```text
path_skip <g>
```

`<g>` is the decimal waypoint index with no leading zeros. It is the same `g` as in `W_g` and `stages[g]`. The line is the ASCII bytes of `path_skip`, one ASCII space (U+0020), the decimal digits of `g`, and one LF (U+000A). No carriage return and no second space. This includes a POST that does not succeed, a miss on an ask due at or below the cut-off, and a cut-off miss. Produce stores empty path bytes and continues. Empty bytes are not a canonical path body. `path_skip` is not a silent skip. It does not authorize pausing the drive to wait for further headers. Test and live use this one line. No other spelling is this record.

That final step writes the lines in ascending `g`. Within that step, a line for a smaller `g` is not written after a line for a larger `g`. `not fetched by design 17` is last.

The stderr line for `W_17` is exactly the ASCII bytes of `not fetched by design`, one ASCII space (U+0020), `17`, and one LF (U+000A):

```text
not fetched by design 17
```

That line is the last line of the final stderr step. It is not `path_skip`. `W_17` is never asked, including when a jump would put `H+2`, `H+3`, or `H+4` at or below the cut-off. Produce does not send `GET {bari-url}/path/{leaf}` for `W_17`. Produce stores empty path bytes for stage 17 and continues to encode the object. It does not authorize waiting for a header above the cut-off. Produce does not write `not fetched by design` for `W_15`, `W_16`, or any other waypoint. Produce writes nothing on stderr for stage 0. Stage 0 has no path.

### 13.4 Bari restart

**Informative.**

Replacing a live Bari during a running produce is normal. A waypoint already in the pool is retained across a quick restart. A skip is the worst case, not a fee bump.

### 13.5 Live event log

**Normative.**

The live profile writes one append-only event log while produce runs, so a reader can see progress before the hike object exists. Only the live profile writes this log. The log is not the hike object, it is not a field of that object, and it is not an input to a leg, a checkpoint, a leg result, or a waypoint. The producer does not write the log from inside a leg. It does not write one line per checkpoint. It does not write one line per leg result. It does not write the log on the path that places the hike object.

The file path is the hike object path with the suffix `.events`. At the start of a live run the producer creates that file, replacing a file already at that path, and then only appends. Each record is one compact JSON object with no insignificant whitespace, keys in the order listed below for that record, then one newline (U+000A). The producer appends the record after the work that record names, and those bytes are visible to a reader before produce continues. The hike object is still written only when the hike returns. If the log is not created or a record is not appended, the hike, the path ask, and `path_skip` are unchanged.

A `tip` in this log is the shared public tick: the natural hash and the height. It is not a count of confirmations, not a fee, and not an archive. This section does not add a wait for a tip.

The records are:

1. **Block.** After the live producer reads a new header from the tip sequence. A header it ignores is not a record. `event` is `hike.produce.block`. `height` is that header’s height. `tip` is 64 lowercase hexadecimal digits, byte 0 first, the natural hash.

2. **Waypoint.** When the live producer takes waypoint `W_g` for stage `g ≥ 1` to post, after that waypoint is known and before the POST. There is no waypoint record for stage 0. The seed is not posted. `W_g` is the pivot from section 12.2. The record is not written inside that calculation. `event` is `hike.produce.waypoint`. `stage` is `g`, from 1 through 17. `waypoint` is 64 lowercase hexadecimal digits, byte 0 first.

3. **Posted.** After that stage’s waypoint POST has settled, once. A retry is not its own record. `event` is `hike.produce.posted`. `stage` and `waypoint` are the same as the waypoint record. `posted` is true when Bari answered the POST, and false when it did not. `succeeded` is true only when that post returned status 204. Status 204 means the waypoint was appended. `status` is that HTTP status when Bari answered, and is omitted when it did not. The waypoint is on this record. A missed plant is not fee-bumped, replaced, or repaired.

4. **Checked.** After a status 200 body is checked under section 13.1, and only then. A miss is not this check. `event` is `hike.produce.checked`. `waypoint` is the asked waypoint, 64 lowercase hexadecimal digits, byte 0 first. `checked` is true only when the body is a path for that waypoint and the path’s block is one height in the anchor window. Otherwise `checked` is false.

5. **Fetch.** After `GET {bari-url}/path/{leaf}` has returned and the body has been read. The record is not written when the request is sent. `event` is `hike.produce.fetch`. `waypoint` is the asked waypoint, 64 lowercase hexadecimal digits, byte 0 first. `worked` is true only when this fetch is the successful fetch section 13.1 stores. `status` is the HTTP status when the producer has one, and is omitted when it does not. `block` is the path body’s block hash, 64 lowercase hexadecimal digits, byte 0 first, natural order, not Bitcoin display order, and `block_height` is that block’s height. Those two keys are present only when the body names that block. `miss` is present only when the body is one of these phrases: `still in the pool`, `sealed but not in a block yet`, `not included`, `already archived after it was served`, `unknown`. A miss stays a miss. `not included` is not a successful fetch. Stderr `path_skip` remains the record that an empty path was stored. This fetch record does not replace it.

Key order, omitting a key this section says is omitted:

1. Block: `event`, `height`, `tip`.
2. Waypoint: `event`, `stage`, `waypoint`.
3. Posted: `event`, `stage`, `waypoint`, `posted`, `succeeded`, then `status` only when Bari answered.
4. Checked: `event`, `waypoint`, `checked`.
5. Fetch: `event`, `waypoint`, `worked`, then `status` only when the producer has one, then `block` and `block_height` only when the body names that block, then `miss` only when the body is exactly one of the five phrases and no other bytes.

The log does not choose a Bari URL. `{bari-url}` stays the URL section 13 already uses. On the live profile the reference implementation's built-in default stays `https://bari.groundinglab.io`, an example deployment, not a requirement; a producer may name any Bari operator's https origin. This section does not define a path that skips Bari.

## 14. Verify

**Normative.**

### Rule A4. What verify measures against the portable index

Input is one hike object, the portable index (`{index-url}`, or the documents those reads return), and the profile in force. A `conformance-small` run also takes the four integers in section 16.1. A rig may supply the Rule A4.4 draw bytes. There is no network-mode argument. The plant comparison accepts a returned document whose `network` is `main` or `synthetic` (P14.5).

The profile in force is the section 6 constants, unless the verifier is running `conformance-small`. That run may differ only in `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG`. The profile name and those four integers are verifier inputs. The hike object gains no new field. The verify record must state the profile it ran under, and, when that profile is `conformance-small`, `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG`. That statement is measurement provenance, not a producer claim. Injected draw bytes are rig-only, for a deterministic Rule A4.4 vector. They are never a production input. Production verify reads the operating-system CSPRNG.

Section 14 records measurements against the object and the portable index. It does not judge them. It does not read Bitcoin as its procedure. Outcome tokens are the section 4 set: `match`, `mismatch`, `present`, `skip`, `not defined`, `not read`, `not fetched by design`, `n/a`, `anchored`, `not anchored`, and `earlier version`. There is no yes/no token. This section does not pass, fail, reject, deny, or admit.

**Evidence and convenience.** Evidence, the hash-bound values, is the seed preimage (`public_key`, `seed_beacon`), the checkpoints, the leg results, the waypoints `W_1` … `W_17`, the Bitcoin headers (`stages[g].beacon` for `g ≥ 1`, and `last_interrupt`), the path’s leaf, Merkle path and root, which bind the path to the waypoint, `stages[0].waypoint` (the seed, `SHA-256(public_key ‖ seed_beacon)`), and any calculated field whose hash dependencies enforce order and cost. Convenience signposts are the path locators (`txid`, `vout`, `block_hash`, `block_height`), checked against the index, `leg_count`, `beacon_height`, `last_interrupt_height`, the section 6 constants, and `stages[0].beacon` (= `seed_beacon`). Verify records `leg_count` under shape: `shape_item` includes `leg_count` when it differs from `len(checkpoints)` (Rule A4.1). Section 6 constants are recorded as `constant`. Stored heights are recorded as `index_comparison` (Rule A4.2). They are convenience. `d_g` and `delta_h` do not subtract a stored height unless the index comparison that confirms it is `match` (Rules A4.5 and A4.8). Verify checks a convenience signpost against the evidence or the index and never relies on it. A broken signpost is a recorded `mismatch`. Section 15 is where an admission service may judge that recorded mismatch. `N`, `S`, `delta_h`, `d_g`, `a`, `b`, spans, and rates are computed by verify from that evidence and the portable index, are never producer claims, and are fields of the verify record.

The measurements are recorded in section order: Rule A4.1 (section 14.1), Rule A4.2 (section 14.2), Rule A4.3 then Rule A4.5 (section 14.3), Rules A4.6 through A4.11 (section 14.4), Rule A4.4 (section 14.6). Later measurements are still recorded. Section 15 is not applied inside this section.

Verification does not GET an HTTP tip. Verification does not GET a Bari path. Verification does not Observe, so it cannot recover an unstored header by assuming the next height.

**Informative.** By design, any verifier may instead read a Bitcoin node directly (block headers with nTime, and the waypoint plants' OP_RETURNs) and build its own index. This specification does not require it.

#### Verify record

**Normative.** The verify record is one JSON object. It is not the hike object. The hike object gains no new field. The profile name and, under `conformance-small`, the four integers `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG` are verifier inputs. They appear on this record as measurement provenance. `profile` is always on the record: `conformance-small`, or `production` for a run under the section 6 constants. The profile is not absent from the record. When `profile` is `conformance-small`, the record also has `sample_bytes`, `slot_bytes`, `terrain_bytes`, and `steps_per_leg`. Those four integer keys are present only for `conformance-small`. When `profile` is `production`, those four integer keys are absent. They are not a producer claim. The keys of this full record, in this order, are `profile`, and when `profile` is `conformance-small` the four integers `sample_bytes`, `slot_bytes`, `terrain_bytes`, and `steps_per_leg`, then `shape`, `shape_item`, `headers`, `seed_value`, `checkpoints`, `waypoints`, `paths`, `anchors`, `anchored_total`, `skip_list`, `a`, `b`, `N`, `S`, `delta_h`, `fixed_start`, `P_b`, `height_P_b`, `index_comparison_P_b`, `T_P_b`, `height_I_a_minus_1`, `T_I_a_minus_1`, `six_leg`, `six_leg_runs`, and `analysis`. On this full record, every key listed above is present. The fast record in section 15.3 includes `checkpoints[*].legs[*].result` from the Rule A4.3 digest and omits `six_leg` and `six_leg_runs`. The Rule A4.3 checkpoint digest is a cheap check on every checkpoint. It hashes the stored sample. It does not recompute a leg. The default policy runs it with the other cheap checks. It is not thorough-only. The fast checkpoint-binding checks are the fold and the waypoint rehash under section 12.2 from stored checkpoints. Every other section 14 key stays, including `checkpoints[*].g`, `checkpoints[*].legs[*].j`, `waypoints[*].result`, and `seed_value`. It has no placeholders. It does not fill an omitted key with `not defined`. There is no JSON null. Tokens are JSON strings. Top-level key order is not significant for the values. The bytes hashed as `verify_record_sha256` are this object encoded with RFC 8785 (JSON Canonicalization Scheme). The reference CLI writes this one object with no insignificant whitespace, keys in the order listed above, and one trailing newline (U+000A). The historical pack `spec/conformance/v2.0/` is the withdrawn construction and is not this record. The v2.1 pack does not publish a section 14 record. That byte order is not a second record. The compact line that named only `profile`, `shape`, `checkpoints`, `waypoints`, `leg_rate`, `six_leg`, and `anchors` is not a conformance output and the reference CLI does not write it. That encoding is the default for this cut. If a future profile stores the record as CBOR instead, that profile must name deterministic CBOR (RFC 8949 §4.2.1) explicitly. This file does not define a third encoding. Arrays are JSON arrays. `n/a` is used only as `plant` when `path_state` is `skip`: the plant comparison does not apply. `n/a` is not `not defined`. A field Rule A4.11 leaves undefined stores `not defined`. Display rounding is not under test. The rate quotient is not a key of the verify record.

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

When the object’s `kind` is `KIND` and the object decodes, and `last_interrupt` and `last_interrupt_height` are both absent, and every other Rule A3 condition that uses only present fields is met, `shape_item` is exactly `["field set", "last_interrupt", "header height order", "header hash differ"]`. That is a shape measurement of a v2.0 object. It is not Rule A5. Rule A5 applies only when `kind` is the earlier kind named in section 14.7. `headers` still has 19 elements. `headers[18].stored_hash` and `headers[18].stored_height` are `not defined`. `headers[18].index_comparison` is `not read`, and `headers[18].T` is `not read`. If only the `last_interrupt` hash is absent and `last_interrupt_height` is present and strictly greater than `stages[17].beacon_height`, `header height order` is not violated.

`headers` is an array of 19 objects, index 0 through 18, in stored-header order. Index 0 is `stages[0].beacon` (`seed_beacon`). Index `k` for `k` from 1 through 17 is `stages[k].beacon` (`I_{k−1}`). Index 18 is `last_interrupt` (`I_17`). Each object has exactly `stored_hash`, `stored_height`, `index_comparison`, and `T`. `stored_hash` is 64 lowercase hexadecimal digits, natural byte order, byte 0 first, the hash stored for that header, or `not defined` when that hash is absent. `stored_height` is the unsigned height stored for that header, or `not defined` when that height is absent. An integer above `2^53 − 1` in that object byte is not recorded as that unsigned value: `shape` is `mismatch`, `shape_item` includes `height_range`, and the value is `not defined`. It is not `not read`. `index_comparison` is `match`, `mismatch`, or `not read`. When `stored_hash` is `not defined`, `index_comparison` is `not read`. `T` is an integer number of Unix seconds only when `index_comparison` is `match` and `header.time` follows section 9, and `not read` otherwise.

`seed_value` is `match` if `stages[0].waypoint` equals `SHA-256(public_key ‖ seed_beacon)`, and `mismatch` otherwise.

`checkpoints` is an array of 18 objects, index `g` from 0 through 17, one per stage. Each object has `g` and `legs`. `legs` is an array in leg order, `j` from 0 through one less than the number of checkpoints in that stage. `legs` runs over the checkpoints. Each leg object has `j` and `result`. `result` is `match` or `mismatch`. A checkpoint with no leg result has `result` `mismatch`.

`waypoints` is an array of 17 objects for the Rule A4.3 recomputes of `W_1` … `W_17`, in ascending `g` from 1 through 17. Each object has `g` and `result` (`match` or `mismatch`). When the stage bytes needed for that recompute are absent or unusable, including malformed CBOR, `result` is `not defined`. It is not `mismatch`. `checkpoints[g].legs` is `not defined` in that same case. An empty checkpoint array is usable: `legs` is `[]`, the waypoint is still recomputed, and a differing digest is `mismatch`. A present checkpoint with no leg result still has `result` `mismatch`.

`paths` is an array of 17 objects in ascending `g` from 1 through 17. Array position `g − 1` is `W_g`.

For `g` from 1 through 16, each object has `g`, `path_state`, `plant`, `P_g`, `height_P_g`, `d_g`, and `claimed_block`. `path_state` is `present` or `skip`. `plant` is `match`, `mismatch`, `not read`, or `n/a`. `plant` is `n/a` only when `path_state` is `skip`. Then `P_g`, `height_P_g`, `d_g`, and `claimed_block` are `not defined`.

For `g = 17`, the object has `g`, `w17_path`, and `claimed_block`. When `stages[17].path` is empty, `w17_path` is `not fetched by design` and `claimed_block` is `not defined`. When `stages[17].path` is not empty, `w17_path` is `not defined`, `shape_item` includes `W_17 path`, and `claimed_block` is the block encoding below when both fields can be read, otherwise `not defined`. Those bytes are not `present` and are not anchored and fetched.

`P_g`, `P_b`, and `claimed_block`, when they are objects, have exactly `block_hash` and `block_height`. `block_hash` is 64 lowercase hexadecimal digits in Bitcoin display order: the section 9 reversal of the path body’s natural-order `block_hash` bytes. It is not a byte-0-first spelling of those bytes. `block_height` is an integer. `root` in an index plant comparison stays natural-order hex. That comparison is not these keys. The interrupt-header check uses the path body’s natural bytes and does not use this display spelling.

When `plant` is `match`, `P_g` is that object and `height_P_g` is the index-confirmed `block_height` (the plant the index recorded). Otherwise `P_g` and `height_P_g` are `not defined`. `d_g` is the signed integer `height_P_g − height(I_{g−1})` only when `plant` is `match` and the index comparison of `I_{g−1}` (`headers` index `g`) is `match`. `height(I_{g−1})` in that subtraction is `header.height` from that matching index document. Otherwise `d_g` is `not defined`. `match` for that header requires the stored hash to agree and the stored height to equal `header.height`. Do not subtract `stages[g].beacon_height` when that comparison is not `match`. On a `present` path, `claimed_block` is that object when both fields can be read from the path body, and `not defined` when they cannot. That gap is not an index read.

`anchored_total` is the count of `g` in 1…16 whose `anchors` token is `anchored`. A present path outside the window is not in this count. `skip_list` is a JSON array of integers, ascending, each a `g` in 1…16 whose `path_state` is `skip`. It is `[]` when there is none.

`a` and `b` are JSON numbers in `0 … 2^53 − 1`, or `not defined`, or `not read`. `N` is a JSON number in `0 … 2^53 − 1`, or `not defined`, or `not read`. `S` is a JSON number of seconds in ±(2^53−1), or `not read`, or `not defined`. `delta_h` is a JSON number in ±(2^53−1), or `not defined`, or `not read`. `P_b` is the block encoding for `W_b` when bookends exist. Which of these fields are integers and which are tokens is Rule A4.11.

The raw endpoints are present as their own keys. The read for `P_b` is `GET {index-url}/index/{P_b.block_height}`. `index_comparison_P_b` is `match`, `mismatch`, or `not read`. Decode `header.hash` and `P_b.block_hash` each as 32 display-order bytes under section 7.2. Do not reverse either side. Letter case does not change the bytes. `index_comparison_P_b` is `match` when those 32 bytes are equal and `header.height` equals `P_b.block_height`. It is `mismatch` when the document was read and either the 32 bytes differ or the height differs. It is `not read` when the read does not return a document. `height_P_b` is that document’s `header.height` when `index_comparison_P_b` is `match`, and `not defined` otherwise. `T_P_b` is the integer `header.time` only when `index_comparison_P_b` is `match` and `header.time` follows section 9. A time that does not follow section 9 is `not read`. Otherwise `T_P_b` is `not read`. `height_I_a_minus_1` is `header.height` from the index document for `I_{a−1}` when that header’s `index_comparison` is `match`, and `not defined` otherwise. It is not the stored `stages[a].beacon_height` taken alone. `match` for that header requires the stored hash to agree and the stored height to equal `header.height`. `T_I_a_minus_1` is `T(I_{a−1})` under Rule A4.2, an integer only when that `index_comparison` is `match`, and `not read` otherwise. Which of `a`, `b`, `N`, `S`, `delta_h`, `P_b`, `height_P_b`, `index_comparison_P_b`, `T_P_b`, `height_I_a_minus_1`, `T_I_a_minus_1`, `fixed_start.N`, `fixed_start.S`, `fixed_start.delta_h`, and `analysis.legs_outside` are integers, `not defined`, or `not read` is Rule A4.11, in the order that rule states. A returned header `mismatch` is not the step that sets every bookend field to `not read`.

`fixed_start` is a verify-computed key. Verify computes it from keys this record already holds. The producer does not claim it. The hike object does not carry `fixed_start`. When bookends are defined, `fixed_start` is an object with `N`, `S`, and `delta_h`. `fixed_start.N` is the checkpoint count over stages 1 through `b−1`. It does not read `leg_count`. `fixed_start.S` is `T_P_b − headers[1].T`. If either time is `not read`, `fixed_start.S` is `not read`. `fixed_start.delta_h` is the index-confirmed `height(P_b)` minus the index-confirmed `header.height` of the header at `headers[1]` when `headers[1].index_comparison` is `match` and the `P_b` height read is `match`. `headers[1].stored_height` is not that height. `fixed_start.delta_h` is `not read` when `headers[1].index_comparison` is `not read` or the `P_b` height read is `not read`. A `not read` on either of those reads stays `not read`. `fixed_start.delta_h` is `not defined` only when the object's own bytes leave no bookends, when two or more paths are present, every read returned, and fewer than two `anchors` tokens are `anchored`, or when both of those reads returned and `headers[1].index_comparison` is `mismatch` or the `P_b` height read is `mismatch`. The two-or-more present paths case is index-settled. When the object's own bytes leave no bookends, `fixed_start.N`, `fixed_start.S`, and `fixed_start.delta_h` are `not defined`.

`six_leg` is an object `{ "indices": [q0, q1, q2, q3, q4, q5], "results": [token, token, token, token, token, token] }` in draw order when six indices are drawn. Each result is `match` or `mismatch`. When fewer than six legs exist, `six_leg` is `not defined`. `six_leg_runs` is `[]` in that case. The draw is not made. That array is `[]` only in this case. When the draw is `not read`, `six_leg_runs` is the string `not read` and `six_leg` is `not read`. That count is settled by the object's own bytes. When the byte source cannot supply the next 8 bytes, `six_leg` is `not read`. A short-byte six-leg draw stays re-readable. It is not `not defined`. Short path bytes for a six-leg run are that `not read`. `six_leg_runs` is an array of those objects, one Rule A4.4 draw per element, in the order the runs were requested. The recorded order is that request order. Whether the runs execute together is Rule A4.4.4. `six_leg` is `six_leg_runs[0]` when `six_leg_runs` is a non-empty array. When the six-leg draw is `not read`, `six_leg_runs` is `not read` and `six_leg` is `not read`. That array is not `[]`.

`analysis` is an object with `whole_hike_legs`, `whole_hike_span`, `per_stage`, `legs_outside`, `hike_legs`, and `hike_span`, in that order. `hike_legs` and `hike_span` are Rule A4.10. `whole_hike_legs` is the number of checkpoints over stages 1–17, a JSON number in `0 … 2^53 − 1`. `whole_hike_span` is the JSON number `T(I_17) − T(I_0)` in ±(2^53−1), or `not read`. `per_stage` is an array of 18 objects, index `g` from 0 through 17, each with `g`, `checkpoint_count`, and `span`. `checkpoint_count` is the number of checkpoints in the stage. It is not a read of the object’s `leg_count`. `span` is the JSON number of the per-stage span in ±(2^53−1), or `not read`. `legs_outside` is the number of checkpoints over stages outside `a` … `b−1`, a JSON number in `0 … 2^53 − 1`. It is `not defined` when the bookend fields are `not defined`, and `not read` when those fields are `not read`. `whole_hike_legs` counts checkpoints, including a stage whose count is zero. `whole_hike_span` and each `per_stage` span stay `not read` when an endpoint `T` was not taken, including when the bookend fields are `not defined`. `analysis` has no rate key.

### 14.1 Shape

**Normative.**

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
- An integer above `2^53 − 1` in the object's own bytes, including `stored_height` and `P_b` when those bytes are the object's and that height is not inside a hash-verified path body, is shape `mismatch`. `shape_item` includes `height_range`. The value is `not defined`. It is not `not read`. `block_height` in a body that decodes is in `0 … 2^32 − 1`, which is inside `0 … 2^53 − 1`. A hash-verified canonical path body has no height above `2^53 − 1`. A body that does not decode, or whose `validate_for` does not succeed, has `plant` `mismatch` and `claimed_block` `not defined`. That is not `shape_item` `height_range`.

Shape does not decode a path on stages 1–17 and does not record **skip** or **present**. Rule A4.5 does. This measurement does not GET a path.

### 14.2 Headers

**Normative.**

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

### 14.3 Waypoints and paths

**Normative.**

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

#### Rule A4.5 Paths

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

### 14.4 Bookends

**Normative.**

The verifier’s wall clock is not an input. Verify does not Observe, so it cannot recover an unstored header by assuming the next height. Block times are `T` from Rule A4.2, read as they are, and a `T` is recorded only when that header’s `index_comparison` is `match`. No clamp and no reordering is applied to a recorded `T`. Bookend `S` uses those raw `header.time` values. `hike_span` does not. Rule A4.10 sets `hike_span` from median-time-past at the first beacon and at the last beacon. No other adjustment is applied. `delta_h` (`Δh`) is recorded in this bookend block when both heights are index-confirmed. `delta_h` stays in the bookend block. Rule B3 does not read it. Rule B3 is not an admission input. It is a raw measurement. It is not analysis and it is not policy.

#### Rule A4.6 Bookends

`W_a` and `W_b` are the anchored-and-fetched waypoints with the lowest index and the highest index, wherever they sit. “First” and “last” mean those indices, not the order in which asks completed. Anchored and fetched is the `anchors` token `anchored` from section 14.3: in the section 13.1 window, an interrupt header stored on the object, and plant `match`. A present path outside that window is never a bookend. A present path whose plant comparison is **mismatch** is never a bookend.

`a` and `b` are those indices. Both are in `1 … 16`. `W_17` is not fetched by design, so it is not a bookend. Stage 0 has no path, so `a` is not 0.

#### Rule A4.7 Bookend legs

`N` counts the checkpoints in stages `a` through `b−1`. Each stage’s checkpoints are folded into the next waypoint. `N` does not read `leg_count`. `N` is a JSON number in `0 … 2^53 − 1`.

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

#### Rule A4.9 Bookend leg rate

The verify record holds the integer `N` and the integer `S` only. It does not hold a floating-point rate. It does not hold any other encoding of the quotient.

The bookend display `N÷S` is not the admission rate. Rule B0 reads `(hike_legs, hike_span)`. Rule A4.11 records `N` and `S` as `not defined` in steps 1 and 3 of that rule, and as `not read` in steps 2 and 4. This rule does not then store integers. A bookend field of `not defined` does not, by itself, refuse admission.

The rate display is `N÷S` when `S > 0`. Otherwise the rate display is `not defined`. `N` and `S` are still shown. `not defined` here is a display token. It is not a floating-point value, and it is not a judgment. Display rounding is not under test. The quotient is not in the verify record. The display is informative. It is not an admission input.

When `S` is **not read**, the rate display is **not defined**, and there is no numeric pair.

#### Rule A4.10 Analysis

Record these beside the bookend block. `hike_legs` and `hike_span` are the admission inputs (Rule B0). The other lines in this rule are analysis. Rule B0 does not read `whole_hike_legs`, `whole_hike_span`, a per-stage span, `legs_outside`, or the bookend pair for the floor. Each span below uses `T` from Rule A4.2, so each endpoint time exists only when that header’s `index_comparison` is `match`. The span is a signed integer of seconds when both endpoint times were read, and `not read` when either endpoint `T` is `not read`. Each rate below is display only, on the same rule as Rule A4.9: the quotient when the span is an integer greater than 0, otherwise `not defined`. Display rounding is not under test. The verify record’s analysis holds the integers. It does not hold a float. `delta_h` is not an analysis value.

**First beacon.** The first beacon is `seed_beacon`, the object field `stages[0].beacon`. Its stored height is `stages[0].beacon_height`. In the verify record it is `headers[0]`.

**Last beacon.** The last beacon is `last_interrupt`, which stores `I_17`. Its stored height is `last_interrupt_height`. In the verify record it is `headers[18]`.

**How the verifier reads the times.** Rule A4.2 already issues `GET {index-url}/index/{stages[0].beacon_height}` and `GET {index-url}/index/{last_interrupt_height}`, each height in the shortest base-10 digits, under section 9. `T(first beacon)` is `headers[0].T`. `T(last beacon)` is `headers[18].T`. Each is the integer `header.time` of that index document only when that header’s `index_comparison` is `match` and `header.time` follows section 9. Otherwise that `T` is `not read`. The object stores no block time. Those two `T` values are not `hike_span`. Bookend `S` and the analysis spans still use `T`. The verifier does not subtract a time taken from a producer-stored height for those spans.

**Median-time-past.** `hike_span` uses median-time-past, not raw `nTime`, so a producer cannot shorten the span by choosing a beacon whose `nTime` sits ahead of real time. For a beacon at height `h`, let `n = min(11, h + 1)`. The verifier reads the `n` headers at heights `h − n + 1` through `h` inclusive. Each read is `GET {index-url}/index/{height}` under section 9, with the height in the shortest base-10 digits. The beacon’s own header is the Rule A4.2 read already required for `headers[0]` or `headers[18]`. The other heights in that window are additional reads. They are required for `hike_span` only. They are not a plant scan and they do not choose a different interrupt. Take `header.time` from the beacon’s document only when that header’s `index_comparison` is `match` and the time follows section 9. Take `header.time` from each other document in the window when the read returned a document, `header.height` equals the requested height, `header.time` follows section 9, and, where the index body carries a previous-hash, that header’s previous-hash links to the next header toward the beacon. A carried previous-hash that does not link that way leaves that header’s time missing. If any of the `n` times is missing, MTP for that beacon is `not read`. Otherwise sort the `n` times into non-decreasing order and take the value at index `n // 2`, counting the first as index 0. For the ordinary case `n = 11`, that index is 5, the middle of the 11. A height below 10 uses the shorter window. A beacon on the production profile is above height 10, so `n = 11`.

`MTP(first beacon)` is that value at `stages[0].beacon_height`. `MTP(last beacon)` is that value at `last_interrupt_height`. The verifier reads those windows from the portable index. It does not read a time the producer stored on the object.

- `hike_legs`: the number of checkpoints over stages 0 through 17, the sum of each stage’s checkpoint count, including stage 0 and including a stage whose count is zero. The count does not read `leg_count`. It is not `whole_hike_legs`. It is not bookend `N`.
- `hike_span`: `MTP(last beacon) − MTP(first beacon)`, a JSON number in ±(2^53−1) when both MTP values are integers, including zero or a negative value. If either MTP is `not read`, `hike_span` is `not read`. Do not invent an integer. `hike_span` ≤ 0 is still that integer. The rate that uses it is undefined, and Rule B2 refuses. `hike_span` is not `T(last beacon) − T(first beacon)`.
- Whole-hike, analysis only: the number of checkpoints over stages 1–17, and the span `T(I_17) − T(I_0)`. `I_0` is `stages[1].beacon`. `I_17` is `last_interrupt`. Stage 0’s checkpoints are not in `whole_hike_legs`. They are in `hike_legs`. The count does not read `leg_count`. This pair is not the admission rate.
- Per-stage span: `T(I_0) − T(seed_beacon)` for stage 0; `T(I_g) − T(I_{g−1})` for stages 1–16; `T(I_17) − T(I_16)` for stage 17. The per-stage count is the number of checkpoints in that stage. The per-stage rate display divides that count by that span. It does not read `leg_count`. A per-stage rate is not an admission test. There is no per-stage leg minimum.
- Legs outside the bookends: the number of checkpoints over the stages that are not in `a` … `b−1`. The count does not read `leg_count`. When bookends are **not defined**, this count is **not defined**. This count is not an admission input.

`delta_h` is not in this list. It stays in the bookend block (Rule A4.8).

#### Rule A4.11 Fewer than two anchored and fetched

Apply these steps in order. Stop at the first step that records the bookend fields. `fixed_start` is a JSON object with keys `N`, `S`, and `delta_h` in every step. `fixed_start` itself is not the string `not defined` or `not read`. The bookend fields are `a`, `b`, `N`, `S`, `delta_h`, `P_b`, `height_P_b`, `index_comparison_P_b`, `T_P_b`, `height_I_a_minus_1`, `T_I_a_minus_1`, `fixed_start.N`, `fixed_start.S`, `fixed_start.delta_h`, and `analysis.legs_outside`.

1. A present path is a non-empty path on `W_1` … `W_16` that decodes and whose `validate_for` succeeds. When fewer than two paths among `W_1` … `W_16` are present, record every bookend field as `not defined`.

2. An issued plant GET is a plant read for a present path whose inclusion height is in the section 13.1 window and whose inclusion header is a stored interrupt. When any issued plant GET does not return a document, record every bookend field as `not read`. This step runs before the anchored count. A `plant` `not read` whose GET was not issued, because the path is outside the window or its inclusion header is not a stored interrupt, does not satisfy this step. That path is `not anchored`. The nineteen Rule A4.2 header reads are not this step.

3. When fewer than two `anchors` tokens among `W_1` … `W_16` are `anchored`, record every bookend field as `not defined`. Unread Rule A4.2 headers do not change this step. The anchor-window vector is this step: three paths are present, one is `anchored`, the issued plant read returned a document, and the header reads returned no document. The bookend fields are `not defined`. The header tokens on that vector are `not read`.

4. Otherwise `a` is the lowest anchored index and `b` is the highest, both in `1 … 16`, and `N` is the checkpoint count over stages `a` … `b−1` (Rule A4.7), including a stage whose checkpoint count is zero. Then read the endpoints. If `headers[a]` (`I_{a−1}`) has `index_comparison` `not read`, or the path at `W_b` does not yield a block, or the header read of `P_b.block_height` is `not read`, record every bookend field as `not read`. A returned document whose comparison is `mismatch` is not this case: `a`, `b`, and `N` stay the integers, `S` is `not read` when a `T` was not taken, and `delta_h` is `not defined` when a height is not index-confirmed (Rule A4.8). An unread header that is not `headers[a]` and is not the `P_b` read does not, by itself, change `a`, `b`, or `N`.

`analysis.whole_hike_span` and each `per_stage` span stay the Rule A4.10 values. They are `not read` when an endpoint `T` was not taken, including when step 1 or step 3 records the bookend fields as `not defined`. When two or more waypoints are `anchored` and step 4 does not apply, Rules A4.6 through A4.8 record the integers.

**Informative.** The legs in `N` start from `W_a`, which hashes `I_{a−1}`, so they could not begin before `I_{a−1}` existed. They are folded into `W_b`, whose plant is in `P_b`, so they existed by then. Naming an older interrupt only makes `S` longer. The interrupts in between, and any skipped waypoints, do not change `N÷S`, apart from `nTime` effects at the two ends of `S`. An honest `S` includes the gap from `I_{b−1}` to `P_b`, which is two, three, or four blocks when headers arrive one at a time. Miner `nTime` can move either end. That remark is about the bookend display. It is not the admission rate. Rule B3 does not adjust `hike_span`. That remark is not a Part A number. A stored interrupt shows that the header exists. It does not show that produce ran live. Only anchored and fetched shows that the fold existed by the plant’s block.

### 14.5 Anchored and fetched

**Normative.**

`anchored_total` is Rule A4.5: the count of the `anchors` token `anchored` among `W_1` … `W_16`. A `skip` is not in that count. A present path outside the window is not in that count. A `present` path with `plant` `mismatch` or `not read` is not in that count. `W_17` is not in that count. This section records the count and does not judge it. Rule B4, settled and normative, compares the count with 6. That comparison is SPEC text. It is not a Part A measurement and it is not a candidate. How many waypoints are fetchable is Rule A3.3.

### 14.6 Six-leg check

**Normative.**

#### Rule A4.4 Six-leg check

Six legs are drawn. The draw count is 6. It is not `STAGES`. It is not `AUDIT_K`. `N` in this file is bookend legs (Rule A4.7). This section does not use `N` for the hike’s leg total.

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

The index draw in this section runs on the calling thread. It finishes before any leg result is recomputed. A verifier may recompute the six leg results concurrently, and may run Rule A4.3 checkpoint comparisons concurrently. How many Rule A4.3 comparisons run at once is a verifier setting. The verify record does not depend on that number. One at a time is conforming. The reference verifier’s default is six concurrent Rule A4.3 comparisons, and that default is not a conformance constant. Elements of `six_leg_runs` may execute in parallel. This file does not require a thread pool. That one-at-a-time permission is the section 14 measurement record. It does not apply to the default policy’s `thorough_rounds` (section 15.5).

For each chosen index, recompute that leg from section 11.2. The previous checkpoint is that stage’s leg-0 start when `j = 0` (Rule A2.2): `SHA-256(public_key ‖ seed_beacon)` on stage 0, and the stored `waypoint` on stage `g ≥ 1`. Otherwise it is the stored `checkpoints[j − 1]` of that stage. The draw count is 6 either way.

The order of the chosen indices is the order drawn. The order in which the leg computations finish does not change that order. For each chosen index, record `match` if the recomputed leg result equals `u64_le(leg_results[j])`. Record `mismatch` otherwise.

### 14.7 Earlier objects

**Normative.**

#### Rule A5. Earlier objects

An object whose `kind` text is `satoshi-hike-v1.35-object` or `satoshi-hike-v2.0-object` is an earlier object. Those two strings are the earlier kinds this file names. The check is the kind text. It runs before any terrain fill. Verify records:

```json
{"version":"earlier version","kind":"<the earlier kind text>"}
```

`version` is the measurement `earlier version`. `kind` is the kind text the object carried. The record does not apply sections 14.1–14.6. It is not a decode failure. It is not `shape` `mismatch`. It does not say the object could not be read. A kind text that is not `KIND` and is not one of these two earlier kinds stays a shape measurement under section 14.1. An earlier object is not a fake leg. The admission service does not add its key to the ignore record. Bytes that are not a CBOR map, or a map that does not yield a `kind` text, are still a decode failure.

The bytes of a kept example stay where they were published. This rule does not rewrite them.

## 15. Admission service policy

**Normative for the admission service only. Part B.** Rule B9 is normative. This section defines the parameters and the comparisons. It may discuss thresholds. Apart from `anchored_total ≥ 6` (Rule B4, settled 21 Oct 2026), it sets no threshold value. The leg-rate floor is TBD and is set by a policy. Values belong to a policy chosen by the relying party (see the policy companion `services/admission-gate/POLICY.md`, which is a recommendation and not part of this file). A `provisional` state grants no action (`provisional_action_cap`). The re-verify rule dated 2026-10-07 is locked. Rules B0, B2, B3, B4, B6, and B8 are normative as rules. Rule B4’s comparison `anchored_total ≥ 6` is settled. Rule B7’s fake-leg ignore is settled and normative. Rule B10, the submission deadline, is normative. Its block bound is placeholder, not ratified. Rule B7’s cap figures, and Rules B1 and B5, remain candidates. The leg-rate floor’s numeric value is policy. Locked 2026-10-10: the section 15.1 sign-in wire. Amended 2026-10-16: the signed origin is the requesting site's own origin, compared exactly (section 15.1 **Origin**); `https://groundinglab.io` is an example deployment. That wire is the challenge link, `challenge_bytes`, the BIP-340 message, the origin display, the lifetime and the clock skew, the submit request, the client response codes, and the website outcome tokens. This lock does not add a leg-rate value or an admission bar. Section 15.5 states the default policy (cheap checks, then one draw of up to 18 distinct legs split into 3 rounds of 6, each round in parallel) and the floor’s measuring method. It locks no other figure.

Part A records and measures. This section is the only judgment. The producer in section 13 records evidence and does not judge. The verifier in section 14 outputs a verify record of measurements and does not judge. The admission service applies this section.

The admission service is the actor in this section. Any service that meets this section conforms to it. The groundinglab Admission Gate (Gate) is one implementation, run for the example deployment. It is not a second specification. The signing client uses one route, `POST /v1/sign-in`, at the default door named in section 15.1. `POST /v1/sign-in/challenge`, `POST /v1/challenge`, and `POST /v1/admit` are not routes. The website mints the challenge link and owns the browser session. That session is not a field of the hike object and not a field of the verify record. The grant named later in this section is not placed in the link and is not returned to the signing client. A deployment of the admission service may spell `refuse` as `deny` on its wire. The word in this file is `refuse`.

A threshold here can change without changing section 13 and without changing the measurements in section 14. No order among B0–B8 is locked. Section 15.5 lists numeric examples. It does not lock them.

The Gate decision log is the companion specification Gate decision log, outside this file. This file does not write that log and does not write log JSON. Thorough outcomes are: thorough decides `accept` and the accept stands; `revoke`; `refuse`; or `held`. `held` is a thorough `not read` or a retry. It is not a negative settle. A negative thorough outcome, or a failed policy-version re-verify, is `revoke` if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. `provisional` then `held` then a negative thorough outcome is `revoke`. A later `revoke` cites a new policy version or a new measurement. Admission outcomes inside this section are `provisional`, `accept`, `refuse`, `revoke`, and `held`. `held` is an admission outcome. It is not a decision-log event. Section 15.1 names the signing client's response and the website outcome.

### 15.1 Inputs

**Normative.** The admission service runs its own conforming section 14 verifier on the hike object it receives. It never takes a verify record supplied by the submitter. The record it produces is bound to the object hash: `SHA-256` of the exact object bytes.

The inputs are:

1. The hike object, its bytes.
2. The object’s `public_key`.
3. The object hash, `SHA-256` of those bytes.
4. The challenge defined in this section. The website mints it as the link in this section.
5. A BIP-340 signature over that challenge, checked against `public_key`. Produce and verify do not check a signature (section 2). The signature check is not a section 14 measurement.
6. A policy profile (section 15.4). The profile may name a friendly list. The friendly list is not a field of the hike object and not a field of the verify record.

#### Challenge

The challenge is a hard cut from a signature over a display-order index header hash, and from the Option B layout (domain `pcoc-admit-challenge-b-v1`, a 16-byte id, little-endian milliseconds, and no object hash). The Sign-in door does not accept either of those. The cut is deliberate.

**Domain.** The domain is the 19 ASCII bytes `pcoc-gate-signin-v1`. There is no trailing NUL in the tag.

**Origin.** `origin` is the origin of the requesting site: the site that minted the challenge link and that the signature is for. It is the ASCII serialization of a web origin under RFC 6454 section 6.2: the lowercase scheme `https`, then `://`, then the host in lowercase (an internationalized host in its A-label form), then `:` and the port in shortest decimal only when the port is not 443. It has no userinfo, no path, no query, no fragment, and no trailing slash. `n` is its byte length, from 1 through 255.

Comparison is exact. Two origins are the same only when their serialized bytes are equal. Nothing is case-folded, trimmed, decoded, or completed with a default port before the comparison.

- A signing client MUST take `origin` from the scheme, host, and port of the challenge link it signs, serialized as above. It MUST NOT substitute a built-in value. It MUST NOT sign when the link's origin is not in serialized form.
- Any operator MAY run an admission service. An admission service MUST accept a signed `origin` only when it is byte-for-byte equal to an origin that service is built or set to serve. It MUST NOT accept any other value. It MUST NOT normalize the received value first.
- A received `origin` that is not equal to a served origin is check-order step 2: client `malformed`, unchanged from the check order below.

`https://groundinglab.io` is one example deployment. It is not a requirement and is not special: for it, `n` is 23. `https://www.groundinglab.io`, `https://groundinglab.io/`, `http://groundinglab.io`, and `https://groundinglab.io:443` are not equal to it. The door host is not the Sign-in origin. Vectors: `spec/conformance/v2.0/sign-in-origins.json` signs the same challenge for `https://example.com` and for near misses, and gives what a service that serves only `https://example.com` returns for each.

**Other fields.** `nonce` is 32 bytes. The website draws them from a CSPRNG when it mints the link. `expiry_unix_s` is a `u64` count of Unix seconds, not milliseconds. `object_sha256` is the 32-byte SHA-256 of the exact object bytes that are submitted. The preimage does not carry an IP address, a Cloudflare token, a session cookie, a user-agent, or a second copy of `public_key`.

**Lifetime and clock skew.** The lifetime is 120 seconds. `expiry_unix_s = issue_unix_s + 120`. `issue_unix_s` is the integer Unix second at which the website mints the link, the floor of the millisecond clock divided by 1000. The clock-skew allowance is 5 seconds. The lifetime is not part of the hash formula. The Gate compares the integers with a wide sum so the addition does not wrap. The Gate accepts the challenge when `now_unix_s < expiry_unix_s + 5` and `expiry_unix_s <= now_unix_s + 125`. Otherwise the client code is `malformed`. There is no `expired` code. The signing client does not enforce the window. A one-hour grace after expiry is retired.

**Bytes.** Integers are big-endian.

```text
challenge_bytes =
    ASCII "pcoc-gate-signin-v1"
  ‖ 0x00
  ‖ uint8(n)
  ‖ origin_utf8[0 .. n)
  ‖ nonce[32]
  ‖ uint64_be(expiry_unix_s)
  ‖ object_sha256[32]
```

The length is `93 + n` bytes. Examples: `https://groundinglab.io` gives `n = 23` and 116 bytes; `https://example.com` gives `n = 19` and 112 bytes.

**BIP-340 message.**

```text
bip340_message_32 = SHA-256(challenge_bytes)
```

That 32-byte digest is the BIP-340 message. The signer must not hash it again. `SHA-256(bip340_message_32)` is not the message. The signature is 64 bytes under the object's x-only `public_key`. Verify is `bip340_verify(public_key, bip340_message_32, signature)`.

**Challenge link.** One canonical form. The Android QR encodes that same UTF-8 string, with no byte-order mark. The link starts with the requesting site's origin, exactly as serialized under **Origin**; `groundinglab.io` is the example deployment's host. The path is `/sign-in/` with the trailing slash. The query order is fixed. There is no extra parameter, no fragment, no userinfo, and no port other than one that is part of the origin. The link does not name the door. The link does not name an object.

```text
<origin>/sign-in/?v=1&nonce=<64 lowercase hex>&exp=<decimal>
```

Example deployment: `https://groundinglab.io/sign-in/?v=1&nonce=<64 lowercase hex>&exp=<decimal>`.

`v` stays `1`. It names the challenge-bytes domain `pcoc-gate-signin-v1`. The object hash is not a link parameter. The client binds `object_sha256` from the hike object it chooses. `nonce` is 64 lowercase hex digits of the 32 nonce bytes. `exp` is `expiry_unix_s` in the shortest decimal form: no sign, and no leading zero except the value 0. A paste trims only leading and trailing ASCII whitespace: space, tab, CR, and LF. The remainder equals that string. Uppercase hex, `http`, an origin that is not in serialized form, an extra parameter, a parameter named `object`, a swapped parameter order, a fragment, a bare 64-hex string, a leading zero on `exp`, an `exp` that is not a `u64`, and a leading U+00A0 are not links. The client stops before any POST. The website does not emit `object=`. A link that carries `object=` is not a link. Clients do not accept it. The form `<origin>/sign-in/?h=` plus 64 hex digits is retired. The link classes in `sign-in.json` are given for the example origin `https://groundinglab.io`; there, `www.groundinglab.io` is another origin, not that site's link. A signature over a display-order index header hash is retired.

**Origin display.** Before the signature is produced, the client shows the origin taken from the link (example deployment: `https://groundinglab.io`). It shows no path, no query, and no door host. On Android that origin is the only confirmation. The client then signs and POSTs, with no second control. On desktop the sign control stays disabled until the origin is shown. One press signs and POSTs once. The client chooses the hike object. That object is the current or selected produced object on the device. When the device holds several, the client lets the user pick. The chosen object's SHA-256 is `object_sha256` in `challenge_bytes` and in the POST body. When the device holds none, the client does not sign and does not POST. That stop is local. It is not a Gate token.

**Submit.** Each signing client is built for the deployments it serves. The Sign-in door URL for each origin the client accepts is part of the client's build configuration. A client MUST NOT derive or guess a door URL from the origin. A client MUST refuse a link whose origin has no configured door; it does not sign and does not POST. Every signing client POSTs once to the configured Sign-in door for the link's origin. In the example deployment that door is `POST https://admission.groundinglab.io/v1/sign-in`. The content type is `application/json`. The client does not follow a redirect. The POST target is that door. It is not the scanned URL. The body is one JSON object with each of these keys once:

Example deployment:

```json
{"origin":"https://groundinglab.io","nonce":"<64 lowercase hex>","exp":<u64>,"object":"<64 lowercase hex>","object_b64":"<standard base64, with padding, no whitespace>","signature":"<128 lowercase hex>"}
```

`exp` is a JSON integer in the `u64` range, not a string. `object_b64` is standard base64 of the exact object bytes, with `=` padding and with no whitespace. Any other key, a missing key, a repeated key, `exp` as a string, or uppercase hex is `malformed`. The Gate rebuilds `challenge_bytes` from `origin`, `nonce`, `exp`, and `object`. The signature origin is accepted only under **Origin**. The origin is the body field. It is not the caller's `Origin` header.

**Client response.** The client class is one of three. These sentences are the only sign-in sentences the client shows: `Sign-in submitted`, `bad signature`, and `malformed`.

- `bad_signature`. HTTP 400. The body is `{"error":{"code":"bad_signature"}}` and no other field. The request was not submitted. The client shows `bad signature`.
- `malformed`. HTTP 400. The body is `{"error":{"code":"malformed"}}` and no other field. The request was not submitted. The client shows `malformed`.
- Delivery OK. Any other completed HTTP response, including 200, 413, 415, 429, and 503, except a redirect. The client shows `Sign-in submitted`. On the check-order success path the body is `{"delivery":"ok"}` and no other field. The POST body does not carry `outcome`. The client ignores `decision`, `reason`, `reasons`, `outcome`, and `verifier_ran`.

A redirect (HTTP 3xx) is not followed and is not delivery OK. An HTTP 400 whose body is not one of the two objects above is not delivery OK. For a redirect, for that other HTTP 400, and when no HTTP response arrives, the client does not show `Sign-in submitted` and does not show a Gate outcome. The client does not poll. The client does not call the result read. The client does not show `accept`, `refuse`, `provisional`, `held`, `revoke`, or a policy sentence. `unknown_challenge`, `expired`, and `reused` are not client codes.

A BIP-340 failure is an error named `bad_signature`. It is not a section 15 decision and it is not a fast final `refuse`. There is no decision line. An append-only hash-chained log entry may record it. v1 has no signed head and no signed receipt. This file does not write the entry.

**Website outcome.** The website reads `GET /v1/sign-in/result/<64 lowercase hex nonce>`. The response is HTTP 200 and the body is `{"outcome":"<token>"}` with no other field. An unknown nonce, and a nonce the Gate has not stored, is `pending`. That read does not create a row. The signing client does not call this route. The website of the requesting origin is the reader (example deployment: `https://groundinglab.io`).

| `outcome` | Words on the website |
|---|---|
| `pending` | Progress. The word pending is not the result |
| `accepted` | Open `/welcome/`. The website mints the browser session. This token is not a hike grade |
| `malformed` | malformed |
| `bad_signature` | bad signature |
| `too_short` | too short |
| `rejected` | rejected |
| `earlier_version` | earlier version |
| `too_few_waypoints_anchored` | too few waypoints anchored |

`rejected` has no extra field. `too_short` is a decoded object shorter than 32 bytes. Those bytes cannot hold an x-only key, so no signature check runs. The POST is delivery OK. The signing client shows `Sign-in submitted`.

`earlier_version` is the kind `satoshi-hike-v1.35-object` or the kind `satoshi-hike-v2.0-object` after a valid signature. That outcome is not a fake-leg ignore. `accepted` is the thorough accept. While admission is unsettled, including a thorough `not read`, the website outcome stays `pending`. An unfinished page wait is not `rejected`. The website session lifetime is the website's own lifetime. It is not this challenge lifetime.

`too_few_waypoints_anchored` is the words for the already-written refuse-at-once: fewer than 6 of `W_1` through `W_16` could be anchored, wherever those waypoints sit. A build that does not enforce that row does not emit the phrase. The old candidate of two is withdrawn. A legitimate v2.0 object that the Gate refuses, when the refusal has no phrase in the table, is `rejected`. A later lock may add a plain phrase to the table. Until the phrase is in the table, the website outcome is `rejected`. The leg-rate floor the deployed policy sets stays enforced. The website and any public reason string do not show the fraction, a threshold, a verifier note, or a retry instruction. This lock adds no leg-rate value and no admission bar.

**Check order.** The door applies these steps in order and returns on the first step that decides.

1. The body is not one JSON object, or `nonce` is not 64 lowercase hex digits. The client code is `malformed`. There is no result row. The nonce is not consumed.
2. `origin` is not byte-for-byte equal to an origin this service serves (see **Origin**), a field breaks the **Submit** grammar, or the window does not accept the challenge. The client code is `malformed`. The nonce is not consumed. When the object bytes hash to `object` and this nonce has no consumed row, the website outcome is `malformed`. Otherwise the website outcome stays as it was.
3. SHA-256 of the object bytes is not `object`. The client code is `bad_signature`. The body does not say which check missed. The nonce is not consumed. The website outcome stays as it was.
4. A consumed row exists for this nonce. The same `object` and the same signature return the original client class and the original website outcome. Policy does not run again. Any other object or signature is client `malformed`, and the stored website outcome stays. A `submission_sha256` of the same submission returns the original outcome and does not mint a grant.
5. The object is shorter than 32 bytes. The website outcome is `too_short`. The client class is delivery OK. The nonce is consumed.
6. The bytes yield no 32-byte public key, or the signature does not verify over `bip340_message_32`. The client code is `bad_signature`. The body does not say which check missed. The nonce is not consumed. The website outcome is `bad_signature`. A later consuming submit replaces this recorded outcome.
7. The public key is on this gate’s persistent fake-leg ignore record. The website outcome is `rejected`. The client class is delivery OK. The nonce is consumed. Section 14 does not run. There is no new draw. The same refusal is returned for every later submission from that key at this gate.
8. The object is an earlier kind, `satoshi-hike-v1.35-object` or `satoshi-hike-v2.0-object`. The website outcome is `earlier_version`. The client class is delivery OK. The nonce is consumed. This step does not add the key to the fake-leg ignore record. It does not fill terrain.
9. The object is not a v2.1 hike object (`KIND` `satoshi-hike-v2.1-object`). The website outcome is `rejected`. The client class is delivery OK. The nonce is consumed. This step does not add the key to the fake-leg ignore record.
10. The object is a v2.1 hike object. The client class is delivery OK. The nonce is consumed when this step starts. The website outcome is `pending` until policy settles, then `accepted`, or `too_few_waypoints_anchored` when that refusal is the one this section names, or `rejected`. A later policy refusal does not free the nonce. A cheap-check failure on this object is a plain refusal and does not add the key to the fake-leg ignore record.

**Transport edge.** Before the check order, the door may refuse a request as oversize, as the wrong content type, or as over its transport rate. Each of those is a completed HTTP response, and the client class is delivery OK. The wrong content type writes no website outcome. Oversize and the transport rate write website `rejected`, and do not consume the nonce, only when the body is the JSON object, `nonce` is 64 lowercase hex digits, the nonce has no consumed row, and the object bytes hash to `object`. Otherwise the website outcome stays as it was. An unavailable door writes no website outcome. This lock does not change the door's transport figures and does not put those figures on the sign-in wire.

**Single-use.** The service stores a row keyed by `nonce`: `origin`, `expiry_unix_s`, `object_sha256`, the signature, the client class, and the website outcome. The row is written when the nonce is consumed. Two racing submits of one fresh nonce must not both consume the nonce. The set is an atomic compare-and-set. The nonce compare-and-set does not admit the provisional grant. A result row stays readable for 600 seconds after the website outcome becomes something other than `pending`. A `pending` row stays until it settles or the process drops it. A restart that drops the row forgets an outstanding challenge. The client asks again. After the row is gone, the result read is `pending` and creates no row. A late replay whose window has closed is `malformed`.

```text
submission_sha256 = SHA-256(
    object_sha256 ‖ public_key ‖ bip340_message_32 ‖ signature)
```

That preimage is 32 ‖ 32 ‖ 32 ‖ 64 = 160 bytes. The Gate decision log names how a submission is logged. This file does not write its JSON. A negative thorough outcome in that log, or a failed policy-version re-verify, is `revoke` if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. A later `revoke` cites a new policy version or a new measurement.

`verify_cap_per_door_session` counts verifies at this door. It is not a cap on a website session. A `provisional` state grants no action (`provisional_action_cap`). No irreversible action is allowed while `provisional`, in every profile. That value is fixed in this file. It is not a profile knob. A `provisional` grant is short, read-only access until thorough settles or `async_catch_budget` is hit. The friendly-list entry is added only at a thorough `accept`.

**Informative.** EXAMPLE, not production. Origin `https://groundinglab.io`, nonce `0123456789abcdef` repeated to 32 bytes, `expiry_unix_s` `1893456000`, `object_sha256` 32 bytes of `0xaa`. That `object_sha256` is the 32-byte field the client binds from the object it chooses. It is not a link parameter. It is not the hash of a file of 32 `0xaa` bytes. The link is `https://groundinglab.io/sign-in/?v=1&nonce=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&exp=1893456000`. `challenge_bytes` hex:

```text
70636f632d676174652d7369676e696e2d7631001768747470733a2f2f67726f756e64696e676c61622e696f0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0000000070dbd880aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

`bip340_message_32` hex: `90e38d7538d543aff5148318b7cb310595def390dac199c99bb10bc9ead5e0c2`. The vector `spec/conformance/v2.0/sign-in.json` signs that digest once under the fixed test key, with 32 aux bytes of zero. SHA-256 of the digest does not verify. The example expiry lies outside a window near 2026, so a live-clock submit of this example is `malformed` on the window. The test key is not a production key.

#### Friendly list

A friendly-list entry is added only at a thorough `accept`. A friendly list maps a `public_key` to an accepted `object_sha256` and a policy version. A listed key may sign in again only for that same already-accepted object hash. While a later-policy re-verify is `not read`, including after either reverify budget is hit, signature-only sign-in continues on the earlier accept. Only a re-verify that refuses, other than a B7 cap refuse and other than Rule B10, ends that sign-in as a `revoke`, and it removes the friendly entry only when this object is the key's current friendly entry. That sign-in does not run section 14 again. A new object hash is a new section 15.1 submission. When the policy version changes, the Gate re-verifies the accepted object. The Gate keeps those object bytes. The decision-log companion publishes every decided object. If those bytes are missing when a re-verify is needed, that is not `not read`. The next sign-in requires a full section 15.1 submission of the object. It is not a signature-only return treated as `not read`. One entry per key, and that entry is one accepted object. Custody candidate, not yet locked: only a thorough `accept` of a newer object replaces the entry. Newer is the decision-log `seq` of that thorough `accept`. A higher `seq` is newer. It is not the submission time and it is not a hike Bitcoin time. A `refuse` or a `revoke` of any other object does not change the entry. A re-verify of an older object does not displace the current entry. A re-verify `accept` keeps the entry only when this object is already the key's current friendly entry. If a newer accept B later loses its accept, A is not restored. The replaced pair's logged decision stays unchanged. A logged manual operator strike of the key is the other removal. A `revoke` in the object's own admission cycle, from `provisional` or from `held`, when thorough falls short of policy, ends that pair's grant and any signature-only sign-in for the object. Signature-only sign-in requires state `accepted`. Who edits the list by hand is a deployment concern. The list has no time expiry.

**Normative.** Locked 2026-10-07 00:15. Re-verify is the re-check rule locked 2026-10-07 00:15. An accept stands. A fake-leg ignore is not cleared by a policy-version change and is not cleared by a restart of a gate process. “That gate” is the admission service’s persistent per-key record, shared by every process of that service. A re-verify of a key on that record does not draw again. A policy-version change means the Gate re-verifies. If that re-verify refuses, other than a B7 cap refuse and other than Rule B10, it is a `revoke`. A B7 cap refuse is never a re-verify refuse, never a `revoke`, and never removes a friendly entry. The key's friendly entry is removed only when this object is the key's current friendly entry, as with any `revoke`. The removal is a compare-and-remove on `object_sha256`: the Gate removes the entry only if the current entry's object hash still matches this pair. A `not read` never removes the entry and never changes sign-in state. Before either `reverify_max_attempts` or `max_held_blocks` is hit, the Gate retries within the existing reverify budget. After either budget is hit on a later-policy re-verify `not read`, the hit is not a `revoke`, does not remove the friendly entry, and does not change sign-in state. After the hit, the Gate keeps retrying on each later `{index-url}` tip advance: at most one retry per strict increase in that tip height. A jump of several blocks is one retry. An equal or lower tip, including a reorg, is not a retry. These post-budget retries do not restart the spent budget and do not re-arm `held_bound`. The number of such retries is unbounded.

#### Binding

The fast decision and a later `revoke` bind the same submission: the object bytes, the object hash, the challenge, and the signature. This file does not mint a website session and does not add a session field to the hike object or the verify record. The grant defined above is the only session handle this section names. A `provisional` grant is short, read-only access until thorough settles or `async_catch_budget` is hit. The friendly-list entry is added only at a thorough `accept`.

### 15.2 Outputs and states

The HTTP response outcomes are `provisional`, `accept`, `refuse`, `revoke`, and `held`. Those words are admission outcomes. The signing client does not receive them. Section 15.1 is the client response and the website outcome. **States** are `received`, `provisional`, `accepted`, `refused`, `revoked`, and `held`. `accept` is the thorough outcome whose state is `accepted`. The fast decision token is `provisional`. A fast-phase `accept` is invalid. The fast phase never issues `accept`. It may issue a final `refuse` when a fast rule fails on the object's own bytes: decode (the object bytes fail to decode as a hike object), waypoint rehash, shape `mismatch`, including `shape_item` `height_range`, seed-value `mismatch`, a plant or header `mismatch` confirmed by a fresh read from `{index-url}` that bypasses the local cache, an object-settled refuse-at-once, or a cap on a signed submission. Rule B10 is also that final `refuse` when `tip_height − last_beacon_height > K`. That `refuse` does not start a thorough run and does not issue a grant. Otherwise the fast phase gives `provisional`. A fast-phase `not read` is `held`. Only the thorough phase issues `accept`. `refuse` is the response whose state is `refused`. `revoke` is the response whose state is `revoked`. `held` is both a response outcome and a state. `held` is not a decision-log event. Thorough outcomes are: thorough decides `accept` and the accept stands; `revoke`; `refuse`; or `held`. `held` is a thorough `not read` or a retry. It is not a negative settle. A negative thorough outcome, or a failed policy-version re-verify, is `revoke` if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. `provisional` then `held` then a negative thorough outcome is `revoke`. A later `revoke` cites a new policy version or a new measurement. Thorough decides `accept`; the accept stands. Sections 8–14 do not use these words as outcomes. This section does not say that a hike object passes or fails. This file does not write the decision-log JSON.

A client cannot trigger `revoke`. `revoke_scope` is the pair `(public_key, object_sha256)`. A `revoke` binds to `(public_key, object_sha256)`. It does not end another object's grant. A `revoke` in that object's own admission cycle ends that pair's grant and any signature-only sign-in for the object. A later-policy re-verify that refuses, other than a B7 cap refuse and other than Rule B10, is a `revoke` and removes the friendly entry only when this object is the key's current friendly entry, by a compare-and-remove on `object_sha256`, only if the current entry's object hash still matches this pair. A `not read` does not. The Gate invalidates the pair's grant before it returns or logs a `revoke`. Every grant use or signature-only sign-in checks the pair's current state. How relying parties hear of a `revoke` stays a deployment concern.

An admission cycle runs from `received` for a `(public_key, object_sha256)` under a `policy_version` until a thorough `accept`, or a `refuse` or `revoke` of the pair (including reason `held_bound`). A cap refuse (`verify_cap_per_object` or `verify_cap_per_door_session`) does not end the cycle. A later policy-version re-verify is not a new admission cycle and never issues `provisional`.

The service keeps an atomic per-cycle record keyed on `(public_key, object_sha256, policy_version)`. That record is not the nonce row. The nonce compare-and-set stays for single-use nonces only. The record admits at most one provisional grant in the cycle. Concurrent submissions of the same pair cannot each receive one. A new submission while the cycle is open returns the current state, with no grant and no new run. A new submission while the cycle is closed `refused` or `revoked` under this `policy_version` returns that outcome and does not open a new cycle. While the pair is `accepted` and this object is the key's current friendly entry, friendly-list and signature-only rules apply. The replaced-object case is the custody-candidate row that follows. A client submission of a pair that is `accepted`, under a new `policy_version`, is a re-verify under the locked re-verify rule. Custody candidate: a pair that is `refused` or `revoked` under an old `policy_version`, submitted under a new `policy_version`, starts a new admission cycle under the new version and may issue `provisional` under the same rules as any fresh cycle.

Row order. The new-submission rows are checked before the cap rows. A cap row applies to the submission the per-cycle record has admitted. It does not apply to a new submission of a pair whose cycle is already open. A cap hit stays in `received`. The submission response is `refuse`. It does not end the cycle.

| From | When | To |
|---|---|---|
| `received` | A new submission of this `(public_key, object_sha256)` under this `policy_version` while the admission cycle is already open in `received`. The atomic per-cycle record is keyed on `(public_key, object_sha256, policy_version)`. It is not the nonce row. Return the current state. No grant. No new run | `received` |
| `provisional` | A new submission of this pair under this `policy_version` while the admission cycle is open. Return the current state. No grant. No new run | `provisional` |
| `held` | A new submission of this pair under this `policy_version` while the admission cycle is open. Return the current state. No grant. No new run | `held` |
| `refused` | A new submission of this pair under this `policy_version`. The cycle is closed `refused`. Return that outcome. No new cycle | `refused` |
| `revoked` | A new submission of this pair under this `policy_version`. The cycle is closed `revoked`. Return that outcome. No new cycle | `revoked` |
| `refused` | Custody candidate. This pair is `refused` under an old `policy_version`. A client submits it under a new `policy_version`. A key on this gate’s persistent fake-leg ignore record does not start that cycle and is not this row. Otherwise the submission starts a new admission cycle under the new version. That cycle may issue `provisional` under the same rules as any fresh cycle. The pair may be accepted | `received` |
| `revoked` | Custody candidate. This pair is `revoked` under an old `policy_version`. A client submits it under a new `policy_version`. A key on this gate’s persistent fake-leg ignore record does not start that cycle and is not this row. Otherwise the submission starts a new admission cycle under the new version. That cycle may issue `provisional` under the same rules as any fresh cycle. The pair may be accepted | `received` |
| `accepted` | Custody candidate. A new submission of this pair under this `policy_version`, and a newer accept has replaced this object as the key's friendly entry. Return the logged `accepted` decision. State that this object is not the key's friendly entry. How that statement is carried is a decision-log companion concern. This file does not name a field for it. No grant. No new run. No signature-only sign-in through this object. This submission does not displace the current friendly entry | `accepted` |
| `accepted` | A new submission of this same pair under this `policy_version`, and this object is the key's current friendly entry. Friendly-list and signature-only rules apply. This is not a new admission cycle and it never issues `provisional` | `accepted` |
| `accepted` | A client submission of a pair that is `accepted` under any `policy_version`. It never issues `provisional`. Under a new `policy_version` it is a re-verify of that pair: straight to thorough, subject to `verify_cap_per_object` on `(public_key, object_sha256, policy_version)`. The result is logged under the new version. A refuse is a `revoke` and removes the friendly entry only when this object is the key's current friendly entry, by a compare-and-remove on `object_sha256`. A `not read` keeps the entry and sign-in state. Signature-only sign-in continues. After either reverify budget is hit on a `not read`, the Gate keeps retrying on each later `{index-url}` tip advance: at most one retry per strict increase in that tip height. A jump of several blocks is one retry. An equal or lower tip, including a reorg, is not a retry. These post-budget retries do not restart the spent budget and do not re-arm `held_bound`. The number of such retries is unbounded. | `accepted` |
| `received` | `provisional` is on, no grant was issued in that admission cycle, the fast measurements and the signature meet the profile, and every fast measurement the profile needs for `provisional` is present as an integer or a token other than `not read` or `not defined`. At most one provisional grant is issued in that cycle | `provisional` |
| `received` | The profile sets a figure, and on a valid signature and a fresh challenge `verify_cap_per_object` is hit. A `verify_cap_per_object` refuse is final for that `submission_sha256` only. It does not change the pair's state or friendly entry. It is not the pair's thorough decision. A cap refuse (`verify_cap_per_object` or `verify_cap_per_door_session`) does not end the cycle. The pair's state does not move to `refused`. The submission response is `refuse`. Service-started re-verifies and `held` retries are not in the count. An unauthenticated rate limit is not this row. A transport rate limit is not this row | `received` |
| `received` | The profile sets a figure, and on a valid signature and a fresh challenge `verify_cap_per_door_session` is hit. A cap refuse (`verify_cap_per_object` or `verify_cap_per_door_session`) does not end the cycle. The pair's state does not move to `refused`. The submission response is `refuse`. Candidate figures are 3, 3, and 5 (`strict`, `default`, `lenient`). Not locked. Held retries do not count. An unauthenticated rate limit is not this row. A transport rate limit is not this row | `received` |
| `received` | A plant or header `mismatch` is read from the local cache and is not yet confirmed by a fresh read from `{index-url}` that bypasses the local cache | `held` |
| `received` | That same plant or header `mismatch` is confirmed by a fresh read from `{index-url}` that bypasses the local cache. Invariant fast final `refuse`. No thorough run. No grant. `phase` `fast` | `refused` |
| `received` | The object bytes fail to decode as a hike object. Invariant fast final `refuse`. No thorough run. No grant | `refused` |
| `received` | A fast `not read` other than a six-leg read, including an unreadable header or a height or plant the mirror does not hold, when the profile needs that measurement for `provisional` | `held` |
| `received` | Settled by bytes already in hand: fewer than 6 waypoints among `W_1` … `W_16` would be anchored, wherever those waypoints sit, even if every `not read` plant or path read among them came back anchored, or the section 14.6 draw is `not defined` because fewer than six legs exist. Bookend `N` `not defined` is not this row. If a re-read could bring that waypoint count to 6 or more, the measurement is `not read` and the state is `held`, and this row does not apply. A short-byte six-leg draw is not this row. `hike_span` `not read` is not this row. Bookend `S` `not read` is not this row | `refused` |
| `received` | `hike_span` is `not read`. The state enters `held` and the read is retried. Bookend `S` `not read` is not this row | `held` |
| `received` | Rule B10. The submission deadline, decided once for this client submission. `tip_height − last_beacon_height > K`. `phase` `fast`. Reason `submission_deadline`. Not a fake leg. No thorough run. No grant | `refused` |
| `received` | Shape `mismatch`, including `shape_item` `height_range`, seed-value `mismatch`, or waypoint rehash `mismatch` is measured from the object. Waypoint rehash is the recomputed waypoint compared with the stored waypoint. Invariant fast final `refuse`. No thorough run. No grant. Not gated on a lock | `refused` |
| `provisional` | A thorough `not read`, including a short or missing six-leg read, a short-byte six-leg draw, or short path bytes for a six-leg run. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. The service re-verifies | `held` |
| `received` | `provisional: off`. A thorough `not read`, including a short or missing six-leg read, a short-byte six-leg draw, or short path bytes for a six-leg run. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. The service re-verifies | `held` |
| `received` | `provisional` is on, and the fast phase has not left `received` within `fast_accept_bound`. No grant. The state enters `held`. This timeout hold does not return to `provisional` | `held` |
| `received` | `provisional: off`. Thorough decides `accept`; the accept stands | `accepted` |
| `received` | `provisional: off`. Thorough negative. No grant was issued | `refused` |
| `provisional` | `async_catch_budget` is hit before thorough settles. The move from `provisional` to `held` ends the grant. It does not resume. No grant remains. The state enters `held`. This hold does not return to `provisional`. A later thorough refuse is `revoke`, because a grant was issued in the cycle | `held` |
| `provisional` | Thorough decides `accept`; the accept stands | `accepted` |
| `provisional` | Thorough negative. A grant was issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`). `phase` `thorough`. The `revoke` cites `fast_seq` | `revoked` |
| `held` | `provisional` is on, no grant was issued in that admission cycle, and this hold was not entered from `fast_accept_bound` or from `async_catch_budget`. The fast inputs are read on retry | `provisional` |
| `held` | This hold was entered from `fast_accept_bound` or from `async_catch_budget`. Thorough decides `accept`; the accept stands. This hold does not return to `provisional` | `accepted` |
| `held` | This hold was entered from `fast_accept_bound` or from `async_catch_budget`. Thorough negative, and no grant was issued in that cycle. `phase` `thorough`. `fast_seq` is omitted. This hold does not return to `provisional` | `refused` |
| `held` | This hold was entered from `fast_accept_bound` or from `async_catch_budget`. Thorough negative, and a grant was issued in the cycle. A later thorough refuse is `revoke`. `phase` `thorough`. The `revoke` cites `fast_seq`. This hold does not return to `provisional` | `revoked` |
| `held` | This hold was entered from `fast_accept_bound` or from `async_catch_budget`. `reverify_max_attempts` or `max_held_blocks` is hit, whichever comes first. Same budgets, no reset. When both are hit on the same check, the detail is `reverify_max_attempts`. Reason `held_bound`. No grant was issued in that cycle. `phase` is the phase running when the budget ran out. This hold does not return to `provisional` | `refused` |
| `held` | This hold was entered from `fast_accept_bound` or from `async_catch_budget`. `reverify_max_attempts` or `max_held_blocks` is hit, whichever comes first. Same budgets, no reset. When both are hit on the same check, the detail is `reverify_max_attempts`. Reason `held_bound`. A grant was issued in that cycle. `phase` `thorough`. The `revoke` cites `fast_seq`. This hold does not return to `provisional` | `revoked` |
| `held` | Thorough decides `accept`; the accept stands | `accepted` |
| `held` | Thorough negative, and a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`). `phase` `thorough`. The `revoke` cites `fast_seq` | `revoked` |
| `held` | Thorough negative, and no grant was issued | `refused` |
| `held` | A thorough `not read`, including a short or missing six-leg read, a short-byte six-leg draw, or short path bytes for a six-leg run. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. Neither `reverify_max_attempts` nor `max_held_blocks` is hit | `held` |
| `held` | `reverify_max_attempts` or `max_held_blocks` is hit, whichever comes first, and a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`). Reason `held_bound`. Detail names the budget. When both budgets are hit on the same check, the detail is `reverify_max_attempts`. `phase` `thorough`. The `revoke` cites `fast_seq` | `revoked` |
| `held` | No grant was issued, and the budget ran out while the fast phase was still running. Reason `held_bound`. Detail names the budget. When both budgets are hit on the same check, the detail is `reverify_max_attempts`. `phase` `fast`. No thorough line. The line carries the fast record. `fast_seq` is omitted | `refused` |
| `held` | No grant was issued, and the budget ran out while the thorough phase was running. That includes `provisional: off`, and a fast-entered hold that later ran thorough when the budget hit. Reason `held_bound`. Detail names the budget. When both budgets are hit on the same check, the detail is `reverify_max_attempts`. `phase` `thorough`. `fast_seq` is omitted. It is never encoded as JSON `null`. The line carries the thorough record | `refused` |
| `held` | A cache-sourced plant or header `mismatch` is confirmed by a fresh read from `{index-url}` that bypasses the local cache, and no grant was ever issued. Invariant fast final `refuse`. `phase` `fast`. No thorough run. No grant | `refused` |
| `held` | A cache-sourced plant or header `mismatch` is confirmed by a fresh read from `{index-url}` that bypasses the local cache, and a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`). Invariant. `phase` `thorough`. The `revoke` cites `fast_seq`. The reason is the confirmed plant or header `mismatch` | `revoked` |
| `revoked` | Any later check of that same submission | `revoked` |
| `accepted` | The policy version changes. Re-verify the stored object under the new policy version. While that re-verify is `not read`, including after either reverify budget is hit, signature-only sign-in continues on the earlier accept. Thorough decides `accept`; the accept stands. The friendly entry is kept only when this object is already the key's current friendly entry. If a newer accept B later loses its accept, A is not restored. A re-verify of an older object does not displace a newer entry. This re-verify is not a new admission cycle and never issues `provisional` | `accepted` |
| `accepted` | That re-verify refuses under the new policy version. The refuse is a measured `mismatch` or any B-rule outcome other than a B7 cap refuse and other than Rule B10. A `not read` is not this row. The outcome is `revoke`. `phase` `thorough`. The `revoke` cites the new policy version and `fast_seq`. The Gate invalidates the pair's grant before it returns or logs the `revoke`. When this object is the key's current friendly entry, the `revoke` removes that entry by a compare-and-remove on `object_sha256`, only if the current entry's object hash still matches this pair. A re-verify of an older object does not displace a newer entry. If a newer accept B later loses its accept, A is not restored | `revoked` |
| `accepted` | That re-verify is `not read`, and neither `reverify_max_attempts` nor `max_held_blocks` counted from the first `not read` on that re-verify is hit. The earlier accept stands. No suspend. No `held_bound`. Friendly returns continue. The re-verify is retried later | `accepted` |
| `accepted` | That re-verify is still `not read`, and `reverify_max_attempts` or `max_held_blocks` counted from the first `not read` on that re-verify is hit, whichever comes first. The hit is not a `revoke`, does not remove the friendly entry, and does not change sign-in state. Signature-only sign-in continues on the earlier accept. After the hit, the Gate keeps retrying on each later `{index-url}` tip advance: at most one retry per strict increase in that tip height. A jump of several blocks is one retry. An equal or lower tip, including a reorg, is not a retry. These post-budget retries do not restart the spent budget and do not re-arm `held_bound`. The number of such retries is unbounded | `accepted` |
| `accepted` | The state is `accepted`. A listed key signs in again for the same already-accepted object hash. While a later-policy re-verify is `not read`, including after either reverify budget is hit, signature-only sign-in continues on the earlier accept. Only a re-verify that refuses, other than a B7 cap refuse and other than Rule B10, ends that sign-in as a `revoke`, and it removes the friendly entry only when this object is the key's current friendly entry. Section 14 does not run again. Signature-only sign-in requires state `accepted` | `accepted` |
| `refused` | Any later check of that same submission | `refused` |

**Normative.** Policy `refuse`s at once only when the waypoint count is settled by bytes already in hand: fewer than 6 waypoints among `W_1` … `W_16` would be anchored, wherever those waypoints sit, even if every `not read` plant or path read among them came back anchored. If a re-read could bring that count to 6 or more, the measurement is `not read` and the state is `held`. Bookend `N` `not defined` is not, by itself, an admission refuse. Fewer than six legs for the section 14.6 draw is `not defined` from the object's own bytes and is that same refuse-at-once. That draw is not a leg-count floor. `hike_span` `not read` stays `held`. Bookend `S` `not read` is not this hold. A fast `not read`, including an unreadable header, is `held`. A short or missing six-leg read, a short-byte six-leg draw, and short path bytes for a six-leg run are thorough-only. They are `not read`. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. The state is `held` from `received` when `provisional` is off, from `provisional`, or from `held`. They are not a fast `received` to `held` row. The service re-verifies. A short-byte six-leg draw stays re-readable.

A move from `provisional` to `held` ends the grant. It does not resume. No action is allowed while `held`. `held` leaves to `provisional` only when `provisional` is on, no grant was issued in that admission cycle, the hold was not entered from `fast_accept_bound` or from `async_catch_budget`, and the fast inputs are read on retry. A hold entered from `fast_accept_bound` or from `async_catch_budget` does not return to `provisional`. It leaves only to a thorough `accept`, to `refuse`, to `revoke`, or to refuse/revoke with reason `held_bound` when `reverify_max_attempts` or `max_held_blocks` runs out. Those are the same budgets, with no reset. At most one provisional grant is issued in that admission cycle. `held` leaves to `accepted` when thorough decides `accept`, to `revoked` on a negative thorough outcome when a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), or to `refused` on a negative thorough outcome when no grant was issued. `provisional` then `held` then a negative thorough outcome is `revoke`. While neither `reverify_max_attempts` nor `max_held_blocks` is hit, a thorough `not read` is retried and the state is `held`. Exhausting either budget is refuse/revoke with reason `held_bound`: `revoke` if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. It does not stay `held`. That `held` is not a negative settle. `held` for a missing measurement is only for `not read`. Policy `refuse`s at once only when the waypoint count is settled by bytes already in hand: fewer than 6 waypoints among `W_1` … `W_16` would be anchored, wherever those waypoints sit, even if every `not read` plant or path read among them came back anchored. If a re-read could bring that count to 6 or more, the measurement is `not read` and the state is `held`. Bookend `N` `not defined` is not, by itself, an admission refuse. Fewer than six legs for the section 14.6 draw is `not defined` from the object's own bytes and is that same refuse-at-once. That draw is not a leg-count floor. `hike_span` `not read` stays `held`. Bookend `S` `not read` is not this hold. That `not defined` does not go to `held`. A fast `not read`, including an unreadable header, is `held`. A short or missing six-leg read, a short-byte six-leg draw, and short path bytes for a six-leg run are thorough-only. They are `not read`. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. The state is `held` from `received` when `provisional` is off, from `provisional`, or from `held`. They are not a fast `received` to `held` row. A short-byte six-leg draw stays re-readable. A Rule A4.3 checkpoint digest that has not been recomputed does not put an honest object in `held`.

A height or plant the mirror does not hold is `not read`, and the state is `held`. A plant or header `mismatch` read from that mirror is `held` until a fresh read from `{index-url}` that bypasses the local cache confirms the same comparison. `{index-url}` is the upstream. The local mirror is a cache of `{index-url}` responses. An operator's own Bitcoin-built index counts as its `{index-url}`. Anyone may still check that index against Bitcoin. Only a `mismatch` confirmed by that fresh read can `refuse`.

**Normative.** A fake leg is a section 14.6 `mismatch` on a recomputed leg. The gate refuses that submission, and that gate ignores every later submission from that public key. The ignore is per key and that gate only. This file sets no expiry on it. A gate policy may set a duration. Every later submission from that key at that gate gets the same refusal, for any object, with no new draw and no new section 14 run. A cheap-check failure, including a Rule A4.3 checkpoint-digest `mismatch`, a shape `mismatch`, a waypoint rehash `mismatch`, a seed-value `mismatch`, and an `earlier_version` refuse, is a plain refusal. It does not add the key. The ignore record is persistent. A policy-version bump does not clear it and does not open a new cycle for that key. A gate restart does not clear it. A re-verify does not give a free redraw. A cap refuse under Rule B7 is not this ignore. The cap figures are candidates. They are not a ban. A `refuse` or a `revoke` that is not a fake leg still binds to `(public_key, object_sha256)` under that policy version. The same `submission_sha256` does not become `accepted` under that policy version. A resubmit of the same `submission_sha256` returns the original outcome. v1 does not issue a signed receipt. It does not mint a grant. A new submission needs a new challenge. `held` is not a sticky `refuse` and it is not `accept`. The decision-log companion publishes the refuse. This file does not write that log. Cross-gate sharing of the fake-leg record is out of scope.

Signature-only sign-in requires state `accepted`. A listed key may sign in again only for that same already-accepted object hash. While a later-policy re-verify is `not read`, including after either reverify budget is hit, signature-only sign-in continues on the earlier accept. Only a re-verify that refuses, other than a B7 cap refuse and other than Rule B10, ends that sign-in as a `revoke`, and it removes the friendly entry only when this object is the key's current friendly entry. That sign-in does not run section 14 again. A new object hash is a new section 15.1 submission. When the policy version changes, a re-verify that refuses under the new policy version revokes an `accepted` object. That refuse is a measured `mismatch` or any B-rule outcome other than a B7 cap refuse and other than Rule B10. A `not read` is not a refuse and does not revoke. That `revoke` cites the new policy version. While that re-verify is `not read` and neither `reverify_max_attempts` nor `max_held_blocks` counted from the first `not read` on that re-verify is hit, the earlier accept stands. It does not suspend the accept, it does not apply `held_bound`, and friendly returns continue. The re-verify is retried later. Both `reverify_max_attempts` and `max_held_blocks` for that re-verify are counted from the first `not read` on that re-verify. `max_held_blocks` starts at the Gate’s `{index-url}` tip height at that first `not read`. It is hit when a retry finds that tip ≥ start + `max_held_blocks` and the re-verify is still `not read`. `reverify_max_attempts` is hit when that many retries of this re-verify, counted from that first `not read`, are still `not read`. Before either budget is hit, the Gate retries within the existing reverify budget. After either budget is hit, the hit is not a `revoke`, does not remove the friendly entry, and does not change sign-in state. Signature-only sign-in continues on the earlier accept. After the hit, the Gate keeps retrying on each later `{index-url}` tip advance: at most one retry per strict increase in that tip height. A jump of several blocks is one retry. An equal or lower tip, including a reorg, is not a retry. These post-budget retries do not restart the spent budget and do not re-arm `held_bound`. The number of such retries is unbounded. A re-verify that refuses, other than a B7 cap refuse and other than Rule B10, is a `revoke` and removes the friendly entry only when this object is the key's current friendly entry. When that re-verify decides `accept`, the entry is kept only when this object is already the key's current friendly entry. If a newer accept B later loses its accept, A is not restored. A re-verify of an older object does not displace a newer entry. Only a thorough `accept` of a newer object replaces the entry. Newer is the decision-log `seq` of that thorough `accept`. A higher `seq` is newer. The replaced pair's logged decision stays unchanged. This file does not write that log. An index outage does not revoke that accepted object. Thorough decides `accept` and the accept stands.

#### Rule B9. Invariant refuses

**Normative.** These outcomes are not Rule B0–B8 candidates. They do not wait on a configured threshold. A `revoke` in this list is `phase` `thorough`. A grant means the fast phase already finished. `fast_seq` is omitted when the line does not cite one. It is never encoded as JSON `null`.

- The object bytes fail to decode as a hike object. Invariant fast final `refuse`. No thorough run. No grant. `phase` `fast`.
- Waypoint rehash is the recomputed waypoint compared with the stored waypoint. That `mismatch` is an invariant fast final `refuse`. No thorough run. No grant. `phase` `fast`.
- Shape `mismatch`, including `shape_item` `height_range`, and seed-value `mismatch`, are invariant fast final `refuse`s. No thorough run. No grant. `phase` `fast`.
- A plant or header `mismatch` confirmed by a fresh read from `{index-url}` that bypasses the local cache is an invariant. No grant was ever issued: `refuse`, `phase` `fast`, no thorough run. A grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`): `revoke`, `phase` `thorough`, and the `revoke` cites `fast_seq`. The reason is the confirmed plant or header `mismatch`.
- A checkpoint-digest `mismatch` is an invariant. The thorough phase measures it. No grant was ever issued: `refuse`, `phase` `thorough`. A grant was ever issued: `revoke`, `phase` `thorough`, and the `revoke` cites `fast_seq`. It is not a candidate `refuse`. It is a cheap-check failure. It does not add the public key to the fake-leg ignore record.
- A six-leg comparison of `mismatch` is an invariant. The thorough phase measures it. No grant was ever issued: `refuse`, `phase` `thorough`. A grant was ever issued: `revoke`, `phase` `thorough`, and the `revoke` cites `fast_seq`. It is not a candidate `refuse`. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. That is a thorough `not read`. The state is `held`. That `mismatch` is a fake leg. Rule B7 and section 15.5 then ignore later submissions of that public key at that gate.
- A cap hit is not in this list. Rule B7 states it. It applies only when the profile sets that figure.
- When the section 14 record’s `version` is `earlier version`, the admission service refuses with reason `earlier_version`. That reason is this policy. It is not a section 14 token. Verify does not record it. No thorough run. No grant. `phase` `fast`.

### 15.3 Two-phase async

1. **Fast phase.** The fast list is the section 14.1–14.5 measurements, and the cheap checks include the Rule A4.3 checkpoint digest on every checkpoint: shape, the seed, waypoint and fold recompute from stored checkpoint bytes, path and plant comparison, bookends, and the anchored count, plus the signature. Header and plant facts come from the service's local mirror of the portable index. `{index-url}` is the upstream. The local mirror is a cache of `{index-url}` responses and serves the section 9 body rules. The mirror holds only `{index-url}` responses. It is filled from the portable index only. A height or plant the mirror does not hold is `not read`, and the state is `held`. The fast phase does no other network read. A plant or header `mismatch` read from that mirror is `held` until a fresh read from `{index-url}` that bypasses the local cache confirms the same comparison. An operator's own Bitcoin-built index counts as its `{index-url}`. Anyone may still check that index against Bitcoin. Only a `mismatch` confirmed by that fresh read can `refuse`. The fast checkpoint-binding checks are the fold and the waypoint rehash under section 12.2 from stored checkpoints. The fast phase never recomputes a leg. Leg recompute stays thorough-only. The fast phase does not turn a partial view into the finished verify record. Branch on phase. There is no new token for the phase. The Rule A4.3 checkpoint digest is a cheap check on every checkpoint. It hashes the stored sample. It does not recompute a leg. The default policy runs it with the other cheap checks. It is not thorough-only. The fast record is the section 14 record with `six_leg` and `six_leg_runs` omitted. `checkpoints[*].legs[*].result` stays, because the Rule A4.3 digest is a cheap check. Every other section 14 key stays, including `shape_item`, `checkpoints[*].g`, `checkpoints[*].legs[*].j`, `waypoints[*].result`, `seed_value`, `paths`, `anchors`, the bookend keys, and `analysis`. Omitted keys are absent. They are not filled with `not defined`. The full section 14 record, after thorough, has every key present, and `six_leg_runs` holds the thorough draws. Both records are encoded with RFC 8785. The fast record has no placeholders. It does not fill an omitted key with `not defined`. It is not the finished record. The fast decision token is `provisional`. A fast-phase `accept` is invalid. The fast phase never issues `accept`. It may issue a final `refuse` when a fast rule fails on the object's own bytes: decode (the object bytes fail to decode as a hike object), waypoint rehash, shape `mismatch`, including `shape_item` `height_range`, seed-value `mismatch`, a plant or header `mismatch` confirmed by a fresh read from `{index-url}` that bypasses the local cache, an object-settled refuse-at-once, or a cap on a signed submission. Rule B10 is also that final `refuse` when `tip_height − last_beacon_height > K`. That `refuse` does not start a thorough run and does not issue a grant. Otherwise the fast phase gives `provisional`. A fast-phase `not read` is `held`. When profile mode `provisional` is `on`, it returns within `fast_accept_bound`. That bound is a profile parameter. No duration is locked. When `provisional` is `off`, the fast phase does not grant `provisional`. `provisional: off` is that mode. It is not a value of `fast_accept_bound`.
2. **Held.** `held` for a missing measurement is only for `not read`. When a fast measurement the active profile needs for `provisional` is `not read`, the fast phase does not issue `provisional` and does not record a sticky `refuse`. The state is `held`. Policy `refuse`s at once only when the waypoint count is settled by bytes already in hand: fewer than 6 waypoints among `W_1` … `W_16` would be anchored, wherever those waypoints sit, even if every `not read` plant or path read among them came back anchored. If a re-read could bring that count to 6 or more, the measurement is `not read` and the state is `held`. Bookend `N` `not defined` is not, by itself, an admission refuse. Fewer than six legs for the section 14.6 draw is `not defined` from the object's own bytes and is that same refuse-at-once. That draw is not a leg-count floor. `hike_span` `not read` stays `held`. Bookend `S` `not read` is not this hold. That `not defined` does not go to `held`. A fast `not read`, including an unreadable header, is `held`. A short or missing six-leg read, a short-byte six-leg draw, and short path bytes for a six-leg run are thorough-only. They are `not read`. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. The state is `held` from `received` when `provisional` is off, from `provisional`, or from `held`. They are not a fast `received` to `held` row. A short-byte six-leg draw stays re-readable. A missing Rule A4.3 checkpoint digest is not that `not defined` case: the digest is not yet run. A height or plant the mirror does not hold is `held`, as section 15.2 states. A cache-sourced plant or header `mismatch` is `held` until a fresh read from `{index-url}` that bypasses the local cache confirms it. `{index-url}` is the upstream. The local mirror is a cache of `{index-url}` responses. An operator's own Bitcoin-built index counts as its `{index-url}`. Anyone may still check that index against Bitcoin. The fast phase must not issue `provisional` on a floor that is `not read` or `not defined`. A fast-phase `not read` is `held`. A plant `mismatch` confirmed by a fresh read from `{index-url}` that bypasses the local cache is a fast final `refuse`: no thorough run and no grant.
3. **Thorough phase.** The thorough phase is the leg sample in section 15.5: 18 random legs as 3 rounds of 6, each round’s 6 legs run in parallel. The Rule A4.3 digest is a cheap check and is not this phase. Leg recompute (section 14.6) is thorough-only. A short or missing six-leg read, a short-byte six-leg draw, and short path bytes for a six-leg run are that thorough `not read`. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. The state is `held` from `received` when `provisional` is off, from `provisional`, or from `held`. They are not a fast `received` to `held` row. Only the thorough phase issues `accept`. The fast phase never issues `accept`. The thorough phase issues `refuse` or `revoke` by the grant-ever-issued rule. A fast final `refuse` is the case named in the fast phase. Thorough outcomes are: thorough decides `accept` and the accept stands (`accepted`); `revoke`; `refuse`; or `held`. `held` is a thorough `not read` or a retry. It is not a negative settle. A negative thorough outcome, or a failed policy-version re-verify, is `revoke` if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. `provisional` then `held` then a negative thorough outcome is `revoke`. A later `revoke` cites a new policy version or a new measurement. `held` is an HTTP response outcome. It is not a decision-log event. If the object was `provisional`, a move to `held` ends the grant. It does not resume. A grant was issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`). A later negative thorough outcome is `revoke`. It is `refuse` only when no grant was issued.
4. **Provisional.** The fast-phase outcome is `provisional`, a reversible grant, until thorough settles. It is never `accept`. A `provisional` state grants no action (`provisional_action_cap`). No irreversible action is allowed while `provisional`, in every profile. That value is fixed in this file. It is not a profile knob. A `provisional` grant is short, read-only access until thorough settles or `async_catch_budget` is hit. The friendly-list entry is added only at a thorough `accept`. The window before thorough settles is the attacker surface of a provisional grant. A provisional grant is not a finished admission. The residual risk is QR relay phishing: a victim scans a QR that an attacker relayed. **Informative.** Although a `provisional` state grants no action, a forged object can still reach `provisional` for free: the fast phase has no leg, and the plant gap is record-only. That grant is short, read-only access until thorough settles or `async_catch_budget` is hit. `verify_cap_per_door_session` is the main limit.

**Held bound.** Normative. `reverify_max_attempts` and `max_held_blocks` are profile inputs. Candidates are 3 and 6. The numbers are not locked. `max_held_blocks` applies to every hold, whatever the cause: a fast `not read`, a thorough `not read`, `fast_accept_bound`, or `async_catch_budget`. It starts at the Gate’s `{index-url}` tip height at the first entry into `held` in the cycle. It does not reset. That tip height is the normative clock. An append-only hash-chained log entry may record that tip height as `bitcoin_tip_height`. That name is the log entry's name for the tip. The entry is unsigned. It is not a signed head. v1 has no signed head and no signed receipt. This file does not define a second clock. It is hit when a check finds that tip ≥ start + `max_held_blocks`, even when no `not read` has occurred. `reverify_max_attempts` counts `not read` retries only, fast or thorough. A hold with no `not read`, including `fast_accept_bound` and `async_catch_budget`, does not count toward it. It is hit when that many retries are still `not read`. Whichever budget is hit first, the outcome is refuse/revoke with reason `held_bound`. The detail names the budget that hit, `reverify_max_attempts` or `max_held_blocks`. This file names that detail. It does not write the decision-log JSON. Exhausting either budget is `revoke` if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. It does not stay `held`. `phase` is the phase running when the budget ran out, and the line carries that phase's record. No grant was issued and the fast phase was still running: the outcome is `refuse`, reason `held_bound`, the detail names the budget that hit, `phase` `fast`, no thorough line, and the line carries the fast record. No grant was issued and the thorough phase was running, including `provisional: off` and a fast-entered hold that later ran thorough: the outcome is `refuse`, reason `held_bound`, the detail names the budget that hit, `phase` `thorough`, `fast_seq` is omitted, and the line carries the thorough record. `fast_seq` is omitted. It is never encoded as JSON `null`. A grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`): the outcome is `revoke`, `phase` `thorough`, and that `revoke` cites `fast_seq`. Every `revoke` is `phase` `thorough`. When both `reverify_max_attempts` and `max_held_blocks` are hit on the same check, the detail is `reverify_max_attempts`. This file does not write the line. After a grant was issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), a `held_bound` outcome is `revoke`, not `refuse`. That `revoke` binds to `(public_key, object_sha256)`. It does not end another object's grant. A `revoke` in that object's own admission cycle ends that pair's grant and any signature-only sign-in for the object. A later-policy re-verify that refuses, other than a B7 cap refuse and other than Rule B10, is a `revoke` and removes the friendly entry only when this object is the key's current friendly entry, by a compare-and-remove on `object_sha256`, only if the current entry's object hash still matches this pair. A `not read` does not.

Which named checks are fast and which are thorough is the profile schema in section 15.4. The fast list does not include a leg.

The hit rule, the `{index-url}` tip clock, and the phase of a `held_bound` line are the Held bound paragraph above. An append-only hash-chained log entry may record that tip height as `bitcoin_tip_height`. That name is the log entry's name for the tip. The entry is unsigned. It is not a signed head. v1 has no signed head and no signed receipt. That `held` is not a negative settle. The fast phase itself does not `refuse` for a `not read` gap, or for a height or plant the mirror does not hold. Policy `refuse`s at once only when the waypoint count is settled by bytes already in hand: fewer than 6 waypoints among `W_1` … `W_16` would be anchored, wherever those waypoints sit, even if every `not read` plant or path read among them came back anchored. If a re-read could bring that count to 6 or more, the measurement is `not read` and the state is `held`. Bookend `N` `not defined` is not, by itself, an admission refuse. Fewer than six legs for the section 14.6 draw is `not defined` from the object's own bytes and is that same refuse-at-once. That draw is not a leg-count floor. `hike_span` `not read` stays `held`. Bookend `S` `not read` is not this hold. That `not defined` does not go to `held`. A short-byte six-leg draw stays re-readable. The state is `held`. The service re-verifies.

### 15.4 Named parameters and profile schema

A policy profile names parameters and lists its fast checks and its thorough checks. The fast list is the cheap checks: shape, header match, the Rule A4.3 checkpoint digest on every checkpoint, the seed, waypoint and fold recompute from stored checkpoints, path and plant comparison, bookends, the anchored count, and the signature. The thorough list is the section 15.5 leg sample. The fast checkpoint-binding checks are the fold and the waypoint rehash under section 12.2 from stored checkpoints. The Rule A4.3 checkpoint digest is a cheap check on every checkpoint. It hashes the stored sample. It does not recompute a leg. The default policy runs it with the other cheap checks. It is not thorough-only. The fast list never recomputes a leg. A profile supplies values. This section does not set them. A `provisional` state grants no action (`provisional_action_cap`). No irreversible action is allowed while `provisional`, in every profile. That value is not a profile knob. `revoke_scope` is the pair `(public_key, object_sha256)`. `fast_checkpoint_sample` is moot: the fast checkpoint-binding checks are the fold and the waypoint rehash under section 12.2, and the Rule A4.3 checkpoint digest is a cheap check on every checkpoint.

| Name | Rule | What the name stands for |
|---|---|---|
| `leg_rate_floor` | B2 | A ratio `p/q` in checkpoints per second. Compared in integers on `(hike_legs, hike_span)`. The value is TBD. Section 15.5 gives the measuring method and a placeholder `1/5` that is not ratified. |
| `submission_deadline_blocks` | B10 | `K`, in blocks. Placeholder 3, not ratified. The fast-phase check is Rule B10. Decided once per client submission. |
| `span_sanity_bound` | B3 | Not an admission input. Rule B3 does not adjust `hike_span`. |
| `span_scale_seconds` | B3 | Not an admission input. A span adjustment is not applied to bookend `S` and is not applied to `hike_span`. |
| `span_endpoint_mode` | B3 | Not an admission input. |
| `fixed_start` | B6 | The verify record may still hold `fixed_start`. The switch is not an admission input. The admission service does not read it for the rate. |
| `min_anchored_paths` | B4 | Settled integer 6. The comparison is `anchored_total ≥ 6`, anywhere among `W_1` … `W_16`. No first-planted rule. No last-planted rule. |
| `min_last_bookend` | B4 | Withdrawn. There is no last-planted rule. |
| `plant_gap_ceiling` | B5 | Record-only candidate. Not a fast final `refuse`. A configured threshold on this ceiling is not the plant-`mismatch` fast `refuse`. |
| `plant_gap_floor` | B5 | Candidate integer. The comparison is `d_g >= plant_gap_floor`. The candidate value is in section 15.5. |
| `thorough_rounds` | B7 | Default 3. One draw of up to 18 distinct indices, split into rounds of 6, and each round’s 6 run in parallel. Settled. The count is policy. Section 15.5. The v2.0 name `thorough_six_leg_runs` and its candidates 15, 8, and 8 are withdrawn. |
| `verify_cap_per_object` | B7 | Candidate compute limit of 1 per `(public_key, object_sha256, policy_version)`. Not locked. Not a ban. A hit's refuse is final for that `submission_sha256` only. It does not change the pair's state or friendly entry. It is not the pair's thorough decision. A cap refuse (`verify_cap_per_object` or `verify_cap_per_door_session`) does not end the cycle. Service-started re-verifies and `held` retries are not in the count. The same object under a new policy version may be verified again. When the profile sets this figure, a hit on a valid signature and a fresh challenge is a fast final `refuse` of that attempt. |
| `verify_cap_per_door_session` | B7 | Candidate 3, 3, and 5 (`strict`, `default`, `lenient`). Not locked. Counts client submissions in the session. Held retries do not count. Bounds the fast verifies and the thorough runs those submissions trigger. Not a cap on a website session. A transport rate limit stays outside this policy. A hit stays in `received`. The submission response is `refuse`. A cap refuse does not end the cycle. |
| `provisional_action_cap` | 15.3 | A `provisional` state grants no action. No irreversible action while `provisional`, in every profile. Not a profile knob. |
| `revoke_scope` | 15.2 | Fixed `(public_key, object_sha256)`. A `revoke` binds to that pair. It does not end another object's grant. A `revoke` in that object's own admission cycle ends that pair's grant and any signature-only sign-in for the object. A later-policy re-verify that refuses, other than a B7 cap refuse and other than Rule B10, is a `revoke` and removes the friendly entry only when this object is the key's current friendly entry, by a compare-and-remove on `object_sha256`, only if the current entry's object hash still matches this pair. A `not read` does not. |
| `async_catch_budget` | 15.3 | Thorough-phase budget. No number is locked. |
| `provisional` | 15.3 | Profile mode `on` or `off`. `off` means no `provisional` state. It is not a value of `fast_accept_bound`. |
| `fast_accept_bound` | 15.3 | Fast-phase budget when `provisional` is `on`. No number is locked. Unused when `provisional` is `off`. |
| `reverify_max_attempts` | 15.3 | Profile input. Counts `not read` retries only, fast or thorough. A hold with no `not read`, including `fast_accept_bound` and `async_catch_budget`, does not count toward it. It is hit when that many retries are still `not read`. Whichever of this budget and `max_held_blocks` is hit first is refuse/revoke with reason `held_bound`. The detail names the budget that hit. When both budgets are hit on the same check, the detail is `reverify_max_attempts`. Exhausting it is `revoke`, `phase` `thorough`, if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. After a grant was issued the outcome is `revoke`, not `refuse`. Candidate 3. No number is locked. |
| `max_held_blocks` | 15.3 | Profile input. Applies to every hold, whatever the cause: a fast `not read`, a thorough `not read`, `fast_accept_bound`, or `async_catch_budget`. Starts at the Gate’s `{index-url}` tip height at the first entry into `held` in the cycle. It does not reset. An append-only hash-chained log entry may record that tip as `bitcoin_tip_height`. That name is the log entry's name for the tip. The entry is unsigned. It is not a signed head. v1 has no signed head and no signed receipt. Hit when a check finds that tip ≥ start + `max_held_blocks`, even when no `not read` has occurred. Whichever of this budget and `reverify_max_attempts` is hit first is refuse/revoke with reason `held_bound`. The detail names the budget that hit. Exhausting it is `revoke` if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. Candidate 6. No number is locked. |
| `no_pair_outcome` | B0 | Binds to `(public_key, object_sha256)`. When `hike_span` is `not read`, the state enters `held` from `received`, from `provisional`, or from `held`, and the read is retried. Bookend `S` `not read` is not this hold. Bookend `N` `not defined` is not an admission refuse. When fewer than 6 waypoints among `W_1` … `W_16` would be anchored, wherever those waypoints sit, even if every `not read` plant or path read among them came back anchored, the outcome is `refuse` at once. That result does not go to `held`. If a re-read could bring that count to 6 or more, the measurement is `not read` and the outcome is `held`. A short or missing network read is `not read`, the outcome is `held`, and the service re-verifies. |

`verify_cap_per_object` is a candidate of 1, keyed on `(public_key, object_sha256, policy_version)`. It is a compute limit, not a ban. A hit's refuse is final for that `submission_sha256` only. It does not change the pair's state or friendly entry. It is not the pair's thorough decision. A cap refuse (`verify_cap_per_object` or `verify_cap_per_door_session`) does not end the cycle. Service-started re-verifies and `held` retries are not in the count. A new policy version is a new count. The figure is not locked.

#### Rule B0. Which rate the admission service reads

**Normative.** The admission service reads `(hike_legs, hike_span)` from Rule A4.10. `leg_rate = hike_legs / hike_span`. That rate is the admission rate. The bookend pair `(N, S)` is a section 14 measurement. It is not this rate. `whole_hike_legs` and `whole_hike_span` stay analysis. When `hike_span` is `not read`, there is no numeric rate. The state is `held` and the read is retried (`no_pair_outcome`). Bookend `S` `not read` is not this hold. Bookend `N` `not defined` is not, by itself, an admission refuse. When fewer than 6 waypoints among `W_1` … `W_16` would be anchored, wherever those waypoints sit, even if every `not read` plant or path read among them came back anchored, a re-read cannot change that result. Policy `refuse`s it at once. It does not go to `held`. If a re-read could bring that count to 6 or more, the measurement is `not read` and the state is `held`. A short or missing network read is `not read`. The state is `held`. The service re-verifies.

#### Rule B1. Integrity

**Candidate, not locked.** The bullets below are the remaining candidates: thresholds, floors, and the policy counts. Those refuses are Rule B9. They are not these bullets. `not read` is a technical outcome of an index document that is missing or unreadable, or of a header `T` that is not taken. It is not an automatic `refuse`. On the fast path it is `held` when the profile needs the measurement for `provisional`. A height or plant the mirror does not hold is that `not read`. On the thorough path a `not read` is `held` and is retried. When `reverify_max_attempts` or `max_held_blocks` is hit, whichever comes first, the outcome is `revoke`, `phase` `thorough`, if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. The reason token is `held_bound`. The detail names the budget that hit. When both budgets are hit on the same check, the detail is `reverify_max_attempts`. Policy `refuse`s at once only when the waypoint count is settled by bytes already in hand: fewer than 6 waypoints among `W_1` … `W_16` would be anchored, wherever those waypoints sit, even if every `not read` plant or path read among them came back anchored. If a re-read could bring that count to 6 or more, the measurement is `not read` and the state is `held`. Bookend `N` `not defined` is not, by itself, an admission refuse. Fewer than six legs for the section 14.6 draw is `not defined` from the object's own bytes and is that same refuse-at-once. That draw is not a leg-count floor. `hike_span` `not read` stays `held`. Bookend `S` `not read` is not this hold. That `not defined` does not go to `held`. A fast `not read`, including an unreadable header, is `held`. A short or missing six-leg read, a short-byte six-leg draw, and short path bytes for a six-leg run are thorough-only. They are `not read`. When the six-leg draw is `not read`, `six_leg_runs` is `not read`. The state is `held` from `received` when `provisional` is off, from `provisional`, or from `held`. They are not a fast `received` to `held` row. A short-byte six-leg draw stays re-readable. A cache-sourced plant or header `mismatch` is `held` until a fresh read from `{index-url}` that bypasses the local cache confirms it. `{index-url}` is the upstream. The local mirror is a cache of `{index-url}` responses. An operator's own Bitcoin-built index counts as its `{index-url}`. Anyone may still check that index against Bitcoin. A plant or header `mismatch` confirmed by that fresh read is an invariant in section 15.2. No grant was ever issued: the outcome is `refuse`, `phase` `fast`. A grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`): the outcome is `revoke`, `phase` `thorough`, and the `revoke` cites `fast_seq`. `not defined` is not `not read`.

- Candidate, not locked. An `accepted` submission uses mainnet headers and mainnet waypoints. A `match` on a document whose `network` is `synthetic` is not a mainnet fact. Section 14 records `plant` `mismatch` when a returned document’s `network` is neither `main` nor `synthetic`.
- Shape `mismatch`, including `shape_item` `height_range`, seed-value `mismatch`, waypoint rehash `mismatch`, a confirmed header `mismatch`, a confirmed plant `mismatch`, a checkpoint-digest `mismatch`, and a six-leg `mismatch` are the invariant refuses in section 15.2. They are not candidates. An unconfirmed cache-sourced plant or header `mismatch` stays `held`. A document that was read and has no plant for that block is `plant` `mismatch`.
- Candidate, not locked. `index_comparison` `not read`, `T` `not read`, and `plant` `not read` are not, by themselves, a candidate `refuse`. The candidate is re-verify. `plant` `not read` is only a document that was not returned or could not be read.
- Candidate, not locked. A six-leg draw recorded as `not defined` because fewer than six legs exist is settled by the object's own bytes. Policy `refuse`s it at once. A short-byte six-leg draw is thorough-only. It is `not read`. The state is `held` from `received` when `provisional` is off, from `provisional`, or from `held`. It is not a fast `received` to `held` row. The service re-verifies.
- A `skip` is not, by itself, a `refuse`. Rule B4 is the paths comparison: `anchored_total ≥ 6`.
- Candidate, not locked. `profile` `conformance-small` is not offered to the production admission service.

#### Rule B2. Leg-rate floor

**Normative comparison. The value is TBD.** `leg_rate_floor` is a profile parameter `p/q` in checkpoints per second, `hike_legs` over `hike_span`, with `hike_span` in Unix seconds. One checkpoint is one leg. This file sets no ratified value for `p/q`. Section 15.5 states the measuring method. A test placeholder of `1/5` is placeholder, not ratified. The comparison is integer-only: a ratio `p/q` meets the floor when `hike_span` is an integer greater than 0 and `q · hike_legs ≥ p · hike_span`. `hike_span` ≤ 0 means the rate is undefined. The outcome is `refuse`. It is not `held`. `held` is only `hike_span` `not read`. The profile supplies `p` and `q`. The comparison does not round. Rule B3 does not change `hike_span`. There is no comparison when `hike_span` is `not read`. The fast phase does not issue `provisional` on that gap. The state is `held`. A short or missing network read stays `not read` and the state is `held`. A numeric rate that falls short of the floor is `refuse`. The bookend pair is not an input to this comparison.

#### Rule B10. Submission deadline

**Normative. The bound is placeholder, not ratified.** This check is fast-phase. It is decided once per client submission. `tip_height` is the section 15.3 Gate `{index-url}` tip clock, the same clock as the held bound. The gate reads it fresh at receipt, bypassing the local mirror and its cache. It is not a height stored on the object. A failed read or a stale read is `not read`. The state is `held`. That hold is never a pass. When several index sources are configured, `tip_height` is the highest tip among those that were read. `last_beacon_height` is `last_interrupt_height`, the height of the last beacon. The deciding tip is the first successful tip read for that submission: the receipt read, or, when that receipt was `not read`, the first retry that reads a tip, bounded by `max_held_blocks`. Held retries reuse that tip and that decision. A service-started re-verify reuses that tip and that decision. A policy-version bump does not decide this rule again and never revokes on it. A client resubmission is a new receipt, and this rule is decided again for that receipt. When the deciding `tip_height` is below `last_beacon_height`, the receipt is not yet at the last beacon. The state is `held`. That hold is not a `refuse`. When the deciding `tip_height` is an integer greater than or equal to `last_beacon_height` and `tip_height − last_beacon_height > K`, the gate `refuse`s. `phase` is `fast`. The reason token is `submission_deadline`. The state moves from `received` to `refused`. That refuse is not a fake leg and does not add the key to the ignore record. A lower tip after a reorg does not reopen that refuse. `K` is the profile parameter `submission_deadline_blocks`, in blocks. This file sets no ratified value for `K`. A test placeholder of `K = 3` is placeholder, not ratified. With that placeholder, a gap of 3 is not refused by this rule and a gap of 4 refuses. A missing tip read is `not read` and the state is `held`. It is not this refuse.

#### Rule B3. Span sanity

**Normative.** Rule B3 is not the admission rate and is not an admission input. The v2.0 candidate `S_eff = max(S, delta_h × span_scale_seconds)` is withdrawn. No span adjustment is applied to bookend `S` for admission, and none is applied to `hike_span`. `delta_h` stays the Rule A4.8 measurement. A later policy that names a span adjustment must name it against `hike_span`, not against bookend `S`. This file does not name one.

#### Rule B4. Paths minimum

**Normative. Settled 21 Oct 2026.** `anchored_total ≥ 6`. The 6 waypoints sit anywhere among `W_1` … `W_16`. There is no first-planted rule and no last-planted rule. `W_17` is not one of the 16. `min_last_bookend` is withdrawn. The v2.0 candidates 8, 7, and 7 are withdrawn. The count is Rule A4.5. A count settled below 6 by bytes already in hand is `refuse` at once. If a re-read could bring the count to 6 or more, the measurement is `not read` and the state is `held`.

#### Rule B5. Plant gap

**Candidate, not locked.** `plant_gap_ceiling` stays record-only. `d_g` is recorded. This rule does not `refuse` in the fast phase. A plant `mismatch` from a fresh read from `{index-url}` that bypasses the local cache is a fast final `refuse`. This plant-gap ceiling is not that `refuse`. The fast phase does not recompute a leg. The named lower bound is `plant_gap_floor`: `d_g >= plant_gap_floor`. `plant_gap_floor` is a candidate integer in the same unit as `d_g`, a difference of block heights, and the candidate is 1 block. No value is locked.

#### Rule B6. Fixed start

**Normative.** `fixed_start` is not an admission input. Section 14 may still record `fixed_start.N`, `fixed_start.S`, and `fixed_start.delta_h` when bookends are defined. The admission service does not read those three for the rate, for the anchor comparison, or for any other admission comparison. Part A still records the actual `a`, `b`, `N`, and `S`. A `fixed_start` field of `not read` or `not defined` does not, by itself, `refuse` or hold the submission.

#### Rule B7. Resubmits

**Candidate, not locked.** `verify_cap_per_object` is a candidate of 1 per `(public_key, object_sha256, policy_version)`. Not locked. It is a compute limit, not a ban. A hit's refuse is final for that `submission_sha256` only. It does not change the pair's state or friendly entry. It is not the pair's thorough decision. A cap refuse (`verify_cap_per_object` or `verify_cap_per_door_session`) does not end the cycle. Service-started re-verifies and `held` retries are not in the count. The same object under a new policy version may be verified again. Candidate `verify_cap_per_door_session` is 3, 3, and 5 (`strict`, `default`, `lenient`). Not locked. It counts client submissions in the session, so it bounds the fast verifies and the thorough runs those submissions trigger. Held retries are Gate-internal and do not count. It is not a cap on a website session. A transport rate limit stays outside this policy. These caps are compute limits. They do not ban a key. They are not the fake-leg ignore.

**Normative. Settled 21 Oct 2026.** The default policy recomputes 18 distinct legs, drawn once and split into 3 rounds of 6, in that draw order. Each round’s 6 legs run in parallel. The round count is the policy parameter `thorough_rounds`. The default is 3. Section 15.5 is that rule, including the case `6 ≤ leg_total < 18`. A fake leg, a section 14.6 `mismatch`, refuses that submission. That gate then ignores every later submission from that public key, with no expiry in this file, on the gate’s persistent per-key record. A cheap-check failure does not. The caps above are not this ignore. Cross-gate sharing of the record is out of scope. The v2.0 candidates of 15, 8, and 8 draws (90, 48, and 48 leg results) are withdrawn.

**Normative.** When a profile sets a figure, a hit on `verify_cap_per_object` or `verify_cap_per_door_session`, on a valid signature and a fresh challenge, is a fast final `refuse` of that attempt. A `verify_cap_per_object` refuse is final for that `submission_sha256` only. It does not change the pair's state or friendly entry. It is not the pair's thorough decision. A cap refuse (`verify_cap_per_object` or `verify_cap_per_door_session`) does not end the cycle. A `verify_cap_per_door_session` hit stays in `received`. The submission response is `refuse`. Candidate `verify_cap_per_door_session` is 3, 3, and 5 (`strict`, `default`, `lenient`). Not locked. It counts client submissions in the session, so it bounds the fast verifies and the thorough runs those submissions trigger. Held retries are Gate-internal and do not count. These are compute limits. They do not ban the key. The hit does not start a thorough run and does not issue a grant. An unauthenticated rate limit is not this rule. A transport rate limit stays outside this policy.

#### Rule B8. Leg count

**Normative.** There is no minimum or maximum leg count as policy. There is no per-stage admission minimum and no whole-hike leg-count minimum. `leg_count ≥ 1` per stage follows from Rule A1.1 and is recorded under shape (Rule A4.1). It is the section 13.2 loop. It is not an admission threshold. A shape `mismatch` is Rule B9. It does not delete the draw rule below. The section 14.6 token `not defined`, when fewer than six checkpoints exist, is the draw’s own token. It is not a leg-count floor. The default sample size of 18 is a count of legs to recompute. It is not a leg-count floor. When `leg_total` < 6 the section 14.6 draw is `not defined` and the admission service refuses at once. When `leg_total` ≥ 18 the default policy draws 18 distinct global indices once, by the section 14.6 unbiased draw, discarding an index already chosen, and splits that sequence into 3 rounds of 6 in draw order. The 6 indices of one round are distinct. No index repeats across the 18. When `6 ≤ leg_total < 18` the same draw continues for 3 rounds of 6. An index is not repeated while an unused index remains. When no unused index remains, a later draw may repeat an index, and it still discards an index already chosen in the current round. The 6 indices of the current round stay distinct. Each index is one section 14.6 unbiased draw. A discarded candidate is not used and the draw continues. A `mismatch` refuses that submission. The gate does not start the next round. The index draw is one sequence. The rounds are that sequence split into sixes. They are not three independent draws.

### 15.5 Default policy and the floor

**Normative for the default policy. The floor value is TBD.**

Cheap checks run on every checkpoint and every waypoint. Those checks are shape, header match, the seed, the Rule A4.3 checkpoint digest on every checkpoint, and the section 12.2 fold and waypoint rehash. The Rule A4.3 digest hashes the stored sample. It does not recompute a leg. A cheap-check failure refuses the submission and does not add the public key to the ignore record. After the cheap checks, the gate draws distinct global indices once and runs them as 3 rounds of 6. When `leg_total` ≥ 18 the draw is 18 distinct indices, split in draw order into rounds `[0:6]`, `[6:12]`, and `[12:18]`. When `6 ≤ leg_total < 18` the draw still fills 3 rounds of 6, indices stay distinct while an unused index remains, and a later draw may repeat an index only after every index has been used. The current round’s 6 stay distinct. When `leg_total` < 6 the draw is `not defined` and the gate refuses. The 6 legs of a round run in parallel. The round count is policy. The parameter is `thorough_rounds`. The default is 3. The v2.0 candidates of 15, 8, and 8 draws (90, 48, and 48 leg results) are withdrawn. A `mismatch` in a round refuses that submission. The gate does not start the next round. Legs of the current round that are already running finish only for the record; their results do not turn the refuse into an accept.

**Normative.** The current group of 6 is the indices in `phase` from index `6` times the number of complete groups already in `phase`, through the end. It is empty when the length of `phase` is a multiple of 6, including at the start. A candidate already in that group is discarded. The draw has not stopped while `phase` holds fewer than 18 indices and the section 14.6 byte source still yields a candidate. The draw stops when `phase` holds 18 indices. The draw also stops when that byte source is exhausted before 18 indices. Exhaustion is `not read`. Rounds already filled stay recorded. The unfinished group is not a round. The state is `held`. It is not a `mismatch` refuse.

```text
if leg_total < 6:
    the draw is not defined
    refuse
else:
    phase = []
    # one draw, then split. Not three independent draws.
    while phase has fewer than 18 indices and the draw has not stopped:
        # stopped: 18 indices are in phase, or the byte source is exhausted before 18
        q = one section 14.6 unbiased draw from 0 … leg_total − 1
        if q is already in the current group of 6:
            discard q and continue
        if q is already in phase and some index in 0 … leg_total − 1 is not in phase:
            discard q and continue
        append q to phase
    rounds = the complete groups of 6 in phase, in order
    # a trailing group with fewer than 6 indices is not a round
    for round in rounds:                       # up to thorough_rounds, default 3
        start all 6 legs before recording any of them
        if any of the 6 is mismatch:
            refuse
            do not start the next round
        record the round after all 6 have a result
```

**Informative.** Detection at a fake-leg fraction `f = 0.25`, for `n` sampled legs, is `1 − (1 − f)^n`.

| Rounds finished | Legs `n` | Detection |
|---|---|---|
| 1 | 6 | 82.2% |
| 2 | 12 | 96.8% |
| 3 | 18 | 99.44% |

Those three figures are `1 − (1 − 0.25)^n` at `n = 6`, `n = 12`, and `n = 18`. They are not a second sample size.

**Normative. Settled 21 Oct 2026.** A fake leg is a section 14.6 `mismatch` on a recomputed leg. The gate refuses that submission, and that gate ignores every later submission from that public key. The ignore is per key and that gate only. This file sets no expiry. A gate policy may set a duration. Later submissions receive that same refusal, with no new draw. A cheap-check failure does not add the key. “That gate” is the admission service’s persistent per-key record. The record survives a policy-version bump and a restart of the gate. A re-verify never gives a free redraw. Cross-gate sharing of the record is out of scope. An implementation of this default policy that runs one round’s 6 legs one after another does not follow the default policy. Rule A4.4.4 still applies to the section 14 measurement record: the record does not depend on finish order, and a verifier that is not running this default policy may recompute its six legs one at a time. That permission does not apply to `thorough_rounds`. The default policy starts all 6 legs of the round before any of those 6 is recorded, and it records the round only after all 6 have a result.

**Floor.** `leg_rate_floor` is TBD. This file sets no ratified value. A test placeholder of `1/5` (0.2 checkpoints per second) is placeholder, not ratified. The withdrawn examples `7/20`, `1/4`, and `1/8` are not the floor.

**Measuring method.** Measure on an owned commodity host. The low end of that set is the low-end owned host (an Intel Core i7-10610U laptop, no SHA-NI). Use the current reference or SDK. Run a real beacon-paced hike. The rate is the first-beacon to last-beacon rate, `hike_legs / hike_span`, Rule A4.10, with `hike_span` the median-time-past difference. The floor is calibrated on the best legal packed rate per machine. That rate is `N` parallel PCOCs with distinct keys. Distinct keys make distinct terrains under this seed. An interleave of those hikes is the packed rate. Shared-terrain packing across distinct PCOCs is impossible under this seed. It is not a legal schedule. Record the host, the CPU, the implementation, the commit, and that packed rate beside the single-hike rate. Set the floor a little below the low-end host. Do not measure an implementation that still uses the withdrawn fill pin or the withdrawn v2.0 fold. This method does not set the floor. The floor stays TBD. The placeholder `1/5` is not ratified.

**Informative.** The table below keeps profile names from the 2026-10-06 candidates note for the parameters this correction does not settle. Rows for `span_scale_seconds`, `span_sanity_bound`, `span_endpoint_mode`, and `fixed_start` are not admission inputs (Rules B3 and B6). Earlier figures, superseded and not in force: leg-rate ratios 0.9 and 1.0; `min_anchored_paths` 12, 8, 7, and 7; `min_last_bookend` 14, 14, and 13; `plant_gap_ceiling` 5; `span_scale_seconds` around 300; floors `7/20`, `1/4`, and `1/8`; thorough draws of 90 and 48 legs. Decimal spellings 0.5 and 0.7 were old forms only.

| Parameter | `strict` | `default` | `lenient` |
|---|---|---|---|
| `leg_rate_floor` | TBD. Placeholder `1/5` is not ratified | TBD. Same placeholder | TBD. Same placeholder |
| `span_scale_seconds` | not an admission input (Rule B3) | not an admission input (Rule B3) | not an admission input (Rule B3) |
| `span_sanity_bound` | not an admission input (Rule B3) | not an admission input (Rule B3) | not an admission input (Rule B3) |
| `span_endpoint_mode` | not an admission input (Rule B3) | not an admission input (Rule B3) | not an admission input (Rule B3) |
| `fixed_start` | not an admission input | not an admission input | not an admission input |
| `min_anchored_paths` | 6. Settled. Anywhere among `W_1` … `W_16` | 6. Settled | 6. Settled |
| `min_last_bookend` | withdrawn. No last-planted rule | withdrawn | withdrawn |
| `plant_gap_ceiling` | record `d_g`; no integer | record `d_g`; no integer | record `d_g`; no integer |
| `plant_gap_floor` | 1 block. Candidate | 1 block. Candidate | 1 block. Candidate |
| `thorough_rounds` | 18 distinct indices drawn once, split into 3 rounds of 6, each round in parallel. Default | 3. Default | 3. Default |
| `provisional` | `off` | `on` | `on` |
| `fast_accept_bound` | unused | 2 seconds | 2 seconds |
| `async_catch_budget` | 40.11 seconds | 22.05 seconds | 22.05 seconds |
| `reverify_max_attempts` | 3 | 3 | 3 |
| `max_held_blocks` | 6 | 6 | 6 |
| `verify_cap_per_object` | 1 per `(public_key, object_sha256, policy_version)`. Candidate | 1 per `(public_key, object_sha256, policy_version)`. Candidate | 1 per `(public_key, object_sha256, policy_version)`. Candidate |
| `verify_cap_per_door_session` | 3. Candidate. Not locked | 3. Candidate. Not locked | 5. Candidate. Not locked |

The candidates note also names `fast_six_leg_runs` and `fast_checkpoint_sample`. Leg recompute stays thorough, so `fast_six_leg_runs` is not a fast check. `fast_checkpoint_sample` is moot: the fast checkpoint-binding checks are the fold and the waypoint rehash under section 12.2, and the Rule A4.3 checkpoint digest is a cheap check on every checkpoint. The default sample is `thorough_rounds` 3, each round 6 legs in parallel. The v2.0 figures of 90 and 48 leg results are withdrawn. The `async_catch_budget` candidates, in table order `strict`, `default`, `lenient`, are 40.11 seconds, 22.05 seconds, and 22.05 seconds. These candidate budgets time the parallel checkpoint-digest pass and do not include recomputing legs of `STEPS_PER_LEG` steps. They are not locked. `fast_accept_bound` candidates are unused, 2 seconds, and 2 seconds. `reverify_max_attempts` and `max_held_blocks` are profile inputs. Candidates are 3 and 6. `reverify_max_attempts` counts `not read` retries only, fast or thorough. A hold with no `not read`, including `fast_accept_bound` and `async_catch_budget`, does not count toward it. It is hit when that many retries are still `not read`. `max_held_blocks` applies to every hold, whatever the cause: a fast `not read`, a thorough `not read`, `fast_accept_bound`, or `async_catch_budget`. It starts at the Gate’s `{index-url}` tip height at the first entry into `held` in the cycle and does not reset. `held_bound` fires when a check finds that tip ≥ start + `max_held_blocks`, even when no `not read` has occurred. Whichever budget is hit first, the outcome is refuse/revoke with reason `held_bound`. The detail names the budget that hit. Exhausting either budget is `revoke` if a grant was ever issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`), and `refuse` if none was. After a grant was issued (`provisional` or `accept`, including one already ended by the move from `provisional` to `held`) that outcome is `revoke`, not `refuse`. A `held_bound` `revoke` binds to `(public_key, object_sha256)`. It does not end another object's grant. A `revoke` in that object's own admission cycle ends that pair's grant and any signature-only sign-in for the object. A later-policy re-verify that refuses, other than a B7 cap refuse and other than Rule B10, is a `revoke` and removes the friendly entry only when this object is the key's current friendly entry, by a compare-and-remove on `object_sha256`, only if the current entry's object hash still matches this pair. A `not read` does not. `verify_cap_per_object` is a candidate of 1 per `(public_key, object_sha256, policy_version)`. It is a compute limit, not a ban. A hit's refuse is final for that `submission_sha256` only. It does not change the pair's state or friendly entry. It is not the pair's thorough decision. A cap refuse (`verify_cap_per_object` or `verify_cap_per_door_session`) does not end the cycle. Service-started re-verifies and `held` retries are not in the count. Candidate `verify_cap_per_door_session` is 3, 3, and 5 (`strict`, `default`, `lenient`). Not locked. It counts client submissions in the session, so it bounds the fast verifies and the thorough runs those submissions trigger. Held retries are Gate-internal and do not count. A transport rate limit stays outside this policy. The `leg_rate_floor` is TBD. The placeholder `1/5` is not ratified. The figures `7/20`, `1/4`, and `1/8` are withdrawn. `min_anchored_paths` is 6. `min_last_bookend` is withdrawn. An append-only hash-chained log entry may record the `{index-url}` tip height as `bitcoin_tip_height`. That name is the log entry's name for the tip. The entry is unsigned. It is not a signed head. v1 has no signed head and no signed receipt. These other counts are not shared across Gates in v1. One entry per key. Only a thorough `accept` of a newer object replaces it. Newer is the decision-log `seq` of that thorough `accept`. A higher `seq` is newer. A `refuse` or a `revoke` of any other object does not change the entry. A re-verify of an older object does not displace the current entry. A re-verify that refuses, other than a B7 cap refuse and other than Rule B10, is a `revoke` and removes the friendly entry only when this object is the key's current friendly entry. A `not read` does not remove the entry. A logged manual operator strike of the key is the other removal. Signature-only sign-in requires state `accepted`. The list has no time expiry. A `provisional` state grants no action (`provisional_action_cap`). A `provisional` grant is short, read-only access until thorough settles or `async_catch_budget` is hit. `revoke_scope` is the pair `(public_key, object_sha256)`.

## 16. Conformance

**Normative.**

Conformance is stated separately for three layers. A layer does not take on another layer’s job.

1. A conforming **producer** follows section 13 and emits the deterministic encoding of section 7.3, or emits no object when the drive’s clock read does not succeed. It records evidence. It does not emit a verify record. It does not say `accept`, `refuse`, `revoke`, or `held`. A producer that pauses the drive to wait for a further header, or pauses on a wall clock, before the first leg of a stage does not follow section 13. Stage 0 has no `path` key in that encoding.
2. A conforming **verifier** follows section 14 and outputs the verify record: measurements as integers and the section 4 tokens. The profile name and, under `conformance-small`, `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG` are verifier inputs. They appear on the section 14 verify record as measurement provenance. The profile is not absent from that record. The hike object gains no new field. It does not judge those measurements. It does not say `accept`, `refuse`, `revoke`, or `held`. It does not post a waypoint and it does not ask Bari for a path.
3. An **admission service** is not a conformance target. Section 15 is policy for a relying party. Apart from Rule B4 (`anchored_total ≥ 6`, settled), threshold values are policy and are not set by this file. They are not vectors. A service may run section 14 itself and may ignore a submitter-supplied verify record. Doing so does not make the service conform or fail to conform.

Sections 8–14 do not use pass, fail, reject, deny, or admit for the object or for a measurement. Those outcomes are not verifier outputs. The decisions `accept`, `refuse`, `revoke`, and `held`, and the states in section 15.2, are section 15 only.

**Vectors.** Conformance means Part A only. For v2.1, a producer or a verifier conforms when its outputs equal the published vectors under `spec/conformance/v2.1/` byte for byte. That pack is the seed, the NL4 word, the terrain digests, the §11.1 fold, the leg, checkpoint, and waypoint results at the `conformance-small` sizes, the check that the seed equals `stages[0].waypoint`, the rate and anchor comparisons, and the thorough-round index draw. It does not republish a full hike-object encoding. The pack under `spec/conformance/v2.0/` is historical and is not the v2.1 target. It was generated under the withdrawn fill pin and the withdrawn v2.0 fold. A byte match to that pack is not v2.1 conformance. A historical v2.0 producer or verifier matched that pack byte for byte: the same hike-object bytes where the vector is a producer target (`kat-small` is that target), and the same section 14 measurement record, meaning the same integers and the same measurement tokens. The vectors are language-agnostic. `spec/conformance/v2.0/sign-in.json` is the locked sign-in wire: the link rules, `challenge_bytes`, the digest, and one signature. It is not a section 14 record. Matching that wire is not Part A conformance. The hike pack generator does not write it. This section is the contract. The files are in the repository. A vector compares bytes, integers, and tokens. It does not grade a hike object as pass or fail. This pack publishes the integer rate comparison and the thorough-round index draw. It does not publish section 15 states. Those states are not a conformance target. This file governs. This file stays the specification. The vectors under `spec/conformance/v2.0/` are historical worked examples regenerated from the Rust reference under the withdrawn fill pin and the withdrawn v2.0 fold. They are not the v2.1 target. The v2.1 vectors are `spec/conformance/v2.1/`. That pack publishes the seed, the NL4 base words, the terrain digests, the §11.1 fold, the `conformance-small` leg, checkpoint, and waypoint bytes, `waypoint_equals_seed`, the integer rate and anchor comparisons, and the thorough-round index draw. It does not publish a hike-object encoding, a section 14 record, `object_hex`, or `draw_bytes_hex`. A conforming producer or verifier matches those published bytes. A section 14 record is defined by section 14. v2.1 does not add a second record encoding in this pack. An earlier object is the Rule A5 line: `version` and `kind` only. A current object’s record carries every key section 14 requires, including `shape_item`, `headers`, `paths`, `anchors`, bookends, `N`, and `S`. The compact line that named only `profile`, `shape`, `checkpoints`, `waypoints`, `leg_rate`, `six_leg`, and `anchors` is not a conformance output. A conforming verifier outputs this section 14 record. Where a published vector and a sentence of this file conflict, the conflict is a defect in this specification. It blocks release until a specification change settles which side is right. An implementer reports the conflict and does not pick a side. Where a reference implementation disagrees with this file, this file wins and the reference has a bug. A published vector does not settle the conflict. The conformance runner may mark that vector `known-defect` until the specification change settles it.

`spec/conformance/v2.1/` is the v2.1 pack. Its files and the bytes they pin are listed in `v2.1/manifest.json` and `v2.1/README.md`. `v2.1/kat-small.json` and `v2.1/test-3leg.json` pin terrain digests and per-stage leg, checkpoint, and waypoint bytes at the profile integers in each file. `legs_le_hex` is the hexadecimal spelling of the little-endian wire bytes `le64(leg_result)`. It is not the `%016llx` spelling of the integer. They do not pin `object_hex`, a section 14 `record`, bookends, or a six-leg draw. `spec/conformance/v2.0/` is historical. A sentence in this file that names `object_hex`, `draw_bytes_hex`, `header_reads`, or `plant_reads` names that historical pack, not v2.1. A section 13.2 producer is not required to replay a v2.0 `object_hex`. No git tag is cut, and no deployment cuts over to `KIND` `satoshi-hike-v2.1-object`, until every port passes this pack and the production-size vectors. The intended tag name is `spec-v2.1.0`. The maintainers cut that tag on the merge commit. This file does not create the tag.

When two implementations follow this file at the production constants in section 6, they agree on the terrain bytes, each leg result, each checkpoint, the seed, and each waypoint `W_1` … `W_17`. Given the same path-body bytes and the same headers, they agree on the deterministic encoding of the object.

Section 10 defines the seed, the seed preimage, the NL4 word, and every terrain byte. There is no fill pin. The checkpoint preimage and the waypoint preimage are defined in sections 11 and 12. The leg is defined in section 11. Path bytes on stages 1–17 are the checked body of the successful fetch in section 13.1, or the empty path bytes section 13.3 stores on `path_skip` or on `not fetched by design`. A verifier does not fetch those bytes again. Section 14.3 records `skip`, `present`, plant `match` or `mismatch` or `not read` or `n/a`, and `d_g`. Each verification draws its own six indices, as section 14.6 states, and records each recomputed leg result against the stored leg result.

### 16.1 Profile `conformance-small`

The profile in force is the section 6 constants, unless the verifier is running `conformance-small`. That profile may differ only in `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG`. The profile name and those four integers are verifier inputs. The hike object gains no new field. The verify record must state the profile, and must state `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG` when the profile is `conformance-small`. A live object still carries the section 6 size fields. When the profile name is `conformance-small`, all four integers are required inputs. The verifier does not substitute the section 6 values for an omitted integer. It does not change `KIND`. A verify CLI that passes `--profile conformance-small` without all four integers is a usage error, exit code 2. This file assigns no default integers to that name. The record states the four integers it was given.

The profile may set only these section 6 parameters:

| Parameter | Rule ID | Why it may shrink |
|---|---|---|
| `SAMPLE_BYTES` | P6.5 | It is the terrain-sample limb of every checkpoint preimage. |
| `SLOT_BYTES` | P6.1 | It is the slot the step reads and the slot-sample limb of every waypoint preimage. |
| `TERRAIN_BYTES` | P6.3 | It is the terrain fill. |
| `STEPS_PER_LEG` | P6.7 | It is the number of steps in one leg. |

`WORDS_PER_SLOT`, `N_SLOTS`, and `OFFSET_MODULUS` are not free integers under the profile. They stay the section 6 formulas (P6.23, P6.24, P6.25) applied to the profile values. `SLOT_BYTES` is at least 8, a multiple of 8, and at most the section 6 value. `STEPS_PER_LEG` is at least 1 and at most the section 6 value. `SAMPLE_BYTES` is at least 1 and at most the section 6 value. `TERRAIN_BYTES` is at most the section 6 value, an exact multiple of `SLOT_BYTES`, and at least `SAMPLE_BYTES`, so `OFFSET_MODULUS` stays at least 1. The step folds every word of the slot by §11.1: `word(w) XOR S`, then wrapping addition of `mix64`, for `w = 0 … WORDS_PER_SLOT − 1`. There is no `w` term. Equal words do not cancel under addition. The checkpoint preimage stays `32 + 32 + SAMPLE_BYTES + 8` bytes. The waypoint preimage stays `32 + SLOT_BYTES + 32 + 32` bytes. Hash order does not change.

The profile does not set `STAGES`, `HASH_BYTES`, `LEG_RESULT_BYTES`, `PUBLIC_KEY_BYTES`, `BEACON_BYTES`, `SEAL_DOMAIN`, `KIND`, `FILL`, or the mix constants P6.17–P6.22. It does not set a domain label on the seed. `SEED_DOMAIN` is retired. It does not set the Hashcash difficulty (P13.5), the ask window (P13.2), or the six-leg draw count (A4.4.1). A thread pool is not required (A4.4.4). Eighteen stages stay eighteen. Waypoint indices, `W_17`, and the cut-off stay where section 13 puts them.

**Legs.** `legs_per_stage` is vector fixture metadata, not a section 6 constant. It is a list of 18 integers, one per stage, each at least 1. Producer KATs use it: the rig’s tip schedule follows that list, and the vector’s object stores those counts as `leg_count`. Production produce does not read `legs_per_stage`. Principle 9 still holds: there is no maximum leg count on a live object. A vector that exercises Rule A4.4 needs at least 6 legs in the hike, because fewer than 6 leaves `six_leg` `not defined`. The draw count stays 6.

**Draw bytes.** Injected draw bytes are rig-only, for a deterministic Rule A4.4 vector. The rig supplies them in order as the section 14.6 byte source. A published vector names those bytes `draw_bytes_hex`. The verifier consumes them in that order. It does not expand them again. They are never a production input. Production verify reads the operating-system CSPRNG. The draw count stays 6. Fewer than six legs leaves the draw unconsumed and `six_leg` `not defined`.

**Wire.** The hike object gains no new field. Its size fields stay `slot_bytes`, `n_slots`, `sample_bytes`, and `steps_per_leg`. Under `conformance-small` those fields hold the verifier’s four integers, and `n_slots` is the formula. `shape_item` includes `constant` when a field differs from the profile in force (A4.1.2). A live producer emits section 6 only.

**Rig checks.** Whether the six recomputes run together is Rule A4.4.4. A rig does not add a required pool. A rig may also check the producer rules in this file: no pause before the first leg (section 13.1), and the wait from 15 seconds through 45 seconds (P13.7). Rig-only inputs, not specification parameters, are `now_unix_s`, the issuing nonce, phase timing, the local mirror contents, a Bari mock, injected draw bytes, and the block-height (tip) clock.

The same profile integers and the same inputs produce the same bytes. The profile does not relax an INVARIANT. QA chooses the integers in the published vectors. This section does not lock a second set of production numbers.

## 17. Closed

**Informative.**

The items below were open. They are now part of the construction in the sections named here. This section does not add a second construction.

### 17.1 Waypoint path fetch

Closed. Section 13.1 posts `W_1` … `W_17`. It does not post the seed. Asks are for `W_1` … `W_16` under section 13.1. `W_17` is posted and is not asked. No ask is made above the cut-off. Test and live use that one rule. On stages 1–17 the field is `path`, a CBOR byte string, in the key order of section 7.3. Stage 0 has no `path` key. Those bytes are not an input to section 11.4 or section 12.2. A POST that does not succeed, and an ask that is not a checked 200, are a miss. Produce stores empty bytes and writes `path_skip <g>` for `W_1` … `W_16`, and continues. It does not withhold the object. `W_17` writes `not fetched by design 17`. A `skip` is a measurement (Rule A4.5). Rule B4 judges it: `anchored_total ≥ 6`, wherever those waypoints sit. A `present` path is measured in section 14.3, including `CanonicalPathBody` `validate_for` and the chain bind. Verify does not GET the path again.

### 17.2 Portable-index height fetch during verify

Closed. Verify reads the portable index. For each stored header, including `last_interrupt`, it GETs `{index-url}/index/{height}` and records `index_comparison` and `T` under Rule A4.2. That header GET is separate from a plant GET of the same URL. A body with no usable `header` is header `not read` and can still be plant `match`. `T` is `header.time`, Bitcoin `nTime`, Unix seconds, only when `index_comparison` is `match`. Otherwise `T` is `not read`. `header.time` is the portable index’s record of `nTime` for the block whose hash is `header.hash`. `index_comparison` uses hash and height only. It does not use `network`. An unsuccessful header read is `not read`. A hash or height that differs, when the read returned them, is `mismatch`, and `T` is `not read`. That record is part of Rule A4.2. The admission service applies section 15 to the record. Verify does not GET an HTTP tip. Verify does not Observe.

A document whose `network` is `synthetic` can still record **match** on the plant comparison. A header that matches that document is not a **mismatch** for that `network`. A **match** against a synthetic document is not a mainnet fact.

### 17.3 Path and chain bind

Closed. Section 14.3 records `plant` `match`, `mismatch`, or `not read` for each `present` path among `W_1` … `W_16`. The inclusion-height GET is made only when that height is in the window and that header is a stored interrupt. Otherwise `plant` is `not read` and the index is not read for that path. That not-issued `not read` is not a missing GET under Rule A4.11. `not read` also means an issued read that does not return a document. A document that was read and has no plant for that block, or whose plants do not match, or whose `network` is neither `main` nor `synthetic`, is `mismatch`. A document with no `header` member can still be plant `match`. `anchored` requires the inclusion height in the section 13.1 window, that header stored on the object as an interrupt, and plant `match`. Plant `match` alone is not enough. On `mismatch` and on `not read`, `d_g` is `not defined`. `claimed_block` is the path body’s `block_hash` and `block_height` when those fields can be read, otherwise `not defined`. The measurement is distinct from Rule A4.2. A `skip` is not a plant comparison. Verify does not GET the path from Bari again. The pack field `index_reads` lists only those issued plant heights.

A **match** on a document whose `network` is `synthetic` is not a mainnet fact.

## 18. Follow-up wording

**Informative for items 1–8. Item 9 is locked.**

Items 1–8 are Docs follow-ups. This file does not edit the files named there. Docs applies the replacements an item still names, in a later change. Item 1 remains Informative. Its closing sentences lock the public verify page’s website rows as **valid** and **invalid** and do not ask Docs to retoken those rows. The proposals follow section 4. Item 9 is locked law for the verify CLI.

1. **Verify page.** In `apps/www/verify/verify.js` and `apps/www/verify/README.md`, replace “One broken checkpoint breaks the proof” with “One checkpoint recorded as mismatch is a measurement. The Gate judges it.” Replace “An empty path does not fail” with “An empty path on W_1 through W_16 is recorded as skip. The Gate’s paths minimum is Rule B4, `anchored_total ≥ 6`, and that comparison is what judges skips.” The public verify page’s website rows stay **valid** and **invalid**. This item must not require those website rows, including the lines for `shape`, `checkpoint_mismatch`, and `successor_waypoint_differs`, to become the section 14 match and mismatch tokens. Apps (CLI, Desktop, and Android) keep the section 14 measurement vocabulary (match, mismatch, and the other section 14 tokens).

2. **Ops.** In the operator's environment documents (not published), replace “A verify pass is a separate result” with “A verify measurement record is separate from the Sign-in door. Verify records match, mismatch, present, skip, not defined, not read, not fetched by design, and n/a. The Sign-in door is the HTTP door of the groundinglab Admission Gate (section 15).” A Bari POST miss or an incomplete ask is `path_skip` and produce continues. Those pages must not say a missed plant stops the hike.

3. **Gate `/v1/verify`.** Replace the double-negative fields `checkpoint_mismatch: false` and `successor_waypoint_differs: false` with positive tokens, for example `checkpoints: match` and `waypoints: match`, using **match** and **mismatch**. The same response’s `present_path_does_not_match_plant` and `opening_waypoint_is_zero` follow the same change: plant comparison **match** or **mismatch**, and the seed-value comparison **match** or **mismatch**. `opening_waypoint_is_zero` does not describe the seed.

4. **Leg names.** Where a measurement name or a test name says “re-walk” or “re-walked leg,” say “leg” or “recomputed leg result.” Section 14.6 in this file already uses that wording. Other files still say “re-walk,” including `reference/rust/hike/src/verify.rs` and `services/admission-gate/src/jobs.rs`. Those stay for a later change. The constant `SEAL_DOMAIN` (`pcoc-walk-v1`) is a byte string. It is not leg wording, and it stays.

5. **Withdrawn.** This item is withdrawn. The number is not reused.

6. **Record keys.** A later verify response uses the keys pinned in the section 14 verify record: `profile` (and, when that profile is `conformance-small`, `sample_bytes`, `slot_bytes`, `terrain_bytes`, and `steps_per_leg`), `shape`, `shape_item`, `headers` (`stored_hash`, `stored_height`, `index_comparison`, `T`), `seed_value`, `checkpoints`, `waypoints`, `paths` (`path_state`, `plant`, `P_g`, `height_P_g`, `d_g`, `claimed_block`, and for `g = 17` `w17_path`), `anchors`, `anchored_total`, `skip_list`, `a`, `b`, `N`, `S`, `delta_h`, `fixed_start` (`N`, `S`, `delta_h`), `P_b`, `height_P_b`, `index_comparison_P_b`, `T_P_b`, `height_I_a_minus_1`, `T_I_a_minus_1`, `six_leg`, `six_leg_runs`, and `analysis` (`whole_hike_legs`, `whole_hike_span`, `per_stage`, `legs_outside`, `hike_legs`, `hike_span`; `per_stage` carries `checkpoint_count`, not `leg_count`). Tokens stay lowercase with spaces, including `n/a`. Do not add a rate field. Admission outcomes for those keys are section 15.

7. **Bari status 204.** Call it a successful POST. Do not call it an admit outside section 15.

8. **Produce event log.** In section 13.5 the keys `posted`, `succeeded`, `checked`, and `worked` stay JSON `true` or `false` on that log. They are not section 14 tokens. A later wording pass may describe those log fields without using them as verify outcomes.

9. **Verify CLI. Locked.** Locked 2026-10-07. The verify CLI emits only section 14 record tokens. Stdout for a current object is the one section 14 record, keys in the order section 14 lists. The compact line is not a conformance output and this CLI does not write it. Exit code `0` means the record was produced, whatever it measured, including all `not read` or `mismatch`. Exit code `1` means the object file cannot be read or decoded. That is the object file, not the `not read` token. Exit code `2` means a usage or invocation error. No other exit code. Stdout and stderr carry no pass, fail, accept, refuse, or admission wording. A named `--profile` is `production` or `conformance-small`. An omitted `--profile`, `test`, and `live` still measure with the section 6 constants. `--profile live` selects the live index only. Admission never runs inside verify. `--profile` is not a Gate policy profile. Profile judgement lives only in the policy library and the Gate. When the profile name is `conformance-small`, all four integers are required inputs. The verifier does not substitute the section 6 values for an omitted integer. It does not change `KIND`. A verify CLI that passes `--profile conformance-small` without all four integers is a usage error, exit code 2. This file assigns no default integers to that name. The record states the four integers it was given.
