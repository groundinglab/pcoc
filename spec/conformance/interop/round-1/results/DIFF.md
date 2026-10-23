# Round 1 diff: B1 (Sonnet 5.5 low, TypeScript) vs B2 (GPT-5.6 Sol low, Python)

Written 22 Oct 2026, ACDT. Inputs:
- B1: builder repository @ `a3ac71d74a44ab07c9fc441e41af2e9f46411187`. It was read file by file. All 67 blobs are under `b1-sonnet-ts/`, and each one's git blob SHA matches the tree. The 30 `objects/index/*.json` files were regenerated with B1's own `writeSyntheticIndex`, and they are byte-identical to the repo blobs.
- B2: a git bundle of the builder repository, cloned to `b2-sol-py/` @ `14e8c50d6f6d893886644c454b89e4d29c7516a1`. **All 14 candidate files are in the bundle.** The store's `candidates/` copy also has all 14 now, and it is identical to the bundle, as is `objects/`. Nothing is "not produced".
- SPEC: pcoc `2b50df7` `spec/satoshi-hike-v2.2/SPEC.md`, sha256 `3055ef48…6319`. Line numbers below are for that file (local copy `round1/SPEC.md`).
- Tools: `diffcand.py` (raw keys), `norm.py` (key-spelling normalisation, then `diff.json`), `hc_check.py` (independent Hashcash recompute).

## Headline

Only `seed.txt` is byte-identical as a file. That is because CASES.md fixes the key prefix but not the rest of the key layout. Each build spelled the keys its own way: `stage00.leg00` vs `stage.0.leg.0`, `pk_A.` included or not, `S_in`/`S_out` vs `S`/`S_next`, `final` vs `stored`, `sha256` vs `digest`, `record_hex` vs `record`.

**After normalising key spelling only, every construction value agrees.** That covers the seed, NL4 base and stored words, the terrain digests (S1, S2 and PROD), the per-step fold trace, all 54 kat-small and 126 test-3leg leg/checkpoint/waypoint values, the PROD leg and checkpoint, `waypoint_equals_seed`, the anchor window, the six-leg draw, the unknown-kind records, and the four fold negatives the SPEC defines exactly.

The two builds' O1, O2 and O3 objects are also the same bytes apart from fixture beacon heights. B1 uses 1000…1018 and B2 uses 100…118. Re-encoding B2's objects with heights +900 gives B1's exact bytes for all three objects, including O3 at production size.

| File | Match | Mismatch | Only B1 | Only B2 | Root cause of the rest |
|---|---|---|---|---|---|
| seed | 12 | 0 | 0 | 0 | — (file byte-identical) |
| nl4 | 48 | 0 | 24 | 0 | C1: CASES fixture (j=8…15 do not exist at S1) |
| terrain | 3 | 0 | 0 | 0 | — |
| fold | 17 | 0 | 0 | 0 | — |
| kat-small | 54 | 0 | 0 | 0 | — |
| test-3leg | 126 | 0 | 0 | 0 | — |
| prod | 3 | 0 | 0 | 0 | — |
| wes | 3 | 0 | 0 | 0 | — |
| rate | 28 | 17 | 0 | 0 | C2: SPEC has no outcome word for "meets" (`meets` vs `meet`) |
| anchor | 7 | 0 | 0 | 0 | — |
| draw | 3 | 0 | 5 | 0 | C3: B2 omitted the thorough-round draw (CASES wording + B2 incomplete) |
| unknown-kind | 4 | 0 | 0 | 0 | — |
| hashcash | 0 | 7 | 4 | 0 | C4: different fixture inputs (CASES lets each build pick); both are SPEC-correct |
| negatives | 16 | 8 | 0 | 0 | C5: SPEC names 8 negatives without defining them, plus B2 defects |
| **Total** | **304** | **32** | **33** | **0** | |

(Raw, un-normalised key matching gives 12 matches in total. Every other key differs only in spelling.)

### Key spelling (fixture-format issue, not a SPEC issue)
CASES.md L14 gives `<case>.<fixture>.<…>.<field>` and says "Stage and leg indices are decimal". It does not fix the separators, zero-padding, whether `pk_A` appears in the key, or per-field names such as the fold input/output state and the stored NL4 word. The SPEC names `S`, `S'`, `acc` and `leg_result`, but it has no names for "base word" vs "final stored word". **Fix in CASES (round 2):** publish the exact key template for every case, e.g. `kat-small.S1.stage.<g>.leg.<j>.legs_le_hex`, unpadded, with no `pk_A` when the case has only one key. Then the files can be compared with `cmp`.

## Mismatches grouped by root cause

### C1. NL4 j = 0…15 at S1 (CASES fixture at fault)
- CASES case 2 asks for words j = 0…15 of slots 0, 1 and `N_SLOTS−1`. S1 has `SLOT_BYTES` = 64, so `WORDS_PER_SLOT` = 8 (SPEC §6 P6.23 L126; §16.1 L2092).
- B1 emitted the NL4 **base** formula for j = 8…15 as well (24 extra `base_u64` keys, with no stored word). The §10.4 formula (L749–756) is defined for `j in 0 … WORDS_PER_SLOT − 1`, so values for j ≥ 8 are outside the SPEC's domain. B2 emitted j = 0…7 only.
- j = 0…7 (base and stored) agree on all 48 keys.
- **Verdict:** a CASES bug, as already known. The SPEC is clear. Fix: ask for j = 0…7 at S1, or take the NL4 case at S2 or PROD.

