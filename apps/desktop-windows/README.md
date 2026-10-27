# PCOC Windows producer core (W1)

Headless producer core for a future Windows app. This tree is `apps/desktop-windows/`. It does not replace the Linux GTK app in `apps/desktop`.

W1 is compile and harness only. There is no window crate, no UI toolkit, no NIP-04 direct message, no Wine check, and no installer.

## Pin

| Item | Value |
|---|---|
| SPEC | 2.3 |
| `STAGES` | 18 |
| Object kind | `v2.2-pcoc-object` |
| SDK | 2.3.1 (`sdk/VERSION`) |
| SDK crates | path deps `../../sdk/rust/pcoc` and `../../sdk/rust/pcoc-produce` |

The crate is `pcoc-windows-core`. Rust is pinned by `rust-toolchain.toml` in this folder (1.99.0), the same channel the Linux desktop uses.

## Endpoints

Defaults match the Linux desktop. Lab mirrors are overrides only.

| Name | URL |
|---|---|
| Sign-in origin | `https://groundinglab.io` |
| Sign-in `POST` | `https://admission.groundinglab.io/v1/sign-in` |
| Admission | `https://admission.groundinglab.io` |
| Bari | `https://bari.groundinglab.io` |
| Index | `https://index.groundinglab.io` |
| Verify (named; measurement is in-process) | `https://verify.groundinglab.io` |

The client under test does not poll `/v1/sign-in/result/`.

## Ask A1 — no auto NIP-05 or badge

Auto NIP-05 and badge are omitted. `claims::auto_claims` fails closed and does not build a claims request. Claims happen when the user logs in manually elsewhere. This tree has no `POST` to a claims host.

## History

The Windows default is `%USERPROFILE%\PCOC\hikes\`. Files there are **cleartext**. Encryption is later. This crate has no encryption API.

A finished produce always writes `{public-hex}.cbor` under the history root. Tests pass an override root. On a Linux harness, `history_root(None)` uses that Windows default when `USERPROFILE` is set and `HOME` is not.

## Keys

Supply a public identifier (`npub` or hex) and, optionally, a private key. The private key may be an `nsec` or 64 hex digits (optional `0x`). The public identifier is derived from the private key. Secrets stay in memory, `Debug` redacts them, and preferences have no secret field. A planted `nsec` in a prefs document is refused.

## Test (native, on the Linux host)

From this directory:

```sh
cargo test
```

That runs the W1 harness on the host. It does not cross-compile.

## Cross-build

The mingw linker is `x86_64-w64-mingw32-gcc` (`.cargo/config.toml`). The Rust target is `x86_64-pc-windows-gnu`.

```sh
cargo build --target x86_64-pc-windows-gnu
```

W1 does not run the resulting binary under Wine.
