from __future__ import annotations
import argparse, sys
from pathlib import Path
from .core import Profile, PRODUCTION
from .verify import json_line, verify_bytes


def main():
    p=argparse.ArgumentParser(prog="satoshi-hike")
    sub=p.add_subparsers(dest="command",required=True)
    v=sub.add_parser("verify"); v.add_argument("object",type=Path)
    v.add_argument("--profile",choices=("production","conformance-small"),default="production")
    for x in ("sample-bytes","slot-bytes","terrain-bytes","steps-per-leg"):
        v.add_argument("--"+x,type=int)
    a=p.parse_args()
    try: data=a.object.read_bytes()
    except OSError as e: print(e,file=sys.stderr); return 1
    if a.profile=="conformance-small":
        vals=(a.sample_bytes,a.slot_bytes,a.terrain_bytes,a.steps_per_leg)
        if any(x is None for x in vals): p.error("conformance-small requires all four size integers")
        profile=Profile(slot_bytes=a.slot_bytes,sample_bytes=a.sample_bytes,
                        terrain_bytes=a.terrain_bytes,steps_per_leg=a.steps_per_leg)
    else: profile=PRODUCTION
    try: record=verify_bytes(data,profile,a.profile)
    except Exception as e: print(e,file=sys.stderr); return 1
    sys.stdout.write(json_line(record)); return 0
if __name__=="__main__": raise SystemExit(main())
