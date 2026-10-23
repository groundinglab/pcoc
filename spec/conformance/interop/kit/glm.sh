#!/usr/bin/env bash
# GLM extras: test with the AVX-512-only test skipped, and the external timing harness.
R=~/interop-r2; source $R/kit/env.sh; S=$R/src/F-glm/impl; O=$R/out/F-glm
cd $S
case $1 in
 testskip) swift test -c release --skip prodTerrainScalarEqualsAvx512 > $O/test-skip.log 2>&1; echo "rc=$?" >> $O/test-skip.log; tail -3 $O/test-skip.log;;
 harness) clang -O2 -I Sources/CSatoshiKernels/include $R/kit/glm_bench.c .build/release/CSatoshiKernels.build/*.o -o $O/glm_bench && echo built; ls .build/release/CSatoshiKernels.build/;;
 bench) D=$(awk '$1=="seed.pk_A.D"{print $2}' $O/cands/seed.txt); for i in 1 2 3; do $O/glm_bench $D; done | tee $O/bench-harness.log;;
esac
