"""Structural break cases for sdk/tools/crosscheck.py.

Each case decodes the kat-small object (spec/conformance/v2.0), edits it as a
CBOR value and re-encodes it, so one field or stage can be absent, mistyped,
of the wrong width or out of range while everything else stays intact. The
decode/encode pair below round-trips kat-small byte for byte (checked in
cases()). Plus raw byte inputs: non-map top levels (section 7.3 / 14.1 row 1)
and map-then-bytes inputs (the trailing-bytes row).

cases(pack) returns (name, object bytes, index documents, seed or None).
A seed pins the six-leg draw for cases that need one particular leg drawn.
"""
import copy, json, binascii

# With audit seed 01*32 the draw over kat-small with a second leg in stage 3
# (leg_total 19) picks global index 4 = stage 3 leg j=1, whose previous is the
# stored checkpoints[0]. 0594e6d98cd90356 is le64 of the leg recomputed from
# the first 8 bytes of kat-small's stage-3 checkpoint (section 11.2), so that
# leg is `match` exactly when a verifier reads checkpoints[0][0:8].
DRAW_S3_J1 = "01" * 32


class M(list):  # ordered map: list of [key, value]
    def get(self, k):
        for kk, v in self:
            if kk == k: return v
        raise KeyError(k)
    def set(self, k, v):
        for p in self:
            if p[0] == k: p[1] = v; return
        self.append([k, v])
    def rename(self, k, new):
        for p in self:
            if p[0] == k: p[0] = new; return
        raise KeyError(k)
    def drop(self, k):
        self[:] = [p for p in self if p[0] != k]
def dec(b, i=0):
    ib = b[i]; mt, ai = ib >> 5, ib & 31; i += 1
    if ai < 24: n = ai
    elif ai == 24: n = b[i]; i += 1
    elif ai == 25: n = int.from_bytes(b[i:i+2], "big"); i += 2
    elif ai == 26: n = int.from_bytes(b[i:i+4], "big"); i += 4
    elif ai == 27: n = int.from_bytes(b[i:i+8], "big"); i += 8
    else: raise ValueError("ai")
    if mt == 0: return n, i
    if mt == 1: return -1 - n, i
    if mt == 2: return bytes(b[i:i+n]), i + n
    if mt == 3: return b[i:i+n].decode(), i + n
    if mt == 4:
        out = []
        for _ in range(n): v, i = dec(b, i); out.append(v)
        return out, i
    if mt == 5:
        out = M()
        for _ in range(n):
            k, i = dec(b, i); v, i = dec(b, i); out.append([k, v])
        return out, i
    raise ValueError("mt")
def head(mt, n):
    if n < 24: return bytes([mt << 5 | n])
    if n < 256: return bytes([mt << 5 | 24, n])
    if n < 65536: return bytes([mt << 5 | 25]) + n.to_bytes(2, "big")
    if n < 2**32: return bytes([mt << 5 | 26]) + n.to_bytes(4, "big")
    return bytes([mt << 5 | 27]) + n.to_bytes(8, "big")
def enc(v):
    if isinstance(v, bool): return b"\xf5" if v else b"\xf4"
    if v is None: return b"\xf6"
    if isinstance(v, int): return head(0, v) if v >= 0 else head(1, -1 - v)
    if isinstance(v, (bytes, bytearray)): return head(2, len(v)) + bytes(v)
    if isinstance(v, str): e = v.encode(); return head(3, len(e)) + e
    if isinstance(v, M): return head(5, len(v)) + b"".join(enc(k) + enc(x) for k, x in v)
    if isinstance(v, list): return head(4, len(v)) + b"".join(enc(x) for x in v)
    if isinstance(v, Raw): return v.b
    raise TypeError(type(v))
class Raw:
    def __init__(self, b): self.b = b


