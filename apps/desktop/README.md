# PCOC Hike for Linux

Desktop app for Satoshi Hike v2.0 (Produce, Verify, Sign-in). Rust + GTK4 + libadwaita.

Needs GTK 4.14+ and libadwaita 1.5+ (Ubuntu 24.04, Debian 13, Fedora 40 or newer).

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
