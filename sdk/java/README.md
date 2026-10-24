# Java SDK

JDK 21. From this directory the full test command is:

```sh
./gradlew test
```

That is what CI runs. It needs a clone of this repository and nothing beside it. `JAVA_HOME` must point at a JDK 21: `check/negatives.sh` refuses to run without it, and the suite runs that script.

Each test walks up from the working directory to the directory that holds `sdk/VERSION` and `spec/`. If that directory is not there, the test fails. It does not skip.

`check/negatives.sh` checks that the release jar does not contain the negatives generator. Run it on its own only after `./gradlew jar` has built the release jar.

The files in `candidates/` and `objects/` are the committed round-2 bytes. The suite regenerates them, checks them byte for byte, and checks each file's SHA-256 against the pins in the tests. Section 14 of the committed objects is measured against the CASES synthetic index, which the test harness serves on loopback.

This SDK is version 2.3.1 (Rule I5) and implements SPEC 2.3 (tag `spec-2.3.0`). `BuildInfo.VERSION` and `Version.STRING` are `2.3.1`. The wire `KIND` stays `v2.2-pcoc-object`. The release tag for this language is `sdk-java-v2.3.1`. `pcoc --version` prints `2.3.1`, then the `tag=` line of `sdk/SPEC_PIN`, then that file's `vector-sha256`. The release job owns the pin file; this tree does not set it.

## Quickstart

From this directory:

```sh
./gradlew run --quiet --args=quickstart
```

The run uses `-XX:+UnlockDiagnosticVMOptions`, which is how this SDK reports `sha-ni` when the host has it. Six lines. The first names `java`. The second names the SHA-256 backend on this machine. The other four lines are the same text the Rust, C, and Go quickstarts print. The production-leg line is the pinned `legs_le_hex`. The command does not time a leg.
