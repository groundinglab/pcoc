#!/usr/bin/env bash
cd ~/interop-r2; source kit/env.sh
if ! command -v dotnet >/dev/null || ! dotnet --list-sdks | grep -q '^8\.'; then
  curl -fsSL https://dot.net/v1/dotnet-install.sh -o toolchains/dotnet-install.sh && bash toolchains/dotnet-install.sh --channel 8.0 --install-dir toolchains/dotnet8 --no-path > dotnet-setup.log 2>&1; source kit/env.sh; fi
dotnet --version
bash kit/run-build.sh E-kimi build test cands objects bench
bash kit/xgrid-kimi.sh
bash kit/pack.sh
echo KIMI DONE $(date +%T)
