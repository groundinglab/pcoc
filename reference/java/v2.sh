#!/usr/bin/env bash
# v2.0 conformance pack for the Java produce and verify port.
# Reads conformance/v2.0 and compares the section 14 record to the Rust reference.
set -euo pipefail
cd "$(dirname "$0")"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
mkdir -p "$out/classes"
find src -name '*.java' | sort > "$out/sources.txt"
javac --release 21 -Xlint:all -Werror -d "$out/classes" @"$out/sources.txt"
java -cp "$out/classes" pcoc.conformance.V2Pack
java -cp "$out/classes" pcoc.hike.Spec6Pins