### C2. `rate` outcome word: `meets` (B1) vs `meet` (B2), 17 keys
- Every refuse/meet decision agrees on all 45 grid points. Only the spelling of the passing outcome differs.
- SPEC Rule B2 L1973: "a ratio `p/q` **meets the floor** when `hike_span` is an integer greater than 0 and `q · hike_legs ≥ p · hike_span` … The outcome is `refuse`." The SPEC gives a token for the failing side (`refuse`) and none for the passing side. "Meets" is only a verb. The §15 outcome vocabulary (L1683: `provisional`, `accept`, `refuse`, `revoke`, `held`) has no "passes the floor" word, because passing the floor alone is not `accept`.
- **Verdict:** genuinely ambiguous, compounded by CASES case 9 ("the SPEC's outcome word"), which assumes such a word exists. Fix: either the SPEC names a comparison token (e.g. "`meets` / `below`", defined in B2 or §4), or CASES defines the vector tokens. B2 also flagged this.

### C3. `draw`: thorough rounds missing in B2 (5 keys only in B1)
- The six-leg indices for leg totals 6, 18 and 54 agree (§14.6 Rule A4.4, L1624–1656).
- B1 also emitted `thorough.indices` (18 indices, 3 rounds, per §15.5 L2005–2007 and the pseudocode after it), `thorough_rounds = 3` and the `b0_label`. B2 emitted only the six-leg draw.
- CASES case 11 says "the §14.6 thorough-round / six-leg index draw". The slash can be read as "these are the same thing" or as "both". The SPEC pack list (§16 L2065) publishes "the thorough-round index draw", and L2073 says `kat-small`/`test-3leg` do not pin a six-leg draw. So the pack vector is the §15.5 thorough draw. But `thorough_rounds` is a §15.4 policy lever, not a SPEC constant, so the SPEC never says which round count that vector uses, and the draw itself is described in §15.5, not §14.6.
- Independent check (reviewer, Python, written from §14.6 and §15.5 only): B1's thorough indices for 6, 18 and 54 legs are reproduced exactly, with each draw restarting the byte source at `b_0`. Whether the thorough draw restarts the source or continues after the six-leg draw isn't stated in CASES.
- **Verdict:** CASES is ambiguous, and the SPEC has a gap: the vector depends on a policy lever with no fixture value named. B2 is incomplete. B1's thorough output can't be cross-checked until another build emits it. Fix: CASES (or §16) names `thorough_rounds = 3` for the vector and asks for both the six-leg and the thorough indices under fixed key names.

### C4. `hashcash`: different inputs, both correct (7 mismatches, 4 only in B1)
- CASES L23 lets each core choose fixture names. B1 used origin `https://<sha256('pcoc-pack-2.2/bari_origin')[0:16] hex>.invalid` plus a synthetic 80-byte fee header and a waypoint, and published all of them. B2 used origin `https://example.com` (not derived per CASES L23), plus a header_hash and waypoint from unstated labels, and did not publish the fee header.
- I recomputed both independently (`hc_check.py`). Both match §13.1 (L1051–1055): `challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)`, digest `SHA-256(challenge ‖ waypoint ‖ le64(nonce))`, and the first nonce from 0 with ≥ 21 leading zero bits. B1 nonce 1548251, B2 nonce 4447969, both minimal. For B1, header_hash = dSHA256(fee_header) holds as well.
- **Verdict:** a fixture mismatch, not a disagreement. The SPEC is clear. Fix: CASES fixes the origin string, the 80 fee-header bytes and the waypoint.

### C5. `negatives`: 8 of 12 values differ
- **These four agree:** `slot_summary_fold`, `xor_accumulator_fold`, `xor_w_fold`, `s_plus_w_fold`. The SPEC defines each by formula in §11.1 L824.
- **These eight differ:** `prefold_hoist`, `partial_slot_read`, `partial_slot_read_2047`, `one_lane_flip`, `all4_kernel`, `f3_shared_terrain`, `cached_leg`, `squeezed_seed_terrain`. The SPEC only **names** them: §16 L2067, and §19 F2/F4 L2167/L2169. There is no construction for any of them:
  - `prefold_hoist`: no formula anywhere. B1 used the informative hoist of §11.1 L822 (XOR of pre-final values, then the final `y ^ (y>>31)` once). B2 made it identical to `xor_w_fold`.
  - `partial_slot_read`: no read length given. B1 read half the slot, B2 read one word.
  - `partial_slot_read_2047`: CASES says "at PROD `WORDS_PER_SLOT` semantics", but the case is on S1. B1 read `wps−1` words (7 of 8). B2 read `words[:2047]`, which at S1 is the **whole slot**, so its value equals the honest leg (`4c9a6a98574cbd31`).
  - `one_lane_flip` and `all4_kernel`: §10.4 L762–764 defines a "one-lane map" and a "linear map" informatively, but does not tie them to these names or say whether the parent mixing and `slot_mix` still apply. B1 used those maps with the full dag2 parents and `slot_mix`.
  - `f3_shared_terrain`: B1 ran pk_A's leg on pk_B's terrain.
  - `cached_leg`: B1 used a per-slot fold cache keyed by slot index.
  - `squeezed_seed_terrain`: §10.2 L721–723 forbids "a squeeze of the seed to one 64-bit fill pin" but never defines the squeeze. B1 used `P0⊕P1⊕P2⊕P3` as all four lanes.
