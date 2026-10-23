"""Section 14 verifier and section 14.6 draw."""

from __future__ import annotations

import json
import os
from collections.abc import Callable, Iterator
from typing import Any

from . import cbor
from .core import (
    Terrain,
    checkpoint,
    fill_terrain,
    leg,
    seed,
    waypoint,
)
from .object import FILL, KIND, STAGES
from .profiles import Profile

MATCH = "match"
MISMATCH = "mismatch"
NOT_DEFINED = "not defined"
NOT_READ = "not read"
MAX_SAFE = (1 << 53) - 1
IndexReader = Callable[[int], dict[str, Any] | None]

SHAPE_ITEMS = (
    "malformed cbor",
    "field set",
    "stage-0 path key",
    "W_17 path",
    "constant",
    "stages length",
    "leg_count",
    "checkpoint width",
    "leg result width",
    "seed beacon",
    "last_interrupt",
    "header height order",
    "header hash differ",
    "height_range",
)

TOP_FIELDS = {
    "kind",
    "fill",
    "slot_bytes",
    "n_slots",
    "sample_bytes",
    "steps_per_leg",
    "public_key",
    "seed_beacon",
    "stages",
    "last_interrupt",
    "last_interrupt_height",
}
STAGE0_FIELDS = {
    "beacon",
    "beacon_height",
    "leg_count",
    "waypoint",
    "checkpoints",
    "leg_results",
}
STAGE_FIELDS = STAGE0_FIELDS | {"path"}


