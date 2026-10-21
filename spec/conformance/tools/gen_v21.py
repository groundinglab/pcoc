#!/usr/bin/env python3
"""Generate spec/conformance/v2.1 from Satoshi Hike v2.1.

The construction is §10 (seed, NL4, parents, slot_mix), §11.1 (C1 fold and
seal), §11.2–§11.4, and §12.2. Run from the repository root:

    python3 spec/conformance/tools/gen_v21.py

Optional self-check of the withdrawn fill-pin terrain plus the C1 fold
against the pinned driver outputs (not a v2.1 vector):

    python3 spec/conformance/tools/gen_v21.py --self-check /path/to/inputs
"""

import argparse
import hashlib
import json
import os
import subprocess

MASK = (1 << 64) - 1
MIX_ADD = 0x9E3779B97F4A7C15
MIX_MUL1 = 0xBF58476D1CE4E5B9
MIX_MUL2 = 0x94D049BB133111EB
WORD_XOR = 0xC0FFEE
PARENT_MUL = 0xD1B54A32D192ED03
SLOT_MUL = 0xA24BAED4963EE407
SEAL = b"pcoc-walk-v1"
# Withdrawn v2.0 fill-pin label. The fill-pin negative is the only use.
WITHDRAWN_V20_SEED_DOMAIN = b"PSEED64\x00"

# Pinned by VECTORS.md (unlabelled seed, pk 0x11, beacon 0x22).
SEED_HEX = "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c"
LANES = (
    "545dfe297dc78951",
    "27858669c45e046a",
    "9cdac73ac1a5fe85",
    "7c81df6efbf55f11",
)
NL4_SAMPLES = (
    (0, 0, "c7582082a632c2c8"),
    (0, 1, "6395f245d5934a68"),
    (0, 2, "dd3aaa1f4cc23a0b"),
    (0, 3, "23da68c5377ed50a"),
    (1, 0, "a9d24682c24f67c9"),
    (1, 1, "6f4529b850deed8f"),
    (7, 7, "38ac197bd39defe0"),
    (255, 255, "9e61ddf478cdd5c7"),
)
SLOT0_SHA = "307ad685fb999e9a5135ad89a43ea1f0b809996f47b841f04d125ce053e0e241"
PK2_LANES = (
    "57368a41f5ee27a5",
    "ce51fb630e892916",
    "0aed5364ed8cfd6b",
    "dcc127a76c751bab",
)
SEAL0 = "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533"
SEAL0_SPRIME = "76042423afbd1648"


def mix64(x):
    y = (x + MIX_ADD) & MASK
    y = ((y ^ (y >> 30)) * MIX_MUL1) & MASK
    y = ((y ^ (y >> 27)) * MIX_MUL2) & MASK
    return (y ^ (y >> 31)) & MASK


def bare64(x):
    """Finalizer with the + MIX_ADD omitted. Not mix64."""
    y = x & MASK
    y = ((y ^ (y >> 30)) * MIX_MUL1) & MASK
    y = ((y ^ (y >> 27)) * MIX_MUL2) & MASK
    return (y ^ (y >> 31)) & MASK


def rotl(w, r):
    r &= 63
    w &= MASK
    if r == 0:
        return w
    return ((w << r) | (w >> (64 - r))) & MASK


def reverse32(v):
    r = 0
    v &= 0xFFFFFFFF
    for k in range(32):
        if v & (1 << k):
            r |= 1 << (31 - k)
    return r


def floor_log2(n):
    return n.bit_length() - 1


def parents(i):
    bits = max(floor_log2(i), 1)
    bitrev = (reverse32(i) >> (32 - bits)) % i
    h = (i * MIX_ADD) & MASK
    mul = (h >> 32) % i
    return (i - 1, bitrev, mul)


def seed_digest(pk, beacon):
    return hashlib.sha256(pk + beacon).digest()


def lanes_of(digest):
    return [int.from_bytes(digest[8 * k : 8 * k + 8], "little") for k in range(4)]


def nl4_word(P, i, j):
    c = ((i << 32) ^ j ^ WORD_XOR) & MASK
    t = mix64(P[j % 4] ^ c)
    t = mix64(t ^ rotl(P[(j + 1) % 4], 17))
    t = mix64(t ^ rotl(P[(j + 2) % 4], 31))
    t = mix64(t ^ rotl(P[(j + 3) % 4], 47))
    return t


def ref_word(P, i, j):
    c = ((i << 32) ^ j ^ WORD_XOR) & MASK
    return mix64(P[j % 4] ^ c)


def all4_word(P, i, j):
    c = ((i << 32) ^ j ^ WORD_XOR) & MASK
    x = P[j % 4]
    x ^= rotl(P[(j + 1) % 4], 17)
    x ^= rotl(P[(j + 2) % 4], 31)
    x ^= rotl(P[(j + 3) % 4], 47)
    x ^= c
    return mix64(x & MASK)


def pair_word(P, i, j):
    """First two NL4 steps only. Not a terrain word."""
    c = ((i << 32) ^ j ^ WORD_XOR) & MASK
    t = mix64(P[j % 4] ^ c)
    t = mix64(t ^ rotl(P[(j + 1) % 4], 17))
    return t


def fill_pin(preimage):
    x = int.from_bytes(WITHDRAWN_V20_SEED_DOMAIN, "little")
    for i in range(8):
        lane = int.from_bytes(preimage[8 * i : 8 * i + 8], "little")
        x = mix64(x ^ lane ^ ((i * MIX_ADD) & MASK))
    return x


def pin_word(pin, i, j):
    return mix64(pin ^ ((i << 32) & MASK) ^ j ^ WORD_XOR)


def fill_terrain(base_word, P_or_pin, slot_bytes, terrain_bytes):
    words = slot_bytes // 8
    nslots = terrain_bytes // slot_bytes
    slots = []
    out = bytearray(terrain_bytes)
    for i in range(nslots):
        w = [base_word(P_or_pin, i, j) for j in range(words)]
        if i > 0:
            pars = parents(i)
            for k, p in enumerate(pars, start=1):
                pw = slots[p]
                for j in range(words):
                    rot = (j + k) % 64
                    w[j] = mix64((w[j] ^ rotl(pw[j], rot)) ^ ((k * PARENT_MUL) & MASK))
            sm = (i * SLOT_MUL) & MASK
            w = [mix64(word ^ sm) for word in w]
        slots.append(w)
        for j, word in enumerate(w):
            out[i * slot_bytes + 8 * j : i * slot_bytes + 8 * j + 8] = word.to_bytes(8, "little")
    return bytes(out)


def base_slot(word_fn, P, nwords):
    raw = bytearray()
    for j in range(nwords):
        raw += word_fn(P, 0, j).to_bytes(8, "little")
    return bytes(raw)


def seal_digest(folded):
    return hashlib.sha256(SEAL + (folded & MASK).to_bytes(8, "little")).digest()


