# Rust SDK

Satoshi Hike 2.2. The construction and the verifier are the `pcoc` crate. `pcoc-produce` is the scripted offline producer. The `pcoc` command is `verify`, `selftest`, `candidates`, and `produce --rig`.

The specification is [spec/satoshi-hike-v2.2/SPEC.md](../../spec/satoshi-hike-v2.2/SPEC.md), read with [ERRATUM-1.md](../../spec/satoshi-hike-v2.2/ERRATUM-1.md). Where this tree and those files disagree, the specification wins.

Rust 1.99.0 (`rust-toolchain.toml`). Release and test builds target the baseline ISA (`target-cpu=x86-64` in `.cargo/config.toml`). SHA-256 uses the `sha2` crate, which selects SHA-NI at runtime when the host has it.

## Test

From this directory, the full suite:

```sh
CARGO_BUILD_JOBS=4 cargo test --locked --release -- --test-threads=4
```

The checkout must contain `sdk/VERSION` and `spec/`. Each test walks up from its crate to that directory and fails if the walk does not find both. The suite does not read a path outside the repository.

Candidate files under `candidates/` and objects under `objects/` are checked against SHA-256 pins in the tests. The generator is re-run and its output must match the committed files byte for byte. Each object is verified against the CASES synthetic index, which the test harness serves on loopback.

Then run the check scripts:

```sh
sh check/negatives.sh
```

`check/negatives.sh` is also run by the `ac_a10` test. It builds the release library and the `pcoc` binary in its own target directory.
