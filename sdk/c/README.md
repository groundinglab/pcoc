# Satoshi Hike 2.3, C

Baseline ISA. SHA-NI is selected at runtime.

## Test

From this directory the full suite is:

    make check

That is the command CI runs. It builds the library, the `pcoc` tool, and the candidate generator, then runs every conformance check.

`check/negatives.sh` checks that negative constructions stay out of the release library and the `pcoc` binary. `make check` runs it. After a build it can also be run on its own:

    sh check/negatives.sh

The checks locate the repository by walking up to the directory that holds `sdk/VERSION` and `spec/`. If that directory is not found, the check fails.

## Quickstart

From this directory, after `make`:

```sh
./build/pcoc quickstart
```

Six lines. The first names `c`. The second names the SHA-256 backend on this machine. The other four lines are the same text the Rust, Go, and Java quickstarts print. The production-leg line is the pinned `legs_le_hex`. The command does not time a leg.
