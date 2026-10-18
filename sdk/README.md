# PCOC SDKs

A **PCOC** is a self-issued identifier. Its cost is a resource commitment, not a payment: the maker holds a large terrain in memory and computes legs of work over it, back to back, while Bitcoin supplies the clock. Each new Bitcoin header ends one stage, and the hike runs for 18 stages. Anyone can check the result (the hike object) against public Bitcoin data. Satoshi Hike v2.0 is the specification.

This directory has three SDKs for it, in Rust, Go and Java. Each one can verify a hike object, which means writing the section 14 measurement record, and produce one. All three are tested against the same conformance vectors and cross-checked against each other. The admission gate and the desktop app in this repository build on the Rust SDK.

The [specification](../spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md) and its [conformance vectors](../spec/conformance/v2.0/) are the source of truth. Where an SDK disagrees with them, the specification wins and the SDK has a bug.

## Layers

1. **Specification and conformance vectors** ([spec/](../spec/)): the law, and the bytes every implementation must reproduce.
2. **SDKs** (this directory): libraries and command-line programs that implement the specification.
3. **Services**: the [admission gate](../services/admission-gate/), which applies a policy to verify records, and the [index](../services/index/), which serves the public Bitcoin data a verifier reads.
4. **Apps**: the [desktop app](../apps/desktop/), and the Android app (in its own repository, through the Rust core).

## Which one

| You are writing | Use | Read |
|---|---|---|
| A Rust service or app, or an Android app through the Rust core (JNI) | Rust: crate `hike` (plus `nip06` for demo keys and `hike-net` for direct node reads) | [rust/README.md](rust/README.md) |
| A Go service or tool | Go: module `github.com/groundinglab/pcoc/sdk/go/v2`, package `hike` | [go/README.md](go/README.md) |
| A JVM service or tool | Java: packages `pcoc.hike` and `pcoc.json` (JDK 21; no build tool needed) | [java/README.md](java/README.md) |

All three write byte-identical records. [examples/](examples/) has the same programs in each language: a quickstart that reproduces a conformance vector, an offline produce run, and a live verify against the public index. `sdk/test.sh` runs or compiles every one of them.

Only Rust has the section 15 helpers (`admit`, `failure_facts`) and direct node reads.

## Version and pinning

| | Value |
|---|---|
| SDK version | `2.0.0-rc.2`, in [VERSION](VERSION) |
| Repository tag | `sdk-v2.0.0-rc.2`. Rust and Java pin this one. |
| Go module tag | `sdk/go/v2.0.0-rc.2`. The module path requires the `sdk/go/` prefix. |
| Specification | The tag and vector hash in [SPEC_PIN](SPEC_PIN). `sdk/test.sh` checks both. |

Both tags point at the same commit. Pin a tag, never `main`.

Inside this repository, the gate and the desktop app pin the SDK by path (`sdk/rust/hike`), and each reports the version it was built with:
- the gate: in `/v1/health` (`sdk`) and in `admission-service --version`;
- the desktop app: on its About page.

> Both `rc.2` tags are published on GitHub. Inside a checkout you can also depend by path, as `examples/` does. `git rev-parse sdk-v2.0.0-rc.2^{commit}` shows the commit the tag names.

Earlier tags of these trees are kept as history: `ref-rust-v2.0.1` … `ref-rust-v2.0.6`, `ref-java-v2.0.1` … `ref-java-v2.0.2`, and the Go module tags under the old module path. Nothing new is tagged under them.

## Conformance vectors: the source of truth

`spec/conformance/v2.0/` holds the vectors. [Its README](../spec/conformance/v2.0/README.md) describes each file. They include:
- hike objects, each with its recorded index replies and the exact section 14 record bytes it must produce: `test-3leg`, `kat-small`, `anchor-window`, `anchor-w16-i17`, `bind-threads`, `earlier-run-b`;
- known answers at the small `conformance-small` sizes;
- one checkpoint preimage;
- the P9.14 produce tip script (`stage0-start.json`);
- the section 6 constants;
- the section 15.1 sign-in wire vectors.

