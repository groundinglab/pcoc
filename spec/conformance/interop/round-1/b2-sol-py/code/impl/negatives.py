"""Generator-only permanent negatives from SPEC §16/§19.

This module is intentionally never imported by construction or verification.
"""
from __future__ import annotations
from .core import MASK, Profile, le64, mix64, sha256, u64_le

NAMES = ("slot_summary_fold","xor_accumulator_fold","xor_w_fold","s_plus_w_fold",
         "prefold_hoist","partial_slot_read","partial_slot_read_2047",
         "one_lane_flip","all4_kernel","f3_shared_terrain","cached_leg",
         "squeezed_seed_terrain")


def fold_variant(words, state, name):
    if name == "slot_summary_fold":
        x = state
        for j,w in enumerate(words): x ^= int(w) ^ j
        return x & MASK
    if name == "xor_accumulator_fold":
        x = 0
        for j,w in enumerate(words): x ^= mix64(int(w)^j^state)
        return x
    if name in ("xor_w_fold","prefold_hoist"):
        return sum(mix64(int(w)^j^state) for j,w in enumerate(words)) & MASK
    if name == "s_plus_w_fold":
        return sum(mix64(int(w)^((state+j)&MASK)) for j,w in enumerate(words)) & MASK
    if name == "partial_slot_read": words=words[:1]
    elif name == "partial_slot_read_2047": words=words[:2047]
    return sum(mix64(int(w)^state) for w in words) & MASK


def variant_leg(terrain, previous, profile: Profile, name):
    state=u64_le(previous)
    wps=profile.words_per_slot
    for _ in range(profile.steps_per_leg):
        words=terrain[(state%profile.n_slots)*wps:((state%profile.n_slots)+1)*wps]
        acc=fold_variant(words,state,name)
        state=u64_le(sha256(b"pcoc-walk-v1"+le64(acc)))
    return state
