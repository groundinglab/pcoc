# Java reference

The Java implementation of Satoshi Hike v2.0. It is one of three independent references. It is a guide, not the specification. Where it disagrees with [Satoshi Hike v2.0](../../spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md), the specification wins and this tree has a bug.

This port does not generate the conformance vectors. `v2.sh` compiles the sources and runs `pcoc.conformance.V2Pack` and `pcoc.hike.Spec6Pins` against `spec/conformance/v2.0/`.

## Tag

The current tag is `ref-java-v2.0.2`.

## Toolchain

JDK 21 or newer is required. JDK 17 does not work. There is no Gradle and no Maven. `reference/java/v2.sh` runs `javac --release 21`.

Check with `javac -version`. It must print 21 or higher. On 17, install a JDK 21 side by side, point `JAVA_HOME` at it, and put `$JAVA_HOME/bin` first on `PATH`. `v2.sh` stops at once with that message when `javac` is older than 21. It does not skip.

## Test

From the repository root:

```sh
bash reference/java/v2.sh
```

## Produce and verify

After `v2.sh` has compiled (or the same `javac --release 21` line), the classes are `pcoc.cli.Produce` and `pcoc.cli.Verify`. Verify writes a section 14 record. Exit status is not an admission decision.

## Spec pin

[../SPEC_PIN](../SPEC_PIN) names `spec-v2.0.3` and the vector hash. Check it with `python3 reference/tools/check_spec_pin.py` from the repository root.

## Demo keys

`--demo-mnemonic` is outside this port. It is not `production_seed`.
