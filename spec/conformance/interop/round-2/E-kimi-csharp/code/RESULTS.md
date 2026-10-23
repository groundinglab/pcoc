# RESULTS — Satoshi Hike SPEC 2.2 (erratum 1), C# / .NET 8

Independent implementation built only from the three pinned inputs:

- `spec-ref/SPEC.md` — SPEC 2.2 at commit `7b40353` (sha256 `8e591ea02e7c73ac3d1a4e61602f25baaae88cabdc482240d310d2629dfa5889`, verified)
- `spec-ref/POLICY.md` + `spec-ref/gl-admit-v9.toml` — groundinglab-admission policy at the same commit
- `spec-ref/CASES.md` — round-2 interop fixtures at commit `8c40cf8`

No other implementation, repository, branch, or code about PCOC / Satoshi Hike was read, fetched, or copied.

---

## Possible SPEC flaw

I looked specifically for ways to produce correct, verifying outputs with **less** work than the SPEC intends. What I found:

**1. Lazy (compute-on-first-read) terrain fill — real, but worth almost nothing, because of the `i − 1` parent edge.**
All three dag2 parents of slot `i` (`i − 1`, `bitrev_parent(i)`, `mul_parent(i)`, SPEC.md lines 769–773) are `< i`. Therefore slot `i` depends only on slots `0 … i − 1`, and a filler can compute slots lazily on first touch, memoized, producing byte-identical terrain words for every slot actually read. A verifier (or producer) using lazy fill pays for slots `0 … M` where `M` is the highest slot index ever sampled, instead of `N_SLOTS`.
What it saves in practice:

- Production profile: `N_SLOTS` = 16384 and one leg draws 2,000,000 samples (§11.3 slot index `u64_le(B[0:8]) mod N_SLOTS`). The probability that the maximum of 2M draws leaves any low slot untouched is ~0; `M` = 16383 effectively immediately. Savings ≈ 0%.
- Conformance-small S1 (1024 slots, 1800 draws per hike): coupon-collector leaves ~17% of slots untouched, but the whole S1 fill is ~65 µs, so the absolute saving is noise.
- The one shape where it would matter — a verifier that only checks a few steps of a huge terrain — does not exist in SPEC 14: the full record recomputes every leg of every stage (§14 line 1281: the length of `legs` is the number of checkpoints; Rule A4.3), which touches everything.

So the only reason this is *not* an exploitable flaw is the `i − 1` edge: it forces the closure of slot `i` to be the entire prefix `0 … i`. If that parent were absent (e.g. only `bitrev`/`mul` parents), lazy evaluation would skip most of the fill on small profiles and the "terrain is filled once per terrain" cost (line 164) would be unenforceable. **I did not implement lazy fill; the full §10.4 fill runs every time.** I report it because the task asks for it, with the honest conclusion that at SPEC 2.2 parameters it saves nothing measurable.

**2. Fold and seal do not compress.**
`S_folded = Σ mix64(word(w) XOR S) mod 2^64` (§11.1). Because `mix64` multiplies by odd constants between xorshift stages, `mix64(a XOR S)` has no homomorphism in `a` that survives the multiply: no per-slot summary `f(slot)` allows `S_folded = g(f(slot), S)` for arbitrary `S`. The four §11.1/§16 fold negatives in `candidates/negatives.txt` are exactly the compressible variants, and all differ from honest on the S1 vector (all `matches_honest` = `false`). The per-step seal `S' = u64_le(SHA-256(SEAL_DOMAIN ‖ le64(S_folded))[0:8])` (§11.1) makes the step chain sequential and non-mergeable; the slot index of step `t + 1` depends on `S_{t+1}`, so trajectory precomputation across steps is impossible. I found no correct shortcut in the leg loop.

**3. The §15.3 fast record is SPEC-sanctioned less work — by design, not a flaw.**
Line 1275 states the admission fast record checks checkpoint digests from *stored* limbs and "does not recompute a leg". A fabricated hike with self-consistent digests passes the fast record but is caught by the full §14 record's leg recompute. This is the SPEC's stated fast/thorough split, so it is not reported as a flaw — but integrators should note the fast record alone does not bind terrain.

**4. No verifier-side caching opportunity exists across legs or stages.**
Every leg starts from the previous checkpoint digest (§12), so the hike is one sequential chain; no two steps of an honest verify share work. The only reuse the SPEC allows — terrain filled once per run — is mandated, not a cheat.