def seal(folded):
    return int.from_bytes(seal_digest(folded)[:8], "little")


def fold_xor_acc(slot_bytes, S, mix):
    """Withdrawn XOR accumulator: acc XOR= mix(word XOR w XOR S). Not §11.1."""
    words = len(slot_bytes) // 8
    acc = 0
    for w in range(words):
        word = int.from_bytes(slot_bytes[8 * w : 8 * w + 8], "little")
        acc ^= mix((word ^ w ^ S) & MASK)
        acc &= MASK
    return acc


def fold_xor_w(slot_bytes, S, mix):
    """Withdrawn f0c5b63 fold: acc += mix(word XOR w XOR S). Negative xor_w_fold."""
    words = len(slot_bytes) // 8
    acc = 0
    for w in range(words):
        word = int.from_bytes(slot_bytes[8 * w : 8 * w + 8], "little")
        acc = (acc + mix((word ^ w ^ S) & MASK)) & MASK
    return acc


def fold_c1(slot_bytes, S, mix):
    """§11.1: acc = (acc + mix(word XOR ((S + w) mod 2^64))) mod 2^64."""
    words = len(slot_bytes) // 8
    acc = 0
    for w in range(words):
        word = int.from_bytes(slot_bytes[8 * w : 8 * w + 8], "little")
        mixed = (word ^ ((S + w) & MASK)) & MASK
        acc = (acc + mix(mixed)) & MASK
    return acc


def fold_v2_0(slot_bytes, S):
    """Withdrawn v2.0 fold: S XOR (XOR of (word XOR w)). Not the §11.1 fold."""
    words = len(slot_bytes) // 8
    acc = 0
    for w in range(words):
        word = int.from_bytes(slot_bytes[8 * w : 8 * w + 8], "little")
        acc ^= (word ^ w) & MASK
    return (S ^ acc) & MASK


def fold_partial(slot_bytes, S, nread):
    """Short read of the §11.1 wrapping-add fold."""
    acc = 0
    for w in range(nread):
        word = int.from_bytes(slot_bytes[8 * w : 8 * w + 8], "little")
        acc = (acc + mix64((word ^ ((S + w) & MASK)) & MASK)) & MASK
    return acc


def leg_result(terrain, slot_bytes, steps, S, fold):
    nslots = len(terrain) // slot_bytes
    words = slot_bytes // 8
    for _ in range(steps):
        i = S % nslots
        slot = terrain[i * slot_bytes : (i + 1) * slot_bytes]
        S = seal(fold(slot, S))
    return S


def checkpoint(terrain, sample_bytes, prev, limb, leg_u64):
    offmod = len(terrain) - sample_bytes + 1
    offset = int.from_bytes(prev[:8], "little") % offmod
    pre = prev + limb + terrain[offset : offset + sample_bytes] + leg_u64.to_bytes(8, "little")
    return hashlib.sha256(pre).digest()


def waypoint(terrain, slot_bytes, beacon, stage_w, checkpoints):
    fold = bytes(32)
    for c in checkpoints:
        fold = hashlib.sha256(fold + c).digest()
    nslots = len(terrain) // slot_bytes
    si = int.from_bytes(beacon[:8], "little") % nslots
    sample = terrain[si * slot_bytes : (si + 1) * slot_bytes]
    return hashlib.sha256(beacon + sample + stage_w + fold).digest()


def u64hex(n):
    return f"{n & MASK:016x}"


def meets_floor(legs, span, p, q):
    return span > 0 and q * legs >= p * span


def phase_indices(leg_total, draws, nrounds=3, per=6, stop_on_end=False):
    """§15.5 thorough draw. draws are the section 14.6 u64 values, in order.

    When leg_total >= 18 this rejection sampler is one without-replacement
    draw of 18 indices, and the three rounds are that sequence split into
    sixes. When 6 <= leg_total < 18 an index stays unused until every index
    has been chosen, and a later draw may then repeat.
    """
    if leg_total < 6:
        return []
    modulus = 1 << 64
    rem = modulus % leg_total
    limit = modulus if rem == 0 else modulus - rem
    phase = []
    rounds = []
    i = 0
    for _ in range(nrounds):
        chosen = []
        guard = 0
        while len(chosen) < per:
            if i >= len(draws):
                if stop_on_end:
                    return rounds
                raise SystemExit("rounds: byte source ended")
            u = draws[i]
            i += 1
            guard += 1
            if guard > 10000:
                raise SystemExit("rounds: did not fill")
            if u >= limit:
                continue
            q = u % leg_total
            if q in chosen:
                continue
            unused = any(x not in phase for x in range(leg_total))
            if q in phase and unused:
                continue
            chosen.append(q)
            phase.append(q)
        rounds.append(chosen)
    return rounds


def unxs(y, s):
    x = y & MASK
    y &= MASK
    k = s
    while k < 64:
        x = y ^ (x >> s)
        k += s
    return x & MASK


def inv64(a):
    x = a & MASK
    a &= MASK
    for _ in range(6):
        x = (x * ((2 - a * x) & MASK)) & MASK
    return x


def mix64_inv(z):
    y = unxs(z, 31)
    y = (y * inv64(MIX_MUL2)) & MASK
    y = unxs(y, 27)
    y = (y * inv64(MIX_MUL1)) & MASK
    y = unxs(y, 30)
    return (y - MIX_ADD) & MASK


def pin_x4(pk):
    x = int.from_bytes(WITHDRAWN_V20_SEED_DOMAIN, "little")
    for i in range(4):
        lane = int.from_bytes(pk[8 * i : 8 * i + 8], "little")
        x = mix64(x ^ lane ^ ((i * MIX_ADD) & MASK))
    return x


def f3_mate(pk1):
    pk2 = bytearray(pk1)
    pk2[0] ^= 0x5A
    pk2[9] ^= 0x3C
    x = int.from_bytes(WITHDRAWN_V20_SEED_DOMAIN, "little")
    for i in range(3):
        lane = int.from_bytes(pk2[8 * i : 8 * i + 8], "little")
        x = mix64(x ^ lane ^ ((i * MIX_ADD) & MASK))
    lane3 = mix64_inv(pin_x4(pk1)) ^ x ^ ((3 * MIX_ADD) & MASK)
    pk2[24:32] = (lane3 & MASK).to_bytes(8, "little")
    return bytes(pk2)


def dump(obj):
    return (json.dumps(obj, indent=2, ensure_ascii=False) + "\n").encode()


