#!/usr/bin/env python3
"""Rust / Go / Java verify cross-check.

Runs the three `satoshi-hike-verify` programs on the same inputs and fails
unless all three write byte-identical stdout (the section 14 measurement
record) and exit with the same code. With --lib-rust/--lib-go/--lib-java-classes
it also runs the sdk/examples `librecord` programs, which print the record each
SDK's verify call returns (the programs may withhold it), and compares those.

Inputs, all from spec/conformance/v2.0 (the source of truth):
  * kat-small.json and test-3leg.json objects, served their recorded index
    replies by a local HTTP index (GET /index/<height>; a null body is a 404);
  * earlier-kind.cbor;
  * deterministic mutants of the kat-small object (byte flips, CBOR head bytes,
    truncations, a trailing byte), so the three must also agree on refusals
    and mismatches;
  * targeted section 14.1 edits: one field absent, mistyped or out of order;
  * one-byte CBOR items that are not a map (uint, empty array, empty text);
  * several audit seeds per object, so the drawn legs differ between runs.

Usage (from the repository root):
  python3 sdk/tools/crosscheck.py --rust BIN --go BIN --java-classes DIR \
      [--lib-rust BIN --lib-go BIN --lib-java-classes DIR]
Exit 0 and a final "crosscheck PASS" line on agreement; exit 1 otherwise.
"""
import argparse, binascii, hashlib, http.server, json, os, subprocess, sys, tempfile, threading

PACK = "spec/conformance/v2.0"

# Open disagreements between the three verify programs (sdk/KNOWN-DIVERGENCES.md).
# Each entry pins the first 16 hex digits of the SHA-256 of the Rust, Go and
# Java stdout ("-" pins an empty stdout). A listed case that starts to agree, a
# listed case whose bytes change, or any unlisted disagreement fails the run.
KNOWN_DIVERGENCES = {
    # one well-formed CBOR item that is not a map: Rust writes a field set
    # record (exit 0); Go/Java treat it as undecodable (exit 1, no record).
    # Open spec question (section 14.1 rows 1 and 2), not a known bug.
    "cbor-uint": ("c83953b6c66dfe1f", "-", "-"),
    "cbor-array": ("c83953b6c66dfe1f", "-", "-"),
    "cbor-text": ("c83953b6c66dfe1f", "-", "-"),
    # a checkpoint of the wrong width (31 bytes) in stage 2: all three differ.
    # Rust records W_3 `mismatch` and the stage-2 leg `mismatch` and leaves the
    # checkpoint counts `not defined`; Go and Java record W_3 `not defined`, the
    # leg `match`, and count the element; Go leaves legs_outside `not defined`,
    # Java counts 17. Found in this pass; not yet fixed.
    "field-s2-cpwidth": ("9ab891c42b95fafd", "bac1fe407e6aa300", "dd8b5bbfcf612d01"),
}