def cases(pack):
    k = json.load(open(f"{pack}/kat-small.json"))
    docs = {str(d["height"]): d["body"] for d in k["documents"]}
    raw = binascii.unhexlify(k["object_hex"])
    base, end = dec(raw)
    assert end == len(raw) and enc(base) == raw, "kat-small does not round-trip"
    out = []
    def add(name, fn, seed=None):
        v = copy.deepcopy(base); r = fn(v)
        out.append((name, r if isinstance(r, bytes) else enc(v), docs, seed))
    st = lambda v, g: v.get("stages")[g]
    def cpw(g, n=31, j=0):
        def f(v): c = st(v, g).get("checkpoints"); c[j] = c[j][:n] if n < 32 else c[j] + b"\0" * (n - 32)
        return f
    for g in (0, 1, 2, 9, 16, 17): add(f"cpw-s{g}", cpw(g))
    add("cpw-s4-33", cpw(4, 33))
    add("cpw-s4-0", cpw(4, 0))
    add("cp-text-s4", lambda v: st(v, 4).get("checkpoints").__setitem__(0, "x" * 32))
    # F9: a checkpoint or leg-result element that is not bytes is a wrong
    # value type (section 14.1 row 2) as well as the width condition.
    add("cp-uint-s4", lambda v: st(v, 4).get("checkpoints").__setitem__(0, 7))
    add("cp-map-s4", lambda v: st(v, 4).get("checkpoints").__setitem__(0, M()))
    add("cp-arr-s4", lambda v: st(v, 4).get("checkpoints").__setitem__(0, []))
    add("lr-text-s4", lambda v: st(v, 4).get("leg_results").__setitem__(0, "short"))
    add("lr-uint-s4", lambda v: st(v, 4).get("leg_results").__setitem__(0, 7))
    add("lr-map-s4", lambda v: st(v, 4).get("leg_results").__setitem__(0, M()))
    add("lr-arr-s4", lambda v: st(v, 4).get("leg_results").__setitem__(0, []))
    def two_cps(first_bad, fix_lc):
        def f(v):
            s = st(v, 3); c = s.get("checkpoints"); l = s.get("leg_results")
            c.insert(0, c[0][:31] if first_bad else c[0]); l.insert(0, l[0])
            if fix_lc: s.set("leg_count", 2)
        return f
    add("cp2-s3-firstbad-lc2", two_cps(True, True))
    add("cp2-s3-firstbad-lc1", two_cps(True, False))
    add("cp2-s3-ok-lc2", two_cps(False, True))
    add("cpw-s3-lc2", lambda v: (cpw(3)(v), st(v, 3).set("leg_count", 2)))
    add("lrw-s4", lambda v: st(v, 4).get("leg_results").__setitem__(0, b"\0" * 7))
    add("lr-empty-s4", lambda v: st(v, 4).set("leg_results", []))
    add("lrw-cpw-s4", lambda v: (st(v, 4).get("leg_results").__setitem__(0, b"\0" * 7), cpw(4)(v)))
    for key in ("checkpoints", "leg_results", "waypoint", "path", "beacon", "beacon_height", "leg_count"):
        add(f"absent-s5-{key}", lambda v, key=key: st(v, 5).drop(key))
        add(f"text-s5-{key}", lambda v, key=key: st(v, 5).set(key, "x"))
    add("absent-s0-waypoint", lambda v: st(v, 0).drop("waypoint"))
    add("absent-s0-checkpoints", lambda v: st(v, 0).drop("checkpoints"))
    add("absent-s17-beacon", lambda v: st(v, 17).drop("beacon"))
    add("absent-s17-height", lambda v: st(v, 17).drop("beacon_height"))
    add("absent-s17-path", lambda v: st(v, 17).drop("path"))
    add("short-s5-waypoint", lambda v: st(v, 5).set("waypoint", b"\0" * 31))
    # F4: seed 11*32 draws stage 5 leg 0. A waypoint that is not 32 bytes
    # makes that leg mismatch (section 14.6 reads the stored waypoint).
    DRAW_S5_J0 = "11" * 32
    add("s5-waypoint-31", lambda v: st(v, 5).set("waypoint", st(v, 5).get("waypoint")[:31]), DRAW_S5_J0)
    add("s5-waypoint-33", lambda v: st(v, 5).set("waypoint", st(v, 5).get("waypoint") + b"\0"), DRAW_S5_J0)
    add("s5-waypoint-8", lambda v: st(v, 5).set("waypoint", st(v, 5).get("waypoint")[:8]), DRAW_S5_J0)
    add("short-s8-beacon", lambda v: st(v, 8).set("beacon", st(v, 8).get("beacon")[:31]))
    # 31 bytes is the wrong width (section 14.1 row 11), not a wrong type.
    # shape_item lists last_interrupt and does not list field set.
    add("short-li", lambda v: v.set("last_interrupt", v.get("last_interrupt")[:31]))
    for key in ("public_key", "seed_beacon", "stages", "last_interrupt", "last_interrupt_height", "kind", "n_slots"):
        add(f"absent-top-{key}", lambda v, key=key: v.drop(key))
    add("text-top-li-height", lambda v: v.set("last_interrupt_height", "x"))
    # F7: a text last_interrupt or last_interrupt_height is a wrong value type
    # (section 14.1 row 2), so field set is listed with last_interrupt.
    add("text-last_interrupt", lambda v: v.set("last_interrupt", "x"))
    add("text-last_interrupt_height", lambda v: v.set("last_interrupt_height", "not-a-height"))
    add("text-top-stages", lambda v: v.set("stages", "x"))
    add("short-pubkey", lambda v: v.set("public_key", v.get("public_key")[:31]))
    add("stages-17", lambda v: v.get("stages").pop())
    add("stages-19", lambda v: v.get("stages").append(copy.deepcopy(st(v, 17))))
    # F6: 19 stages, and stage 3's header duplicates stage 2's. stages length
    # does not drop the other conditions over the stages that are present.
    def stages19_dup(v):
        v.get("stages").append(copy.deepcopy(st(v, 17)))
        st(v, 3).set("beacon", st(v, 2).get("beacon"))
        st(v, 3).set("beacon_height", st(v, 2).get("beacon_height"))
    add("stages19-dup-header", stages19_dup)
    add("stage5-uint", lambda v: v.get("stages").__setitem__(5, 7))
    add("extra-top-key", lambda v: v.append(["zzz", 1]))
    add("extra-s5-key", lambda v: st(v, 5).append(["zzz", 1]))
    add("s0-path-key", lambda v: st(v, 0).append(["path", b""]))
    add("s17-path-nonempty", lambda v: st(v, 17).set("path", b"\1"))
    def bad_header_and_absent(v):
        st(v, 5).drop("beacon"); s6 = st(v, 6); s6.set("beacon_height", st(v, 4).get("beacon_height"))
    add("absent-s5-beacon-s6-low", bad_header_and_absent)
    add("absent-s5-beacon-s6-dup", lambda v: (st(v, 6).set("beacon", st(v, 5).get("beacon")), st(v, 5).drop("beacon")))
    add("absent-s5-height-s6-low", lambda v: (st(v, 5).drop("beacon_height"), st(v, 6).set("beacon_height", 0)))
    add("mixed-1", lambda v: (st(v, 3).set("leg_count", "x"), st(v, 9).drop("beacon_height"), v.set("slot_bytes", "x"), st(v, 12).set("beacon", st(v, 11).get("beacon"))))
    add("mixed-2", lambda v: (v.drop("fill"), st(v, 2).drop("checkpoints"), cpw(7)(v), st(v, 10).drop("beacon")))
    add("height-2^53", lambda v: st(v, 17).set("beacon_height", 2**53))
    # F5 / Q-a: header height order compares the exact unsigned integer.
    # A height above 2^53−1 is height_range and is written not defined, and
    # it still takes part in the order. 2^53 and 2^64−1 are greater than
    # stage 17's height, so those rows list height_range only.
    add("li-height-2^53", lambda v: v.set("last_interrupt_height", 2**53))
    add("li-height-2^64-1", lambda v: v.set("last_interrupt_height", 2**64 - 1))
    # Stage 17 above 2^53−1 and still below last_interrupt_height: no order violation.
    def s17_below_li(v):
        st(v, 17).set("beacon_height", 2**53)
        v.set("last_interrupt_height", 2**64 - 1)
    add("s17-h-2^53-lt-li", s17_below_li)
    # A stage height of 2^64−1 followed by the next stage's smaller height.
    add("s16-h-2^64-1", lambda v: st(v, 16).set("beacon_height", 2**64 - 1))
    # Q-j: a non-height integer above 2^53−1. Java lists height_range.
    # Rust and Go do not. Pinned, not decided.
    add("legcount-2^53-s5", lambda v: st(v, 5).set("leg_count", 2**53))
    add("nslots-2^64-1", lambda v: v.set("n_slots", 2**64 - 1))
    add("neg-s5-height", lambda v: st(v, 5).set("beacon_height", -1))
    add("leg_count-0-s5", lambda v: st(v, 5).set("leg_count", 0))
    add("all-empty-s5", lambda v: (st(v, 5).set("checkpoints", []), st(v, 5).set("leg_results", []), st(v, 5).set("leg_count", 0)))
    add("dup-top-key", lambda v: v.append(["fill", v.get("fill")]))
    add("nonshortest-int", lambda v: v.set("n_slots", Raw(b"\x1b" + v.get("n_slots").to_bytes(8, "big"))))
    add("absent-s1-waypoint", lambda v: st(v, 1).drop("waypoint"))
    add("absent-s2-waypoint", lambda v: st(v, 2).drop("waypoint"))
    add("stages-0", lambda v: v.set("stages", []))
    add("stages-1", lambda v: v.set("stages", v.get("stages")[:1]))
    add("text-s17-path", lambda v: st(v, 17).set("path", "x"))
    add("text-s1-path", lambda v: st(v, 1).set("path", "x"))
    add("stage0-uint", lambda v: v.get("stages").__setitem__(0, 7))
    add("stage17-uint", lambda v: v.get("stages").__setitem__(17, 7))
    add("stage1-uint", lambda v: v.get("stages").__setitem__(1, 7))
    def prev_bad(n):
        def f(v):
            s = st(v, 3); c = s.get("checkpoints"); l = s.get("leg_results")
            c[:] = [c[0][:n], c[0]]; l[:] = [l[0], bytes.fromhex("0594e6d98cd90356")]; s.set("leg_count", 2)
        return f
    add("cpw-prev-s3", prev_bad(31), DRAW_S3_J1)
    add("cpw-prev7-s3", prev_bad(7), DRAW_S3_J1)
    add("cp-prev-ok-s3", prev_bad(32), DRAW_S3_J1)
    def prev_wp_absent(v):
        prev_bad(32)(v); st(v, 3).drop("waypoint")
    add("cp-prev-nowp-s3", prev_wp_absent, DRAW_S3_J1)
    def prev_text(v):
        prev_bad(32)(v); st(v, 3).get("checkpoints")[0] = "abcdefghij"
    add("cp-prev-text-s3", prev_text, DRAW_S3_J1)
    # A text previous whose UTF-8 bytes would recompute the stored leg: still
    # mismatch, because section 14.6 reads a stored checkpoint (a byte string).
    def prev_text_forged(v):
        prev_text(v); st(v, 3).get("leg_results")[1] = bytes.fromhex("ab5034e88d67d965")
    add("cp-prev-text-forged-s3", prev_text_forged, DRAW_S3_J1)
    # No public key: no seed, so no terrain. A stored leg forged from an
    # all-zero key still reads mismatch (section 14.6 needs the seed inputs).
    def nokey_forged(v):
        prev_bad(32)(v); st(v, 3).get("leg_results")[1] = bytes.fromhex("8f4aa71f884f9a4c"); v.drop("public_key")
    add("cp-prev-nokey-forged-s3", nokey_forged, DRAW_S3_J1)
    # A trailing byte after a map whose fields are absent, mistyped or of the
    # wrong width (section 7.3 trailing bytes, combined with section 14.1).
    def trail(name, fn):
        v = copy.deepcopy(base); fn(v)
        out.append((name, enc(v) + b"\x00", docs, None))
    trail("trail-absent-top-public_key", lambda v: v.drop("public_key"))
    trail("trail-absent-top-stages", lambda v: v.drop("stages"))
    trail("trail-absent-s5-checkpoints", lambda v: st(v, 5).drop("checkpoints"))
    trail("trail-cpw-s2", cpw(2))
    trail("trail-stage5-uint", lambda v: v.get("stages").__setitem__(5, 7))
    trail("trail-legcount-2-s3", lambda v: st(v, 3).set("leg_count", 2))
    trail("trail-extra-s5-key", lambda v: st(v, 5).append(["zzz", 1]))
    trail("trail-extra-top-key", lambda v: v.append(["zzz", 1]))
    trail("trail-s0-path-key", lambda v: st(v, 0).append(["path", b""]))
    trail("trail-absent-top-last_interrupt", lambda v: v.drop("last_interrupt"))
    trail("trail-earlier-kind-only", lambda v: v.__setitem__(slice(None), M([["kind", "satoshi-hike-v1.35-object"]])))
    # F2: a declared length or count past the bytes still in the buffer.
    # Each is one CBOR head and nothing after it.
    for nm, b in [
        ("os-top-bstr-2pow63", bytes.fromhex("5b7fffffffffffffff")),
        ("os-top-text-2pow63", bytes.fromhex("7b7fffffffffffffff")),
        ("os-top-arr-2pow64-1", bytes.fromhex("9bffffffffffffffff")),
        ("os-top-map-2pow32", bytes.fromhex("baffffffff")),
        ("os-top-arr-1e9", bytes.fromhex("9a3b9aca00")),
    ]:
        out.append((nm, b, docs, None))
    # The same array head in place of kat-small's stages value.
    def stages_huge(v):
        v.set("stages", Raw(bytes.fromhex("9a3b9aca00")))
    add("os-stages-arr-1e9", stages_huge)
    full = enc(base)
    out.append(("indef-map", b"\xbf" + full[1 + (1 if full[0] & 31 >= 24 else 0):] + b"\xff", docs, None))
    out.append(("tagged-map", b"\xd8\x18" + full, docs, None))
    out.append(("map-then-map", full + full, docs, None))
    out.append(("map-then-01", full + b"\x01", docs, None))
    for nm, b in [("nm-bytes", b"\x40"), ("nm-false", b"\xf4"), ("nm-null", b"\xf6"), ("nm-float", b"\xfb" + b"\0" * 8),
                  ("nm-neg", b"\x20"), ("nm-tag-uint", b"\xc1\x01"), ("nm-01-00", b"\x01\x00"), ("nm-arr-of-map", b"\x81" + full),
                  ("nm-indef-arr", b"\x9f\xff"), ("nm-empty-map", b"\xa0"), ("nm-map-uintkey", b"\xa1\x01\x01"),
                  ("nm-kind-only", enc(M([["kind", base.get("kind")]]))), ("nm-earlier-kind-only", enc(M([["kind", "satoshi-hike-v1.35-object"]])))]:
        out.append((nm, b, docs, None))
    return out