def parse_vec(path):
    lines = [ln.split() for ln in open(path) if ln.strip()]
    head = lines[0]
    slot_b, terr_b, sample_b, steps = (int(head[i]) for i in range(4))
    pk = bytes.fromhex(head[4])
    beacon = bytes.fromhex(head[5])
    nst = int(head[7])
    stages = []
    row = 1
    for _ in range(nst):
        stored_w = bytes.fromhex(lines[row][1])
        nlegs = int(lines[row][2])
        row += 1
        row += nlegs
        nxt = lines[row][0]
        row += 1
        next_b = None if nxt == "-" else bytes.fromhex(nxt)
        stages.append({"stored_w": stored_w, "nlegs": nlegs, "next": next_b})
    return {
        "slot_bytes": slot_b,
        "terrain_bytes": terr_b,
        "sample_bytes": sample_b,
        "steps": steps,
        "pk": pk,
        "beacon": beacon,
        "stages": stages,
    }


def walk(terrain, prof, opening, stages, chain, fold=None):
    """chain True: stage 0 opens at `opening` and each later stage opens at the
    previous waypoint. chain False: each stage opens at its stored waypoint.
    fold defaults to the §11.1 wrapping-add fold. The historical C1 pin uses
    the withdrawn XOR accumulator.
    """
    if fold is None:
        fold = lambda sl, S: fold_c1(sl, S, mix64)
    slot_b = prof["slot_bytes"]
    sample_b = prof["sample_bytes"]
    steps = prof["steps"]
    wcur = opening
    out = []
    for st in stages:
        limb = wcur if chain else st["stored_w"]
        prev = limb
        legs = []
        cps = []
        for _ in range(st["nlegs"]):
            S0 = int.from_bytes(prev[:8], "little")
            r = leg_result(terrain, slot_b, steps, S0, fold)
            c = checkpoint(terrain, sample_b, prev, limb, r)
            legs.append(r.to_bytes(8, "little").hex())
            cps.append(c.hex())
            prev = c
        nw = None
        if st["next"] is not None:
            nw = waypoint(terrain, slot_b, st["next"], limb, [bytes.fromhex(c) for c in cps]).hex()
            if chain:
                wcur = bytes.fromhex(nw)
        out.append({
            "leg_count": st["nlegs"],
            "next_beacon_hex": None if st["next"] is None else st["next"].hex(),
            "legs_le_hex": legs,
            "checkpoints_hex": cps,
            "next_waypoint_hex": nw,
        })
    return out


def self_check(inputs):
    pinned = {
        "kat-small": os.path.join(inputs, "expected", "C1-pinned", "kat-small_C1.json"),
        "test-3leg": os.path.join(inputs, "expected", "C1-pinned", "test-3leg_C1.json"),
    }
    vecs = {
        "kat-small": os.path.join(inputs, "reference", "vectors", "kat-small.vec"),
        "test-3leg": os.path.join(inputs, "reference", "vectors", "test-3leg.vec"),
    }
    for name, vp in vecs.items():
        spec = parse_vec(vp)
        pre = spec["pk"] + spec["beacon"]
        pin = fill_pin(pre)
        terrain = fill_terrain(pin_word, pin, spec["slot_bytes"], spec["terrain_bytes"])
        got_sha = hashlib.sha256(terrain).hexdigest()
        # C1-pinned files are the withdrawn XOR accumulator, not §11.1.
        got = walk(
            terrain,
            spec,
            spec["stages"][0]["stored_w"],
            spec["stages"],
            chain=False,
            fold=lambda sl, S: fold_xor_acc(sl, S, mix64),
        )
        exp = json.load(open(pinned[name]))
        if exp["terrain_sha256"] != got_sha:
            raise SystemExit(f"self-check {name} terrain {got_sha} != {exp['terrain_sha256']}")
        if len(exp["stages"]) != len(got):
            raise SystemExit(f"self-check {name} stage count")
        for i, (e, g) in enumerate(zip(exp["stages"], got)):
            if e["legs"] != g["legs_le_hex"] or e["checkpoints"] != g["checkpoints_hex"]:
                raise SystemExit(f"self-check {name} stage {i} legs/checkpoints differ")
            ew = e["next_waypoint"]
            if ew != g["next_waypoint_hex"]:
                raise SystemExit(f"self-check {name} stage {i} waypoint {g['next_waypoint_hex']} != {ew}")
        print(f"self-check {name} fill_pin+C1 matches pinned")


def assert_pins(pk, beacon):
    d = seed_digest(pk, beacon)
    if d.hex() != SEED_HEX:
        raise SystemExit("seed pin mismatch")
    P = lanes_of(d)
    if tuple(u64hex(x) for x in P) != LANES:
        raise SystemExit("lane pin mismatch")
    for i, j, hx in NL4_SAMPLES:
        if u64hex(nl4_word(P, i, j)) != hx:
            raise SystemExit(f"NL4 sample {i},{j} mismatch")
    slot = base_slot(nl4_word, P, 2048)
    if hashlib.sha256(slot).hexdigest() != SLOT0_SHA:
        raise SystemExit("slot0 pin mismatch")
    dig = seal_digest(0)
    if dig.hex() != SEAL0 or u64hex(int.from_bytes(dig[:8], "little")) != SEAL0_SPRIME:
        raise SystemExit("seal(0) pin mismatch")
    pk2 = f3_mate(pk)
    P2 = lanes_of(seed_digest(pk2, beacon))
    if tuple(u64hex(x) for x in P2) != PK2_LANES:
        raise SystemExit(f"pk2 lanes {tuple(u64hex(x) for x in P2)}")
    return d, P, pk2


def hike_doc(name, spec, chain_opening):
    d = seed_digest(spec["pk"], spec["beacon"])
    P = lanes_of(d)
    terrain = fill_terrain(nl4_word, P, spec["slot_bytes"], spec["terrain_bytes"])
    opening = d if chain_opening else spec["stages"][0]["stored_w"]
    stages = walk(terrain, spec, opening, spec["stages"], chain=True)
    return {
        "id": name,
        "class": "positive",
        "profile": {
            "slot_bytes": spec["slot_bytes"],
            "terrain_bytes": spec["terrain_bytes"],
            "sample_bytes": spec["sample_bytes"],
            "steps_per_leg": spec["steps"],
        },
        "public_key_hex": spec["pk"].hex(),
        "seed_beacon_hex": spec["beacon"].hex(),
        "seed_hex": d.hex(),
        "opening_waypoint_hex": opening.hex(),
        "waypoint_equals_seed": 1 if opening == d else 0,
        "terrain_sha256": hashlib.sha256(terrain).hexdigest(),
        "stages": stages,
        "note": "NL4 terrain and the §11.1 wrapping-add fold. Stage 0 opens at the seed. Each later stage opens at the previous waypoint.",
    }


WRAP16K_SHA = "7bb281f1f5d982f4778c4e866276e235a80f457f775b1aac8df73294eceac5e3"
PROD_TERRAIN_SHA = "994f2d55e48396d12f8f3fbf23102ea79ea87e5a7f18089abc16c0411bb32d3b"
PROD_OFFSET = 213205398
# Round-2 XOR-accumulator pins. A helper that still XORs fails these.
XOR_LEG0 = "c8658b2b05a93205"
XOR_CP0 = "e8bfeed49fb20d41090e9efada63e0ca170606957e4e112719387ac60aef12d1"
OLD_ADD_LEG0 = "112a630861105796"
OLD_ADD_CP0 = "4f4e190bf1b0d16a8eb9c71c6c9f1b1dcb1537cb48fbbb33e0bd5edc178b7955"


