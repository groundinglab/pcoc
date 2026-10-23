"""Sections 10–12 construction, backed by the in-repo native library."""

from __future__ import annotations

import ctypes
import hashlib
import struct
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from .profiles import Profile

ROOT: Final = Path(__file__).resolve().parents[2]
LIB_PATH: Final = ROOT / "impl" / "native" / "libhike.so"
MASK64: Final = (1 << 64) - 1
SEAL_DOMAIN: Final = b"pcoc-walk-v1"


def sha256(data: bytes | bytearray | memoryview) -> bytes:
    return hashlib.sha256(data).digest()


def seed(public_key: bytes, seed_beacon: bytes) -> bytes:
    if len(public_key) != 32 or len(seed_beacon) != 32:
        raise ValueError("public_key and seed_beacon must each be 32 bytes")
    return sha256(public_key + seed_beacon)


def lanes(digest: bytes) -> tuple[int, int, int, int]:
    if len(digest) != 32:
        raise ValueError("seed must be 32 bytes")
    return struct.unpack("<4Q", digest)


def _load_library() -> ctypes.CDLL:
    if not LIB_PATH.exists():
        subprocess.run(["make", "native"], cwd=ROOT, check=True)
    lib = ctypes.CDLL(str(LIB_PATH))
    u8p = ctypes.POINTER(ctypes.c_uint8)
    u64p = ctypes.POINTER(ctypes.c_uint64)
    lib.sh_fill.argtypes = [u8p, ctypes.c_uint64, ctypes.c_uint64, u8p, ctypes.c_int, ctypes.c_int]
    lib.sh_fill.restype = ctypes.c_int
    lib.sh_leg.argtypes = [
        u8p,
        ctypes.c_uint64,
        ctypes.c_uint64,
        ctypes.c_uint64,
        u8p,
        ctypes.c_int,
        ctypes.c_int,
        u64p,
        u64p,
        u64p,
        u64p,
        u64p,
        ctypes.POINTER(ctypes.c_int),
    ]
    lib.sh_leg.restype = ctypes.c_int
    lib.sh_mix64.argtypes = [ctypes.c_uint64]
    lib.sh_mix64.restype = ctypes.c_uint64
    lib.sh_backend.argtypes = []
    lib.sh_backend.restype = ctypes.c_char_p
    return lib


_LIB = _load_library()


def backend() -> str:
    return _LIB.sh_backend().decode("ascii")


def mix64(x: int) -> int:
    return int(_LIB.sh_mix64(x & MASK64))


def rotl64(x: int, r: int) -> int:
    r &= 63
    x &= MASK64
    return x if r == 0 else ((x << r) | (x >> (64 - r))) & MASK64


def reverse32(v: int) -> int:
    return int(f"{v:032b}"[::-1], 2)


def parent_indices(i: int) -> tuple[int, int, int]:
    if i < 1:
        raise ValueError("parents exist only for i > 0")
    bits = max(i.bit_length() - 1, 1)
    bitrev = (reverse32(i) >> (32 - bits)) % i
    mul = (((i * 0x9E3779B97F4A7C15) & MASK64) >> 32) % i
    return i - 1, bitrev, mul


def nl4_base_word(digest: bytes, i: int, j: int) -> int:
    p = lanes(digest)
    c = ((i << 32) ^ j ^ 0xC0FFEE) & MASK64
    t = mix64(p[j % 4] ^ c)
    t = mix64(t ^ rotl64(p[(j + 1) % 4], 17))
    t = mix64(t ^ rotl64(p[(j + 2) % 4], 31))
    return mix64(t ^ rotl64(p[(j + 3) % 4], 47))


@dataclass(slots=True)
class Terrain:
    profile: Profile
    data: bytearray
    optimized: bool = True

    def digest(self) -> bytes:
        return sha256(self.data)

    def sample(self, previous: bytes) -> memoryview:
        offset = int.from_bytes(previous[:8], "little") % self.profile.offset_modulus
        return memoryview(self.data)[offset : offset + self.profile.sample_bytes]

    def slot(self, index: int) -> memoryview:
        start = index * self.profile.slot_bytes
        return memoryview(self.data)[start : start + self.profile.slot_bytes]


