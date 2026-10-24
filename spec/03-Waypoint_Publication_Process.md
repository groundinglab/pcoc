# Waypoint Publication Process

**Responsibility.** Our scoped open Bitcoin-anchor channel: Hashcash → pool → batch/tree → confirm → path fetch window (Bari-shaped). Used by a producer that publishes paths via this channel. Constrained by Construction’s two-way Bitcoin bind.

**Source.** Inherited without change of wording from the following sections of Satoshi Hike 2.3 (tag `spec-2.3.0`; formerly the monolith at `spec/satoshi-hike-v2.3/SPEC.md`, not present on this branch’s public face):

- §13.6 Bari waypoint service
- Rule BR1 Acceptance and Hashcash
- Rule BR2 Deduplication
- Rule BR3 Pool and tree
- Rule BR4 Path serving

**FRAMING (2026-10-24).** Construction owns the required two-way Bitcoin bind (headers into the hike; anchors that make history observe the hike). This document scopes **batching** as the path we specify. Direct-to-Bitcoin plant/confirm/fetch with different timing is a similar but out-of-scope mechanism; Construction’s bind still holds. Plant-*measurement* procedure (former §14.3 path/plant checks against the portable index) lives in [06-Measurement_Record_Process.md](./06-Measurement_Record_Process.md). Canonical path-body layout is part of that measurement text. Bari fetchability (path served in the anchor window) is the real include window for anchored paths.

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

**FRAMING note on BR4 “section 14.3”.** The canonical path body layout formerly adjacent in §14.3 is now in Measurement Record (§14.3 there). Wording of Rule BR4 itself is unchanged.

