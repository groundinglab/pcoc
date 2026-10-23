# Waypoint Publication Process

**Responsibility.** Tree-batching process for accepting, pooling, and serving waypoints. Used by a producer that publishes paths and by a verifier that checks them.

**Source.** Inherited without change of wording from the following sections of Satoshi Hike 2.3 (`spec/satoshi-hike-v2.3/SPEC.md`):

- §13.6 Bari waypoint service
- Rule BR1 Acceptance and Hashcash
- Rule BR2 Deduplication
- Rule BR3 Pool and tree
- Rule BR4 Path serving
- Path format details from §14.3 beyond Construction

---

### 13.6 Bari waypoint service

**Normative for a Bari operator.** Bari is the service at `{bari-url}`. This section states what a conforming Bari does with a waypoint POST and a path ask. It is the whole Bari rule set for this file. An implementation constant does not add another rule.

<a id="rule-br1"></a>

#### Rule BR1. Paid waypoint

Bari only accepts or denies a submitted waypoint. It accepts a POST if and only if the POST carries a valid Hashcash payment solved within ±1 block of Bari’s current block. That is, all of these hold:

1. The body is exactly 32 bytes. `X-Bari-Hashcash-Nonce` is the shortest decimal spelling of a `u64`. `X-Bari-Hashcash-Header` is 160 hexadecimal digits, the 80 fee-header bytes.
2. The fee header is one of these three blocks, judged against Bari’s own view of the Bitcoin best chain when the POST arrives:
   - **current:** its `header_hash` equals the hash of Bari’s tip;
   - **before:** its `header_hash` equals the previous-block hash of Bari’s tip;
   - **after:** its previous-block field equals the hash of Bari’s tip, its `nBits` equals the required target for the plane (live plane: the Bitcoin mainnet consensus target for the height of Bari’s tip plus one, including a retarget at a 2016-block boundary; test plane: the fixed synth `nBits` of section 9), and `SHA-256(SHA-256(header))`, read as a little-endian 256-bit integer, is at most the target that `nBits` encodes.
3. The section 13.1 digest over `SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)`, the 32 body bytes, and the nonce has at least 21 leading zero bits. Here `bari_origin` is Bari’s own public origin, serialized as section 13.1 states.

An accepted waypoint is answered 204 and Bari holds it. Nothing else about timing applies: the gap between Bari’s tip and the producer’s tip, the arrival time, and which of the three blocks the fee was solved on do not change the answer. Any other POST, unpaid or invalid in any way, is answered with a plain 402 and nothing more. Bari’s only answers to a waypoint POST are 204 and 402.

<a id="rule-br2"></a>

#### Rule BR2. Dedupe

**Normative.** Dedupe applies only to paid posts. The dedupe key is the 32 waypoint bytes within one pool. A POST that meets Rule BR1 and whose waypoint Bari already holds in the current pool is answered 204 and makes no second entry. Any POST without a valid fee is answered with a plain 402, duplicate or not. Bari checks the fee before dedupe.

<a id="rule-br3"></a>

#### Rule BR3. Pool and tree

**Normative.** Bari appends each accepted waypoint to its pool. When a beacon arrives, that is, a new block on Bari’s best chain, Bari builds the tree from the pool, and that clears the pool for the next stage. Pool capacity, and what happens when a pool is full, are out of scope of this file.

<a id="rule-br4"></a>

#### Rule BR4. Path serve

**Normative.** `GET {bari-url}/path/{leaf}` answers 200 with the canonical path body (section 14.3) once the waypoint’s plant is included in a block, through the last offset of the anchor window (section 13.1). Otherwise it answers 404 with exactly one of the phrases `still in the pool`, `sealed but not in a block yet`, `not included`, `already archived after it was served`, or `unknown`. A path ask is never refused for timing.

**Informative.** Bari’s cost per entry is one Hashcash fee. The fee header can only come from a three-block span of the best chain, so precomputed fees are bounded to that span, and the origin binding stops one fee from being reused at a second Bari operator. The 21-bit difficulty is a constant of this file (P13.5).

<a id="sec-14"></a>


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