def changed_words(fn, P, Q, nslots, words):
    n = 0
    for i in range(nslots):
        for j in range(words):
            if fn(P, i, j) != fn(Q, i, j):
                n += 1
    return n


def mtp_of(times):
    if any(t is None for t in times):
        return None
    ordered = sorted(times)
    return ordered[len(ordered) // 2]


def mtp_window(height, times):
    n = min(11, height + 1)
    if len(times) != n:
        raise SystemExit("mtp window width")
    raw = times[-1]
    return {
        "height": height,
        "n": n,
        "reads": list(range(height - n + 1, height + 1)),
        "times": times,
        "raw_ntime": raw,
        "mtp": mtp_of(times),
    }


def production_from_c():
    """Shell out to prod_v21.c. This generator does not walk a production leg in Python."""
    src = os.path.join(os.path.dirname(os.path.abspath(__file__)), "prod_v21.c")
    binary = "/tmp/prod_v21_helper"
    need = (not os.path.exists(binary)) or os.path.getmtime(binary) < os.path.getmtime(src)
    if need:
        subprocess.check_call(["gcc", "-O2", "-Wall", "-Wextra", "-o", binary, src, "-lcrypto"])
    text = subprocess.check_output([binary], text=True)
    fields = {}
    for line in text.splitlines():
        key, value = line.split(None, 1)
        fields[key] = value.strip()
    return fields


def build_prod(pk, beacon, d, P):
    slot = 16384
    terr_b = 256 * 1024 * 1024
    sample = 1024
    steps = 2_000_000
    got = production_from_c()
    if got["seed"] != d.hex():
        raise SystemExit("prod seed " + got["seed"])
    if got["wrap16k"] != WRAP16K_SHA:
        raise SystemExit("prod wrap " + got["wrap16k"])
    if got["terrain"] != PROD_TERRAIN_SHA:
        raise SystemExit("prod terrain " + got["terrain"])
    if got["leg0"] in (XOR_LEG0, OLD_ADD_LEG0) or got["cp0"] in (XOR_CP0, OLD_ADD_CP0):
        raise SystemExit("prod value is a withdrawn fold")
    if int(got["offset"]) != PROD_OFFSET:
        raise SystemExit("prod offset " + got["offset"])
    if P[0] != int.from_bytes(d[:8], "little"):
        raise SystemExit("prod lane0")
    return {
        "id": "prod-leg0",
        "class": "positive",
        "profile": "production",
        "public_key_hex": pk.hex(),
        "seed_beacon_hex": beacon.hex(),
        "slot_bytes": slot,
        "terrain_bytes": terr_b,
        "sample_bytes": sample,
        "steps": steps,
        "offset_modulus": terr_b - sample + 1,
        "terrain_sha256": got["terrain"],
        "leg0_u64_hex": got["leg0"],
        "sample_offset": int(got["offset"]),
        "checkpoint0_hex": got["cp0"],
        "note": "Stage 0 leg 0 opens at the seed. S_0 = u64_le(D[0:8]) = P[0]. The §11.1 accumulator is wrapping addition. The checkpoint is SHA-256(seed || seed || terrain[offset:offset+sample] || le64(leg)). Produced by spec/conformance/tools/prod_v21.c. This generator does not walk a production leg in Python.",
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-check", default="")
    ap.add_argument("--out", default="spec/conformance/v2.1")
    ap.add_argument("--skip-production", action="store_true")
    args = ap.parse_args()
    if args.self_check:
        self_check(args.self_check)

    pk = bytes([0x11]) * 32
    beacon = bytes([0x22]) * 32
    d, P, pk2 = assert_pins(pk, beacon)

    os.makedirs(args.out, exist_ok=True)

    seed = {
        "id": "seed",
        "class": "positive",
        "public_key_hex": pk.hex(),
        "seed_beacon_hex": beacon.hex(),
        "seed_preimage_hex": (pk + beacon).hex(),
        "seed_hex": d.hex(),
        "lanes_u64_hex": [u64hex(x) for x in P],
        "waypoint_equals_seed": 1,
        "note": "D = SHA-256(public_key || seed_beacon), no label. The seed equals stages[0].waypoint by design. Informative: leg 0 of stage 0 starts at S_0 = u64_le(D[0:8]) = P[0], and the first terrain-sample offset is P[0] mod the offset modulus. Those same 64 bits are lane 0, the first walk state, and the first sample offset.",
    }

    nl4 = {
        "id": "nl4-words",
        "class": "positive",
        "public_key_hex": pk.hex(),
        "seed_beacon_hex": beacon.hex(),
        "samples": [{"i": i, "j": j, "u64_hex": hx} for i, j, hx in NL4_SAMPLES],
        "slot0_words": 2048,
        "parents": False,
        "slot0_sha256": hashlib.sha256(base_slot(nl4_word, P, 2048)).hexdigest(),
        "note": "Base words only. Slot 0 has no parents. SHA-256 is over the 2048 little-endian words.",
    }

    t256 = fill_terrain(nl4_word, P, 64, 256)
    t1024 = fill_terrain(nl4_word, P, 64, 1024)
    t16k = fill_terrain(nl4_word, P, 16384, 65536)
    t16k_sha = hashlib.sha256(t16k).hexdigest()
    if t16k_sha != WRAP16K_SHA:
        raise SystemExit("16KiB wrap " + t16k_sha)
    terrain = {
        "id": "terrain-small",
        "class": "positive",
        "public_key_hex": pk.hex(),
        "seed_beacon_hex": beacon.hex(),
        "cases": [
            {"slot_bytes": 64, "terrain_bytes": 256, "sha256": hashlib.sha256(t256).hexdigest()},
            {"slot_bytes": 64, "terrain_bytes": 1024, "sha256": hashlib.sha256(t1024).hexdigest()},
            {
                "slot_bytes": 16384,
                "terrain_bytes": 65536,
                "sha256": t16k_sha,
                "note": "Four 16384-byte slots. Parent rotation (j + k) mod 64 wraps.",
            },
        ],
        "note": "Full NL4 fill, parents and slot_mix included. 1024 bytes is 16 slots of 64, so parents run. The 16384-byte case is four slots and exercises (j + k) mod 64.",
    }

    slot0 = base_slot(nl4_word, P, 2048)
    steps = []
    for s_name, S in (("zero", 0), ("lane0", P[0])):
        folded = fold_c1(slot0, S, mix64)
        dig = seal_digest(folded)
        steps.append({
            "S_u64_hex": u64hex(S),
            "S_folded_u64_hex": u64hex(folded),
            "digest_hex": dig.hex(),
            "S_prime_u64_hex": u64hex(int.from_bytes(dig[:8], "little")),
        })
    step = {
        "id": "step-c1",
        "class": "positive",
        "seal0_digest_hex": SEAL0,
        "seal0_s_prime_u64_hex": SEAL0_SPRIME,
        "public_key_hex": pk.hex(),
        "seed_beacon_hex": beacon.hex(),
        "slot_index": 0,
        "words": 2048,
        "parents": False,
        "steps": steps,
        "note": "One §11.1 step on the 2048-word NL4 base slot. seal(0) is the §7.4 check.",
    }

    # Beacon schedules for the conformance-small equivalents. Stage 0 opens at the seed.
    fix = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures")
    kat_spec = parse_vec(os.path.join(fix, "kat-small.vec"))
    t3_spec = parse_vec(os.path.join(fix, "test-3leg.vec"))
    # Positive equivalents use the fixture public key and seed beacon, NL4, and
    # a chain that opens at the seed. kat-small's fixture key is 0x7a.
    kat = hike_doc("kat-small", kat_spec, True)
    t3 = hike_doc("test-3leg", t3_spec, True)
    if kat["waypoint_equals_seed"] != 1 or t3["waypoint_equals_seed"] != 1:
        raise SystemExit("opening waypoint is not the seed")

    rate = {
        "id": "rate",
        "floor": {"p": 1, "q": 5, "mark": "placeholder, not ratified"},
        "comparison": "meets when hike_span > 0 and q * hike_legs >= p * hike_span; hike_span <= 0 means the rate is undefined and refuses",
        "cases": [
            {"name": "above", "class": "positive", "hike_legs": 2, "hike_span": 9, "meets": True},
            {"name": "below", "class": "negative", "must_reject": True, "hike_legs": 2, "hike_span": 11, "meets": False},
            {"name": "boundary-meet", "class": "positive", "hike_legs": 1, "hike_span": 5, "meets": True},
            {"name": "boundary-fail", "class": "negative", "must_reject": True, "hike_legs": 1, "hike_span": 6, "meets": False},
            {"name": "span-zero", "class": "negative", "must_reject": True, "hike_legs": 2, "hike_span": 0, "meets": False, "outcome": "refuse"},
            {"name": "span-negative", "class": "negative", "must_reject": True, "hike_legs": 2, "hike_span": -1, "meets": False, "outcome": "refuse"},
        ],
    }
    for c in rate["cases"]:
        if meets_floor(c["hike_legs"], c["hike_span"], 1, 5) != c["meets"]:
            raise SystemExit(f"rate case {c['name']}")

    def round_case(name, leg_total, draws, cls, must_reject=False, mismatch_on_round=None, byte_source_ends=False):
        got = phase_indices(leg_total, draws, stop_on_end=byte_source_ends)
        if byte_source_ends:
            published = got
            outcome = "not read"
            started = len(got)
        elif mismatch_on_round is not None:
            published = got[:mismatch_on_round]
            outcome = "refuse"
            started = mismatch_on_round
        elif leg_total < 6:
            published = []
            outcome = "refuse"
            started = 0
        else:
            published = got
            outcome = "recorded"
            started = 3
        case = {
            "name": name,
            "class": cls,
            "leg_total": leg_total,
            "outcome": outcome,
            "rounds_started": started,
            "rounds": published,
        }
        if must_reject:
            case["must_reject"] = True
        if leg_total < 6:
            case["draw"] = "not defined"
        else:
            case["draws_u64_hex"] = [u64hex(u) for u in draws]
        if mismatch_on_round is not None:
            case["mismatch_on_round"] = mismatch_on_round
        if byte_source_ends:
            case["byte_source"] = "exhausted"
        return case

    straight = list(range(18))
    with_dup = [0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]
    with_limit = [(1 << 64) - 1] + straight
    expect_straight = [list(range(i, i + 6)) for i in (0, 6, 12)]
    if phase_indices(32, straight) != expect_straight:
        raise SystemExit("rounds: 18 distinct")
    if phase_indices(32, with_dup)[0] != [0, 1, 2, 3, 4, 5]:
        raise SystemExit("rounds: duplicate in the round was kept")
    short = phase_indices(8, straight)
    if short[0] != [0, 1, 2, 3, 4, 5] or short[1] != [6, 7, 0, 1, 2, 3] or short[2] != [4, 5, 6, 7, 0, 1]:
        raise SystemExit("rounds: short hike %s" % short)
    flat = [q for rnd in short for q in rnd]
    if flat[:8] != list(range(8)):
        raise SystemExit("rounds: repeated before unused indices were exhausted")
    if phase_indices(6, with_limit)[0] != [0, 1, 2, 3, 4, 5]:
        raise SystemExit("rounds: limit skip")

    rounds = {
        "id": "rounds",
        "thorough_rounds": 3,
        "per_round": 6,
        "note": "The 18 indices are one draw without replacement, then split into 3 rounds of 6, when leg_total is at least 18. For 6 <= leg_total < 18 the indices stay distinct while an unused index remains, and a later draw may repeat only after those indices are exhausted. leg_total < 6 is draw not defined and refuses. Each round's 6 run in parallel. A mismatch does not start the next round. The current group of 6 is the indices appended since the last multiple of 6. The draw stops when 18 indices are in hand, or when the byte source is exhausted before 18. Exhaustion is not read. An unfinished group is not a round.",
        "cases": [
            round_case("below-six", 5, [], "negative", must_reject=True),
            round_case("eighteen-distinct", 32, straight, "positive"),
            round_case("discard-duplicate", 32, with_dup, "positive"),
            round_case("short-then-repeat", 8, straight, "positive"),
            round_case("skip-above-limit", 6, with_limit, "positive"),
            round_case("mismatch-stops", 32, straight, "negative", must_reject=True, mismatch_on_round=1),
            round_case("byte-source-ends", 8, list(range(8)), "negative", byte_source_ends=True),
        ],
    }

    anchors = {
        "id": "anchors",
        "minimum": 6,
        "cases": [
            {"name": "six", "class": "positive", "indices": [2, 4, 6, 8, 10, 12], "meets": True},
            {"name": "five", "class": "negative", "must_reject": True, "indices": [2, 4, 6, 8, 10], "meets": False},
        ],
        "note": "anchored_total >= 6 anywhere among W_1 .. W_16. No first-planted or last-planted rule.",
    }

    pin = fill_pin(pk + beacon)
    pin_t = fill_terrain(pin_word, pin, 64, 256)
    nl4_t = t256
    ref_w = ref_word(P, 0, 0)
    all_w = all4_word(P, 0, 0)
    bare = fold_c1(slot0, 0, bare64)
    good = fold_c1(slot0, 0, mix64)
    fat = fold_v2_0(slot0, 0)
    xor_acc = fold_xor_acc(slot0, 0, mix64)
    # At S = 0, (S + w) equals w, so the withdrawn per-word XOR matches §11.1.
    # The negative uses S = 1, where those inputs differ.
    xor_w_S = 1
    xor_w = fold_xor_w(slot0, xor_w_S, mix64)
    xor_w_good = fold_c1(slot0, xor_w_S, mix64)
    partial = fold_partial(slot0, 0, 1)
    f3_profile = (64, 1024)
    pin1 = fill_pin(pk + beacon)
    pin2 = fill_pin(pk2 + beacon)
    if pin1 != pin2:
        raise SystemExit("F3 fill pins differ; the negative would not show the old collision")
    fp1 = hashlib.sha256(fill_terrain(pin_word, pin1, *f3_profile)).hexdigest()
    fp2 = hashlib.sha256(fill_terrain(pin_word, pin2, *f3_profile)).hexdigest()
    n1 = hashlib.sha256(fill_terrain(nl4_word, P, *f3_profile)).hexdigest()
    n2 = hashlib.sha256(fill_terrain(nl4_word, lanes_of(seed_digest(pk2, beacon)), *f3_profile)).hexdigest()
    if fp1 != fp2 or n1 == n2:
        raise SystemExit("F3 negative did not separate fill_pin from NL4")

    flip = 0x8000000000000001
    Qflip = list(P)
    Qflip[2] ^= flip
    nslots_s, words_s = 256 // 64, 64 // 8
    total_w = nslots_s * words_s
    nl4_c = changed_words(nl4_word, P, Qflip, nslots_s, words_s)
    ref_c = changed_words(ref_word, P, Qflip, nslots_s, words_s)
    pair_c = changed_words(pair_word, P, Qflip, nslots_s, words_s)
    if (nl4_c, ref_c, pair_c) != (total_w, total_w // 4, total_w // 2):
        raise SystemExit("one-lane flip counts %s %s %s" % (nl4_c, ref_c, pair_c))
    base_nl4 = hashlib.sha256(t256).hexdigest()
    flip_nl4 = hashlib.sha256(fill_terrain(nl4_word, Qflip, 64, 256)).hexdigest()
    if flip_nl4 == base_nl4:
        raise SystemExit("one-lane flip terrain did not change")

    Qall = list(P)
    Qall[0] ^= MASK
    Qall[1] ^= MASK
    all_a = hashlib.sha256(fill_terrain(all4_word, P, 64, 256)).hexdigest()
    all_b = hashlib.sha256(fill_terrain(all4_word, Qall, 64, 256)).hexdigest()
    if all_a != all_b:
        raise SystemExit("ALL4 kernel terrains differ")
    nl4_all = hashlib.sha256(fill_terrain(nl4_word, Qall, 64, 256)).hexdigest()
    nl4_all_c = changed_words(nl4_word, P, Qall, nslots_s, words_s)
    if nl4_all == base_nl4 or nl4_all_c != total_w:
        raise SystemExit("ALL4 kernel NL4 did not change")

    sample_b = 32
    leg_u = P[0]
    good_cp = checkpoint(t256, sample_b, d, d, leg_u)
    offmod = len(t256) - sample_b + 1
    forged_off = int.from_bytes(d[:8], "little") % offmod
    if forged_off + 1 + sample_b > len(t256):
        raise SystemExit("forged window has no shifted byte")
    bad_window = t256[forged_off + 1 : forged_off + 1 + sample_b]
    bad_cp = hashlib.sha256(d + d + bad_window + leg_u.to_bytes(8, "little")).digest()
    if good_cp == bad_cp:
        raise SystemExit("forged window did not differ")
    partial2047 = fold_partial(slot0, 0, 2047)

    negative = {
        "id": "negatives",
        "class": "negative",
        "public_key_hex": pk.hex(),
        "seed_beacon_hex": beacon.hex(),
        "cases": [
            {
                "name": "fill_pin_terrain",
                "must_reject": True,
                "slot_bytes": 64,
                "terrain_bytes": 256,
                "forbidden_sha256": hashlib.sha256(pin_t).hexdigest(),
                "conforming_sha256": hashlib.sha256(nl4_t).hexdigest(),
            },
            {
                "name": "ref_word",
                "must_reject": True,
                "i": 0,
                "j": 0,
                "forbidden_u64_hex": u64hex(ref_w),
                "conforming_u64_hex": u64hex(nl4_word(P, 0, 0)),
            },
            {
                "name": "all4_word",
                "must_reject": True,
                "i": 0,
                "j": 0,
                "forbidden_u64_hex": u64hex(all_w),
                "conforming_u64_hex": u64hex(nl4_word(P, 0, 0)),
            },
            {
                "name": "mix64_without_mix_add",
                "must_reject": True,
                "words": 2048,
                "S_u64_hex": "0000000000000000",
                "forbidden_S_folded_u64_hex": u64hex(bare),
                "conforming_S_folded_u64_hex": u64hex(good),
            },
            {
                "name": "v2_0_fold",
                "must_reject": True,
                "words": 2048,
                "S_u64_hex": "0000000000000000",
                "forbidden_S_folded_u64_hex": u64hex(fat),
                "conforming_S_folded_u64_hex": u64hex(good),
            },
            {
                "name": "xor_accumulator_fold",
                "must_reject": True,
                "words": 2048,
                "S_u64_hex": "0000000000000000",
                "forbidden_S_folded_u64_hex": u64hex(xor_acc),
                "conforming_S_folded_u64_hex": u64hex(good),
                "note": "acc XOR= mix64(word XOR w XOR S) is not §11.1. The conforming accumulator is wrapping addition of mix64(word XOR ((S + w) mod 2^64)). This is not the v2.0 fold.",
            },
            {
                "name": "xor_w_fold",
                "must_reject": True,
                "words": 2048,
                "S_u64_hex": u64hex(xor_w_S),
                "forbidden_S_folded_u64_hex": u64hex(xor_w),
                "conforming_S_folded_u64_hex": u64hex(xor_w_good),
                "note": "acc += mix64(word XOR w XOR S) is the withdrawn fold. word XOR w does not depend on S, so that term can be stored in place of the terrain word. It is not §11.1. S is 1. At S = 0 the two per-word inputs match, because (S + w) equals w.",
            },
            {
                "name": "partial_slot_read",
                "must_reject": True,
                "words": 2048,
                "words_read": 1,
                "S_u64_hex": "0000000000000000",
                "forbidden_S_folded_u64_hex": u64hex(partial),
                "conforming_S_folded_u64_hex": u64hex(good),
            },
            {
                "name": "partial_slot_read_2047",
                "must_reject": True,
                "words": 2048,
                "words_read": 2047,
                "S_u64_hex": "0000000000000000",
                "forbidden_S_folded_u64_hex": u64hex(partial2047),
                "conforming_S_folded_u64_hex": u64hex(good),
            },
            {
                "name": "one_lane_flip",
                "must_reject": True,
                "lane": 2,
                "flip_u64_hex": u64hex(flip),
                "slot_bytes": 64,
                "terrain_bytes": 256,
                "words": total_w,
                "nl4_changed": nl4_c,
                "ref_changed": ref_c,
                "pair_changed": pair_c,
                "conforming_sha256": base_nl4,
                "forbidden_sha256": flip_nl4,
                "note": "Flip lane 2 by 0x8000000000000001. Counted on base words, before parents. NL4 changes every word. The one-lane word changes 25 percent. The first two NL4 steps change 50 percent. forbidden_sha256 is the SHA-256 of the NL4 terrain after that lane flip. conforming_sha256 is the SHA-256 of the unflipped NL4 terrain.",
            },
            {
                "name": "all4_kernel",
                "must_reject": True,
                "lanes": [0, 1],
                "xor_u64_hex": u64hex(MASK),
                "slot_bytes": 64,
                "terrain_bytes": 256,
                "words": total_w,
                "nl4_changed": nl4_all_c,
                "all4_sha256": all_a,
                "all4_sha256_flipped": all_b,
                "conforming_sha256": base_nl4,
                "forbidden_sha256": nl4_all,
                "note": "XOR lanes 0 and 1 with all-ones. The linear four-lane word is unchanged, because each lane is mixed once and all-ones twice cancels. NL4 changes every base word. forbidden_sha256 is the SHA-256 of the NL4 terrain after that lane change. conforming_sha256 is the SHA-256 of the unflipped NL4 terrain. all4_sha256 and all4_sha256_flipped are the linear four-lane terrains, and they match.",
            },
            {
                "name": "forged_window",
                "must_reject": True,
                "fake_leg_ignore": False,
                "reason": "A4.3 mismatch",
                "slot_bytes": 64,
                "terrain_bytes": 256,
                "sample_bytes": sample_b,
                "leg_u64_hex": u64hex(leg_u),
                "sample_offset": forged_off,
                "forbidden_window": "shifted-one",
                "conforming_hex": good_cp.hex(),
                "forbidden_hex": bad_cp.hex(),
                "note": "Limb 3 is not the verifier terrain window. This is an A4.3 mismatch. It does not add the key to the fake-leg ignore record.",
            },
            {
                "name": "f3_shared_terrain",
                "must_reject": True,
                "pk2_hex": pk2.hex(),
                "slot_bytes": f3_profile[0],
                "terrain_bytes": f3_profile[1],
                "forbidden_sha256_pk1": fp1,
                "forbidden_sha256_pk2": fp2,
                "conforming_sha256_pk1": n1,
                "conforming_sha256_pk2": n2,
                "note": "The withdrawn fill pin collides, so those two terrains match. NL4 terrains differ. A shared terrain for this pair is rejected.",
            },
        ],
    }
    for c in negative["cases"]:
        if "forbidden_sha256" in c and c["forbidden_sha256"] == c["conforming_sha256"]:
            raise SystemExit(c["name"] + " does not differ")
        if "forbidden_u64_hex" in c and c["forbidden_u64_hex"] == c["conforming_u64_hex"]:
            raise SystemExit(c["name"] + " word does not differ")
        if "forbidden_S_folded_u64_hex" in c and c["forbidden_S_folded_u64_hex"] == c["conforming_S_folded_u64_hex"]:
            raise SystemExit(c["name"] + " fold does not differ")
        if "forbidden_hex" in c and c["forbidden_hex"] == c["conforming_hex"]:
            raise SystemExit(c["name"] + " checkpoint does not differ")

    def span_case(name, first, last, cls, outcome, must_reject=False):
        case = {
            "name": name,
            "class": cls,
            "first": first,
            "last": last,
            "outcome": outcome,
        }
        if first["raw_ntime"] is None or last["raw_ntime"] is None:
            case["raw_span"] = "not read"
        else:
            case["raw_span"] = last["raw_ntime"] - first["raw_ntime"]
        if first["mtp"] is None or last["mtp"] is None:
            case["hike_span"] = "not read"
        else:
            case["hike_span"] = last["mtp"] - first["mtp"]
        if must_reject:
            case["must_reject"] = True
        return case

    replay_first = mtp_window(100, list(range(1100, 1110)) + [5000])
    replay_last = mtp_window(118, list(range(2100, 2110)) + [2200])
    zero_first = mtp_window(100, [1100] * 10 + [9000])
    zero_last = mtp_window(118, [1100] * 10 + [1000])
    short = mtp_window(2, [30, 10, 20])
    if short["mtp"] != 20:
        raise SystemExit("short-window mtp")
    if replay_last["mtp"] - replay_first["mtp"] != 1000:
        raise SystemExit("replay mtp span")
    if replay_last["raw_ntime"] - replay_first["raw_ntime"] != -2800:
        raise SystemExit("replay raw span")
    mtp = {
        "id": "mtp",
        "rule": "A4.10",
        "note": "hike_span is MTP(last beacon) minus MTP(first beacon). MTP is the BIP113 median: sort the nTime values of the n headers ending at that block, n = min(11, height + 1), and take index n // 2. The verifier reads those heights from the portable index. Bookend S still uses raw header.time. A raw-nTime span is not hike_span. This file is a numeric replay, not a live header corpus.",
        "cases": [
            span_case("replay-eleven", replay_first, replay_last, "positive", "read"),
            span_case("span-zero", zero_first, zero_last, "negative", "refuse", True),
            span_case("span-negative", replay_last, replay_first, "negative", "refuse", True),
            {
                "name": "short-window",
                "class": "positive",
                "first": short,
                "outcome": "read",
                "note": "Height 2 uses n = 3. Sorted index 1 is the median.",
            },
            span_case(
                "window-incomplete",
                mtp_window(100, [1100] * 10 + [None]),
                replay_last,
                "negative",
                "held",
            ),
        ],
    }

    kind = {
        "id": "kind",
        "class": "positive",
        "kind": "satoshi-hike-v2.1-object",
        "kind_bytes": 24,
        "earlier": ["satoshi-hike-v2.0-object", "satoshi-hike-v1.35-object"],
        "earlier_reason": "earlier_version",
        "note": "A v2.0 object is earlier_version. That refuse is not a fake leg. SDKs and the gate move together with this kind.",
    }
    if len(kind["kind"].encode()) != 24 or len(kind["earlier"][0].encode()) != 24:
        raise SystemExit("kind length")

    def deadline_outcome(last_h, tip, K):
        if tip is None:
            return "held", "not read"
        gap = tip - last_h
        if tip < last_h:
            return "held", gap
        if gap > K:
            return "refuse", gap
        return "not refused by this rule", gap

    deadline_cases = []
    for name, last_h, tip, cls in (
        ("gap-3", 100, 103, "positive"),
        ("gap-4", 100, 104, "negative"),
        ("tip-below", 100, 99, "negative"),
        ("tip-not-read", 100, None, "negative"),
    ):
        outcome, gap = deadline_outcome(last_h, tip, 3)
        case = {
            "name": name,
            "class": cls,
            "K": 3,
            "last_beacon_height": last_h,
            "tip_height": tip,
            "gap": gap,
            "outcome": outcome,
            "fake_leg_ignore": False,
        }
        if outcome == "refuse":
            case["must_reject"] = True
            case["reason"] = "submission_deadline"
        deadline_cases.append(case)
    if [c["outcome"] for c in deadline_cases] != ["not refused by this rule", "refuse", "held", "held"]:
        raise SystemExit("deadline outcomes")
    deadline = {
        "id": "deadline",
        "K": 3,
        "K_mark": "placeholder, not ratified",
        "reason": "submission_deadline",
        "note": "tip_height is the Gate index-url tip clock, read fresh at receipt, bypassing the local mirror. A failed or stale read is not read and the state is held. Several index sources use the highest tip among those read. The check is decided once per client submission, at the first successful tip read. Held retries and service-started re-verifies reuse it. A policy-version bump never revokes on it. A client resubmission is a new receipt. A lower tip after a reorg does not reopen a refuse. last_beacon_height is last_interrupt_height. tip_height minus last_beacon_height greater than K refuses. K is policy. A placeholder of 3 is placeholder, not ratified. A gap of 3 is not refused by this rule. A gap of 4 refuses as submission_deadline and is not a fake leg. When tip_height is below last_beacon_height the state is held.",
        "cases": deadline_cases,
    }

    constants = {
        "id": "constants",
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
        "KIND": "satoshi-hike-v2.1-object",
        "FILL": "dag2",
        "MIX_ADD": "0x9E3779B97F4A7C15",
        "MIX_MUL1": "0xBF58476D1CE4E5B9",
        "MIX_MUL2": "0x94D049BB133111EB",
        "WORD_XOR": "0xC0FFEE",
        "PARENT_MUL": "0xD1B54A32D192ED03",
        "SLOT_MUL": "0xA24BAED4963EE407",
    }
    files = {
        "constants.json": constants,
        "seed.json": seed,
        "nl4-words.json": nl4,
        "terrain-small.json": terrain,
        "step-c1.json": step,
        "kat-small.json": kat,
        "test-3leg.json": t3,
        "rate.json": rate,
        "rounds.json": rounds,
        "anchors.json": anchors,
        "negatives.json": negative,
        "mtp.json": mtp,
        "deadline.json": deadline,
        "kind.json": kind,
    }
    if not args.skip_production:
        files["prod-leg0.json"] = build_prod(pk, beacon, d, P)
    prod_line = ""
    if "prod-leg0.json" in files:
        prod_line = "\n`prod-leg0.json` is one production leg: slot 16384, terrain 256 MiB, 2,000,000 steps, sample 1024. Stage 0 opens at the seed.\n"
    readme = """# Conformance pack, Satoshi Hike v2.1

Generated by `spec/conformance/tools/gen_v21.py`. These files are the v2.1 target for the corrected seed, the NL4 word, the §11.1 wrapping-add fold, the small hikes, and the rate and anchor comparisons. They are not a full hike-object encoding. The pack under `../v2.0/` is historical. The withdrawn folds in `negatives.json` are the v2.0 fold, the XOR accumulator `xor_accumulator_fold`, and `xor_w_fold` (`mix64(word XOR w XOR S)` with a wrapping-add accumulator).

`legs_le_hex` in a hike file is the hexadecimal spelling of the little-endian wire bytes `le64(leg_result)`. It is not the `%016llx` spelling of the integer.

`rate.json` uses floor `1/5`. That value is placeholder, not ratified. The floor in the specification is TBD. `hike_span` of 0 or below means the rate is undefined and the outcome is `refuse`. `mtp.json` pins median-time-past for that span. Bookend `S` still uses raw `header.time`.

`rounds.json` pins the default thorough draw. When at least 18 legs exist, the 18 indices are one draw without replacement, split into 3 rounds of 6. For 6 <= leg_total < 18 the indices stay distinct while an unused index remains. `leg_total` < 6 is `not defined` and refuses. A mismatch does not start the next round. The current group of 6 is the indices appended since the last multiple of 6. When the byte source runs out before 18 indices, the outcome is `not read` and an unfinished group is not a round.

`one_lane_flip.forbidden_sha256` is the NL4 terrain after the named lane flip. `all4_kernel.forbidden_sha256` is the NL4 terrain after the named lanes change. In both, `conforming_sha256` is the unflipped NL4 terrain.

`kind.json` pins `satoshi-hike-v2.1-object`. The two earlier kinds refuse as `earlier_version`, and that refuse is not a fake leg.

`deadline.json` pins the submission deadline. `K = 3` is placeholder, not ratified. A gap of 3 is not refused by this rule. A gap of 4 refuses as `submission_deadline`. A tip below the last beacon, and a tip that was not read, are `held`. That refuse is not a fake leg.
""" + prod_line + """
A negative case publishes the withdrawn result and the conforming result. An implementation that emits the withdrawn result does not conform.

No git tag is cut, and no deployment cuts over, until every port passes this pack and the production-size vectors. The intended tag name is `spec-v2.1.0`. The maintainers cut that tag on the merge commit.
"""
    for name, obj in files.items():
        path = os.path.join(args.out, name)
        with open(path, "wb") as f:
            f.write(dump(obj))
    with open(os.path.join(args.out, "README.md"), "w") as f:
        f.write(readme)

    names = sorted(files)
    entries = []
    for name in names + ["README.md"]:
        blob = open(os.path.join(args.out, name), "rb").read()
        rel = f"spec/conformance/v2.1/{name}"
        entries.append((rel, hashlib.sha256(blob).digest()))
    h = hashlib.sha256()
    for rel, dig in sorted(entries):
        h.update(rel.encode() + b"\0" + dig)
    manifest = {
        "pack": "satoshi-hike-v2.1",
        "generator": "python3 spec/conformance/tools/gen_v21.py",
        "files": names + ["README.md"],
        "ids": [files[n]["id"] for n in names],
        "vector_sha256": h.hexdigest(),
        "vector_sha256_note": "SHA-256 over spec/conformance/v2.1 except manifest.json, each file as path, NUL, sha256(bytes), sorted by path. The sdk/SPEC_PIN hash covers the whole spec/conformance tree except tools/.",
        "floor_mark": "placeholder, not ratified",
    }
    with open(os.path.join(args.out, "manifest.json"), "wb") as f:
        f.write(dump(manifest))
    print(f"wrote {args.out} vector_sha256={manifest['vector_sha256']}")


if __name__ == "__main__":
    main()
