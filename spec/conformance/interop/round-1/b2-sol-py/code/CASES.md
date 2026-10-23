# Shared candidate-vector fixtures for the SPEC 2.2 cores (reviewer, 22 Oct 2026)

Every core (Rust, C, Go, Java) is written independently from `spec/satoshi-hike-v2.2/SPEC.md`. Each one emits the **same cases with the same inputs** in the **same text format**, so the pack job can compare them byte for byte. SPEC §16.1 says QA picks the profile integers for published vectors; these are those choices. They are fixture inputs, not SPEC parameters.

## Output format (identical in every language)
- Directory `sdk/<lang>/candidates/`, one file per case group: `seed.txt`, `nl4.txt`, `terrain.txt`, `fold.txt`, `kat-small.txt`, `test-3leg.txt`, `prod.txt`, `wes.txt`, `rate.txt`, `anchor.txt`, `draw.txt`, `unknown-kind.txt`, `hashcash.txt`, `negatives.txt`.
- Each line is `key<TAB>value<LF>`. Use ASCII only, no blank lines, no comments, no trailing whitespace, and sort lines by key (byte order).
- Value spellings:
  - bytes: lowercase hex, no `0x`;
  - `u64`: `_u64` suffix on the key, value as 16 lowercase hex digits of the integer (big-endian spelling of the number, *not* the wire bytes);
  - wire little-endian bytes: lowercase hex of the bytes, e.g. `legs_le_hex`;
  - other integers: decimal;
  - booleans and outcomes: lowercase words.
- Key names: `<case>.<fixture>.<…>.<field>`. Fields use the SPEC's own names (`D`, `P0`…`P3`, `checkpoint`, `waypoint`, `legs_le_hex`, `S`, `acc`, …). Stage and leg indices are decimal and 0-based, as the SPEC numbers them. **If the SPEC names a value differently from this file, use the SPEC name and note it in DONE.md.**
- The generator is a command in the SDK, e.g. `cargo run --bin gen-candidates`, `make candidates`, `go run ./cmd/gencandidates`, or a Java main. It writes these files deterministically, and a test re-runs it and checks the committed files are unchanged.

## Fixed inputs
- `pk_A` = `f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9` (BIP-340 test vector 0 public key; public data).
- `pk_B` = `dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659` (BIP-340 test vector 1 public key; public data).
- Synthetic beacons (test-plane fixtures, 32 bytes each):
  - `seed_beacon` = SHA-256(ASCII `pcoc-pack-2.2/beacon/seed`);
  - `I_k` = SHA-256(ASCII `pcoc-pack-2.2/beacon/` ‖ decimal k), for k = 0 … 17.
- Any other fixed byte string the SPEC needs and this file doesn't name (header bytes, Bari origin, nonce start, and so on): derive it as SHA-256(ASCII `pcoc-pack-2.2/<name>`), with the name chosen by you. Put the name in the key, e.g. `hashcash.origin_label`, so the other cores can see it. A mismatch caused only by a different chosen name is reported as a fixture mismatch, not a disagreement.
- Path bytes on stages 1–17: the empty path bytes of §13.3 (`path_skip`) for every stage.
- Profiles (each meets the §16.1 constraints):

| Fixture | `SLOT_BYTES` | `SAMPLE_BYTES` | `TERRAIN_BYTES` | `STEPS_PER_LEG` | `legs_per_stage` |
|---|---|---|---|---|---|
| `S1` (`kat-small`) | 64 | 32 | 65536 | 100 | 18 × `1` |
| `S2` (`test-3leg`) | 256 | 64 | 1048576 | 1000 | 18 × `3` |
| `PROD` | section 6 | section 6 | section 6 | section 6 | first leg only |

## Cases (SPEC §16 construction list)
1. `seed`: for `pk_A` and `pk_B`, each with `seed_beacon`: the `preimage` (64 bytes), `D`, and lanes `P0_u64` … `P3_u64`.
2. `nl4`: profile S1, `pk_A`. The NL4 **base** words `j` = 0 … 15 of slots 0, 1 and `N_SLOTS − 1`, plus the final stored terrain words for the same indices (after parent mixing and `slot_mix`).
3. `terrain`: SHA-256 of the whole terrain for S1, S2 and PROD (`pk_A`).
4. `fold`: S1, `pk_A`, the first leg of stage 0. A per-step trace for steps 0 … 3: the input state, the slot index read, the §11.1 accumulator after the full slot, and the output state, plus the final `leg_result`.
5. `kat-small`: S1, `pk_A`, the full 18-stage hike at `legs_per_stage`. For every leg: `legs_le_hex`. For every stage: its checkpoint(s) and waypoint, exactly where the SPEC defines them (§11.4, §12).
6. `test-3leg`: as `kat-small`, at S2.
7. `prod`: PROD, `pk_A`. The terrain digest, and the first leg of stage 0 at section 6 constants: `legs_le_hex` and its checkpoint.
8. `wes`: `waypoint_equals_seed` for S1 `pk_A`, as the stage-0 waypoint and `D` side by side, and `equal`.
9. `rate`: the integer leg-rate comparison of §14/§15 (the one the floor uses), over the grid:
   - `hike_legs` ∈ {0, 1, 5, 6, 1000};
   - `hike_span` (s) ∈ {1, 300, 3600};
   - floor `p/q` ∈ {1/5, 1/4, 7/20}.

   The value is the SPEC's outcome word.
10. `anchor`: the integer anchor-window comparison (offsets 2, 3, 4 after stored stage height `H`) for `H` = 100 and header heights 100 … 106. The value is the SPEC's outcome word.
11. `draw`: the §14.6 thorough-round / six-leg index draw. The injected byte source is the SHA-256 chain `b_0` = SHA-256(ASCII `pcoc-pack-2.2/draw`), `b_{n+1}` = SHA-256(`b_n`), concatenated. Hike leg counts are 6, 18 and 54; give the indices drawn.
12. `unknown-kind`: the Rule A5 record bytes (hex) for the labels `v2.1-pcoc-object`, `satoshi-hike-v2.0-object`, the empty label, and `v2.2-pcoc-objecT`.
13. `hashcash`: the §13.1 challenge bytes and digest for a stated origin and fee-header hash, and the first nonce from 0 that meets the 21-bit difficulty, with its digest.
14. `negatives`: every §16 permanent negative, computed by the shortcut it names, on S1 `pk_A` (terrain digest or `legs_le_hex` as fits), each with `matches_honest` = `false`:
    - `slot_summary_fold`, `xor_accumulator_fold`, `xor_w_fold`, `s_plus_w_fold`;
    - `prefold_hoist`, `partial_slot_read`, `partial_slot_read_2047` (at PROD `WORDS_PER_SLOT` semantics, as the SPEC defines it);
    - `one_lane_flip`, `all4_kernel`, `f3_shared_terrain`;
    - `cached_leg`, `squeezed_seed_terrain`.

    **Negatives live only in the generator, in a module named `negatives`, never on an SDK code path.**

## Not in the cores (later phases)
- The Bari paid rule (current/before/after/outside, wrong origin, display-order mismatch, live- and test-plane `nBits`) needs real or synthetic headers with PoW. It belongs to P2/P6.
- Sign-in vectors need signatures and go to P7.
- The cores list these as pending, not missing by mistake.