- **B2 defects (misreads, not SPEC ambiguity):**
  1. `one_lane_flip`, `all4_kernel`, `f3_shared_terrain` and `squeezed_seed_terrain` are **placeholders**: `sha256(name + honest_digest)` (generate.py L98–99). No shortcut is computed, which violates CASES case 14 ("computed by the shortcut it names").
  2. `cached_leg` falls through to the honest fold (negatives.py has no cache branch), so its value **equals the honest leg** `4c9a6a98574cbd31`.
  3. `partial_slot_read_2047` also equals the honest leg at S1.
  4. `matches_honest` is hard-coded `"false"` (generate.py L102), so items 2 and 3 are **false claims**: they match honest but report `false`.
- B1 checks `value !== honest` at run time and throws otherwise (negatives.ts, end of `computeNegatives`).
- **Verdict:** the SPEC is genuinely underspecified for 8 permanent negatives. Each one needs a normative construction (or the pack has to define it), because §16 makes "reproduces every permanent negative as a non-match" a conformance condition. The `partial_slot_read_2047`-at-S1 wording is a CASES fault.

## Cross-verify (Task 3)

I ran each build's verifier CLI on the box: B1 under Node 22.20.0 (installed at `<work>`, because B1 needs Node ≥ 22.18 for native .ts), B2 under Python 3.13.5 with numpy 2.5.4. The records are in `round1/xverify/`.

Objects: O1 = S1, 18×1 legs. O2 = S2, 18×3 legs. O3 = PROD, 18×1 legs. S1 and S2 run under `--profile conformance-small` with the CASES integers. O3 runs at production.

| Verifier → object | shape | seed | checkpoints (A4.3) | waypoints | six-leg recompute | headers vs index | time |
|---|---|---|---|---|---|---|---|
| B1 → B1 O1 | match | match | 18/18 match | 17/17 match | 6/6 match | match (B1 index) | <1 s |
| B1 → B2 O1 | match | match | 18/18 match | 17/17 match | 6/6 match | not read ¹ | <1 s |
| B2 → B2 O1 | match | match | 18/18 match | 17/17 match | 6/6 match | not read ² | <1 s |
| B2 → B1 O1 | match | match | 18/18 match | 17/17 match | 6/6 match | not read ² | <1 s |
| B1 → B1 O2 | match | match | 54/54 match | 17/17 match | 6/6 match | match | ~1 s |
| B1 → B2 O2 | match | match | 54/54 match | 17/17 match | 6/6 match | not read ¹ | ~1 s |
| B2 → B2 O2 | match | match | 54/54 match | 17/17 match | 6/6 match | not read ² | ~1 s |
| B2 → B1 O2 | match | match | 54/54 match | 17/17 match | 6/6 match | not read ² | ~1 s |
| B1 → B2 O3 (PROD) | match | match | 18/18 match | 17/17 match | 6/6 match (6 production legs) | not read ¹ | 60 s |
| B2 → B1 O3 (PROD) | match | match | 18/18 match | 17/17 match | 6/6 match (6 production legs) | not read ² | 267 s |

¹ B2's objects carry heights 100…118, and B1's synthetic index only covers 989…1018, so those reads are `not read`. That's a fixture difference, not a disagreement.
² B2's verifier CLI has no index input at all. Its own RESULTS.md says index, MTP and path handling is incomplete, and `anchors`/`anchored_total` are hard-coded `not anchored`/0 (verify.py L109, L134). So bookends, spans and `hike_span` are `not read` under B2 whatever the object.

**Sanity negative:** I flipped one bit of stage 5's leg result in O1 (`xverify/O1-tampered-leg5.cbor`). Both verifiers record checkpoint `mismatch` at stage 5. B1's six-leg draw happened to pick index 5 and recorded `mismatch` there too.

**Object bytes:** with B2's heights shifted +900, B2's O1, O2 and O3 encode to exactly B1's bytes (B2's encoder, which also round-trips B1's bytes unchanged). So the two producers' deterministic CBOR (§7.3) agrees, and all 18 production legs, checkpoints and waypoints of O3 agree.

**Verify-record shape:** both verifiers emit the same key set and order for the §14 record (profile … analysis). They differ only where B2 has no index reader.

## Not mismatches, but noted
- **Slot 0 `slot_mix` (B1 question).** SPEC §10.4 L766 says "Slot 0 keeps those words. It has no parent lanes and no slot mix", and L788 says "For every slot, including slot 0, write …". This is clear. Both builds apply no mix, and the terrain digests agree.
- **Object beacon heights.** These are fixture choices that CASES doesn't fix (B1 1000+, B2 100+). The objects are otherwise byte-identical.
- **Sort order.** Both files are byte-sorted. B2's unpadded indices sort `stage.10` before `stage.2`, which is legal but awkward. Round 2 should fix the padding.

## Appendix: per-file key lists (canonical key names)

Match/mismatch is computed after normalising key spelling only (see *Key spelling*). Values are compared byte for byte.

### seed.txt

- Matching keys (12):
  `seed.pk_A.D`, `seed.pk_A.P0_u64`, `seed.pk_A.P1_u64`, `seed.pk_A.P2_u64`, `seed.pk_A.P3_u64`, `seed.pk_A.preimage`, `seed.pk_B.D`, `seed.pk_B.P0_u64`, `seed.pk_B.P1_u64`, `seed.pk_B.P2_u64`, `seed.pk_B.P3_u64`, `seed.pk_B.preimage`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### nl4.txt

