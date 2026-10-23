"""Satoshi Hike 2.2 construction (SPEC §§6–12)."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import struct
from typing import Iterable

import numpy as np

MASK = (1 << 64) - 1
MIX_ADD = 0x9E3779B97F4A7C15
MIX_MUL1 = 0xBF58476D1CE4E5B9
MIX_MUL2 = 0x94D049BB133111EB
WORD_XOR = 0xC0FFEE
PARENT_MUL = 0xD1B54A32D192ED03
SLOT_MUL = 0xA24BAED4963EE407
SEAL_DOMAIN = b"pcoc-walk-v1"
KIND = "v2.2-pcoc-object"
FILL = "dag2"
STAGES = 18


@dataclass(frozen=True)
class Profile:
    slot_bytes: int = 16384
    sample_bytes: int = 1024
    terrain_bytes: int = 268435456
    steps_per_leg: int = 2_000_000

    def __post_init__(self) -> None:
        if not (8 <= self.slot_bytes <= 16384 and self.slot_bytes % 8 == 0):
            raise ValueError("SLOT_BYTES")
        if not (1 <= self.sample_bytes <= 1024):
            raise ValueError("SAMPLE_BYTES")
        if not (self.sample_bytes <= self.terrain_bytes <= 268435456):
            raise ValueError("TERRAIN_BYTES")
        if self.terrain_bytes % self.slot_bytes:
            raise ValueError("TERRAIN_BYTES multiple")
        if not (1 <= self.steps_per_leg <= 2_000_000):
            raise ValueError("STEPS_PER_LEG")

    @property
    def words_per_slot(self) -> int:
        return self.slot_bytes // 8

    @property
    def n_slots(self) -> int:
        return self.terrain_bytes // self.slot_bytes

    @property
    def offset_modulus(self) -> int:
        return self.terrain_bytes - self.sample_bytes + 1


PRODUCTION = Profile()
S1 = Profile(64, 32, 65536, 100)
S2 = Profile(256, 64, 1048576, 1000)


def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def le64(x: int) -> bytes:
    return struct.pack("<Q", x)


def u64_le(b: bytes) -> int:
    return struct.unpack("<Q", b[:8])[0]


def mix64(x: int) -> int:
    y = (x + MIX_ADD) & MASK
    y = ((y ^ (y >> 30)) * MIX_MUL1) & MASK
    y = ((y ^ (y >> 27)) * MIX_MUL2) & MASK
    return (y ^ (y >> 31)) & MASK


def _mix64v(x: np.ndarray) -> np.ndarray:
    with np.errstate(over="ignore"):
        y = x + np.uint64(MIX_ADD)
        y = (y ^ (y >> np.uint64(30))) * np.uint64(MIX_MUL1)
        y = (y ^ (y >> np.uint64(27))) * np.uint64(MIX_MUL2)
        return y ^ (y >> np.uint64(31))


def rotl(x: int, r: int) -> int:
    return x if r == 0 else ((x << r) | (x >> (64 - r))) & MASK


def reverse32(v: int) -> int:
    return int(f"{v:032b}"[::-1], 2)


def seed(public_key: bytes, seed_beacon: bytes) -> tuple[bytes, tuple[int, ...]]:
    if len(public_key) != 32 or len(seed_beacon) != 32:
        raise ValueError("seed inputs must be 32 bytes")
    d = sha256(public_key + seed_beacon)
    return d, tuple(u64_le(d[8*k:8*k+8]) for k in range(4))


def nl4_word(i: int, j: int, lanes: tuple[int, ...]) -> int:
    c = (i << 32) ^ j ^ WORD_XOR
    t = mix64(lanes[j % 4] ^ c)
    t = mix64(t ^ rotl(lanes[(j + 1) % 4], 17))
    t = mix64(t ^ rotl(lanes[(j + 2) % 4], 31))
    return mix64(t ^ rotl(lanes[(j + 3) % 4], 47))


def terrain(public_key: bytes, seed_beacon: bytes, profile: Profile) -> np.ndarray:
    """Fill terrain once (§10.4); uint64 words are native little-endian."""
    _, lanes = seed(public_key, seed_beacon)
    out = np.empty(profile.terrain_bytes // 8, dtype="<u8")
    wps = profile.words_per_slot
    for i in range(profile.n_slots):
        words = np.fromiter((nl4_word(i, j, lanes) for j in range(wps)),
                            dtype=np.uint64, count=wps)
        if i:
            bits = max(i.bit_length() - 1, 1)
            parents = (i - 1, (reverse32(i) >> (32 - bits)) % i,
                       (((i * MIX_ADD) & MASK) >> 32) % i)
            js = np.arange(wps, dtype=np.uint64)
            for k, p in enumerate(parents, 1):
                pw = out[p*wps:(p+1)*wps]
                r = (js + k) & 63
                rotated = (pw << r) | (pw >> ((64 - r) & 63))
                words = _mix64v(words ^ rotated ^ np.uint64((k * PARENT_MUL) & MASK))
            words = _mix64v(words ^ np.uint64((i * SLOT_MUL) & MASK))
        out[i*wps:(i+1)*wps] = words
    return out


def terrain_bytes(t: np.ndarray) -> bytes:
    return t.astype("<u8", copy=False).tobytes()


def step(t: np.ndarray, state: int, profile: Profile, trace: bool = False):
    """Fold every word of the full selected slot, then seal (§11.1)."""
    i = state % profile.n_slots
    wps = profile.words_per_slot
    slot = t[i*wps:(i+1)*wps]  # the complete slot, every invocation
    mixed = _mix64v(slot ^ np.uint64(state))
    acc = int(np.add.reduce(mixed, dtype=np.uint64))
    nxt = u64_le(sha256(SEAL_DOMAIN + le64(acc))[:8])
    return (nxt, i, acc) if trace else nxt


def leg(t: np.ndarray, previous: bytes, profile: Profile,
        trace_steps: int = 0) -> tuple[int, list[tuple[int, int, int, int]]]:
    state = u64_le(previous[:8])
    trace = []
    for n in range(profile.steps_per_leg):
        before = state
        state, slot, acc = step(t, state, profile, True)
        if n < trace_steps:
            trace.append((before, slot, acc, state))
    return state, trace


def terrain_sample(t_bytes: bytes, previous: bytes, profile: Profile) -> bytes:
    off = u64_le(previous[:8]) % profile.offset_modulus
    return t_bytes[off:off + profile.sample_bytes]


def checkpoint(previous: bytes, limb: bytes, sample: bytes, result: int) -> bytes:
    return sha256(previous + limb + sample + le64(result))


def checkpoint_fold(checkpoints: Iterable[bytes]) -> bytes:
    f = bytes(32)
    for c in checkpoints:
        f = sha256(f + c)
    return f


def waypoint(interrupt: bytes, t_bytes: bytes, prior_waypoint: bytes,
             checkpoints: Iterable[bytes], profile: Profile) -> bytes:
    idx = u64_le(interrupt[:8]) % profile.n_slots
    slot = t_bytes[idx*profile.slot_bytes:(idx+1)*profile.slot_bytes]
    return sha256(interrupt + slot + prior_waypoint + checkpoint_fold(checkpoints))


def build_hike(public_key: bytes, seed_beacon: bytes, interrupts: list[bytes],
               heights: list[int], profile: Profile, legs_per_stage: list[int]):
    if len(interrupts) != 18 or len(heights) != 19 or len(legs_per_stage) != 18:
        raise ValueError("18 stages and 19 heights required")
    d, _ = seed(public_key, seed_beacon)
    terr = terrain(public_key, seed_beacon, profile)
    tb = terrain_bytes(terr)
    stages, wp, beacon = [], d, seed_beacon
    for g in range(18):
        cps, results, previous = [], [], wp
        for _ in range(legs_per_stage[g]):
            result, _ = leg(terr, previous, profile)
            cp = checkpoint(previous, wp, terrain_sample(tb, previous, profile), result)
            results.append(le64(result)); cps.append(cp); previous = cp
        st = {"beacon": beacon, "beacon_height": heights[g], "leg_count": len(cps),
              "waypoint": wp, "checkpoints": cps, "leg_results": results}
        if g:
            st["path"] = b""
        stages.append(st)
        if g < 17:
            wp = waypoint(interrupts[g], tb, wp, cps, profile)
            beacon = interrupts[g]
    return {"kind": KIND, "fill": FILL, "slot_bytes": profile.slot_bytes,
            "n_slots": profile.n_slots, "sample_bytes": profile.sample_bytes,
            "steps_per_leg": profile.steps_per_leg, "public_key": public_key,
            "seed_beacon": seed_beacon, "stages": stages,
            "last_interrupt": interrupts[17], "last_interrupt_height": heights[18]}, terr
