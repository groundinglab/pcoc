#!/usr/bin/env python3
"""Rust / Go / Java verify cross-check.

Runs the three `satoshi-hike-verify` programs on the same inputs and fails
unless all three write byte-identical stdout (the section 14 measurement
record) and exit with the same code.

Inputs, all from spec/conformance/v2.0 (the source of truth):
  * kat-small.json and test-3leg.json objects, served their recorded index
    replies by a local HTTP index (GET /index/<height>; a null body is a 404);
  * earlier-kind.cbor;
  * deterministic mutants of the kat-small object (byte flips, CBOR head bytes,
    truncations), so the three must also agree on refusals and mismatches;
  * several audit seeds per object, so the drawn legs differ between runs.

Usage (from the repository root):
  python3 sdk/tools/crosscheck.py --rust BIN --go BIN --java-classes DIR
Exit 0 and a final "crosscheck PASS" line on agreement; exit 1 otherwise.
"""
import argparse, binascii, hashlib, http.server, json, os, subprocess, sys, tempfile, threading

PACK = "spec/conformance/v2.0"

# Open Rust vs Go/Java disagreements found by this harness on 18 Oct 2026 and
# present on main before the SDK move (reference Rust at 565add5 gives the
# same bytes). Go and Java agree with each other in every case. Not fixed in
# the move: see sdk/KNOWN-DIVERGENCES.md. A listed case that starts to agree,
# or any unlisted disagreement, fails the run, so this list can only shrink.
KNOWN_DIVERGENCES = {
    "kat-flip@2",     # field set: Rust stops measuring; Go/Java also report constant + measure on
    "kat-head@2",     # same object as kat-flip@2 (head byte gives the same flip)
    "kat-flip@562",   # field set in a stage map: Rust stops; Go/Java add header height order + measure on
    "kat-head@562",   # same as kat-flip@562
    "kat-flip@3938",  # Rust omits shape_item header height order; Go/Java include it
}


def load_cases():
    cases = []
    kat = json.load(open(f"{PACK}/kat-small.json"))
    kat_docs = {str(d["height"]): d["body"] for d in kat["documents"]}
    kat_obj = binascii.unhexlify(kat["object_hex"])
    cases.append(("kat-small", kat_obj, kat_docs))
    t3 = json.load(open(f"{PACK}/test-3leg.json"))
    t3_docs = {str(r["height"]): r["body"] for r in t3["header_reads"] + t3["plant_reads"]}
    cases.append(("test-3leg", binascii.unhexlify(t3["object_hex"]), t3_docs))
    cases.append(("earlier-kind", open(f"{PACK}/earlier-kind.cbor", "rb").read(), kat_docs))
    # Deterministic mutants of the kat object.
    n = len(kat_obj)
    for k, at in enumerate(sorted({1, 2, 9, 17, 40, 64, n // 7, n // 3, n // 2, (2 * n) // 3, n - 40, n - 2})):
        m = bytearray(kat_obj)
        m[at] ^= 0x01
        cases.append((f"kat-flip@{at}", bytes(m), kat_docs))
        m = bytearray(kat_obj)
        m[at] = [0x1B, 0x5B, 0xBB, 0xFF, 0x00, 0x18][k % 6]
        cases.append((f"kat-head@{at}", bytes(m), kat_docs))
    for cut in (0, 1, 8, n // 4, n // 2, n - 1):
        cases.append((f"kat-cut@{cut}", kat_obj[:cut], kat_docs))
    cases.append(("kat-trailing", kat_obj + b"\x00", kat_docs))
    return cases


class Index(http.server.BaseHTTPRequestHandler):
    docs = {}

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        body = self.docs.get(path[len("/index/"):]) if path.startswith("/index/") else None
        if body is None:
            self.send_response(404)
            self.send_header("content-length", "0")
            self.end_headers()
            return
        raw = body.encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def log_message(self, *args):
        pass


def run(cmd, timeout):
    try:
        p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rust", required=True)
    ap.add_argument("--go", required=True)
    ap.add_argument("--java-classes", required=True)
    ap.add_argument("--timeout", type=int, default=120)
    a = ap.parse_args()

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Index)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{server.server_address[1]}"
    seeds = ["11" * 32, "a5" * 32, hashlib.sha256(b"pcoc-crosscheck").hexdigest()]
    langs = {
        "rust": [a.rust],
        "go": [a.go],
        "java": ["java", "-cp", a.java_classes, "pcoc.cli.Verify"],
    }
    fails, runs, decoded, known_seen = [], 0, 0, set()
    with tempfile.TemporaryDirectory() as tmp:
        for name, obj, docs in load_cases():
            Index.docs = docs
            path = os.path.join(tmp, "object.bin")
            open(path, "wb").write(obj)
            for seed in seeds if not name.startswith("kat-") or name == "kat-small" else seeds[:1]:
                args = ["--input", path, "--index-url", url, "--profile", "conformance-small", "--audit-seed", seed]
                out = {lang: run(cmd + args, a.timeout) for lang, cmd in langs.items()}
                runs += 1
                codes = {lang: o[0] for lang, o in out.items()}
                digests = {lang: hashlib.sha256(o[1]).hexdigest()[:16] for lang, o in out.items()}
                same = len(set(codes.values())) == 1 and len({o[1] for o in out.values()}) == 1
                if out["rust"][1]:
                    decoded += 1
                gojava = codes["go"] == codes["java"] and out["go"][1] == out["java"][1]
                known = name in KNOWN_DIVERGENCES
                if same and not known:
                    status = "ok  "
                elif not same and known and gojava:
                    status = "KNOWN"
                    known_seen.add(name)
                else:
                    status = "FAIL"
                    fails.append(name + (" (listed as known but now agrees: remove it)" if same else ""))
                print(f"{status:<5} {name:<16} seed={seed[:8]} exit={codes['rust']} record={digests['rust']}"
                      + ("" if same else f"  codes={codes} digests={digests}"))
    server.shutdown()
    base_ok = decoded >= 3
    if not base_ok:
        fails.append(f"only {decoded} runs wrote a record; expected the base objects to decode")
    missing = KNOWN_DIVERGENCES - known_seen
    if missing:
        fails.append(f"known divergence cases not run: {sorted(missing)}")
    print(f"{runs} cases x 3 languages, {decoded} wrote a record, {len(known_seen)} known Rust-vs-Go/Java divergences (sdk/KNOWN-DIVERGENCES.md)")
    if fails:
        print("crosscheck FAIL: " + ", ".join(fails))
        return 1
    print("crosscheck PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
