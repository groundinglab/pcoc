# Satoshi Hike SPEC 2.2 (erratum 1) — C# / .NET implementation

An independent, optimized implementation of the Satoshi Hike SPEC 2.2 (with erratum 1), built only from the pinned public inputs committed under `spec-ref/`:

- `spec-ref/SPEC.md` — SPEC 2.2 @ `7b40353` (sha256 verified)
- `spec-ref/POLICY.md`, `spec-ref/gl-admit-v9.toml` — groundinglab-admission policy @ `7b40353`
- `spec-ref/CASES.md` — round-2 interop fixtures @ `8c40cf8`

`RESULTS.md` (committed, required by the fixtures) documents the model, machine, timings, optimizations, every SPEC question met, and the possible-SPEC-flaw analysis.

## Layout

- `impl/` — .NET 8 solution (`Pcoc.sln`): the `Pcoc` library (SPEC §§6–14, 16.1), the `pcoc-cli` CLI (candidate generator, offline producer, verifier, benchmark; the §16 permanent negatives live only in `impl/src/Pcoc.Cli/negatives.cs`), and xUnit tests.
- `candidates/` — the 14 deterministic candidate files with the exact CASES keys.
- `objects/` — the offline test-plane objects `O1-S1.cbor`, `O2-S2.cbor`, `O3-PROD.cbor`.

## Run it

```bash
cd impl
dotnet build Pcoc.sln -c Release
dotnet test  Pcoc.sln -c Release        # 35 tests, incl. re-running the generator
                                       # and byte-comparing the committed candidates
PC=src/Pcoc.Cli/bin/Release/net8.0/pcoc-cli.dll
dotnet $PC gen-candidates --out ../candidates
dotnet $PC produce --out ../objects
dotnet $PC verify ../objects/O3-PROD.cbor --synthetic-index --draw-label pcoc-pack-2.2/draw
dotnet $PC bench                        # scalar / AVX2 / AVX-512 fill + leg timings
```

The terrain fill and the §11.1 fold are vectorized (scalar, AVX2, AVX-512 engines, runtime dispatch; `PCOC_ENGINE=scalar|avx2|avx512` overrides) and the §11.1 seal uses SHA-NI through libcrypto when available. All engines are bit-identical.