_FILL_VARIANTS = {"honest": 0, "one_lane_flip": 1, "all4_kernel": 2, "squeezed_seed_terrain": 3}


def fill_terrain(
    digest: bytes,
    profile: Profile,
    *,
    optimized: bool = True,
    variant: str = "honest",
) -> Terrain:
    if len(digest) != 32:
        raise ValueError("seed must be 32 bytes")
    try:
        variant_id = _FILL_VARIANTS[variant]
    except KeyError as exc:
        raise ValueError(f"unknown terrain variant: {variant}") from exc
    data = bytearray(profile.terrain_bytes)
    out = (ctypes.c_uint8 * len(data)).from_buffer(data)
    seed_buf = (ctypes.c_uint8 * 32).from_buffer_copy(digest)
    rc = _LIB.sh_fill(
        out,
        profile.terrain_bytes,
        profile.slot_bytes,
        seed_buf,
        variant_id,
        int(optimized),
    )
    if rc:
        raise RuntimeError(f"native terrain fill failed ({rc})")
    return Terrain(profile, data, optimized)


@dataclass(frozen=True, slots=True)
class StepTrace:
    state: int
    slot_index: int
    accumulator: int
    next_state: int


@dataclass(frozen=True, slots=True)
class Leg:
    result: int
    trace: tuple[StepTrace, ...]
    revisited: bool = False


_LEG_MODES = {
    "honest": 0,
    "slot_summary_fold": 1,
    "xor_accumulator_fold": 2,
    "xor_w_fold": 3,
    "s_plus_w_fold": 4,
    "partial_slot_read": 5,
    "partial_slot_read_2047": 6,
    "prefold_hoist": 7,
    "final_stage_hoist": 8,
    "cached_leg": 9,
}


def leg(
    terrain: Terrain,
    previous_checkpoint: bytes,
    *,
    optimized: bool | None = None,
    mode: str = "honest",
    trace_steps: int = 0,
) -> Leg:
    if len(previous_checkpoint) != 32:
        raise ValueError("previous checkpoint must be 32 bytes")
    try:
        mode_id = _LEG_MODES[mode]
    except KeyError as exc:
        raise ValueError(f"unknown leg mode: {mode}") from exc
    use_optimized = terrain.optimized if optimized is None else optimized
    raw = (ctypes.c_uint8 * len(terrain.data)).from_buffer(terrain.data)
    previous = (ctypes.c_uint8 * 32).from_buffer_copy(previous_checkpoint)
    result = ctypes.c_uint64()
    arrays = [(ctypes.c_uint64 * 4)() for _ in range(4)]
    revisited = ctypes.c_int()
    rc = _LIB.sh_leg(
        raw,
        terrain.profile.terrain_bytes,
        terrain.profile.slot_bytes,
        terrain.profile.steps_per_leg,
        previous,
        mode_id,
        int(use_optimized),
        ctypes.byref(result),
        arrays[0],
        arrays[1],
        arrays[2],
        arrays[3],
        ctypes.byref(revisited),
    )
    if rc:
        raise RuntimeError(f"native leg failed ({rc})")
    count = min(max(trace_steps, 0), 4, terrain.profile.steps_per_leg)
    trace = tuple(
        StepTrace(arrays[0][i], arrays[1][i], arrays[2][i], arrays[3][i])
        for i in range(count)
    )
    return Leg(result.value, trace, bool(revisited.value))


def checkpoint(
    terrain: Terrain,
    previous: bytes,
    stage_limb: bytes,
    leg_result: int,
) -> bytes:
    return sha256(
        previous
        + stage_limb
        + terrain.sample(previous)
        + struct.pack("<Q", leg_result)
    )


def checkpoint_fold(checkpoints: list[bytes]) -> bytes:
    value = bytes(32)
    for item in checkpoints:
        value = sha256(value + item)
    return value


def waypoint(
    terrain: Terrain,
    interrupt: bytes,
    current_waypoint: bytes,
    checkpoints: list[bytes],
) -> bytes:
    slot_index = int.from_bytes(interrupt[:8], "little") % terrain.profile.n_slots
    return sha256(
        interrupt
        + terrain.slot(slot_index)
        + current_waypoint
        + checkpoint_fold(checkpoints)
    )
