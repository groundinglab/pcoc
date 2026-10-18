#!/usr/bin/env bash
# One harness for every SDK. Run from anywhere:  bash sdk/test.sh
#
# Steps (all run; the script exits non-zero if any step fails):
#   1. pins          sdk/SPEC_PIN matches spec/conformance/ (vector hash), and
#                    every Rust crate version equals sdk/VERSION
#   2. rust          cargo test --locked --release in sdk/rust (unit tests,
#                    conformance pack, malformed-input tests, nip06)
#   3. go            go vet ./... and go test ./... in sdk/go (unit tests and
#                    the conformance pack)
#   4. java          sdk/java/v2.sh (conformance pack and section 6 pins)
#   5. cross-check   the Rust, Go and Java verify programs on the same pack
#                    objects and mutants must write byte-identical records
#
# Needs: python3, Rust (rustup reads sdk/rust/rust-toolchain.toml), Go >= 1.22,
# JDK >= 21. A missing tool is a failure, not a skip. No network is used
# except by cargo to fetch locked crates on a cold cache.
set -uo pipefail
cd "$(dirname "$0")/.."
root="$(pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
declare -a results=()
failed=0

step() {
  local name="$1"; shift
  echo "==> $name"
  local t0=$SECONDS
  if "$@"; then
    results+=("PASS  $name ($((SECONDS - t0))s)")
  else
    results+=("FAIL  $name ($((SECONDS - t0))s)")
    failed=1
  fi
}

need() {
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || { echo "sdk/test.sh: required tool not found: $tool" >&2; return 1; }
  done
}

spec_pin() { need python3 && python3 sdk/tools/check_spec_pin.py && python3 sdk/tools/check_versions.py; }

rust() {
  need cargo && (cd sdk/rust && cargo test --locked --release)
}

go_sdk() {
  need go && (cd sdk/go && go vet ./... && go test ./...)
}

java_sdk() {
  need javac java && bash sdk/java/v2.sh
}

crosscheck() {
  need cargo go javac java python3 || return 1
  (cd sdk/rust && cargo build --locked --release -q -p satoshi-hike-verify) || return 1
  (cd sdk/go && go build -o "$work/go-verify" ./cmd/satoshi-hike-verify) || return 1
  mkdir -p "$work/java"
  (cd sdk/java && find src -name '*.java' | sort > "$work/java-sources.txt" \
    && javac --release 21 -d "$work/java" @"$work/java-sources.txt") || return 1
  python3 sdk/tools/crosscheck.py \
    --rust "$root/sdk/rust/target/release/satoshi-hike-verify" \
    --go "$work/go-verify" \
    --java-classes "$work/java"
}

step "pins: spec vectors + sdk version" spec_pin
step "rust: unit + conformance" rust
step "go: vet + unit + conformance" go_sdk
step "java: conformance" java_sdk
step "cross-check: rust/go/java verify" crosscheck

echo
echo "sdk/test.sh summary"
printf '  %s\n' "${results[@]}"
if [ "$failed" -ne 0 ]; then
  echo "sdk/test.sh: FAIL"
  exit 1
fi
echo "sdk/test.sh: PASS"
