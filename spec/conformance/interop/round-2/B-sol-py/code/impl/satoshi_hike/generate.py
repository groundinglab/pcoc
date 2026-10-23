"""CASES.md candidate and offline-object generator."""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path
from typing import Any

from .core import Terrain, fill_terrain, lanes, leg, nl4_base_word, seed, sha256
from .object import encode_object, produce_offline
from .profiles import PRODUCTION, S1, S2, Profile
from .verify import chain_draw_source, draw_indices, thorough_draw, unknown_kind_record

PK_A = bytes.fromhex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
PK_B = bytes.fromhex("dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659")
SEED_BEACON = sha256(b"pcoc-pack-2.2/beacon/seed")
INTERRUPTS = [sha256(b"pcoc-pack-2.2/beacon/" + str(k).encode()) for k in range(18)]
SEED_HEIGHT = 1000
INTERRUPT_HEIGHTS = list(range(1001, 1019))
DRAW_LABEL = b"pcoc-pack-2.2/draw"


def fixture_index(height: int) -> dict[str, Any] | None:
    if not 989 <= height <= 1018:
        return None
    if height == 1000:
        natural = SEED_BEACON
    elif 1001 <= height <= 1018:
        natural = INTERRUPTS[height - 1001]
    else:
        natural = sha256(b"pcoc-pack-2.2/index/filler/" + str(height).encode())
    return {
        "network": "synthetic",
        "header": {
            "hash": natural[::-1].hex(),
            "height": height,
            "time": 1_700_000_000 + 600 * height,
        },
    }


def _write_group(directory: Path, name: str, values: dict[str, str]) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    content = "".join(f"{key}\t{values[key]}\n" for key in sorted(values))
    (directory / f"{name}.txt").write_text(content, encoding="ascii", newline="\n")


def _seed_values() -> dict[str, str]:
    values: dict[str, str] = {}
    for label, key in (("pk_A", PK_A), ("pk_B", PK_B)):
        preimage = key + SEED_BEACON
        digest = sha256(preimage)
        values[f"seed.{label}.preimage"] = preimage.hex()
        values[f"seed.{label}.D"] = digest.hex()
        for i, lane in enumerate(lanes(digest)):
            values[f"seed.{label}.P{i}_u64"] = f"{lane:016x}"
    return values


def _nl4_values(terrain: Terrain, digest: bytes) -> dict[str, str]:
    values: dict[str, str] = {}
    for i in (0, 1, 1023):
        slot = terrain.slot(i)
        for j in range(8):
            values[f"nl4.S1.slot.{i}.word.{j}.base_u64"] = f"{nl4_base_word(digest, i, j):016x}"
            values[f"nl4.S1.slot.{i}.word.{j}.stored_u64"] = (
                f"{int.from_bytes(slot[j*8:(j+1)*8], 'little'):016x}"
            )
    return values


def _fold_values(terrain: Terrain, digest: bytes) -> dict[str, str]:
    result = leg(terrain, digest, trace_steps=4)
    values: dict[str, str] = {"fold.S1.leg_result_u64": f"{result.result:016x}"}
    for n, trace in enumerate(result.trace):
        values[f"fold.S1.step.{n}.S_u64"] = f"{trace.state:016x}"
        values[f"fold.S1.step.{n}.slot_index"] = str(trace.slot_index)
        values[f"fold.S1.step.{n}.acc_u64"] = f"{trace.accumulator:016x}"
        values[f"fold.S1.step.{n}.S_next_u64"] = f"{trace.next_state:016x}"
    return values


def _hike_values(prefix: str, label: str, terrain: Terrain, obj: dict[str, Any]) -> dict[str, str]:
    root = f"{prefix}.{label}"
    values = {f"{root}.terrain_sha256": terrain.digest().hex()}
    for g, stage in enumerate(obj["stages"]):
        values[f"{root}.stage.{g}.waypoint"] = stage["waypoint"].hex()
        for j, (wire, checkpoint) in enumerate(zip(stage["leg_results"], stage["checkpoints"])):
            values[f"{root}.stage.{g}.leg.{j}.legs_le_hex"] = wire.hex()
            values[f"{root}.stage.{g}.leg.{j}.checkpoint"] = checkpoint.hex()
    return values


