from pathlib import Path
from impl.generate import generate


def test_committed_candidates_are_reproducible(tmp_path):
    # Production is intentionally included: this is the byte-for-byte release test.
    generate(tmp_path, None, include_production=True)
    expected=Path("candidates")
    assert [p.name for p in sorted(tmp_path.iterdir())] == [p.name for p in sorted(expected.iterdir())]
    for got in tmp_path.iterdir():
        assert got.read_bytes() == (expected/got.name).read_bytes()