# Library-level disagreements (Rust verify_sized + verify_record_json, Go
# Verify + Record.Marshal, Java Measure.record), pinned the same way;
# "none" is a Java null. Filled from a run; see sdk/KNOWN-DIVERGENCES.md.
LIB_DIVERGENCES = {
    # Rust: a `field set` record; Go: a `malformed cbor` record; Java: null.
    "cbor-uint": ("e243d445c914e094", "9eb729dc587187e9", "none"),
    "cbor-array": ("e243d445c914e094", "9eb729dc587187e9", "none"),
    "cbor-text": ("e243d445c914e094", "9eb729dc587187e9", "none"),
    # the program-level three-way split above, at the library level.
    "field-s2-cpwidth": ("d65a1f5efcd40b30", "57441d8ac5b43ce6", "2dd10956cbbe5334"),
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
        head = [0x1B, 0x5B, 0xBB, 0xFF, 0x00, 0x18][k % 6]
        # A head equal to the byte already there would duplicate kat-small.
        m[at] = head if head != kat_obj[at] else head ^ 0x80
        cases.append((f"kat-head@{at}", bytes(m), kat_docs))
    for cut in (0, 1, 8, n // 4, n // 2, n - 1):
        cases.append((f"kat-cut@{cut}", kat_obj[:cut], kat_docs))
    cases.append(("kat-trailing", kat_obj + b"\x00", kat_docs))
    # Targeted section 14.1 cases: one field absent, mistyped or out of order,
    # everything else intact. Each must list every violated condition and
    # still record the later measurements.
    for name, obj in field_cases(kat_obj):
        cases.append((name, obj, kat_docs))
    # One well-formed CBOR item that is not a map (section 14.1 rows 1 and 2).
    cases.append(("cbor-uint", b"\x01", kat_docs))
    cases.append(("cbor-array", b"\x80", kat_docs))
    cases.append(("cbor-text", b"\x60", kat_docs))
    return cases


def nth(data, needle, k):
    at = -1
    for _ in range(k + 1):
        at = data.index(needle, at + 1)
    return at


def field_cases(obj):
    """Byte-level edits of the kat-small object that keep it one CBOR map."""
    def edit(fn):
        m = bytearray(obj)
        fn(m)
        return bytes(m)

    def rename(needle, k):
        # Change the last letter of the k-th occurrence of a key: that key is absent.
        def fn(m):
            m[nth(obj, needle, k) + len(needle) - 1] = ord("X")
        return fn

    def beacon_value(k):
        at = nth(obj, b"\x66beacon\x58\x20", k) + 9
        return at, obj[at:at + 32]

    def slot_as_text(m):
        at = nth(obj, b"\x6aslot_bytes\x18", 0) + 11
        m[at] = 0x61  # text of length 1 in place of a uint
    def li_equals_stage17(m):
        at = nth(obj, b"\x6elast_interrupt\x58\x20", 0) + 17
        m[at:at + 32] = beacon_value(17)[1]
    def leg_count_two(m):
        at = nth(obj, b"\x69leg_count\x01", 3) + 10
        m[at] = 0x02  # stage 3 says 2 legs and stores 1
    def order_and_hash(m):
        at = nth(obj, b"\x6dbeacon_height\x19", 4) + 16
        m[at] -= 1  # stage 4 height equals stage 3
        at10, _ = beacon_value(10)
        m[at10:at10 + 32] = beacon_value(9)[1]  # stage 10 hash equals stage 9

    def short_checkpoint(obj):
        # Stage 2's one checkpoint becomes 31 bytes (the object gets one byte shorter).
        at = nth(obj, b"\x6bcheckpoints\x81\x58\x20", 2) + 13
        return obj[:at] + b"\x58\x1f" + obj[at + 2:at + 33] + obj[at + 34:]

    return [
        ("field-slot-text", edit(slot_as_text)),
        ("field-s0-beacon", edit(rename(b"\x66beacon", 0))),
        ("field-s5-beacon", edit(rename(b"\x66beacon", 5))),
        ("field-s1-height", edit(rename(b"\x6dbeacon_height", 1))),
        ("field-s3-legcount", edit(rename(b"\x69leg_count", 3))),
        ("field-s3-legcount-2", edit(leg_count_two)),
        ("field-li-equal", edit(li_equals_stage17)),
        ("field-order-hash", edit(order_and_hash)),
        ("field-s2-cpwidth", short_checkpoint(obj)),
    ]


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
    ap.add_argument("--lib-rust", help="sdk/examples/rust librecord binary")
    ap.add_argument("--lib-go", help="sdk/examples/go librecord binary")
    ap.add_argument("--lib-java-classes", help="classes dir with LibRecord and the Java SDK")
    a = ap.parse_args()
    libs = None
    if a.lib_rust and a.lib_go and a.lib_java_classes:
        libs = {
            "rust": [a.lib_rust],
            "go": [a.lib_go],
            "java": ["java", "-cp", a.lib_java_classes, "LibRecord"],
        }

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
    lib_runs, lib_known_seen = 0, set()
    with tempfile.TemporaryDirectory() as tmp:
        for name, obj, docs in load_cases():
            Index.docs = docs
            path = os.path.join(tmp, "object.bin")
            open(path, "wb").write(obj)
            docs_path = os.path.join(tmp, "docs.json")
            # a null body is a 404: leave that height out
            json.dump({h: b for h, b in docs.items() if b is not None}, open(docs_path, "w"))
            for seed in seeds if name in ("kat-small", "test-3leg", "earlier-kind") else seeds[:1]:
                args = ["--input", path, "--index-url", url, "--profile", "conformance-small", "--audit-seed", seed]
                out = {lang: run(cmd + args, a.timeout) for lang, cmd in langs.items()}
                runs += 1
                codes = {lang: o[0] for lang, o in out.items()}
                digests = {lang: hashlib.sha256(o[1]).hexdigest()[:16] if o[1] else "-" for lang, o in out.items()}
                same = len(set(codes.values())) == 1 and len({o[1] for o in out.values()}) == 1
                if out["rust"][1]:
                    decoded += 1
                known = name in KNOWN_DIVERGENCES
                pinned = (digests["rust"], digests["go"], digests["java"])
                if same and not known:
                    status = "ok  "
                elif not same and known and KNOWN_DIVERGENCES[name] == pinned:
                    status = "KNOWN"
                    known_seen.add(name)
                else:
                    status = "FAIL"
                    why = ""
                    if same:
                        why = " (listed as known but now agrees: remove it)"
                    elif known:
                        why = f" (known divergence changed bytes: pinned {KNOWN_DIVERGENCES[name]}, got {pinned})"
                    fails.append(name + why)
                print(f"{status:<5} {name:<16} seed={seed[:8]} exit={codes['rust']} record={digests['rust']}"
                      + ("" if same else f"  codes={codes} digests={digests}"))
                if libs is None:
                    continue
                # Library level: the record each SDK's verify call returns for
                # the same input (Rust verify_sized, Go Verify, Java
                # Measure.record), whatever its program then does with it.
                lib = {lang: run(cmd + [path, docs_path, seed], a.timeout) for lang, cmd in libs.items()}
                lib_runs += 1
                if any(o[0] != 0 for o in lib.values()):
                    fails.append(f"lib {name} (harness exit {[o[0] for o in lib.values()]})")
                    continue
                body = {lang: o[1].rstrip(b"\n") for lang, o in lib.items()}
                ld = tuple(hashlib.sha256(body[lang]).hexdigest()[:16] if body[lang] != b"<no record>" else "none"
                           for lang in ("rust", "go", "java"))
                lsame = len(set(body.values())) == 1
                lknown = name in LIB_DIVERGENCES
                if lsame and not lknown:
                    continue
                if not lsame and lknown and LIB_DIVERGENCES[name] == ld:
                    lib_known_seen.add(name)
                    print(f"LIBKN {name:<16} seed={seed[:8]} rust/go/java={ld}")
                    continue
                why = " (listed as known but now agrees: remove it)" if lsame else (
                    f" (pinned {LIB_DIVERGENCES[name]}, got {ld})" if lknown else f" ({ld})")
                print(f"LIBFL {name:<16} seed={seed[:8]} rust/go/java={ld}")
                fails.append(f"lib {name}{why}")
    server.shutdown()
    base_ok = decoded >= 3
    if not base_ok:
        fails.append(f"only {decoded} runs wrote a record; expected the base objects to decode")
    missing = set(KNOWN_DIVERGENCES) - known_seen
    if missing:
        fails.append(f"known divergence cases not run: {sorted(missing)}")
    print(f"{len(load_cases())} inputs, {runs} runs x 3 languages, {decoded} wrote a record, {len(known_seen)} known divergences (sdk/KNOWN-DIVERGENCES.md)")
    if libs is not None:
        lib_missing = set(LIB_DIVERGENCES) - lib_known_seen
        if lib_missing:
            fails.append(f"known library divergence cases not run: {sorted(lib_missing)}")
        print(f"library level: {lib_runs} runs x 3 languages, {len(lib_known_seen)} known library divergences")
    if fails:
        print("crosscheck FAIL: " + ", ".join(fails))
        return 1
    print("crosscheck PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
