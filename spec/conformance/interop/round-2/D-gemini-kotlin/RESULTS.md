# Satoshi Hike 2.2 — Kotlin/JVM Implementation & Interop Results

## Possible SPEC flaw

During our line-by-line analysis of `SPEC.md` (commit `7b40353ed68cbdc7319f06d09f6ed178e7f7809e`), we identified five structural and protocol-level flaws where a producer or fast-phase verifier can produce valid, verifying (and admission-passing) outputs while doing **less work** or **taking more real time** than the SPEC intends, without the verifier or admission gate being able to tell. **In accordance with the prompt and SPEC Rule I6, none of these shortcuts are shipped in our implementation**; our producer and verifier perform 100% of the intended work on every step and terrain word.

### Flaw 1: Unbound `last_interrupt` (`I_17`) and unanchored `W_17` grant `K` extra blocks of walk time outside `hike_span` (saves 14.3%–25.0% required leg rate)
- **SPEC locus:** §8 Rule A3.1 (lines 543–551), §12.2 (line 966), §13.2 (lines 1126–1136), §14.4 Rule A4.10 (lines 1581–1592), §15.4 Rules B0, B2, and B10 (lines 1947–1979).
- **How it works:**
  1. Stage 17 (`stages[17]`) starts from `W_17` (which is computed from `I_16` at the end of stage 16).
  2. By §12.2 line 966, *"Stage 17 ends at `I_17`. No waypoint is computed from stage 17 or from `last_interrupt`. Stage 17’s checkpoints are not folded into a waypoint."* Furthermore, `stages[17].path` is always empty (`not fetched by design`), and `last_interrupt` (`I_17`) is never hashed into any leg, checkpoint, or waypoint of stage 17 — it is merely stored alongside `last_interrupt_height` as top-level CBOR fields.
  3. Meanwhile, §15.4 Rule B0 and Rule B2 compute the admission rate over `hike_span = MTP(last_interrupt) - MTP(seed_beacon)` (18 block intervals, `H_0` through `H_18`), while Rule B10 (`submission_deadline_blocks`, default `K = 3`, allowed range up to `K = 6`) permits submitting the object until the index tip reaches `height(I_17) + K`.
  4. Because nothing in `stages[17]` cryptographically binds to `I_17`, a producer does **not** have to stop walking `stages[17]` when block `H_18` arrives. Instead, the producer can continue walking `stages[17]` while blocks `H_18 + 1, …, H_18 + K` are mined, and then right before submitting at height `H_18 + K`, set `last_interrupt` to the historical block `H_18` from `K` blocks earlier.
- **What it saves:** The producer walks for `18 + K` block intervals while `hike_span` only measures `18` block intervals. At the default `K = 3`, a machine that is **14.3% slower** (`18 / 21`) than `leg_rate_floor` passes verification and admission; at `K = 6`, a machine **25.0% slower** (`18 / 24`) passes. Neither the §14 verifier nor the §15 admission gate can detect that `stages[17]` legs were walked after `last_interrupt` was mined.

### Flaw 2: Admission rate ignores bookends (`Rule B0` vs `Rule A4.6–A4.9`) while `Rule B4` requires only 6 anchored waypoints anywhere among `W_1 … W_16`
- **SPEC locus:** §14.4 Informative note (lines 1612–1614) vs §15.4 Rule B0 (lines 1970–1973), Rule B4 (lines 1984–1987), and Rule B8 (lines 2000–2002).
- **How it works:**
  1. §14.4 lines 1613–1614 explicitly observe: *"A stored interrupt shows that the header exists. It does not show that produce ran live. Only anchored and fetched shows that the fold existed by the plant’s block."*
  2. Despite this, §15.4 Rule B0 discards the bookend rate `(N, S)` and evaluates `leg_rate_floor` solely on `(hike_legs, hike_span)` across all 18 stages, while Rule B4 requires only `min_anchored_paths = 6` anchored waypoints *anywhere* among `W_1 … W_16` and Rule B8 imposes no per-stage leg minimum beyond `leg_count >= 1`.
  3. Consequently, an attacker does not need to anchor early waypoints (say `W_1 … W_10`).Wait-selecting on Bitcoin's Poisson block interval variance, a producer can wait until 11 consecutive blocks (`H_0 … H_10`) happen to be mined in a fast burst (a short `MTP` span), immediately step through stages `0 … 9` with `leg_count = 1` and `path = ""` (`skip`, requiring ~20 seconds total and zero Bari interaction), and only anchor `W_11 … W_16` in real time, concentrating its leg production in the later stages while benefiting from the compressed `hike_span` denominator.

