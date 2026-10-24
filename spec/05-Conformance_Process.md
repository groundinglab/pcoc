# Conformance Process

**FRAMING.** Construction + Conformance = definitional pair (specification + matching test specification). Design-time for producer and verifier authors. At runtime a verifier applies those same rules while measuring: record tokens, including `mismatch`. That is not a reject gate that skips measurement, and it is not admission. Admission policy values are not a conformance target. This specification defines no tip-versus-last-beacon submit deadline.


**Responsibility.** Test-specification half of the Construction + Conformance definitional pair: yes/no rules, permanent negatives, and the one pack. Also the process for claiming an implementation matches that pack. Used by a producer or verifier implementer who wants that claim.

**Source.** Satoshi Hike 2.3 (tag `spec-2.3.0`). Section numbers are the specification section numbers:

- §16 Conformance
- §16.1 Profile conformance-small
- §16.2 Implementation requirements
- §17 Clarifications
- §19 Fixed issues — do not reintroduce

---

## 16. Conformance

**Informative. Summary.** Conformance is per layer. Matching the one pack byte for byte, and reproducing every permanent negative as a non-match, are part of conformance. They are not the whole of it. Section 16.2 states the rest, including Rules I1–I6. The admission service is not a conformance target. Section 16.1 defines the `conformance-small` profile, and 16.2 the SDK requirements (Rules I1–I6).

**Normative.**

Conformance is stated separately for three layers. A layer does not take on another layer’s job.

1. A conforming **producer** follows section 13 and emits the deterministic encoding of section 7.3 for every run that reaches `I_17`. It records evidence. It does not emit a verify record. It does not say `accept`, `refuse`, `revoke`, or `held`. A producer that pauses a leg for a post, a solve, an ask, a header or peer read, a log write, or a wall clock does not follow section 13 (Rule A6). A producer that withholds the object because of a clock-read failure or another transitory network condition does not follow section 13. Stage 0 has no `path` key in that encoding.
2. A conforming **verifier** follows section 14 and outputs the verify record: measurements as integers and the section 4 tokens. The profile name and, under `conformance-small`, `SAMPLE_BYTES`, `SLOT_BYTES`, `TERRAIN_BYTES`, and `STEPS_PER_LEG` are verifier inputs. They appear on the section 14 verify record as measurement provenance. The profile is not absent from that record. The hike object gains no new field. It does not judge those measurements. It does not say `accept`, `refuse`, `revoke`, or `held`. It does not post a waypoint and it does not ask Bari for a path.
3. An **admission service** is not a conformance target. Section 15 is policy for a relying party. Lever values (section 15.4) are policy and are not set by this file. They are not vectors. A service may run section 14 itself and may ignore a submitter-supplied verify record. Doing so does not make the service conform or fail to conform.

Sections 8–14 do not use pass, fail, reject, deny, or admit for the object or for a measurement. Those outcomes are not verifier outputs. The decisions `accept`, `refuse`, `revoke`, and `held`, and the states in section 15.2, are section 15 only.

**Vectors.** Conformance means Part A, the Hashcash rule of section 13.1, and the Bari paid rule of section 13.6. A producer or a verifier conforms when its outputs equal the published vectors of the one pack, `spec/conformance/pack/`, byte for byte, and when it reproduces every permanent negative as a non-match. The pack publishes the seed, the NL4 base words, the terrain digests, the §11.1 fold, the `conformance-small` leg, checkpoint, and waypoint bytes, one production-size leg, `waypoint_equals_seed`, the integer rate and anchor comparisons, the thorough-round index draw, the Hashcash challenge and digest for a stated origin and fee header, the Bari paid rule for current, before, after, and outside fee headers (including a wrong origin, and a display-order mismatch: a current, a before, and an after case each built with Bari’s tip hash reversed, each answered 402 under section 9), with the “after” and “wrong `nBits`” cases published separately for the live plane (mainnet target, including a retarget boundary) and the test plane (fixed synth `nBits`, section 9), each vector labelled with its plane, the unknown-kind record, and the sign-in wire (`sign-in.json`, `sign-in-origins.json`). It does not publish a hike-object encoding, a section 14 record, `object_hex`, or `draw_bytes_hex`.

