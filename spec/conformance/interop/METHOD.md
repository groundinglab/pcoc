# Method

## Commissioning a build

- **Inputs.** Each builder gets only pinned public files: the SPEC, the policy files and the round's fixtures file (`CASES.md`), all pinned to one pcoc commit. The fixtures name the cases and the exact output keys; they carry no expected values.
- **Prompt.** One prompt per round, published verbatim in the round folder. Only the language line differs between builds.
- **Isolation.** Each build runs as an autonomous coding agent in its own new, empty repository. It may not read any other implementation, repository, branch or code about PCOC or Satoshi Hike.
- **What a build delivers.** The construction, object encode and decode, the section 14 verifier (with the section 16.1 conformance-small profile), a deterministic candidate generator for every fixture case, a test-plane producer for objects O1 (S1), O2 (S2) and O3 (production profile), and `RESULTS.md`: model, commands, machine, timings, optimisations, SPEC questions and any possible SPEC flaw.
- **Effort.** Round 1 ran at low effort, to expose ambiguity. Round 2 ran at high effort and asked each builder to optimise as far as its language allows within the SPEC, and to report any way to do less work than the SPEC intends as a possible flaw.
- **What is recorded.** Provider, model, effort, language and the source commit. Agent run identifiers and the builders' working repository names are not published.

## Building where it counts

A build can be written anywhere, by anyone or any model. Timings only count when the build is compiled from its own source and run on a fixed reference machine, so figures can be compared. Timings that builders report from their own cloud machines only show that a build works; they are not used.

Round-2 reference machines (details in [REFERENCE-SYSTEMS.md](REFERENCE-SYSTEMS.md)):

| Machine | Role | CPU |
|---|---|---|
| Reference system 1 | primary reference | AMD Ryzen 5 5600H, 6 cores, SHA-NI, AVX2 |
| Reference system 2 | the SPEC's low-end host (section 15.5) | Intel Core i7-10610U, 4 cores, no SHA-NI, AVX2 |
| Reference system 3 | second commodity laptop | AMD Ryzen 5 7430U, 6 cores, SHA-NI, AVX2 |

For each build, on each machine, in this order: build as its README says; run its own tests with its committed candidates in place; regenerate the candidates; produce O1–O3; time it (3 runs); cross-verify.

## Measurements

- **Bench.** One 256 MiB production terrain fill and one 2,000,000-step production leg, each in the build's optimised and plain scalar form. Median and minimum over 3 runs.
- **Outputs.** Every candidate file and every object is compared byte for byte across builds and against the build's committed copy.
- **Cross-verify.** Every build verifies every build's O1, O2 and O3 (6 × 6 × 3 = 108 cells per machine). A cell is "all match" when every `result`, `results[]`, `index_comparison`, `seed_value` and `shape` field in the section 14 record is `match`. Verifiers use their synthetic fixture index and the fixed draw label `pcoc-pack-2.2/draw`, except where noted.
- **Packed runs.** N independent hikes run at once on one machine, each with its own key, seed and 256 MiB terrain, and each chaining production legs from its own checkpoints. Nothing is shared and nothing is skipped. All N start together; the terrain fill and leg 0 are discarded; then a 60-second window is measured with all N running. Per-hike rate = (legs ending in the window − 1) / (time between the first and last of those leg ends). The last counted leg of every instance is recomputed by a second, independent build and must match.
- **Load.** Each machine's load during the runs is recorded in the results. A batch job on reference system 2 was paused while it was timed.

The packed figures are steady leg rates over a window. They are not yet the full beacon-paced hike rate (`hike_legs / hike_span` over a real hike) that SPEC section 15.5 calibrates on. That confirmation is open; see [calibration/](calibration/).
