#!/usr/bin/env python3
"""Ring 1 checks for Satoshi Hike 2.2.

Run from the repository root. Exit 1 on any failure. Short and deterministic.

The 2.0 script opened spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md and
spec/conformance/v2.0/. Those paths are gone. This script reads
spec/satoshi-hike-v2.2/SPEC.md, ERRATUM-1.md, spec/conformance/pack/, and
sdk/SPEC_PIN.

Replacements for the four 2.0 checks:

1. Section 6 vs constants.json (22 names). The 2.2 pack has no constants.json:
   the previous file named another KIND and a seed-domain label, and section 6
   says there is no seed-domain label. The replacement parses the section 6
   table in SPEC.md and checks each name against the value written there.
2. sign-in.json locks the wire. The pack publishes sign-in.json and
   sign-in-origins.json. The check reads the section 15.1 sentences in
   SPEC.md and the example digest in sign-in.json.
3. Relative links under spec/ stay inside spec/, with the same spec/README.md
   parent allowlist.
4. A hash of every file under spec/conformance/ except tools/. Rule I5 pins the
   SHA-256 of spec/conformance/pack/manifest.json in sdk/SPEC_PIN, not a tree
   hash. The replacement checks that pin, that every manifest path exists, and
   that blockhash-display.json reverses the genesis digest as section 9 states.
"""
import hashlib, json, os, re, sys

SPEC = "spec/satoshi-hike-v2.2/SPEC.md"
ERRATUM = "spec/satoshi-hike-v2.2/ERRATUM-1.md"
PACK = "spec/conformance/pack"
PIN = "sdk/SPEC_PIN"
# spec/README.md may link these repository parents. No other page under spec/ may.
README_PARENTS = {
    "sdk", "services", "apps", "docs",
    "STATUS.md", "SECURITY.md", "CONTRIBUTING.md",
    "LICENSE-DOCS.md", "README.md", "PRINCIPLES.md", "LICENSE",
}
# Section 6 table, transcribed from SPEC.md. A byte string is stored as hex.
SECTION6 = {
    "SLOT_BYTES": 16384,
    "WORDS_PER_SLOT": 2048,
    "TERRAIN_BYTES": 268435456,
    "N_SLOTS": 16384,
    "SAMPLE_BYTES": 1024,
    "OFFSET_MODULUS": 268434433,
    "STEPS_PER_LEG": 2000000,
    "STAGES": 18,
    "HASH_BYTES": 32,
    "LEG_RESULT_BYTES": 8,
    "PUBLIC_KEY_BYTES": 32,
    "BEACON_BYTES": 32,
    "SEAL_DOMAIN": b"pcoc-walk-v1".hex(),
    "KIND": "v2.2-pcoc-object",
    "FILL": "dag2",
    "MIX_ADD": "0x9E3779B97F4A7C15",
    "MIX_MUL1": "0xBF58476D1CE4E5B9",
    "MIX_MUL2": "0x94D049BB133111EB",
    "WORD_XOR": "0xC0FFEE",
    "PARENT_MUL": "0xD1B54A32D192ED03",
    "SLOT_MUL": "0xA24BAED4963EE407",
}
fails = []

def check(name, ok, detail=""):
    print(("ok   " if ok else "FAIL ") + name + (f": {detail}" if detail and not ok else ""))
    if not ok:
        fails.append(name)

def spec_value(raw):
    raw = raw.strip()
    m = re.match(r"^`(0x[0-9A-F]+)`$", raw)
    if m:
        return m.group(1)
    m = re.match(r"^the (\d+) bytes `([^`]+)`$", raw)
    if m:
        return m.group(2).encode().hex()
    m = re.match(r"^`([^`]+)`$", raw)
    if m:
        return m.group(1)
    return int(raw.split()[0])

spec = open(SPEC, encoding="utf-8").read()
erratum = open(ERRATUM, encoding="utf-8").read()

check("erratum names SPEC.md", "spec/satoshi-hike-v2.2/SPEC.md" in erratum)
check("SPEC tag sentence", "Its tag is `spec-2.2.0`." in spec)

sec6 = spec[spec.index("## 6. Constants"):]
sec6 = sec6[: sec6.index("\n## 7")]
rows = re.findall(r"^\| `([A-Z][A-Z0-9_]+)` \| (.+?) \|$", sec6, re.M)
parsed = {name: spec_value(raw) for name, raw in rows}
check("section6 names", set(parsed) == set(SECTION6), sorted(set(parsed) ^ set(SECTION6)))
for name, want in SECTION6.items():
    check(f"section6 {name}", parsed.get(name) == want, f"spec={parsed.get(name)!r} want={want!r}")

# Section 15.1 sentences, and the pack's sign-in files.
for needle in (
    "Two origins are the same only when their serialized bytes are equal.",
    "It names the challenge-bytes domain `pcoc-gate-signin-v1`.",
    "`accepted`",
    "`pending`",
    "`rejected`",
    "`unknown_kind`",
):
    check(f"section15.1 {needle}", needle in spec, needle)
manifest = json.load(open(f"{PACK}/manifest.json", encoding="utf-8"))
listed = set(manifest.get("files", []))
check("pack lists sign-in.json", "sign-in.json" in listed and "sign-in-origins.json" in listed)
check("pack status complete", manifest.get("status") == "complete" and manifest.get("pack") == "pack-2.2.0")
sign_in = open(f"{PACK}/sign-in.json", encoding="utf-8").read()
check(
    "sign-in example digest",
    "90e38d7538d543aff5148318b7cb310595def390dac199c99bb10bc9ead5e0c2" in sign_in,
)

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

pin = {}
for line in open(PIN, encoding="utf-8"):
    line = line.strip()
    if not line or line.startswith("#") or "=" not in line:
        continue
    key, value = line.split("=", 1)
    pin[key] = value
manifest_bytes = open(f"{PACK}/manifest.json", "rb").read()
digest = hashlib.sha256(manifest_bytes).hexdigest()
check("SPEC_PIN tag", pin.get("tag") == "spec-2.2.0", pin.get("tag", ""))
check("SPEC_PIN vector-sha256", pin.get("vector-sha256") == digest, f"pin={pin.get('vector-sha256')} file={digest}")
for name in sorted(listed):
    check(f"pack file {name}", os.path.isfile(os.path.join(PACK, name)), name)

display = json.load(open(f"{PACK}/blockhash-display.json", encoding="utf-8"))
natural = bytes.fromhex(display["natural_block_hash"])
shown = bytes.fromhex(display["display_block_hash"])
check("blockhash display reversal", shown == natural[::-1] and len(natural) == 32)
header = bytes.fromhex(display["header_hex"])
check("blockhash header is 80 bytes", len(header) == 80, str(len(header)))

print("vector-sha256", digest)
print(f"{len(fails)} failures")
sys.exit(1 if fails else 0)