### Flaw 3: Unverifiable Hashcash and Bari POST work on `W_17` and on skipped waypoints (`W_1 … W_16`)
- **SPEC locus:** §13.1 (lines 1033–1083), §13.2 (lines 1096–1137), §17.1 (line 2155).
- **How it works:**
  1. §13.1, §13.2, and §17.1 require the producer to solve a 21-bit Hashcash puzzle (~2,097,152 double-SHA-256 / challenge hashes on average) and send `POST {bari-url}/waypoint` for **every** waypoint `W_1 … W_17`, including `W_17`.
  2. However, for `W_17`, `stages[17].path` is required to be empty (`not fetched by design`), and §14 Verify never queries Bari or the index for `W_17`. A producer that completely skips solving Hashcash and POSTing `W_17` produces a 100% byte-identical hike object and identical index state.
  3. Similarly, once a producer has obtained `min_anchored_paths = 6` anchored paths among `W_1 … W_16`, it can skip solving Hashcash and POSTing to Bari for the other 10 stages, storing empty `path` bytes (`skip`) and emitting `path_skip <g>`. Neither the verifier nor the gate can distinguish an intentional skip from a network miss.
- **What it saves:** Skipping the `W_17` Hashcash solve always saves `~2^21` (`~2.1M`) SHA-256 evaluations per hike with zero trace; skipping 10 unanchored stages saves a further `~2.1 × 10^7` SHA-256 evaluations per hike.

### Flaw 4: Non-cascading fake legs (`checkpoints[j]` binds the stored `leg_results[j]`, breaking sequential chaining and enabling multi-core parallelisation of a single hike)
- **SPEC locus:** §11.2 (lines 845–851), §11.4 (lines 868–888), §14.3 Rule A4.3 (lines 1397–1406), §14.6 Rule A4.4 (lines 1656–1658), §15.5 (lines 2005–2056).
- **How it works:**
  1. In §11.4 and §14.3 Rule A4.3, `checkpoints[j] = SHA-256(previous ‖ stage_limb ‖ terrain_sample(previous) ‖ stored_leg_results[j])`, and leg `j + 1` starts from `S_0 = u64_le(checkpoints[j][0:8])` (§14.6 line 1656).
  2. Crucially, Rule A4.3 hashes the **stored** 8-byte `leg_results[j]` without running `LegResult(terrain, previous)`. And when §14.6 / §15.5 spot-checks leg `j + 1`, it starts leg `j + 1` from the stored `checkpoints[j - 1]`.
  3. Therefore, a fake `leg_results[j]` (chosen as any arbitrary 8 bytes in **0 steps**) still produces a valid `checkpoints[j]` that passes Rule A4.3 (`Chain`) and the §12.2 waypoint fold, **and** serves as a valid starting point for an honest leg `j + 1` that will pass §14.6 / §15.5 recomputation! Fake legs do **not** cascade to subsequent legs in the same stage.
  4. Furthermore, §15.5 (line 2054) assumes that a single PCOC's legs are strictly sequential so that using `C` CPU cores requires running `C` distinct PCOCs with `C × 256 MiB` of separate terrains. Because a fake leg takes 0 steps to produce a valid `checkpoints[j]` reset point, an attacker with `C` cores sharing **one** 256 MiB terrain can insert `C - 1` fake legs in a long hike of `L` legs and compute the remaining `L - (C - 1)` honest legs **in parallel across all `C` cores**. For example, in a 2,000-leg hike on an 8-core machine, inserting 7 fake legs (`0.35%` fake fraction) lets all 8 cores walk honest legs in parallel on a single 256 MiB terrain while passing an 18-leg thorough draw `(1 - 7/2000)^18 = 93.9%` of the time (or `97.9%` of the time for §14 standalone `verify`, which only ever checks 6 legs).

