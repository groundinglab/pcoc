#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
root=$(CDPATH= cd -- ../.. && pwd)
pub=$(mktemp -d)
cleanup() { rm -rf "$pub"; }
trap cleanup EXIT
dotnet publish cli/Pcoc.Cli/Pcoc.Cli.csproj -c Release -o "$pub" --nologo
python3 -u - "$root" "$pub" <<'PY'
import json, pathlib, sys
root, pub = sys.argv[1:]
neg = json.loads(pathlib.Path(root, "spec/conformance/pack/negatives.json").read_text())
names = []
for key in neg["values"]:
    if not key.startswith("negatives.") or key.startswith("negatives.honest."):
        continue
    name = key.split(".")[1]
    if name not in names:
        names.append(name)
if len(names) != 13:
    print(f"negatives: expected 13 names, found {len(names)}", file=sys.stderr)
    sys.exit(1)
files = sorted(p for p in pathlib.Path(pub).iterdir() if p.is_file())
have = {p.name for p in files}
missing = sorted({"PcocLib.dll", "pcoc.dll"} - have)
if missing:
    print("negatives: publish output missing " + ", ".join(missing), file=sys.stderr)
    sys.exit(1)
blobs = []
for p in files:
    print(f"negatives: scan {p.name}")
    blobs.append(p.read_bytes())

def norm_text(s):
    return s.lower().replace("_", "")

def utf8_norm(b):
    return norm_text(b.decode("utf-8", errors="replace"))

def utf16_norm(b):
    n = len(b) - (len(b) % 2)
    return norm_text(b[:n].decode("utf-16-le", errors="replace"))

norm8 = [utf8_norm(b) for b in blobs]
norm16 = [utf16_norm(b) for b in blobs]
src_parts = []
for folder in ("src/Pcoc", "cli/Pcoc.Cli"):
    for p in pathlib.Path(folder).rglob("*.cs"):
        if any(part in ("bin", "obj") for part in p.parts):
            continue
        src_parts.append(p.read_text())
src = "\n".join(src_parts)
src_norm = norm_text(src)
bad = False
for name in names:
    raw = name.encode("utf-8")
    wide = name.encode("utf-16-le")
    folded = norm_text(name)
    hit = name in src or folded in src_norm
    if not hit:
        for blob in blobs:
            if raw in blob or wide in blob:
                hit = True
                break
    if not hit:
        for text in norm8:
            if folded in text:
                hit = True
                break
    if not hit:
        for text in norm16:
            if folded in text:
                hit = True
                break
    if hit:
        print(f"negatives: {name} is in the release library or CLI", file=sys.stderr)
        bad = True
gen_funcs = (
    "FoldSlotSummary",
    "FoldXorAcc",
    "FoldXorW",
    "FoldSPlusW",
    "FoldFinalHoist",
    "Prefold",
    "Cached",
    "OneLane",
    "All4",
    "Squeezed",
    "FoldWords",
    "MixBeforeFinal",
)
for fn in gen_funcs:
    raw = fn.encode("utf-8")
    wide = fn.encode("utf-16-le")
    hit = fn in src
    if not hit:
        for blob in blobs:
            if raw in blob or wide in blob:
                hit = True
                break
    if hit:
        print(f"negatives: generator name {fn} is in the release library or CLI", file=sys.stderr)
        bad = True
for folder in ("src/Pcoc", "cli/Pcoc.Cli"):
    for p in pathlib.Path(folder).rglob("*"):
        if any(part in ("bin", "obj") for part in p.parts):
            continue
        if not p.is_file() or p.suffix not in (".cs", ".csproj"):
            continue
        body = p.read_text()
        if "Pcoc.Gen" in body or "Pcoc.Gen.csproj" in body:
            print(f"negatives: {p} references the generator", file=sys.stderr)
            bad = True
if bad:
    sys.exit(1)
shown = ", ".join(p.name for p in files)
print(f"negatives: 13 names absent from publish output ({shown})")
PY
