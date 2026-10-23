# RESULTS

Model: Grok 4.7. No separate effort tier was exposed for this run.

## Possible SPEC flaw

None found that still produces honest bytes.

SPEC §16 names thirteen constructions that would do less work than §11.1 if they computed the same leg or the same terrain: a per-slot summary, an XOR accumulator, a `word XOR w` term, a prefold of that term, a hoist of `mix64`'s last line, a read of word 0 only, a read that drops the last word, one lane, a linear mix of the four lanes, another public key's terrain, a cached `S_folded`, and a seed squeezed to one lane. This build computes each one in `impl/src/negatives.zig` and compares it with the honest S1 `pk_A` value. All thirteen differ (`matches_honest` is `false` on every row). The generator returns an error if any negative equals honest, and it checks that the honest S1 leg revisits a slot before it emits `cached_leg`.

What those negatives would have saved, had they matched, is the per-step load. A summary or a prefold replaces the 16 KiB slot with 8 bytes. A partial read touches 8 bytes, or all but one word. A cached fold reuses an earlier visit. None of them is the §11.1 sum, so none of them is shipped on the produce or verify path.

The fast engine does the same work on a wider register. Every selected slot is loaded in full, and every word is mixed with the current `S` by §10.1, including `MIX_ADD`. Parents and `slot_mix` are unchanged. SHA-256 is the same FIPS function. The bytes match the scalar engine (`terrain_equal=true`, `leg_equal=true` in `hike bench`).

§14.3's cheap checkpoint hashes the stored leg result and does not rerun the leg. That is the rule as written. The §14.6 draw then recomputes six legs from §11.2. On O1, O2, and O3 those recomputed legs recorded `match`. A forged leg fails that recompute when the draw selects it. The sample is the one §14.6 specifies.

## What was built

Zig 0.15.2 implementation of Satoshi Hike SPEC 2.2 (the pinned file, which includes erratum 1), under `impl/`.

- Construction §§6–12: seed, NL4 dag2 fill, leg, checkpoint, waypoint.
- Object encode and decode, §7.3 and §8.
- Section 14 verifier, including Rule A5 and the §16.1 `conformance-small` profile. With no `--index-url` and no `--no-index`, header reads use the synthetic index defined in `CASES.md` (heights 989…1018).
- Candidate generator (`gen-candidates`) writing the 14 CASES files. Negatives live only in `impl/src/negatives.zig`. The SDK root does not import that file.
- Offline producer (`hike produce-fixtures`) writing `objects/O1-S1.cbor`, `objects/O2-S2.cbor`, and `objects/O3-PROD.cbor`, and the §13.3 stderr step.
- CLI `hike verify <object>` printing one section 14 record and a trailing newline.
- `hike bench` timing a scalar loop and the fast engine.

Pins, checked in this tree:

| File | sha256 |
|---|---|
| `spec/satoshi-hike-v2.2/SPEC.md` | `8e591ea02e7c73ac3d1a4e61602f25baaae88cabdc482240d310d2629dfa5889` |
| `policy/groundinglab-admission/POLICY.md` | `7b8ca9491b8eadc2d8307a401d777cc884baf3c06ae60dc845cb3e403572cf71` |
| `policy/groundinglab-admission/gl-admit-v9.toml` | `9182ea9107b9587a2f95c855cf1b4d490c1f3eaecd1061e7c4d80d90521c8deb` |
| `spec/conformance/interop/round2/CASES.md` | `0e6de698e468138ba6503b55a876cee94a24cc84bdc649adfa5af643db5a5290` |

Part A does not read lever values. The rate grid and `thorough_rounds = 3` come from CASES.

## Commands

```sh
cd impl
zig build --release=fast
zig build test --release=fast
./zig-out/bin/gen-candidates --out ../candidates
./zig-out/bin/hike produce-fixtures --out ../objects
./zig-out/bin/hike bench
./zig-out/bin/hike verify ../objects/O1-S1.cbor \
  --profile conformance-small \
  --sample-bytes 32 --slot-bytes 64 --terrain-bytes 65536 --steps-per-leg 100 \
  --draw-label pcoc-pack-2.2/draw
./zig-out/bin/hike verify ../objects/O2-S2.cbor \
  --profile conformance-small \
  --sample-bytes 64 --slot-bytes 256 --terrain-bytes 1048576 --steps-per-leg 1000 \
  --draw-label pcoc-pack-2.2/draw
./zig-out/bin/hike verify ../objects/O3-PROD.cbor --no-index
```

