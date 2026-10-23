# Satoshi Hike 2.2 interop fixtures, round 2 (groundinglab, 22 Oct 2026)

> Written against **SPEC 2.2 with erratum 1** (`spec/satoshi-hike-v2.2/SPEC.md` on main at 7b40353). Every token and definition below is a SPEC reading, with the SPEC section named. If this file and the SPEC ever differ, the SPEC wins: implement the SPEC and report the difference in RESULTS.md. This file gives inputs and exact keys only. It gives **no expected values**.

Every core is written independently from `spec/satoshi-hike-v2.2/SPEC.md`. Each one emits the **same cases with the same inputs** under the **same keys**, so the pack job can compare files byte for byte. SPEC §16.1 says QA picks the profile integers for published vectors; these are those choices. They are fixture inputs, not SPEC parameters.

## Output format (identical in every language)
- Directory `sdk/<lang>/candidates/`, one file per case group: `seed.txt`, `nl4.txt`, `terrain.txt`, `fold.txt`, `kat-small.txt`, `test-3leg.txt`, `prod.txt`, `wes.txt`, `rate.txt`, `anchor.txt`, `draw.txt`, `unknown-kind.txt`, `hashcash.txt`, `negatives.txt`.
- Each line is `key<TAB>value<LF>`, ASCII only. No blank lines, no comments, no trailing whitespace. Lines are sorted by key in byte order. The file ends with the LF of its last line.
- **Keys are exactly the templates in "Exact keys" below.**
  - Do not rename, pad or reorder their parts.
  - `<i>`, `<j>`, `<g>`, `<n>`, `<L>`, `<s>`, `<h>` are unpadded decimal.
  - Indices are 0-based, as the SPEC numbers them.
  - If you think the SPEC names a value differently, keep the key as given and say so in RESULTS.md.
- Value spellings:
  - **bytes:** lowercase hex, no `0x`.
  - **`_u64` keys:** the integer as 16 lowercase hex digits, big-endian spelling of the number, *not* the wire bytes.
  - **`legs_le_hex`:** lowercase hex of the 8 wire bytes `le64(leg_result)`.
  - **other integers:** decimal.
  - **index lists:** decimal, comma-separated, no spaces, in draw order.
  - **outcomes:** the exact lowercase token given for the case.
- The generator is a command in the SDK, e.g. `cargo run --bin gen-candidates`, `make candidates`, `go run ./cmd/gencandidates`, or a Java main.
  - It writes these files deterministically.
  - A test re-runs it and checks that the committed files are unchanged.
- **RESULTS.md is required and is committed with the candidates.**
  - It lists every SPEC question you met: the SPEC line or section, the readings you considered, and the reading you implemented.
  - It also lists every deviation from this file.
  - A build without a committed RESULTS.md is incomplete.

## Fixed inputs
- `pk_A` = `f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9` (BIP-340 test vector 0 public key; public data).
- `pk_B` = `dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659` (BIP-340 test vector 1 public key; public data).
- Synthetic beacons (test-plane fixtures, 32 bytes each):
  - `seed_beacon` = SHA-256(ASCII `pcoc-pack-2.2/beacon/seed`);
  - `I_k` = SHA-256(ASCII `pcoc-pack-2.2/beacon/` ‖ decimal k), for k = 0 … 17.
- Beacon heights (every object in this file):
  - `seed_beacon` is at height 1000;
  - `I_k` is at height 1001 + k;
  - so `stages[g].beacon_height` = 1000 + g and `last_interrupt_height` = 1018.
- Path bytes on stages 1–17: the empty path bytes of §13.3 (`path_skip`) for every stage.
- Every other fixed byte string is named in its case below. Do not invent fixture inputs. If you think one is missing, say so in RESULTS.md.
- Profiles (each meets the §16.1 constraints):

| Fixture | `SLOT_BYTES` | `SAMPLE_BYTES` | `TERRAIN_BYTES` | `STEPS_PER_LEG` | `legs_per_stage` |
|---|---|---|---|---|---|
| `S1` (`kat-small`) | 64 | 32 | 65536 | 100 | 18 × `1` |
| `S2` (`test-3leg`) | 256 | 64 | 1048576 | 1000 | 18 × `3` |
| `PROD` | section 6 | section 6 | section 6 | section 6 | first leg only |

At S1, `WORDS_PER_SLOT` = 8 and `N_SLOTS` = 1024. At S2 they are 32 and 4096.