- Matching keys (48):
  `nl4.S1.slot.0.word.0.base_u64`, `nl4.S1.slot.0.word.0.stored_u64`, `nl4.S1.slot.0.word.1.base_u64`, `nl4.S1.slot.0.word.1.stored_u64`, `nl4.S1.slot.0.word.2.base_u64`, `nl4.S1.slot.0.word.2.stored_u64`, `nl4.S1.slot.0.word.3.base_u64`, `nl4.S1.slot.0.word.3.stored_u64`, `nl4.S1.slot.0.word.4.base_u64`, `nl4.S1.slot.0.word.4.stored_u64`, `nl4.S1.slot.0.word.5.base_u64`, `nl4.S1.slot.0.word.5.stored_u64`, `nl4.S1.slot.0.word.6.base_u64`, `nl4.S1.slot.0.word.6.stored_u64`, `nl4.S1.slot.0.word.7.base_u64`, `nl4.S1.slot.0.word.7.stored_u64`, `nl4.S1.slot.1.word.0.base_u64`, `nl4.S1.slot.1.word.0.stored_u64`, `nl4.S1.slot.1.word.1.base_u64`, `nl4.S1.slot.1.word.1.stored_u64`, `nl4.S1.slot.1.word.2.base_u64`, `nl4.S1.slot.1.word.2.stored_u64`, `nl4.S1.slot.1.word.3.base_u64`, `nl4.S1.slot.1.word.3.stored_u64`, `nl4.S1.slot.1.word.4.base_u64`, `nl4.S1.slot.1.word.4.stored_u64`, `nl4.S1.slot.1.word.5.base_u64`, `nl4.S1.slot.1.word.5.stored_u64`, `nl4.S1.slot.1.word.6.base_u64`, `nl4.S1.slot.1.word.6.stored_u64`, `nl4.S1.slot.1.word.7.base_u64`, `nl4.S1.slot.1.word.7.stored_u64`, `nl4.S1.slot.1023.word.0.base_u64`, `nl4.S1.slot.1023.word.0.stored_u64`, `nl4.S1.slot.1023.word.1.base_u64`, `nl4.S1.slot.1023.word.1.stored_u64`, `nl4.S1.slot.1023.word.2.base_u64`, `nl4.S1.slot.1023.word.2.stored_u64`, `nl4.S1.slot.1023.word.3.base_u64`, `nl4.S1.slot.1023.word.3.stored_u64`, `nl4.S1.slot.1023.word.4.base_u64`, `nl4.S1.slot.1023.word.4.stored_u64`, `nl4.S1.slot.1023.word.5.base_u64`, `nl4.S1.slot.1023.word.5.stored_u64`, `nl4.S1.slot.1023.word.6.base_u64`, `nl4.S1.slot.1023.word.6.stored_u64`, `nl4.S1.slot.1023.word.7.base_u64`, `nl4.S1.slot.1023.word.7.stored_u64`
- Mismatching keys (0): none
- Only in B1 (24):
  - `nl4.S1.slot.0.word.10.base_u64` = `dc54b82bbb0753f9`
  - `nl4.S1.slot.0.word.11.base_u64` = `81e984fe1ba03fae`
  - `nl4.S1.slot.0.word.12.base_u64` = `520e6604d20acf0f`
  - `nl4.S1.slot.0.word.13.base_u64` = `c3ced43f07dd374c`
  - `nl4.S1.slot.0.word.14.base_u64` = `14f762b1d29ffcf7`
  - `nl4.S1.slot.0.word.15.base_u64` = `bdaa7792353ac4f7`
  - `nl4.S1.slot.0.word.8.base_u64` = `3be9a5eaa5ec713b`
  - `nl4.S1.slot.0.word.9.base_u64` = `d1135b41308bce0f`
  - `nl4.S1.slot.1.word.10.base_u64` = `195257e9e0df4751`
  - `nl4.S1.slot.1.word.11.base_u64` = `7051ac047fc71436`
  - `nl4.S1.slot.1.word.12.base_u64` = `8c46e5c95ecdd66a`
  - `nl4.S1.slot.1.word.13.base_u64` = `fb4da41cfb45b64c`
  - `nl4.S1.slot.1.word.14.base_u64` = `e7b8766a59e6849d`
  - `nl4.S1.slot.1.word.15.base_u64` = `5a864920b515ba83`
  - `nl4.S1.slot.1.word.8.base_u64` = `2808b6bca1520ebb`
  - `nl4.S1.slot.1.word.9.base_u64` = `e9cc4757449d630d`
  - `nl4.S1.slot.1023.word.10.base_u64` = `df4aa60dbc9914da`
  - `nl4.S1.slot.1023.word.11.base_u64` = `52077769b7203681`
  - `nl4.S1.slot.1023.word.12.base_u64` = `b4ab6a9b4c614e83`
  - `nl4.S1.slot.1023.word.13.base_u64` = `6a63e550e8e1da6f`
  - `nl4.S1.slot.1023.word.14.base_u64` = `71f72b044a47e13a`
  - `nl4.S1.slot.1023.word.15.base_u64` = `298d2ba90b748e5a`
  - `nl4.S1.slot.1023.word.8.base_u64` = `7e263c1d1a40dd97`
  - `nl4.S1.slot.1023.word.9.base_u64` = `12d8c5468b8f2f06`
- Only in B2 (0): none

### terrain.txt

- Matching keys (3):
  `terrain.PROD.digest`, `terrain.S1.digest`, `terrain.S2.digest`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### fold.txt

