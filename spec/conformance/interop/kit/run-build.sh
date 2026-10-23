#!/usr/bin/env bash
# usage: run-build.sh <id> [phase...]   phases: build test cands objects bench
set -u
ID=$1; shift; PH=${*:-build test cands objects bench}
R=~/interop-r2; source $R/kit/env.sh
S=$R/src/$ID; O=$R/out/$ID; mkdir -p $O
SSLI=""; [ -f /usr/include/openssl/sha.h ] || SSLI="-I$R/toolchains/ssl/usr/include -I$R/toolchains/ssl/usr/include/x86_64-linux-gnu"
step(){ local name=$1; shift; echo "== $ID $name $(date +%T)" | tee -a $O/steps.log; local t0=$(date +%s.%N); ( "$@" ) > $O/$name.log 2>&1; local rc=$?; local t1=$(date +%s.%N); awk -v r=$rc -v a=$t0 -v b=$t1 "BEGIN{printf \"rc=%s wall=%.1fs\\n\", r, b-a}" | tee -a $O/steps.log; return $rc; }
ver(){ { uname -a; gcc --version|head -1; case $ID in
  A-sonnet) node -v; (cd $S/impl && npx tsc -v);;
  B-sol) python3 -V; echo "CFLAGS=-O3 -DNDEBUG -fPIC -Wall -Wextra -Wpedantic -march=native $SSLI";;
  C-grok) zig version;;
  D-gemini) java -version 2>&1; (cd $S && ./impl/gradlew -p impl --version);;
  F-glm) swift --version 2>&1;;
  E-kimi) dotnet --version; which dotnet;;
  esac; } > $O/versions.txt 2>&1; }
for p in $PH; do case $ID:$p in
  A-sonnet:build) ver; step build bash -c "cd $S/impl && npm install && npm run build";;
  A-sonnet:test) step test bash -c "cd $S/impl && npm test";;
  A-sonnet:cands) step cands bash -c "cd $S/impl && node dist/gen-candidates.js candidates";;
  A-sonnet:objects) step objects bash -c "cd $S/impl && node dist/cli.js produce --all objects";;
  A-sonnet:bench) for i in 1 2 3; do step bench$i bash -c "cd $S/impl && node dist/bench.js timings"; done;;
  B-sol:build) ver; step build bash -c "cd $S && make native CFLAGS='-O3 -DNDEBUG -fPIC -Wall -Wextra -Wpedantic -march=native $SSLI'";;
  B-sol:test) step test bash -c "cd $S && make test";;
  B-sol:cands) step cands bash -c "cd $S && python3 -m impl.satoshi_hike generate-all";;
  B-sol:objects) echo "(objects produced by generate-all)";;
  B-sol:bench) step bench1 bash -c "cd $S && make benchmark";;
  C-grok:build) ver; step build bash -c "cd $S/impl && zig build --release=fast";;
  C-grok:test) step test bash -c "cd $S/impl && zig build test --release=fast --summary all";;
  C-grok:cands) step cands bash -c "cd $S/impl && ./zig-out/bin/gen-candidates --out ../candidates";;
  C-grok:objects) step objects bash -c "cd $S/impl && ./zig-out/bin/hike produce-fixtures --out ../objects";;
  C-grok:bench) for i in 1 2 3; do step bench$i bash -c "cd $S/impl && ./zig-out/bin/hike bench"; done;;
  D-gemini:build) ver; step build bash -c "cd $S && ./impl/gradlew -p impl --no-daemon fatJar installDist";;
  D-gemini:test) step test bash -c "cd $S && ./impl/gradlew -p impl --no-daemon test";;
  D-gemini:cands) step cands bash -c "cd $S && ./impl/gradlew -p impl --no-daemon :negatives:genCandidates";;
  D-gemini:objects) step objects bash -c "cd $S && ./impl/gradlew -p impl --no-daemon genObjects";;
  D-gemini:bench) for i in 1 2 3; do step bench$i bash -c "cd $S && ./impl/gradlew -p impl --no-daemon benchmark"; done;;
  F-glm:build) ver; step build bash -c "cd $S/impl && swift build -c release";;
  F-glm:test) step test bash -c "cd $S/impl && swift test -c release";;
  F-glm:cands) step cands bash -c "cd $S/impl && swift run -c release GenCandidatesCLI";;
  F-glm:objects) step objects bash -c "cd $S/impl && swift run -c release SHikeCLI produce --out objects";;
  F-glm:bench) for i in 1 2 3; do step bench$i bash -c "cd $S/impl && swift run -c release SHikeCLI bench"; done;;
  E-kimi:build) ver; step build bash -c "cd $S/impl && dotnet build Pcoc.sln -c Release";;
  E-kimi:test) step test bash -c "cd $S/impl && dotnet test Pcoc.sln -c Release";;
  E-kimi:cands) step cands bash -c "cd $S/impl && dotnet src/Pcoc.Cli/bin/Release/net8.0/pcoc-cli.dll gen-candidates --out ../candidates";;
  E-kimi:objects) step objects bash -c "cd $S/impl && dotnet src/Pcoc.Cli/bin/Release/net8.0/pcoc-cli.dll produce --out ../objects";;
  E-kimi:bench) for i in 1 2 3; do step bench$i bash -c "cd $S/impl && dotnet src/Pcoc.Cli/bin/Release/net8.0/pcoc-cli.dll bench"; done;;
esac; done
# collect (only when generation ran)
case " $PH " in *" cands "*|*" objects "*) ;; *) echo "== $ID done $(date +%T)" | tee -a $O/steps.log; exit 0;; esac
CD=$S/candidates; OD=$S/objects; [ -d $S/impl/candidates ] && CD=$S/impl/candidates; [ -d $S/impl/objects ] && OD=$S/impl/objects
mkdir -p $O/cands $O/objects; cp $CD/*.txt $O/cands/ 2>/dev/null; cp $OD/*.cbor $O/objects/ 2>/dev/null
(cd $O && sha256sum objects/*.cbor > objects.sha256; for f in cands/*.txt; do echo "$(git hash-object $f 2>/dev/null || python3 -c "import hashlib,sys;b=open('$f','rb').read();print(hashlib.sha1(b'blob %d\0'%len(b)+b).hexdigest())") $f"; done > cands.blob)
echo "== $ID done $(date +%T)" | tee -a $O/steps.log
