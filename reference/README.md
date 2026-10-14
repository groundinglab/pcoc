# reference

Rust, Go, and Java reference programs for Satoshi Hike v2.0.

## Not here

Hike construction law is [spec/](../spec/). Admission policy is a relying-party choice, outside this tree. The visitor story is the [root README](../README.md).

## Read order

Open [Satoshi Hike v2.0](../spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md) first. Then the Rust workspace under [rust/](rust/).

## Pins

The programs follow [Satoshi Hike v2.0](../spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md). `satoshi-hike-verify` calls `hike::verify`. Measurements and judgment are that hike specification.

A leg is `2_000_000` steps and ends in a leg result. The checkpoint sample is 1024 bytes. Produce records 18 stages. The tip these programs read is the public sequencer: it paces stages and orders them. Payment, confirmation counts, and an archive of the chain sit outside that role. Verify measures the object. Whether a relying party requires the object for admission, a rate limit, or a Sybil cost is that party's choice.

Rule A4.3 checkpoint comparisons run together. `PCOC_BIND_THREADS` sets how many run at once. Unset, empty, or not a positive integer uses 6. `1` runs them one after another. The verify record does not depend on that number. Section 13.1’s `ANCHOR_WINDOW_OFFSETS` (2, 3, 4) is the only offset list the producer and the verifier use.

## Spec pin

[SPEC_PIN](SPEC_PIN) names the spec tag these programs follow, `spec-v2.0.1`, and the conformance vector hash at that tag. `tools/check_spec_pin.py` recomputes the hash from `conformance/v2.0/` and fails on a mismatch.

## Index

| Child | Role |
|---|---|
| [rust/](rust/) | Cargo workspace for the Rust reference. |
| [rust/hike/](rust/hike/) | Shared library: fill, leg, bind, hike object, path body, and Hashcash. |
| [rust/hike-net/](rust/hike-net/) | Network and policy code kept out of measurement: the test-plane policy judgment and the direct Bitcoin node read. |
| [rust/nip06/](rust/nip06/) | DEMO keys only. BIP-39 English 12-word, empty passphrase, NIP-06 path, BIP-340 x-only. Not the hike's `production_seed`. |
| [rust/satoshi-hike-produce/](rust/satoshi-hike-produce/) | Reference produce program. Lab `--demo-mnemonic` derives the x-only key. |
| [rust/satoshi-hike-verify/](rust/satoshi-hike-verify/) | Reference verify program. Calls `hike::verify`. |
| [go/](go/) | Go port. Module path `github.com/groundinglab/pcoc/reference/go/v2`; current tag `reference/go/v2.0.2`. |
| [java/](java/) | Java port. `reference/java/v2.sh` runs the conformance pack. |

```text
cargo test --manifest-path reference/rust/Cargo.toml
cargo build --release --manifest-path reference/rust/Cargo.toml
```

Release binaries:

- `reference/rust/target/release/satoshi-hike-produce`
- `reference/rust/target/release/satoshi-hike-verify`

## Steer

↑ [root README](../README.md) · law ↑ [spec/](../spec/) · services → [services/](../services/)
