# reference

Three independent implementations of the PCOC specification, in Rust, Go, and Java. Each is tested against the shared vectors under [spec/conformance/v2.0](../spec/conformance/v2.0/). That is the evidence the specification is complete enough to build from. They are the first tested examples, a guide, and optional. Where a reference disagrees with the specification, the specification wins and the reference has a bug.

They follow [SPEC_PIN](SPEC_PIN): tag `spec-v2.0.3` (proposed, not yet tagged), and the vector hash recorded on the next line. [tools/check_spec_pin.py](tools/check_spec_pin.py) recomputes that hash from `spec/conformance/` (every file except `spec/conformance/tools/`) and fails on a mismatch. Run it from the repository root: `python3 reference/tools/check_spec_pin.py`.

Verify writes the section 14 measurement record. It does not judge. Judgment is policy (specification section 15) and is not this folder, except `hike-net`, which is network and test-plane policy kept out of the measurement crate.

Rust generates the vectors (`PCOC_REGEN_V2=1`). Go and Java do not. They compare against the bytes Rust wrote.

## Read order

Open [Satoshi Hike v2.0](../spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md) first. Then any of the three. None of them is the specification.

## What the programs do

A leg is `2_000_000` steps and ends in a leg result. Produce records 18 stages. The tip these programs read is the public sequencer: it paces stages and orders them. Payment, confirmation counts, and an archive of the chain sit outside that role.

Rule A4.3 checkpoint comparisons run together. `PCOC_BIND_THREADS` sets how many run at once. Unset, empty, or not a positive integer uses 6. `1` runs them one after another. The verify record does not depend on that number. Section 13.1’s `ANCHOR_WINDOW_OFFSETS` (2, 3, 4) is the only offset list the producer and the verifier use.

## Languages

Each tag below is cut.

| Language | Tag | Toolchain | Test | Binaries |
|---|---|---|---|---|
| Rust | `ref-rust-v2.0.4` (services and desktop pin it; the index through `bari-v0.6.7`) | Rust 1.99.0 (`rust/rust-toolchain.toml`). Crate `rust-version` 1.83 (`rust/hike/Cargo.toml`) | `cargo test --locked --release` in `reference/rust` | `satoshi-hike-produce`, `satoshi-hike-verify` |
| Go | `reference/go/v2.0.3`. The module path forces this tag shape (`go/go.mod`, module `github.com/groundinglab/pcoc/reference/go/v2`). There is no `ref-go-` tag | Go 1.22 (`go/go.mod`). No third-party dependencies | `go vet ./...` and `go test ./...` in `reference/go` | `cmd/satoshi-hike-produce`, `cmd/satoshi-hike-verify` |
| Java | `ref-java-v2.0.2` | Java 21. No build tool. `javac --release 21` (`java/v2.sh`) | `bash reference/java/v2.sh` | `pcoc.cli.Produce`, `pcoc.cli.Verify` (`v2.sh` runs `pcoc.conformance.V2Pack` and `pcoc.hike.Spec6Pins`) |

## Index

| Child | Role |
|---|---|
| [rust/](rust/) | Cargo workspace for the Rust reference. |
| [rust/hike/](rust/hike/) | Shared library: fill, leg, bind, hike object, path body, and Hashcash. |
| [rust/hike-net/](rust/hike-net/) | Network and policy code kept out of measurement: the test-plane policy and the direct Bitcoin node read. |
| [rust/nip06/](rust/nip06/) | DEMO keys only. BIP-39 English 12-word, empty passphrase, NIP-06 path, BIP-340 x-only. Not the hike's `production_seed`. |
| [rust/satoshi-hike-produce/](rust/satoshi-hike-produce/) | Reference produce program. Lab `--demo-mnemonic` derives the x-only key. |
| [rust/satoshi-hike-verify/](rust/satoshi-hike-verify/) | Reference verify program. Calls `hike::verify`. |
| [go/](go/) | Go port. Module path `github.com/groundinglab/pcoc/reference/go/v2`; current tag `reference/go/v2.0.3`. |
| [java/](java/) | Java port. `reference/java/v2.sh` runs the conformance pack. |

## Steer

↑ [root README](../README.md) · law ↑ [spec/](../spec/) · services → [services/](../services/)
