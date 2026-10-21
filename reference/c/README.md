# Satoshi Hike v2.1 — C reference

C11 producer and verifier for the final v2.1 construction (`spec/v2.1-correction` at `3ee7bc0`). Written from `spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md` and `spec/conformance/v2.1/`. It does not use the Rust, Go, or Java trees, and it does not use the vector generator.

The §11.1 accumulator is wrapping addition. The seal digest is `E`. `KIND` is `satoshi-hike-v2.1-object`. `make test` includes the production-size leg in `prod-leg0.json`.

`make test` runs every file in `spec/conformance/v2.1/` and the permanent adversarial rejects, including a cached leg. The test binary is `hike-test`. The CLI is `hike`.

```
make
make test
./hike info
./hike produce schedule.txt
./hike verify record.txt --leg 0 --leg 3
./hike rate --legs 2 --span 9
./hike anchors 2,4,6,8,10,12
nice -n 10 ./hike bench
```

A schedule is line-oriented: `public_key`, `seed_beacon`, `slot_bytes`, `terrain_bytes`, `sample_bytes`, `steps_per_leg`, then one `stage <leg_count> <interrupt hex>` per stage. The last stage uses `-` instead of an interrupt.

The producer walks every leg and writes a `PCOC-CHECKPOINT-V21` record. The verifier fills the terrain once from the seed, checks the seed and the stage limbs, re-walks the chosen legs (every leg when no `--leg` is given), checks each checkpoint, and recomputes each waypoint from the stored checkpoints. `hike rate` applies Rule B2 with placeholder floor `1/5`. `hike anchors` applies Rule B4 (`anchored_total >= 6` among `W_1` … `W_16`).

Hardware SHA-256 is used when CPUID reports SHA-NI and the SHA-NI compress matches software on the empty block. Otherwise the software compress runs. `hike info` prints `sha256_hw`.

See `GAP.md` for the section 7.3 object and the section 14 record. See `SPEC-FINDINGS.md`: the draft §16 catalogue is closed, and this resync found no new conflict. See `FLAGS.md` for the speed-ups that were not implemented.