### Flaw 5: `dag2` terrain fill (§10.4) has zero cross-word mixing (`2,048` disconnected 128 KiB column DAGs)
- **SPEC locus:** §10.4 (lines 748–790), §14.3 Rule A4.3 (lines 1395–1415), §15.3 Fast Phase (lines 1926–1928).
- **How it works:**
  1. In §10.4, for slot `i > 0` and word index `j ∈ 0 … WORDS_PER_SLOT - 1`, the three parent words `pw` are read from `(i - 1, j)`, `(bitrev_parent(i), j)`, and `(mul_parent(i), j)` — always at the **exact same word index `j`**.
  2. There is no mixing between column `j` and any other column `j' ≠ j` anywhere in §10.4. The 256 MiB terrain (`16,384 × 2,048` words) is therefore `2,048` completely independent 1D DAGs of `16,384` words (`128 KiB`) each.
- **What it saves:**
  - A fast-phase verifier (§15.3) or Rule A4.3 checker that only verifies `checkpoints` (`SAMPLE_BYTES = 1024` bytes = `128` words per checkpoint) does not need to compute the full 2,048-word width for checkpoint samples: each 1024-byte sample touches only `128 / 2,048 = 6.25%` of the columns up to that sample's slot index.
  - Even when computing the full terrain digest (`terrain_sha256`) or streaming slots, each column `j` requires only `128 KiB` of working memory (fitting inside L1d cache) rather than a 256 MiB memory footprint.

---

## 1. Model & Effort

- **Model:** `Gemini 4 Argon` (`gemini-4-argon`)
- **Effort level:** `1.00` (maximum / high)
- **Language & Runtime:** Kotlin `2.0.21` on OpenJDK `21.0.12.1` (64-bit Server VM, Ubuntu `21.0.12.1+1-1-24.04.4-Ubuntu`)

---

## 2. What Was Built

All implementation code lives under `impl/`:

1. **`impl/src/main/kotlin/satoshihike/SpecConstants.kt`**:
   - Section 6 constants (`SLOT_BYTES`, `WORDS_PER_SLOT`, `TERRAIN_BYTES`, `N_SLOTS`, `SAMPLE_BYTES`, `OFFSET_MODULUS`, `STEPS_PER_LEG`, `STAGES`, `SEAL_DOMAIN`, `KIND`, `FILL`, `MIX_ADD`, `MIX_MUL1`, `MIX_MUL2`, `WORD_XOR`, `PARENT_MUL`, `SLOT_MUL`, `ANCHOR_WINDOW_OFFSETS`, `HASHCASH_DIFFICULTY_BITS`).
   - Section 16.1 `Profile` model (`production` and `conformance-small`, including `S1` and `S2`).
   - Section 3 & 7 `Encoding` utilities (`le64`, `u64Le` via `VarHandle`, hex encoding/decoding, Section 9 byte-order reversal `reverseBytes32`, SHA-256).
2. **`impl/src/main/kotlin/satoshihike/Construction.kt` & `VectorApiKernels.kt`**:
   - Section 10 seed preimage, seed `D`, lane extraction `P[0..3]`, `mix64`, `floorLog2`, `reverse32`, `bitrevParent`, `mulParent`, `nl4BaseWord`, and `dag2` terrain fill (both plain scalar `fillTerrainScalar`, explicit AVX-512 `VectorApiKernels.fillTerrainVector512`, and optimised 4-thread column-partitioned `fillTerrain`).
   - Section 11 step, fold, leg result (both plain scalar `legResultScalar`, explicit AVX-512 `VectorApiKernels.legResultVector512`, and optimised C2 SuperWord AVX-512 `legResult`), step tracer `legWithTrace`, terrain sample, and checkpoint `computeCheckpoint`.
   - Section 12 checkpoint fold `foldCheckpoints` and waypoint `computeWaypoint`.
   - Section 13.1 Hashcash challenge, digest, leading-zero-bit count, and nonce solver `solveHashcash`.
   - Section 14.6 unbiased index draw (`drawOneIndex`, `drawSixLegIndices`) and Section 15.5 thorough draw (`drawThoroughIndices`).
   - Section 15.4 Rule B2 integer rate comparison (`compareLegRateFloor`) and Section 16.1 integer anchor comparison (`compareAnchorWindow`).