A Part A implementation conforms when it reproduces every vector byte for byte. Two fields in `kat-small.json` are easy to confuse:
- `sha256` is the SHA-256 of the **object** bytes;
- `record` is the record string itself.

`draw_bytes_hex` is the six-leg draw stream that `record` was made with. It is the pinned stream SHA-256(seed ‖ le64(0)) ‖ SHA-256(seed ‖ le64(1)) ‖ …, with the seed being 32 bytes of `0x5a`.

- **The pin.** [SPEC_PIN](SPEC_PIN) names the spec tag, plus a SHA-256 over every file under `spec/conformance/` except `tools/`. [tools/check_spec_pin.py](tools/check_spec_pin.py) recomputes that hash in the working tree and in the tag's tree. A changed vector without a new pin fails.
- **Who writes the vectors.** The Rust hike pack generator in `sdk/rust/hike` writes the files listed in `manifest.json` (`PCOC_REGEN_V2=1`, only in a spec change). `sign-in-origins.json` is written by the admission gate's tests (`PCOC_REGEN_SIGNIN_ORIGINS=1`). `sign-in.json` and `stage0-start.json` are not generator output. Go and Java only read the vectors.
- **No copies.** Each SDK reads the pack from `spec/conformance/v2.0/` in place; none keeps its own copy. A vector change is a spec change: it comes with a new spec tag and a new `SPEC_PIN`.

## One harness

```sh
bash sdk/test.sh
```

It runs every step, then exits non-zero if any step failed:

1. **pins:** `SPEC_PIN` against `spec/conformance/` and against its tag's tree, and every Rust crate version against `VERSION`;
2. **Rust:** `cargo test --locked --release`, covering unit tests, the conformance pack and malformed-input tests;
3. **Go:** `go vet ./...` and `go test ./...`, covering unit tests and the conformance pack;
4. **Java:** `java/v2.sh`, covering the conformance pack and the section 6 pins;
5. **cross-check:** the Rust, Go and Java verify programs run on the pack objects, deterministic mutants of `kat-small`, targeted section 14.1 edits, one-byte non-map CBOR items and the structural break cases in `sdk/tools/hikecases.py`, and must write byte-identical records and exit codes. Each SDK's verify call is compared the same way at the library level, through the `librecord` examples ([tools/crosscheck.py](tools/crosscheck.py));
6. **examples:** every example compiles. The quickstarts must reproduce the `kat-small` record byte for byte, and the produce examples must write the same object in all three languages;
7. **docs:** every version, tag, spec tag, digest, output block and code snippet in these READMEs must match `VERSION`, `SPEC_PIN`, the example output and the example sources ([tools/check_docs.py](tools/check_docs.py)).

It needs `python3`, Rust (rustup picks up `rust/rust-toolchain.toml`), Go 1.22 or newer, JDK 21 or newer, and a git clone that has the spec tag. A missing tool is a failure, not a skip. Cargo writes build output to `sdk/rust/target/` and `sdk/examples/rust/target/` (both git-ignored), or to `$CARGO_TARGET_DIR` when that is set. CI runs the harness as the `sdk` job on the self-hosted runners.

The end of a passing run looks like this:

```text
sdk/test.sh summary
  PASS  pins: spec vectors + sdk version (0s)
  PASS  rust: unit + conformance (57s)
  ...
sdk/test.sh: PASS
```

On a few malformed inputs the SDKs still disagree, on two open spec questions (a `stages` element that is not a map, and a `last_interrupt_height` above 2^53 − 1). [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md) lists them with the other open spec questions, and the rows fixed for rc.2; [CHANGES.md](CHANGES.md) describes rc.1 → rc.2. The harness pins each open case by name and by its output digests.

## The verify programs

`satoshi-hike-verify` takes the same flags in all three SDKs:
- `--input PATH`, `--index-url URL` and `--profile production|conformance-small|test|live`;
- `--audit-seed HEX`, which pins the six-leg draw;
- `--report PATH`, which also writes a JSON report;
- `--log quiet|verbose`: with `verbose`, the record is also written to stderr;
- `--chain-rpc URL`, live profile only.

