# PCOC SDKs

A **PCOC** is a self-issued identifier. Its cost is a resource commitment, not a payment: the maker holds a large terrain in memory and computes legs of work over it, back to back, while Bitcoin supplies the clock. Each new Bitcoin header ends one stage, and the hike runs for 18 stages. Anyone can check the result (the hike object) against public Bitcoin data. Satoshi Hike 2.2 is the specification.

This directory has three SDKs for it, in Rust, Go and Java. Each one can verify a hike object, which means writing the section 14 measurement record, and produce one. All three are tested against the same conformance vectors and cross-checked against each other. The admission gate and the desktop app in this repository build on the Rust SDK.

The [specification](../spec/satoshi-hike-v2.2/SPEC.md) and its [conformance pack](../spec/conformance/pack/) are the source of truth. Where an SDK disagrees with them, the specification wins and the SDK has a bug. The pack is incomplete until the SDK phase.

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
| SDK version | `2.0.0-rc.3`, in [VERSION](VERSION) |
| Repository tag | `sdk-v2.0.0-rc.3`. Rust and Java pin this one. |
| Go module tag | `sdk/go/v2.0.0-rc.3`. The module path requires the `sdk/go/` prefix. |
| Specification | The tag and vector hash in [SPEC_PIN](SPEC_PIN). `sdk/test.sh` checks both. |

Both tags point at the same commit. Pin a tag, never `main`.

Inside this repository, the gate and the desktop app pin the SDK by path (`sdk/rust/hike`), and each reports the version it was built with:
- the gate: in `/v1/health` (`sdk`) and in `admission-service --version`;
- the desktop app: on its About page.

> The rc.3 tags name this version (`sdk-v2.0.0-rc.3` and `sdk/go/v2.0.0-rc.3`). Inside a checkout you can depend by path, as `examples/` does. Those tags are not created by this commit.

Earlier tags of these trees are kept as history: `ref-rust-v2.0.1` … `ref-rust-v2.0.6`, `ref-java-v2.0.1` … `ref-java-v2.0.2`, and the Go module tags under the old module path. Nothing new is tagged under them.

## Conformance vectors: the source of truth

`spec/conformance/pack/` is the one pack. [Its README](../spec/conformance/pack/README.md) lists every SPEC §16 vector and permanent negative as present or missing. The pack is incomplete until the SDK phase.

- **The pin.** [SPEC_PIN](SPEC_PIN) names tag `spec-2.2.0` and the SHA-256 of `spec/conformance/pack/manifest.json`. [tools/check_spec_pin.py](tools/check_spec_pin.py) still hashes every file under `spec/conformance/` except `tools/` and looks the tag up. That checker is not edited in this change.
- **Who writes the vectors.** The SDK phase generates them. Go and Java do not keep a private copy.
- **No copies.** Each SDK is meant to read `spec/conformance/pack/` in place. The readers still name the removed pack path. Those lines are not edited here.

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

The three SDKs agree on the cross-check, including a `stages` element that is not a map. [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md) lists the rows fixed for rc.2 and rc.3. The harness pins each open case by name and by its output digests. There are no open pins.

## The verify programs

`satoshi-hike-verify` takes the same flags in all three SDKs:
- `--input PATH`, `--index-url URL` and `--profile production|conformance-small|test|live`;
- `--sample-bytes N`, `--slot-bytes N`, `--terrain-bytes N` and `--steps-per-leg N`, all four required with `--profile conformance-small`. Any of the four with another profile is a usage error, exit 2;
- `--audit-seed HEX`, which pins the six-leg draw;
- `--report PATH`, which also writes a JSON report;
- `--log quiet|verbose`: with `verbose`, the record is also written to stderr;
- `--chain-rpc URL`, live profile only.

Verify writes the record to stdout. It exits 1 only when no top-level CBOR map decodes: the file cannot be read, the first CBOR item does not decode (section 7.3), or it is well-formed but not a map. In every other case, trailing bytes after a map included, it writes the record and exits 0. Usage errors exit 2. The library call still returns the `["malformed cbor"]` record for bytes the program refuses. The three SDKs agree (rc.2).

The produce flags differ by SDK. See each program's `--help`.

## What producing costs

- **Wall time.** A production hike waits for **18 new Bitcoin headers** after its seed header: the interrupts `I_0` … `I_17`, one per stage. A stage ends at the first new header read after one of its checkpoints (section 13.2). Wall time is therefore about 18 × the mean interval between new headers, which is about 3 hours at Bitcoin's 10-minute target. Path asks are sent when they are due (section 13.1).
- **Memory and compute.** For that whole time the producer holds `TERRAIN_BYTES`, which is 256 MiB at production sizes, and computes legs of 2,000,000 steps back to back.
- **Hashcash.** Each waypoint post to the plant service carries a Hashcash proof. The plant service sets its difficulty.

The offline produce examples use a scripted tip sequence at `conformance-small` sizes, so they finish in under a second.

## What verify does and does not do

Verify writes the section 14 measurement record. It does not judge. Judgment is policy (section 15): a gate applies a policy to the record.
- **The SDKs' policy code.** The only policy code in the SDKs is Rust's `hike::admit` and `hike::failure_facts`. They apply the section 15 invariant refusals: shape, checkpoint mismatch, seed value, plant mismatch, successor, and the six legs. They apply **no threshold**.
- **Thresholds.** Values such as the leg-rate floor or the anchored-path minimum belong to the relying party's policy. groundinglab's policy is [policy/groundinglab-admission/POLICY.md](../policy/groundinglab-admission/POLICY.md). It is not part of the specification.
- **Nesting limit (rc.3).** Every SDK's CBOR decoder stops at 64 nested arrays and maps (`hike::MAX_CBOR_NESTING`, Go `hike.MaxCBORNesting`, Java `Cbor.MAX_CBOR_NESTING`). A deeper item is malformed CBOR. This is an implementation resource limit, not a spec rule: a valid hike object nests 4 deep. It bounds the work a gate does on unauthenticated bytes before any signature check. See [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md#implementation-resource-limits).
- **No policy values in the SDKs (rc.2).** The spec defines measurements, a policy sets the values. groundinglab's policy is `policy/groundinglab-admission/gl-admit-v9.toml`. The test-plane judgment is a fixture in `services/admission-gate/tests/test_plane_policy.rs`.

## Layout

| Path | What |
|---|---|
| [rust/](rust/) | Cargo workspace: `hike`, `hike-net`, `nip06`, and the programs `satoshi-hike-produce` and `satoshi-hike-verify` |
| [go/](go/) | Go module: package `hike`, programs under `cmd/`, the conformance test under `conformance/` |
| [java/](java/) | Java sources (`pcoc.hike`, `pcoc.json`, `pcoc.cli`, `pcoc.conformance`) and `v2.sh` |
| [examples/](examples/) | Quickstart, produce and live examples in each language, all built by `test.sh`, and the `librecord` programs the library-level cross-check runs |
| [tools/](tools/) | `check_spec_pin.py`, `check_versions.py`, `check_docs.py`, `crosscheck.py` |
| [VERSION](VERSION), [SPEC_PIN](SPEC_PIN) | SDK version; spec tag and vector hash |
| [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md) | Where the SDKs disagree and the open spec questions |

↑ [root README](../README.md)
