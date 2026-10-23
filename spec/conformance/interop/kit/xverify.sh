#!/usr/bin/env bash
# usage: xverify.sh <verifier-id> <object-file> <O1|O2|O3>  -> prints record on stdout
R=~/interop-r2; source $R/kit/env.sh
V=$1; F=$(readlink -f $2); K=$3; S=$R/src/$V
case $K in
 O1) P="--profile conformance-small --sample-bytes 32 --slot-bytes 64 --terrain-bytes 65536 --steps-per-leg 100";;
 O2) P="--profile conformance-small --sample-bytes 64 --slot-bytes 256 --terrain-bytes 1048576 --steps-per-leg 1000";;
 O3) P="";;
esac
L=pcoc-pack-2.2/draw
case $V in
 A-sonnet) cd $S/impl && exec node dist/cli.js verify $F $P --index-fixture --draw-label $L;;
 B-sol) cd $S && exec python3 -m impl.satoshi_hike verify $F $P --fixture-index --draw-label $L;;
 C-grok) D="--draw-label $L"; [ $K = O3 ] && D=""; cd $S/impl && exec ./zig-out/bin/hike verify $F $P $D;;   # zig refuses --draw-label outside conformance-small
 D-gemini) cd $S && exec ./impl/verify $P --draw-label $L $F;;
 E-kimi) cd $S/impl && exec dotnet src/Pcoc.Cli/bin/Release/net8.0/pcoc-cli.dll verify $F $P --synthetic-index --draw-label $L;;
 F-glm) [ $K = O3 ] && P="--profile production"; cd $S/impl && exec .build/release/SHikeCLI verify $F $P --draw-source chain:$L;;
esac
