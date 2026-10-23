"""Rule B2 and section 16.1 integer comparisons."""

from __future__ import annotations


def leg_rate(legs: int, span: int, p: int, q: int) -> str:
    """Rule B2 exact integer comparison."""
    if min(legs, p, q) < 0 or p < 1 or q < 1:
        raise ValueError("invalid rate input")
    return "meets" if span > 0 and q * legs >= p * span else "refuse"


def anchor(stage_height: int, inclusion_height: int) -> str:
    """Section 16.1 anchor integer comparison."""
    return (
        "anchored"
        if inclusion_height in {stage_height + 2, stage_height + 3, stage_height + 4}
        else "not anchored"
    )