**Permanent negatives.** The pack carries, and every SDK must reject as non-matching, at least the thirteen negatives below. Each negative is computed at the vector’s profile integers, for the vector’s public key and seed beacon, with the section 10.4 terrain and the section 11.1 step except where its entry says otherwise. A leg negative is the first leg of stage 0, from its Rule A2.2 start (the seed), and is published as `legs_le_hex` of that leg. A terrain negative is published as the SHA-256 of the whole terrain array. Each entry is a construction this file forbids; it is defined here only so that the pack can pin it as a non-match. The pack generator compares each negative with the honest value of the same vector and stops with an error when they are equal; QA then chooses other fixture inputs. A negative is never removed while the construction it guards stands.

- `slot_summary_fold`, `xor_accumulator_fold`, `xor_w_fold`, `s_plus_w_fold`: the four folds that section 11.1 names. Leg negatives.
- `prefold_hoist`: the fold that computes once per slot `pre(i) = (sum over w of mix64(word(w) XOR w)) mod 2^64` and, on every step that reads slot `i`, uses `S_folded = (pre(i) + S) mod 2^64` in place of the section 11.1 accumulator, then the section 11.1 seal. Leg negative. It is the per-slot summary that section 11.1 forbids: `word(w) XOR w` does not depend on `S`.
- `final_stage_hoist`: the fold whose `S_folded` is `x XOR (x >> 31)`, where `x` is the XOR, over every word of the slot, of `mix64` applied to `word(w) XOR S` without its last line (the value `y` just before `return y XOR (y >> 31)` in section 10.1). Leg negative. It is the hoist that section 11.1 describes as informative. Its value equals an XOR accumulator of `mix64(word(w) XOR S)`.
- `partial_slot_read`: the section 11.1 fold over word 0 only of the slot each step reads. Leg negative.
- `partial_slot_read_2047`: the section 11.1 fold over words `0 … WORDS_PER_SLOT − 2` of the slot each step reads, leaving out the last word. Under section 6 that is 2047 of the 2048 words. Leg negative.
- `one_lane_flip`: the section 10.4 fill with the one-lane map of section 10.4 in place of the NL4 base word. Parents and the slot mix are unchanged. Terrain negative.
- `all4_kernel`: the section 10.4 fill with the linear map of section 10.4 in place of the NL4 base word. Parents and the slot mix are unchanged. Terrain negative.
- `f3_shared_terrain`: the first leg of stage 0 of the vector’s public key, from that key’s own start, run on the terrain of a second public key that the vector states, with the same seed beacon. Leg negative. The `f3` in the name is not row F3 of section 19. This negative guards row F4, shared terrain.
- `cached_leg`: the first leg of stage 0 in which a step that reads a slot already read earlier in the same leg reuses the `S_folded` of that earlier read instead of folding the slot with the current `S`. Leg negative. The vector’s leg reads at least one slot twice.
- `squeezed_seed_terrain`: the section 10.4 fill with each of the four lanes replaced by the one 64-bit value `P[0] XOR P[1] XOR P[2] XOR P[3]`, the squeeze that section 10.2 forbids. Terrain negative.

The vectors are language-agnostic. The sign-in wire vectors are not a section 14 record; matching them is not Part A conformance. A vector compares bytes, integers, and tokens. It does not grade a hike object as pass or fail. This pack does not publish section 15 states. Those states are not a conformance target. A section 14 record is defined by section 14. An unknown kind is the Rule A5 line: `kind` and `kind_text` only. A current object’s record carries every key section 14 requires, including `shape_item`, `headers`, `paths`, `anchors`, bookends, `N`, and `S`. The compact line that named only `profile`, `shape`, `checkpoints`, `waypoints`, `leg_rate`, `six_leg`, and `anchors` is not a conformance output. A conforming verifier outputs this section 14 record.

**Precedence.** This file governs, together with the pack. Where a published vector and a sentence of this file conflict, the conflict is a defect in this specification. It blocks release until a specification change settles which side is right. An implementer reports the conflict and does not pick a side. Where any SDK disagrees with this file or the pack, this file and the pack win and the SDK has a bug, whatever that SDK’s role as a reference implementation (section 16.2). No SDK output settles a conflict, and an SDK behaviour that skips work never becomes conforming because an SDK does it. The conformance runner may mark a vector `known-defect` until the specification change settles it.

