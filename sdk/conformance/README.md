# Cross-verify gate

`cross-verify.sh` is the Phase A gate for the four Satoshi Hike 2.2 SDKs (Rust, C, Go, and Java). It builds each verify CLI from this tree and checks that they agree on the CASES v2 objects, the candidate files, the pinned digests, the thirteen permanent negatives, and the malformed inputs.

The specification is [spec/satoshi-hike-v2.2/SPEC.md](../../spec/satoshi-hike-v2.2/SPEC.md) read with [ERRATUM-1.md](../../spec/satoshi-hike-v2.2/ERRATUM-1.md). The negative table is parsed from that erratum when the gate runs. It is not copied into this directory.

## What it checks

Run from the repository root.

1. **Build.** Release build of each verify CLI. Rust and C are built for the baseline ISA (`target-cpu=x86-64` / `-march=x86-64`), not `-march=native`. A missing toolchain is an error that names the tool.
2. **Matrix.** Every verifier measures every object. Objects are `sdk/<lang>/objects/O1-S1.cbor`, `O2-S2.cbor`, and, on the full tier, `O3-PROD.cbor`. With `--ref-dir`, each reference build's copies are included. S1 and S2 use `conformance-small` and the four CASES integers. O3 uses the section 6 profile. The rig draw label is `pcoc-pack-2.2/draw`. The CASES synthetic index (heights 989 through 1018) is served on `127.0.0.1` only.
3. **Pins.** Each object file's SHA-256 is the pinned digest, and `prod.PROD.stage.0.leg.0.legs_le_hex` is `88a04ac5beacb8a7`.
4. **Candidates.** The 14 CASES v2 files under `sdk/<lang>/candidates/` are byte-identical across the four SDKs. With `--ref-dir`, they also match the reference builds' agreed bytes (the byte string held by the largest set of reference builds). A difference prints the first differing key.
5. **Negatives.** Each `negatives.txt` matches the erratum reference table, every `matches_honest` is false, the leg values are pairwise distinct, and the terrain digests are pairwise distinct. Then each verifier measures an S1 object whose `leg_results` are that leg negative, and the six-leg results are `mismatch`. Those objects are built in this harness by replacing the 8-byte leg strings. No SDK negative module is linked.
6. **Malformed.** Empty input and a truncated object (a byte string that claims 5 bytes and supplies 1) must exit 0 with `shape` `mismatch` and `shape_item` exactly `["malformed cbor"]`. The four CASES unknown-kind labels must be the Rule A5 record and exit 0. Invalid UTF-8 in `kind` is malformed CBOR too (erratum 2 E01). All three malformed records must be byte-identical across the four verifiers and match the pinned record. See [Malformed CBOR records (erratum 2)](#malformed-cbor-records-erratum-2).

A matrix cell matches when the section 14 records are byte-identical across verifiers for those object bytes, `shape` is `match`, every header comparison is `match`, every six-leg result is `match`, and the record has no `not read`. Stages 1–17 store the empty path, so the bookends `a`, `b`, `N`, `S`, and `delta_h` are `not defined`.

Identical object bytes are verified once per verifier. The report names every source that shares the SHA-256.

The Rust, C, and Go verify CLIs do not take an index argument, so their matrix cells call the same verify entry the CLI calls (`pcoc_verify`, `pcoc.Verify`, and `pcoc_testing::verify_from_bytes`) and pass documents from the loopback index. The Java CLI takes `--index` as a directory of `{height}.json` files. Those files are the bodies the loopback listener returns. The commands and their output are in the report under "CLI surface".

## How to run

From the repository root:

```sh
bash sdk/conformance/cross-verify.sh --tier pr
bash sdk/conformance/cross-verify.sh --tier full --ref-dir ../ref-round2 --out ../XV-FULL.md
```

`--tier` defaults to `pr`. The script prints the markdown report on stdout and, with `--out`, writes the same text to that file. Exit 0 means every check passed. Exit 1 means a named cell failed. Exit 2 is usage.

`XVERIFY_INPUT_ROOT`, when set, is the tree of `rust`, `c`, `go`, and `java` directories to read objects and candidates from. The CLIs are still built from this repository. CI does not set it. It is how a reviewer points the gate at a throwaway copy.

## Tiers

| Tier | Matrix objects | Also checked |
|---|---|---|
| `pr` | O1-S1, O2-S2 | pins of all three objects (hash only), the PROD leg hex, candidates, negatives, six-leg negative cells, malformed inputs |
| `full` | O1-S1, O2-S2, O3-PROD | the same checks, and a full section 14 measurement of O3-PROD |

`pr` is the CI tier. It does not recompute a production leg. `full` does, once per verifier per distinct O3-PROD byte string, as a correctness check. The script does not print timings.

## Toolchains CI needs

- Rust 1.99, as pinned by `sdk/rust/rust-toolchain.toml`, with Cargo. The build is offline against the crates already named by `sdk/rust`.
- Go, new enough for `sdk/go/go.mod` (`go 1.24.0`). The build uses the checksums in `sdk/go/go.sum`.
- JDK 21. `sdk/java/build.gradle.kts` sets the compiler release to 21, so JDK 17 cannot compile that SDK.
- gcc and make, for the C SDK (`-march=x86-64`).
- Python 3, standard library only, for this harness.
- A self-hosted runner. The script does not start a workflow.

Thread use is capped at 4 compile jobs. Verifiers run one at a time under `nice -n 10`. `GOMAXPROCS` is 1 while verifying.

## Malformed CBOR records (erratum 2)

SPEC 2.2 erratum 2 settled the record of a malformed CBOR value (E02, option A) and made every CBOR text string that is not valid UTF-8, a map key or a `kind` value, a malformed CBOR value (E01). The gate has no mask: `PENDING_ERRATUM_2` in `harness.py` is `()`. For the empty input, the truncated bytes `58 05 01`, and the one-pair map `a1 64 6b 69 6e 64 61 ff` (a `kind` text that is the byte `0xFF`), the four verifiers must exit 0 and print byte-identical records, and each record (with its newline) must have SHA-256 `MALFORMED_RECORD_SHA256` (`eb7bff4d55ddcc47e47c600276693db8a406a0083a47c2526462651ffa5bdb8c`). In that record `shape` is `mismatch`, `shape_item` is `["malformed cbor"]`, `paths[g].path_state` and `paths[g].plant` for `g` in 1…16 are `not defined`, `skip_list` is `[]`, and every `analysis.per_stage[g].checkpoint_count`, `analysis.whole_hike_legs` and `analysis.hike_legs` are `not defined`. Any difference fails the gate.

## Adding a fifth SDK

1. Put it at `sdk/<lang>/` with `objects/O1-S1.cbor`, `O2-S2.cbor`, `O3-PROD.cbor` and the candidate files, including `br1.txt`. Do not add a negative to its release CLI.
2. In `cross-verify.sh`, add a release build (baseline ISA, no `-march=native`) that produces a verify CLI. Fail with `missing toolchain:` if that compiler is absent.
3. Teach the CLI, or a driver under `sdk/conformance/`, to verify one object with `--profile`, the four `conformance-small` integers, `--draw-label`, and the CASES index. If the CLI cannot take the index, add a driver that calls that SDK's verify entry and reads `GET http://127.0.0.1:PORT/index/{height}`. A directory of `{height}.json` is acceptable only when that is the CLI's own index argument, and the files must be the loopback bodies.
4. Add the language to `LANGS` in `harness.py`, and to the matrix command and the CLI command. Keep the verifier order stable so the report's columns stay readable.
5. Run `--tier pr` and then `--tier full` with the reference directory. A new column appears in the matrix. Exit 0 is the same rule as for four.