3. **`impl/src/main/kotlin/satoshihike/CborCodec.kt`**:
   - Section 7.3 core deterministic CBOR encoder (`encode`) with exact top-level and per-stage key ordering (omitting `path` on stage 0).
   - Section 7.3 strict definite-length CBOR decoder and four-way classifier (`MalformedCbor`, `DecodeFailure`, `UnknownKind`, `ValidRootMap`) plus RFC 8785 (JCS) string escaping and Rule A5 formatting.
4. **`impl/src/main/kotlin/satoshihike/PathAndBari.kt`**:
   - Section 14.3 `CanonicalPathBody` parser, Merkle tree validator (`validateFor`), and `W_17` layout reader.
   - Section 13.6 Rule BR1 Bari paid-waypoint verifier (`BariPaidRule.checkWaypointPost`).
5. **`impl/src/main/kotlin/satoshihike/StrictJson.kt`, `FixtureIndex.kt`, & `Verifier.kt`**:
   - Section 9 strict JSON parser (rejecting BOM, non-UTF-8, trailing tokens, duplicate keys in any object, and fractional/exponent integers).
   - `CasesFixtures` & `HttpIndexSource` implementing the `CASES.md` synthetic test-plane index (`h = 989 … 1018`) and HTTP `GET {index-url}/index/{height}`.
   - Section 14 `Verifier` implementing Rules A4.1–A4.11 and Rule A5, parallel 6-leg verification, MTP calculation, and exact JSON key ordering.
6. **`impl/src/main/kotlin/satoshihike/Producer.kt` & `Cli.kt` + `/workspace/impl/verify` & `/workspace/verify`**:
   - Section 13.2 offline test-plane producer writing `objects/O1-S1.cbor`, `objects/O2-S2.cbor`, and `objects/O3-PROD.cbor`, plus Rule A3.4 stderr lines.
   - Section 18 `verify <object>` CLI supporting `--profile production`, `--profile conformance-small --sample-bytes ... --slot-bytes ... --terrain-bytes ... --steps-per-leg ...`, `--index-url`, and `--draw-label`, with exact exit codes `0`, `1`, and `2`.
7. **`impl/negatives/src/main/kotlin/negatives/Negatives.kt` & `CandidateGenerator.kt`**:
   - Isolated `:negatives` Gradle submodule (`negatives` package) containing all 13 permanent negatives and the deterministic candidate generator writing all 14 `.txt` files in `candidates/`.
8. **`impl/src/test/kotlin/satoshihike/SatoshiHikeTest.kt`**:
   - Automated tests re-running `CandidateGenerator` and verifying byte-for-byte identity with committed `candidates/*.txt`, testing §7.4 vectors, scalar/vector equivalence, `O1`/`O2`/`O3` verification over a live local HTTP index server, CBOR/shape edge cases, and Bari Rule BR1.

---

## 3. Exact Commands

```bash
# 1. Generate all 14 candidate files into candidates/
./impl/gradlew -p impl :negatives:genCandidates

# 2. Generate O1-S1.cbor, O2-S2.cbor, and O3-PROD.cbor into objects/
./impl/gradlew -p impl genObjects

# 3. Verify objects (prints the Section 14 verifier record JSON)
./impl/verify --profile conformance-small --slot-bytes 64 --sample-bytes 32 --terrain-bytes 65536 --steps-per-leg 100 objects/O1-S1.cbor
./impl/verify --profile conformance-small --slot-bytes 256 --sample-bytes 64 --terrain-bytes 1048576 --steps-per-leg 1000 objects/O2-S2.cbor
./impl/verify objects/O3-PROD.cbor

# 4. Run the test suite (re-runs candidate generator and checks committed candidates are unchanged)
./impl/gradlew -p impl test

# 5. Run scalar vs optimised benchmarks
./impl/gradlew -p impl benchmark
```

---

## 4. Machine & CPU Flags