## Cases (SPEC §16 construction list)
1. **`seed`**: for `pk_A` and `pk_B`, each with `seed_beacon`: the 64-byte `preimage` (`public_key ‖ seed_beacon`), `D`, and lanes `P0` … `P3`.
2. **`nl4`**: S1, `pk_A`. For slots 0, 1 and 1023, and `j` = 0 … 7 (that is, 0 … `WORDS_PER_SLOT − 1`):
   - the NL4 **base** word (§10.4, before parents and `slot_mix`);
   - the **stored** terrain word (after parent mixing and `slot_mix`; slot 0 as §10.4 defines it).
3. **`terrain`**: the SHA-256 of the whole terrain array for S1, S2 and PROD (`pk_A`).
4. **`fold`**: S1, `pk_A`, the first leg of stage 0. For steps 0 … 3, give:
   - the input state `S`;
   - the slot index read;
   - the §11.1 accumulator `acc` after the full slot;
   - the output state.

   Also give the leg's final `leg_result`.
5. **`kat-small`**: S1, `pk_A`, the full 18-stage hike at `legs_per_stage`.
   - For every leg: `legs_le_hex` and its checkpoint.
   - For every stage `g`: the stored `stages[g].waypoint` field. Stage 0 is the seed `D`; stage g ≥ 1 is `W_g`.
   - The terrain digest.
6. **`test-3leg`**: as `kat-small`, at S2 (3 legs per stage).
7. **`prod`**: PROD, `pk_A`. The terrain digest, and the first leg of stage 0 at the section 6 constants: `legs_le_hex` and its checkpoint.
8. **`wes`**: `waypoint_equals_seed` for S1 `pk_A`. Give `D` and the stage-0 waypoint side by side, and `equal` (`true` / `false`).
9. **`rate`**: the Rule B2 integer comparison `q · hike_legs ≥ p · hike_span`, over this grid:
   - `hike_legs` ∈ {0, 1, 5, 6, 1000};
   - `hike_span` (s) ∈ {1, 300, 3600};
   - floor `p/q` ∈ {1/5, 1/4, 7/20}.

   The value is `meets` when the comparison holds and `refuse` when it does not (SPEC Rule B2; §4 **Comparison result**).
10. **`anchor`**: the integer anchor comparison (SPEC §16.1 **Anchor comparison**; `ANCHOR_WINDOW_OFFSETS` 2, 3, 4 after stored stage height `H`) for `H` = 100 and inclusion heights 100 … 106.
    - As that paragraph says, the header at that height is taken as an interrupt stored on the object with plant comparison `match`; neither condition is tested.
    - The value is `anchored` or `not anchored`.
11. **`draw`**:
    - The byte source is the SPEC §16.1 **Draw bytes** chain from the label `pcoc-pack-2.2/draw`: `b_0` = SHA-256(ASCII label), `b_(n+1)` = SHA-256(`b_n`), concatenated. Each draw reads it 8 bytes at a time as §14.6 states.
    - Hike leg counts are 6, 18 and 54. For each, give:
      - the §14.6 six-leg draw (6 indices);
      - the §15.5 thorough-round draw at `thorough_rounds` = 3 (`D` = 18 indices, in draw order; at 6 legs §15.5 repeats an index only after every index has been used).
    - **Each of the six draws reads the byte source from `b_0`.** None continues where another stopped (SPEC §16.1 **Thorough rounds**).
    - `thorough_rounds` = 3 is fixture metadata for this vector, not a policy value and not read from any policy file (SPEC §16.1 **Thorough rounds**).
12. **`unknown-kind`**: the Rule A5 record bytes (hex) for these labels, numbered `<n>` in this order:
    - 0 `v2.1-pcoc-object`
    - 1 `satoshi-hike-v2.0-object`
    - 2 the empty label
    - 3 `v2.2-pcoc-objecT`

    The bytes are the record as SPEC Rule A5 defines it: no insignificant whitespace, keys `kind`, `kind_text` in that order, `kind_text` spelled as RFC 8785 spells a JSON string. They **do not include** the CLI's trailing newline. Also give the label itself, as hex of its UTF-8 bytes. Each input is the S1 object O1 with only its `kind` value replaced by the label (one well-formed CBOR map, so SPEC §7.3 sorts it as Rule A5).