def compact_json(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def unknown_kind_record(kind_text: str) -> dict[str, str]:
    return {"kind": "unknown kind", "kind_text": kind_text}


def _is_uint(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _safe_height(value: Any) -> int | str:
    return value if _is_uint(value) and value <= MAX_SAFE else NOT_DEFINED


def _bytes(value: Any, length: int | None = None) -> bool:
    return isinstance(value, bytes) and (length is None or len(value) == length)


def _header_document(document: Any, requested: int) -> tuple[str, int | str, bytes | None]:
    if not isinstance(document, dict):
        return NOT_READ, NOT_READ, None
    header = document.get("header")
    if not isinstance(header, dict):
        return NOT_READ, NOT_READ, None
    text = header.get("hash")
    height = header.get("height")
    if (
        not isinstance(text, str)
        or len(text) != 64
        or not _is_uint(height)
        or height > MAX_SAFE
    ):
        return NOT_READ, NOT_READ, None
    try:
        display = bytes.fromhex(text)
    except ValueError:
        return NOT_READ, NOT_READ, None
    if len(display) != 32:
        return NOT_READ, NOT_READ, None
    time = header.get("time")
    timestamp: int | str = (
        time if _is_uint(time) and time <= 0xFFFFFFFF else NOT_READ
    )
    return ("usable" if height == requested else "usable"), timestamp, display[::-1]


def _header_measure(
    stored_hash: Any, stored_height: Any, index: IndexReader
) -> tuple[dict[str, Any], dict[str, Any] | None]:
    shown_hash = stored_hash.hex() if _bytes(stored_hash, 32) else NOT_DEFINED
    shown_height = _safe_height(stored_height)
    result = {
        "stored_hash": shown_hash,
        "stored_height": shown_height,
        "index_comparison": NOT_READ,
        "T": NOT_READ,
    }
    if shown_hash == NOT_DEFINED or shown_height == NOT_DEFINED:
        return result, None
    document = index(shown_height)
    status, timestamp, natural = _header_document(document, shown_height)
    if status == NOT_READ:
        return result, document
    header = document["header"]
    if natural != stored_hash or header["height"] != stored_height:
        result["index_comparison"] = MISMATCH
        return result, document
    result["index_comparison"] = MATCH
    result["T"] = timestamp
    return result, document


def _decode_path(raw: bytes) -> dict[str, Any] | None:
    if len(raw) < 146 or raw[0] != 1:
        return None
    sibling_count = raw[41]
    if sibling_count > 32 or len(raw) != 146 + 32 * sibling_count:
        return None
    pos = 42
    siblings = [raw[pos + 32 * i : pos + 32 * (i + 1)] for i in range(sibling_count)]
    pos += 32 * sibling_count
    body = {
        "leaf": raw[1:33],
        "leaf_index": int.from_bytes(raw[33:37], "big"),
        "tree_size": int.from_bytes(raw[37:41], "big"),
        "siblings": siblings,
        "root": raw[pos : pos + 32],
        "txid": raw[pos + 32 : pos + 64],
        "vout": int.from_bytes(raw[pos + 64 : pos + 68], "big"),
        "block_hash": raw[pos + 68 : pos + 100],
        "block_height": int.from_bytes(raw[pos + 100 : pos + 104], "big"),
    }
    if body["tree_size"] < 1 or body["leaf_index"] >= body["tree_size"]:
        return None
    width, levels = body["tree_size"], 0
    while width > 1:
        levels += 1
        width = width // 2 + width % 2
    if sibling_count != levels:
        return None
    return body


def _path_valid(body: dict[str, Any], waypoint_bytes: bytes) -> bool:
    if body["leaf"] != waypoint_bytes:
        return False
    import hashlib

    node = hashlib.sha256(b"\x1a" + body["leaf"]).digest()
    index, width = body["leaf_index"], body["tree_size"]
    for sibling in body["siblings"]:
        if width % 2 and index == width - 1:
            if sibling != node:
                return False
            left, right = node, node
        elif index % 2:
            left, right = sibling, node
        else:
            left, right = node, sibling
        node = hashlib.sha256(b"\x1b" + left + right).digest()
        index //= 2
        width = width // 2 + width % 2
    return node == body["root"]


def _block(body: dict[str, Any]) -> dict[str, Any]:
    return {
        "block_hash": body["block_hash"][::-1].hex(),
        "block_height": body["block_height"],
    }


def _plant_matches(document: dict[str, Any], body: dict[str, Any]) -> bool:
    if document.get("network") not in {"main", "synthetic"}:
        return False
    plants: list[Any] = []
    if isinstance(document.get("plants"), list):
        plants.extend(document["plants"])
    if isinstance(document.get("plant"), dict):
        plants.append(document["plant"])
    for plant in plants:
        if not isinstance(plant, dict):
            continue
        try:
            root = bytes.fromhex(plant["root"])
            txid = bytes.fromhex(plant["txid"])[::-1]
            block_hash = bytes.fromhex(plant["block_hash"])[::-1]
        except (KeyError, TypeError, ValueError):
            continue
        if (
            root == body["root"]
            and txid == body["txid"]
            and block_hash == body["block_hash"]
            and plant.get("vout") == body["vout"]
            and plant.get("block_height") == body["block_height"]
        ):
            return True
    return False


def draw_indices(total: int, count: int, source: Iterator[bytes]) -> list[int] | str:
    """Section 14.6 unbiased distinct draw."""
    if total < count:
        return NOT_DEFINED
    limit = (1 << 64) - ((1 << 64) % total)
    chosen: list[int] = []
    while len(chosen) < count:
        try:
            raw = next(source)
        except StopIteration:
            return NOT_READ
        if len(raw) != 8:
            return NOT_READ
        value = int.from_bytes(raw, "little")
        if value >= limit:
            continue
        candidate = value % total
        if candidate not in chosen:
            chosen.append(candidate)
    return chosen


def thorough_draw(
    total: int, rounds: int, source: Iterator[bytes]
) -> list[int] | str:
    """Section 15.5 draw, including total below D."""
    if total < 6:
        return NOT_DEFINED
    target = 6 * rounds
    limit = (1 << 64) - ((1 << 64) % total)
    phase: list[int] = []
    while len(phase) < target:
        try:
            raw = next(source)
        except StopIteration:
            return NOT_READ
        if len(raw) != 8:
            return NOT_READ
        value = int.from_bytes(raw, "little")
        if value >= limit:
            continue
        candidate = value % total
        group = phase[(len(phase) // 6) * 6 :]
        if candidate in group:
            continue
        if candidate in phase and len(set(phase)) < total:
            continue
        phase.append(candidate)
    return phase


def os_draw_source() -> Iterator[bytes]:
    while True:
        yield os.urandom(8)


def chain_draw_source(label: bytes) -> Iterator[bytes]:
    import hashlib

    value = hashlib.sha256(label).digest()
    pending = bytearray()
    while True:
        pending.extend(value)
        value = hashlib.sha256(value).digest()
        while len(pending) >= 8:
            yield bytes(pending[:8])
            del pending[:8]


def _shape(obj: dict[str, Any], profile: Profile) -> tuple[str, list[str]]:
    violated: set[str] = set()
    if set(obj) != TOP_FIELDS:
        violated.add("field set")
    if not (
        isinstance(obj.get("kind"), str)
        and isinstance(obj.get("fill"), str)
        and _is_uint(obj.get("slot_bytes"))
        and _is_uint(obj.get("n_slots"))
        and _is_uint(obj.get("sample_bytes"))
        and _is_uint(obj.get("steps_per_leg"))
        and _bytes(obj.get("public_key"), 32)
        and _bytes(obj.get("seed_beacon"), 32)
        and isinstance(obj.get("stages"), list)
        and isinstance(obj.get("last_interrupt"), bytes)
        and _is_uint(obj.get("last_interrupt_height"))
    ):
        violated.add("field set")
    stages = obj.get("stages")
    if not isinstance(stages, list) or len(stages) != STAGES:
        violated.add("stages length")
    usable_stages = stages if isinstance(stages, list) else []
    for g, stage in enumerate(usable_stages):
        if not isinstance(stage, dict):
            violated.update({"field set", "leg_count"})
            continue
        wanted = STAGE0_FIELDS if g == 0 else STAGE_FIELDS
        if set(stage) != wanted:
            violated.add("field set")
        if not (
            _bytes(stage.get("beacon"), 32)
            and _is_uint(stage.get("beacon_height"))
            and _is_uint(stage.get("leg_count"))
            and _bytes(stage.get("waypoint"), 32)
            and isinstance(stage.get("checkpoints"), list)
            and isinstance(stage.get("leg_results"), list)
            and (g == 0 or isinstance(stage.get("path"), bytes))
        ):
            violated.add("field set")
        if g == 0 and "path" in stage:
            violated.add("stage-0 path key")
        checkpoints = stage.get("checkpoints")
        results = stage.get("leg_results")
        count = stage.get("leg_count")
        if (
            not isinstance(checkpoints, list)
            or not isinstance(results, list)
            or not _is_uint(count)
            or count < 1
            or count != len(checkpoints)
            or count != len(results)
        ):
            violated.add("leg_count")
        if isinstance(checkpoints, list) and any(not _bytes(x, 32) for x in checkpoints):
            violated.add("checkpoint width")
        if isinstance(results, list) and any(not _bytes(x, 8) for x in results):
            violated.add("leg result width")
    if len(usable_stages) <= 17 or not isinstance(usable_stages[17], dict) or usable_stages[17].get("path") != b"":
        violated.add("W_17 path")
    constants = (
        obj.get("kind") == KIND
        and obj.get("fill") == FILL
        and obj.get("slot_bytes") == profile.slot_bytes
        and obj.get("n_slots") == profile.n_slots
        and obj.get("sample_bytes") == profile.sample_bytes
        and obj.get("steps_per_leg") == profile.steps_per_leg
    )
    if not constants:
        violated.add("constant")
    if (
        not usable_stages
        or not isinstance(usable_stages[0], dict)
        or usable_stages[0].get("beacon") != obj.get("seed_beacon")
    ):
        violated.add("seed beacon")

    hashes: list[Any] = [
        stage.get("beacon") if isinstance(stage, dict) else None
        for stage in usable_stages[:18]
    ]
    heights: list[Any] = [
        stage.get("beacon_height") if isinstance(stage, dict) else None
        for stage in usable_stages[:18]
    ]
    hashes += [obj.get("last_interrupt")]
    heights += [obj.get("last_interrupt_height")]
    if any(_is_uint(h) and h > MAX_SAFE for h in heights):
        violated.add("height_range")
    safe_heights = [h if _is_uint(h) and h <= MAX_SAFE else None for h in heights]
    for a, b in zip(safe_heights, safe_heights[1:]):
        if a is None or b is None or b <= a:
            violated.add("header height order")
    for a, b in zip(hashes, hashes[1:]):
        if not _bytes(a, 32) or not _bytes(b, 32) or a == b:
            violated.add("header hash differ")
    if (
        not _bytes(obj.get("last_interrupt"), 32)
        or len(hashes) < 19
        or not _bytes(hashes[-2], 32)
        or hashes[-1] == hashes[-2]
        or len(safe_heights) < 19
        or safe_heights[-1] is None
        or safe_heights[-2] is None
        or safe_heights[-1] <= safe_heights[-2]
    ):
        violated.add("last_interrupt")
    result = [name for name in SHAPE_ITEMS if name in violated]
    return (MATCH if not result else MISMATCH), result


def _empty_record(profile: Profile) -> dict[str, Any]:
    record: dict[str, Any] = {"profile": profile.name}
    if profile.name == "conformance-small":
        record.update(
            {
                "sample_bytes": profile.sample_bytes,
                "slot_bytes": profile.slot_bytes,
                "terrain_bytes": profile.terrain_bytes,
                "steps_per_leg": profile.steps_per_leg,
            }
        )
    record.update(
        {
            "shape": MISMATCH,
            "shape_item": ["malformed cbor"],
            "headers": [
                {
                    "stored_hash": NOT_DEFINED,
                    "stored_height": NOT_DEFINED,
                    "index_comparison": NOT_READ,
                    "T": NOT_READ,
                }
                for _ in range(19)
            ],
            "seed_value": MISMATCH,
            "checkpoints": [{"g": g, "legs": NOT_DEFINED} for g in range(18)],
            "waypoints": [{"g": g, "result": NOT_DEFINED} for g in range(1, 18)],
            "paths": [
                (
                    {
                        "g": g,
                        "path_state": "skip",
                        "plant": "n/a",
                        "P_g": NOT_DEFINED,
                        "height_P_g": NOT_DEFINED,
                        "d_g": NOT_DEFINED,
                        "claimed_block": NOT_DEFINED,
                    }
                    if g < 17
                    else {
                        "g": 17,
                        "w17_path": NOT_DEFINED,
                        "claimed_block": NOT_DEFINED,
                    }
                )
                for g in range(1, 18)
            ],
            "anchors": ["not anchored"] * 16,
            "anchored_total": 0,
            "skip_list": [],
        }
    )
    _set_bookends(record, NOT_DEFINED)
    record.update(
        {
            "six_leg": NOT_DEFINED,
            "six_leg_runs": [],
            "analysis": {
                "whole_hike_legs": NOT_DEFINED,
                "whole_hike_span": NOT_READ,
                "per_stage": [
                    {"g": g, "checkpoint_count": NOT_DEFINED, "span": NOT_READ}
                    for g in range(18)
                ],
                "legs_outside": NOT_DEFINED,
                "hike_legs": NOT_DEFINED,
                "hike_span": NOT_READ,
            },
        }
    )
    return record


def _set_bookends(record: dict[str, Any], token: str) -> None:
    record.update(
        {
            "a": token,
            "b": token,
            "N": token,
            "S": token,
            "delta_h": token,
            "fixed_start": {"N": token, "S": token, "delta_h": token},
            "P_b": token,
            "height_P_b": token,
            "index_comparison_P_b": token,
            "T_P_b": token,
            "height_I_a_minus_1": token,
            "T_I_a_minus_1": token,
        }
    )


def verify_bytes(
    data: bytes,
    profile: Profile,
    index: IndexReader | None = None,
    draw_source: Iterator[bytes] | None = None,
    *,
    optimized: bool = True,
) -> dict[str, Any]:
    """Produce the complete section 14 record."""
    if index is None:
        index = lambda _height: None
    try:
        decoded = cbor.decode(data)
    except cbor.CBORError:
        return _empty_record(profile)
    if not isinstance(decoded, dict) or not isinstance(decoded.get("kind"), str):
        raise cbor.CBORError("decode failure")
    if decoded["kind"] != KIND:
        return unknown_kind_record(decoded["kind"])
    obj = decoded

    record: dict[str, Any] = {"profile": profile.name}
    if profile.name == "conformance-small":
        record.update(
            {
                "sample_bytes": profile.sample_bytes,
                "slot_bytes": profile.slot_bytes,
                "terrain_bytes": profile.terrain_bytes,
                "steps_per_leg": profile.steps_per_leg,
            }
        )
    shape, shape_items = _shape(obj, profile)
    record["shape"] = shape
    record["shape_item"] = shape_items

    stages = obj.get("stages") if isinstance(obj.get("stages"), list) else []
    header_values: list[tuple[Any, Any]] = []
    for g in range(18):
        stage = stages[g] if g < len(stages) and isinstance(stages[g], dict) else {}
        header_values.append((stage.get("beacon"), stage.get("beacon_height")))
    header_values.append((obj.get("last_interrupt"), obj.get("last_interrupt_height")))
    headers: list[dict[str, Any]] = []
    header_documents: list[dict[str, Any] | None] = []
    for stored_hash, stored_height in header_values:
        measured, document = _header_measure(stored_hash, stored_height, index)
        headers.append(measured)
        header_documents.append(document)
    record["headers"] = headers

    public_key, seed_beacon = obj.get("public_key"), obj.get("seed_beacon")
    computed_seed = (
        seed(public_key, seed_beacon)
        if _bytes(public_key, 32) and _bytes(seed_beacon, 32)
        else None
    )
    stage0_waypoint = (
        stages[0].get("waypoint")
        if stages and isinstance(stages[0], dict)
        else None
    )
    record["seed_value"] = (
        MATCH
        if computed_seed is not None and stage0_waypoint == computed_seed
        else MISMATCH
    )
    terrain: Terrain | None = (
        fill_terrain(computed_seed, profile, optimized=optimized)
        if computed_seed is not None
        else None
    )

    checkpoint_records: list[dict[str, Any]] = []
    for g in range(18):
        if g >= len(stages) or not isinstance(stages[g], dict) or terrain is None:
            checkpoint_records.append({"g": g, "legs": NOT_DEFINED})
            continue
        stage = stages[g]
        cps = stage.get("checkpoints")
        results = stage.get("leg_results")
        if not isinstance(cps, list):
            checkpoint_records.append({"g": g, "legs": NOT_DEFINED})
            continue
        legs: list[dict[str, Any]] = []
        for j, cp in enumerate(cps):
            previous = computed_seed if g == 0 and j == 0 else (
                stage.get("waypoint") if j == 0 else cps[j - 1]
            )
            stage_limb = computed_seed if g == 0 else stage.get("waypoint")
            limb = results[j] if isinstance(results, list) and j < len(results) else None
            valid = all(
                (
                    _bytes(cp, 32),
                    _bytes(previous, 32),
                    _bytes(stage_limb, 32),
                    _bytes(limb, 8),
                )
            )
            if valid:
                expected = __import__("hashlib").sha256(
                    previous + stage_limb + terrain.sample(previous) + limb
                ).digest()
                outcome = MATCH if cp == expected else MISMATCH
            else:
                outcome = MISMATCH
            legs.append({"j": j, "result": outcome})
        checkpoint_records.append({"g": g, "legs": legs})
    record["checkpoints"] = checkpoint_records

    waypoint_records: list[dict[str, Any]] = []
    for g in range(17):
        outcome = NOT_DEFINED
        if (
            terrain is not None
            and g + 1 < len(stages)
            and isinstance(stages[g], dict)
            and isinstance(stages[g + 1], dict)
            and _bytes(stages[g + 1].get("beacon"), 32)
            and _bytes(stages[g].get("waypoint"), 32)
            and isinstance(stages[g].get("checkpoints"), list)
            and all(_bytes(x, 32) for x in stages[g]["checkpoints"])
        ):
            expected = waypoint(
                terrain,
                stages[g + 1]["beacon"],
                stages[g]["waypoint"],
                stages[g]["checkpoints"],
            )
            outcome = MATCH if expected == stages[g + 1].get("waypoint") else MISMATCH
        waypoint_records.append({"g": g + 1, "result": outcome})
    record["waypoints"] = waypoint_records

    paths: list[dict[str, Any]] = []
    anchors: list[str] = []
    skip_list: list[int] = []
    valid_paths = 0
    issued_not_read = False
    path_bodies: dict[int, dict[str, Any]] = {}
    interrupt_lookup = {
        (stage.get("beacon_height"), stage.get("beacon"))
        for stage in stages[1:18]
        if isinstance(stage, dict)
    }
    if _safe_height(obj.get("last_interrupt_height")) != NOT_DEFINED and _bytes(obj.get("last_interrupt"), 32):
        interrupt_lookup.add((obj["last_interrupt_height"], obj["last_interrupt"]))
    for g in range(1, 17):
        stage = stages[g] if g < len(stages) and isinstance(stages[g], dict) else {}
        raw = stage.get("path")
        if raw == b"" or not isinstance(raw, bytes):
            paths.append(
                {
                    "g": g,
                    "path_state": "skip",
                    "plant": "n/a",
                    "P_g": NOT_DEFINED,
                    "height_P_g": NOT_DEFINED,
                    "d_g": NOT_DEFINED,
                    "claimed_block": NOT_DEFINED,
                }
            )
            anchors.append("not anchored")
            if raw == b"":
                skip_list.append(g)
            continue
        body = _decode_path(raw)
        valid = body is not None and _bytes(stage.get("waypoint"), 32) and _path_valid(body, stage["waypoint"])
        plant, p_g, height_p, d_g, claimed = MISMATCH, NOT_DEFINED, NOT_DEFINED, NOT_DEFINED, NOT_DEFINED
        anchored = "not anchored"
        if valid and body is not None:
            valid_paths += 1
            path_bodies[g] = body
            claimed = _block(body)
            h = stage.get("beacon_height")
            in_window = _is_uint(h) and body["block_height"] in {h + 2, h + 3, h + 4}
            is_interrupt = (body["block_height"], body["block_hash"]) in interrupt_lookup
            if in_window and is_interrupt:
                document = index(body["block_height"])
                if document is None:
                    plant = NOT_READ
                    issued_not_read = True
                elif _plant_matches(document, body):
                    plant = MATCH
                    anchored = "anchored"
                    p_g = claimed
                    height_p = body["block_height"]
                    if headers[g]["index_comparison"] == MATCH:
                        d_g = height_p - headers[g]["stored_height"]
                else:
                    plant = MISMATCH
            else:
                plant = NOT_READ
        paths.append(
            {
                "g": g,
                "path_state": "present",
                "plant": plant,
                "P_g": p_g,
                "height_P_g": height_p,
                "d_g": d_g,
                "claimed_block": claimed,
            }
        )
        anchors.append(anchored)
    stage17 = stages[17] if len(stages) > 17 and isinstance(stages[17], dict) else {}
    raw17 = stage17.get("path")
    claimed17: Any = NOT_DEFINED
    if isinstance(raw17, bytes) and raw17:
        body17 = _decode_path(raw17)
        if body17 is not None:
            claimed17 = _block(body17)
    paths.append(
        {
            "g": 17,
            "w17_path": "not fetched by design" if raw17 == b"" else NOT_DEFINED,
            "claimed_block": claimed17,
        }
    )
    record["paths"] = paths
    record["anchors"] = anchors
    record["anchored_total"] = anchors.count("anchored")
    record["skip_list"] = skip_list

    counts = [
        len(stage.get("checkpoints", []))
        if isinstance(stage, dict) and isinstance(stage.get("checkpoints"), list)
        else None
        for stage in (stages + [None] * 18)[:18]
    ]
    anchored_indices = [g for g, token in enumerate(anchors, 1) if token == "anchored"]
    if valid_paths < 2:
        _set_bookends(record, NOT_DEFINED)
    elif issued_not_read:
        _set_bookends(record, NOT_READ)
    elif len(anchored_indices) < 2:
        _set_bookends(record, NOT_DEFINED)
    else:
        a, b = anchored_indices[0], anchored_indices[-1]
        body_b = path_bodies[b]
        if headers[a]["index_comparison"] == NOT_READ:
            _set_bookends(record, NOT_READ)
        else:
            p_doc = index(body_b["block_height"])
            status, t_pb, p_natural = _header_document(p_doc, body_b["block_height"])
            if status == NOT_READ:
                _set_bookends(record, NOT_READ)
            else:
                p_match = (
                    p_natural == body_b["block_hash"]
                    and p_doc["header"]["height"] == body_b["block_height"]
                )
                p_cmp = MATCH if p_match else MISMATCH
                height_pb = p_doc["header"]["height"] if p_match else NOT_DEFINED
                t_pb = t_pb if p_match else NOT_READ
                t_ia = headers[a]["T"]
                height_ia = (
                    headers[a]["stored_height"]
                    if headers[a]["index_comparison"] == MATCH
                    else NOT_DEFINED
                )
                n_value = sum(counts[a:b]) if all(x is not None for x in counts[a:b]) else NOT_DEFINED
                s_value = t_pb - t_ia if isinstance(t_pb, int) and isinstance(t_ia, int) else NOT_READ
                dh = height_pb - height_ia if isinstance(height_pb, int) and isinstance(height_ia, int) else NOT_DEFINED
                fixed_n = sum(counts[1:b]) if all(x is not None for x in counts[1:b]) else NOT_DEFINED
                fixed_t = headers[1]["T"]
                fixed_s = t_pb - fixed_t if isinstance(t_pb, int) and isinstance(fixed_t, int) else NOT_READ
                fixed_h = headers[1]["stored_height"] if headers[1]["index_comparison"] == MATCH else NOT_DEFINED
                fixed_dh = height_pb - fixed_h if isinstance(height_pb, int) and isinstance(fixed_h, int) else NOT_DEFINED
                record.update(
                    {
                        "a": a,
                        "b": b,
                        "N": n_value,
                        "S": s_value,
                        "delta_h": dh,
                        "fixed_start": {"N": fixed_n, "S": fixed_s, "delta_h": fixed_dh},
                        "P_b": _block(body_b),
                        "height_P_b": height_pb,
                        "index_comparison_P_b": p_cmp,
                        "T_P_b": t_pb,
                        "height_I_a_minus_1": height_ia,
                        "T_I_a_minus_1": t_ia,
                    }
                )

    total = sum(counts) if all(value is not None for value in counts) else 0
    source = draw_source if draw_source is not None else os_draw_source()
    indices = draw_indices(total, 6, source)
    if indices == NOT_DEFINED:
        record["six_leg"] = NOT_DEFINED
        record["six_leg_runs"] = []
    elif indices == NOT_READ:
        record["six_leg"] = NOT_READ
        record["six_leg_runs"] = NOT_READ
    else:
        outcomes: list[str] = []
        starts = [0]
        for count in counts:
            starts.append(starts[-1] + (count or 0))
        for q in indices:
            g = next(g for g in range(18) if starts[g] <= q < starts[g + 1])
            j = q - starts[g]
            stage = stages[g]
            previous = (
                computed_seed if g == 0 and j == 0 else
                stage["waypoint"] if j == 0 else stage["checkpoints"][j - 1]
            )
            expected = leg(terrain, previous, optimized=optimized).result if terrain is not None else -1
            actual = int.from_bytes(stage["leg_results"][j], "little") if _bytes(stage["leg_results"][j], 8) else -2
            outcomes.append(MATCH if expected == actual else MISMATCH)
        six = {"indices": indices, "results": outcomes}
        record["six_leg"] = six
        record["six_leg_runs"] = [six]

    def span(left: int, right: int) -> int | str:
        a_t, b_t = headers[left]["T"], headers[right]["T"]
        return b_t - a_t if isinstance(a_t, int) and isinstance(b_t, int) else NOT_READ

    whole_legs: int | str = sum(counts[1:]) if all(x is not None for x in counts[1:]) else NOT_DEFINED
    per_stage = [
        {
            "g": g,
            "checkpoint_count": counts[g] if counts[g] is not None else NOT_DEFINED,
            "span": span(g, g + 1),
        }
        for g in range(18)
    ]

    def mtp(height: Any, beacon_header: dict[str, Any]) -> int | str:
        if not _is_uint(height) or height > MAX_SAFE or beacon_header["index_comparison"] != MATCH:
            return NOT_READ
        n = min(11, height + 1)
        times: list[int] = []
        for h in range(height - n + 1, height + 1):
            if h == height:
                value = beacon_header["T"]
            else:
                document = index(h)
                status, value, _natural = _header_document(document, h)
                if status == NOT_READ or not isinstance(document, dict) or document.get("header", {}).get("height") != h:
                    return NOT_READ
            if not isinstance(value, int):
                return NOT_READ
            times.append(value)
        times.sort()
        return times[n // 2]

    first_mtp = mtp(header_values[0][1], headers[0])
    last_mtp = mtp(header_values[18][1], headers[18])
    hike_span = (
        last_mtp - first_mtp
        if isinstance(first_mtp, int) and isinstance(last_mtp, int)
        else NOT_READ
    )
    if isinstance(record["a"], int) and isinstance(record["b"], int):
        inside = sum(counts[record["a"] : record["b"]])
        legs_outside: int | str = total - inside
    else:
        legs_outside = record["a"]
    record["analysis"] = {
        "whole_hike_legs": whole_legs,
        "whole_hike_span": span(1, 18),
        "per_stage": per_stage,
        "legs_outside": legs_outside,
        "hike_legs": total if all(x is not None for x in counts) else NOT_DEFINED,
        "hike_span": hike_span,
    }
    return record
