#!/usr/bin/env python3
"""Ring 1 checks: SPEC section 6 names vs constants.json, the locked sign-in wire,
links that stay inside spec/, and the vector manifest hash.
Run from the repository root. Exit 1 on any failure. Short and deterministic."""
import hashlib, json, os, re, sys

SPEC = "spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md"
# Section 6 is the v2.1 table. The locked sign-in wire stays on the v2.0 files.
PACK = "spec/conformance/v2.0"
CONSTS = "spec/conformance/v2.1/constants.json"
# spec/README.md may link these repository parents. No other page under spec/ may.
README_PARENTS = {
    "sdk", "services", "apps", "docs",
    "STATUS.md", "SECURITY.md", "CONTRIBUTING.md",
    "LICENSE-DOCS.md", "README.md", "PRINCIPLES.md", "LICENSE",
}
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
consts = json.load(open(CONSTS))
def spec_value(raw):
    raw = raw.strip()
    m = re.match(r"^`(0x[0-9A-F]+)`$", raw)
    if m: return m.group(1)
    m = re.match(r"^the (\d+) bytes `([^`]+)`( followed by a NUL byte)?$", raw)
    if m: return (m.group(2).encode() + (b"\0" if m.group(3) else b"")).hex()
    m = re.match(r"^`([^`]+)`$", raw)
    if m: return m.group(1)
    return int(raw.split()[0])
check("section6 has 21 names", len(rows) == 21, str(len(rows)))
for name, raw in rows:
    check(f"section6 {name}", consts.get(name) == spec_value(raw), f"json={consts.get(name)!r} spec={spec_value(raw)!r}")

# 2. sign_in_spec_locks_the_wire: the SPEC locks the section 15.1 wire and sign-in.json matches it.
si = json.load(open(f"{PACK}/sign-in.json"))
check("sign_in_spec_locks_the_wire: lock sentence",
      "Locked 2026-10-10: the section 15.1 sign-in wire." in spec)
wire = {
    "origin": f"`{si['origin']}` is one example deployment.",
    "origin rule": "Two origins are the same only when their serialized bytes are equal.",
    "origin vectors": "`spec/conformance/v2.0/sign-in-origins.json`",
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

# 3. relative links from spec/ stay inside spec/, except the spec/README.md parent allowlist.
for d, _, fs in os.walk("spec"):
    for f in fs:
        if not f.endswith(".md"):
            continue
        p = os.path.join(d, f)
        for u in re.findall(r"\]\(([^)#\s]+)", open(p, encoding="utf-8").read()):
            if re.match(r"^[a-z]+:", u):
                continue
            t = os.path.normpath(os.path.join(d, u))
            if p == "spec/README.md" and u.startswith("../"):
                head = u[3:].split("/", 1)[0]
                check(f"link {p} -> {u}", head in README_PARENTS and os.path.exists(t), t)
                continue
            inside = t.split(os.sep)[0] == "spec"
            check(f"link {p} -> {u}", inside and os.path.exists(t), t)

# 4. vector hash: every file under spec/conformance/ except tools/, sorted by path.
h = hashlib.sha256()
for root, _, fs in sorted(os.walk("spec/conformance")):
    if "/tools" in root + "/" and root.startswith("spec/conformance/tools"):
        continue
    for f in sorted(fs):
        p = os.path.join(root, f)
        h.update(p.encode() + b"\0" + hashlib.sha256(open(p, "rb").read()).digest())
print("vector-sha256", h.hexdigest())

print(f"{len(fails)} failures")
sys.exit(1 if fails else 0)