- Matching keys (17):
  `fold.S1.leg_result_u64`, `fold.S1.step.0.S_next_u64`, `fold.S1.step.0.S_u64`, `fold.S1.step.0.acc_u64`, `fold.S1.step.0.slot_index`, `fold.S1.step.1.S_next_u64`, `fold.S1.step.1.S_u64`, `fold.S1.step.1.acc_u64`, `fold.S1.step.1.slot_index`, `fold.S1.step.2.S_next_u64`, `fold.S1.step.2.S_u64`, `fold.S1.step.2.acc_u64`, `fold.S1.step.2.slot_index`, `fold.S1.step.3.S_next_u64`, `fold.S1.step.3.S_u64`, `fold.S1.step.3.acc_u64`, `fold.S1.step.3.slot_index`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### kat-small.txt

- Matching keys (54):
  `kat-small.S1.stage.0.leg.0.checkpoint`, `kat-small.S1.stage.0.leg.0.legs_le_hex`, `kat-small.S1.stage.0.waypoint`, `kat-small.S1.stage.1.leg.0.checkpoint`, `kat-small.S1.stage.1.leg.0.legs_le_hex`, `kat-small.S1.stage.1.waypoint`, `kat-small.S1.stage.10.leg.0.checkpoint`, `kat-small.S1.stage.10.leg.0.legs_le_hex`, `kat-small.S1.stage.10.waypoint`, `kat-small.S1.stage.11.leg.0.checkpoint`, `kat-small.S1.stage.11.leg.0.legs_le_hex`, `kat-small.S1.stage.11.waypoint`, `kat-small.S1.stage.12.leg.0.checkpoint`, `kat-small.S1.stage.12.leg.0.legs_le_hex`, `kat-small.S1.stage.12.waypoint`, `kat-small.S1.stage.13.leg.0.checkpoint`, `kat-small.S1.stage.13.leg.0.legs_le_hex`, `kat-small.S1.stage.13.waypoint`, `kat-small.S1.stage.14.leg.0.checkpoint`, `kat-small.S1.stage.14.leg.0.legs_le_hex`, `kat-small.S1.stage.14.waypoint`, `kat-small.S1.stage.15.leg.0.checkpoint`, `kat-small.S1.stage.15.leg.0.legs_le_hex`, `kat-small.S1.stage.15.waypoint`, `kat-small.S1.stage.16.leg.0.checkpoint`, `kat-small.S1.stage.16.leg.0.legs_le_hex`, `kat-small.S1.stage.16.waypoint`, `kat-small.S1.stage.17.leg.0.checkpoint`, `kat-small.S1.stage.17.leg.0.legs_le_hex`, `kat-small.S1.stage.17.waypoint`, `kat-small.S1.stage.2.leg.0.checkpoint`, `kat-small.S1.stage.2.leg.0.legs_le_hex`, `kat-small.S1.stage.2.waypoint`, `kat-small.S1.stage.3.leg.0.checkpoint`, `kat-small.S1.stage.3.leg.0.legs_le_hex`, `kat-small.S1.stage.3.waypoint`, `kat-small.S1.stage.4.leg.0.checkpoint`, `kat-small.S1.stage.4.leg.0.legs_le_hex`, `kat-small.S1.stage.4.waypoint`, `kat-small.S1.stage.5.leg.0.checkpoint`, `kat-small.S1.stage.5.leg.0.legs_le_hex`, `kat-small.S1.stage.5.waypoint`, `kat-small.S1.stage.6.leg.0.checkpoint`, `kat-small.S1.stage.6.leg.0.legs_le_hex`, `kat-small.S1.stage.6.waypoint`, `kat-small.S1.stage.7.leg.0.checkpoint`, `kat-small.S1.stage.7.leg.0.legs_le_hex`, `kat-small.S1.stage.7.waypoint`, `kat-small.S1.stage.8.leg.0.checkpoint`, `kat-small.S1.stage.8.leg.0.legs_le_hex`, `kat-small.S1.stage.8.waypoint`, `kat-small.S1.stage.9.leg.0.checkpoint`, `kat-small.S1.stage.9.leg.0.legs_le_hex`, `kat-small.S1.stage.9.waypoint`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### test-3leg.txt