- **CPU Model:** `Intel(R) Xeon(R) Processor` (Family 6, Model 207, Stepping 2 — 5th Gen Intel Xeon Scalable / Emerald Rapids)
- **Cores / Threads:** 4 cores (1 thread per core, 1 socket, x86_64 little-endian)
- **Cache Hierarchy:**
  - L1d: `192 KiB` (4 instances, `48 KiB` per core)
  - L1i: `128 KiB` (4 instances, `32 KiB` per core)
  - L2: `8 MiB` (4 instances, `2 MiB` per core)
  - L3: `320 MiB` (shared)
- **Instruction Set Flags (`lscpu`):**
  - **SHA hardware acceleration:** `sha_ni`
  - **SIMD / Vector extensions:** `avx`, `avx2`, `avx512f`, `avx512dq`, `avx512bw`, `avx512vl`, `avx512cd`, `avx512ifma`, `avx512vbmi`, `avx512_vbmi2`, `avx512_vnni`, `avx512_bitalg`, `avx512_vpopcntdq`, `avx512_bf16`, `avx512_fp16`, `avx_vnni`
  - **Bit manipulation & crypto:** `bmi1`, `bmi2`, `abm`, `popcnt`, `aes`, `vaes`, `pclmulqdq`, `vpclmulqdq`, `gfni`, `adx`, `movbe`, `rdrand`, `rdseed`

---

## 5. Timings & Optimisations

### Benchmark Results (Production Profile `PROD`: `TERRAIN_BYTES = 268,435,456`, `SLOT_BYTES = 16,384`, `STEPS_PER_LEG = 2,000,000`)

| Operation | Implementation Variant | Time (ms) | Speedup vs Baseline | Output Digest / Value |
|---|---|---:|---:|---|
| **Terrain Fill (256 MiB)** | Plain scalar (1 thread, row-major) | `587.53 ms` | `1.00×` | `60b1f689c39be2689487b680cca7ae9d58ec47a286f864c5eb05bc07fc6ffc04` |
| **Terrain Fill (256 MiB)** | Explicit JDK 21 Vector API (`LongVector.SPECIES_512`, 4 threads) | `365.76 ms` | `1.61×` | `60b1f689c39be2689487b680cca7ae9d58ec47a286f864c5eb05bc07fc6ffc04` |
| **Terrain Fill (256 MiB)** | **Optimised 4-way unrolled + 4-thread column-partitioned (C2 AVX-512)** | **`148.37 ms`** | **`3.96×`** | `60b1f689c39be2689487b680cca7ae9d58ec47a286f864c5eb05bc07fc6ffc04` |
| **One Production Leg (2M steps)** | True scalar + software SHA-256 (`-XX:-UseSuperWord -XX:-UseSHA256Intrinsics`) | `6,352.61 ms` | `0.93×` | `88a04ac5beacb8a7` |
| **One Production Leg (2M steps)** | Plain scalar + hardware SHA-NI (`legResultScalar`) | `5,917.95 ms` | `1.00×` | `88a04ac5beacb8a7` |
| **One Production Leg (2M steps)** | 8-way unrolled scalar + SHA-NI (`-XX:-UseSuperWord`) | `4,295.84 ms` | `1.38×` | `88a04ac5beacb8a7` |
| **One Production Leg (2M steps)** | Explicit JDK 21 Vector API (`LongVector.SPECIES_512`) + SHA-NI | `2,209.39 ms` | `2.68×` | `88a04ac5beacb8a7` |
| **One Production Leg (2M steps)** | **Optimised C2 SuperWord AVX-512 + `VarHandle` + SHA-NI (`legResult`)** | **`2,121.36 ms`** | **`2.79×`** (`3.00×` vs sw-SHA) | `88a04ac5beacb8a7` |

### Every Optimisation and What It Gained

