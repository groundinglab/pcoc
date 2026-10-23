# Round 1 six-way candidate diff: Rust, C, Go, Java (high-effort cores) + B1, B2 (reviewer, 22 Oct 2026, ACDT)

**Inputs:** SPEC 2b50df7 and CASES.md v1, the same for all six builds.

| Build | Source (commit) | Model / provider class | Host for the timing |
|---|---|---|---|
| Rust | `sdk/v2.2-core-rust` e1ddcb48 | high-effort core | Reference system 1, Ryzen 5 5600H, SHA-NI |
| C | `sdk/v2.2-core-c` d7796b80 | high-effort core | Reference system 3, Ryzen 5 7430U, SHA-NI present but software SHA used |
| Go | `sdk/v2.2-core-go` c57069ab | high-effort core | Reference system 2, i7-10610U, no SHA-NI |
| Java | `sdk/v2.2-core-java` 20e03f82 | high-effort core | Reference system 2, i7-10610U, no SHA-NI |
| B1 |  a3ac71d | Sonnet, TS + WASM | B1's own cloud VM (CPU not recorded) |
| B2 | repo.bundle 14e8c50 | Sol, Python + numpy | (no PROD leg timing) |

- The candidate files were taken from the pushed branches; the Rust and C working trees on their hosts are clean at those heads.
- Each high-effort core build's own report (DONE.md) is not published here.

**Method:** the same as DIFF.md.
- Keys are normalised for spelling and layout only (`sixway.py`):
  - unpadded indices, synonyms for the same SPEC field (`S'`/`S_out`/`S_prime`/`S_next`, `i`/`slot`/`slot_index`, `digest`/`sha256`/`terrain_sha256`);
  - `legs_le_hex` versus `leg_result_u64`, byte-reversed;
  - per-index draw lines reassembled into lists.
- Values are compared byte for byte and are never normalised.
- Full data: `sixway.json`.

## Headline
1. **Every SPEC-defined construction value is byte-identical in all six builds: 266 of 266 keys.**
   - seed: 12 keys;
   - NL4 base and stored words, j = 0…7: 48;
   - terrain SHA-256 at S1, S2 and PROD: 3;
   - S1 fold trace and leg result: 17;
   - kat-small (S1): 54;
   - test-3leg (S2): 126;
   - PROD leg 0, its checkpoint and the PROD terrain: 3;
   - `waypoint_equals_seed`: 3.

   Three builds (Rust, Java, B1) also emitted NL4 base words for j = 8…15, and they agree on all 24. **No high-effort core disagrees with B1 or B2 on any seed, terrain, fold, leg, checkpoint or waypoint value. No shared low-effort misreading of the construction showed up, and no construction bug in any high-effort core.**
2. **The rest of the SPEC-defined values also agree in all six builds:**
   - the anchor window, 7/7;
   - the unknown-kind record bytes, 4/4, with no LF;
   - the 4 formula-defined negatives (`slot_summary_fold`, `xor_accumulator_fold`, `xor_w_fold`, `s_plus_w_fold`), 4/4;
   - the six-leg draw at 6 legs, 1/1.
3. **All 65 disagreeing keys trace to SPEC gaps or to CASES fixtures. None is a construction defect.**
   - `rate`, 45 keys: the decisions agree 45/45, but the pass word has four spellings and Java's fail word differs. SPEC gap, L1973 (C-S2).
   - `draw`, 2 keys:
     - C read one byte stream on across the 6 → 18 → 54 draws, where the other five restarted at `b_0`.
     - The reviewer's `ref/draw_ref.py` reproduces both readings exactly, so C's §14.6 draw is correct under its own reading.
     - CASES did not say which (C-F7). The SPEC doesn't say either (C-S3).
   - `hashcash`, 10 keys: six different fixture input sets. The four high-effort core lines each recompute correctly under §13.1 from their own stated inputs (a check script, `hc_check6.py`: challenge, digest, and first nonce from 0). B1 and B2 were checked in DIFF.md. CASES fault (C-F3).
   - `negatives`, 8 keys: 7 of the 8 named-only negatives split (`f3_shared_terrain` is counted under value types, below). See the root causes. SPEC gap, L2067 (C-S1).