`spec/conformance/pack/` is the one pack. No other pack exists, and no file or sentence of this specification names another. Its files and the bytes they pin are listed in `manifest.json` and `README.md` in that directory. `kat-small.json` and `test-3leg.json` pin terrain digests and per-stage leg, checkpoint, and waypoint bytes at the profile integers in each file. `legs_le_hex` is the hexadecimal spelling of the little-endian wire bytes `le64(leg_result)`. It is not the `%016llx` spelling of the integer. They do not pin `object_hex`, a section 14 `record`, bookends, or a six-leg draw. The pack is pinned by the SHA-256 of its manifest, recorded in `sdk/SPEC_PIN` beside the tag of this file (Rule I5). Renaming, adding, or removing a vector changes the pin. No git tag is cut, and no deployment cuts over to `KIND` `v2.2-pcoc-object`, until every SDK passes this pack and the production-size vectors. This conformance process document and its siblings under `spec/` carry the Satoshi Hike 2.3 rules (tag `spec-2.3.0`). This file does not create that tag.

When two implementations follow this file at the production constants in section 6, they agree on the terrain bytes, each leg result, each checkpoint, the seed, and each waypoint `W_1` … `W_17`. Given the same path-body bytes and the same headers, they agree on the deterministic encoding of the object.

Section 10 defines the seed, the seed preimage, the NL4 word, and every terrain byte. There is no fill pin. The checkpoint preimage and the waypoint preimage are defined in sections 11 and 12. The leg is defined in section 11. Path bytes on stages 1–17 are the checked body of the successful fetch in section 13.1, or the empty path bytes section 13.3 stores on `path_skip` or on `not fetched by design`. A verifier does not fetch those bytes again. Section 14.3 records `skip`, `present`, plant `match` or `mismatch` or `not read` or `n/a`, and `d_g`. Each verification draws its own six indices, as section 14.6 states, and records each recomputed leg result against the stored leg result.

<a id="sec-16-1"></a>

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

The profile does not set `STAGES`, `HASH_BYTES`, `LEG_RESULT_BYTES`, `PUBLIC_KEY_BYTES`, `BEACON_BYTES`, `SEAL_DOMAIN`, `KIND`, `FILL`, or the mix constants P6.17–P6.22. It does not set a domain label on the seed. It does not set the Hashcash difficulty (P13.5), the ask window (P13.2), or the six-leg draw count (A4.4.1). A thread pool is not required (A4.4.4). Eighteen stages stay eighteen. Waypoint indices, `W_17`, and the cut-off stay where section 13 puts them.

**Legs.** `legs_per_stage` is vector fixture metadata, not a section 6 constant. It is a list of 18 integers, one per stage, each at least 1. Producer KATs use it: the rig’s tip schedule follows that list, and the vector’s object stores those counts as `leg_count`. Production produce does not read `legs_per_stage`. Principle 9 still holds: there is no maximum leg count on a live object. A vector that exercises Rule A4.4 needs at least 6 legs in the hike, because fewer than 6 leaves `six_leg` `not defined`. The Rule A4.4 draw count stays 6. The thorough-round draw vector is the section 15.5 draw and draws `D` indices (**Thorough rounds**, below).

**Thorough rounds.** The thorough-round index draw vector states its own `thorough_rounds`, as vector fixture metadata like `legs_per_stage`. That value is not a policy value, it is not read from section 15.4, and it sets no policy. The vector draws `D = 6 · thorough_rounds` indices under section 15.5 from its own byte source (**Draw bytes**, below), read from the first byte. A vector that publishes both a six-leg draw and a thorough draw, or draws for more than one leg total, reads each draw from the first byte of its byte source. No draw continues where another stopped.

**Anchor comparison.** The integer anchor vector gives a stored stage height `H` and a candidate inclusion height. Its answer is `anchored` when the inclusion height is `H` plus one of `ANCHOR_WINDOW_OFFSETS`, and `not anchored` otherwise, as if that header were an interrupt stored on the object and its plant comparison were `match`. It does not test those two conditions. Rule A4.5 still requires both on a real object. The integer anchor vector is not the anchor-window vector of sections 9 and 14.4, which is a full object.

**Draw bytes.** Injected draw bytes are rig-only, for a deterministic draw vector: the Rule A4.4 six-leg draw, or the section 15.5 thorough-round draw. A published draw vector names its byte source as a SHA-256 chain from a stated ASCII label: `b_0 = SHA-256(label)` and `b_(n+1) = SHA-256(b_n)`, concatenated in that order. The pack publishes the label, not the bytes. The rig expands the label into that chain and supplies the bytes in order as the section 14.6 byte source. The verifier consumes them in that order and does not hash them again. They are never a production input. Production verify reads the operating-system CSPRNG. The Rule A4.4 draw count stays 6; the thorough-round draw vector draws `D` indices (**Thorough rounds**, above). Fewer than six legs leaves the draw unconsumed and `six_leg` `not defined`.

