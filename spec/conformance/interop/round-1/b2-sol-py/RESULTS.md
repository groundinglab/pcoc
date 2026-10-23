# Results

- Model: GPT-5.6 Sol, autonomous coding agent (effort setting not exposed).
- Built: Python 3 seed, NL4/dag2 terrain, full-slot fold and seal, legs,
  checkpoints, waypoints, deterministic CBOR codec, offline object builder,
  section-14 record CLI, conformance-small profiles, candidate generator, and
  generator-only permanent negatives.

## Commands

```sh
python3 -m pip install -e '.[test]'
python3 -m impl.generate --out candidates --objects objects
python3 -m impl.cli verify --profile conformance-small \
  --slot-bytes 64 --sample-bytes 32 --terrain-bytes 65536 \
  --steps-per-leg 100 objects/O1.cbor
python3 -m pytest
```

## Timings

- Editable build/install: 4.0 s.
- S1 + S2 candidate/object generation: 4.16 s.
- Full candidate + O1/O2/O3 generation: 643.859 s.
- Full candidate reproducibility test (including the production vector): 90.81 s.
- O3 effective throughput in that end-to-end run: 18 / 643.859 =
  **0.027956 legs/s**. This conservative figure includes all candidate work,
  O1/O2, and both production terrain fills; an isolated leg-only rate was not
  measured.

## SPEC questions and implemented readings

1. CASES `nl4`, profile S1: CASES asks for words `j=0…15`, but S1 has
   `SLOT_BYTES=64`, hence SPEC §6/§16.1 gives `WORDS_PER_SLOT=8`. Stored words
   8…15 do not exist. I emitted every defined base/stored pair, `j=0…7`, rather
   than inventing bytes or reading into the next slot.
2. CASES `rate`: “the SPEC's outcome word” does not name a closed comparison
   vocabulary. I used `meet` when `q*hike_legs >= p*hike_span` with positive
   span, and `refuse` otherwise, matching Rule B2's judgment.
3. CASES `unknown-kind`: “record bytes” does not state whether the bytes include
   the CLI trailing LF. I encoded the compact JSON object from Rule A5 without
   LF before hex encoding; the Rule A5 snippet itself contains only the object.
4. SPEC §7.3 says `KIND` is longer than 23 bytes, but the required text
   `v2.2-pcoc-object` is 16 ASCII bytes. I encoded its actual 16-byte length,
   because RFC 8949 and the normative constant determine those bytes.

## Not finished

- The contradictory S1 NL4 words 8…15 cannot be emitted without fixture
  clarification.
- Full path-body/index/MTP handling in the section-14 verifier is incomplete;
  the supplied offline objects have empty paths and no index, which are covered.
