#!/usr/bin/env python3
"""Drop G1 hits whose exact "path:line" text is in the allowlist file (argv[1]); print the rest."""
import re
import sys

allow = set()
try:
    with open(sys.argv[1], encoding="utf-8", newline="") as f:
        for raw in f.read().split("\n"):
            raw = raw.rstrip("\r")
            if raw and not raw.startswith("#"):
                allow.add(raw)
except FileNotFoundError:
    pass
for hit in sys.stdin.read().split("\n"):
    if not hit:
        continue
    m = re.match(r"^(?:\./)?([^:]+):\d+:(.*?)\r?$", hit)
    if m and f"{m.group(1)}:{m.group(2)}" in allow:
        continue
    print(hit)
