# RESULTS: Satoshi Hike SPEC 2.2 (erratum 1) in TypeScript on Node.js

## Possible SPEC flaw

**No work-skipping shortcut found.** I looked for one on purpose and I did not ship any. What I checked:

| Candidate | Result |
| --- | --- |
| Skip or cache slot reads inside a leg (the `cached_leg` negative in CASES) | Not valid. The slot read at step `t` is chosen by the running state `S`, which depends on every earlier fold. A repeated slot is folded with a different `S`, so the contribution differs (`acc = Σ mix64(word ⊕ S)` is not separable in `S`). The `cached_leg` candidate fails verification, as CASES intends. |
| Reuse work across legs | Not valid. The next leg starts at `S0 = u64_le(prev[0:8])` from the previous checkpoint, so its fold is a different function of the slot words. |
| Partial slot reads | Not valid. Every word goes through `mix64(word ⊕ S)` and into the sum. Dropping any word changes `acc` with overwhelming probability. |
| Precompute per-slot summaries | Not valid, for the same reason. The mix is nonlinear in `S`, and `S` is unknown until the step runs. |
| Share the terrain across the six recomputed legs and across all legs of a hike | Allowed and intended. One hike has one terrain. I fill it once per verification. This is the SPEC's cost model, not a saving beyond it. |
| A verifier that cannot tell | No. A wrong `legs_le_hex`, a wrong checkpoint, a wrong waypoint and a `cached_leg` object all yield `mismatch` at the intended place (tests `verify.test.ts`, `candidates.test.ts`). |

Observations that are not flaws but the SPEC authors should know:

1. **The "must use SHA instructions" rule is hard to follow from Node, and the compliant path is slower here than a software SHA.** SPEC §11.1 (line 837) and Rule I6 require hardware SHA-256 for every SHA-256 in the construction where the host has it. Node has no synchronous raw-block SHA call, so each of the 2,000,000 per-step seal hashes goes through `node:crypto` (OpenSSL, SHA-NI on this machine) and the measured cost is about 0.9 µs per step more than a software SHA-256 block in WebAssembly ((6.68 − 4.83) s / 2,000,000). On this machine, a one-block software SHA-256 written in WebAssembly is faster end to end (4.8 s against 6.7 s per production leg). I ship the `node:crypto` path as the default (the SPEC wording) and the software path only as a labelled measurement (`bench.js`, `sha=wasm`). Rule I6 also says an SDK change that raises the measured leg rate at unchanged hardware is a suspected break. A platform-level per-call overhead can invert the rule's intent. It would help to say how to treat runtimes whose call overhead exceeds the software cost.
2. **Terrain fill is cheap against a leg.** 256 MiB fills in 0.18 s on 4 threads (0.48 s on one). One leg is 4.8–6.8 s. Fill is parallel by column (Section "Optimisations"), so an attacker with many cores pays little for the fill. That is a design property, not a shortcut, but the SPEC's cost argument rests on the legs.
3. The fold step costs roughly 1 ns per 64-bit word (16 KiB slot = 2048 words, about 2 µs per step) with WebAssembly. I could not find a SIMD form that beats it (see the failed SIMD experiment).

## Model and effort

Claude Sonnet 5.5, high effort (as far as I can tell from my session; not independently confirmed).

## What was built

All code is in `impl/` (TypeScript, Node.js 22, ES modules, `strict`).