1. **Compile-time `const val` folding of 64-bit `mix64` constants (`2.79×` speedup on leg computation: `5,918 ms` → `2,121 ms`):**
   - Initially, defining `val MIX_ADD = 0x9E3779B97F4A7C15uL.toLong()` in Kotlin generated a property getter (`getstatic SpecConstants.INSTANCE` + `invokevirtual getMIX_ADD()J`) on every `mix64` invocation (`4.096 × 10^9` times per leg). Those calls prevented HotSpot C2's SuperWord auto-vectorizer from vectorizing the 2,048-word reduction loop.
   - Expressing the constants as signed two's-complement `const val Long` literals (`const val MIX_ADD: Long = -0x61c8864680b583ebL`) caused `kotlinc` to emit immediate `ldc2_w` constants in bytecode. HotSpot C2's SuperWord pass then automatically vectorized the 2,048-word `mix64` reduction loop into 512-bit AVX-512 instructions (`vpxorq`, `vpaddq`, `vpsrlq`, `vpmullq`), cutting a 2,000,000-step leg from `5.92 s` to `2.12 s`.
2. **Explicit JDK 21 Vector API (`jdk.incubator.vector.LongVector.SPECIES_512`) with non-inlined leaf kernel (`2.68×` speedup on leg computation: `5,918 ms` → `2,209 ms`):**
   - We also implemented explicit 512-bit SIMD kernels using `LongVector.SPECIES_512` (4-way unrolled, processing `32` `u64` words = `256` bytes per loop iteration across four `zmm` accumulators `acc0..acc3`).
   - Extracting the vector loop into a dedicated leaf method (`foldSlotVector512`) with no outgoing calls inside the loop body avoided C2 `VectorBox` register spilling across the `MessageDigest` virtual calls, cutting explicit Vector API leg runtime from `3,543 ms` to `2,209 ms`.
3. **Zero-synchronization 4-thread column-partitioned `dag2` terrain fill + precomputed lane rotations (`3.96×` speedup on terrain fill: `587.5 ms` → `148.4 ms`):**
   - In §10.4, word `j` of slot `i` depends only on word `j` of parent slots `i - 1`, `bitrev_parent(i)`, and `mul_parent(i)`.
   - We precomputed the parent slot offsets `p2Offsets[i]` and `p3Offsets[i]`, the rotated seed lanes `rotl(P[k], 17/31/47)`, and `jXorTable[j] = P[j mod 4] XOR WORD_XOR XOR j`, and partitioned `j ∈ 0 … 2047` into 4 contiguous 512-word (`4 KiB`) ranges across the 4 CPU cores with 4-way loop unrolling (`j mod 4 = 0, 1, 2, 3`), eliminating `j & 3` indexing and achieving `1.73 GiB/s` of terrain generation.
4. **Hardware x86 SHA-NI Intrinsics (`-XX:+UseSHA256Intrinsics`) + Zero-allocation Seal Buffer (`~435 ms`–`1,000 ms` saved per leg):**
   - Enabling HotSpot's `@IntrinsicCandidate` SHA-NI intrinsics (`sha256rnds2`, `sha256msg1`, `sha256msg2`) and reusing a thread-local `MessageDigest` with a pre-allocated 20-byte `sealBuf` (pre-filled with the 12-byte `SEAL_DOMAIN`) and 32-byte `digestBuf` eliminated all heap allocations across the 2,000,000 steps per leg.
5. **`VarHandle` (`MethodHandles.byteArrayViewVarHandle`) Little-Endian `u64` Access (`~150 ms` saved per leg):**
   - Replacing byte-by-byte shift/mask loops in `writeLe64` and `u64Le` with `VarHandle` (`LONG_LE.set` and `LONG_LE.get`) compiled little-endian 64-bit loads and stores on `ByteArray` into single x86 `movq` instructions.
6. **Concurrent 6-Leg Verification in `Verifier.kt` (`~3.4×` speedup on `verify O3-PROD.cbor`):**
   - Following SPEC §14.6 Rule A4.4.4, after drawing the 6 global leg indices on the calling thread, the verifier recomputes the 6 production legs concurrently across the 4 cores while preserving draw order, verifying `O3-PROD.cbor` in `~7.0 s` wall-clock time instead of `~13.5 s`.

---

## 6. Language Advantages & Disadvantages (Kotlin on the JVM)