- Matching keys (126):
  `test-3leg.S2.stage.0.leg.0.checkpoint`, `test-3leg.S2.stage.0.leg.0.legs_le_hex`, `test-3leg.S2.stage.0.leg.1.checkpoint`, `test-3leg.S2.stage.0.leg.1.legs_le_hex`, `test-3leg.S2.stage.0.leg.2.checkpoint`, `test-3leg.S2.stage.0.leg.2.legs_le_hex`, `test-3leg.S2.stage.0.waypoint`, `test-3leg.S2.stage.1.leg.0.checkpoint`, `test-3leg.S2.stage.1.leg.0.legs_le_hex`, `test-3leg.S2.stage.1.leg.1.checkpoint`, `test-3leg.S2.stage.1.leg.1.legs_le_hex`, `test-3leg.S2.stage.1.leg.2.checkpoint`, `test-3leg.S2.stage.1.leg.2.legs_le_hex`, `test-3leg.S2.stage.1.waypoint`, `test-3leg.S2.stage.10.leg.0.checkpoint`, `test-3leg.S2.stage.10.leg.0.legs_le_hex`, `test-3leg.S2.stage.10.leg.1.checkpoint`, `test-3leg.S2.stage.10.leg.1.legs_le_hex`, `test-3leg.S2.stage.10.leg.2.checkpoint`, `test-3leg.S2.stage.10.leg.2.legs_le_hex`, `test-3leg.S2.stage.10.waypoint`, `test-3leg.S2.stage.11.leg.0.checkpoint`, `test-3leg.S2.stage.11.leg.0.legs_le_hex`, `test-3leg.S2.stage.11.leg.1.checkpoint`, `test-3leg.S2.stage.11.leg.1.legs_le_hex`, `test-3leg.S2.stage.11.leg.2.checkpoint`, `test-3leg.S2.stage.11.leg.2.legs_le_hex`, `test-3leg.S2.stage.11.waypoint`, `test-3leg.S2.stage.12.leg.0.checkpoint`, `test-3leg.S2.stage.12.leg.0.legs_le_hex`, `test-3leg.S2.stage.12.leg.1.checkpoint`, `test-3leg.S2.stage.12.leg.1.legs_le_hex`, `test-3leg.S2.stage.12.leg.2.checkpoint`, `test-3leg.S2.stage.12.leg.2.legs_le_hex`, `test-3leg.S2.stage.12.waypoint`, `test-3leg.S2.stage.13.leg.0.checkpoint`, `test-3leg.S2.stage.13.leg.0.legs_le_hex`, `test-3leg.S2.stage.13.leg.1.checkpoint`, `test-3leg.S2.stage.13.leg.1.legs_le_hex`, `test-3leg.S2.stage.13.leg.2.checkpoint`, `test-3leg.S2.stage.13.leg.2.legs_le_hex`, `test-3leg.S2.stage.13.waypoint`, `test-3leg.S2.stage.14.leg.0.checkpoint`, `test-3leg.S2.stage.14.leg.0.legs_le_hex`, `test-3leg.S2.stage.14.leg.1.checkpoint`, `test-3leg.S2.stage.14.leg.1.legs_le_hex`, `test-3leg.S2.stage.14.leg.2.checkpoint`, `test-3leg.S2.stage.14.leg.2.legs_le_hex`, `test-3leg.S2.stage.14.waypoint`, `test-3leg.S2.stage.15.leg.0.checkpoint`, `test-3leg.S2.stage.15.leg.0.legs_le_hex`, `test-3leg.S2.stage.15.leg.1.checkpoint`, `test-3leg.S2.stage.15.leg.1.legs_le_hex`, `test-3leg.S2.stage.15.leg.2.checkpoint`, `test-3leg.S2.stage.15.leg.2.legs_le_hex`, `test-3leg.S2.stage.15.waypoint`, `test-3leg.S2.stage.16.leg.0.checkpoint`, `test-3leg.S2.stage.16.leg.0.legs_le_hex`, `test-3leg.S2.stage.16.leg.1.checkpoint`, `test-3leg.S2.stage.16.leg.1.legs_le_hex`, `test-3leg.S2.stage.16.leg.2.checkpoint`, `test-3leg.S2.stage.16.leg.2.legs_le_hex`, `test-3leg.S2.stage.16.waypoint`, `test-3leg.S2.stage.17.leg.0.checkpoint`, `test-3leg.S2.stage.17.leg.0.legs_le_hex`, `test-3leg.S2.stage.17.leg.1.checkpoint`, `test-3leg.S2.stage.17.leg.1.legs_le_hex`, `test-3leg.S2.stage.17.leg.2.checkpoint`, `test-3leg.S2.stage.17.leg.2.legs_le_hex`, `test-3leg.S2.stage.17.waypoint`, `test-3leg.S2.stage.2.leg.0.checkpoint`, `test-3leg.S2.stage.2.leg.0.legs_le_hex`, `test-3leg.S2.stage.2.leg.1.checkpoint`, `test-3leg.S2.stage.2.leg.1.legs_le_hex`, `test-3leg.S2.stage.2.leg.2.checkpoint`, `test-3leg.S2.stage.2.leg.2.legs_le_hex`, `test-3leg.S2.stage.2.waypoint`, `test-3leg.S2.stage.3.leg.0.checkpoint`, `test-3leg.S2.stage.3.leg.0.legs_le_hex`, `test-3leg.S2.stage.3.leg.1.checkpoint`, `test-3leg.S2.stage.3.leg.1.legs_le_hex`, `test-3leg.S2.stage.3.leg.2.checkpoint`, `test-3leg.S2.stage.3.leg.2.legs_le_hex`, `test-3leg.S2.stage.3.waypoint`, `test-3leg.S2.stage.4.leg.0.checkpoint`, `test-3leg.S2.stage.4.leg.0.legs_le_hex`, `test-3leg.S2.stage.4.leg.1.checkpoint`, `test-3leg.S2.stage.4.leg.1.legs_le_hex`, `test-3leg.S2.stage.4.leg.2.checkpoint`, `test-3leg.S2.stage.4.leg.2.legs_le_hex`, `test-3leg.S2.stage.4.waypoint`, `test-3leg.S2.stage.5.leg.0.checkpoint`, `test-3leg.S2.stage.5.leg.0.legs_le_hex`, `test-3leg.S2.stage.5.leg.1.checkpoint`, `test-3leg.S2.stage.5.leg.1.legs_le_hex`, `test-3leg.S2.stage.5.leg.2.checkpoint`, `test-3leg.S2.stage.5.leg.2.legs_le_hex`, `test-3leg.S2.stage.5.waypoint`, `test-3leg.S2.stage.6.leg.0.checkpoint`, `test-3leg.S2.stage.6.leg.0.legs_le_hex`, `test-3leg.S2.stage.6.leg.1.checkpoint`, `test-3leg.S2.stage.6.leg.1.legs_le_hex`, `test-3leg.S2.stage.6.leg.2.checkpoint`, `test-3leg.S2.stage.6.leg.2.legs_le_hex`, `test-3leg.S2.stage.6.waypoint`, `test-3leg.S2.stage.7.leg.0.checkpoint`, `test-3leg.S2.stage.7.leg.0.legs_le_hex`, `test-3leg.S2.stage.7.leg.1.checkpoint`, `test-3leg.S2.stage.7.leg.1.legs_le_hex`, `test-3leg.S2.stage.7.leg.2.checkpoint`, `test-3leg.S2.stage.7.leg.2.legs_le_hex`, `test-3leg.S2.stage.7.waypoint`, `test-3leg.S2.stage.8.leg.0.checkpoint`, `test-3leg.S2.stage.8.leg.0.legs_le_hex`, `test-3leg.S2.stage.8.leg.1.checkpoint`, `test-3leg.S2.stage.8.leg.1.legs_le_hex`, `test-3leg.S2.stage.8.leg.2.checkpoint`, `test-3leg.S2.stage.8.leg.2.legs_le_hex`, `test-3leg.S2.stage.8.waypoint`, `test-3leg.S2.stage.9.leg.0.checkpoint`, `test-3leg.S2.stage.9.leg.0.legs_le_hex`, `test-3leg.S2.stage.9.leg.1.checkpoint`, `test-3leg.S2.stage.9.leg.1.legs_le_hex`, `test-3leg.S2.stage.9.leg.2.checkpoint`, `test-3leg.S2.stage.9.leg.2.legs_le_hex`, `test-3leg.S2.stage.9.waypoint`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### prod.txt