13. **`hashcash`**: §13.1, with these exact inputs:
    - `bari_origin` = ASCII `https://example.com`;
    - fee header (80 bytes) = SHA-256(ASCII `pcoc-pack-2.2/hashcash/fee_header/0`) ‖ SHA-256(ASCII `pcoc-pack-2.2/hashcash/fee_header/1`) ‖ the first 16 bytes of SHA-256(ASCII `pcoc-pack-2.2/hashcash/fee_header/2`). It is not a valid Bitcoin header; it is only hashed.
    - `header_hash` = SHA-256(SHA-256(fee header)), natural order;
    - `waypoint` = SHA-256(ASCII `pcoc-pack-2.2/hashcash/waypoint`);
    - difficulty 21 bits.

    Give the inputs, `header_hash`, the challenge, the first `nonce` counting up from 0 whose digest has at least 21 leading zero bits, that digest, and its leading zero bit count.
14. **`negatives`**: S1, `pk_A`, `seed_beacon`, the first leg of stage 0 (`f3_shared_terrain` also uses `pk_B`).
    - Compute each negative exactly as SPEC §16 **Permanent negatives** defines it. The summaries below follow that text; the SPEC wins.
    - Also give the honest leg and the honest terrain digest.
    - `matches_honest` is `true` or `false`, computed by comparing with the honest value. Never write a literal. The generator must stop with an error if any negative equals honest.
    - The leg negatives are `legs_le_hex` of the first leg of stage 0, starting from the seed (SPEC Rule A2.2):
      - `slot_summary_fold`, `xor_accumulator_fold`, `xor_w_fold`, `s_plus_w_fold`: the four folds of SPEC §11.1.
      - `partial_slot_read`: the §11.1 fold over word 0 only of the slot each step reads.
      - `partial_slot_read_2047`: the §11.1 fold over words 0 … `WORDS_PER_SLOT − 2` (all but the last word) of the slot each step reads. That is 7 of 8 words at S1, and 2047 of 2048 at section 6.
      - `f3_shared_terrain`: `pk_A`'s first leg of stage 0, from `pk_A`'s own start (the seed), run on `pk_B`'s S1 terrain (same `seed_beacon`). This is a leg value, not a terrain digest. The `f3` is not §19 row F3; it guards row F4.
      - `prefold_hoist`: compute once per slot `pre(i) = (Σ_w mix64(word(w) XOR w)) mod 2^64`. On each step that reads slot `i`, use `S_folded = (pre(i) + S) mod 2^64` in place of the §11.1 accumulator, then the §11.1 seal.
      - `final_stage_hoist`: `S_folded = x XOR (x >> 31)`, where `x` is the XOR over every word of the slot of `mix64(word(w) XOR S)` without its last line (the value `y` just before `return y XOR (y >> 31)` in §10.1).
      - `cached_leg`: the first leg of stage 0 in which a step that reads a slot already read earlier in the same leg reuses the `S_folded` of that earlier read, instead of folding the slot with the current `S`. (At S1 `pk_A` the leg does revisit a slot; the generator must check that it does.)
    - The terrain negatives are the SHA-256 of the whole terrain array:
      - `one_lane_flip`: the §10.4 fill with the one-lane map of §10.4 in place of the NL4 base word. Parents and `slot_mix` are unchanged.
      - `all4_kernel`: the §10.4 fill with the linear map of §10.4 in place of the NL4 base word. Parents and `slot_mix` are unchanged.
      - `squeezed_seed_terrain`: the §10.4 fill with each of the four lanes replaced by the one value `P0 XOR P1 XOR P2 XOR P3`.

    **Negatives live only in the generator, in a module named `negatives`, never on an SDK code path.** They are permitted nowhere else.

## Exact keys
`<g>` = 0 … 17. `<j>` in kat-small is 0. In test-3leg it is 0 … 2.

