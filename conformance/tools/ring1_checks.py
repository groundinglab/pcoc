#!/usr/bin/env python3
"""Ring 1 checks: SPEC section 6 names vs constants.json, the locked sign-in wire,
links that stay inside spec/ and conformance/, and the vector manifest hash.
Run from the repository root. Exit 1 on any failure. Short and deterministic."""
import hashlib, json, os, re, sys

SPEC = "spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md"
PACK = "conformance/v2.0"
fails = []

def check(name, ok, detail=""):
    print(("ok   " if ok else "FAIL ") + name + (f": {detail}" if detail and not ok else ""))
    if not ok:
        fails.append(name)

spec = open(SPEC, encoding="utf-8").read()

# 1. section 6: every name in the table has a matching value in constants.json (#302).
sec6 = spec[spec.index("## 6. Constants"):]
sec6 = sec6[: sec6.index("\n## 7")]
rows = re.findall(r"^\| `([A-Z][A-Z0-9_]+)` \| (.+?) \|$", sec6, re.M)
consts = json.load(open(f"{PACK}/constants.json"))
def spec_value(raw):
    raw = raw.strip()
    m = re.match(r"^`(0x[0-9A-F]+)`$", raw)
    if m: return m.group(1)
    m = re.match(r"^the (\d+) bytes `([^`]+)`( followed by a NUL byte)?$", raw)
    if m: return (m.group(2).encode() + (b"\0" if m.group(3) else b"")).hex()
    m = re.match(r"^`([^`]+)`$", raw)
    if m: return m.group(1)
    return int(raw.split()[0])
check("section6 has 22 names", len(rows) == 22, str(len(rows)))
for name, raw in rows:
    check(f"section6 {name}", consts.get(name) == spec_value(raw), f"json={consts.get(name)!r} spec={spec_value(raw)!r}")

# 2. sign_in_spec_locks_the_wire: the SPEC locks the section 15.1 wire and sign-in.json matches it.
si = json.load(open(f"{PACK}/sign-in.json"))
check("sign_in_spec_locks_the_wire: lock sentence",
      "Locked 2026-10-10: the section 15.1 sign-in wire." in spec)
wire = {
    "origin": f"The only signature origin is `{si['origin']}`.",
    "domain": f"It names the challenge-bytes domain `{si['domain']}`.",
    "lifetime_s": f"The lifetime is {si['lifetime_s']} seconds.",
    "door": f"`{si['method']} {si['door']}`",
    "content_type": f"The content type is `{si['content_type']}`.",
    "client_ok": f"`{si['client_ok']}`",
    "example.link": f"The link is `{si['example']['link']}`.",
}
for k, needle in wire.items():
    check(f"sign_in_spec_locks_the_wire: {k}", needle in spec, needle)
for tok in si["website_outcomes"]:
    check(f"sign_in_spec_locks_the_wire: outcome {tok}", f"`{tok}`" in spec)

# 3. no relative link leaves spec/ or conformance/.
for top in ("spec", "conformance"):
    for d, _, fs in os.walk(top):
        for f in fs:
            if not f.endswith(".md"):
                continue
            p = os.path.join(d, f)
            for u in re.findall(r"\]\(([^)#\s]+)", open(p, encoding="utf-8").read()):
                if re.match(r"^[a-z]+:", u):
                    continue
                t = os.path.normpath(os.path.join(d, u))
                inside = t.split(os.sep)[0] in ("spec", "conformance")
                check(f"link {p} -> {u}", inside and os.path.exists(t), t)

# 4. vector hash: every file under conformance/ except tools/, sorted by path.
h = hashlib.sha256()
for root, _, fs in sorted(os.walk("conformance")):
    if "/tools" in root + "/" and root.startswith("conformance/tools"):
        continue
    for f in sorted(fs):
        p = os.path.join(root, f)
        h.update(p.encode() + b"\0" + hashlib.sha256(open(p, "rb").read()).digest())
print("vector-sha256", h.hexdigest())

print(f"{len(fails)} failures")
sys.exit(1 if fails else 0)