- Matching keys (3):
  `prod.PROD.stage.0.leg.0.checkpoint`, `prod.PROD.stage.0.leg.0.legs_le_hex`, `prod.PROD.terrain_digest`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### wes.txt

- Matching keys (3):
  `wes.S1.D`, `wes.S1.equal`, `wes.S1.stage.0.waypoint`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### rate.txt

- Matching keys (28):
  `rate.legs.0.span.1.floor.1_4`, `rate.legs.0.span.1.floor.1_5`, `rate.legs.0.span.1.floor.7_20`, `rate.legs.0.span.300.floor.1_4`, `rate.legs.0.span.300.floor.1_5`, `rate.legs.0.span.300.floor.7_20`, `rate.legs.0.span.3600.floor.1_4`, `rate.legs.0.span.3600.floor.1_5`, `rate.legs.0.span.3600.floor.7_20`, `rate.legs.1.span.300.floor.1_4`, `rate.legs.1.span.300.floor.1_5`, `rate.legs.1.span.300.floor.7_20`, `rate.legs.1.span.3600.floor.1_4`, `rate.legs.1.span.3600.floor.1_5`, `rate.legs.1.span.3600.floor.7_20`, `rate.legs.1000.span.3600.floor.7_20`, `rate.legs.5.span.300.floor.1_4`, `rate.legs.5.span.300.floor.1_5`, `rate.legs.5.span.300.floor.7_20`, `rate.legs.5.span.3600.floor.1_4`, `rate.legs.5.span.3600.floor.1_5`, `rate.legs.5.span.3600.floor.7_20`, `rate.legs.6.span.300.floor.1_4`, `rate.legs.6.span.300.floor.1_5`, `rate.legs.6.span.300.floor.7_20`, `rate.legs.6.span.3600.floor.1_4`, `rate.legs.6.span.3600.floor.1_5`, `rate.legs.6.span.3600.floor.7_20`
- Mismatching keys (17):
  - `rate.legs.1.span.1.floor.1_4`: B1 `meets` / B2 `meet`
  - `rate.legs.1.span.1.floor.1_5`: B1 `meets` / B2 `meet`
  - `rate.legs.1.span.1.floor.7_20`: B1 `meets` / B2 `meet`
  - `rate.legs.1000.span.1.floor.1_4`: B1 `meets` / B2 `meet`
  - `rate.legs.1000.span.1.floor.1_5`: B1 `meets` / B2 `meet`
  - `rate.legs.1000.span.1.floor.7_20`: B1 `meets` / B2 `meet`
  - `rate.legs.1000.span.300.floor.1_4`: B1 `meets` / B2 `meet`
  - `rate.legs.1000.span.300.floor.1_5`: B1 `meets` / B2 `meet`
  - `rate.legs.1000.span.300.floor.7_20`: B1 `meets` / B2 `meet`
  - `rate.legs.1000.span.3600.floor.1_4`: B1 `meets` / B2 `meet`
  - `rate.legs.1000.span.3600.floor.1_5`: B1 `meets` / B2 `meet`
  - `rate.legs.5.span.1.floor.1_4`: B1 `meets` / B2 `meet`
  - `rate.legs.5.span.1.floor.1_5`: B1 `meets` / B2 `meet`
  - `rate.legs.5.span.1.floor.7_20`: B1 `meets` / B2 `meet`
  - `rate.legs.6.span.1.floor.1_4`: B1 `meets` / B2 `meet`
  - `rate.legs.6.span.1.floor.1_5`: B1 `meets` / B2 `meet`
  - `rate.legs.6.span.1.floor.7_20`: B1 `meets` / B2 `meet`
- Only in B1 (0): none
- Only in B2 (0): none

### anchor.txt

- Matching keys (7):
  `anchor.H.100.height.100`, `anchor.H.100.height.101`, `anchor.H.100.height.102`, `anchor.H.100.height.103`, `anchor.H.100.height.104`, `anchor.H.100.height.105`, `anchor.H.100.height.106`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### draw.txt

