# Round 2 same-box comparison: Satoshi Hike SPEC 2.2 + erratum 1

Every number below was measured on these machines from each build's own source at its round-2 HEAD. Cloud-VM figures in the builds' RESULTS.md are not used.
Times are seconds, shown as median / min over 3 runs (or more samples where the build's own bench takes more), one 256 MiB production terrain fill and one 2,000,000-step production leg.

Builds: A Sonnet 5.5 TypeScript `b3d7b02`, B Sol Python+C `b18a5f6`, C Grok Zig `ffcfc22`, D Gemini Kotlin `fd24932`, E Kimi C#/.NET 8 `d9fa6ea`, F GLM Swift `6d976f4`.

## Reference system 1 (PRIMARY timing reference)

AMD Ryzen 5 5600H with Radeon Graphics; 6 cores / 12 threads; L3 16 MiB (1 instance); RAM 15.0 GiB; flags: avx2 sha_ni (no AVX-512)

| Build | Language | Toolchain | Fill opt | Fill scalar | Leg opt | Leg scalar | Own tests | Notes |
|---|---|---|---|---|---|---|---|---|
| A Sonnet | TypeScript/Node 22 (+WASM) | v22.23.4 | 0.42 / 0.42 | 10.52 / 10.47 | 6.32 / 6.30 | 139.44 / 139.02 | 38 pass / 0 fail | 1 thread; opt leg = WASM fold + node:crypto SHA; WASM fold + WASM SHA leg 5.00 / 4.98 |
| B Sol | Python + C ext | gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; Python 3.12.3; CFLAGS=-O3 -DNDEBUG -fPIC -Wall -Wextra -Wpedantic -march=native | 0.61 / 0.60 | 0.61 / 0.60 | 5.74 / 5.73 | 5.73 / 5.73 | 7 run, OK | backend=scalar+openssl; 1 thread |
| C Grok | Zig 0.15.2 | 0.15.2 | 0.24 / 0.24 | 0.35 / 0.35 | 2.47 / 2.47 | 2.51 / 2.50 | 13/13 pass | 1 thread; fast=@Vector path |
| D Gemini | Kotlin/JVM 21 | openjdk version "21.0.12.1"; Gradle 8.10.2 | 0.15 / 0.15 | 0.54 / 0.54 | 2.75 / 2.74 | 6.11 / 6.02 | 6/6 pass | each run = its own min of 2-3 reps; opt fill uses 4 threads; Vector-API-512 path emulated on AVX2 (not used) |
| E Kimi | C#/.NET 8 | 8.0.426 | 0.42 / 0.42 | 0.68 / 0.68 | 3.94 / 3.89 | 6.22 / 6.22 | full run: 33/35 pass (2 EngineEquivalence tests call the AVX-512 engine directly: PlatformNotSupportedException); with those 2 excluded: 33/33 pass | 1 thread; opt = Avx2 engine; seal backend=libcrypto |
| F GLM | Swift 6.1.2 + C kernels | Swift version 6.1.2 | 0.44 / 0.43 | 0.43 / 0.43 | 3.17 / 3.17 | 4.85 / 4.83 | full run: SIGILL (prodTerrainScalarEqualsAvx512 calls AVX-512 kernel directly); with that test skipped: 19/19 pass | external C harness on its compiled kernels (own bench SIGILLs w/o AVX-512); opt fill = dispatch (scalar w/o AVX-512); opt leg = AVX2 fold |

**Candidates** (generated on this machine; `=` same bytes as the build's committed file and as every other build, else the file differs):

| file | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| anchor.txt | = | = | = | = | = | = |
| draw.txt | = | = | = | = | = | = |
| fold.txt | = | = | = | = | = | = |
| hashcash.txt | = | = | = | = | = | DIFF vs others |
| kat-small.txt | = | = | = | = | = | = |
| negatives.txt | = | = | = | = | = | = |
| nl4.txt | = | = | = | = | = | = |
| prod.txt | = | = | = | = | = | = |
| rate.txt | = | = | = | = | = | = |
| seed.txt | = | = | = | = | = | = |
| terrain.txt | = | = | = | = | = | = |
| test-3leg.txt | = | = | = | = | = | = |
| unknown-kind.txt | = | = | = | = | = | = |
| wes.txt | = | = | = | = | = | = |

**Objects** (sha256 of objects produced on this machine; prefix shown):

| build | O1-S1 | O2-S2 | O3-PROD | same as committed |
|---|---|---|---|---|
| A Sonnet | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| B Sol | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| C Grok | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| D Gemini | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| E Kimi | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| F GLM | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |

**Cross-verify O1** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| B Sol | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| C Grok | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| D Gemini | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| E Kimi | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| F GLM | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |

Distinct verify records for O1: 1 (all byte-identical after canonical JSON)

**Cross-verify O2** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| B Sol | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| C Grok | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| D Gemini | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| E Kimi | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| F GLM | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |

Distinct verify records for O2: 1 (all byte-identical after canonical JSON)

**Cross-verify O3** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| B Sol | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| C Grok | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| D Gemini | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| E Kimi | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| F GLM | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |

Distinct verify records for O3: 7 — differences are only Grok's six-leg indices: its CLI refuses `--draw-label` on the production profile and draws from the OS CSPRNG, so its 6 indices are random; all results match.

## Reference system 2 (low-end host per SPEC §15.5; i7-10610U, no SHA-NI; batch job ~40% of one thread during bench runs)

Intel(R) Core(TM) i7-10610U CPU @ 1.80GHz; 4 cores / 8 threads; L3 8 MiB (1 instance); RAM 31.2 GiB; flags: avx2 (no AVX-512)

| Build | Language | Toolchain | Fill opt | Fill scalar | Leg opt | Leg scalar | Own tests | Notes |
|---|---|---|---|---|---|---|---|---|
| A Sonnet | TypeScript/Node 22 (+WASM) | v22.23.4 | 0.38 / 0.36 | 13.94 / 12.53 | 9.47 / 9.43 | 183.77 / 183.52 | 38 pass / 0 fail | 1 thread; opt leg = WASM fold + node:crypto SHA; WASM fold + WASM SHA leg 7.06 / 7.02 |
| B Sol | Python + C ext | gcc (Debian 12.2.0-14+deb12u1) 12.2.0; Python 3.11.2; CFLAGS=-O3 -DNDEBUG -fPIC -Wall -Wextra -Wpedantic -march=native | 0.57 / 0.56 | 0.57 / 0.56 | 7.92 / 7.91 | 7.91 / 7.91 | 7 run, OK | backend=scalar+openssl; 1 thread |
| C Grok | Zig 0.15.2 | 0.15.2 | 0.29 / 0.27 | 0.40 / 0.38 | 7.59 / 6.99 | 4.21 / 4.20 | 13/13 pass | 1 thread; fast=@Vector path |
| D Gemini | Kotlin/JVM 21 | openjdk version "21.0.12.1"; Gradle 8.10.2 | 0.18 / 0.18 | 0.66 / 0.65 | 5.16 / 5.03 | 9.52 / 8.97 | 6/6 pass | each run = its own min of 2-3 reps; opt fill uses 4 threads; Vector-API-512 path emulated on AVX2 (not used) |
| E Kimi | C#/.NET 8 | 8.0.426 | 0.41 / 0.40 | 0.64 / 0.62 | 6.34 / 6.32 | 8.77 / 8.77 | full run: 33/35 pass (2 EngineEquivalence tests call the AVX-512 engine directly: PlatformNotSupportedException); with those 2 excluded: 33/33 pass | 1 thread; opt = Avx2 engine; seal backend=libcrypto |
| F GLM | Swift 6.1.2 + C kernels | Swift version 6.1.2 | 0.53 / 0.52 | 0.52 / 0.52 | 5.04 / 5.03 | 7.80 / 7.77 | full run: SIGILL (prodTerrainScalarEqualsAvx512 calls AVX-512 kernel directly); with that test skipped: 19/19 pass | external C harness on its compiled kernels (own bench SIGILLs w/o AVX-512); opt fill = dispatch (scalar w/o AVX-512); opt leg = AVX2 fold |

**Candidates** (generated on this machine; `=` same bytes as the build's committed file and as every other build, else the file differs):

| file | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| anchor.txt | = | = | = | = | = | = |
| draw.txt | = | = | = | = | = | = |
| fold.txt | = | = | = | = | = | = |
| hashcash.txt | = | = | = | = | = | DIFF vs others |
| kat-small.txt | = | = | = | = | = | = |
| negatives.txt | = | = | = | = | = | = |
| nl4.txt | = | = | = | = | = | = |
| prod.txt | = | = | = | = | = | = |
| rate.txt | = | = | = | = | = | = |
| seed.txt | = | = | = | = | = | = |
| terrain.txt | = | = | = | = | = | = |
| test-3leg.txt | = | = | = | = | = | = |
| unknown-kind.txt | = | = | = | = | = | = |
| wes.txt | = | = | = | = | = | = |

**Objects** (sha256 of objects produced on this machine; prefix shown):

| build | O1-S1 | O2-S2 | O3-PROD | same as committed |
|---|---|---|---|---|
| A Sonnet | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| B Sol | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| C Grok | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| D Gemini | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| E Kimi | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| F GLM | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |

**Cross-verify O1** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| B Sol | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| C Grok | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| D Gemini | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| E Kimi | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| F GLM | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |

Distinct verify records for O1: 1 (all byte-identical after canonical JSON)

**Cross-verify O2** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| B Sol | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| C Grok | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| D Gemini | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| E Kimi | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| F GLM | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |

Distinct verify records for O2: 1 (all byte-identical after canonical JSON)

**Cross-verify O3** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| B Sol | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| C Grok | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| D Gemini | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| E Kimi | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| F GLM | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |

Distinct verify records for O3: 7 — differences are only Grok's six-leg indices: its CLI refuses `--draw-label` on the production profile and draws from the OS CSPRNG, so its 6 indices are random; all results match.

## Reference system 3

AMD Ryzen 5 7430U with Radeon Graphics; 6 cores / 12 threads; L3 16 MiB (1 instance); RAM 15.0 GiB; flags: avx2 sha_ni (no AVX-512)

| Build | Language | Toolchain | Fill opt | Fill scalar | Leg opt | Leg scalar | Own tests | Notes |
|---|---|---|---|---|---|---|---|---|
| A Sonnet | TypeScript/Node 22 (+WASM) | v22.23.4 | 0.41 / 0.41 | 10.10 / 9.91 | 6.56 / 6.48 | 133.81 / 133.61 | 38 pass / 0 fail | 1 thread; opt leg = WASM fold + node:crypto SHA; WASM fold + WASM SHA leg 5.20 / 5.14 |
| B Sol | Python + C ext | gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0; Python 3.12.3; CFLAGS=-O3 -DNDEBUG -fPIC -Wall -Wextra -Wpedantic -march=native -I~/interop-r2/toolchains/ssl/usr/include -I~/interop-r2/toolchains/ssl/usr/include/x86_64-linux-gnu | 0.59 / 0.58 | 0.60 / 0.58 | 5.85 / 5.84 | 5.75 / 5.74 | 7 run, OK | backend=scalar+openssl; 1 thread |
| C Grok | Zig 0.15.2 | 0.15.2 | 0.32 / 0.29 | 0.34 / 0.34 | 2.72 / 2.58 | 2.72 / 2.60 | 13/13 pass | 1 thread; fast=@Vector path |
| D Gemini | Kotlin/JVM 21 | openjdk version "21.0.12.1"; Gradle 8.10.2 | 0.15 / 0.15 | 0.55 / 0.55 | 2.95 / 2.92 | 6.09 / 6.08 | 6/6 pass | each run = its own min of 2-3 reps; opt fill uses 4 threads; Vector-API-512 path emulated on AVX2 (not used) |
| E Kimi | C#/.NET 8 | 8.0.426 | 0.41 / 0.41 | 0.66 / 0.66 | 4.03 / 3.84 | 6.14 / 6.14 | full run: 33/35 pass (2 EngineEquivalence tests call the AVX-512 engine directly: PlatformNotSupportedException); with those 2 excluded: 33/33 pass | 1 thread; opt = Avx2 engine; seal backend=libcrypto |
| F GLM | Swift 6.1.2 + C kernels | Swift version 6.1.2 | 0.43 / 0.43 | 0.43 / 0.43 | 3.27 / 3.26 | 5.04 / 5.02 | full run: SIGILL (prodTerrainScalarEqualsAvx512 calls AVX-512 kernel directly); with that test skipped: 19/19 pass | external C harness on its compiled kernels (own bench SIGILLs w/o AVX-512); opt fill = dispatch (scalar w/o AVX-512); opt leg = AVX2 fold |

**Candidates** (generated on this machine; `=` same bytes as the build's committed file and as every other build, else the file differs):

| file | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| anchor.txt | = | = | = | = | = | = |
| draw.txt | = | = | = | = | = | = |
| fold.txt | = | = | = | = | = | = |
| hashcash.txt | = | = | = | = | = | DIFF vs others |
| kat-small.txt | = | = | = | = | = | = |
| negatives.txt | = | = | = | = | = | = |
| nl4.txt | = | = | = | = | = | = |
| prod.txt | = | = | = | = | = | = |
| rate.txt | = | = | = | = | = | = |
| seed.txt | = | = | = | = | = | = |
| terrain.txt | = | = | = | = | = | = |
| test-3leg.txt | = | = | = | = | = | = |
| unknown-kind.txt | = | = | = | = | = | = |
| wes.txt | = | = | = | = | = | = |

**Objects** (sha256 of objects produced on this machine; prefix shown):

| build | O1-S1 | O2-S2 | O3-PROD | same as committed |
|---|---|---|---|---|
| A Sonnet | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| B Sol | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| C Grok | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| D Gemini | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| E Kimi | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |
| F GLM | 800ad07a87a567c8 | 6bfc769b831b62de | b16ecdaa13ad6df2 | yes |

**Cross-verify O1** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| B Sol | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| C Grok | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| D Gemini | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| E Kimi | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| F GLM | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |

Distinct verify records for O1: 1 (all byte-identical after canonical JSON)

**Cross-verify O2** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| B Sol | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| C Grok | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| D Gemini | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| E Kimi | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |
| F GLM | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) | all match(104) |

Distinct verify records for O2: 1 (all byte-identical after canonical JSON)

**Cross-verify O3** (row = verifier, column = producer; cell = outcome of every `result`/`results`/`index_comparison`/`seed_value`/`shape` field, count in brackets):

| verifier \ producer | A Sonnet | B Sol | C Grok | D Gemini | E Kimi | F GLM |
|---|---|---|---|---|---|---|
| A Sonnet | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| B Sol | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| C Grok | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| D Gemini | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| E Kimi | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |
| F GLM | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) | all match(68) |

Distinct verify records for O3: 7 — differences are only Grok's six-leg indices: its CLI refuses `--draw-label` on the production profile and draws from the OS CSPRNG, so its 6 indices are random; all results match.

## Method
- Sources fetched file by file from each builder's repository into `src/<id>/`; every file's git blob sha was checked against the tree (all OK). SPEC/POLICY/CASES copies were taken from local files with identical blob shas. Gemini's `gradlew.bat` was not fetched (Windows only).
- Toolchains were installed user-locally in `~/interop-r2/toolchains` on each machine: Zig 0.15.2, Temurin JDK 21.0.12.1 (Kotlin 2.0.21 via the Gradle 8.10.2 wrapper), Node 22.23.4 LTS, Swift 6.1.2 (Ubuntu 24.04 build on reference system 1 and reference system 3, Debian 12 build on reference system 2), and .NET SDK 8.0.426. ref1's .NET is the system install; reference system 3 and reference system 2 got it user-locally via `dotnet-install.sh --channel 8.0` into toolchains/dotnet8. No sudo or apt install was used. gcc comes from the system. The same toolchain versions were used on all three machines.
- For each build, in order: build as README says → own tests with the committed candidates in place → candidate generator → producer → bench (3 runs) → cross-verify. The 5×5 grid ran first; Kimi's build was fetched later (builder repository @ d9fa6ea, all 48 blob shas verified), and its 33 extra cells (Kimi verifying all six producers, the other five verifying Kimi's objects, ×3 objects) were added without re-running existing cells, giving 6×6. Builds ran one at a time.
- Cross-verify used each verifier's synthetic fixture index (Sonnet `--index-fixture`, Sol `--fixture-index`, Kimi `--synthetic-index`, Gemini and Grok default fixture, GLM default) and the fixed draw label `pcoc-pack-2.2/draw` (GLM: `--draw-source chain:pcoc-pack-2.2/draw`). Exception: Grok's CLI refuses `--draw-label` outside conformance-small (usage error, exit 2), so its O3 runs use its default OS-CSPRNG draw. The first grid pass hit that usage error on Grok×O3; those 5 cells were rerun without the label.
- A cell is "all match" when every `result`, `results[]`, `index_comparison`, `seed_value` and `shape` value in the §14 record is `match` (68 such fields for O1 and O3, 104 for O2). Other fields (`not defined`, `skip`, `not anchored`, `not fetched by design`) are the expected states for these offline test-plane objects and are identical across verifiers.

## Build fixes applied
- None to any build's code or build files.
- Sol on reference system 3: libssl-dev headers aren't installed system-wide. I extracted the libssl-dev .deb (`apt-get download`, no install) into toolchains/ssl and passed it as extra `-I` flags in CFLAGS on the make command line. Sol's own flags were kept: `-O3 -DNDEBUG -fPIC -Wall -Wextra -Wpedantic -march=native`.
- Swift on reference system 2 needed libncurses6. The .deb was extracted user-locally and LD_LIBRARY_PATH was set (toolchain only).
- GLM timing harness (`kit/glm_bench.c`): GLM's `SHikeCLI bench` calls `shk_terrain_fill_avx512` unconditionally and dies with SIGILL on all three machines (none has AVX-512). The harness links GLM's unchanged compiled kernel objects (`.build/release/CSatoshiKernels.build/*.o`) and calls the same dispatching entry points the SDK uses (`shk_terrain_fill`, `shk_leg_run` mode 0 scalar / 1 AVX2). The leg value matches all other builds (`88a04ac5beacb8a7`).

## Failures and why
- **Kimi own test suite:** `EngineEquivalence.TerrainIdenticalAcrossEngines` and `FoldIdenticalAcrossEngines` call `Terrain.FillAvx512`/`Leg.FoldAvx512` directly. Without AVX-512 they throw `PlatformNotSupportedException`, so the full run is 33/35 on all three machines. With those two excluded via `--filter`, it's 33/33. Its runtime dispatch (`EngineSelection`), bench, producer and verifier all pick Avx2 correctly. The bench checks scalar and Avx2 give the same leg value (a7b8acbec54aa088 as u64, the same value as the 88a04ac5beacb8a7 LE hex the others print).
- **GLM own test suite:** `prodTerrainScalarEqualsAvx512` calls the AVX-512 kernel directly and crashes with SIGILL without AVX-512, which takes the whole `swift test` run down. With that one test skipped (`--skip`), 19/19 pass. The SDK paths (produce, verify, generator) dispatch correctly and run fine.
- **GLM own bench:** SIGILL for the same reason (see harness above).
- Gemini's "Explicit Vector API (AVX-512)" bench path runs about 10× slower than scalar on these AVX2-only CPUs (JDK emulates 512-bit species). It is not its production path and is not reported in the table.

## Anomalies and findings
1. **GLM `hashcash.txt` differs from the other four builds** on every machine, and also in its committed file. GLM's generator computes `header_hash` = SHA-256(fee header), a single hash. CASES.md line 98 says SHA-256(SHA-256(fee header)). The other four agree on `9e48d971…9d54`, which I checked independently as SHA-256d. So the GLM challenge, nonce and digest differ too. This is generator-only (`GenCandidatesCore/CandidateFile.swift:297`): GLM's SDK hashcash takes the header hash as an input. It's a builder error, not a SPEC flaw.
2. No faster-than-expected result with mismatching bytes, and no verifier accepted anything it shouldn't. Every build's optimised and scalar paths produced identical terrain and leg values.
3. Sol's optimised path is the same code as its scalar path on these machines. Its only fast path is AVX-512, chosen at compile time (`#if __AVX512F__`), so on AVX2 machines "optimised" means scalar C with gcc auto-vectorisation (5.7 s/leg).
4. Grok's and GLM's scalar paths are close to their optimised ones because the compiler auto-vectorises the "scalar" loop at -O/ReleaseFast with -march=native.
5. Gemini's optimised fill uses 4 threads; every other fill figure is 1 thread. Each Gemini sample is already the minimum of 2–3 internal reps, so its median/min is a median of minima.
6. Neither 16 MiB nor 8 MiB of L3 holds the 256 MiB terrain, so these are DRAM-bound figures, unlike the cloud Xeons (320 MiB L3).
7. **Reference system 2 is the low-end host per SPEC §15.5.** It's an i7-10610U with 4 cores, 8 MiB L3 and no SHA-NI. Its figures are the low-end reference, not the primary one (reference system 1 is). A batch job (about 40% of one thread) was running on reference system 2 throughout the bench runs; it was not touched. Reference system 2 numbers may be inflated by that load.
8. **Grok on reference system 2: "fast" leg is slower than its scalar leg.** Median 7.59 s vs 4.21 s over 3 runs, with the same value (88a04ac5beacb8a7, `leg_equal=true`). On the SHA-NI machines the two are equal (2.47 vs 2.51 s on reference system 1). Not investigated or tuned; flagged only.
9. **Kimi's bench labels its seal backend "libcrypto SHA256 (SHA-NI)" on reference system 2**, which has no SHA-NI. The label is cosmetic: libcrypto chooses its own SHA path, and all values match.
10. Kimi's candidates (all 14 files), its objects (O1/O2/O3 sha256 = committed) and all 33 new grid cells match on all three machines. Kimi agrees with the four-build majority on `hashcash.txt` (SHA-256d), so GLM stays the only outlier.

## Packed runs (N concurrent independent production hikes on one machine)

Producer: C Grok (Zig), normal optimised `.fast` engine, plus `.scalar` on reference system 2. Gemini was not used as a producer (trimmed plan); it is the cross-checker.
Driver: `kit/packed/grok_packed.zig`, a thin driver compiled against Grok's unchanged `hike` library module (`src/root.zig`; `zig build-exe -OReleaseFast --dep hike -Mroot=… -Mhike=src/root.zig`). Each instance is its own hike:
- its own pk = SHA-256(label `pcoc-packed/<machine>/grok/<engine>/N<n>/i<k>`), so its own seed (SPEC §10 with the CASES seed beacon) and its own 256 MiB terrain fill;
- chained 2,000,000-step production legs from its own checkpoints: previous = waypoint = seed D, then each leg's §11.4 checkpoint feeds the next leg (stage-0 chaining, SPEC §11.2–11.4);
- no shared terrain and no shortcuts.
Timing: all N start together. The steady window opens when every instance has finished leg 0 (fill and leg 0 discarded) and runs 60 s. Per-instance legs/s = (legs ending in the window − 1) / (time between the first and last of those leg ends). Instances keep walking until the window closes, so every counted leg ran under full N-way load.
Cross-check: the last counted leg of every instance was recomputed from its logged pk and previous checkpoint by Gemini's unchanged library (`kit/packed/GeminiPacked.java` against `satoshi-hike-all.jar`: computeSeed → fillTerrain → legResult → computeCheckpoint). Both leg result and checkpoint must match. As a sanity check, both drivers reproduce the CASES pk_A leg 0 (`88a04ac5beacb8a7`).
Memory: RSS per instance read mid-window; used/available from `free`. Raw per-instance data is in `packed-<machine>.csv`.

### Reference system 1 (primary)

Load: desktop session only (a desktop app, cinnamon); nothing heavy.

| engine | N | per-hike legs/s median | min | aggregate legs/s | s/leg (median) | RSS/instance | mem used / avail | swap used | load (1 min) | sampled legs vs Gemini |
|---|---|---|---|---|---|---|---|---|---|---|
| fast | 1 | 0.402 | 0.402 | 0.402 | 2.49 | 256 MiB | 4.43 / 10.60 GiB | 0.00 GiB | 0.74 | 1/1 match |
| fast | 4 | 0.283 | 0.282 | 1.133 | 3.54 | 256 MiB | 5.16 / 9.87 GiB | 0.00 GiB | 2.56 | 4/4 match |
| fast | 6 | 0.190 | 0.184 | 1.140 | 5.26 | 256 MiB | 5.65 / 9.38 GiB | 0.00 GiB | 4.58 | 6/6 match |

Below 0.2 legs/s per hike at **N=6** (median 0.190, min 0.184). N=4 holds 0.283. Aggregate throughput saturates at about 1.13–1.14 legs/s from N=4 on: the 256 MiB terrains are DRAM-bound (16 MiB L3), so adding hikes past 4 only splits a fixed bandwidth.

### Reference system 2 (low-end host per SPEC §15.5)

Load: a batch job (about 40% of one thread, not a groundinglab production service) was paused for the timed runs and resumed right after. The PCOC CI runner containers (ciheavy podman) were left untouched; they were idle (about 0.3% CPU).

| engine | N | per-hike legs/s median | min | aggregate legs/s | s/leg (median) | RSS/instance | mem used / avail | swap used | load (1 min) | sampled legs vs Gemini |
|---|---|---|---|---|---|---|---|---|---|---|
| fast | 1 | 0.134 | 0.134 | 0.134 | 7.44 | 256 MiB | 1.96 / 29.26 GiB | 0.00 GiB | 0.84 | 1/1 match |
| fast | 2 | 0.137 | 0.137 | 0.274 | 7.31 | 256 MiB | 2.20 / 29.02 GiB | 0.00 GiB | 1.35 | 2/2 match |
| scalar | 1 | 0.242 | 0.242 | 0.242 | 4.13 | 256 MiB | 1.95 / 29.27 GiB | 0.00 GiB | 1.36 | 1/1 match |
| scalar | 2 | 0.238 | 0.238 | 0.477 | 4.19 | 256 MiB | 2.20 / 29.02 GiB | 0.00 GiB | 1.56 | 2/2 match |

Only N=1 and N=2 were run (trimmed plan). Neither engine drops below its own single-hike rate at N=2 (4 cores). Best legal packed rate on the low-end host: **0.238 legs/s per hike at N=2 with Grok `.scalar`** (aggregate 0.477). Grok `.fast` is **below 0.2 even at N=1** (0.134 legs/s, 7.44 s/leg). That is Grok's no-SHA-NI anomaly: its `.fast` path is slower than its scalar path on this CPU.

**Single-hike `hike bench` with the worker paused** (3 runs, s, median / min): fill fast 0.263 / 0.263, fill scalar 0.382 / 0.382, leg fast 7.564 / 6.956, leg scalar 4.207 / 4.188. These are the same as the loaded figures in the per-machine table (7.59 / 6.99 and 4.21 / 4.20), so the worker's about 40% of one thread had no measurable effect on reference system 2 leg timing.

Swap: Reference system 2 has no swap device; the 1.8 MB reported is noise from `free`. Available memory stayed at 29 GiB.

### Reference system 3

Load: Chrome open (about 43 processes, about 12% CPU at start), plus a desktop app and gnome-system-monitor. 2 GiB of swap was already in use before the run (pre-existing); swap_used below is the total.

| engine | N | per-hike legs/s median | min | aggregate legs/s | s/leg (median) | RSS/instance | mem used / avail | swap used | load (1 min) | sampled legs vs Gemini |
|---|---|---|---|---|---|---|---|---|---|---|
| fast | 1 | 0.375 | 0.375 | 0.375 | 2.67 | 256 MiB | 6.88 / 8.14 GiB | 2.31 GiB | 0.51 | 1/1 match |
| fast | 4 | 0.246 | 0.242 | 0.985 | 4.06 | 256 MiB | 7.43 / 7.59 GiB | 2.51 GiB | 2.18 | 4/4 match |
| fast | 6 | 0.170 | 0.163 | 1.015 | 5.89 | 256 MiB | 7.77 / 7.25 GiB | 2.67 GiB | 3.93 | 6/6 match |

Below 0.2 legs/s per hike at **N=6** (median 0.170, min 0.163). N=4 holds 0.246 (min 0.242), so N=6 was run per the rule. Aggregate saturates at about 1.0 legs/s, the same DRAM-bound shape as reference system 1 and about 10% lower, consistent with Chrome load and the slower 7430U.

Swap: swap use rose from 2.31 to 2.67 GiB across the ladder, but 7.2 GiB stayed available throughout. That looks like the kernel paging out cold desktop/Chrome pages, not the hikes being swapped: each instance's RSS stayed at its full 256 MiB and leg times were steady. Flagged.

