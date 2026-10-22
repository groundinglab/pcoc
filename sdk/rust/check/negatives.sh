#!/bin/sh
# AC-A10. Three checks. Check 4 (mutants) is Phase B: N/A.
# Negatives are built only by the gen crate. Release artefacts are the pcoc
# library and the pcoc binary.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"
export CARGO_BUILD_JOBS=6
export CARGO_TARGET_DIR="${PCOC_NEGATIVES_TARGET:-/tmp/pcoc-rust-negatives-target}"

nice -n 10 cargo build -p pcoc -p pcoc-cli --release >/tmp/pcoc-negatives-build.log

for crate in pcoc pcoc-produce pcoc-cli pcoc-testing; do
    if grep -E 'path = "\.\./gen"|pcoc_gen' "$crate/Cargo.toml" >/dev/null 2>&1; then
        echo "FAIL $crate depends on gen"
        exit 1
    fi
done
found=$(find "$ROOT" -name 'negatives.rs' -not -path '*/target/*')
if [ "$found" != "$ROOT/gen/src/negatives.rs" ]; then
    echo "FAIL negatives.rs is not only gen/src/negatives.rs"
    echo "$found"
    exit 1
fi
echo "check 1 separation: PASS"

names='slot_summary_fold xor_accumulator_fold xor_w_fold s_plus_w_fold prefold_hoist final_stage_hoist partial_slot_read partial_slot_read_2047 f3_shared_terrain cached_leg one_lane_flip all4_kernel squeezed_seed_terrain'

bin="$CARGO_TARGET_DIR/release/pcoc"
if [ ! -f "$bin" ]; then
    echo "FAIL missing $bin"
    exit 1
fi
libs=$(find "$CARGO_TARGET_DIR/release/deps" -name 'libpcoc-*.rlib')
if [ -z "$libs" ]; then
    echo "FAIL missing libpcoc rlib"
    exit 1
fi

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
for art in $bin $libs; do
    case "$art" in
        *libpcoc_gen*) echo "FAIL scanned gen artefact $art"; exit 1 ;;
    esac
    nm -a "$art" >"$tmp" || { echo "FAIL nm $art"; exit 1; }
    strings -a "$art" >"$tmp.str" || { echo "FAIL strings $art"; exit 1; }
    for name in $names; do
        if grep -F -q -- "$name" "$tmp"; then
            echo "FAIL symbol $name in $art"
            exit 1
        fi
        if grep -F -q -- "$name" "$tmp.str"; then
            echo "FAIL string $name in $art"
            exit 1
        fi
    done
    rm -f "$tmp.str"
done

echo "check 2 symbols: PASS"
echo "check 3 strings: PASS"
echo "check 4 mutants: N/A"
echo PASS