Verify writes the record to stdout. It exits 1 only when no top-level CBOR map decodes: the file cannot be read, the first CBOR item does not decode (section 7.3), or it is well-formed but not a map. In every other case, trailing bytes after a map included, it writes the record and exits 0. Usage errors exit 2. The library call still returns the `["malformed cbor"]` record for bytes the program refuses. The three SDKs agree (rc.2).

The produce flags differ by SDK. See each program's `--help`.

## What producing costs

- **Wall time.** A production hike waits for **18 new Bitcoin headers** after its seed header: the interrupts `I_0` … `I_17`, one per stage. A stage ends at the first new header read after one of its checkpoints (section 13.2). Wall time is therefore about 18 × the mean interval between new headers, which is about 3 hours at Bitcoin's 10-minute target. Add the path asks due at the cut-off header, each after a wait of 15 to 45 seconds (section 13.1).
- **Memory and compute.** For that whole time the producer holds `TERRAIN_BYTES`, which is 256 MiB at production sizes, and computes legs of 2,000,000 steps back to back.
- **Hashcash.** Each waypoint post to the plant service carries a Hashcash proof. The plant service sets its difficulty.

The offline produce examples use a scripted tip sequence at `conformance-small` sizes, so they finish in under a second.

## What verify does and does not do

Verify writes the section 14 measurement record. It does not judge. Judgment is policy (section 15): a gate applies a policy to the record.
- **The SDKs' policy code.** The only policy code in the SDKs is Rust's `hike::admit` and `hike::failure_facts`. They apply the section 15 invariant refusals: shape, checkpoint mismatch, seed value, plant mismatch, successor, and the six legs. They apply **no threshold**.
- **Thresholds.** Values such as the leg-rate floor or the anchored-path minimum belong to the relying party's policy. groundinglab's argued default is [services/admission-gate/POLICY.md](../services/admission-gate/POLICY.md). It is a recommendation, not part of the specification.
- **Nesting limit (rc.3).** Every SDK's CBOR decoder stops at 64 nested arrays and maps (`hike::MAX_CBOR_NESTING`, Go `hike.MaxCBORNesting`, Java `Cbor.MAX_CBOR_NESTING`). A deeper item is malformed CBOR. This is an implementation resource limit, not a spec rule: a valid hike object nests 4 deep. It bounds the work a gate does on unauthenticated bytes before any signature check. See [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md#implementation-resource-limits).
- **No policy values in the SDKs (rc.2).** The section 15.5 leg-rate floor examples and the test-plane judgment (`judge_test_policy`) are no longer SDK code: the spec defines measurements, a policy sets the values. The examples stay in `spec/conformance/v2.0/constants.json` and the SDK test fixtures; the test-plane judgment is a fixture in `services/admission-gate/tests/test_plane_policy.rs`.

## Layout

| Path | What |
|---|---|
| [rust/](rust/) | Cargo workspace: `hike`, `hike-net`, `nip06`, and the programs `satoshi-hike-produce` and `satoshi-hike-verify` |
| [go/](go/) | Go module: package `hike`, programs under `cmd/`, the conformance test under `conformance/` |
| [java/](java/) | Java sources (`pcoc.hike`, `pcoc.json`, `pcoc.cli`, `pcoc.conformance`) and `v2.sh` |
| [examples/](examples/) | Quickstart, produce and live examples in each language, all built by `test.sh`, and the `librecord` programs the library-level cross-check runs |
| [tools/](tools/) | `check_spec_pin.py`, `check_versions.py`, `check_docs.py`, `crosscheck.py` |
| [VERSION](VERSION), [SPEC_PIN](SPEC_PIN) | SDK version; spec tag and vector hash |
| [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md), [RC-NOTES.md](RC-NOTES.md) | Where the SDKs disagree and the open spec questions; release-candidate notes, consumer impact and backlog |

↑ [root README](../README.md)
