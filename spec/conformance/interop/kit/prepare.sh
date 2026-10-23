#!/usr/bin/env bash
# Lay out ~/interop-r2 from a groundinglab/pcoc checkout. Run from the repository root.
set -eu
I=spec/conformance/interop; R=${R:-$HOME/interop-r2}
mkdir -p "$R/src" "$R/kit"
cp -r "$I/kit/." "$R/kit/"
for p in A-sonnet:A-sonnet-ts B-sol:B-sol-py C-grok:C-grok-zig D-gemini:D-gemini-kotlin E-kimi:E-kimi-csharp F-glm:F-glm-swift; do
  id=${p%%:*}; dir=${p#*:}
  rm -rf "$R/src/$id"; cp -r "$I/round-2/$dir/code" "$R/src/$id"
done
echo "ready: $R"