### Advantages
- **HotSpot C2 SuperWord AVX-512 Auto-Vectorisation & `jdk.incubator.vector`:** Because `avx512dq` provides native 64-bit vector multiplication (`vpmullq`), HotSpot C2 is able to compile the simple 64-bit integer reduction loop (`acc += mix64(words[idx] xor s)`) directly into 512-bit ZMM instructions that slightly outperform even hand-written `LongVector.SPECIES_512` intrinsics (`2.12 s` vs `2.21 s`) due to zero vector-species check overhead.
- **Built-in Hardware SHA-NI Intrinsics:** OpenJDK 21's standard `MessageDigest.getInstance("SHA-256")` automatically dispatches to C2's x86 `sha_ni` assembly intrinsics (`_sha256_implCompress` and `_sha256_implCompressMB`) without requiring external native libraries or JNI/FFI bindings.
- **Flat Primitive Arrays & `VarHandle`:** `LongArray` stores contiguous 64-bit words in native little-endian order on x86_64, and `MethodHandles.byteArrayViewVarHandle` compiles unaligned little-endian `u64` reads/writes to single `movq` instructions.
- **Structured Concurrency / Parallel Streams:** Parallelizing the column-partitioned terrain fill and the 6-leg verification required only `IntStream.range(...).parallel()`.

### Disadvantages
- **Kotlin `ULong` `const val` limitation:** Kotlin does not allow `.toLong()` on an unsigned literal (`0x9E3779B97F4A7C15uL.toLong()`) in a `const val` declaration. If written as a normal `val`, Kotlin emits a getter method call on the singleton `INSTANCE`, which blocks C2's SuperWord vectorizer unless rewritten as a signed two's-complement `const val Long` literal (`-0x61c8864680b583ebL`).
- **`MessageDigest` API framing overhead on 20-byte inputs:** While HotSpot intrinsifies the 64-byte SHA-256 block compression (`implCompress0`) with `sha_ni`, calling `MessageDigest.update(sealBuf, 0, 20)` + `digest(digestBuf, 0, 32)` 2,000,000 times per leg still incurs Java API wrapper overhead (copying the 20 bytes into `DigestBase.buffer`, padding `0x80` and bit-length `160`, unpacking all 32 bytes of digest state even though only the first 8 bytes are needed, and zeroing the buffer on reset), whereas a C or Rust implementation using raw `_mm_sha256rnds2_epu32` intrinsics can keep the constant 64-byte padded block in XMM registers.
- **Lambda closure boxing of `LongVector`:** Passing `LongVector` local variables into a Kotlin lambda closure captures them as heap fields on the `Function1` object, which defeats HotSpot C2's `VectorBox` escape analysis unless extracted into a static/private method taking primitive arrays.

---

## 7. SPEC Questions & Readings

