# Satoshi Hike 2.2 — Python

Independent Python 3 implementation built from the public SPEC 2.2.

```sh
python3 -m pip install -e '.[test]'
gen-candidates --objects objects
satoshi-hike verify --profile conformance-small --slot-bytes 64 --sample-bytes 32 --terrain-bytes 65536 --steps-per-leg 100 objects/O1.cbor
pytest
```

The production generator deliberately performs the full 256 MiB terrain fill and
every full-slot fold. It has no fast mode, cache, table, or partial-read path.
# New repo

This repo was created by a Cursor cloud agent.