Net: within the full §14 record, the intended work (fill once + every word of every slot of every step + one SHA-256 per step) is, to my analysis, unshirkable. The nearest thing to a flaw is (1), neutralized by the `i − 1` edge.

---

## Model and effort

- Model: **Kimi K3** (`kimi-k3`), running as a Cursor cloud agent (autonomous background session).
- Effort/reasoning setting: not exposed to the agent; whatever the default for this agent runtime is.
- The pinned SPEC.md, POLICY.md, gl-admit-v9.toml, and CASES.md are committed under `spec-ref/`; everything else in this repo was written in-session from those files.

## What was built (`impl/`)

.NET 8 solution `impl/Pcoc.sln`, C# 12, three projects:

- `src/Pcoc` — the construction library:
  - §6 constants and §16.1 `conformance-small` profile (`Profile.cs`);
  - §10 terrain: seed, NL4 base word, parent indices, dag2 fill, with three interchangeable engines — scalar, AVX2, AVX-512 — selected at runtime (`Terrain.cs`, `Prims.cs`);
  - §11 leg/step: fold (three engines, bit-identical), seal, checkpoint, §12.2 checkpoint fold and waypoint (`Leg.cs`);
  - SHA-256 backend: managed `SHA256` or SHA-NI libcrypto via P/Invoke (`Sha256Fast.cs`);
  - §7.3 CBOR deterministic encode and strict decode (`Cbor.cs`), §7–8 object model (`HikeObject.cs`);
  - §9 index reading (HTTP + the CASES synthetic index), §14.3 canonical path body (`Index.cs`, `PathBody.cs`, `Json.cs`);
  - §13.1 Hashcash (`Hashcash.cs`), §14.6/§15.5 draws (`Draw.cs`), RFC 8785 string spelling for Rule A5 (`Rfc8785.cs`);
  - the §14 verifier incl. Rule A5 and the §16.1 profile (`Verifier.cs`).
- `src/Pcoc.Cli` — `pcoc-cli` with subcommands `gen-candidates`, `produce`, `verify`, `bench`. The §16 permanent negatives live only here, in the module named `negatives` (`src/Pcoc.Cli/negatives.cs`), never on an SDK path.
- `tests/Pcoc.Tests` — 35 xUnit tests, including the required re-run-the-generator-and-compare test (`Conformance.cs::CandidatesUnchanged`).

Artifacts: `candidates/` (14 files, the exact CASES keys), `objects/O1-S1.cbor`, `O2-S2.cbor`, `O3-PROD.cbor`.

## Exact commands

```bash
export PATH="$HOME/.dotnet:$PATH"          # .NET SDK 8.0.426 lives in ~/.dotnet
cd impl
dotnet build Pcoc.sln -c Release
dotnet test  Pcoc.sln -c Release                                    # 35/35 pass
PC=src/Pcoc.Cli/bin/Release/net8.0/pcoc-cli.dll
dotnet $PC gen-candidates --out ../candidates
dotnet $PC produce --out ../objects
# verify (conformance-small runs take the four §16.1 integers)
dotnet $PC verify ../objects/O1-S1.cbor --profile conformance-small \
  --slot-bytes 64 --sample-bytes 32 --terrain-bytes 65536 --steps-per-leg 100 \
  --synthetic-index --draw-label pcoc-pack-2.2/draw
dotnet $PC verify ../objects/O2-S2.cbor --profile conformance-small \
  --slot-bytes 256 --sample-bytes 64 --terrain-bytes 1048576 --steps-per-leg 1000 \
  --synthetic-index --draw-label pcoc-pack-2.2/draw
dotnet $PC verify ../objects/O3-PROD.cbor --synthetic-index --draw-label pcoc-pack-2.2/draw
dotnet $PC bench --engine scalar   # also: avx2, avx512
```

`verify` exits 0 and prints the §14 record (one trailing LF); exit 1 is a §7.3 decode failure; exit 2 is usage. All three committed objects verify with `shape: match`, all 19 headers `match`, `seed_value: match`, every checkpoint/waypoint `match`, paths 1–16 `skip`/`n/a`, W17 `not fetched by design`, 16 × `not anchored`, bookends `not defined`, `six_leg` 6 × `match`, `whole_hike_span` 10200, `hike_span` 10800, per-stage spans 600.

## Machine

