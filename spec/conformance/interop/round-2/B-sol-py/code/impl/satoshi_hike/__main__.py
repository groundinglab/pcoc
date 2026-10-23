"""Command-line interfaces required by SPEC sections 13, 14, 16, and 18."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from . import cbor
from .benchmark import print_run
from .generate import fixture_index, generate_all, generate_candidates, produce_objects
from .profiles import PRODUCTION, conformance_small
from .verify import chain_draw_source, compact_json, verify_bytes


def _profile(args: argparse.Namespace):
    if args.profile == "production":
        return PRODUCTION
    values = (
        args.slot_bytes,
        args.sample_bytes,
        args.terrain_bytes,
        args.steps_per_leg,
    )
    if any(value is None for value in values):
        raise ValueError(
            "conformance-small requires --slot-bytes, --sample-bytes, "
            "--terrain-bytes, and --steps-per-leg"
        )
    return conformance_small(
        slot_bytes=args.slot_bytes,
        sample_bytes=args.sample_bytes,
        terrain_bytes=args.terrain_bytes,
        steps_per_leg=args.steps_per_leg,
    )


def _index_reader(args: argparse.Namespace):
    if args.fixture_index:
        return fixture_index
    if args.index_dir:
        root = Path(args.index_dir)

        def read(height: int):
            path = root / f"{height}.json"
            if not path.is_file():
                return None
            try:
                return json.loads(
                    path.read_text(encoding="utf-8"),
                    object_pairs_hook=_reject_duplicates,
                    parse_float=lambda _text: (_ for _ in ()).throw(ValueError()),
                )
            except (OSError, UnicodeError, ValueError, json.JSONDecodeError):
                return None

        return read
    return lambda _height: None


def _reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON member")
        result[key] = value
    return result


def _verify(args: argparse.Namespace) -> int:
    try:
        profile = _profile(args)
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    try:
        data = Path(args.object).read_bytes()
    except OSError as exc:
        print(str(exc), file=sys.stderr)
        return 1
    source = (
        chain_draw_source(args.draw_label.encode("ascii"))
        if args.draw_label is not None
        else None
    )
    try:
        record = verify_bytes(
            data,
            profile,
            _index_reader(args),
            source,
            optimized=not args.scalar,
        )
    except cbor.CBORError:
        return 1
    sys.stdout.buffer.write(compact_json(record) + b"\n")
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(prog="python -m impl.satoshi_hike")
    commands = root.add_subparsers(dest="command", required=True)

    verify = commands.add_parser("verify", help="emit the SPEC section 14 record")
    verify.add_argument("object")
    verify.add_argument(
        "--profile", choices=("production", "conformance-small"), default="production"
    )
    verify.add_argument("--slot-bytes", type=int)
    verify.add_argument("--sample-bytes", type=int)
    verify.add_argument("--terrain-bytes", type=int)
    verify.add_argument("--steps-per-leg", type=int)
    verify.add_argument("--fixture-index", action="store_true")
    verify.add_argument("--index-dir")
    verify.add_argument("--draw-label")
    verify.add_argument("--scalar", action="store_true")
    verify.set_defaults(func=_verify)

    candidates = commands.add_parser("generate-candidates")
    candidates.add_argument("--output", default="candidates")
    candidates.set_defaults(
        func=lambda args: (generate_candidates(Path(args.output)), 0)[1]
    )
    objects = commands.add_parser("produce-objects")
    objects.add_argument("--output", default="objects")
    objects.set_defaults(func=lambda args: (produce_objects(Path(args.output)), 0)[1])
    everything = commands.add_parser("generate-all")
    everything.add_argument("--candidates", default="candidates")
    everything.add_argument("--objects", default="objects")
    everything.set_defaults(
        func=lambda args: (
            generate_all(Path(args.candidates), Path(args.objects)),
            0,
        )[1]
    )
    benchmark = commands.add_parser("benchmark")
    benchmark.set_defaults(func=lambda _args: (print_run(), 0)[1])
    return root


def main() -> int:
    args = parser().parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
