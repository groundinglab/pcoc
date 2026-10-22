#!/bin/sh
# AC-A10. Negatives stay out of the release library and the pcoc binary.
# Check 4 (mutants) is Phase B.
set -eu

fail() { echo "FAIL: $1" >&2; exit 1; }

# Walk up to the directory that holds sdk/VERSION and spec/.
start=$(CDPATH= cd "$(dirname "$0")" && pwd)
d=$start
found=
while :; do
  if [ -f "$d/sdk/VERSION" ] && [ -d "$d/spec" ]; then
    found=$d
    break
  fi
  if [ "$d" = "/" ]; then
    break
  fi
  d=$(dirname "$d")
done
if [ -z "$found" ]; then
  fail "cannot locate repository root (no directory holding sdk/VERSION and spec/)"
fi
cd "$found/sdk/c"
ROOT=$(pwd)

# (1) separation: gen/ is not a library or CLI source.
makefile=$ROOT/Makefile
grep -q 'gen/negatives.c' "$makefile" || fail "generator source not named"
# Release objects are listed as LIB_SRC. They must not include gen/ or negatives.
lib=$(awk '/^LIB_SRC/,/[^\\]$/' "$makefile")
printf '%s\n' "$lib" | grep -q 'negatives' && fail "negatives in LIB_SRC"
printf '%s\n' "$lib" | grep -q 'gen/' && fail "gen/ in LIB_SRC"
printf '%s\n' "$lib" | grep -q 'src/sha256.c' || fail "library sources missing"

# (2) symbols and (3) strings, on the release artefacts only.
names='slot_summary_fold xor_accumulator_fold xor_w_fold s_plus_w_fold prefold_hoist final_stage_hoist partial_slot_read partial_slot_read_2047 f3_shared_terrain cached_leg one_lane_flip all4_kernel squeezed_seed_terrain'
arts="$ROOT/build/libpcoc.a $ROOT/build/libpcoc.so $ROOT/build/pcoc"
for a in $arts; do
  [ -f "$a" ] || fail "missing $a"
  if nm -a "$a" 2>/dev/null | grep -E 'neg_leg|negatives|fold_slot_summary|fold_prefold|run_cached' >/dev/null; then
    fail "negative symbol in $a"
  fi
  strings "$a" > "$ROOT/build/strings.txt"
  for n in $names; do
    if grep -F -q "$n" "$ROOT/build/strings.txt"; then
      fail "string $n in $a"
    fi
  done
done
rm -f "$ROOT/build/strings.txt"
echo PASS
exit 0
