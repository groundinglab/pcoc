#!/usr/bin/env bash
# extend the 5x5 grid to 6x6: every cell that involves E-kimi
R=~/interop-r2; X=$R/xv; ALL="A-sonnet B-sol C-grok D-gemini E-kimi F-glm"
run(){ v=$1;p=$2;k=$3; f=$(ls $R/out/$p/objects/$k-*.cbor 2>/dev/null | head -1); [ -z "$f" ] && { echo "$v $p $k NOOBJ" | tee -a $X/grid.log; return; }
  t0=$(date +%s.%N); timeout 900 bash $R/kit/xverify.sh $v $f $k > $X/$v.on.$p.$k.json 2> $X/$v.on.$p.$k.err; rc=$?; t1=$(date +%s.%N)
  awk -v v=$v -v p=$p -v k=$k -v r=$rc -v a=$t0 -v b=$t1 'BEGIN{printf "%s %s %s rc=%s %.1fs\n",v,p,k,r,b-a}' | tee -a $X/grid.log; }
for k in O1 O2 O3; do for p in $ALL; do run E-kimi $p $k; done; for v in A-sonnet B-sol C-grok D-gemini F-glm; do run $v E-kimi $k; done; done
