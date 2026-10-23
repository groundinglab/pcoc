# Satoshi Hike 2.2 — independent Python implementation

This is an independent implementation of Satoshi Hike SPEC 2.2 with erratum 1,
built only from the pinned public inputs copied under `public-inputs/` and
`spec/conformance/interop/round2/CASES.md`.

The public API and orchestration are Python. The required hot loop is an
in-repo C native library (`impl/native/hike.c`) loaded with `ctypes`. It has a
portable scalar path and an AVX-512 path selected at build time. Both use the
system OpenSSL SHA-256 implementation (and therefore SHA-NI on the benchmark
host). NumPy and third-party Python packages are not used.

## Build and generate

```sh
make native
python3 -m impl.satoshi_hike generate-all
```

The generator writes all 14 exact-key candidate groups to `candidates/` and
the three deterministic CBOR objects to `objects/`.

## Verify

Production profile (the default):

```sh
python3 -m impl.satoshi_hike verify objects/O3-PROD.cbor \
  --fixture-index --draw-label pcoc-pack-2.2/draw
```

S1 `conformance-small`:

```sh
python3 -m impl.satoshi_hike verify objects/O1-S1.cbor \
  --profile conformance-small \
  --slot-bytes 64 --sample-bytes 32 --terrain-bytes 65536 \
  --steps-per-leg 100 \
  --fixture-index --draw-label pcoc-pack-2.2/draw
```

`--fixture-index` is the offline synthetic index defined by `CASES.md`.
Without an index option, index measurements are `not read`. An on-disk index
can be supplied as `--index-dir DIR`, with one `<height>.json` file per row.
When `--draw-label` is omitted, verification draws from the operating-system
CSPRNG.

## Test and benchmark

```sh
make test
make benchmark
```

The tests regenerate every committed candidate and compare bytes. The
benchmark measures scalar and vectorized production terrain fill and one
production leg, and checks that both paths produce identical values.

See `RESULTS.md` for provenance, machine details, timings, optimizations,
SPEC questions, and completion notes.
