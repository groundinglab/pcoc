# Satoshi Hike 2.2 interoperability results

## Possible SPEC flaw

The thorough verifier is probabilistic, so a producer can omit complete legs
and still sometimes receive an all-`match` section 14 record.

For an omitted leg, the producer chooses an arbitrary `u64` leg result, then
computes the ordinary checkpoint from that value and continues the checkpoint
and waypoint chains. Every cheap check is valid. If none of the omitted legs
is among the `D = 6 * thorough_rounds` indices drawn after commitment, all
recomputed legs match and the verifier cannot distinguish this object from one
that performed every leg.

One omitted production leg saves exactly:

- 2,000,000 full 16 KiB slot loads (32,768,000,000 bytes, about 30.52 GiB);
- 4,096,000,000 `mix64` evaluations; and
- 2,000,000 seal SHA-256 evaluations.

It still costs one checkpoint hash and the normal downstream work. For `F`
omitted legs among `L`, with `L >= 18` and the default 18 distinct draws, the
probability of escaping detection is
`C(L-F, 18) / C(L, 18)`. At a 25% omitted fraction this approaches
`0.75^18 = 0.5636%`, the complement of the SPEC's stated 99.44% detection
figure. With one omitted leg among 54, it is exactly 2/3.

This implementation does not use this shortcut. Every generated leg performs
every specified slot load and mix. The issue is reported because the task's
criterion says a verifier that sometimes cannot tell skipped work is a
possible SPEC flaw, even though §15.5 explicitly documents probabilistic
detection.

## Provenance

- Model: GPT-5.6 Sol.
- Execution mode: autonomous coding agent. A separate numeric “effort” setting
  was not exposed.
- Date: 22 October 2026.
- Language: Python 3.12.3 with one in-repo C native library loaded through
  `ctypes`.
- Native dependency: the system OpenSSL 3.0.13 `libcrypto.so.3` SHA-256 ABI.
- NumPy and third-party Python packages: not used.
- Public material read: only the four URLs in the task. Copies are committed
  under `public-inputs/` and
  `spec/conformance/interop/round2/CASES.md`.
- SPEC SHA-256:
  `8e591ea02e7c73ac3d1a4e61602f25baaae88cabdc482240d310d2629dfa5889`
  (matches the task).

## What was built

- §10 NL4/dag2 terrain, seed, lanes, parents, and `mix64`.
- §11 full-slot fold, seal, leg, terrain sample, and checkpoint.
- §12 stages, checkpoint fold, and waypoint.
- §7.3 strict definite-length CBOR decoder and deterministic encoder.
- §8 hike object model.
- §14 full verifier record, including unknown kind, shape, portable-index
  headers, paths/plants, bookends, MTP analysis, and six-leg recomputation.
- §16.1 arbitrary valid `conformance-small` profiles and injected draw bytes.
- §15.5 thorough draw and Rule B2/anchor integer comparisons.
- §13.1 Hashcash challenge, digest, leading-zero count, and solver.
- Deterministic generators for every `CASES.md` key, with all permanent
  negatives isolated in `impl/satoshi_hike/generators/negatives.py`.
- Offline O1/S1, O2/S2, and O3/production objects with one leg per stage in
  O3.
- Section 18-style CLI and offline fixture index.

Artifact hashes:

| Artifact | SHA-256 |
|---|---|
| `objects/O1-S1.cbor` | `800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246` |
| `objects/O2-S2.cbor` | `6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0` |
| `objects/O3-PROD.cbor` | `b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7` |

## Exact commands

```sh
sha256sum public-inputs/SPEC.md public-inputs/POLICY.md \
  public-inputs/gl-admit-v9.toml \
  spec/conformance/interop/round2/CASES.md
make native
python3 -m impl.satoshi_hike generate-all
make test
python3 -m impl.satoshi_hike verify objects/O1-S1.cbor \
  --profile conformance-small \
  --slot-bytes 64 --sample-bytes 32 --terrain-bytes 65536 \
  --steps-per-leg 100 \
  --fixture-index --draw-label pcoc-pack-2.2/draw
python3 -m impl.satoshi_hike verify objects/O3-PROD.cbor \
  --fixture-index --draw-label pcoc-pack-2.2/draw
python3 -m impl.satoshi_hike benchmark
```

The complete O1 record had shape, all 19 headers, every checkpoint, all 17
waypoints, and all six drawn legs `match`. Its MTP `hike_span` was 10,800
seconds. The complete production O3 record had the same all-`match` results;
its six indices were `7,2,1,15,10,16`. Tests regenerated all 14 candidate
files and compared their bytes with the committed files.

## Machine

- OS: Linux 6.12.94+, x86-64, glibc 2.39.
- CPU: Intel Xeon Processor, family 6, model 207, stepping 2.
- Allocated CPUs: 4, one thread per core.
- Cache reported by `lscpu`: 192 KiB L1d, 128 KiB L1i, 8 MiB L2, 320 MiB L3.
- Relevant flags: `avx2`, `avx512f`, `avx512dq`, `avx512bw`, `avx512vl`,
  `avx512ifma`, `avx_vnni`, `sha_ni`, `bmi1`, `bmi2`, `fma`, `aes`.
