# W1 evidence — Windows producer core

Host: `pepebook` (Linux). Branch: `desktop/windows-w1`. Tree: `apps/desktop-windows/`. Crate: `pcoc-windows-core`. No push, no merge, no PR.

Commands were run from `apps/desktop-windows/` on 27 Oct 2026.

## Commands

| Command | Exit |
|---|---|
| `cargo test --offline` | 0 |
| `cargo build --offline --target x86_64-pc-windows-gnu` | 0 |

`cargo test` ran 18 lib tests, 18 passed, plus doc-tests (0). The cross build finished a `dev` profile `rlib` for `x86_64-pc-windows-gnu` (`libpcoc_windows_core.rlib`). Linker: `x86_64-w64-mingw32-gcc`. Rust target was already installed. Wine was not used.

## W1-AC1–AC9

| AC | Result | Proof |
|---|---|---|
| W1-AC1 Compiles on named host | met | Host `pepebook`. `cargo build --offline --target x86_64-pc-windows-gnu` exit 0. Core crate only. No window crate. |
| W1-AC2 SPEC / SDK pin | met | `core/src/pin.rs` and `pin::tests`: SPEC `2.3`, `STAGES` 18, kind `v2.2-pcoc-object`, SDK `2.3.1` from in-tree `pcoc::VERSION` and `sdk/VERSION`. Path deps in the workspace manifest. |
| W1-AC3 Keys | met | `keys` tests: generate, nsec, and hex secret derive the same public identifier. `Debug` prints `SecretKey([redacted])`. `Prefs::load` uses `deny_unknown_fields`, so a planted `nsec` / secret hex document falls back to defaults and is not re-saved. The published NIP-19 example in `vectors-allowlist/` is the public test vector, same as the Linux desktop. |
| W1-AC4 Bari miss continues | met | `produce::tests::bari_post_failure_is_recorded_and_the_object_is_still_saved`: 17 simulated waypoint POST failures, each with an error string, `stages_closed_after_post_errors == 17`, object kind `v2.2-pcoc-object`, `.cbor` written. |
| W1-AC5 Measurement only | met | `verify::tests::saved_fixture_returns_a_measurement_and_not_a_gate_word`: `pcoc::verify` JSON has `shape` and no `admitted` / `refused`. `verify::admission::opinion()` is `NotAsked`. |
| W1-AC6 Sign-in door | met | Origin `https://groundinglab.io`. POST `https://admission.groundinglab.io/v1/sign-in`. Sign-in module source (above the tests) does not name the result URL. |
| W1-AC7 History cleartext + always save | met | Default display `%USERPROFILE%\PCOC\hikes\`. `history_root` uses `USERPROFILE` when `HOME` is unset. Finished produce writes `{public-hex}.cbor`. README states cleartext now, encryption later. No encryption API. |
| W1-AC8 Defaults, no auto badge | met | Endpoint constants match Linux `apps/desktop/core/src/endpoints.rs`. `claims::auto_claims` returns `ClaimsOmitted::AskA1`. Claims module has no claims URL and no `/badge` or `/name` request. |
| W1-AC9 Local build | met | Built on `pepebook` by Grok Build. No Cursor cloud agent. Evidence names this host. |

## Out of scope (not done)

UI toolkit, window crate, GTK changes, NIP-04 DM send, Wine, emulator, MSI, auto claims POST, land/ship.
