from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from impl.satoshi_hike.generate import generate_candidates


class GeneratedArtifactsTests(unittest.TestCase):
    def test_candidate_generator_is_reproducible(self) -> None:
        committed = Path("candidates")
        if not committed.is_dir():
            self.skipTest("candidates not generated yet")
        with tempfile.TemporaryDirectory() as directory:
            regenerated = Path(directory)
            generate_candidates(regenerated)
            names = sorted(path.name for path in committed.glob("*.txt"))
            self.assertEqual(
                names, sorted(path.name for path in regenerated.glob("*.txt"))
            )
            for name in names:
                self.assertEqual(
                    (committed / name).read_bytes(),
                    (regenerated / name).read_bytes(),
                    name,
                )


if __name__ == "__main__":
    unittest.main()
