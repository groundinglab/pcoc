# W2 evidence — egui one-screen, NIP-04, MSI

Host: pepebook (Linux x86_64). Branch: `desktop/windows-w2`. Tree: `apps/desktop-windows/`. Date: 28 Oct 2026.

Commands were run from `apps/desktop-windows/`.

## Commands

| Command | Exit |
|---|---|
| `cargo test --locked -p pcoc-windows-core -- --test-threads=8` | 0 |
| `cargo test --locked -p pcoc-windows --no-default-features -- --test-threads=8` | 0 |
| `sh installer/build-msi.sh` | 0 |

`pcoc-windows-core` ran 31 lib tests, 31 passed, plus doc-tests (0). That includes the W1 harness. `pcoc-windows` without the `gui` feature ran 10 lib tests, 10 passed, plus doc-tests (0). Those tests do not open a window.

`installer/build-msi.sh` cross-builds `x86_64-pc-windows-gnu` and packs the MSI with `wixl`. The exe is a PE32+ GUI binary. Linker: `x86_64-w64-mingw32-gcc`.

## Artifact

| Path | What |
|---|---|
| `apps/desktop-windows/target/x86_64-pc-windows-gnu/release/pcoc-windows.exe` | egui window, links `pcoc-windows-core` |
| `apps/desktop-windows/target/msi/pcoc-windows.msi` | per-machine MSI; installs `Program Files\PCOC\pcoc-windows.exe` |

`target/` is gitignored. The MSI is produced by the command above. It is not committed.

## W2-AC1–AC9

| AC | Result | Proof |
|---|---|---|
| W2-AC1 egui one screen builds | met on this host | `sh installer/build-msi.sh` exit 0. Binary is `pcoc-windows` (eframe/egui) and depends on `pcoc-windows-core`. |
| W2-AC2 nsec only, Save on demand | met | `keys_ui::hex_secret_is_rejected_and_nsec_is_accepted` and `save_writes_only_when_asked`. `session::generate_does_not_write_a_key_until_save_and_logs_omit_it` checks the public log, `Debug`, and persisted JSON. |
| W2-AC3 one screen, Stop, history | met | `the_window_source_is_one_screen` (one `CentralPanel`; Generate, Save key, Start, Stop, Show history, Open folder). `stop_during_a_tip_wait_writes_no_file` returns inside 3 seconds with no `.cbor`. |
| W2-AC4 save, measure, one sign-in POST, no claims POST | met | `scripted_finish_posts_once_measures_and_keeps_the_file_when_dm_fails`: one `POST /v1/sign-in`, measurement sentence, file kept. Finish sources do not name a claims path. `claims::auto_claims` still fails closed. |
| W2-AC5 NIP-04 kind 4 to the object's npub | met | `dm` tests match the Linux desktop / Android vector (`content_matches_the_android_vector_byte_for_byte`, kind 4, one acceptance is a send). Empty relay list reports failure and the `.cbor` remains. |
| W2-AC6 honest success text | met | The block names the save path, a measurement (not a gate decision), the sign-in sentence, the direct-message outcome, and `Claim on the groundinglab website.` |
| W2-AC7 MSI | met on this host | `sh installer/build-msi.sh` exit 0. Path: `apps/desktop-windows/target/msi/pcoc-windows.msi`. |
| W2-AC8 Christian VM smoke | not run here | Install and the live Start/Stop smoke are Christian's CodeAnywhere Windows VM. This file does not mark AC8. |
| W2-AC9 process | local commands recorded | g1 and `ci-ok` are the PR checks. Red+White is on the opened PR (Gate 2), not before it. |

## Out of scope (not done)

Auto claims POST, Wine as gold, hex secret in the window, history encryption, merge, deploy, tag.
