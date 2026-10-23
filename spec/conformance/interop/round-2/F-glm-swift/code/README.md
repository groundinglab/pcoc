# Satoshi Hike 2.2 — independent Swift implementation

An independent, optimised implementation of **Satoshi Hike SPEC 2.2 (with
erratum 1)** in Swift (with C kernels for SHA-NI/AVX-512), built only from the
three pinned public files kept under [`refs/`](refs/):

- `refs/SPEC.md` — the SPEC at commit `7b40353e…` (sha256
  `8e591ea02e7c73ac3d1a4e61602f25baaae88cabdc482240d310d2629dfa5889`, verified)
- `refs/POLICY.md`, `refs/gl-admit-v9.toml` — the admission policy at the same
  commit
- `refs/CASES.md` — the round-2 interop fixtures and exact output keys

This is one core of the public Satoshi Hike interoperability test. The
evidence for the test is [`RESULTS.md`](RESULTS.md).

## Layout

```
refs/            the three pinned input files (SPEC, policy, CASES)
impl/            the implementation
  Sources/CSatoshiKernels/   C kernels: SHA-256 (SHA-NI + scalar), slot fold
                             (AVX-512 / AVX-2 / scalar), terrain fill, leg driver
  Sources/SatoshiHike/       the SPEC construction (sections 6–12), CBOR (7.3),
                             object (8), verify record (14), draw (14.6/15.5),
                             hashcash (13.1), fixture index, policy (15.4)
  Sources/Negatives/         the section 16 permanent negatives — a generator-only
                             module; never linked into the SDK paths
  Sources/GenCandidatesCore/ the deterministic candidate generator
  Sources/GenCandidatesCLI/  `gen-candidates` (the only executable that links Negatives)
  Sources/SHikeCLI/          `shike verify|produce|bench|policy-check`
  Tests/                     differential and conformance tests
  candidates/                the 14 CASES.md files, committed
  objects/                   O1-S1.cbor, O2-S2.cbor, O3-PROD.cbor, committed
RESULTS.md       the required build report (evidence for the interop matrix)
```

## Commands

Swift 6.1.2 on Linux (toolchain from swift.org; no external package
dependencies — everything is local and offline):

```sh
cd impl
swift build -c release

# the deterministic candidate generator (CASES.md output format)
swift run -c release GenCandidatesCLI          # writes impl/candidates/

# the offline test-plane producer (O1 S1, O2 S2, O3 production profile)
swift run -c release SHikeCLI produce --out objects

# the section 14 verifier
swift run -c release SHikeCLI verify objects/O1-S1.cbor \
    --profile conformance-small \
    --sample-bytes 32 --slot-bytes 64 --terrain-bytes 65536 --steps-per-leg 100 \
    --draw-source chain:pcoc-pack-2.2/draw
swift run -c release SHikeCLI verify objects/O2-S2.cbor \
    --profile conformance-small \
    --sample-bytes 64 --slot-bytes 256 --terrain-bytes 1048576 --steps-per-leg 1000 \
    --draw-source chain:pcoc-pack-2.2/draw
swift run -c release SHikeCLI verify objects/O3-PROD.cbor \
    --profile production --draw-source chain:pcoc-pack-2.2/draw

# tests (re-run the generator and check the committed candidates are unchanged)
swift test -c release

# benchmarks for RESULTS.md
swift run -c release SHikeCLI bench

# section 15.4 policy file validation
swift run -c release SHikeCLI policy-check ../refs/gl-admit-v9.toml
```

Exit codes follow section 18: `0` a record was produced (including a
malformed-CBOR record), `1` unreadable file or a section 7.3 decode failure,
`2` usage error (including `--profile conformance-small` without all four
integers).
