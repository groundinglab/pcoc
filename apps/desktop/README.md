# PCOC Hike for Linux

PCOC Hike for Linux makes a PCOC, checks one, and signs in, on your own machine, using the Rust reference at git tag `ref-rust-v2.0.5` (`Cargo.toml`). A demonstrator, not part of the specification.

A full hike is eighteen stages paced by Bitcoin blocks. That is hours, not a button press. The key stays on the machine. Verify reads the portable index (the live default is the public index). Anyone can bypass that index by checking Bitcoin. The app does not say pass or fail. Sign-in sends the object to a relying party. Accept, refuse, and revoke are that party's policy.

**Mainnet only.** Start hike runs on Bitcoin mainnet and posts each waypoint to the public Bari from your real key. The app has no test plane and no test-plane switch. The app uses one application id. If the Flatpak build is already running, starting a second copy (for example `cargo run`) opens the running Flatpak window instead of a new one. Quit the Flatpak first to run the source build.

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
