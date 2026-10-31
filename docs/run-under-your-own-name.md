# Run it under your own name

Every groundinglab host in this repository is an example deployment, not a requirement. This page lists each place a builder changes to run the pieces for their own site. File and line numbers were checked against `main` on 2026-10-16.

Nothing here is a config system. Some values are settings you already have. Others are constants you edit and then rebuild.

## Sign-in doors are client build configuration

Each signing client is built for the deployments it serves. The Sign-in door URL for each origin the client accepts is part of that client's build: there is no discovery. A client MUST NOT derive or guess a door URL from the origin, and MUST refuse a link whose origin has no configured door (SPEC §15.1 **Submit**). To serve your site, add your origin and its door to the client build.

## Admission Gate (`services/admission-gate`)

The Gate's sign-in origin is a single constant. To run a gate for your site, edit it and rebuild.

| What | Where | Change |
|---|---|---|
| Signed sign-in origin | `src/wire.rs:14` `pub const ORIGIN: &str = "https://groundinglab.io";` | Your site's origin, serialized as in SPEC §15.1 **Origin** (for example `https://example.com`) |
| Where it is checked | `src/door.rs:242` (check-order step 2) | Nothing. It compares against `wire::ORIGIN` |
| Browser CORS | `allowed_origins`, `extra_origins` in your gate TOML (`src/config.rs`, `src/http.rs:672`) | Your website's origins. These are separate from the signed origin and default to loopback only |
| Index the Gate reads | `index_url` in your gate TOML (`deploy/gate.toml:13`) | Your own `pcoc-index`, or any index you trust |
| Deploy file | `deploy/gate.toml:54` | groundinglab's deploy file. Copy it and edit it for your hosts |

## Rust SDK (`sdk/rust/hike`)

| What | Where | Change |
|---|---|---|
| Live defaults | `src/consts.rs:46,49` (`LIVE_BARI_URL`, `LIVE_INDEX_URL`), used as `DEFAULT_BARI_URL` and `DEFAULT_INDEX_URL` at `:52-53` | Nothing. Pass `--bari-url` or `--index-url` |
| Live-profile host check | `src/args.rs:413-420` (produce, `--bari-url`) and `src/args.rs:539-546` (verify, `--index-url`) | Nothing. Any `https` origin with no path is accepted. A host other than groundinglab's prints a warning and proceeds |
| Test-plane guard | `src/plane.rs:5-9` | Nothing. It only stops the test profile from reaching groundinglab's live hosts |

## Desktop app (`apps/desktop/core`)

| What | Where | Change |
|---|---|---|
| Sign-in origin | `src/signin.rs:14` `ORIGIN` | Your site's origin |
| Accepted link prefixes | `src/signin.rs:26-27` (`LINK_PREFIX`, `RETIRED_PREFIX`) and the display form at `:117` | Your origin plus `/sign-in/?v=1&nonce=` |
| Sign-in door for the origin | `src/endpoints.rs:16` `SIGN_IN` (the door for `signin.rs:14` `ORIGIN`) | Your Gate's `/v1/sign-in` URL, paired with your origin |
| Hosts | `src/endpoints.rs:7` `ADMISSION`, `:9` `BARI`, `:11` `INDEX`, `:13` `VERIFY`, `:16` `SIGN_IN` | Your Gate, Bari, index and verify hosts. The asserts at `:29-32` change with them |

## Android app (`groundinglab/primal-pcoc`)

| What | Where | Change |
|---|---|---|
| Sign-in origin | `app/src/main/kotlin/net/primal/android/pcoc/admit/SignInLink.kt:102` `SIGN_IN_ORIGIN` | Your site's origin |
| Link checks | `SignInLink.kt:68` (the `www` host message) and the link forms in the comments at `:7` and `:31` | Your host |
| Sign-in door for the origin | `admit/AdmissionDoorClient.kt:175` `SIGN_IN_DOOR` (the door for `SignInLink.kt:102` `SIGN_IN_ORIGIN`), with `:176` `SIGN_IN_RESULT` | Your Gate's URLs, paired with your origin |
| Notices shown to people | `admit/SignInNotices.kt:33,35,36,79,81` | Your Gate's host name |
| Bari and index | `bridge/PcocNative.kt:4` `INDEX_URL`, `:7` `BARI_URL`; `pcoc-bridge/bridge/src/lib.rs:15` `BARI_URL`, `:17` `INDEX_URL` (from `hike::DEFAULT_INDEX_URL`) | Your Bari and index |
| App Link | `app/src/main/AndroidManifest.xml:126` `android:host="groundinglab.io"` | Your host. App Links also need your site to publish its own asset links file |
| Vendored reference | `pcoc-bridge/vendor/hike/` | An earlier copy of `sdk/rust/hike` (vendored at `ref-rust-v2.0.5`). It still has the old live-host check, so update it from `sdk/rust/hike` |

## Admission claims (`services/admission-claims`)

All of these are defaults you can override in config. If you leave one unset, your deployment publishes groundinglab's value.

| What | Where |
|---|---|
| Claims origin | `src/config.rs:11` `DEFAULT_CLAIMS_ORIGIN` |
| Badge id, name and description | `src/config.rs:15,16,18` |
| Badge images | `src/config.rs:19,20` |
| Relay | `src/config.rs:23` `DEFAULT_RELAY` |
| Storage buckets | `src/config.rs:24,29` |
| Public site | `src/config.rs:26` `DEFAULT_PUBLIC_SITE` |
| Allowed origins | `src/config.rs:39` `DEFAULT_ORIGINS` |
| Worker routes and bucket | `services/cf-claims-worker/wrangler.jsonc:5-17`, a deploy file for groundinglab's zone. Its header says to copy it and edit it for your zone |

## Portable index (`services/index`)

| What | Where | Change |
|---|---|---|
| Browser CORS | `src/lib.rs:38-41` `origin_allowed` allows `https://groundinglab.io`, `https://www.groundinglab.io` and loopback | Edit the two origins to your website's. There is no flag for it |

## Bari (`services/bari`)

| What | Where | Change |
|---|---|---|
| Fees address on the operator page | `src/operator.rs:2467` `FEES_LIGHTNING_ADDRESS` | Yours |
| Operator page header text | `src/operator.rs:2023` | Your name |

## The spec

SPEC §15.1 lets any operator run an admission service for its own origin and compares origins exactly. `https://groundinglab.io` is the example deployment. The origin vectors are `spec/conformance/pack/sign-in-origins.json`: the same challenge signed for `https://example.com` and for near misses, and what a service that serves only `https://example.com` returns for each.
