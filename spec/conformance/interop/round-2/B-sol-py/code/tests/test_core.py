from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

from impl.satoshi_hike import cbor
from impl.satoshi_hike.core import fill_terrain, lanes, leg, seed
from impl.satoshi_hike.generate import (
    DRAW_LABEL,
    PK_A,
    SEED_BEACON,
    fixture_index,
)
from impl.satoshi_hike.profiles import S1
from impl.satoshi_hike.verify import (
    chain_draw_source,
    draw_indices,
    thorough_draw,
    verify_bytes,
)


class CoreTests(unittest.TestCase):
    def test_section_7_checks(self) -> None:
        self.assertEqual(
            hashlib.sha256(b"abc").hexdigest(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        seal = hashlib.sha256(b"pcoc-walk-v1" + bytes(8)).digest()
        self.assertEqual(seal.hex(), "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533")
        self.assertEqual(int.from_bytes(seal[:8], "little"), 0x76042423AFBD1648)

    def test_section_7_seed_example(self) -> None:
        digest = seed(bytes([0x11]) * 32, bytes([0x22]) * 32)
        self.assertEqual(
            digest.hex(),
            "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c",
        )
        self.assertEqual(
            lanes(digest),
            (
                0x545DFE297DC78951,
                0x27858669C45E046A,
                0x9CDAC73AC1A5FE85,
                0x7C81DF6EFBF55F11,
            ),
        )

    def test_optimized_and_scalar_are_identical(self) -> None:
        digest = seed(PK_A, SEED_BEACON)
        scalar = fill_terrain(digest, S1, optimized=False)
        optimized = fill_terrain(digest, S1, optimized=True)
        self.assertEqual(scalar.data, optimized.data)
        self.assertEqual(
            leg(scalar, digest, optimized=False),
            leg(optimized, digest, optimized=True),
        )

    def test_cbor_accepts_non_shortest_uint_and_rejects_trailing(self) -> None:
        self.assertEqual(cbor.decode(b"\x18\x01"), 1)
        with self.assertRaises(cbor.CBORError):
            cbor.decode(b"\x01\x00")
        with self.assertRaises(cbor.CBORError):
            cbor.decode(b"\xbf\xff")

    def test_fixture_draws_restart_at_b0(self) -> None:
        six = draw_indices(18, 6, chain_draw_source(DRAW_LABEL))
        thorough = thorough_draw(18, 3, chain_draw_source(DRAW_LABEL))
        self.assertIsInstance(six, list)
        self.assertIsInstance(thorough, list)
        self.assertEqual(six, thorough[:6])

    def test_committed_o1_full_verifier_record(self) -> None:
        path = Path("objects/O1-S1.cbor")
        if not path.exists():
            self.skipTest("objects not generated yet")
        record = verify_bytes(
            path.read_bytes(),
            S1,
            fixture_index,
            chain_draw_source(DRAW_LABEL),
        )
        self.assertEqual(record["shape"], "match")
        self.assertTrue(all(h["index_comparison"] == "match" for h in record["headers"]))
        self.assertTrue(
            all(
                leg_record["result"] == "match"
                for stage in record["checkpoints"]
                for leg_record in stage["legs"]
            )
        )
        self.assertTrue(all(item["result"] == "match" for item in record["waypoints"]))
        self.assertEqual(record["six_leg"]["results"], ["match"] * 6)
        self.assertEqual(record["analysis"]["hike_legs"], 18)
        self.assertEqual(record["analysis"]["hike_span"], 10_800)


if __name__ == "__main__":
    unittest.main()