- **Construction (§6–12).** `profile.ts` (constants, `makeProfile` with the §16.1 range checks, PRODUCTION, S1, S2). `engine.ts`, `kernels.ts`, `wasm.ts`, `worker.ts`: the terrain fill and hike legs as generated WebAssembly. `scalar.ts`: plain TypeScript tier. `bigref.ts`: a naive BigInt oracle used only for cross-checks. `construction.ts`: seed, checkpoints, waypoints, fixtures, `produceHike`.
- **Object encode and decode (§7–8).** `cbor.ts` (deterministic encoder, strict decoder with every §7.3 malformed case), `object.ts` (`HikeObject`, `encodeObject`, `classify`, Rule A5 unknown kind).
- **Verifier (§14, §16.1).** `verify.ts` with `jsonstrict.ts` (duplicate-name rule, §9 body strictness), `indexsource.ts` (HTTP index client and the CASES fixture index), `draw.ts` (§14.6 unbiased draw, six-leg draw, §15.5 thorough draw as a library function), `compare.ts`, `pathbody.ts` (canonical path body, `validate_for`).
- **Candidate generator.** `gen-candidates.ts` writes every CASES case with exactly its keys into `candidates/` (14 files: anchor, draw, fold, hashcash, kat-small, negatives, nl4, prod, rate, seed, terrain, test-3leg, unknown-kind, wes). Negatives live only in `negatives.ts`.
- **Producer and CLI.** `cli.ts`: `produce <O1|O2|O3|--all> <dir>`, `verify <object>`, `serve-index [port]`. Exit codes 0, 1, 2 as in §17.
- **Objects.** `objects/O1-S1.cbor` (3639 B), `O2-S2.cbor` (5189 B), `O3-PROD.cbor` (3644 B; production profile, one leg per stage).
- **Tests.** `src/test/*.test.ts`: 38 tests. They re-run the generator and compare with the committed `candidates/`, re-produce O1 and O2 byte for byte, cross-check kat-small against a naive BigInt hike, and the scalar tier against the engine. They also cover CBOR decoding, the verifier's shape items, index body strictness over real HTTP, MTP, paths and plants, anchors, bookends and draws.

Key values (for cross-checks): PRODUCTION terrain SHA-256 `60b1f689c39be2689487b680cca7ae9d58ec47a286f864c5eb05bc07fc6ffc04`; stage 0 leg 0 `legs_le_hex` `88a04ac5beacb8a7`, checkpoint `4fa854e2421aa429f3d4b7be236261eb022a234d844dac27bee879c26eff2193`; S1 terrain `366e6d1d…bd97f`; hashcash nonce 2400506. Object SHA-256: O1 `800ad07a…d246`, O2 `6bfc769b…e5e0`, O3 `b16ecdaa…bea7`.

## Exact commands

```sh
cd impl
npm install
npm run build
node dist/gen-candidates.js candidates          # regenerates candidates/ (deterministic)
node dist/cli.js produce --all objects          # O1, O2, O3 into objects/
node dist/cli.js verify objects/O1-S1.cbor --profile conformance-small \
  --sample-bytes 32 --slot-bytes 64 --terrain-bytes 65536 --steps-per-leg 100 --index-fixture
node dist/cli.js verify objects/O2-S2.cbor --profile conformance-small \
  --sample-bytes 64 --slot-bytes 256 --terrain-bytes 1048576 --steps-per-leg 1000 --index-fixture
node dist/cli.js verify objects/O3-PROD.cbor --index-fixture   # about 11 s wall clock
npm test                                        # build, then node --test dist/test/*.test.js (about 17 s)
node dist/bench.js timings                      # the table below (about 3.5 min, dominated by the plain TS leg)
node dist/bench.js prodleg | fold | fill | leg | all
```

`verify` also takes `--index-url <url>` for a live HTTP index, `--draw-label`, and `--threads N`.

## Machine

- Intel(R) Xeon(R) Processor (cloud VM, Sapphire Rapids class), 4 vCPUs, Node v22.23.4.
- Flags include: `avx2`, `avx512f`, `avx512dq`, `avx512bw`, `avx512vl`, `avx512ifma`, `avx512vbmi`, `avx_vnni`, `sha_ni`, `bmi1`, `bmi2`, `adx`, `aes`, `vaes`, `gfni`, `amx_*`.
- Shared VM, so timings are noisy (about ±10%). Numbers are wall clock.

## Timings

Production profile (256 MiB terrain, 2,000,000 steps per leg, 16 KiB slots, 1 KiB samples).

| Task | Variant | Time |
| --- | --- | --- |
| Terrain fill | plain TypeScript, 1 thread, no WebAssembly (`scalar.ts`) | 10.72 s |
| Terrain fill | WebAssembly kernel, 1 thread | 0.48 s |
| Terrain fill | WebAssembly kernel, 2 threads | 0.31 s |
| Terrain fill | WebAssembly kernel, 4 threads | 0.18 s |
| One production leg | plain TypeScript (32-bit limbs) + `node:crypto` SHA-256; a full 2M-step run | 147.32 s |
| One production leg | WebAssembly fold + `node:crypto` SHA-256 (SHA-NI). Default | 6.68 s (6.8–7.2 s over repeats) |
| One production leg | WebAssembly fold + WebAssembly software SHA-256. Measurement only, does not follow the §11.1 hardware rule | 4.83 s (4.8–5.3 s over repeats) |
| Six production legs at once (the §14.6 recompute) | WebAssembly fold + `node:crypto`, 6 workers on 4 vCPUs | 9.92 s |
| `verify objects/O3-PROD.cbor` end to end | everything above included | 11.2 s wall, 43 s CPU |