`zig build test` re-runs the generator into a temp directory and checks the committed `candidates/` byte for byte. That test passed. `zig test src/root.zig -OReleaseFast` passed 12 tests, including the §7.4 SHA-256 and seal pins, scalar/fast agreement on the S1 terrain, and the O1 record (`hike_span` 10800, `whole_hike_span` 10200, `hike_legs` 18, six recomputed legs `match`).

## Machine

- Linux 6.12.94+ x86_64
- `lscpu` model name: `Intel(R) Xeon(R) Processor` (hypervisor string). CPUID family 6, model 207, stepping 2 (Emerald Rapids). 4 cores. BogoMIPS 4800.
- Flags present: `avx2`, `avx512f`, `avx512dq`, `sha_ni` (also `avx512bw`, `avx512vl`, and the rest of the Emerald Rapids set).
- Zig 0.15.2, `zig build --release=fast` (ReleaseFast, native CPU). The binary contains `sha256rnds2` and `vpmullq` on `zmm`.

## Timings

`hike bench`, one production terrain (256 MiB, `pk_A`, the CASES seed beacon) and one production leg (2_000_000 steps). Three runs after the final build:

| Run | Terrain scalar | Terrain fast | Leg scalar | Leg fast |
|---|---:|---:|---:|---:|
| 1 | 0.394 s | 0.274 s | 2.805 s | 2.453 s |
| 2 | 0.398 s | 0.286 s | 2.762 s | 1.840 s |
| 3 | 0.301 s | 0.236 s | 2.843 s | 2.070 s |

Both engines wrote the same terrain and the same leg. The fast leg's little-endian wire hex is `88a04ac5beacb8a7`, the same value as `prod.PROD.stage.0.leg.0.legs_le_hex`.

`hike produce-fixtures` took 43 s, almost all of it O3 (18 production legs). `hike verify` on O3 with `--no-index` took 15.8 s (terrain fill plus six production legs).

Object SHA-256 (byte 0 first):

| Object | SHA-256 |
|---|---|
| `objects/O1-S1.cbor` | `800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246` |
| `objects/O2-S2.cbor` | `6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0` |
| `objects/O3-PROD.cbor` | `b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7` |

Each produce wrote `path_skip 1` … `path_skip 16` and then `not fetched by design 17`.

O1 verified under `conformance-small` against the fixture index: `shape` `match`, `seed_value` `match`, every checkpoint and every `W_1`…`W_17` `match`, `hike_legs` 18, `hike_span` 10800, `whole_hike_legs` 17, `whole_hike_span` 10200, anchors all `not anchored`, `anchored_total` 0, bookends `not defined`. O2: `shape` `match`, `hike_legs` 54, the six drawn legs `match`. O3 with `--no-index`: `shape` `match`, checkpoints and waypoints `match`, six drawn production legs `match`, header `T` and `hike_span` `not read`.

## Optimisations

1. Terrain is an array of native little-endian `u64` words. The digest hashes those bytes, which are `le64` on this host. Slot loads in the fold are plain vector loads.
2. `mix64`, the NL4 base word, parent mixing, and `slot_mix` run eight words at a time with `@Vector(8, u64)`. The slot fold keeps 32 words in flight (four independent chains) and then adds them. Every word is still mixed with `S`.
3. The fast SHA-256 is `std.crypto.hash.sha2.Sha256`, which emits SHA-NI when the target has `sha` and `avx2`. The §11.1 seal is a 20-byte one-shot, and the leg loop is specialised on the engine at compile time so that call inlines. The scalar engine uses a direct FIPS 180-4 compressor, including a one-block seal.
4. The production allocation is 2 MiB aligned and is advised `MADV_HUGEPAGE`. After the process exited, `AnonHugePages` was 0, so this run did not observe a promotion.

