#!/usr/bin/env python3
"""Fail unless spec/conformance hashes to the value in sdk/SPEC_PIN.
Same hash as spec/conformance/tools/ring1_checks.py: sha256 over every file
under spec/conformance/ except spec/conformance/tools/, sorted, each as path, NUL, sha256(bytes). Run from the repo root."""
import hashlib, os, sys
pin = dict(l.strip().split("=", 1) for l in open("sdk/SPEC_PIN") if "=" in l)
h = hashlib.sha256()
for root, _, fs in sorted(os.walk("spec/conformance")):
    if "/tools" in root + "/" and root.startswith("spec/conformance/tools"):
        continue
    for f in sorted(fs):
        p = os.path.join(root, f)
        h.update(p.encode() + b"\0" + hashlib.sha256(open(p, "rb").read()).digest())
got = h.hexdigest()
print(f"tag {pin['tag']} pinned {pin['vector-sha256']} got {got}")
sys.exit(0 if got == pin["vector-sha256"] else 1)
