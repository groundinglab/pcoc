# PCOC SDKs

Satoshi Hike 2.3. The specification is [spec/satoshi-hike-v2.3/SPEC.md](../spec/satoshi-hike-v2.3/SPEC.md). Errata 1 and 2 are folded into that file. Where an SDK and that file disagree, the specification wins.

The pack pin is [SPEC_PIN](SPEC_PIN): tag `spec-2.3.0` and the SHA-256 of [spec/conformance/pack/manifest.json](../spec/conformance/pack/manifest.json). There is no erratum key. The pack lists the vectors the four SDKs agree on, plus the Rust sign-in wire. The SDK version in [VERSION](VERSION) is `2.3.1`, a patch version that implements SPEC 2.3. The same file inside tag `spec-2.3.0` still carries the 2.2 pin line and is not the SDK pin; read this one.

## Which SDK

The Rust workspace in this tree is [sdk/rust](rust/).

| You are writing | Use |
|---|---|
| A Rust verifier, a scripted offline producer, or the `pcoc` command | `sdk/rust`: crates `pcoc` and `pcoc-produce`, binary `pcoc` |
| C, Go, or Java | That language's own Phase A tree |

`pcoc` is the construction, the section 7 encodings, the section 14 verify record, the draws, the integer comparisons, and the Hashcash digest. `pcoc-produce` is the offline producer on the scripted rig. The `pcoc` command is `verify`, `selftest`, `candidates`, and `produce --rig`.

A live header source, a Bari client, the sign-in wire (section 15.1), and a policy reader are not in this build.

## Build

Rust 1.99.0 (`sdk/rust/rust-toolchain.toml`). From `sdk/rust`:

```sh
cargo build -p pcoc -p pcoc-cli --release
cargo test -p pcoc --test ac_a9
```

Release builds target the baseline ISA (`target-cpu=x86-64` in `sdk/rust/.cargo/config.toml`). SHA-256 uses the `sha2` crate, which selects SHA-NI at runtime when the host has it. `pcoc selftest` prints the backend actually in use.

`build_info()` reports the SDK version, the spec tag, the pack pin, and that SHA-256 backend.
