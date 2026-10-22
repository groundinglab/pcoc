# PCOC on Android

A demonstrator, not part of the specification. The app is a fork of Primal's Android client. You can make a PCOC on the phone, check one on the phone, and sign in with one. Making and checking use the Rust reference, vendored in the fork and called through JNI. Checking writes a section 14 measurement record. It does not say pass or fail, and it does not apply admission policy.

Sign-in is a different door. The phone signs the enrolled object and POSTs the section 15.1 body to `https://admission.groundinglab.io/v1/sign-in`. The Admission Gate decides. The phone shows that decision. A measurement on the verify screen is not a policy decision, and a policy decision is not a verifier result.

The fork is [groundinglab/primal-pcoc](https://github.com/groundinglab/primal-pcoc). The current demonstrator build is tag [`android-v3.5.46`](https://github.com/groundinglab/primal-pcoc/tree/android-v3.5.46) (commit `4d35661`). Its debug APK is published on the [demonstrations page](https://groundinglab.io/demonstrations/) with SHA-256 `b211a7d0485b0a11e056976cda83036f6dff821f5afd8d0a0be7391021fb5a79`.

## Three doors

Make. `app/src/main/kotlin/net/primal/android/pcoc/create/CreatePcocViewModel.kt` starts the session (`CreateUiEvent.Start` calls `session.start()`). `create/ProduceSession.kt` calls `PcocNative.startProduce`. `pcoc-bridge/bridge/src/produce.rs` calls `hike::produce_with_settlement_sized`. `app/src/main/kotlin/net/primal/android/pcoc/bridge/PcocNative.kt` sets `BARI_URL` to `https://bari.groundinglab.io`.

Check. `verify/VerifyPcocViewModel.kt` calls `engine.verify` with `INDEX_URL`. `bridge/VerifyEngine.kt` calls `PcocNative.verify`. `PcocNative.kt` sets `INDEX_URL` to `https://index.groundinglab.io` and declares the JNI `verify` method. `pcoc-bridge/bridge/src/verify.rs` `verify_object` uses `hike::Sizes::production()` and profile `production`, and returns the section 14 record. It does not apply section 15. The phone does not select `conformance-small`.

Sign in. `admit/AdmissionPayload.kt` is the section 15.1 body (`origin`, `nonce`, `exp`, `object`, `object_b64`, `signature`). `admit/AdmissionDoorClient.kt` POSTs that body once to `SIGN_IN_DOOR`, `https://admission.groundinglab.io/v1/sign-in`.

## Acknowledgement

primal-pcoc is a fork of [PrimalHQ/primal-android-app](https://github.com/PrimalHQ/primal-android-app), Primal's Android Nostr client, by Primal Systems Inc. The PCOC doors are groundinglab's. The app is theirs.

- Upstream licence: MIT. Copyright (c) 2023 PRIMAL SYSTEMS INC. The fork keeps `LICENSE` and `NOTICE.md`.
- Product: [primal.net](https://primal.net). This fork is not an official Primal release and is not endorsed by Primal Systems Inc.
- groundinglab's changes, as shipped on the fork: the `net.primal.android.pcoc` doors (enroll, local produce, local verify, sign-in), the `pcoc-bridge` JNI crate and its vendored `hike`, and `docs/pcoc/`. The complete file list against the upstream commit is in the fork's README. This page does not guess that list.

This repository does not vendor the Android app. The fork's copy is outside this tree. Retarget that fork onto the 2.2 Rust SDK at `sdk/rust`, and onto `pcoc-ffi` when Phase D lands it. That retarget is a follow-up; it is not this change.

## Rebuild and verify

The command below is the fork's debug assemble. It will not reproduce another machine's APK byte for byte. Successive builds already differ. There is no pinned-NDK reproducibility build, and no claim that one is possible today. Builds are not yet reproducible.

On a machine with the Android SDK, JDK 21, and the NDK:

```sh
git clone https://github.com/groundinglab/primal-pcoc.git
cd primal-pcoc
git checkout android-v3.5.46
./pcoc-bridge/build-android.sh
./gradlew :app:assembleAospDebug
```

The APK is the `aospDebug` output under `app/build/outputs/apk/aosp/debug/`. The file name used for that debug assemble is `primal-pcoc-aosp-debug.apk`. It is debug-signed. Compare a rebuild with the procedure in [VERIFY.md](VERIFY.md).

When a release is built from a named commit, that release carries the SHA-256 of the APK. Check the asset against the sum published with that release. This directory does not ship a `SHA256SUMS` file, and it does not put an APK in git.

Toolchain read from the fork at the commit this page was written against (versionName 3.5.46; confirm on the remote before treating the table as current):

| Tool | Version | Where it is set |
|---|---|---|
| versionName / versionCode | 3.5.46 / 2743 | `app/build.gradle.kts` |
| JDK | 21 | `sourceCompatibility` / `targetCompatibility` |
| Android Gradle Plugin | 9.2.1 | `gradle/libs.versions.toml` |
| Gradle | 9.6.0 | `gradle/wrapper/gradle-wrapper.properties` |
| Kotlin | 2.4.0 | `gradle/libs.versions.toml` |
| compileSdk / targetSdk / minSdk | 37 / 36 / 26 | `gradle/libs.versions.toml` |
| NDK | 26.3.11579264 if `ANDROID_NDK_HOME` is unset | `pcoc-bridge/build-android.sh` |

The APK is signed by the Android debug key. `playRelease` and `altRelease` fall back to that debug key when no upload keystore is configured. The plain `release` build type has no signing config. The `benchmark` build type uses the debug key. This is a demonstrator signature. It is not a Play App Signing key. Do not ship this signature as a production identity.

## Status

Local verify needs the public index. Produce posts waypoints to the public Bari. Neither call is optional in the current doors. Cost at scale is not established. See [STATUS.md](../../STATUS.md).

## Licence

The app follows the fork: MIT, upstream copyright intact. groundinglab's additions on the fork are MIT unless a file says otherwise. This page is CC BY 4.0.
