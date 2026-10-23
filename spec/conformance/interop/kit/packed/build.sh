#!/usr/bin/env bash
R=~/interop-r2; source $R/kit/env.sh; K=$R/kit/packed; cd $R/src/C-grok/impl
zig build-exe -OReleaseFast --dep hike -Mroot=$K/grok_packed.zig -Mhike=src/root.zig -femit-bin=$K/grok_packed 2>&1 && echo grok-ok
ls $R/src/D-gemini/impl/build/libs/ ; javac -d $K -cp $R/src/D-gemini/impl/build/libs/satoshi-hike-all.jar $K/GeminiPacked.java && echo gemini-ok