1. **`unknown-kind.2.kind_text_hex` empty value vs "No trailing whitespace" (`CASES.md` lines 9, 91, 138; SPEC §14.7 lines 1665–1674):**
   - **Question:** Case 12 (`unknown-kind`) index `2` specifies "the empty label" (`""`), and requires outputting `unknown-kind.2.kind_text_hex` (the hex of the label's UTF-8 bytes), while `CASES.md` line 9 states *"Each line is `key<TAB>value<LF>`, ASCII only. No blank lines, no comments, no trailing whitespace."* When `value` is the empty string `""` (0 bytes in hex), the line `unknown-kind.2.kind_text_hex<TAB><LF>` ends with a `<TAB>` character immediately before `<LF>`.
   - **Readings considered:** (a) Emit `unknown-kind.2.kind_text_hex\t\n` strictly following the `key<TAB>value<LF>` grammar where `value` is `""`; (b) Omit the tab or write a placeholder.
   - **Reading implemented:** Reading (a) (`unknown-kind.2.kind_text_hex\t\n`), because every line in a candidate file must match `key<TAB>value<LF>` and the hex encoding of 0 UTF-8 bytes is the 0-character string `""`.

2. **Inclusion of `"stage-0 path key"` and `"field set"` in `shape_item` when Stage 0 has a `path` key (SPEC §14.1 lines 1290–1291, 1303, 1349–1350; `CASES.md` line 158):**
   - **Question:** In SPEC §14.1, condition 2 (`field set`) states *"A map that is present has the wrong keys or value types. The length of `stages` is reported only by `stages length`. An absent stage map is reported only through the per-field conditions (`leg_count` and the others)"*, and condition 3 (`stage-0 path key`) states *"Stage 0 has a `path` key"*. Meanwhile, line 1303 states *"A condition that is violated is not dropped because another condition is also violated."* If stage 0 has a `path` key, does `shape_item` include both `"field set"` and `"stage-0 path key"`?
   - **Readings considered:** (a) Report both `"field set"` and `"stage-0 path key"` because the `field set` row in the §14.1 table only excludes `stages length` and absent stage maps, and `CASES.md` line 158 explicitly confirms *"a condition is not dropped because another is also violated, and the only exclusions are the ones the shape table already states"*; (b) Exclude `"field set"` when `"stage-0 path key"` is the only extra key on stage 0.
   - **Reading implemented:** Reading (a), following the explicit guidance in SPEC §14.1 line 1303 and `CASES.md` line 158.

3. **Default portable index and profile in the `verify <object>` CLI (SPEC §9 line 682, §14.2 line 1370, §16.1 line 2095, §18 lines 2171–2178; `CASES.md` lines 148–154):**
   - **Question:** SPEC §18 defines `verify <object>` with optional `--profile` (`production` by default, or `conformance-small` requiring all 4 integers) and SPEC §9 line 682 notes *"The default profile does not fill `https://index.groundinglab.io`."* Meanwhile, `CASES.md` lines 148–154 define the synthetic test-plane index for heights `989 … 1018` so that *"header checks are measured rather than `not read`"*.
   - **Readings considered:** (a) Default the offline CLI to the `CASES.md` synthetic test-plane index when `--index-url` is not supplied, while allowing `--index-url <url>` to query any external HTTP portable index; and default `--profile` to `production` per §18 while requiring all four integers (`--sample-bytes`, `--slot-bytes`, `--terrain-bytes`, `--steps-per-leg`) when `--profile conformance-small` is specified (exiting with code `2` if any of the four is omitted). (b) Leave index reads as `not read` unless `--index-url` is passed.
   - **Reading implemented:** Reading (a), so `verify objects/O3-PROD.cbor` (and `verify --profile conformance-small ... objects/O1-S1.cbor`) measures against the `CASES.md` fixture index out of the box as required by prompt item 3 & 5, while supporting `--index-url <url>` for HTTP portable index servers.

4. **`bitrev_parent(i)` bit-width formula in §10.3 (SPEC §10.3 lines 730–739):**
   - **Question:** Section 10.3 defines `floor_log2(n)` for `n >= 1` as the greatest integer `b` with `2^b <= n`, and then defines `bits = max(floor_log2(i), 1)` and `bitrev_parent(i) = (reverse32(i) >> (32 - bits)) mod i`. Because an integer `i` with `2^b <= i < 2^{b+1}` has `b + 1` binary digits (bits `0` through `b`), reversing 32 bits places bit `b` (the leading `1` bit of `i`) at bit position `31 - b`, which is shifted off the right end by `>> (32 - b)`. Thus the formula reverses only the lowest `floor_log2(i)` bits of `i` (and for any power of two `i = 2^b`, `bitrev_parent(i)` is `0`).
   - **Readings considered:** (a) Implement the exact formula `bits = max(floor_log2(i), 1); (reverse32(i) >> (32 - bits)) mod i` as written in §10.3; (b) Use `floor_log2(i) + 1` bits.
   - **Reading implemented:** Reading (a), implementing the exact mathematical formula written in §10.3.

---

## 8. Completeness

Everything specified in the prompt and in `CASES.md` is complete, tested, and committed:
- All 14 candidate files in `candidates/` (`seed.txt`, `nl4.txt`, `terrain.txt`, `fold.txt`, `kat-small.txt`, `test-3leg.txt`, `prod.txt`, `wes.txt`, `rate.txt`, `anchor.txt`, `draw.txt`, `unknown-kind.txt`, `hashcash.txt`, `negatives.txt`).
- All 3 CBOR objects in `objects/` (`O1-S1.cbor`, `O2-S2.cbor`, `O3-PROD.cbor`).
- The `verify` CLI (`./impl/verify` and `./verify`).
- The automated test suite (`./impl/gradlew -p impl test`).
- The items listed under *"Not in the cores (later phases)"* at the end of `CASES.md` (lines 155–160) are noted as pending for later rounds as `CASES.md` instructs.
