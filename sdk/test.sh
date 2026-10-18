#!/usr/bin/env bash
# One harness for every SDK. Run from anywhere:  bash sdk/test.sh
#
# Steps (all run; the script exits non-zero if any step fails):
#   1. pins          sdk/SPEC_PIN matches spec/conformance/ and its tag's tree
#                    (vector hash), and every Rust crate version equals
#                    sdk/VERSION
#   2. rust          cargo test --locked --release in sdk/rust (unit tests,
#                    conformance pack, malformed-input tests, nip06)
#   3. go            go vet ./... and go test ./... in sdk/go (unit tests and
#                    the conformance pack)
#   4. java          sdk/java/v2.sh (conformance pack and section 6 pins)
#   5. cross-check   the Rust, Go and Java verify programs on the same pack
#                    objects and mutants must write byte-identical records
#   6. examples      every example compiles; the quickstarts reproduce the
#                    kat-small record byte for byte and the offline produce
#                    examples write the same object, in all three languages
#   7. docs          README versions, tags, spec tag, digests and code
#                    snippets match sdk/VERSION, sdk/SPEC_PIN, the example
#                    output and the example sources (sdk/tools/check_docs.py)
#
# Needs: python3, Rust (rustup reads sdk/rust/rust-toolchain.toml), Go >= 1.22,
# JDK >= 21, and a git clone that has the spec tag. A missing tool is a
# failure, not a skip. No network is used except by cargo to fetch locked
# crates on a cold cache. Cargo writes sdk/rust/target/ and
# sdk/examples/rust/target/ (git-ignored), or $CARGO_TARGET_DIR when set.
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

examples() {
  need cargo go javac java || return 1
  local pack="$root/spec/conformance/v2.0" lang ok=0
  # Build every example program. The live ones need the network, so they are
  # only compiled here.
  (cd sdk/examples/rust && cargo build -q --locked --release --bins) || return 1
  (cd sdk/examples/go && go vet ./...) || return 1
  mkdir -p "$work/example-java"
  javac --release 21 -Xlint:all -Werror -d "$work/example-java" \
    $(find sdk/java/src -name '*.java' | sort) sdk/examples/java/*.java || return 1
  # Quickstart: verify kat-small and reproduce its stored record.
  (cd sdk/examples/rust && cargo run -q --locked --release --bin pcoc-quickstart -- "$pack") > "$work/quickstart-rust.txt" || return 1
  (cd sdk/examples/go && go run . "$pack") > "$work/quickstart-go.txt" || return 1
  java -cp "$work/example-java" Quickstart "$pack" > "$work/quickstart-java.txt" || return 1
  # Produce: the stage0-start tip script at conformance-small sizes, offline.
  (cd sdk/examples/rust && cargo run -q --locked --release --bin produce -- "$pack" 2>/dev/null) > "$work/produce-rust.txt" || return 1
  (cd sdk/examples/go && go run ./produce "$pack") > "$work/produce-go.txt" || return 1
  java -cp "$work/example-java" ProduceExample "$pack" > "$work/produce-java.txt" || return 1
  for lang in rust go java; do
    echo "  quickstart ($lang):"; sed 's/^/    /' "$work/quickstart-$lang.txt"
    echo "  produce ($lang):"; sed 's/^/    /' "$work/produce-$lang.txt"
  done
  for lang in go java; do
    cmp -s "$work/quickstart-rust.txt" "$work/quickstart-$lang.txt" || { echo "  quickstart output differs: rust vs $lang"; ok=1; }
    cmp -s "$work/produce-rust.txt" "$work/produce-$lang.txt" || { echo "  produce output differs: rust vs $lang"; ok=1; }
  done
  grep -qx 'draw = draw_bytes_hex: true' "$work/quickstart-rust.txt" || { echo "  quickstart draw is not the vector's"; ok=1; }
  grep -qx 'matches kat-small.json record: true' "$work/quickstart-rust.txt" || { echo "  quickstart record is not the vector's"; ok=1; }
  grep -qx 'shape: match' "$work/produce-rust.txt" || { echo "  produced object does not have shape match"; ok=1; }
  return $ok
}

docs() {
  need python3 || return 1
  [ -s "$work/quickstart-rust.txt" ] && [ -s "$work/produce-rust.txt" ] || { echo "  needs the examples step output"; return 1; }
  python3 sdk/tools/check_docs.py --quickstart "$work/quickstart-rust.txt" --produce "$work/produce-rust.txt"
}

step "pins: spec vectors + sdk version" spec_pin
step "rust: unit + conformance" rust
step "go: vet + unit + conformance" go_sdk
step "java: conformance" java_sdk
step "cross-check: rust/go/java verify" crosscheck
step "examples: quickstart + produce agree, vector reproduced" examples
step "docs: versions, spec tag, digests, snippets" docs

echo
echo "sdk/test.sh summary"
printf '  %s\n' "${results[@]}"
if [ "$failed" -ne 0 ]; then
  echo "sdk/test.sh: FAIL"
  exit 1
fi
echo "sdk/test.sh: PASS"
