#!/usr/bin/env bash
# usage: packed.sh <grok|gemini> <fast|scalar|opt> <N> <window_s> <machine>
# N independent hikes concurrently; steady window starts once every instance finished leg 0.
set -u; R=~/interop-r2; source $R/kit/env.sh; K=$R/kit/packed
B=$1; E=$2; N=$3; W=$4; M=$5; D=$R/packed/$M-$B-$E-N$N; rm -rf $D; mkdir -p $D; STOP=$D/STOP
cmd(){ case $B in grok) echo "$K/grok_packed run pcoc-packed/$M/grok/$E/N$N/i$1 $E $STOP";;
  gemini) echo "java --add-modules=jdk.incubator.vector -XX:+UseSHA -XX:+UnlockDiagnosticVMOptions -XX:+UseSHA256Intrinsics -Xmx1g -cp $K:$R/src/D-gemini/impl/build/libs/satoshi-hike-all.jar GeminiPacked run pcoc-packed/$M/gemini/$E/N$N/i$1 $E $STOP";; esac; }
free -b | awk '/Mem:/{print "mem_before_used="$3" avail="$7}' > $D/mem.txt
pids=""; for i in $(seq 1 $N); do $(cmd $i) > $D/i$i.log 2> $D/i$i.err & pids="$pids $!"; done
echo "$pids" > $D/pids
until [ $(grep -l '^leg,0,' $D/i*.log 2>/dev/null | wc -l) -eq $N ]; do sleep 0.5; done
T0=$(date +%s%N); echo "T0=$T0" >> $D/mem.txt
sleep $((W/2)); free -b | awk '/Mem:/{print "mem_mid_used="$3" avail="$7" swap_note=see_swap"}' >> $D/mem.txt; free -b | awk '/Swap:/{print "swap_used="$3}' >> $D/mem.txt
for p in $pids; do echo "rss_kb_$p=$(awk '/VmRSS/{print $2}' /proc/$p/status)"; done >> $D/mem.txt
uptime >> $D/mem.txt
sleep $((W-W/2)); T1=$(date +%s%N); echo "T1=$T1" >> $D/mem.txt; touch $STOP; wait
echo "done $D"
