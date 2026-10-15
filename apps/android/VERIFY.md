# Comparing two Android builds

This page is how to compare two APKs without claiming the build is reproducible. There is no reproducible-build badge. Builds are not yet reproducible. Equal stripped contents are not promised.

A release built from a named commit will carry the SHA-256 of that APK. This directory does not publish that sum, and it does not contain the APK. `sha256sum -c` is not a CI step here.

1. Run `apksigner verify --print-certs` on each APK. Expect the Android debug certificate on a debug assemble of this fork (`aospDebug`, and `playRelease` or `altRelease` when they fall back to the debug key). Record the certificate digest in the comparison notes.
2. Strip the signature. Remove `META-INF` and the APK signing block, then compare the zip entries. `diffoscope` on the two stripped APKs is the readable report.
3. Equal stripped contents and different signatures means the same payload, signed by a different key. Different stripped contents means the build is not the same. That is the expected result today.
4. Do not publish a reproducible-build badge.