4. **Missing keys come from CASES only.**
   - NL4 j = 8…15.
   - The thorough draw: only B1 emitted it.
   - Hashcash field sets.
   - Rust's extra PROD `partial_slot_read_2047`; Rust has no S1 `partial_slot_read_2047` key.

## Per-file counts (canonical keys)
| File | Canonical keys | Present in all 6 | Identical in all 6 | Keys with any disagreement | Keys missing in ≥1 build |
|---|---|---|---|---|---|
| seed | 12 | 12 | 12 | 0 | 0 |
| nl4 | 72 | 48 | 48 | 0 | 24 |
| terrain | 3 | 3 | 3 | 0 | 0 |
| fold | 18 | 17 | 17 | 0 | 1 |
| kat-small | 54 | 54 | 54 | 0 | 0 |
| test-3leg | 126 | 126 | 126 | 0 | 0 |
| prod | 3 | 3 | 3 | 0 | 0 |
| wes | 3 | 3 | 3 | 0 | 0 |
| rate | 45 | 45 | 0 | 45 | 0 |
| anchor | 7 | 7 | 7 | 0 | 0 |
| draw | 8 | 3 | 1 | 2 | 5 |
| unknown-kind | 4 | 4 | 4 | 0 | 0 |
| hashcash | 17 | 3 | 0 | 10 | 14 |
| negatives | 26 | 22 | 15 | 8 | 4 |
| **total** | 398 | 350 | 293 | 65 | |

## Disagreements by root cause and SPEC line

### R1. Rule B2 has no pass word: SPEC L1973, glossary L323 (45 keys)
| Build | Pass | Fail | Why (DONE.md) |
|---|---|---|---|
| Rust | `meets the floor` | `refuse` | the verb in L1973 (Q2) |
| C | `meet` | `refuse` | (Q2) |
| Go | `meet` | `refuse` | (Q3) |
| Java | `match` | `mismatch` | "§14 token list does not include `refuse`" (Q1); uses `refuse` only for span ≤ 0 |
| B1 | `meets` | `refuse` | B1-11 |
| B2 | `meet` | `refuse` | |

Every build gives the same 17 passes and 28 fails. Java's `mismatch` for a numeric shortfall contradicts L1973 ("A numeric rate that falls short of the floor is `refuse`"). That is a Java misreading, and it adds weight to C-S2.

### R2. Six-leg draw byte source: restart or continue (CASES case 11; SPEC L2065, L2096–2098; 2 keys)
- C (`gen.c` L493–511) reads one chain for 6, then 18, then 54 (`off += used`). The other five restart at `b_0` for each leg total.
- `ref/draw_ref.py` gives:
  - restart: 6 → `1,2,3,4,0,5`; 18 → `7,2,1,15,10,16`; 54 → `7,20,1,51,10,16`
  - continue: 18 → `17,2,11,7,13,3`; 54 → `31,39,32,40,30,29`. These are exactly C's values.
- **The thorough draw: none of the four high-effort cores emitted it.** Rust (Q4) cites L2096/L2098 "The draw count stays 6" as overriding L2065's "thorough-round index draw". Java (Q16) and C/Go treat `thorough_rounds` as admission-only. B1 is still the only build with thorough indices.

### R3. Hashcash fixture inputs (CASES case 13 and L23; SPEC §13.1 L1051–1055 is clear; 10 keys)
| Build | Origin | `header_hash` | Waypoint | Nonce |
|---|---|---|---|---|
| Rust | `https://bari.example` | SHA-256(`pcoc-pack-2.2/hashcash/header_hash`), used directly | SHA-256(`…/hashcash/waypoint`) | 1073395 |
| C | `https://bari.example` | SHA-256(`pcoc-pack-2.2/hashcash.fee-header`), used directly | SHA-256(`…/hashcash.waypoint`) | 65820 |
| Go | `https://bari.example` | SHA-256(SHA-256(80-byte header from `…hashcash.fee_header`, `/2`, `/3`[:16])) | SHA-256(`…/hashcash.waypoint`) | 414931 |
| Java | `https://bari.example` | SHA-256(`pcoc-pack-2.2/hashcash/fee-header`), used directly | SHA-256(`…/hashcash/waypoint`) | 3082081 |
| B1 | `https://4a0e…8835.invalid` | SHA-256(SHA-256(80-byte header)) | SHA-256(`…/hashcash/waypoint`) | 1548251 |
| B2 | `https://example.com` | (own) | (own) | 4447969 |

