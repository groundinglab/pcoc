"""SPEC section 16 permanent negatives.

This module is generator-only. Nothing in the producer or verifier imports it.
"""

from __future__ import annotations

import struct

from ..core import Terrain, fill_terrain, leg
from ..profiles import S1

LEG_NEGATIVES = (
    "slot_summary_fold",
    "xor_accumulator_fold",
    "xor_w_fold",
    "s_plus_w_fold",
    "prefold_hoist",
    "final_stage_hoist",
    "partial_slot_read",
    "partial_slot_read_2047",
    "f3_shared_terrain",
    "cached_leg",
)
TERRAIN_NEGATIVES = ("one_lane_flip", "all4_kernel", "squeezed_seed_terrain")


def generate(
    terrain_a: Terrain,
    terrain_b: Terrain,
    seed_a: bytes,
) -> dict[str, str]:
    honest_leg = leg(terrain_a, seed_a).result
    honest_wire = struct.pack("<Q", honest_leg).hex()
    honest_terrain = terrain_a.digest().hex()
    values: dict[str, str] = {
        "negatives.honest.legs_le_hex": honest_wire,
        "negatives.honest.terrain_sha256": honest_terrain,
    }
    for name in LEG_NEGATIVES:
        source = terrain_b if name == "f3_shared_terrain" else terrain_a
        mode = "honest" if name == "f3_shared_terrain" else name
        result = leg(source, seed_a, mode=mode)
        if name == "cached_leg" and not result.revisited:
            raise RuntimeError("cached_leg fixture did not revisit a slot")
        wire = struct.pack("<Q", result.result).hex()
        matches = wire == honest_wire
        if matches:
            raise RuntimeError(f"negative {name} equals the honest leg")
        values[f"negatives.{name}.legs_le_hex"] = wire
        values[f"negatives.{name}.matches_honest"] = "true" if matches else "false"
    for name in TERRAIN_NEGATIVES:
        negative = fill_terrain(seed_a, S1, variant=name)
        digest = negative.digest().hex()
        matches = digest == honest_terrain
        if matches:
            raise RuntimeError(f"negative {name} equals the honest terrain")
        values[f"negatives.{name}.terrain_sha256"] = digest
        values[f"negatives.{name}.matches_honest"] = "true" if matches else "false"
    if len(values) != 28:
        raise AssertionError(f"expected 28 negative keys, got {len(values)}")
    return values
