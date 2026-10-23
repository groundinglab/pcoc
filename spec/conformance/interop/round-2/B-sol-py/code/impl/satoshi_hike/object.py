"""Sections 7–8 object construction and deterministic encoding."""

from __future__ import annotations

import struct
from collections.abc import Sequence
from typing import Any

from . import cbor
from .core import Terrain, checkpoint, leg, seed, waypoint
from .profiles import Profile

KIND = "v2.2-pcoc-object"
FILL = "dag2"
STAGES = 18


def produce_offline(
    public_key: bytes,
    seed_beacon: bytes,
    interrupts: Sequence[bytes],
    seed_height: int,
    interrupt_heights: Sequence[int],
    terrain: Terrain,
    legs_per_stage: Sequence[int],
    *,
    optimized: bool = True,
) -> dict[str, Any]:
    """Produce a deterministic offline test-plane object (sections 10–13)."""
    if len(public_key) != 32 or len(seed_beacon) != 32:
        raise ValueError("public_key and seed_beacon must be 32 bytes")
    if len(interrupts) != STAGES or any(len(item) != 32 for item in interrupts):
        raise ValueError("exactly 18 32-byte interrupts are required")
    if len(interrupt_heights) != STAGES or len(legs_per_stage) != STAGES:
        raise ValueError("exactly 18 heights and leg counts are required")
    if any(count < 1 for count in legs_per_stage):
        raise ValueError("each stage requires at least one leg")

    current_waypoint = seed(public_key, seed_beacon)
    beacon = seed_beacon
    beacon_height = seed_height
    stages: list[dict[str, Any]] = []

    for g in range(STAGES):
        stage_waypoint = current_waypoint
        previous = stage_waypoint
        checkpoints: list[bytes] = []
        leg_results: list[bytes] = []
        for _ in range(legs_per_stage[g]):
            result = leg(terrain, previous, optimized=optimized).result
            cp = checkpoint(terrain, previous, stage_waypoint, result)
            checkpoints.append(cp)
            leg_results.append(struct.pack("<Q", result))
            previous = cp
        stage: dict[str, Any] = {
            "beacon": beacon,
            "waypoint": stage_waypoint,
            "leg_count": len(checkpoints),
            "checkpoints": checkpoints,
            "leg_results": leg_results,
            "beacon_height": beacon_height,
        }
        if g:
            stage["path"] = b""
        stages.append(stage)
        if g < STAGES - 1:
            current_waypoint = waypoint(
                terrain, interrupts[g], stage_waypoint, checkpoints
            )
            beacon = interrupts[g]
            beacon_height = interrupt_heights[g]

    return {
        "kind": KIND,
        "fill": FILL,
        "slot_bytes": terrain.profile.slot_bytes,
        "n_slots": terrain.profile.n_slots,
        "sample_bytes": terrain.profile.sample_bytes,
        "steps_per_leg": terrain.profile.steps_per_leg,
        "public_key": public_key,
        "seed_beacon": seed_beacon,
        "stages": stages,
        "last_interrupt": interrupts[-1],
        "last_interrupt_height": interrupt_heights[-1],
    }


def encode_object(obj: dict[str, Any]) -> bytes:
    return cbor.encode(obj)


def decode_object(data: bytes) -> dict[str, Any]:
    value = cbor.decode(data)
    if not isinstance(value, dict) or not isinstance(value.get("kind"), str):
        raise cbor.CBORError("decode failure: map with kind text required")
    return value


def profile_from_object(obj: dict[str, Any], name: str) -> Profile:
    terrain_bytes = int(obj["slot_bytes"]) * int(obj["n_slots"])
    return Profile(
        name,
        int(obj["slot_bytes"]),
        int(obj["sample_bytes"]),
        terrain_bytes,
        int(obj["steps_per_leg"]),
    )