- Compiler: Ubuntu Clang 18.1.3.
- Build flags: `-O3 -DNDEBUG -fPIC -Wall -Wextra -Wpedantic -march=native`.
- SHA-256: OpenSSL 3.0.13; this host advertises SHA-NI and OpenSSL dispatches
  its SHA-256 implementation to hardware instructions.

## Timings

Each reported number is the median of three isolated runs in one invocation.
The scalar loops explicitly disable compiler vectorization; both variants are
native C and use the same OpenSSL SHA-256.

| Production operation | Scalar | Optimized | Gain |
|---|---:|---:|---:|
| 256 MiB terrain fill | 0.6082 s | 0.2785 s | 2.18× (54.2% less time) |
| One 2,000,000-step leg | 4.5152 s | 2.7433 s | 1.65× (39.2% less time) |

Both paths produced terrain SHA-256
`60b1f689c39be2689487b680cca7ae9d58ec47a286f864c5eb05bc07fc6ffc04`
and leg integer `a7b8acbec54aa088`.

## Optimizations

1. The hot construction is native C, eliminating billions of Python integer
   allocations and calls. Python retains orchestration, codec, records, and
   deterministic generation.
2. The optimized path evaluates eight independent `mix64` lanes with
   AVX-512DQ. Terrain words and each step's full slot are contiguous.
3. The step accumulates eight wrapping-add lanes and reduces them after the
   full slot. Addition modulo `2^64` is associative, so this changes no byte.
   Every word is still loaded and mixed with the current `S`.
4. Terrain storage is one contiguous mutable 256 MiB byte array in exact
   little-endian wire layout. There is no transformed terrain, digest cache,
   slot summary, partial read, or cross-PCOC reuse.
5. OpenSSL provides hardware-dispatched SHA-256 for seed, seal, checkpoint,
   fold, waypoint, and artifact hashing.
6. A terrain is filled once and shared by the legs of that one PCOC, exactly
   as §11.1 requires. Verifier checks build one fresh terrain per object.

The per-optimization gains are not independently additive: native C is
required for both measured paths, while the table isolates explicit SIMD
against scalar C.

## Language advantage and disadvantage

Python was an advantage for the strict CBOR/JSON record construction,
deterministic generators, arbitrary-size draw arithmetic, fixture-index
injection, and tests. The code directly mirrors the SPEC's byte-level
structures and keeps negative constructions out of production paths.

Python was a severe disadvantage for the mandated production loop: one leg
does 4.096 billion `mix64` calls. A pure-Python implementation would not be a
practical producer. The in-repo C library is therefore part of this Python
implementation. That adds a compiler and `libcrypto` ABI dependency, and the
AVX-512 build is host-specific; the scalar path remains available on hosts
without those vector features.

## SPEC questions and readings

1. **Source pin wording (task, not a SPEC contradiction).** The task says all
   files are pinned to one commit, but the supplied SPEC/policy URLs pin
   `7b40353ed68cbdc7319f06d09f6ed178e7f7809e` while `CASES.md` pins
   `8c40cf855241d6fbfdb508a2a2c1727342df0755`. Readings considered: reject the
   source set as not one commit, or use every exact URL supplied. Implemented:
   use the exact four URLs and record both supplied pins.
2. **Erratum 1.** No separate erratum file was supplied. Readings considered:
   search for another document (forbidden by the source rule), or treat the
   pinned SPEC identified by `CASES.md` as “SPEC 2.2 with erratum 1.”
   Implemented: the latter; no other source was fetched.
3. **§16.1 profile integers and §18 CLI.** Readings considered: infer S1/S2
   from object size fields, or require the four integers as verifier inputs.
   Implemented: require all four for `conformance-small`, because §§14, 16.1,
   and 18 explicitly do. Omitted `--profile` means production.
4. **§14 portable index for literal `verify <object>`.** Readings considered:
   silently use the CASES fixture, or leave network measurements unread unless
   an index is selected. Implemented: default to `not read`; `--fixture-index`
   explicitly selects the CASES index and `--index-dir` supplies offline
   documents.
5. **CASES output location.** CASES gives the comparative layout
   `sdk/<lang>/candidates/`, while the task asks for this standalone build
   under `impl/` and names `candidates/` and `objects/`. Implemented: code under
   `impl/`, generated interoperability artifacts at repository-root
   `candidates/` and `objects/`.
6. **O3 versus `prod.txt`.** CASES asks `prod.txt` for only stage 0's first
   leg, but separately requires O3 with one leg in every stage. Implemented:
   three keys in `prod.txt`, and all 18 production legs in O3.

No construction sentence required a silent guess beyond these interface and
source-layout readings.

## Incomplete or out of scope

Nothing in the requested six build deliverables is intentionally omitted.
The implementation is an offline test-plane producer; it does not implement
the live gossip/Bari asynchronous service, the Bari operator, or the §15
admission/sign-in service, which were not requested for this interoperability
core.
