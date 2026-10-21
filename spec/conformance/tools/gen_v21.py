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

MASK = (1 << 64) - 1
MIX_ADD = 0x9E3779B97F4A7C15
MIX_MUL1 = 0xBF58476D1CE4E5B9
MIX_MUL2 = 0x94D049BB133111EB
WORD_XOR = 0xC0FFEE
PARENT_MUL = 0xD1B54A32D192ED03
SLOT_MUL = 0xA24BAED4963EE407
SEAL = b"pcoc-walk-v1"
SEED_DOMAIN = b"PSEED64\x00"

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


def fill_pin(preimage):
    x = int.from_bytes(SEED_DOMAIN, "little")
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


def fold_c1(slot_bytes, S, mix):
    words = len(slot_bytes) // 8
    acc = 0
    for w in range(words):
        word = int.from_bytes(slot_bytes[8 * w : 8 * w + 8], "little")
        acc ^= mix((word ^ w ^ S) & MASK)
        acc &= MASK
    return acc


def fold_fatslot(slot_bytes, S):
    words = len(slot_bytes) // 8
    acc = 0
    for w in range(words):
        word = int.from_bytes(slot_bytes[8 * w : 8 * w + 8], "little")
        acc ^= (word ^ w) & MASK
    return (S ^ acc) & MASK


def fold_partial(slot_bytes, S, nread):
    acc = 0
    for w in range(nread):
        word = int.from_bytes(slot_bytes[8 * w : 8 * w + 8], "little")
        acc ^= mix64((word ^ w ^ S) & MASK)
        acc &= MASK
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


def phase_indices(leg_total, draws, nrounds=3, per=6):
    """§15.5 thorough draw. draws are the section 14.6 u64 values, in order."""
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
    x = int.from_bytes(SEED_DOMAIN, "little")
    for i in range(4):
        lane = int.from_bytes(pk[8 * i : 8 * i + 8], "little")
        x = mix64(x ^ lane ^ ((i * MIX_ADD) & MASK))
    return x


def f3_mate(pk1):
    pk2 = bytearray(pk1)
    pk2[0] ^= 0x5A
    pk2[9] ^= 0x3C
    x = int.from_bytes(SEED_DOMAIN, "little")
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


def walk(terrain, prof, opening, stages, chain):
    """chain True: stage 0 opens at `opening` and each later stage opens at the
    previous waypoint. chain False: each stage opens at its stored waypoint.
    """
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
            r = leg_result(terrain, slot_b, steps, S0, lambda sl, S: fold_c1(sl, S, mix64))
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
        got = walk(terrain, spec, spec["stages"][0]["stored_w"], spec["stages"], chain=False)
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
        "note": "NL4 terrain and the §11.1 fold. Stage 0 opens at the seed. Each later stage opens at the previous waypoint.",
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-check", default="")
    ap.add_argument("--out", default="spec/conformance/v2.1")
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
        "note": "D = SHA-256(public_key || seed_beacon), no label. The seed equals stages[0].waypoint by design.",
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
    terrain = {
        "id": "terrain-small",
        "class": "positive",
        "public_key_hex": pk.hex(),
        "seed_beacon_hex": beacon.hex(),
        "cases": [
            {"slot_bytes": 64, "terrain_bytes": 256, "sha256": hashlib.sha256(t256).hexdigest()},
            {"slot_bytes": 64, "terrain_bytes": 1024, "sha256": hashlib.sha256(t1024).hexdigest()},
        ],
        "note": "Full NL4 fill, parents and slot_mix included. 1024 bytes is 16 slots, so parents run.",
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
        "comparison": "meets when hike_span > 0 and q * hike_legs >= p * hike_span; hike_span <= 0 refuses",
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

    def round_case(name, leg_total, draws, cls, must_reject=False, mismatch_on_round=None):
        got = phase_indices(leg_total, draws)
        if mismatch_on_round is not None:
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
        "note": "leg_total < 6 is draw not defined and refuses. Otherwise 3 rounds of 6 distinct indices. An index is not repeated while an unused index remains. A mismatch does not start the next round.",
        "cases": [
            round_case("below-six", 5, [], "negative", must_reject=True),
            round_case("eighteen-distinct", 32, straight, "positive"),
            round_case("discard-duplicate", 32, with_dup, "positive"),
            round_case("short-then-repeat", 8, straight, "positive"),
            round_case("skip-above-limit", 6, with_limit, "positive"),
            round_case("mismatch-stops", 32, straight, "negative", must_reject=True, mismatch_on_round=1),
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
    fat = fold_fatslot(slot0, 0)
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
                "name": "fatslot_fold",
                "must_reject": True,
                "words": 2048,
                "S_u64_hex": "0000000000000000",
                "forbidden_S_folded_u64_hex": u64hex(fat),
                "conforming_S_folded_u64_hex": u64hex(good),
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

    files = {
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
    }
    readme = """# Conformance pack, Satoshi Hike v2.1

Generated by `spec/conformance/tools/gen_v21.py`. These files are the v2.1 target for the corrected seed, the NL4 word, the §11.1 fold, the small hikes, and the rate and anchor comparisons. They are not a full hike-object encoding. The pack under `../v2.0/` is historical.

`rate.json` uses floor `1/5`. That value is placeholder, not ratified. The floor in the specification is TBD. `hike_span` of 0 or below does not meet the floor and the outcome is `refuse`.

`rounds.json` pins the default thorough draw. `leg_total` < 6 is `not defined` and refuses. Otherwise the phase draws 3 rounds of 6 distinct indices and does not repeat an index while an unused index remains. A mismatch does not start the next round.

A negative case publishes the withdrawn result and the conforming result. An implementation that emits the withdrawn result does not conform.
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
