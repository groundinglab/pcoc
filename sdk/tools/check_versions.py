#!/usr/bin/env python3
"""Fail unless every Rust SDK crate's package version equals sdk/VERSION.
Go and Java carry no version in code: their version is the git tag
(sdk/go/v<VERSION> for the Go module, sdk-v<VERSION> for the repository).
Run from the repository root."""
import re, sys, glob
want = open("sdk/VERSION").read().strip()
bad = []
for manifest in sorted(glob.glob("sdk/rust/*/Cargo.toml")):
    m = re.search(r'^\[package\][^\[]*?^version\s*=\s*"([^"]+)"', open(manifest).read(), re.M | re.S)
    got = m.group(1) if m else None
    print(f"{manifest}: {got}")
    if got != want:
        bad.append(manifest)
lock = open("sdk/rust/Cargo.lock").read()
for name in ("hike", "hike-net", "nip06", "satoshi-hike-produce", "satoshi-hike-verify"):
    m = re.search(r'\[\[package\]\]\nname = "' + re.escape(name) + r'"\nversion = "([^"]+)"', lock)
    if not m or m.group(1) != want:
        bad.append(f"Cargo.lock {name}")
print(f"sdk/VERSION {want}: " + ("OK" if not bad else "MISMATCH " + ", ".join(bad)))
sys.exit(1 if bad else 0)
