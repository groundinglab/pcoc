#!/bin/sh
# Three checks that the thirteen permanent negatives cannot reach a release
# artefact (SPEC §16, White Hat Q7). Check 4, mutants, is Phase B.
#
#   1. separation: package negatives is imported only by the generator.
#   2. symbols: none of the negative names are symbols of the release library
#      or the pcoc binary.
#   3. strings: none of the thirteen names occur in those artefacts.
#
# Prints PASS and exits 0 when all three hold.

set -eu
cd "$(dirname "$0")/.."

names="slot_summary_fold xor_accumulator_fold xor_w_fold s_plus_w_fold prefold_hoist final_stage_hoist partial_slot_read partial_slot_read_2047 f3_shared_terrain cached_leg one_lane_flip all4_kernel squeezed_seed_terrain"

# 1. separation
importers=$(grep -R -n --include='*.go' 'gen/negatives"' . | grep -v '^./gen/' || true)
if [ -n "$importers" ]; then
    echo "separation: a release file imports negatives" >&2
    echo "$importers" >&2
    exit 1
fi
if ! grep -q 'gen/negatives"' gen/main.go; then
    echo "separation: the generator does not import negatives" >&2
    exit 1
fi

# 2 and 3. build the release artefacts and scan them.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
go build -o "$tmp/pcoc" ./cmd/pcoc
go build -o "$tmp/libpcoc.a" .

for name in $names; do
    if go tool nm "$tmp/pcoc" | grep -q "$name"; then
        echo "symbols: $name in pcoc" >&2
        exit 1
    fi
    if go tool nm "$tmp/libpcoc.a" | grep -q "$name"; then
        echo "symbols: $name in library" >&2
        exit 1
    fi
    if strings "$tmp/pcoc" | grep -q -x -F "$name"; then
        echo "strings: $name in pcoc" >&2
        exit 1
    fi
    if strings "$tmp/libpcoc.a" | grep -q -F "$name"; then
        echo "strings: $name in library" >&2
        exit 1
    fi
done

echo PASS
exit 0
