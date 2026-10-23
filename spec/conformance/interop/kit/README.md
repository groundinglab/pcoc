# Kit

The scripts used for round 2, published as run. They expect a working directory `~/interop-r2` with `src/<ID>/` holding each build and `kit/` holding these scripts. `prepare.sh` lays that out from this checkout.

```sh
# from the root of a groundinglab/pcoc checkout
bash spec/conformance/interop/kit/prepare.sh        # creates ~/interop-r2/{src,kit}
bash ~/interop-r2/kit/setup.sh ubuntu24.04          # or debian12; user-local toolchains, no root
                                                    # .NET 8: see kimi-chain.sh (dotnet-install.sh, user-local)
for id in A-sonnet B-sol C-grok D-gemini E-kimi F-glm; do
  bash ~/interop-r2/kit/run-build.sh $id            # build, test, candidates, objects, bench
done
bash ~/interop-r2/kit/glm.sh testskip; bash ~/interop-r2/kit/glm.sh harness; bash ~/interop-r2/kit/glm.sh bench
IDS="A-sonnet B-sol C-grok D-gemini E-kimi F-glm" bash ~/interop-r2/kit/xgrid.sh   # cross-verify grid
bash ~/interop-r2/kit/packed/build.sh               # packed-run drivers (Grok producer, Gemini checker)
bash ~/interop-r2/kit/packed/ladder.sh <machine> fast "1 4 6" cond6
python3 ~/interop-r2/kit/report/analyze.py ~/interop-r2   # summary of bench, tests and grid
```

| Script | What it does |
|---|---|
| `prepare.sh` | Copies each `round-2/<build>/code/` to `~/interop-r2/src/<ID>/` and the kit to `~/interop-r2/kit/` |
| `setup.sh` | Installs Zig 0.15.2, JDK 21, Node 22, Swift 6.1.2 and OpenSSL headers under `~/interop-r2/toolchains` |
| `env.sh` | Puts those toolchains on `PATH` |
| `run-build.sh <ID> [phase…]` | Build, own tests, candidate generator, producer and bench (3 runs) for one build |
| `xverify.sh`, `xgrid.sh`, `xgrid-kimi.sh` | One cross-verify, the full grid, and the cells added for build E |
| `glm.sh`, `glm_bench.c` | Build F's tests with its AVX-512-only test skipped, and a timing harness that links its unchanged kernels |
| `kimi-chain.sh` | Installs .NET 8 user-locally and runs build E through every phase |
| `packed/` | `grok_packed.zig` (N independent hikes on build C's unchanged library), `GeminiPacked.java` (recomputes a sampled leg on build D's library), `packed.sh`, `ladder.sh`, `verify-sample.sh` |
| `report/` | The scripts that turned raw outputs into the tables in `../round-2/results/` |

Close other heavy work while timing, and record what else was running. Your numbers will differ from ours with hardware and load; that is the point of comparing.
