"""Section 6 and section 16.1 profiles."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class Profile:
    name: str
    slot_bytes: int
    sample_bytes: int
    terrain_bytes: int
    steps_per_leg: int

    def __post_init__(self) -> None:
        if self.name not in {"production", "conformance-small"}:
            raise ValueError("profile name must be production or conformance-small")
        if not 8 <= self.slot_bytes <= 16_384 or self.slot_bytes % 8:
            raise ValueError("SLOT_BYTES must be a multiple of 8 in [8, 16384]")
        if not 1 <= self.sample_bytes <= 1_024:
            raise ValueError("SAMPLE_BYTES must be in [1, 1024]")
        if (
            self.terrain_bytes < self.sample_bytes
            or self.terrain_bytes > 268_435_456
            or self.terrain_bytes % self.slot_bytes
        ):
            raise ValueError("invalid TERRAIN_BYTES")
        if not 1 <= self.steps_per_leg <= 2_000_000:
            raise ValueError("invalid STEPS_PER_LEG")

    @property
    def words_per_slot(self) -> int:
        return self.slot_bytes // 8

    @property
    def n_slots(self) -> int:
        return self.terrain_bytes // self.slot_bytes

    @property
    def offset_modulus(self) -> int:
        return self.terrain_bytes - self.sample_bytes + 1


PRODUCTION = Profile("production", 16_384, 1_024, 268_435_456, 2_000_000)
S1 = Profile("conformance-small", 64, 32, 65_536, 100)
S2 = Profile("conformance-small", 256, 64, 1_048_576, 1_000)


def conformance_small(
    *, slot_bytes: int, sample_bytes: int, terrain_bytes: int, steps_per_leg: int
) -> Profile:
    return Profile(
        "conformance-small",
        slot_bytes,
        sample_bytes,
        terrain_bytes,
        steps_per_leg,
    )