- All six are correct under §13.1 for their own inputs.
- Three high-effort cores (Rust, C, Java) never formed an 80-byte fee header, so their `header_hash` is not `SHA-256(SHA-256(header))` of any stated header. CASES v1 asked for a "fee-header hash", which invited this. L2065 says the pack states a fee header. CASES-v2 (C-F3) fixes the 80 bytes.

### R4. The permanent negatives named but not defined (SPEC L2067, F2 L2167, F4 L2169; 8 value keys)
In the table below, **bold** marks agreement with the C-S1 proposal as written. †: C-S18 revises `prefold_hoist` to the per-slot-summary reading.

| Negative | Rust | C | Go | Java | B1 | B2 | C-S1 proposal |
|---|---|---|---|---|---|---|---|
| prefold_hoist | `a652…65fc`: Σ mix64(word⊕w) per slot, + S | `72fd…59ec`: Σ, ⊕ S | `b180…a7d1`: L822 final-stage hoist | `251d…c738`: Σ + mix64(S) | `b180…a7d1`: L822 hoist | `73a4…` = xor_w_fold | terrain `64a3…cc00` (no build) † |
| partial_slot_read | `7387…24ab`: one word short | `059f…9f54`: half | **`e1ae…d7c5`: word 0** | **`e1ae…d7c5`** | `059f…`: half | **`e1ae…`** | word 0 only |
| partial_slot_read_2047 (S1) | — (PROD only: `92ff831c0f516586`, 2047 of 2048 words) | **`7387…24ab`** | **`7387…`** | **`7387…`** | **`7387…`** | `4c9a…` = honest | WPS−1 words |
| one_lane_flip | **`5c3e…0608`** | **`5c3e…`** | `282c…bd91`: lane 0 complemented, one-lane map, no parents | **`5c3e…`** | **`5c3e…`** | placeholder | one-lane map + parents + slot_mix |
| all4_kernel | **`2d74…cbb6e`** | **`2d74…`** | `f9f9…25b5`: linear map, no parents | **`2d74…`** | **`2d74…`** | placeholder | linear map + parents + slot_mix |
| f3_shared_terrain | `9dcd…078c` = honest pk_B terrain digest | `366e…d97f` = honest pk_A terrain digest | `366e…` = honest pk_A terrain digest (vs a second beacon) | `9dcd…` = honest pk_B digest | **`a730…3cd1`: pk_A leg on pk_B terrain** | placeholder | pk_A leg on pk_B terrain |
| cached_leg | `1cf2…49a3` = S_0 (no steps) | `4d9a…bd31` = honest leg with the low bit flipped | `0265…5fbe` = one step only | `1cf2…` = S_0 | **`3b71…44d9`: per-slot `acc` memo** | `4c9a…` = honest | per-slot `acc` memo |
| squeezed_seed_terrain | `e343…6bd5`: all lanes P0 | **`ed0f…5c22`: P0⊕P1⊕P2⊕P3** | `7948…`: P0, other lanes 0 | `e343…`: all lanes P0 | **`ed0f…`** | placeholder | XOR of the four lanes |

The reviewer reproduced the following from SPEC text (`ref/neg_ref.py` and its variants):
- Rust's, C's and Java's prefold readings: `sum+S`, `sum^S` and `sum+mix(S)`.
- Rust/Java's squeezed value (all P0).
- pk_B's terrain digest `9dcd…078c`.
- That Rust/Java `cached_leg` = `le64(S_0)`.

