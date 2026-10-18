#!/usr/bin/env python3
"""Fail when SDK docs disagree with the sources they copy from.

Checked in sdk/**/*.md and the example manifests:
  * every pre-release version string (N.N.N-rc.N and similar) and every
    sdk-v…, sdk/go/v…, @v… and pcoc-sdk-….jar version equals sdk/VERSION;
  * every spec-vN.N.N equals the tag= line of sdk/SPEC_PIN;
  * a ```text block right after <!-- output: quickstart --> or
    <!-- output: produce --> equals that example's output byte for byte;
  * every 64-hex digest, and every abbreviated digest such as 1234abcd…ef01,
    is one the harness knows: the example outputs, the SPEC_PIN vector hash,
    or the kat-small.json object hash;
  * a code block right after <!-- from: PATH --> is an excerpt of PATH: its
    non-blank lines (whitespace-trimmed) appear in PATH in order and without
    gaps, except where the block has a line that is just `...` or `// ...`.

Usage (from the repository root):
  python3 sdk/tools/check_docs.py --quickstart OUT.txt --produce OUT.txt
"""
import argparse, glob, json, re, sys

ap = argparse.ArgumentParser()
ap.add_argument("--quickstart", required=True)
ap.add_argument("--produce", required=True)
a = ap.parse_args()

version = open("sdk/VERSION").read().strip()
pin = dict(l.strip().split("=", 1) for l in open("sdk/SPEC_PIN") if "=" in l)
outputs = {"quickstart": open(a.quickstart).read(), "produce": open(a.produce).read()}
kat = json.load(open("spec/conformance/v2.0/kat-small.json"))
known = set(re.findall(r"\b[0-9a-f]{64}\b", outputs["quickstart"] + outputs["produce"]))
known |= {pin["vector-sha256"], kat["sha256"]}

files = sorted(set(glob.glob("sdk/**/*.md", recursive=True))
               - set(glob.glob("sdk/**/target/**", recursive=True)))
files += ["sdk/examples/rust/Cargo.toml", "sdk/examples/go/go.mod"]

VERSION_PATTERNS = [
    r"(?<![\w.])(\d+\.\d+\.\d+-(?:rc|alpha|beta)\.\d+)",
    r"sdk-v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]*[0-9A-Za-z])?)",
    r"sdk/go/v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]*[0-9A-Za-z])?)",
    r"/sdk/go/v2@v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]*[0-9A-Za-z])?)",
    r"pcoc-sdk-(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]*[0-9A-Za-z])?)\.jar",
]
problems = []
checked = {"versions": 0, "spec tags": 0, "outputs": 0, "digests": 0, "snippets": 0}


def excerpt_ok(block, source):
    want = [l.strip() for l in block if l.strip()]
    have = [l.strip() for l in source if l.strip()]
    segments, cur = [], []
    for l in want:
        if l in ("...", "// ..."):
            if cur:
                segments.append(cur)
            cur = []
        else:
            cur.append(l)
    if cur:
        segments.append(cur)
    at = 0
    for seg in segments:
        found = next((i for i in range(at, len(have) - len(seg) + 1) if have[i:i + len(seg)] == seg), None)
        if found is None:
            return seg
        at = found + len(seg)
    return None


for path in files:
    text = open(path).read()
    lines = text.split("\n")
    for n, line in enumerate(lines, 1):
        for pat in VERSION_PATTERNS:
            for m in re.finditer(pat, line):
                checked["versions"] += 1
                if m.group(1) != version:
                    problems.append(f"{path}:{n}: version {m.group(1)} is not sdk/VERSION {version}")
        for m in re.finditer(r"spec-v\d+\.\d+\.\d+", line):
            checked["spec tags"] += 1
            if m.group(0) != pin["tag"]:
                problems.append(f"{path}:{n}: {m.group(0)} is not the SPEC_PIN tag {pin['tag']}")
        for m in re.finditer(r"\b[0-9a-f]{64}\b", line):
            checked["digests"] += 1
            if m.group(0) not in known:
                problems.append(f"{path}:{n}: digest {m.group(0)[:16]}… is not one the harness produces or pins")
        for m in re.finditer(r"\b([0-9a-f]{6,})…([0-9a-f]{4,})\b", line):
            checked["digests"] += 1
            if not any(k.startswith(m.group(1)) and k.endswith(m.group(2)) for k in known):
                problems.append(f"{path}:{n}: abbreviated digest {m.group(0)} matches no known digest")
    # Marked blocks.
    for i, line in enumerate(lines):
        m = re.match(r"\s*<!-- (output|from): (\S+) -->\s*$", line)
        if not m:
            continue
        j = i + 1
        while j < len(lines) and not lines[j].strip():
            j += 1
        if j >= len(lines) or not lines[j].lstrip().startswith("```"):
            problems.append(f"{path}:{i + 1}: marker is not followed by a code block")
            continue
        k = j + 1
        while k < len(lines) and not lines[k].lstrip().startswith("```"):
            k += 1
        block = lines[j + 1:k]
        if m.group(1) == "output":
            checked["outputs"] += 1
            want = outputs.get(m.group(2))
            if want is None:
                problems.append(f"{path}:{i + 1}: unknown output {m.group(2)}")
            elif "\n".join(block) + "\n" != want:
                problems.append(f"{path}:{i + 1}: {m.group(2)} output block differs from what the example prints")
        else:
            checked["snippets"] += 1
            try:
                source = open(m.group(2)).read().split("\n")
            except OSError:
                problems.append(f"{path}:{i + 1}: {m.group(2)} does not exist")
                continue
            missing = excerpt_ok(block, source)
            if missing is not None:
                problems.append(f"{path}:{i + 1}: snippet is not an excerpt of {m.group(2)}; first unmatched run starts: {missing[0]!r}")

print("check_docs: " + ", ".join(f"{v} {k}" for k, v in checked.items()) + f" in {len(files)} files")
for p in problems:
    print("  " + p)
if checked["outputs"] < 6 or checked["snippets"] < 6:
    print("  expected at least 6 output blocks and 6 snippet blocks (2 and 2 per language README)")
    problems.append("too few marked blocks")
sys.exit(1 if problems else 0)