**Wire.** The hike object gains no new field. Its size fields stay `slot_bytes`, `n_slots`, `sample_bytes`, and `steps_per_leg`. Under `conformance-small` those fields hold the verifier’s four integers, and `n_slots` is the formula. `shape_item` includes `constant` when a field differs from the profile in force (A4.1.2). A live producer emits section 6 only.

**Rig checks.** Whether the six recomputes run together is Rule A4.4.4. A rig does not add a required pool. A rig may also check the producer rules in this file: no leg is paused (Rule A6), the object closes at `I_17` with no collection window (section 13.1), exactly one resend after a transport failure (P13.13), and no stop on a failed tip read or a stalled header source (P9.12). Rig-only inputs, not specification parameters, are `now_unix_s`, `nonce_store_start_unix_s` (a store restart or flush, and each worker’s process start), the issuing nonce, phase timing, the local mirror contents, a Bari mock, a header-source mock and its clock, injected draw bytes, and the block-height (tip) clock.

The same profile integers and the same inputs produce the same bytes. The profile does not relax an INVARIANT. QA chooses the integers in the published vectors. This section does not fix a second set of production numbers.

<a id="sec-16-2"></a>

### 16.2 Implementation requirements

**Normative.**

<a id="rule-i1"></a>

#### Rule I1. SDKs are the reference implementations

The per-language SDKs under `sdk/` (Rust, Go, Java, C, and any SDK added under these rules) are the reference implementations. There is no other reference tree. Bari, the admission service, and the apps use an SDK; they do not carry their own copy of the construction or of the Hashcash rule. An SDK is a reference implementation only while it passes the whole pack. This file and the pack win over every SDK (section 16, **Precedence**).

<a id="rule-i2"></a>

#### Rule I2. Interfaces

Each SDK exposes at least: the seed and terrain fill (section 10); the leg and the step (section 11); the checkpoint (section 11.4); the waypoint (section 12.2); the Hashcash challenge, digest, and solve (section 13.1); the Bari paid-rule check (Rule BR1); the object encode and decode (section 7.3); the section 14 verify record, including Rule A5; the section 14.6 draw and the section 15.5 thorough draw, each from a supplied byte source; the Rule B2 integer comparison; and the integer anchor comparison (section 16.1, **Anchor comparison**). Each public function names the section it implements. Its behaviour is that section. An SDK’s own prose does not add a rule.

<a id="rule-i3"></a>

#### Rule I3. Producer model

An SDK producer runs the walk on its own thread or threads and runs every post, solve, ask, header-source read, Bari exchange, and log write as asynchronous work (Rule A6). The walk reads the tip through a call that returns at once. No SDK producer sleeps, waits on I/O, takes a lock that I/O holds, or syncs a file on the walk path. A failure of asynchronous work becomes an `alert` record or a logged miss; it never propagates into the walk. Only a producer failure or explicit cancellation ends a run before `I_17`. The header source detects a stalled or lagging peer and recovers on its own, asynchronously; a stall never pauses a leg.

<a id="rule-i4"></a>

#### Rule I4. Tests

Each SDK runs, on every change: the whole pack; every permanent negative as a non-match; the production-size leg vector; a producer test that injects a stalled header source, a failed tip read, a Bari that never answers, a Bari that answers 503, and a log that cannot be written, and checks that legs keep running, that exactly one resend is made after a transport failure, and that the object closes at `I_17`; a header-source test, on a mock clock, with one silent peer among `HEADER_PEERS_MIN` or more, which checks that the silent peer is replaced once `HEADER_PEER_SILENCE_SECONDS` have passed after another peer announced a header and not before, that a gap in which no peer announces a header replaces no peer, that a fall below `HEADER_PEERS_MIN` writes a `header_peers_low` alert and reconnects asynchronously, and that legs keep running and the object closes at `I_17` throughout; and a test that a paid waypoint on each of the current, before, and after headers is accepted by its Bari paid-rule check, and that any other POST is answered 402, including a fee header that matches Bari’s tip only when one side is reversed to display order (section 9, P9.17); and, for an SDK that implements the section 15.1 Sign-in door, a nonce-store test on a mock clock in three cases: (a) a submission consumes its nonce, the single-process door restarts while that challenge’s window is still open and drops every row, and the same submission is replayed; (b) the shared nonce-row store is flushed with no process restart, while the window is open, and the same submission is replayed; (c) two door workers with different process starts share one store and receive the same challenge. The test checks that each replay is client `malformed`, consumes nothing, and runs no policy; that a challenge issued at or after `nonce_store_start_unix_s + challenge_clock_skew_seconds` is accepted and one issued earlier is `malformed`; and that both workers in (c) use the one store start and give the same answer for the same challenge.

