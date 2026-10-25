# PCOC on Android

A demonstrator, not part of the specification. The app is a fork of Primal's Android client. You can make a PCOC on the phone, check one on the phone, and sign in with one. It implements SPEC 2.3 (tag `spec-2.3.0`).

The published debug APK on the [Apps page](https://groundinglab.io/apps/) is version 3.5.50 (versionCode 2747), primal-pcoc commit [`5a1d4f5`](https://github.com/groundinglab/primal-pcoc/commit/5a1d4f5f3c06cd92cfc1ae758418a47122853d3a), SHA-256 `6fd62c90ca78b3905d401b63fbe48dc3dcaf81ef97e3131b03dc7a664abbe69c`. That commit is [primal-pcoc #54](https://github.com/groundinglab/primal-pcoc/pull/54), which is open. There is no `android-v3.5.50` tag. The last release tag is [`android-v3.5.49`](https://github.com/groundinglab/primal-pcoc/tree/android-v3.5.49) (commit `7443f44`). The JNI bridge builds on the Rust SDK tag [`sdk-rust-v2.3.1`](https://github.com/groundinglab/pcoc/releases/tag/sdk-rust-v2.3.1) (crates `pcoc` and `pcoc-produce`, set in `pcoc-bridge/bridge/Cargo.toml`).

Sign-in is a different door. The phone signs the enrolled object and POSTs the section 15.1 body to `https://admission.groundinglab.io/v1/sign-in`. The Admission Gate decides. The phone shows that decision. A measurement on the verify screen is not a policy decision, and a policy decision is not a verifier result.

## Three doors

Make. `pcoc-bridge/bridge/src/produce.rs` runs the SDK producer. Produce posts waypoints to `BARI_URL`, `https://bari.groundinglab.io` (`app/src/main/kotlin/net/primal/android/pcoc/bridge/PcocNative.kt`).

Check. `bridge/VerifyEngine.kt` calls `PcocNative.verify`. `pcoc-bridge/bridge/src/verify.rs` calls `pcoc::verify` against `INDEX_URL`, `https://index.groundinglab.io`, and returns the section 14 record. It does not apply section 15.

Sign in. `admit/AdmissionPayload.kt` is the section 15.1 body (`origin`, `nonce`, `exp`, `object`, `object_b64`, `signature`). `admit/AdmissionDoorClient.kt` POSTs that body once to `SIGN_IN_DOOR`, `https://admission.groundinglab.io/v1/sign-in`.

## Acknowledgement

primal-pcoc is a fork of [PrimalHQ/primal-android-app](https://github.com/PrimalHQ/primal-android-app), Primal's Android Nostr client, by Primal Systems Inc. The PCOC doors are groundinglab's. The app is theirs.

- Upstream licence: MIT. Copyright (c) 2023 PRIMAL SYSTEMS INC. The fork keeps `LICENSE` and `NOTICE.md`.
- Product: [primal.net](https://primal.net). This fork is not an official Primal release and is not endorsed by Primal Systems Inc.
- groundinglab's changes, as shipped on the fork: the `net.primal.android.pcoc` doors (enroll, local produce, local verify, sign-in), the `pcoc-bridge` JNI crate on the pcoc Rust SDK, and `docs/pcoc/`. The complete file list against the upstream commit is in the fork's README. This page does not guess that list.

This repository does not vendor the Android app. The fork's copy is outside this tree.

## Build and verify

The build guide, with toolchain pins, is the fork's [README at `android-v3.5.49`](https://github.com/groundinglab/primal-pcoc/blob/android-v3.5.49/README.md). The published 3.5.50 APK was built from commit `5a1d4f5`, not from that tag. In short, on a machine with the Android SDK, JDK 21, and the NDK, the tagged build is:

```sh
git clone https://github.com/groundinglab/primal-pcoc.git
cd primal-pcoc
git checkout android-v3.5.49
mkdir -p pcoc-bridge/target
./pcoc-bridge/build-android.sh
./gradlew :app:assembleAospDebug
```

The APK is the `aospDebug` output under `app/build/outputs/apk/aosp/debug/`. Builds are not yet reproducible byte for byte. Check the published APK against the SHA-256 on the Apps page, and compare a rebuild with the procedure in [VERIFY.md](VERIFY.md).

The APK is signed by the Android debug key. `playRelease` and `altRelease` fall back to that debug key when no upload keystore is configured. The plain `release` build type has no signing config. The `benchmark` build type uses the debug key. This is a demonstrator signature. It is not a Play App Signing key. Do not ship this signature as a production identity.

## Status

Local verify needs the public index. Produce posts waypoints to the public Bari. Neither call is optional in the current doors. Cost at scale is not established. See [STATUS.md](../../STATUS.md).

## Licence

The app follows the fork: MIT, upstream copyright intact. groundinglab's additions on the fork are MIT unless a file says otherwise. This page is CC BY 4.0.