Speed-up against plain scalar: fill 22× on one thread, 60× on four; leg 22× with the same SHA path.

## Optimisations and what they gained

1. **WebAssembly with native `i64` instead of BigInt or 32-bit limbs.** The mix and fold are 64-bit multiply, rotate and xor chains. The TypeScript scalar tier needs 32-bit limb arithmetic to avoid BigInt. The kernels are emitted from TypeScript by a small WASM emitter (`wasm.ts`, `kernels.ts`) and share one `WebAssembly.Memory`. Gain: about 22× on both fill and leg over plain TypeScript.
2. **Stage-interleaved scalar × 8 fold.** The fold for one step is a long chain per word. Interleaving 8 independent word accumulations hides the multiply latency. `bench.js fold` measures 1.0–1.5 ns per word across shapes; I kept the best scalar shape. I did not keep a separate one-accumulator build to quote an exact ratio.
3. **Fill unrolled ×4.** I did not record a separate before/after number for this one.
4. **Column-parallel terrain fill across `worker_threads`.** The dag2 fill has independent columns. They are split across workers over the shared memory. Gain: 0.48 s → 0.18 s on 4 threads.
5. **Six legs in parallel workers** for the §14.6 recompute: 6 legs in 9.9 s against 6 × 6.7 s serial (4 vCPUs).
6. **Hardware SHA for each step's seal** through `node:crypto` by default, to follow §11.1. The software SHA-256 in WASM is kept only as a measurement path.
7. **Failed experiment: SIMD.** `v128` forms of the fold (2 and 4 lanes, with mixed scalar lanes) were no faster than the scalar × 8 shape (`bench.js fold`: 2.0–2.7 µs/step in all shapes). I did not investigate why in depth; the fold has a data-dependent chain on `S` per step, which limits lane-level gain. I did not ship it.
8. **Memory layout.** The terrain is one linear `Uint8Array`/WASM memory region read with 64-bit loads. There is no per-leg copy. Digest and samples read the same memory.
9. **Not done:** AVX-512 or SHA-NI intrinsics. Node and WASM cannot reach them directly. That is the language limit below.

## Language advantage or disadvantage

**Mixed, mostly a disadvantage for a faithful cost implementation.**

Advantages: strong typing for the object and verifier logic; `Buffer` and the built-in `node:crypto` (hardware SHA, OpenSSL) and `node:http` made the CBOR, JSON strictness and index-over-HTTP parts short; WebAssembly with native `i64` gave a 22× improvement without native code or a compiler toolchain; `worker_threads` with a shared `WebAssembly.Memory` made column-parallel fill easy.

Disadvantages: no intrinsics, no AVX-512, no direct SHA-NI, WASM SIMD gave no gain, so the fold is not vectorised the way C or Rust might. `node:crypto` costs about 0.9 µs per call more than a software SHA block in WASM, so the rule "use SHA instructions" slows this implementation (see Observation 1). JavaScript has no 64-bit integer type except BigInt, so the plain tier is slow (147 s per leg) and WASM is mandatory for a practical build. Garbage-collection pauses are irrelevant here because the hot loops allocate nothing.

I expect a C, C++ or Rust build with `-C target-cpu=native`, SHA-NI intrinsics and AVX-512 to beat this by roughly 2× to 4× per leg. I did not measure it.

## SPEC questions

Each: place, readings considered, and what I implemented.