<a id="rule-i5"></a>

#### Rule I5. Versioning and pin

An SDK version is `MAJOR.MINOR.PATCH`, with `MAJOR.MINOR` equal to this file’s version, 2.3. A `PATCH` release does not change any output byte. Each SDK reads `sdk/SPEC_PIN`, which names the tag of this file and the SHA-256 of the pack manifest, and its test fails when the pack it runs does not match that pin. SDK tags are `sdk-<language>-v2.3.<patch>`. This file’s tag is `spec-2.3.0`. When the SDKs move to 2.3, `sdk/SPEC_PIN` names `spec-2.3.0`. Until then the SDKs stay at 2.2, which produces and verifies the same bytes. An admission service whose pin is `spec-2.3.0` reads a policy file whose `spec` is `"2.2"` or `"2.3"` (section 15.4). Bari and the admission service depend on the Rust SDK at such a tag or by path in the same tree; they do not pin any other tag.

<a id="rule-i6"></a>

#### Rule I6. Cost

No SDK implements a cache, table, summary, partial read, or any other shortcut in the terrain, the leg, the step, or the fold (section 11.1). Every step reads and folds the full slot of `SLOT_BYTES`. Where the host provides SHA-256 instructions, the SDK uses them (section 11.1). An SDK change that raises the measured leg rate at unchanged hardware is a suspected break. It is reviewed against the permanent negatives before release. A reviewed SIMD kernel that loads all `SLOT_BYTES` bytes of the selected slot and mixes every word may be released. Processing several words of one slot at once (SIMD, or compiler or JIT vectorisation) is the section 11.1 loop and is not an efficiency or a speed-up in the sense of section 11.1, provided that on every step all `SLOT_BYTES` bytes of the selected slot (16 KiB under the section 6 constants) are loaded, each of the `WORDS_PER_SLOT` words is mixed with the current `S` by `mix64` with `MIX_ADD`, and the sum is the section 11.1 wrapping sum. Such a kernel’s rate gain is reported under this rule with equality, mutation and load evidence, and after that review it may be released. Skipping, caching, summarising or reusing any word or value across steps remains a break.

<a id="sec-17"></a>


## 17. Clarifications

**Informative.**

The items below restate parts of the construction in the sections named here. This section does not add a second construction.

<a id="sec-17-1"></a>

### 17.1 Waypoint path fetch

Section 13.1 posts `W_1` … `W_17`. It does not post the seed. Asks are for `W_1` … `W_16` under section 13.1. `W_17` is posted and is not asked. No ask is made after the close. Asks are asynchronous, and the object closes at `I_17` with the checked paths that have arrived. Test and live use that one rule. On stages 1–17 the field is `path`, a CBOR byte string, in the key order of section 7.3. Stage 0 has no `path` key. Those bytes are not an input to section 11.4 or section 12.2. A POST that does not succeed, and an ask that is not a checked 200, are a miss. Produce stores empty bytes and writes `path_skip <g>` for `W_1` … `W_16`, and continues. It does not withhold the object. `W_17` writes `not fetched by design 17`. A `skip` is a measurement (Rule A4.5). Rule B4 judges it: `anchored_total ≥ min_anchored_paths`, wherever those waypoints sit. A `present` path is measured in section 14.3, including `CanonicalPathBody` `validate_for` and the chain bind. Verify does not GET the path again.

<a id="sec-17-2"></a>

### 17.2 Portable-index height fetch during verify

Verify reads the portable index. For each stored header, including `last_interrupt`, it GETs `{index-url}/index/{height}` and records `index_comparison` and `T` under Rule A4.2. That header GET is separate from a plant GET of the same URL. A body with no usable `header` is header `not read` and can still be plant `match`. `T` is `header.time`, Bitcoin `nTime`, Unix seconds, only when `index_comparison` is `match`. Otherwise `T` is `not read`. `header.time` is the portable index’s record of `nTime` for the block whose hash is `header.hash`. `index_comparison` uses hash and height only. It does not use `network`. An unsuccessful header read is `not read`. A hash or height that differs, when the read returned them, is `mismatch`, and `T` is `not read`. That record is part of Rule A4.2. The admission service applies section 15 to the record. Verify does not GET an HTTP tip. Verify does not Observe.

