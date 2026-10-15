#!/usr/bin/env python3
# strfry writePolicy v2 (2026-10-14, pcoc-private #383 AC8)
# 1. accept only pubkeys in /etc/strfry/allowlist.txt (re-read each event; edits need no restart)
# 2. kinds 30009 (badge definition), 8 (badge award), 1742 (name record) only from the issuer pubkey(s) in /etc/strfry/issuer.txt
import sys, json
F = "/etc/strfry/allowlist.txt"
I = "/etc/strfry/issuer.txt"
ISSUER_KINDS = {30009, 8, 1742}
def keys(path):
    try:
        return {l.split('#')[0].strip().lower() for l in open(path) if l.split('#')[0].strip()}
    except Exception:
        return set()
for line in sys.stdin:
    try:
        req = json.loads(line)
        ev = req["event"]
        pk = ev["pubkey"].lower()
        res = {"id": ev["id"], "action": "accept"}
        if pk not in keys(F):
            res = {"id": ev["id"], "action": "reject", "msg": "blocked: pubkey not on the relay.groundinglab.io write allowlist"}
        elif ev.get("kind") in ISSUER_KINDS and pk not in keys(I):
            res = {"id": ev["id"], "action": "reject", "msg": "blocked: kind reserved for the groundinglab issuer"}
    except Exception:
        res = {"id": "", "action": "reject", "msg": "error: policy failure"}
    sys.stdout.write(json.dumps(res) + "\n"); sys.stdout.flush()
