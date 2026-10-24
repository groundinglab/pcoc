#!/usr/bin/env bash
# Cross-verify gate for the Rust, C, Go, and Java SDKs.
# Run from the repository root:
#   bash sdk/conformance/cross-verify.sh [--tier pr|full] [--ref-dir DIR] [--out FILE]
# Default tier is pr (S1 and S2). full adds O3-PROD.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
TIER=pr
REF_DIR=
OUT=
CALLER_PWD=$(pwd)

usage() {
  echo "usage: bash sdk/conformance/cross-verify.sh [--tier pr|full] [--ref-dir DIR] [--out FILE]" >&2
  exit 2
}

while [ $# -gt 0 ]; do
  case "$1" in
    --tier)
      [ $# -ge 2 ] || usage
      TIER=$2
      shift 2
      ;;
    --tier=*)
      TIER=${1#--tier=}
      shift
      ;;
    --ref-dir)
      [ $# -ge 2 ] || usage
      REF_DIR=$2
      shift 2
      ;;
    --ref-dir=*)
      REF_DIR=${1#--ref-dir=}
      shift
      ;;
    --out)
      [ $# -ge 2 ] || usage
      OUT=$2
      shift 2
      ;;
    --out=*)
      OUT=${1#--out=}
      shift
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "cross-verify: unknown argument: $1" >&2
      usage
      ;;
  esac
done

case "$TIER" in
  pr|full) ;;
  *)
    echo "cross-verify: --tier must be pr or full" >&2
    exit 2
    ;;
esac

if [ -n "$REF_DIR" ]; then
  case "$REF_DIR" in
    /*) ;;
    *) REF_DIR=$CALLER_PWD/$REF_DIR ;;
  esac
fi
if [ -n "$OUT" ]; then
  case "$OUT" in
    /*) ;;
    *) OUT=$CALLER_PWD/$OUT ;;
  esac
fi

missing() {
  echo "cross-verify: missing toolchain: $1" >&2
  exit 1
}

if [ -x "$HOME/.cargo/bin/cargo" ]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
if [ -x "$HOME/.local/bin/go" ]; then
  export PATH="$HOME/.local/bin:$PATH"
fi

command -v cargo >/dev/null 2>&1 || missing "cargo (Rust 1.99, sdk/rust/rust-toolchain.toml)"
command -v rustc >/dev/null 2>&1 || missing "rustc"
command -v go >/dev/null 2>&1 || missing "go ($HOME/.local/bin/go)"
command -v gcc >/dev/null 2>&1 || missing "gcc"
command -v make >/dev/null 2>&1 || missing "make"
command -v python3 >/dev/null 2>&1 || missing "python3"

# sdk/java/build.gradle.kts sets the compiler release to 21.
if [ -z "${JAVA_HOME:-}" ] || ! "$JAVA_HOME/bin/javac" -version 2>&1 | grep -q 'javac 21'; then
  JAVA_HOME=
  for d in "$HOME"/.local/jdk/jdk-21*; do
    if [ -x "$d/bin/javac" ]; then
      JAVA_HOME=$d
      break
    fi
  done
fi
if [ -z "${JAVA_HOME:-}" ] || [ ! -x "$JAVA_HOME/bin/javac" ]; then
  missing "JDK 21 (sdk/java compiles with --release 21; JDK 17 cannot compile that build file)"
fi
export JAVA_HOME
export PATH="$JAVA_HOME/bin:$PATH"

BUILD=$ROOT/sdk/conformance/build
mkdir -p "$BUILD"
LOG=$BUILD/build.log
: >"$LOG"

fail_build() {
  echo "cross-verify: $1 failed" >&2
  tail -n 60 "$LOG" >&2
  exit 1
}

export CARGO_BUILD_JOBS=4
export RUSTFLAGS="-C target-cpu=x86-64"
export GOAMD64=v1
export GOMAXPROCS=4
export CGO_ENABLED=0

echo "cross-verify: building the four verify CLIs" >&2

(
  cd "$ROOT/sdk/c"
  nice -n 10 make -j4 CC=gcc \
    CFLAGS="-std=c11 -O2 -Wall -Wextra -Wno-unused-function -march=x86-64 -fPIC"
) >>"$LOG" 2>&1 || fail_build "C SDK"

(
  cd "$ROOT/sdk/go"
  nice -n 10 go mod download
  nice -n 10 go build -mod=readonly -o "$BUILD/pcoc-go" ./cmd/pcoc
) >>"$LOG" 2>&1 || fail_build "Go SDK"

(
  cd "$ROOT/sdk/rust"
  nice -n 10 cargo build --offline --release -p pcoc-cli -j 4
) >>"$LOG" 2>&1 || fail_build "Rust SDK"

(
  cd "$ROOT/sdk/java"
  nice -n 10 ./gradlew --offline jar -Dorg.gradle.workers.max=4
) >>"$LOG" 2>&1 || fail_build "Java SDK"

echo "cross-verify: building index drivers for Rust, C, and Go" >&2

nice -n 10 gcc -std=c11 -O2 -Wall -Wextra -Wno-unused-function -march=x86-64 \
  -I "$ROOT/sdk/c/include" \
  "$ROOT/sdk/conformance/c-driver/main.c" \
  "$ROOT/sdk/c/build/libpcoc.a" \
  -o "$BUILD/xverify-c" >>"$LOG" 2>&1 || fail_build "C index driver"

(
  cd "$ROOT/sdk/conformance/go-driver"
  nice -n 10 go build -o "$BUILD/xverify-go" .
) >>"$LOG" 2>&1 || fail_build "Go index driver"

(
  cd "$ROOT/sdk/conformance/rust-driver"
  nice -n 10 cargo build --offline --release -j 4 --target-dir "$BUILD/rust-target"
) >>"$LOG" 2>&1 || fail_build "Rust index driver"

INPUT_ROOT=${XVERIFY_INPUT_ROOT:-$ROOT/sdk}
if [ -n "${XVERIFY_INPUT_ROOT:-}" ]; then
  case "$INPUT_ROOT" in
    /*) ;;
    *) INPUT_ROOT=$CALLER_PWD/$INPUT_ROOT ;;
  esac
fi

ARGS=(
  --repo "$ROOT"
  --tier "$TIER"
  --input-root "$INPUT_ROOT"
  --erratum "$ROOT/sdk/conformance/erratum-negatives.tsv"
  --rust-cli "$ROOT/sdk/rust/target/release/pcoc"
  --c-cli "$ROOT/sdk/c/build/pcoc"
  --go-cli "$BUILD/pcoc-go"
  --java-cli "$ROOT/sdk/java/pcoc"
  --rust-drv "$BUILD/rust-target/release/xverify-rust"
  --c-drv "$BUILD/xverify-c"
  --go-drv "$BUILD/xverify-go"
)
if [ -n "$REF_DIR" ]; then
  ARGS+=(--ref-dir "$REF_DIR")
fi
if [ -n "$OUT" ]; then
  ARGS+=(--out "$OUT")
fi

export GOMAXPROCS=1
exec nice -n 10 python3 "$ROOT/sdk/conformance/harness.py" "${ARGS[@]}"