A document whose `network` is `synthetic` can still record **match** on the plant comparison. A header that matches that document is not a **mismatch** for that `network`. A **match** against a synthetic document is not a mainnet fact.

<a id="sec-17-3"></a>

### 17.3 Path and chain bind

Section 14.3 records `plant` `match`, `mismatch`, or `not read` for each `present` path among `W_1` … `W_16`. The inclusion-height GET is made only when that height is in the window and that header is a stored interrupt. Otherwise `plant` is `not read` and the index is not read for that path. That not-issued `not read` is not a missing GET under Rule A4.11. `not read` also means an issued read that does not return a document. A document that was read and has no plant for that block, or whose plants do not match, or whose `network` is neither `main` nor `synthetic`, is `mismatch`. A document with no `header` member can still be plant `match`. `anchored` requires the inclusion height in the section 13.1 window, that header stored on the object as an interrupt, and plant `match`. Plant `match` alone is not enough. On `mismatch` and on `not read`, `d_g` is `not defined`. `claimed_block` is the path body’s `block_hash` and `block_height` when those fields can be read, otherwise `not defined`. The measurement is distinct from Rule A4.2. A `skip` is not a plant comparison. Verify does not GET the path from Bari again. The pack field `index_reads` lists only those issued plant heights.

A **match** on a document whose `network` is `synthetic` is not a mainnet fact.

<a id="sec-17-4"></a>

### 17.4 Several words of one slot at once

Section 11.1 folds the `WORDS_PER_SLOT` words of the selected slot with a wrapping sum mod 2^64. That sum is associative and commutative, so a kernel that loads all `SLOT_BYTES` bytes of the slot (16 KiB under the section 6 constants) on every step and mixes several of its words with the current `S` at once, by SIMD or by compiler or JIT vectorisation, gives the same `S_folded`, seal, leg, checkpoint, and waypoint bytes as the one-word-at-a-time loop. It is the section 11.1 loop, not an efficiency (Rule I6). Every step still loads every byte of the slot and mixes every word with that step’s `S`. A kernel that skips, caches, summarises, or reuses any word or value across steps is a break whatever its output on a vector, and the permanent negatives of section 16 stay non-matching.

<a id="sec-17-5"></a>

### 17.5 Floor baseline and the low-end host

**Informative.** This subsection adds no rule. Section 11.1’s informative paragraph calls commodity hardware with SHA-256 instructions, for example an AMD Ryzen 5 5600H, “the baseline that sets the floor and the gate policy.” Section 15.5 names the low end of the owned hosts a measurement covers: an Intel Core i7-10610U laptop, which has no SHA-NI. Section 15.5 is the measuring method. It says that method informs the value a policy sets, and that it does not set one. Section 11.1’s paragraph does not set a lever value either. The two sentences name two hosts. They do not set two lever values. Every host runs the same section 11.1 step.

<a id="sec-18"></a>


## 19. Fixed issues — do not reintroduce

**Normative.** This is the only section of this file that names an earlier construction or an earlier defect. Each entry names a defect class, why it was a defect, and the rule of this file that forbids it. An implementation, a pack vector, or a later revision of this file that reintroduces any of them does not conform. An entry is never removed while the rule it points to stands. The labels F1–F15 are list positions, not rule IDs.

