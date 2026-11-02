#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
root=$(CDPATH= cd -- ../.. && pwd)
dll=cli/Pcoc.Cli/bin/Release/net10.0/pcoc.dll
lib=src/Pcoc/bin/Release/net10.0/PcocLib.dll
if [ ! -f "$dll" ] || [ ! -f "$lib" ]; then
  echo "negatives: release build missing" >&2
  exit 1
fi
python3 - "$root" "$dll" "$lib" <<'PY'
import json, pathlib, sys
root, dll, lib = sys.argv[1:]
neg = json.loads(pathlib.Path(root, "spec/conformance/pack/negatives.json").read_text())
names = []
for key in neg["values"]:
    if not key.startswith("negatives.") or key.startswith("negatives.honest."):
        continue
    name = key.split(".")[1]
    if name not in names:
        names.append(name)
blob = pathlib.Path(dll).read_bytes() + pathlib.Path(lib).read_bytes()
text = pathlib.Path("src/Pcoc").read_text() if False else ""
src = ""
for folder in ("src/Pcoc", "cli/Pcoc.Cli"):
    for p in pathlib.Path(folder).rglob("*.cs"):
        src += p.read_text()
bad = False
for name in names:
    if name.encode() in blob or name in src:
        print(f"negatives: {name} is in the release library or CLI", file=sys.stderr)
        bad = True
for folder in ("src/Pcoc", "cli/Pcoc.Cli"):
    for p in pathlib.Path(folder).rglob("*"):
        if not p.is_file() or p.suffix not in (".cs", ".csproj"):
            continue
        body = p.read_text()
        if "Pcoc.Gen" in body or "Pcoc.Gen.csproj" in body:
            print(f"negatives: {p} references the generator", file=sys.stderr)
            bad = True
if bad:
    sys.exit(1)
print(f"negatives: {len(names)} names absent from release Pcoc.dll and pcoc")
PY
