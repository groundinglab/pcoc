#!/usr/bin/env bash
# usage: verify-sample.sh <rundir> <checker grok|gemini> : recompute last steady leg of every instance with the OTHER build
R=~/interop-r2; source $R/kit/env.sh; K=$R/kit/packed; D=$1; C=$2
for f in $D/i*.log; do pk=$(awk -F, '/^pk,/{print $2}' $f); l=$(grep '^leg,' $f | tail -1); IFS=, read -r _ j ts te res prev cp <<< "$l"
 case $C in grok) got=$($K/grok_packed check $pk $prev fast);; gemini) got=$(java --add-modules=jdk.incubator.vector -XX:+UseSHA -XX:+UnlockDiagnosticVMOptions -XX:+UseSHA256Intrinsics -Xmx1g -cp $K:$R/src/D-gemini/impl/build/libs/satoshi-hike-all.jar GeminiPacked check $pk $prev 2>/dev/null);; esac
 [ "$got" = "$res,$cp" ] && v=MATCH || v="MISMATCH got=$got"
 echo "$(basename $f .log),leg$j,$res,$C,$v"; done > $D/verify.csv; cat $D/verify.csv