- Cloud VM, Ubuntu 24.04, kernel 6.12.94+, x86_64, 4 vCPUs, 15 GiB RAM.
- CPU: `Intel(R) Xeon(R) Processor` (hypervisor-generic model string).
- Relevant `/proc/cpuinfo` flags: `avx2`, `avx512f`, `avx512dq`, `avx512bw`, `avx512vl` (also `avx512ifma/vbmi/vbmi2/vnni/bitalg/vpopcntdq/cd/bf16/fp16`, `avx_vnni`), **`sha_ni`**, `bmi1`, `bmi2`, `adx`, `pclmulqdq`, `sse4_1/4_2`.
- .NET SDK 8.0.426 (runtime 8.0, `InvariantGlobalization`).

## Timings

Production profile (TERRAIN = 256 MiB = 16384 slots × 2048 words; one leg = 2,000,000 steps, each step = one 1024-byte sample read + a full 16 KiB slot fold + one seal). `pcoc bench` output on this machine; each configuration run twice, both values shown.

| Measurement | scalar | AVX2 | AVX-512 |
|---|---|---|---|
| Terrain fill (§10.4) | 0.584 s (438 MiB/s) | 0.366 s (699 MiB/s) | 0.293 s (874 MiB/s); repeat 0.318 s (805 MiB/s) |
| One production leg (§11) | 5.744 s / 5.758 s (~348,000 steps/s) | 3.951 s (~506,000 steps/s) | 2.765 s (~723,000 steps/s); repeat 2.976 s (~672,000) |
| Seal, per hash (§11.1) | 687–692 ns managed; 307–314 ns libcrypto | same | same |

All three engines produce the identical leg result (`a7b8acbec54aa088` for the bench vector) — vectorization is bit-exact, as it must be. End-to-end wall times on this machine: `gen-candidates` 5.4 s total (includes the PROD fill and PROD first leg), `produce` (all three objects) 49.7 s, `verify` of O3 16.5 s (fill + 18 legs + 6 redrawn legs), verify of O1/O2 ~0.1 s each.

## Optimizations and what they gained

