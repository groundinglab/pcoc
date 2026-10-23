#!/usr/bin/env bash
# runs the full grid: every verifier x every producer x O1..O3
R=~/interop-r2; X=$R/xv; mkdir -p $X
IDS="${IDS:-A-sonnet B-sol C-grok D-gemini F-glm}"
for v in $IDS; do for p in $IDS; do for k in O1 O2 O3; do
  f=$(ls $R/out/$p/objects/$k-*.cbor 2>/dev/null | head -1); [ -z "$f" ] && { echo "$v $p $k NOOBJ"; continue; }
  t0=$(date +%s.%N); timeout 900 bash $R/kit/xverify.sh $v $f $k > $X/$v.on.$p.$k.json 2> $X/$v.on.$p.$k.err; rc=$?; t1=$(date +%s.%N)
  awk -v v=$v -v p=$p -v k=$k -v r=$rc -v a=$t0 -v b=$t1 'BEGIN{printf "%s %s %s rc=%s %.1fs\n",v,p,k,r,b-a}' | tee -a $X/grid.log
done; done; done
