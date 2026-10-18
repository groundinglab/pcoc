# PCOC SDKs

Rust, Go and Java SDKs for Satoshi Hike v2.0, the PCOC specification. Each SDK can verify a PCOC (it writes the section 14 measurement record) and produce one. All three are tested against the same conformance vectors, and each is cross-checked against the other two. The admission gate and the desktop app in this repository build on the Rust SDK.

The [specification](../spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md) and its [conformance vectors](../spec/conformance/v2.0/) are the source of truth. Where an SDK disagrees with them, the specification wins and the SDK has a bug.

## Which one

| You are writing | Use | Read |
|---|---|---|
| A Rust service or app, or an Android app through the Rust core (JNI) | Rust: crate `hike` (plus `nip06` for demo keys, `hike-net` for direct node reads) | [rust/README.md](rust/README.md) |
| A Go service or tool | Go: module `github.com/groundinglab/pcoc/sdk/go/v2`, package `hike` | [go/README.md](go/README.md) |
| A JVM service or tool | Java: packages `pcoc.hike` and `pcoc.json` (JDK 21, no build tool required) | [java/README.md](java/README.md) |

The three SDKs give byte-identical records. [examples/](examples/) has the same quickstart in each language, and all three print the same record digest.

## Version and pinning

| | Value |
|---|---|
| SDK version | `2.0.0-rc.1` ([VERSION](VERSION)) |
| Repository tag | `sdk-v2.0.0-rc.1` (Rust and Java pin this) |
| Go module tag | `sdk/go/v2.0.0-rc.1` (the module path requires the `sdk/go/` prefix) |
| Specification | `spec-v2.0.5`, with the vector hash in [SPEC_PIN](SPEC_PIN) |

Pin a tag, never `main`. Inside this repository, the gate and the desktop app pin the SDK by path (`sdk/rust/hike`) and report the version they were built with: the gate in `/v1/health` (`sdk`) and in `admission-service --version`, the desktop app on its About page.

> The `rc.1` tags have been prepared but not yet published. Until they are on the remote, depend on a checkout by path, as `examples/` does.

Earlier tags of these trees (`ref-rust-v2.0.1` … `ref-rust-v2.0.6`, `ref-java-v2.0.1` … `ref-java-v2.0.2`, and the Go module tags under the old module path) are kept as history. Nothing new is tagged under them.

## Conformance vectors: the source of truth

`spec/conformance/v2.0/` holds the vectors: hike objects together with their recorded index replies and the exact section 14 record bytes they must produce (`test-3leg`, `kat-small`, `anchor-window`, `anchor-w16-i17`, `bind-threads`, `earlier-run-b`), known answers at the small `conformance-small` sizes (terrain, one leg, a checkpoint, bookends, the six-leg check), one checkpoint preimage, the P9.14 produce vector (`stage0-start.json`), the section 6 constants, and the section 15.1 sign-in wire vectors. The table in [its README](../spec/conformance/v2.0/README.md) describes each file. A Part A implementation conforms when it reproduces every vector byte for byte.

- [SPEC_PIN](SPEC_PIN) names the spec tag, plus a SHA-256 over every file under `spec/conformance/` except `tools/`. [tools/check_spec_pin.py](tools/check_spec_pin.py) recomputes it. A changed vector without a changed pin fails.
- Rust is the only program that writes the vectors (`PCOC_REGEN_V2=1`, in a spec change only). Go and Java compare against the bytes Rust wrote. All three read the pack from `spec/conformance/v2.0/`. No SDK carries a hand copy.
- The SDKs never edit the vectors. A vector change is a spec change: it comes with a new spec tag and a new `SPEC_PIN`.

## One harness

```sh
bash sdk/test.sh
```

This runs, in order, and exits non-zero if any step fails:

1. the pins: `SPEC_PIN` against `spec/conformance/`, and every Rust crate version against `VERSION`;
2. Rust: `cargo test --locked --release` (unit tests, the conformance pack, malformed-input tests);
3. Go: `go vet ./...` and `go test ./...` (unit tests and the conformance pack);
4. Java: `java/v2.sh` (the conformance pack and the section 6 pins);
5. the cross-check: the Rust, Go and Java verify programs run on the pack objects and 25 mutants, and must write byte-identical records ([tools/crosscheck.py](tools/crosscheck.py));
6. the README quickstarts in `examples/`, which must print the same digest in all three languages.

It needs `python3`, Rust (rustup picks up `rust/rust-toolchain.toml`), Go 1.22 or newer, and JDK 21 or newer. A missing tool is a failure, not a skip. CI runs it as the `sdk` job on the self-hosted runners.

On malformed objects, five cases are known where Rust disagrees with Go and Java. They are listed in [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md) and pinned by the harness, and they wait on a spec reading.

## Layout

| Path | What |
|---|---|
| [rust/](rust/) | Cargo workspace: `hike` (verify, produce, admission helpers), `hike-net` (direct Bitcoin node reads, test-plane policy), `nip06` (DEMO keys), `satoshi-hike-produce` and `satoshi-hike-verify` (programs) |
| [go/](go/) | Go module: package `hike`, programs under `cmd/`, the conformance test under `conformance/` |
| [java/](java/) | Java sources (`pcoc.hike`, `pcoc.json`, `pcoc.cli`, `pcoc.conformance`) and `v2.sh` |
| [examples/](examples/) | The quickstart in each language (run by `test.sh`) |
| [tools/](tools/) | `check_spec_pin.py`, `check_versions.py`, `crosscheck.py` |
| [VERSION](VERSION), [SPEC_PIN](SPEC_PIN) | SDK version, and the spec tag plus vector hash |

## What verify does and does not do

Verify writes the section 14 measurement record. It does not judge. Judgment is policy (specification section 15): an admission gate applies a policy to the record. The only policy code in the SDKs is `hike::admit` and the test-plane policy in `hike-net`. A leg is `2_000_000` steps and ends in a leg result. Produce records 18 stages, paced by Bitcoin block headers.

## Steer

↑ [root README](../README.md) · law ↑ [spec/](../spec/) · consumers → [services/admission-gate](../services/admission-gate/), [apps/desktop](../apps/desktop/)