def _rate_values() -> dict[str, str]:
    values: dict[str, str] = {}
    for legs in (0, 1, 5, 6, 1000):
        for span in (1, 300, 3600):
            for p, q in ((1, 5), (1, 4), (7, 20)):
                key = f"rate.legs.{legs}.span.{span}.floor.{p}_{q}"
                values[key] = "meets" if q * legs >= p * span else "refuse"
    return values


def _anchor_values() -> dict[str, str]:
    return {
        f"anchor.H.100.height.{height}": (
            "anchored" if height in {102, 103, 104} else "not anchored"
        )
        for height in range(100, 107)
    }


def _draw_values() -> dict[str, str]:
    values = {
        "draw.b0_label": DRAW_LABEL.decode(),
        "draw.thorough_rounds": "3",
    }
    for total in (6, 18, 54):
        six = draw_indices(total, 6, chain_draw_source(DRAW_LABEL))
        thorough = thorough_draw(total, 3, chain_draw_source(DRAW_LABEL))
        if not isinstance(six, list) or not isinstance(thorough, list):
            raise AssertionError("fixture draw unexpectedly failed")
        values[f"draw.legs.{total}.six_leg.indices"] = ",".join(map(str, six))
        values[f"draw.legs.{total}.thorough.indices"] = ",".join(map(str, thorough))
    return values


def _unknown_values(o1: dict[str, Any]) -> dict[str, str]:
    labels = ("v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT")
    values: dict[str, str] = {}
    for n, label in enumerate(labels):
        changed = dict(o1)
        changed["kind"] = label
        encoded = encode_object(changed)
        # Decode is intentionally exercised by the verifier-facing record helper.
        record = unknown_kind_record(label)
        values[f"unknown-kind.{n}.kind_text_hex"] = label.encode().hex()
        values[f"unknown-kind.{n}.record_hex"] = json.dumps(
            record, ensure_ascii=False, separators=(",", ":")
        ).encode().hex()
        if not encoded:
            raise AssertionError("unknown-kind object encoding is empty")
    return values


def _leading_zero_bits(digest: bytes) -> int:
    total = 0
    for byte in digest:
        if byte == 0:
            total += 8
        else:
            total += 8 - byte.bit_length()
            break
    return total


def _hashcash_values() -> dict[str, str]:
    origin = b"https://example.com"
    fee_header = (
        sha256(b"pcoc-pack-2.2/hashcash/fee_header/0")
        + sha256(b"pcoc-pack-2.2/hashcash/fee_header/1")
        + sha256(b"pcoc-pack-2.2/hashcash/fee_header/2")[:16]
    )
    header_hash = sha256(sha256(fee_header))
    waypoint_bytes = sha256(b"pcoc-pack-2.2/hashcash/waypoint")
    challenge = sha256(b"bari-hc-v2" + origin + header_hash)
    nonce = 0
    while True:
        digest = sha256(challenge + waypoint_bytes + struct.pack("<Q", nonce))
        zeros = _leading_zero_bits(digest)
        if zeros >= 21:
            break
        nonce += 1
    return {
        "hashcash.bari_origin": origin.decode(),
        "hashcash.fee_header": fee_header.hex(),
        "hashcash.header_hash": header_hash.hex(),
        "hashcash.waypoint": waypoint_bytes.hex(),
        "hashcash.difficulty_bits": "21",
        "hashcash.challenge": challenge.hex(),
        "hashcash.nonce": str(nonce),
        "hashcash.digest": digest.hex(),
        "hashcash.leading_zero_bits": str(zeros),
    }


def build_material(*, include_production_object: bool) -> dict[str, Any]:
    digest_a = seed(PK_A, SEED_BEACON)
    digest_b = seed(PK_B, SEED_BEACON)
    terrain_s1 = fill_terrain(digest_a, S1)
    terrain_s2 = fill_terrain(digest_a, S2)
    terrain_b = fill_terrain(digest_b, S1)
    terrain_prod = fill_terrain(digest_a, PRODUCTION)
    o1 = produce_offline(
        PK_A, SEED_BEACON, INTERRUPTS, SEED_HEIGHT, INTERRUPT_HEIGHTS,
        terrain_s1, [1] * 18,
    )
    o2 = produce_offline(
        PK_A, SEED_BEACON, INTERRUPTS, SEED_HEIGHT, INTERRUPT_HEIGHTS,
        terrain_s2, [3] * 18,
    )
    o3 = (
        produce_offline(
            PK_A, SEED_BEACON, INTERRUPTS, SEED_HEIGHT, INTERRUPT_HEIGHTS,
            terrain_prod, [1] * 18,
        )
        if include_production_object
        else None
    )
    return {
        "digest_a": digest_a,
        "terrain_s1": terrain_s1,
        "terrain_s2": terrain_s2,
        "terrain_b": terrain_b,
        "terrain_prod": terrain_prod,
        "o1": o1,
        "o2": o2,
        "o3": o3,
    }


