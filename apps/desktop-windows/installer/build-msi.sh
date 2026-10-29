#!/bin/sh
# Build the Windows exe and pack it into an MSI with wixl (msitools).
# Run from any directory. Requires the mingw linker, Rust target
# x86_64-pc-windows-gnu, and the wixl package.
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked --target x86_64-pc-windows-gnu -p pcoc-windows
exe="$PWD/target/x86_64-pc-windows-gnu/release/pcoc-windows.exe"
out="$PWD/target/msi/pcoc-windows.msi"
mkdir -p "$PWD/target/msi"
wixl -v "$PWD/installer/pcoc.wxs" -o "$out" -a x64 -D "SourceExe=$exe"
echo "msi: $out"
ls -l "$out"
