# PCOC Windows producer (W2)

One-screen Windows producer. The egui window is `app/` (`pcoc-windows`). It links `pcoc-windows-core`. The Linux GTK app in `apps/desktop` is a separate tree.

## Pin

| Item | Value |
|---|---|
| SPEC | 2.3 |
| `STAGES` | 18 |
| Object kind | `v2.2-pcoc-object` |
| SDK | 2.3.1 (`sdk/VERSION`) |
| SDK crates | path deps `../../sdk/rust/pcoc` and `../../sdk/rust/pcoc-produce` |

Rust is pinned by `rust-toolchain.toml` in this folder (1.99.0). The numbers are checked in `core/src/pin.rs`.

## Window

One window. Controls: Generate, Save key, Start, Stop, Show history, Open folder. There is no second page.

* **Generate** makes an nsec and keeps it in memory. **Save key** writes that file only when you press it.
* The key box accepts an **nsec** only. A hex secret is rejected with a clear message. The core parser still accepts hex for the W1 harness; the window does not.
* **Start** runs a live hike: Bitcoin mainnet DNS seeds, production profile, Bari posts. The status line says when it is waiting for the next block.
* **Stop** sets the cancel flag. A wait for the first block returns within a second and writes no `.cbor`.
* **Show history** lists `.cbor` files on this screen. **Open folder** opens the history folder.
* History default: `%USERPROFILE%\PCOC\hikes\`. Files are cleartext. There is no encryption API.

## After a finished object

The worker saves `{public-hex}.cbor`, measures it in-process, posts sign-in once, then sends one NIP-04 kind-4 direct message of the `.cbor` to the object's own npub.

| Step | Behaviour |
|---|---|
| Measurement | Section 14 record. The sentence says it is a measurement, not a gate decision. |
| Sign-in | Origin `https://groundinglab.io`. One POST to `https://admission.groundinglab.io/v1/sign-in` when the sign-in link on this screen parses. The website mints that link. No result-route poll. |
| Direct message | Kind 4, same shape as the Linux desktop sender. Relays: `wss://relay.damus.io`, `wss://relay.primal.net`, `wss://nos.lol`. One acceptance is enough. A refusal does not delete the file. |
| Claims | Fail closed. The success text says to claim on the groundinglab website. This app does not submit a claim. |

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

## Test (native, on the Linux host)

From this directory:

```sh
cargo test --locked -p pcoc-windows-core
cargo test --locked -p pcoc-windows --no-default-features
```

The second command tests the window logic without linking egui. It does not open a window.

## Windows binary and MSI

The mingw linker is `x86_64-w64-mingw32-gcc` (`.cargo/config.toml`). The Rust target is `x86_64-pc-windows-gnu`. `wixl` (the `wixl` package) packs the MSI.

```sh
sh installer/build-msi.sh
```

That writes:

* `target/x86_64-pc-windows-gnu/release/pcoc-windows.exe`
* `target/msi/pcoc-windows.msi`

The MSI installs `pcoc-windows.exe` under `Program Files\PCOC\`. Wine is not the gold path. Install the MSI on a Windows machine.

## Evidence

Command exits for this cut are in `W2-EVIDENCE.md`.
