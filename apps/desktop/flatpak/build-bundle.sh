#!/usr/bin/env bash
# Vendor Cargo.lock, build app.pcoc.Hike, and write a single-file bundle.
#
# Usage: build-bundle.sh [output-path]
# Default output: ~/pcoc-build/artifacts/pcoc.flatpak
# Also writes <output>.sha256 beside it.
#
# The host vendors crates (the pinned hike git revision needs credentials).
# flatpak-builder then compiles offline against the GNOME 50 SDK.
set -euo pipefail

here=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../../.." && pwd)
out=${1:-"${HOME}/pcoc-build/artifacts/pcoc.flatpak"}
vendor="${here}/cargo-vendor"
repo="${here}/repo"
build="${here}/build-dir"
config="${here}/cargo-config.toml"

if ! command -v flatpak >/dev/null 2>&1; then
  echo "flatpak is not on PATH" >&2
  exit 1
fi
if ! command -v flatpak-builder >/dev/null 2>&1; then
  echo "flatpak-builder is not on PATH" >&2
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is not on PATH" >&2
  exit 1
fi

mkdir -p "$(dirname "$out")"

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

echo "Vendoring crates from apps/desktop/Cargo.lock"
rm -rf "$vendor"
mkdir -p "$vendor"
vendor_cfg=$(
  # Run from apps/desktop so rustup applies apps/desktop/rust-toolchain.toml.
  cd "$root/apps/desktop"
  cargo vendor --locked --manifest-path Cargo.toml --versioned-dirs "$vendor"
)
VENDOR_CFG="$vendor_cfg" python3 - "$config" <<'PY'
import os
import re
import sys

path = sys.argv[1]
text = os.environ["VENDOR_CFG"]
if '[source.vendored-sources]' not in text:
    sys.stderr.write("cargo vendor did not emit a vendored-sources entry\n")
    sys.exit(1)
# The manifest installs this file at the repository root .cargo/config.toml.
# Point the vendor directory at the absolute sandbox path so it does not
# depend on cargo's working directory.
text = re.sub(
    r'(?m)^directory = ".*"$',
    'directory = "/run/build/pcoc-hike/apps/desktop/flatpak/cargo-vendor"',
    text,
    count=1,
)
header = """[net]
offline = true

[resolver]
incompatible-rust-versions = "fallback"

"""
with open(path, "w", encoding="utf-8") as fh:
    fh.write(header)
    fh.write(text)
    if not text.endswith("\n"):
        fh.write("\n")
PY

rm -rf "$repo" "$build"
echo "Building the Flatpak"
# State stays next to the manifest. The default is .flatpak-builder in the
# current directory, which is the repository root when CI checks out there.
flatpak-builder --user --disable-rofiles-fuse --force-clean --repo="$repo" \
  --state-dir="${here}/.flatpak-builder" \
  --install-deps-from=flathub \
  "$build" "${here}/app.pcoc.Hike.yml"

echo "Writing the single-file bundle"
# A flatpakrepo file, not the ostree repo root. The root answers 403 to a
# GET, and `flatpak install` of the bundle uses this URL to find the runtime.
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  "$repo" "$out" app.pcoc.Hike

sha256sum "$out" | tee "${out}.sha256"
echo "Bundle: $out"