- Matching keys (3):
  `draw.legs.18.indices`, `draw.legs.54.indices`, `draw.legs.6.indices`
- Mismatching keys (0): none
- Only in B1 (5):
  - `draw.b0_label` = `pcoc-pack-2.2/draw`
  - `draw.legs06.thorough.indices` = `1,2,3,4,0,5,5,2,1,3,4,0,5,4,0,2,3,1`
  - `draw.legs18.thorough.indices` = `7,2,1,15,10,16,6,11,17,13,3,14,4,12,5,8,9,0`
  - `draw.legs54.thorough.indices` = `7,20,1,51,10,16,42,29,17,2,43,49,3,31,39,32,40,30`
  - `draw.thorough_rounds` = `3`
- Only in B2 (0): none

### unknown-kind.txt

- Matching keys (4):
  `unknown-kind.empty.record`, `unknown-kind.satoshi-hike-v2.0-object.record`, `unknown-kind.v2.1-pcoc-object.record`, `unknown-kind.v2.2-pcoc-objecT.record`
- Mismatching keys (0): none
- Only in B1 (0): none
- Only in B2 (0): none

### hashcash.txt

- Matching keys (0): none
- Mismatching keys (7):
  - `hashcash.challenge`: B1 `d1d65714b910b3c8c4122434b2ddee01113bce6152b4b1a7bcf74aa07d4115fa` / B2 `58e405b889824c9ff3444c6e11aea358cc5e216e6fce57d794cb1ef2d3bc4f37`
  - `hashcash.digest`: B1 `0000038b74f5f1fd6994ac122b33d322e10a39810660400272679808ce553725` / B2 `000002afa0802db0c0dc9a5b1879f9c0311efd883c6d25a686136db6831f3411`
  - `hashcash.header_hash`: B1 `9e48d971c25a2c21c95449623e42b620c9037038afb020d12f4126309e849d54` / B2 `287c28c72e4a7d23d88e82afbf601cd8740345018abc6ae9a78eec996c44f2a7`
  - `hashcash.nonce`: B1 `1548251` / B2 `4447969`
  - `hashcash.origin`: B1 `https://4a0e0526df729135d43fac93d9548835.invalid` / B2 `https://example.com`
  - `hashcash.origin_label`: B1 `bari_origin` / B2 `bari-origin`
  - `hashcash.waypoint`: B1 `76c7a4a699009dd5c52b91ad9b8454282b8020ea86b208c1c57ea0311a7e2c28` / B2 `6863fc06e217f9a102ee6f5be8761bc27425c0f0f4c6b722ca03aa4e5fdbd771`
- Only in B1 (4):
  - `hashcash.difficulty` = `21`
  - `hashcash.fee_header` = `3ea15674b0a85ce88df08195d1c7c1079ee8842f808899e133163e510cf0f5a92778563e50493b5f0e61645057e894fec66f583a569585c885040c73fe8b251d06dd8e5f27cc8fe353937a8f29acd279`
  - `hashcash.fee_header_labels` = `hashcash/fee_header/0,hashcash/fee_header/1,hashcash/fee_header/2[0:16]`
  - `hashcash.waypoint_label` = `hashcash/waypoint`
- Only in B2 (0): none

### negatives.txt

- Matching keys (16):
  `negatives.all4_kernel.matches_honest`, `negatives.cached_leg.matches_honest`, `negatives.f3_shared_terrain.matches_honest`, `negatives.one_lane_flip.matches_honest`, `negatives.partial_slot_read.matches_honest`, `negatives.partial_slot_read_2047.matches_honest`, `negatives.prefold_hoist.matches_honest`, `negatives.s_plus_w_fold.matches_honest`, `negatives.s_plus_w_fold.value`, `negatives.slot_summary_fold.matches_honest`, `negatives.slot_summary_fold.value`, `negatives.squeezed_seed_terrain.matches_honest`, `negatives.xor_accumulator_fold.matches_honest`, `negatives.xor_accumulator_fold.value`, `negatives.xor_w_fold.matches_honest`, `negatives.xor_w_fold.value`
- Mismatching keys (8):
  - `negatives.all4_kernel.value`: B1 `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` / B2 `af0b0f2228b6505d58417d075653d61c6489ef9f23a5c5257d5d549d284926de`
  - `negatives.cached_leg.value`: B1 `3b715e2a251a44d9` / B2 `4c9a6a98574cbd31`
  - `negatives.f3_shared_terrain.value`: B1 `a73019b8a2c43cd1` / B2 `3f5d4812c0ad1307d215633817f03cfab9a69bc6a8549e86e08c64846cf112fb`
  - `negatives.one_lane_flip.value`: B1 `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` / B2 `a0325421bbc38a5ee9f494e957298684084086ccefb7821d3e8b9785923d2a45`
  - `negatives.partial_slot_read.value`: B1 `059f134920349f54` / B2 `e1ae647ee5a5d7c5`
  - `negatives.partial_slot_read_2047.value`: B1 `738755dabac824ab` / B2 `4c9a6a98574cbd31`
  - `negatives.prefold_hoist.value`: B1 `b1808c8d3927a7d1` / B2 `73a4c7fbd105aae1`
  - `negatives.squeezed_seed_terrain.value`: B1 `ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22` / B2 `8f5162aa56660daf391411b219fe4f8141f43a4a58ac131f5ab762809244f04d`
- Only in B1 (0): none
- Only in B2 (0): none

