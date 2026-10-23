#!/usr/bin/env bash
# usage: ladder.sh <machine> <engine> "<N list>" [cond6]  -- grok producer, gemini checker
R=~/interop-r2; K=$R/kit/packed; M=$1; E=$2; W=${W:-60}
rate(){ awk -F, -v t0=$(grep T0= $1/mem.txt|cut -d= -f2) -v t1=$(grep T1= $1/mem.txt|cut -d= -f2) '/^leg,/{ if($4>=t0 && $4<=t1){n++; if(n==1)a=$4; b=$4} } END{ if(n>1) printf "%.4f\n",(n-1)/((b-a)/1e9); else print 0}' $2; }
for N in $3; do
  if [ "$N" = 6 ] && [ "${4:-}" = cond6 ]; then
    m=$(for f in $R/packed/$M-grok-$E-N4/i*.log; do rate $R/packed/$M-grok-$E-N4 $f; done | sort -n | head -1)
    awk -v m=$m 'BEGIN{exit !(m>=0.2)}' || { echo "skip N=6: N=4 min per-hike $m < 0.2"; continue; }
  fi
  bash $K/packed.sh grok $E $N $W $M
  bash $K/verify-sample.sh $R/packed/$M-grok-$E-N$N gemini
done
echo LADDER DONE $(date +%T)
