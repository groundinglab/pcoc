# Satoshi Hike 2.2 (Zig)

An independent implementation of SPEC 2.2 with erratum 1. The pinned inputs live in this tree:

- `spec/satoshi-hike-v2.2/SPEC.md`
- `policy/groundinglab-admission/POLICY.md` and `gl-admit-v9.toml`
- `spec/conformance/interop/round2/CASES.md`

`RESULTS.md` records the build, the timings, and the readings taken where the SPEC leaves a choice.

## Build

Requires Zig 0.15.2.

```sh
cd impl
zig build --release=fast
zig build test --release=fast
```

`zig build test` re-runs the candidate generator and checks that `candidates/` is unchanged.

## Tools

From `impl/`:

```sh
./zig-out/bin/gen-candidates --out ../candidates
./zig-out/bin/hike produce-fixtures --out ../objects
./zig-out/bin/hike bench
./zig-out/bin/hike verify ../objects/O1-S1.cbor \
  --profile conformance-small \
  --sample-bytes 32 --slot-bytes 64 --terrain-bytes 65536 --steps-per-leg 100
```

`verify` prints one section 14 record and exits 0 when a record was produced. A decode failure exits 1. A usage error, including `conformance-small` without all four sizes, exits 2. Omit `--index-url` to use the synthetic index from `CASES.md`. `--no-index` leaves header reads `not read`. `--profile production` (the default) uses the section 6 sizes.

`produce-fixtures` writes `O1-S1.cbor`, `O2-S2.cbor`, and `O3-PROD.cbor`, and writes `path_skip 1` … `path_skip 16` and `not fetched by design 17` to stderr for each object.
