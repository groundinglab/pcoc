# Java SDK

JDK 21. From this directory the full test command is:

```sh
./gradlew test
```

That is what CI runs. It needs a clone of this repository and nothing beside it. `JAVA_HOME` must point at a JDK 21: `check/negatives.sh` refuses to run without it, and the suite runs that script.

Each test walks up from the working directory to the directory that holds `sdk/VERSION` and `spec/`. If that directory is not there, the test fails. It does not skip.

`check/negatives.sh` checks that the release jar does not contain the negatives generator. Run it on its own only after `./gradlew jar` has built the release jar.

The files in `candidates/` and `objects/` are the committed round-2 bytes. The suite regenerates them, checks them byte for byte, and checks each file's SHA-256 against the pins in the tests. Section 14 of the committed objects is measured against the CASES synthetic index, which the test harness serves on loopback.
