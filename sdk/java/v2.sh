#!/usr/bin/env bash
# v2.0 conformance pack for the Java produce and verify port.
# Reads spec/conformance/v2.0 and compares the section 14 record to the Rust reference.
set -euo pipefail
cd "$(dirname "$0")"
jv="$(javac -version 2>&1 | sed -n 's/^javac \([0-9][0-9]*\).*/\1/p')"
if [ -z "$jv" ] || [ "$jv" -lt 21 ]; then
  echo "v2.sh: JDK 21 or newer is required (javac reports: $(javac -version 2>&1 || echo none)). On JDK 17, install a JDK 21 side by side, point JAVA_HOME at it, and put \$JAVA_HOME/bin first on PATH." >&2
  exit 1
fi
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
mkdir -p "$out/classes"
find src -name '*.java' | sort > "$out/sources.txt"
javac --release 21 -Xlint:all -Werror -d "$out/classes" @"$out/sources.txt"
java -cp "$out/classes" pcoc.conformance.V2Pack
java -cp "$out/classes" pcoc.hike.Spec6Pins
