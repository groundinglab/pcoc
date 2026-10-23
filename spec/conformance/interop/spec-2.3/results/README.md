# SPEC 2.3 re-check of the round-2 evidence

RS3, 23 Oct 2026. Short checks only. Round 2 was not edited. No bench figures are in this folder.

| File | What |
|---|---|
| [../ROUND.md](../ROUND.md) | SPEC commit, SPEC sha256, pack vector-sha256, SDK tags, date, machine |
| [outputs-diff.md](outputs-diff.md) | The 14 fixture outputs compared with each other and with frozen round-2 ref3 |
| [negatives.csv](negatives.csv) | Permanent-negatives output per build |
| [object-pins.csv](object-pins.csv) | SHA-256 of each committed round-2 O1–O3 against that build's `SHA256SUMS` |
| [verify-grid.csv](verify-grid.csv) | Every cross-verify cell: machine, verifier, producer, object, outcome |
| [raw/xv/](raw/xv/) | The section 14 record for every cell |

`ref3` in the tables is RS3, the same machine label as round 2.

## Headline

- **Builds.** 6 of 6 round-2 builds compiled from their committed sources. Toolchains were already present in the user directory: Zig 0.15.2, JDK 21.0.12.1, Node 22.23.4, Swift 6.1.2, .NET SDK 8.0.426. OpenSSL headers came from that same user-local tree.
- **Fixtures.** 84 of 84 regenerated files match the frozen round-2 ref3 bytes (14 files × 6 builds). Across builds, 13 of 14 files are identical. F-glm `hashcash.txt` is the same one-file exception round 2 recorded.
- **Negatives.** 13 of 13 `matches_honest` values are false on every build. Zero are match. The file blob is `bd63be129750e0cbfd0492ab6728f58fc1753332` for every build, the same blob as round 2.
- **Objects.** 18 of 18 committed files match `SHA256SUMS`: O1 `800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246`, O2 `6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0`, O3 `b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7`. Those three digests are shared by all six builds.
- **Cross-verify.** 180 of 180 cells are `all_match` (10 verifiers × 6 producers × 3 objects). Verifiers are the six rebuilt round-2 builds plus the Rust, C, Go, and Java SDKs at the 2.3.0 tags. No cell is a non-match, and none was skipped.
- **Records.** For every verifier except C-grok on O3, the canonical record sha256 prefix is the round-2 value: O1 `256f53ca0960` (68 fields), O2 `5cdba0438ef6` (104 fields), O3 `ae5f65050a23` (68 fields).
- **SDK harness.** `sdk/conformance/cross-verify.sh --tier pr` passed for all four SDKs: 72 of 72 cells match, 0 pending, 48 verifier invocations. Object pins, the PROD leg hex, the candidate files, the thirteen negatives, the six-leg negative cells, and the malformed records all passed.

## C-grok on O3

C-grok's production verifier does not take the rig draw label. `kit/xverify.sh` omits `--draw-label` on O3 because that CLI refuses the label outside conformance-small, and the six indices are a fresh draw on each invocation. Every `result` in those records is `match`. The six C-grok O3 cells were measured separately, so their record prefixes differ from each other and from `ae5f65050a23`. Round 2 shows the same pattern. The other nine verifiers, including the four 2.3.0 SDKs, use the rig label and share `ae5f65050a23`.

Each of the three object byte strings is shared by all six builds. Nine verifiers measured each byte string once, and the cell for every producer is that measurement. C-grok measured O1 once, O2 once, and O3 once per producer.

## Builds and tests

| Build | Build | Own tests | Candidates |
|---|---|---|---|
| A-sonnet | pass | 38/38 pass | 14/14 match round 2 |
| B-sol | pass | 7/7 pass | 14/14 match round 2 |
| C-grok | pass | 13/13 pass | 14/14 match round 2 |
| D-gemini | pass | 6/6 pass | 14/14 match round 2 |
| E-kimi | pass | full run 33/35; without the two AVX-512-direct tests, 33/33 | 14/14 match round 2 |
| F-glm | pass | full run hit SIGILL; with that one test skipped, 19/19 | 14/14 match round 2 |

RS3 has no AVX-512. E-kimi's `EngineEquivalence.TerrainIdenticalAcrossEngines` and `FoldIdenticalAcrossEngines` call the AVX-512 engine directly and throw `PlatformNotSupportedException`. F-glm's `prodTerrainScalarEqualsAvx512` calls that kernel directly and is killed with SIGILL. Both are the same results round 2 recorded on RS3. The kit's skip for F-glm, and a filter that drops those two E-kimi tests, then pass. Dispatch on the producer and verifier paths was not what failed.

A-sonnet's first test run failed one stderr assertion: the job environment had set both `NO_COLOR` and `FORCE_COLOR`, and Node wrote a warning. A rerun with those unset passed 38/38. The candidate files came from the candidate phase, which had already passed, and they match round 2.

B-sol candidates were produced with `generate-candidates`. The kit's `cands` phase runs `generate-all`, which also writes a new O3. This run does not produce a new hike. The fourteen fixture files are the same ones `generate-all` writes, and they match round 2.

## SDK harness against the pack

The harness is `sdk/conformance/cross-verify.sh --tier pr`, at the 2.3.0 tags (the SDK tree at that commit matches this checkout for `sdk/` and the pack). It builds the four verify CLIs and the Rust, C, and Go index drivers, then checks them against the committed SDK objects and candidates, which are the pack vectors pinned by `sdk/SPEC_PIN`.

The `pr` tier does not recompute a production leg. It checks the O3 digest and the PROD leg hex in the candidate file. Section 14 measurement of the committed round-2 O3 is in the grid above, including the four SDK drivers, the same way the harness measures O1 and O2: loopback index for Rust, C, and Go, and Java's `--index` directory of the same bodies.

## Not run

- Bench phases in `kit/run-build.sh`.
- Packed ladders and any new hike object (the `objects` phase, and B-sol `generate-all`).
- The harness `--tier full`, which recomputes a production leg. O3 in this folder is verification of the committed objects.
