"""Reproducible production construction benchmark."""

from __future__ import annotations

import json
import platform
import statistics
import time
from typing import Any

from .core import backend, fill_terrain, leg, seed
from .generate import PK_A, SEED_BEACON
from .profiles import PRODUCTION


def run() -> dict[str, Any]:
    digest = seed(PK_A, SEED_BEACON)
    result: dict[str, Any] = {
        "machine": platform.platform(),
        "python": platform.python_version(),
        "backend": backend(),
    }
    terrains = {}
    for label, optimized in (("scalar", False), ("optimized", True)):
        samples = []
        terrain = None
        for _ in range(3):
            started = time.perf_counter()
            terrain = fill_terrain(digest, PRODUCTION, optimized=optimized)
            samples.append(time.perf_counter() - started)
        assert terrain is not None
        terrains[label] = terrain
        result[f"terrain_{label}_seconds"] = statistics.median(samples)
        result[f"terrain_{label}_samples"] = samples
        result[f"terrain_{label}_sha256"] = terrain.digest().hex()
    legs = {}
    for label, optimized in (("scalar", False), ("optimized", True)):
        samples = []
        value = None
        for _ in range(3):
            started = time.perf_counter()
            value = leg(terrains[label], digest, optimized=optimized).result
            samples.append(time.perf_counter() - started)
        assert value is not None
        legs[label] = value
        result[f"leg_{label}_seconds"] = statistics.median(samples)
        result[f"leg_{label}_samples"] = samples
        result[f"leg_{label}_u64"] = f"{value:016x}"
    result["equivalent"] = (
        result["terrain_scalar_sha256"] == result["terrain_optimized_sha256"]
        and legs["scalar"] == legs["optimized"]
    )
    return result


def print_run() -> None:
    print(json.dumps(run(), separators=(",", ":")))
