# Satoshi Hike 2.2 — Kotlin/JVM Implementation

Independent implementation of **Satoshi Hike SPEC 2.2 (with erratum 1)** in Kotlin on the JVM (OpenJDK 21).

## Repository Layout

- `CASES.md` — Copy of the round 2 interop fixture specification.
- `RESULTS.md` — Detailed interoperability report, SPEC flaw analysis, SPEC questions, machine details, optimisations, and benchmark timings.
- `candidates/` — All 14 deterministic candidate output files (`seed.txt`, `nl4.txt`, `terrain.txt`, `fold.txt`, `kat-small.txt`, `test-3leg.txt`, `prod.txt`, `wes.txt`, `rate.txt`, `anchor.txt`, `draw.txt`, `unknown-kind.txt`, `hashcash.txt`, `negatives.txt`).
- `objects/` — Deterministically encoded test-plane hike objects (`O1-S1.cbor`, `O2-S2.cbor`, `O3-PROD.cbor`).
- `impl/` — Kotlin/Gradle project:
  - `impl/src/main/kotlin/satoshihike/` — Core construction (§6–§12), deterministic CBOR codec (§7–§8), producer (§13), Bari paid rule (§13.6), verifier (§14, §16.1), and CLI (§18).
  - `impl/negatives/` — Isolated `negatives` generator module (`negatives` package) containing the 13 permanent negatives and the candidate generator.
  - `impl/src/test/kotlin/satoshihike/` — Conformance and regression test suite.
  - `impl/verify` (and `./verify`) — Section 18 verifier CLI executable wrapper.

## Commands

```bash
# Run all tests (including re-running the candidate generator and checking committed files)
./impl/gradlew -p impl test

# Regenerate all candidate files in candidates/
./impl/gradlew -p impl :negatives:genCandidates

# Regenerate O1-S1.cbor, O2-S2.cbor, and O3-PROD.cbor in objects/
./impl/gradlew -p impl genObjects

# Verify objects (prints the Section 14 JSON verify record)
./impl/verify --profile conformance-small --slot-bytes 64 --sample-bytes 32 --terrain-bytes 65536 --steps-per-leg 100 objects/O1-S1.cbor
./impl/verify --profile conformance-small --slot-bytes 256 --sample-bytes 64 --terrain-bytes 1048576 --steps-per-leg 1000 objects/O2-S2.cbor
./impl/verify objects/O3-PROD.cbor

# Run scalar vs optimised benchmarks
./impl/gradlew -p impl benchmark
```
