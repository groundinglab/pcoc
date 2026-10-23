"""SPEC section 13.1 Hashcash functions."""

from __future__ import annotations

import struct

from .core import sha256


def challenge(bari_origin: str | bytes, header_hash: bytes) -> bytes:
    origin = bari_origin.encode("ascii") if isinstance(bari_origin, str) else bari_origin
    if len(header_hash) != 32:
        raise ValueError("header_hash must be 32 bytes")
    return sha256(b"bari-hc-v2" + origin + header_hash)


def digest(challenge_bytes: bytes, waypoint: bytes, nonce: int) -> bytes:
    if len(challenge_bytes) != 32 or len(waypoint) != 32:
        raise ValueError("challenge and waypoint must be 32 bytes")
    if not 0 <= nonce < 1 << 64:
        raise ValueError("nonce must be a u64")
    return sha256(challenge_bytes + waypoint + struct.pack("<Q", nonce))


def leading_zero_bits(value: bytes) -> int:
    total = 0
    for byte in value:
        if byte == 0:
            total += 8
            continue
        return total + 8 - byte.bit_length()
    return total


def solve(challenge_bytes: bytes, waypoint: bytes, difficulty: int = 21) -> tuple[int, bytes]:
    if not 0 <= difficulty <= 256:
        raise ValueError("difficulty must be in [0, 256]")
    for nonce in range(1 << 64):
        value = digest(challenge_bytes, waypoint, nonce)
        if leading_zero_bits(value) >= difficulty:
            return nonce, value
    raise RuntimeError("u64 nonce space exhausted")