The gain on the terrain fill is small because LLVM already autovectorises the plain loop (about 0.30 s scalar versus 0.24 s fast on the warm run). The gain on the leg is about 2.8 s scalar versus 2.1 s fast. A production leg reads 2_000_000 × 16 KiB = 32 GiB. That traffic, plus one SHA-256 per step on the critical path to the next slot index, is the bulk of the 2 seconds. The explicit unroll and SHA-NI are the part of that time that is arithmetic.

## Language

Zig was an advantage for the bytes and for the fast path. `@Vector` lowers to AVX-512 (`vpmullq`, `vpaddq` on `zmm`) with no intrinsic layer. `comptime` specialises the leg so the hot loop does not switch on an engine enum. The standard library's SHA-256 already contains `sha256rnds2`, so the conforming path uses the host instructions without a C library.

Zig was a disadvantage for the scalar comparison. There is no loop hint in this compiler that turns autovectorisation off for one function, so the "scalar" column is the plain source loop after LLVM. ReleaseFast still shortens it. Zig 0.15 also refuses to compile one source file into two modules, so `negatives.zig` imports the SDK module instead of importing `terrain.zig` itself. `alignedAlloc`'s alignment is a comptime parameter, so the 2 MiB path is a separate call.

## SPEC questions

1. SPEC §3 line 270, and Rule B2 at §15 line 1974. Reading A: `q · hike_legs` and `p · hike_span` are mathematical integers. Section 3 limits `mod 2^64` to the NL4 word, the parent indices, the slot mix, and the step state, and Rule B2 says the comparison does not round. Reading B: every product in the file is a wrapping `u64`. Implemented A, with `u128`. On the CASES grid the products fit in a `u64`, so both readings write the same `rate.txt`.

2. SPEC §10.3 lines 732–738, `reverse32`. Reading A: reverse the low 32 bits; the sentence says high bits are zero when `v < 2^32`. Reading B: reverse a full `u64`. Every legal slot index fits in 32 bits. Implemented A (`@bitReverse` of a `u32`).

3. SPEC §14.10 lines 1587–1588, median-time-past. `n // 2` is integer division, index 5 when `n` is 11. Implemented that. The fixture clock then gives `hike_span` 10800 (`600 * 18`) and `whole_hike_span` 10200 (`600 * 17`). The O1 record has those integers.

4. SPEC §7.3 lines 496–497, decode order versus Rule A5. Reading, in the order the paragraph gives: a malformed CBOR value is a section 14 record whose `shape_item` is exactly `["malformed cbor"]`, and Rule A5 is not applied. One well-formed item that is not a map, or a map with no `kind` text, is a decode failure and the CLI exits 1. A map whose `kind` text is not `KIND` is the Rule A5 record. Implemented that way. The empty map exits 1. Empty input records `malformed cbor`.

5. CASES "Value spellings" for `_u64` keys, against SPEC §7.4 lines 507–509. CASES asks for 16 lowercase hex digits, the big-endian spelling of the integer. SPEC §7.4 says the digest-byte spelling `4816bdaf23240476` is not the spelling of the integer `S'`. Implemented the CASES key spelling. The §7.4 integer for `S_folded = 0` is checked in a unit test as `0x76042423afbd1648`.

6. SPEC §13.2 against §16.1 "Legs" (line 2108) and CASES "Fixed inputs". The live producer Observes a tip. The fixture fixes beacons, heights, and `legs_per_stage`, and it stores empty paths. Implemented the §13.2 drive with that rig schedule: exactly `legs_per_stage` legs, then the next CASES interrupt, empty path bytes, and the §13.3 stderr step at the close.

## Deviations from CASES

CASES asks for `sdk/<lang>/candidates/`. The task for this repo says `candidates/`. The files are at `candidates/` in the repo root. The keys inside them are the CASES templates.

CASES lists Bari paid-rule vectors, sign-in vectors, non-empty paths, and the erratum-1 shape objects as later phases. They are not generated here. `hashcash.br1` implements Rule BR1's three-block check for a caller that already has both 80-byte headers and the plane's `nBits`. This round has no fixture that pins it.

## Candidates

14 files, 373 lines, sorted by key. Counts: seed 12, nl4 48, terrain 3, fold 17, kat-small 55, test-3leg 127, prod 3, wes 3, rate 45, anchor 7, draw 8, unknown-kind 8, hashcash 9, negatives 28.
