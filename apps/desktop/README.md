# PCOC Hike for Linux

PCOC Hike for Linux makes a PCOC, checks one, and signs in, on your own machine, using the Rust SDK in this repository (`sdk/rust`, by path in `Cargo.toml`; About shows the resolved SDK version). A demonstrator, not part of the specification.

A full hike is eighteen stages paced by Bitcoin blocks. That is hours, not a button press. The key stays on the machine. Verify reads the portable index (the live default is the public index). Anyone can bypass that index by checking Bitcoin. The app does not say pass or fail. Sign-in sends the object to a relying party. Accept, refuse, and revoke are that party's policy.

**Mainnet only.** Start hike runs on Bitcoin mainnet and posts each waypoint to the public Bari from your real key. The app has no test plane and no test-plane switch. The app uses one application id. If the Flatpak build is already running, starting a second copy (for example `cargo run`) opens the running Flatpak window instead of a new one. Quit the Flatpak first to run the source build.

**Send to my phone.** On a finished hike, Send to my phone sends the `.cbor` to the object's own npub as one NIP-04 direct message (kind 4), encrypted with the nsec pasted in this session. It goes to relay.damus.io, relay.primal.net and nos.lol; one relay accepting is enough, and nos.lol refuses most objects for size. The phone app signed in with that npub offers it for approval. Making a PCOC never sends it by itself, and the send never touches the admission gate. The code is `core/src/dm.rs`.

**SDK step in this build.** The app version is still 0.1.4, but the Rust SDK under it moved from `ref-rust-v2.0.5` to the in-tree `sdk/rust` (now 2.0.0-rc.2; rc.1 was `ref-rust-v2.0.6` plus an untrusted-input fix and an nsec fix). Since rc.1 it also has the section 14.1 recording fix, and rc.2 measures on through absent, mistyped and wrong-width fields: checking a malformed object now lists every violated condition and the later measurements. The details are in [sdk/RC-NOTES.md](../../sdk/RC-NOTES.md) and [sdk/CHANGES.md](../../sdk/CHANGES.md).

Needs GTK 4.14+ and libadwaita 1.5+ (Ubuntu 24.04, Debian 13, Fedora 40 or newer). Rust 1.99.0 comes from `rust-toolchain.toml` in this folder.

## 1. Get the code

```sh
git clone https://github.com/groundinglab/pcoc.git
cd pcoc/apps/desktop
```

## 2. Install dependencies

Debian / Ubuntu:

```sh
sudo apt install -y build-essential pkg-config libgtk-4-dev libadwaita-1-dev git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
. "$HOME/.cargo/env"
```

Fedora:

```sh
sudo dnf install -y gcc pkgconf-pkg-config gtk4-devel libadwaita-devel git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
. "$HOME/.cargo/env"
```

## 3. Build

`rust-toolchain.toml` in this folder pins Rust 1.99.0; rustup installs it automatically on the first `cargo` command. Check it:

```sh
cargo --version   # prints cargo 1.99.0, the pinned version
```

Then build:

```sh
cargo build --release --locked
```

## 4. Run the tests

```sh
cargo test --locked -p pcoc-hike-core
```

## 5. Run the app

```sh
cargo run --release --locked -p pcoc-hike
```

## Optional: remove build files (you'll need to build again)

```sh
cargo clean
```