| File | Keys |
|---|---|
| seed.txt | `seed.<pk>.preimage`, `seed.<pk>.D`, `seed.<pk>.P0_u64` … `seed.<pk>.P3_u64`; `<pk>` ∈ {`pk_A`, `pk_B`} |
| nl4.txt | `nl4.S1.slot.<i>.word.<j>.base_u64`, `nl4.S1.slot.<i>.word.<j>.stored_u64`; `<i>` ∈ {0, 1, 1023}, `<j>` = 0 … 7 |
| terrain.txt | `terrain.S1.terrain_sha256`, `terrain.S2.terrain_sha256`, `terrain.PROD.terrain_sha256` |
| fold.txt | `fold.S1.step.<n>.S_u64`, `fold.S1.step.<n>.slot_index`, `fold.S1.step.<n>.acc_u64`, `fold.S1.step.<n>.S_next_u64` for `<n>` = 0 … 3; `fold.S1.leg_result_u64` |
| kat-small.txt | `kat-small.S1.terrain_sha256`; `kat-small.S1.stage.<g>.waypoint`; `kat-small.S1.stage.<g>.leg.<j>.legs_le_hex`; `kat-small.S1.stage.<g>.leg.<j>.checkpoint` |
| test-3leg.txt | the same templates with `test-3leg.S2.` |
| prod.txt | `prod.PROD.terrain_sha256`, `prod.PROD.stage.0.leg.0.legs_le_hex`, `prod.PROD.stage.0.leg.0.checkpoint` |
| wes.txt | `wes.S1.D`, `wes.S1.stage.0.waypoint`, `wes.S1.equal` |
| rate.txt | `rate.legs.<L>.span.<s>.floor.<p>_<q>` for every grid point (45 keys), e.g. `rate.legs.6.span.1.floor.7_20` |
| anchor.txt | `anchor.H.100.height.<h>` for `<h>` = 100 … 106 |
| draw.txt | `draw.b0_label` (= `pcoc-pack-2.2/draw`), `draw.thorough_rounds` (= `3`), `draw.legs.<L>.six_leg.indices`, `draw.legs.<L>.thorough.indices` for `<L>` ∈ {6, 18, 54} |
| unknown-kind.txt | `unknown-kind.<n>.kind_text_hex`, `unknown-kind.<n>.record_hex` for `<n>` = 0 … 3 |
| hashcash.txt | `hashcash.bari_origin` (ASCII text), `hashcash.fee_header`, `hashcash.header_hash`, `hashcash.waypoint`, `hashcash.difficulty_bits`, `hashcash.challenge`, `hashcash.nonce`, `hashcash.digest`, `hashcash.leading_zero_bits` |
| negatives.txt | `negatives.honest.legs_le_hex`, `negatives.honest.terrain_sha256`; for each leg negative `<name>` ∈ {`slot_summary_fold`, `xor_accumulator_fold`, `xor_w_fold`, `s_plus_w_fold`, `prefold_hoist`, `final_stage_hoist`, `partial_slot_read`, `partial_slot_read_2047`, `f3_shared_terrain`, `cached_leg`}: `negatives.<name>.legs_le_hex` and `negatives.<name>.matches_honest`; for each terrain negative `<name>` ∈ {`one_lane_flip`, `all4_kernel`, `squeezed_seed_terrain`}: `negatives.<name>.terrain_sha256` and `negatives.<name>.matches_honest`. 13 negatives, 28 keys in all |

## Objects (for cross-verify, beside the candidates)
- `objects/O1-S1.cbor`: S1, `pk_A`, `legs_per_stage` 18 × 1.
- `objects/O2-S2.cbor`: S2, `pk_A`, 18 × 3.
- `objects/O3-PROD.cbor`: PROD, `pk_A`, 18 × 1.

Each object uses the fixed beacons and heights above and §7.3 deterministic CBOR. With the heights fixed, every core's O1–O3 must be byte-identical; their SHA-256 values are compared.

The synthetic test-plane index serves `GET /index/<h>` for h = 989 … 1018 (the seed height minus 11, through `last_interrupt_height`). Each document is compact JSON, `{"network":"synthetic","header":{"hash":…,"height":h,"time":…}}`, where:
- `hash` is the display-order hex (byte-reversed) of the natural 32 bytes at that height;
- the natural bytes are the beacon stored at that height, or, at any other height, SHA-256(ASCII `pcoc-pack-2.2/index/filler/` ‖ decimal h);
- `time` = 1700000000 + 600 · h.

This rule is kept from round 1 so its index files stay valid. Each verifier's test harness serves this index, so header checks are measured rather than `not read`.

## Not in the cores (later phases)
- The Bari paid rule (current, before, after, outside, wrong origin, display-order mismatch, live- and test-plane `nBits`) needs headers with real or synthetic PoW. It belongs to P2/P6.
- Sign-in vectors need signatures and go to P7.
- Non-empty paths, plant authentication, and the shape/decode edge objects that exercise erratum 1 (C-F12): the SPEC §7.3 decode order (empty, not-CBOR, truncated, trailing-byte, non-map, and malformed-map-with-foreign-kind inputs), a stage-0 `path` key, a 31-byte `last_interrupt` and a 31-byte checkpoint (§14 shape table; `shape_item` reports every violated condition, as SPEC §14 requires: a condition is not dropped because another is also violated, and the only exclusions are the ones the shape table already states), a `2^60` height (§14.1), two `present` paths of which one does not validate (Rule A4.11 step 1), and a header-less `P_b` document (`index_comparison_P_b`). They come in a later round, with exact constructions written when it is scoped.
- The cores list these as pending, not as missing by mistake.
