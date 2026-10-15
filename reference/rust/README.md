# Rust reference

The Rust implementation of Satoshi Hike v2.0. It is one of three independent references. It is a guide, not the specification. Where it disagrees with [Satoshi Hike v2.0](../../spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md), the specification wins and this tree has a bug.

Rust is the program that generates the conformance vectors. `PCOC_REGEN_V2=1` rewrites `spec/conformance/v2.0/`. Go and Java do not generate those bytes. Leave that variable unset when you are checking, not regenerating.

## Tag

Services and the desktop app pin `ref-rust-v2.0.3` today. `ref-rust-v2.0.4` is cut on the merge commit of the publication branch. It is not cut in this tree.

## Toolchain

`rust-toolchain.toml` pins Rust 1.99.0. The `hike` crate sets `rust-version` to 1.83.

## Test

From this directory:

```sh
cargo test --locked --release
```

The conformance pack is included. Do not set `PCOC_REGEN_V2` for that run.

## Produce and verify

```sh
cargo build --release --locked
./target/release/satoshi-hike-produce --help
./target/release/satoshi-hike-verify --input OBJECT --index-url URL
```

`satoshi-hike-verify` writes the section 14 measurement record. It does not apply admission policy.

## Spec pin

[../SPEC_PIN](../SPEC_PIN) names `spec-v2.0.2` and the vector hash. Check it with `python3 ../tools/check_spec_pin.py` from `reference/`.

## Demo keys

`nip06` holds DEMO keys only: BIP-39 English 12-word, empty passphrase, NIP-06 path, BIP-340 x-only. Those keys are not the hike's `production_seed`. `--demo-mnemonic` on the produce program is a lab derivation of the x-only key. It is not a production seed.