| # | Defect class | Why it was a defect | Rule that forbids it |
|---|---|---|---|
| <a id="fixed-f1"></a>F1 | State-free slot fold (“FatSlot”): `S_folded = S XOR (XOR over w of (word(w) XOR w))`. | The inner XOR does not depend on the state, so one 8-byte value per slot stands in for the 16 KiB slot. A table replaces the slot read and the per-step cost collapses. | Section 11.1 fold `acc = (acc + mix64(word(w) XOR S)) mod 2^64`; Rule I6; permanent negative `slot_summary_fold` (section 16). |
| <a id="fixed-f2"></a>F2 | Precomputable step term `word(w) XOR w`. | The term does not depend on the state, so it can be computed once per slot and reused on every visit. | Section 11.1: the per-word input is `word(w) XOR S`, with no `w` term; negatives `xor_w_fold`, `s_plus_w_fold`, `prefold_hoist`. |
| <a id="fixed-f3"></a>F3 | XOR accumulator fold `acc = acc XOR mix64(…)` (xorshift linearity). | The XOR accumulator is linear across words and across states, so per-slot work separates from the state and a summary can stand in for the read. | Section 11.1 wrapping-add fold; negatives `xor_accumulator_fold`, `final_stage_hoist` (section 16). |
| <a id="fixed-f4"></a>F4 | Partial slot reads, lane and seed shortcuts, shared or cached terrain. | Each lets a producer skip bytes or reuse work and so pay less than the full slot every step. | Sections 10 and 11.1; Rule I6; negatives `partial_slot_read`, `partial_slot_read_2047`, `one_lane_flip`, `all4_kernel`, `f3_shared_terrain`, `cached_leg`, and `squeezed_seed_terrain` (section 16). |
| <a id="fixed-f5"></a>F5 | Waypoint post before the next stage’s legs (blocking post). | The walk waited on Bari and on the Hashcash solve, so legs paused at every boundary and the measured rate fell. | Rule A1.4 (post is asynchronous); Rule A6; Rule I3. |
| <a id="fixed-f6"></a>F6 | The 15–45 s wait before each path ask. | It ran inside the walk and cost about 1,000 s per hike (7.6–8 %). Rates and the floor measured with it understate an honest producer. | Section 13.1 (an ask is sent when due, with no wait); P13.7; Rule A6; section 15.5 (re-measure the floor). |
| <a id="fixed-f7"></a>F7 | One-sided tip-retry reject at Bari. | Bari took a fee only on its own tip, retried the previous tip, and rejected a fee on a header one ahead as final. An honest producer that saw a block first lost the plant. | Rule BR1 (current, before, or after block, plain 402 otherwise); P13.13 (one background resend after a transport failure). |
| <a id="fixed-f8"></a>F8 | Single-peer header intake. | The producer kept one gossip peer and had no stale-peer check. One silent peer blinded it for 10–20 minutes, and co-located producers failed together. | Section 9 **Header source**; P9.16; P9.12; Rule A6. |
| <a id="fixed-f9"></a>F9 | Bari pinned to a separate v2.0 SDK tag. | Bari’s Hashcash, leaf, and path code came from a different construction than the producers it served, so the two could disagree. | Rule I1 (Bari uses an SDK); Rule I5 (one cut tag and `sdk/SPEC_PIN`). |
| <a id="fixed-f10"></a>F10 | A clock-read failure, reorg, or stalled source stopping the run or withholding the object. | A network event became a lost hike, and reorg handling invited producer-side judgment. | P9.12; Rule A6; section 13 (producers ignore reorgs). |
| <a id="fixed-f11"></a>F11 | A collection window or wait after the final block. | It delayed the close and made the object depend on wall-clock policy inside production. | P13.9 (close at `I_17`, at once). |
| <a id="fixed-f12"></a>F12 | Accepting objects of another kind, or more than one pack. | Mixed kinds and packs let two SDKs both claim conformance with different bytes. | Rule A5 and reason `unknown_kind`; section 16 (one pack, `sdk/SPEC_PIN`). |
| <a id="fixed-f13"></a>F13 | Policy values written as rules of this file. | A chosen number stated as normative could not change without a new specification, and readers took it for part of the construction. | Section 15.4 (levers with ranges; values belong to a policy). |
| <a id="fixed-f14"></a>F14 | Operator re-open of a `refused` pair. | A hand re-open had no state-table row, no deciding tip, and no log rule. It was a discretionary override of the held budgets, open to pressure through a claimed outage. | Section 15.2 (`refused` is closed for the pair under that `policy_version`: return that outcome, no new cycle); section 15.3 **Held bound** (the budgets do not reset). |
| <a id="fixed-f15"></a>F15 | Unbounded hold (“a held PCOC is not refused”). | A hold with no finite budget, or a block budget that waited for a tip that never came, kept a submission `held` for ever: never settled, and the gate’s work never ended. | Section 15.3 **Held bound** (`reverify_max_attempts` and `max_held_blocks` are finite integers `≥ 1` that the policy must set; `max_held_blocks` applies to every hold and starts at the first successful tip read; `held_retry_seconds` retries without waiting for a tip advance; reason `held_bound`). |