1. **Vectorized terrain fill (§10.4).** The fill is sequential across slots (the `i − 1` parent) but independent across the `j` words within a slot: the parent words `pw` are per-`(j, lane)` reads of already-stored slots, `rot = (j + k) mod 64` is a per-lane variable rotate, and the rest is `mix64`. Implemented with 4 lanes (AVX2) and 8 lanes (AVX-512 `Vector512<ulong>`). 438 → 699 MiB/s (1.6×) and → ~840 MiB/s (≈2.0×) respectively.
2. **64-bit vector multiply.** `mix64` needs 64-bit low multiplies. AVX-512DQ has `vpmullq` (`Avx512DQ.MultiplyLow`) — used directly. AVX2 has none: emulated with three `vpmuludq` partial products (lo·lo, plus the two cross terms shifted 32), which is why AVX2 trails AVX-512.
3. **Branch-free variable rotates.** `rotl(x, r)` with `r = 0` (slot 0's `j + k` can be 0 mod 64) is done as `sllv(x, r) | srlv(x, 64 − r)`; on x86 a variable shift count ≥ 64 yields 0, so the OR is the identity when `r = 0`. No branches, correct for all `r`.
4. **Vectorized fold (§11.1).** Wrapping addition is commutative and associative, so the fold's `acc` can be split into 4/8 lane accumulators summed horizontally at the end — bit-identical to the scalar order. The whole `mix64` (add constant, two xorshift-multiply rounds, final xorshift) vectorizes. One production leg: 348k → 506k (1.45×) → ~700k steps/s (≈2.0×). At 2M steps × 16 KiB the leg streams ~33 GiB of terrain per leg; the measured ~12 GiB/s says the AVX-512 fold is already close to this VM's memory path, so wider vectors would not buy much more.
5. **SHA-NI seal via libcrypto.** .NET 8 has no public x86 SHA intrinsics; `SHA256.TryHashData` on the 20-byte seal message costs ~690 ns, dominated by per-call setup. `Sha256Fast` P/Invokes libcrypto's one-shot `SHA256()` (dlopen `libcrypto.so.3` → `so.1.1` → `so`, one cached delegate, pinned spans), which uses SHA-NI on this host: ~310 ns, **2.2×**. Each production step seals once, so this saves ~0.76 s of the 2.77 s AVX-512 leg. SPEC §11.1 names the digest, not the implementation; bytes are identical (asserted in tests against managed SHA-256 vectors).
6. **Flat terrain layout.** One `ulong[]` for the whole terrain (256 MiB at PROD); a slot is a `Span<ulong>` slice — no per-slot objects, sequential memory for the fold, `MemoryMarshal.AsBytes` for the whole-terrain SHA-256 with zero copies.
7. **Cached engine dispatch.** The fold engine is chosen once into a static delegate (`EngineSelection.Current`, overridable with `PCOC_ENGINE=scalar|avx2|avx512` for A/B); hot scalar helpers are `AggressiveInlining`. No per-step switch.
8. **Verifier does the fill once per run** (SPEC: "once per terrain") and recomputes legs streaming slots directly out of the flat array; the six redrawn §14.6 legs reuse the same terrain.
9. **Rejected / not applicable:** multithreading — the dag2 `i − 1` edge makes the fill strictly sequential across slots, and the checkpoint chain makes legs strictly sequential; there is no in-SPEC parallelism to take beyond SIMD. (The 4-core VM would also have made memory-bandwidth the wall.) Slot-read caching across steps — that is the `cached_leg` negative, forbidden; not done.

## Language advantage / disadvantage

**Advantage, modest.** `System.Runtime.Intrinsics` gives typed, JIT-compiled AVX2/AVX-512 with runtime hardware dispatch from one binary — the same dll runs scalar on old CPUs and AVX-512 here, no separate builds. `Avx512DQ.MultiplyLow` mapping straight to `vpmullq` is nicer than many FFI-bound setups, and spans + `MemoryMarshal` give zero-copy hashing over the flat terrain. AoT was not needed; the JIT fully unrolls and hoists the fold loop.

**Disadvantages.** (a) No public SHA-NI intrinsics in .NET 8 — the seal is the hottest hash and the managed one-shot API costs ~690 ns/call; I had to leave the runtime and P/Invoke libcrypto to get SHA-NI (2.2×). A C/Rust build could inline SHA-NI intrinsics straight into the step loop with no call overhead at all. (b) Bounds-check elimination is very good but not guaranteed; the fold is written so the JIT can prove every access, which constrains code shape. (c) No 128-bit multiply result type for the AVX2 `vpmuludq` emulation path — same as every language without `vpmullq`, but it shows in the AVX2 column. Net for this task: roughly neutral vs C/Rust on the fill and fold, behind on the seal, ahead on ergonomics and single-binary dispatch.

## SPEC questions (section and line refer to the pinned `spec-ref/SPEC.md`)

1. **What "erratum 1" is.** The pinned file contains no literal "erratum" marker. Reading implemented: erratum 1 is the §7.3 input-sorting order (line 497: malformed CBOR → shape `mismatch` with `malformed cbor`; then a well-formed non-map or map-without-kind-text → decode failure; then kind ≠ KIND → Rule A5; everything else → §14), restated at line 228. The verifier implements exactly that order (`Verifier.Verify`).
2. **Stage-0 `path` key: one shape item or two.** §14.1 table item 2 (`field set`, line ~1290) and item 3 (`stage-0 path key`, line 1291) both fire when stage 0 has a `path` key; §8 (line 590) says omission is the encoding. Readings: (a) report item 3 only; (b) report both, since the exact key set is violated *and* the specific rule names its own string, and CASES.md (line ~120) says a condition is not dropped because another is also violated. Implemented: **both**, in table order.
3. **Scope of `height_range`.** Line 580 names `beacon_height`/`last_interrupt_height`; item 14 (line 1302) says "An integer above 2^53 − 1 is in the object's own bytes, **including** …" — non-exhaustive. Readings: (a) only the two height fields; (b) every unsigned integer the verifier consumes from the object's bytes (heights, `leg_count`, the size fields). Implemented: **(b)**; oversized values become unusable → `not defined`, never clamped (line 580). Path-body internals are excluded per P14.6 (line 214).
4. **When a stage's checkpoint count is "usable".** Line 961 ("adds each stage's checkpoint count, including zero … A missing stage leaves `whole_hike_legs` `not defined`") and line 1281 ("An empty checkpoint array is usable"). Readings: (a) count = array length whenever the stage is a map and `checkpoints` is an array, even on other shape mismatches; (b) only when the stage is fully shape-clean. Implemented: **(a)** — ND only when the stage is missing/not a map/`checkpoints` not an array.
5. **`six_leg` when `leg_total` cannot be formed.** The draw "is stated on `leg_total`" (line 1629); line 961 gives ND only for `leg_total < 6`. Readings: (a) ND when any stage's count is unusable; (b) draw over the usable prefix. Implemented: **(a)** — an unsettled total is not a number, so the record is `not defined` (with empty `indices`/`results` per line 1281's missing-value rule), never a partial draw.
6. **Injected draw bytes under the section 6 constants.** Line 1259 ("A rig may supply the Rule A4.4 draw bytes") vs lines 1261/2114 ("Production verify reads the operating-system CSPRNG"). Readings: (a) injection only under `conformance-small`; (b) a rig may inject for any profile — "production verify" names live verification, and the offline test plane (O3) is a rig. Implemented: **(b)** — the CLI accepts `--draw-label` / `--draw-bytes-hex` with any profile and defaults to the OS CSPRNG otherwise. Flagging because the two sentences pull at each other.
7. **Non-empty W17 path: how much decode.** Line 1482: "decode it only far enough to read `block_hash` and `block_height` … using the canonical path layout. Do not run `validate_for`." Readings: (a) full `CanonicalPathBody` decode minus `validate_for` (rejecting non-canonical sibling counts etc.); (b) minimal structural read — sibling count at byte 41, then the two fields at their canonical offsets when `length ≥ 146 + 32·count`, `count ≤ 32`. Implemented: **(b)** (`PathBody.TryReadClaimedBlock`); `w17_path` is `not defined` and `shape_item` gets `W_17 path` whenever the stored bytes are non-empty, whatever the partial read finds.
8. **CBOR nesting depth.** §7.3 (line 497) enumerates malformed conditions but names no depth limit; unbounded recursion is a stack-overflow vector. Readings: (a) no limit; (b) an implementation limit counted as malformed. Implemented: **(b)**, depth 64 — recorded here because it is an implementation choice the SPEC does not name; no conforming object comes near it.
9. **One index GET serving several rules.** Line 1332 speaks of "a plant document with no usable `header` that Rule A4.8 reuses". Readings: (a) each rule issues its own GETs; (b) the verifier may cache one document per height per run. Implemented: **(b)** (A4.2 headers, the A4.10 MTP window, and A4.8 plant/header reads share per-height documents). No record value changes; it only saves reads.
10. **MTP window other-reads need no `header.hash` agreement.** Line 1587: the beacon's *own* document time is taken only when its `index_comparison` is `match`, but each *other* window document only needs "the read returned a document, `header.height` equals the requested height, and `header.time` follows section 9". Reading considered: require hash agreement for all n documents. Implemented: SPEC-literal — no hash requirement on the other n−1 reads (`IndexRead.MtpTime`).
11. **`fixed_start` gating.** Line 1334 is intricate about `not read` vs `not defined`. Implemented: `fixed_start.N` = checkpoint count over stages 1 … b−1; `fixed_start.S = T_P_b − headers[1].T` (`not read` if either time is `not read`); `fixed_start.delta_h` = index-confirmed `height(P_b)` − index-confirmed `headers[1]` height, `not read` when either side is `not read`, `not defined` only in the two named cases (no bookends from the object's own bytes, or a `mismatch` on either side). With all paths `skip` (our objects) step 3 records every bookend field `not defined` while `fixed_start` stays an object of three `not defined` (line 223, line 1601).
12. **Wrong-width byte strings on the record.** A stored beacon/waypoint/checkpoint that is not 32 bytes: hex-it-anyway or `not defined`? Line 1281 ("leaves a key or an array element without a usable value … `not defined`") implemented literally: wrong-width stored values are `not defined` on the record, and the corresponding shape item fires.

## Deviations from CASES.md

1. **Directory layout.** CASES names `sdk/<lang>/candidates/`; this build writes `candidates/` at the repo root (and `objects/` likewise), as the tasking for this build specified. File names, keys, order, and byte format are exactly as CASES defines.
2. **`unknown-kind.2.kind_text_hex` is the empty string** (the empty label, hex of zero bytes), so that one line ends with its TAB separator. Present because the key template requires `<n>` = 0 … 3; no invented value.
3. **Offline test plane.** No Bari POSTs are attempted (this VM has no Bari and the fixtures fix path bytes to empty): produce logs `path_skip 1 … 16` then `not fetched by design 17`, and the self-verify records show `not anchored` × 16 and `not defined` bookends, as expected for all-empty paths.
4. The CASES "later phases" list (paid rule, sign-in vectors, the erratum-1 edge objects) is out of scope for the cores per CASES itself and is not built.

## Not finished / known limits

- Nothing assigned is unfinished. Honest edges: `--index-url` (HTTP index) is implemented and wired but was exercised only against the synthetic in-process index — this VM has no live index to point at; the §15.3 fast record is described in code comments but the CLI prints the full §14 record only; POLICY.md / gl-admit-v9.toml were read for the Rule B2/B3 context used by `rate.txt`/`anchor.txt`, but no admission service (§15) is built, per the task scope.