1. **§14 `seed_value` (line 1312).** The row says `match` if `stages[0].waypoint` equals `SHA-256(public_key ‖ seed_beacon)`, `mismatch` otherwise. It does not say what happens if `stages[0]` has no waypoint, or `public_key` or `seed_beacon` is absent. Readings: (a) `mismatch`; (b) `not defined`. Implemented: `not defined` when the digest `D` cannot be computed; `mismatch` when `D` exists and the waypoint is absent or different; `match` on equal.
2. **§14 shape item `field set` (line 1290) against absent stages.** "An absent stage map is reported only through per-field conditions." Implemented as written: no `field set` for an absent stage map; the per-field conditions report it.
3. **Shape item 3 `stage-0 path key` (line 1291)** is reported when stage 0 has a `path` key of any type or content. Implemented so. Reading (b), only a non-empty path, was rejected: the text says "has a `path` key".
4. **`height_range` (line 1302).** "An integer above 2^53 − 1 is in the object's own bytes." Readings: reject while decoding; or keep the exact integer and report only the shape item. Implemented the second, with exact integers held as BigInt, so later measurements still run (§7.3 says later measurements are still recorded).
5. **`path_state` (line 1320).** Values are `present` or `skip`, but the SPEC does not say what a record shows for a stage whose `path` key is absent entirely (not empty). Implemented: `path_state` `not defined` and `plant` `not read`, `P_g`… `not defined`. A reading with `skip` was rejected because `skip` is the empty-bytes case with `plant` `n/a`.
6. **Byte-source exhaustion in the six-leg draw (lines 1336 and 1651).** Both lines say a short draw is `not read` and not `not defined`. They also say "That count is settled by the object's own bytes". Readings for the number of draws on a short source: restart, or keep the indices drawn so far. Implemented: record `not read` for both `six_leg` and `six_leg_runs`, and discard the indices drawn before the shortage, as line 1651 says.
7. **Hardware SHA (lines 837 and 2148, Rule I6).** Normative use of SHA instructions against a platform where the call overhead beats software. See "Possible SPEC flaw" Observation 1. Implemented the normative text (`node:crypto`).
8. **`cached_leg` (line 2078; CASES line 114).** "The vector's leg reads at least one slot twice." At S1 the CASES says the generator must check it does. Implemented the generator check (`negatives.ts` throws if the leg does not revisit a slot). The reuse of `S_folded` is read as "reuse the folded value of the earlier read" and nothing else.
9. **CBOR malformed list (line 497).** "Any byte after that one data item" and "a duplicate key" are malformed, but nested-depth limits and very large lengths are unspecified. Implemented: a nesting limit of 512 (`MAX_DEPTH` in `cbor.ts`, treated as malformed CBOR); a declared length larger than the remaining input is malformed. The SPEC states no limit, so 512 is my choice.
10. **Plant set (lines 1488 and 1490).** `plants` (array) and `plant` (object) form the set. If `plants` exists but is not an array: ignored or malformed? Implemented: if `plants` is absent or not an array, the set falls back to `plant` alone; an array element that is not an object adds no plant. A document with zero recorded plants gives `plant` `mismatch` (line 1488).
11. **MTP (line 1587).** The window reads `n = min(11, h + 1)` headers. If one of the extra headers is missing from the index, `hike_span` is not computable. Implemented: `hike_span` `not read` for that case. The SPEC text does not give the token.
12. **Exit codes (line 2174).** "Exit 1 means the object file cannot be read, or its bytes are a decode failure under §7.3." The relation to Rule A5 (unknown kind) is not stated. Implemented the order in the file: malformed CBOR gives a record (exit 0); decode failure gives exit 1; then Rule A5; then measurement.
13. **JSON duplicate names (line 614).** "Including a nested object": implemented by a strict recursive reader that refuses repeated names at any depth. It does not say whether the check applies when the repeat is inside an ignored member. Implemented: yes, the whole read fails.
14. **CASES line 9 (file format).** Lines are `key<TAB>value<LF>`, sorted by key bytes, no trailing whitespace. The generator writes exactly that. I did not find a statement about a key with an empty value; none of the cases produced one.
15. **`thorough` draw (§15.5).** I implemented the draw as a library function (`draw.ts`), but the SPEC does not define a producer-side artefact for it in the offline plane. It is not wired to the CLI.

I could not check these readings against another implementation (I was not allowed to read any). Items 1, 5, 9 and 11 are the ones I am least sure of.

## Anything unfinished

- **Later-round edge objects** named in CASES beyond O1–O3 (for example objects with per-object edge choices) are not produced. Only the candidate files and the three objects are.
- **§13 non-empty-path producer, Bari and network parts** are not implemented as a producer. The verifier reads and checks present paths and plants (tests build synthetic ones), but the offline producer only writes empty paths (skip) for stages 1–16.
- **`thorough` rounds** exist as a library function only (see question 15).
- **No `verify_record_sha256`** command or field.
- The CBOR nesting limit (512) is my choice (question 9).
- **No live-plane features:** no live index client against a real Bitcoin index beyond the HTTP client, no real beacon fetch.
- SIMD and native-intrinsics tiers were not built (language limit).
