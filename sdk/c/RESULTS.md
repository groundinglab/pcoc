# C SDK 2.2 — generator results

The generator writes the CASES v2 keys. Where this file and the public six-doc pack under `spec/` differ, the specification wins (`README.md`, `GUIDE.md`, and `01`–`06`). No deviation from CASES.md key templates was required.

## SPEC questions

1. **Invalid UTF-8 in a `kind` text** (§7.3, the sentence “Text values and map keys are definite-length UTF-8 text strings”; erratum 1 open point 4). §7.3’s malformed list does not name invalid UTF-8, and RFC 8785 cannot spell it. Reading implemented: a major-type-3 string that is not valid UTF-8 is not a text string under these rules, so the input is not one definite-length CBOR data item. The verifier records `malformed cbor` and the CLI exits 0. It does not apply Rule A5. Erratum 2 E01 settles this for every CBOR text string, map keys included; this reading is that rule.

2. **GET for an over-range stored height** (erratum 1 open point 1; §14.1 Rule A4.1 after E25; §14.2). E25 makes the height `not defined` and absent for the other shape conditions. Reading implemented: a height above `2^53 − 1` is not a usable height, so Rule A4.2 does not GET it. `index_comparison` and `T` are `not read` because there is no height to request, not because a read failed.

3. **Verify CLI and the portable index** (§9, §14.2, §18). §18 names `--profile live` as selecting the live index and does not name an index URL flag. This phase has no live header client (Phase C). Reading implemented: the library measures whatever documents `pcoc_index_get` returns; a NULL callback records `not read`. The CLI does not open a connection. `--profile live`, `--profile test`, and an omitted profile all measure with the section 6 constants. The CASES synthetic index is served by the test harness, not by the CLI.

4. **`rotl` shift mask** (§10.3). `r` in this section is in `0 … 63`. The implementation reduces a shift by 64 before rotating, which is the same function on that range, including `rotl(w, 0) = w`. The `<<` used for `i << 32` is a 64-bit left shift and is not reduced modulo 32.

## Deviations from CASES.md

None. The candidate files match the round-2 byte files, and `br1.txt` is the Rule BR1 vector the four SDKs agree on. Object hashes:

- `objects/O1-S1.cbor` `800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246`
- `objects/O2-S2.cbor` `6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0`
- `objects/O3-PROD.cbor` `b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7`

PROD leg 0 `legs_le_hex` is `88a04ac5beacb8a7`. `hashcash.header_hash` is SHA-256(SHA-256(fee header)).

## Pending, not missing

Non-empty paths and the later-round shape objects named in CASES.md “Not in the cores”. The Bari paid rule is in this SDK. Sign-in wire is Rust only. SIMD fold kernels and the dispatcher probe are Phase B. The live header source and Bari client are Phase C. The release gate path’s OS-CSPRNG draw has no byte-source parameter (`pcoc_six_leg_draw`, `pcoc_thorough_draw`); the byte-source variants are `pcoc_six_leg_draw_rig` and `pcoc_thorough_draw_rig`.