def generate_candidates(directory: Path, material: dict[str, Any] | None = None) -> None:
    from .generators.negatives import generate as generate_negatives

    m = material or build_material(include_production_object=False)
    first_prod = (
        m["o3"]["stages"][0]["leg_results"][0]
        if m.get("o3")
        else struct.pack("<Q", leg(m["terrain_prod"], m["digest_a"]).result)
    )
    if m.get("o3"):
        first_prod_checkpoint = m["o3"]["stages"][0]["checkpoints"][0]
    else:
        result = int.from_bytes(first_prod, "little")
        from .core import checkpoint
        first_prod_checkpoint = checkpoint(
            m["terrain_prod"], m["digest_a"], m["digest_a"], result
        )
    groups: dict[str, dict[str, str]] = {
        "seed": _seed_values(),
        "nl4": _nl4_values(m["terrain_s1"], m["digest_a"]),
        "terrain": {
            "terrain.S1.terrain_sha256": m["terrain_s1"].digest().hex(),
            "terrain.S2.terrain_sha256": m["terrain_s2"].digest().hex(),
            "terrain.PROD.terrain_sha256": m["terrain_prod"].digest().hex(),
        },
        "fold": _fold_values(m["terrain_s1"], m["digest_a"]),
        "kat-small": _hike_values("kat-small", "S1", m["terrain_s1"], m["o1"]),
        "test-3leg": _hike_values("test-3leg", "S2", m["terrain_s2"], m["o2"]),
        "prod": {
            "prod.PROD.terrain_sha256": m["terrain_prod"].digest().hex(),
            "prod.PROD.stage.0.leg.0.legs_le_hex": first_prod.hex(),
            "prod.PROD.stage.0.leg.0.checkpoint": first_prod_checkpoint.hex(),
        },
        "wes": {
            "wes.S1.D": m["digest_a"].hex(),
            "wes.S1.stage.0.waypoint": m["o1"]["stages"][0]["waypoint"].hex(),
            "wes.S1.equal": (
                "true" if m["digest_a"] == m["o1"]["stages"][0]["waypoint"] else "false"
            ),
        },
        "rate": _rate_values(),
        "anchor": _anchor_values(),
        "draw": _draw_values(),
        "unknown-kind": _unknown_values(m["o1"]),
        "hashcash": _hashcash_values(),
        "negatives": generate_negatives(m["terrain_s1"], m["terrain_b"], m["digest_a"]),
    }
    expected = {
        "seed", "nl4", "terrain", "fold", "kat-small", "test-3leg", "prod",
        "wes", "rate", "anchor", "draw", "unknown-kind", "hashcash", "negatives",
    }
    if set(groups) != expected:
        raise AssertionError("candidate group mismatch")
    directory.mkdir(parents=True, exist_ok=True)
    for stale in directory.glob("*.txt"):
        stale.unlink()
    for name, values in groups.items():
        _write_group(directory, name, values)


def produce_objects(directory: Path, material: dict[str, Any] | None = None) -> None:
    m = material or build_material(include_production_object=True)
    if m["o3"] is None:
        m["o3"] = produce_offline(
            PK_A, SEED_BEACON, INTERRUPTS, SEED_HEIGHT, INTERRUPT_HEIGHTS,
            m["terrain_prod"], [1] * 18,
        )
    directory.mkdir(parents=True, exist_ok=True)
    for name, obj in (("O1-S1.cbor", m["o1"]), ("O2-S2.cbor", m["o2"]), ("O3-PROD.cbor", m["o3"])):
        (directory / name).write_bytes(encode_object(obj))


def generate_all(candidates: Path, objects: Path) -> None:
    material = build_material(include_production_object=True)
    generate_candidates(candidates, material)
    produce_objects(objects, material)
