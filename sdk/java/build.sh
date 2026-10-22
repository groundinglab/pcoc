#!/bin/sh
# Build and test the Java core. Thread cap is the caller's. Tests run at nice 10.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
JAVA_HOME="${JAVA_HOME:-$HOME/.local/jdk21}"
JAVAC="$JAVA_HOME/bin/javac"
JAVA="$JAVA_HOME/bin/java"
# Thread cap 4. Heap covers one production terrain (256 MiB) and nothing more.
JAVA_OPTS="-XX:ActiveProcessorCount=4 -Xms512m -Xmx2g"
OUT="$ROOT/sdk/java/build"
mkdir -p "$OUT"
find "$ROOT/sdk/java/src" -name '*.java' | sort > "$OUT/sources.list"
"$JAVAC" --release 21 -encoding UTF-8 -d "$OUT" @"$OUT/sources.list"
cmd="${1:-}"
if [ "$cmd" = "test" ]; then
  shift
  nice -n 10 "$JAVA" $JAVA_OPTS -cp "$OUT" pcoc.tests.AllTests --repo "$ROOT" "$@"
elif [ "$cmd" = "gen" ]; then
  nice -n 10 "$JAVA" $JAVA_OPTS -cp "$OUT" pcoc.gen.GenCandidates --out "$ROOT/sdk/java/candidates"
elif [ "$cmd" = "version" ]; then
  "$JAVA" $JAVA_OPTS -cp "$OUT" pcoc.cli.Main version
elif [ "$cmd" = "pin-flip" ]; then
  MAN="$ROOT/spec/conformance/pack/manifest.json"
  BAK="/tmp/pcoc-manifest-pin-bak.json"
  cp "$MAN" "$BAK"
  restore() { cp "$BAK" "$MAN"; rm -f "$BAK"; }
  trap restore EXIT
  python3 - "$MAN" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
b = bytearray(p.read_bytes())
b[-1] = (b[-1] + 1) & 0xFF
p.write_bytes(b)
PY
  set +e
  nice -n 10 "$JAVA" $JAVA_OPTS -cp "$OUT" pcoc.tests.AllTests --repo "$ROOT" --pin-only
  rc=$?
  set -e
  restore
  trap - EXIT
  if [ "$rc" -eq 0 ]; then
    echo "pin flip stayed green" >&2
    exit 1
  fi
  nice -n 10 "$JAVA" $JAVA_OPTS -cp "$OUT" pcoc.tests.AllTests --repo "$ROOT" --pin-only
fi
