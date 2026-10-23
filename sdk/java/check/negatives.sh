#!/bin/sh
# AC-A10. Three checks. Check 4 (mutants) is Phase B: N/A.
# Release artefacts are the main jar and this tree's pcoc launcher.
# The generator jar is not a release artefact.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"
if [ -z "${JAVA_HOME:-}" ]; then
    echo "FAIL JAVA_HOME" >&2
    exit 1
fi
JAR=$JAVA_HOME/bin/jar
REL=build/libs/pcoc-2.3.0.jar
NAMES='slot_summary_fold xor_accumulator_fold xor_w_fold s_plus_w_fold prefold_hoist final_stage_hoist partial_slot_read partial_slot_read_2047 f3_shared_terrain cached_leg one_lane_flip all4_kernel squeezed_seed_terrain'

fail() {
    echo "FAIL $1" >&2
    exit 1
}

# (1) separation: gen source set, and the release jar task does not pack it.
grep -q 'sourceSets.create("gen")' build.gradle.kts || fail "separation source set"
jar_block=$(awk '/^tasks\.jar \{/,/^}/' build.gradle.kts)
if printf '%s\n' "$jar_block" | grep -q 'gen'; then
    fail "separation jar packs gen"
fi
# No release source file depends on the negatives package.
if grep -R -n 'io.groundinglab.pcoc.negatives' src/main >/dev/null 2>&1; then
    fail "separation main depends on negatives"
fi

# (2) symbols: the negatives package is absent from the release jar.
[ -f "$REL" ] || fail "release jar missing"
if "$JAR" tf "$REL" | grep -E 'negatives/|Negatives|NegativeHike' >/dev/null; then
    fail "symbols"
fi

# (3) strings: none of the 13 names in the release jar or the launcher.
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/jar"
( cd "$TMP/jar" && "$JAR" xf "$ROOT/$REL" )
BLOB=$TMP/blob
cat "$TMP/jar"/*/*.class "$TMP/jar"/*/*/*.class "$TMP/jar"/*/*/*/*.class > "$BLOB" 2>/dev/null || true
# Also the constant pool lives in the class files; scan every class byte.
find "$TMP/jar" -name '*.class' -print0 | xargs -0 cat > "$BLOB"
for name in $NAMES; do
    if grep -a -F -q "$name" "$BLOB"; then
        fail "strings jar $name"
    fi
    if grep -a -F -q "$name" "$ROOT/pcoc"; then
        fail "strings launcher $name"
    fi
    if grep -R -n -F "$name" src/main >/dev/null 2>&1; then
        fail "strings source $name"
    fi
done

echo PASS
exit 0