Rust's PROD `partial_slot_read_2047`: see the note at the end.

**Findings**
- **Five different `prefold_hoist` values from six builds.**
- **Three high-effort cores (Rust, C, Java) independently read `prefold_hoist` as a per-slot summary Σ mix64(word(w) XOR w), combined with S afterwards.** That is the F2 "computed once per slot and reused on every visit" shortcut, which L830 forbids by name ("a per-slot summary"). They then combined it with S in three different ways. → C-S18 revises C-S1 to that reading.
- **Four of six publish an honest terrain digest as `f3_shared_terrain`** (C/Go pk_A's own, Rust/Java pk_B's), with `matches_honest` `false`. A vector equal to another key's honest terrain is not a shortcut artifact. For C and Go it is the very honest S1 terrain digest of the same `terrain.txt`. This confirms that C-S1 needs both "differs from the honest value of the same vector" and the leg form.
- **`cached_leg` has four readings.** Two of them (C's flipped honest leg, B2's honest leg) are not constructions at all.
- **`partial_slot_read`** agrees with the proposal in 3 of 6 builds. **`partial_slot_read_2047`** as WPS−1 agrees in 4 of 5 builds that emitted it at S1.
- **`one_lane_flip` and `all4_kernel`** agree with the proposal in 4 of 6 builds (Go drops the parents).
- **`squeezed_seed_terrain`** splits three ways (2 / 2 / 1, plus B2's placeholder).

### R5. Missing keys
- **NL4 j = 8…15:** emitted by Rust, Java and B1 (base words only), and agreeing. C, Go and B2 stopped at j = 7. CASES fault (C-F2).
- **The thorough draw:** B1 only (R2).
- **Hashcash:** each build published a different field set (R3).
- **`negatives.PROD.partial_slot_read_2047`:** Rust only.
- **The fold `legs_le_hex` duplicate:** Go and Java emitted both spellings, which agree.


**Rust's PROD `partial_slot_read_2047`:** `92ff831c0f516586`.
- The reviewer reproduced it independently with `ref/prod2047_ref.py`, which uses B2's numpy terrain over words 0…2046: `92ff831c0f516586`.
- The same script gives the honest PROD leg `88a04ac5beacb8a7` and terrain `60b1…fc04`, matching all six builds.

## Production-leg timings (one PROD leg = 2,000,000 steps × 2048 words; Christian's rule: any speed-up is suspect)
| Build | Host / CPU | SHA-256 path | Terrain fill | One PROD leg | ≈ cycles per word at max boost* |
|---|---|---|---|---|---|
| Java | Reference system 2, i7-10610U (4C/8T, max 4.9 GHz) | JDK `MessageDigest`, software (no SHA-NI on the host) | 0.63 s | **5.20 s** (5.23, 5.45 on reruns) | ≈ 6.2 |
| Rust | Reference system 1, Ryzen 5 5600H (max 4.2 GHz) | `sha2` with SHA-NI | 0.60 s | 5.99 s | ≈ 6.1 |
| C | Reference system 3, Ryzen 5 7430U (max 4.3 GHz) | software FIPS (SHA-NI present but not used) | 0.57 s | 6.91 s | ≈ 7.2 |
| B1 | B1 cloud VM (CPU not recorded) | Node OpenSSL | — | 8.3 s (its own report) | n/a |
| Go | Reference system 2, i7-10610U (max 4.9 GHz) | `crypto/sha256`, software | 0.77 s | 9.02 s | ≈ 10.8 |
| B2 | box | numpy | — | not measured (O3 verify 267 s) | n/a |

\*Seconds × max boost clock ÷ 4.096·10⁹ words. This is an upper bound on cycles per word: it ignores the 2·10⁶ seals and any clock below max boost.

**Reading.**
- Every build's PROD leg is byte-identical (`88a04ac5beacb8a7`), so no build skipped or changed work in a way that changes bytes.
- An output-preserving work reduction would need a cache hit on the same `(slot, S)` pair, which the seal makes practically impossible.
- A shortcut that is caught by the bytes therefore cannot be behind any of these times. The remaining risk is a byte-preserving optimisation, which the I6 rule still flags.

**Flags**
1. **Java is the fastest of the six on the slowest host.**
   - It has no SHA-NI and shared the host with the Go job (Java DONE: "The Go core was allowed to run on this host at the same time").
   - Per clock it matches Rust on a SHA-NI host.
   - I read `Step.java`, `Leg.java`, `Terrain.java`, `Mix.java` and `Sha256.java` on the pushed branch:
     - `foldSlot` loops `w = 0 … wordsPerSlot−1` over `long[] terrain` with `acc += mix64(word ^ s)`;
     - `mix64` has `MIX_ADD` and both multiplies;
     - the seal is `MessageDigest` over 20 bytes;
     - the terrain array is the §10.4 words, the same array the digest and the window read;
     - there is no cache, table or summary.
   - **Flag: plausible JIT/ILP speed (the 2 multiplies per word bound it near 2–4 cycles), not a shortcut. It still needs an isolated re-time on reference system 2 with Go idle, and adversarial review before reuse.**
2. **Rust 5.99 s** is a straight §11.1 loop with SHA-NI as L836 requires. It is the reference speed on the L834 baseline CPU. Not a suspect.
3. **C 6.91 s** used software SHA on a SHA-NI host. That is **non-conforming under SPEC L836** ("A software SHA-256 is permitted only where the host has no such instructions"), and C says so itself. It makes C slower, not faster, so it is not a shortcut but a high-effort C-core defect to fix.
4. **B1 8.3 s:** the WASM kernel was already flagged in I6.md as a speed-up but not a shortcut.
5. **Go 9.02 s:** a straight loop and the slowest. It shared reference system 2 with Java.

None of the timings indicates a producer shortcut. Under the rule, **Java and B1 stay flagged as speed-ups pending review. Merge nothing on speed grounds.**

## Appendix A. Every disagreeing key (values per build; `—` = key not emitted)

### rate.txt (45 keys)
All 45 keys disagree on spelling only. Decision (pass/fail) agrees on 45/45. Spellings: Rust `meets the floor`/`refuse`; C, Go, B2 `meet`/`refuse`; Java `match`/`mismatch`; B1 `meets`/`refuse`. Pass side on 17 keys, fail side on 28 in every build.

### draw.txt (2 keys)
| key | rust | c | go | java | b1 | b2 |
|---|---|---|---|---|---|---|
| `draw.legs.18.six_leg` | `7,2,1,15,10,16` | `17,2,11,7,13,3` | `7,2,1,15,10,16` | `7,2,1,15,10,16` | `7,2,1,15,10,16` | `7,2,1,15,10,16` |
| `draw.legs.54.six_leg` | `7,20,1,51,10,16` | `31,39,32,40,30,29` | `7,20,1,51,10,16` | `7,20,1,51,10,16` | `7,20,1,51,10,16` | `7,20,1,51,10,16` |

### hashcash.txt (10 keys)
| key | rust | c | go | java | b1 | b2 |
|---|---|---|---|---|---|---|
| `hashcash.challenge` | `4284847f89790951f5eca6a6d3f7052bf986ecb9597383638bbb47adcb375353` | `6714d0dd1bde62a94584d8942c415aafe5e4e486b6531612c2c4c3a239269e97` | `d63e8345aadedd05091b1cece6c3349827e3440056d880e8ea980f6bcdceda02` | `b563ee85308ff1346f6bb4c6d7bfb7c7a97381d71cb8e83d4dac41456a111d25` | `d1d65714b910b3c8c4122434b2ddee01113bce6152b4b1a7bcf74aa07d4115fa` | `58e405b889824c9ff3444c6e11aea358cc5e216e6fce57d794cb1ef2d3bc4f37` |
| `hashcash.digest` | `000006c7527cda01397e27bfb2fcfc5abd2c456eadfdfdf26831047c7074c8a2` | `0000020a1a86d896416fcad8d943a88d3f3bf32d4c8728135f13553954c2d3c4` | `0000027ac15a7fcb6354f3579a3926e4c85916a5848a651edda9454c5c1760e6` | `0000025566b3a98c831ceb648117b24500ce5fe7bffb41f831af0cd6eb299528` | `0000038b74f5f1fd6994ac122b33d322e10a39810660400272679808ce553725` | `000002afa0802db0c0dc9a5b1879f9c0311efd883c6d25a686136db6831f3411` |
| `hashcash.digest_nonce0` | — | `583a276149cc3166e29017505ce80ce7ee154dd557801bb5434c150993174df6` | `2555ac625fb7acd64321d72e39017cd92b9cdda7b83a53675527dd88bcda8b22` | — | — | — |
| `hashcash.fee_header_label` | — | `pcoc-pack-2.2/hashcash.fee-header` | `pcoc-pack-2.2/hashcash.fee_header` | — | — | — |
| `hashcash.header_hash` | `2a607ce2d0675d6462344a407bd901b9b24fbf501e4e6ee03125b7efc2e645d8` | — | — | `cb39c6f7e332dc799aa1d15ba4d92432239f2836278c138f75d58a2c0ff015e6` | `9e48d971c25a2c21c95449623e42b620c9037038afb020d12f4126309e849d54` | `287c28c72e4a7d23d88e82afbf601cd8740345018abc6ae9a78eec996c44f2a7` |
| `hashcash.nonce` | `1073395` | `65820` | `414931` | `3082081` | `1548251` | `4447969` |
| `hashcash.origin` | `https://bari.example` | — | — | `https://bari.example` | `https://4a0e0526df729135d43fac93d9548835.invalid` | `https://example.com` |
| `hashcash.origin_label` | `https://bari.example` | `https://bari.example` | `https://bari.example` | — | `bari_origin` | `bari-origin` |
| `hashcash.waypoint` | `76c7a4a699009dd5c52b91ad9b8454282b8020ea86b208c1c57ea0311a7e2c28` | — | — | `76c7a4a699009dd5c52b91ad9b8454282b8020ea86b208c1c57ea0311a7e2c28` | `76c7a4a699009dd5c52b91ad9b8454282b8020ea86b208c1c57ea0311a7e2c28` | `6863fc06e217f9a102ee6f5be8761bc27425c0f0f4c6b722ca03aa4e5fdbd771` |
| `hashcash.waypoint_label` | — | `pcoc-pack-2.2/hashcash.waypoint` | `pcoc-pack-2.2/hashcash.waypoint` | — | `hashcash/waypoint` | — |

### negatives.txt (8 keys)
| key | rust | c | go | java | b1 | b2 |
|---|---|---|---|---|---|---|
| `negatives.all4_kernel.value` | `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` | `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` | `f9f9aa9a08688c0fe721e01a1502b542a56d9c7e03e37b43f8f9351c6de125b5` | `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` | `2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e` | `af0b0f2228b6505d58417d075653d61c6489ef9f23a5c5257d5d549d284926de` |
| `negatives.cached_leg.value` | `1cf2e2a5fc0849a3` | `4d9a6a98574cbd31` | `0265f616e37b5fbe` | `1cf2e2a5fc0849a3` | `3b715e2a251a44d9` | `4c9a6a98574cbd31` |
| `negatives.f3_shared_terrain.value` | `9dcd7e927f15553e1ddd757d5bfa902dc58f787910e2f62b5d4479da4606078c` | `366e6d1d27adb715d5c96c64e6b9d881e1701a0151e6e08d49df08d2996bd97f` | `366e6d1d27adb715d5c96c64e6b9d881e1701a0151e6e08d49df08d2996bd97f` | `9dcd7e927f15553e1ddd757d5bfa902dc58f787910e2f62b5d4479da4606078c` | `a73019b8a2c43cd1` | `3f5d4812c0ad1307d215633817f03cfab9a69bc6a8549e86e08c64846cf112fb` |
| `negatives.one_lane_flip.value` | `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` | `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` | `282cc1b65d4d89b15bfc3d7c5e073d08f521dc7c0400244589c08bd516c7bd91` | `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` | `5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608` | `a0325421bbc38a5ee9f494e957298684084086ccefb7821d3e8b9785923d2a45` |
| `negatives.partial_slot_read.value` | `738755dabac824ab` | `059f134920349f54` | `e1ae647ee5a5d7c5` | `e1ae647ee5a5d7c5` | `059f134920349f54` | `e1ae647ee5a5d7c5` |
| `negatives.partial_slot_read_2047.value` | — | `738755dabac824ab` | `738755dabac824ab` | `738755dabac824ab` | `738755dabac824ab` | `4c9a6a98574cbd31` |
| `negatives.prefold_hoist.value` | `a652562037b465fc` | `72fdcef7ceb359ec` | `b1808c8d3927a7d1` | `251da3dcd0d2c738` | `b1808c8d3927a7d1` | `73a4c7fbd105aae1` |
| `negatives.squeezed_seed_terrain.value` | `e343932c4f2705ce637acdd45a156a0fa7ece8a318323fb67e4034e6386d6bd5` | `ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22` | `794883d4ccc0bfe612bf830f48be3c3e38f01f9c158f4cc2c93dcf63e6dddf5c` | `e343932c4f2705ce637acdd45a156a0fa7ece8a318323fb67e4034e6386d6bd5` | `ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22` | `8f5162aa56660daf391411b219fe4f8141f43a4a58ac131f5ab762809244f04d` |

## Appendix B. Keys emitted by only some builds, all values agreeing

- **nl4** (24): `nl4.slot.0.word.10.base` (rust, java, b1); `nl4.slot.0.word.11.base` (rust, java, b1); `nl4.slot.0.word.12.base` (rust, java, b1); `nl4.slot.0.word.13.base` (rust, java, b1); `nl4.slot.0.word.14.base` (rust, java, b1); `nl4.slot.0.word.15.base` (rust, java, b1); `nl4.slot.0.word.8.base` (rust, java, b1); `nl4.slot.0.word.9.base` (rust, java, b1); `nl4.slot.1.word.10.base` (rust, java, b1); `nl4.slot.1.word.11.base` (rust, java, b1); `nl4.slot.1.word.12.base` (rust, java, b1); `nl4.slot.1.word.13.base` (rust, java, b1); `nl4.slot.1.word.14.base` (rust, java, b1); `nl4.slot.1.word.15.base` (rust, java, b1); `nl4.slot.1.word.8.base` (rust, java, b1); `nl4.slot.1.word.9.base` (rust, java, b1); `nl4.slot.1023.word.10.base` (rust, java, b1); `nl4.slot.1023.word.11.base` (rust, java, b1); `nl4.slot.1023.word.12.base` (rust, java, b1); `nl4.slot.1023.word.13.base` (rust, java, b1); `nl4.slot.1023.word.14.base` (rust, java, b1); `nl4.slot.1023.word.15.base` (rust, java, b1); `nl4.slot.1023.word.8.base` (rust, java, b1); `nl4.slot.1023.word.9.base` (rust, java, b1)
- **fold** (1): `fold.leg_result_u64#dup:fold.S1.pk_A.stage0.leg0.legs_le_hex` (go, java)
- **draw** (5): `draw.b0_label` (b1); `draw.legs.18.thorough` (b1); `draw.legs.54.thorough` (b1); `draw.legs.6.thorough` (b1); `draw.thorough_rounds` (b1)
- **hashcash** (7): `hashcash.bits` (c); `hashcash.difficulty` (rust, b1); `hashcash.fee_header` (b1); `hashcash.fee_header_hash_name` (java); `hashcash.fee_header_labels` (b1); `hashcash.header_hash_name` (rust); `hashcash.waypoint_name` (rust, java)
- **negatives** (3): `negatives.PROD.partial_slot_read_2047.matches_honest` (rust); `negatives.PROD.partial_slot_read_2047.value` (rust); `negatives.partial_slot_read_2047.matches_honest` (c, go, java, b1, b2)
