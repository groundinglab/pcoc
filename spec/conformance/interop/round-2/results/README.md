# Round-2 results

All figures were measured on the three reference machines from each build's own source. Builders' cloud timings are not used.

| File | What |
|---|---|
| [REFERENCE-SYSTEMS.md](../../REFERENCE-SYSTEMS.md) | The reference systems and the load during the runs |
| [SAME-BOX.md](SAME-BOX.md) | Full per-machine report: build and leg timings, tests, candidate and object comparison, cross-verify grids |
| [SAME-BOX-packed.md](SAME-BOX-packed.md) | Packed runs: N independent hikes per machine |
| [SAME-BOX-notes.md](SAME-BOX-notes.md) | Method details, fixes applied to the environment (none to any build), failures and anomalies |
| [bench.csv](bench.csv) | Terrain fill and leg, optimised and scalar, median and minimum per build per machine |
| [packed-ref1.csv](packed-ref1.csv), [packed-ref3.csv](packed-ref3.csv), [packed-ref2.csv](packed-ref2.csv) | One row per packed instance |
| [verify-grid.csv](verify-grid.csv) | Every cross-verify cell: machine, verifier, producer, object, outcome |
| [outputs-diff.md](outputs-diff.md) | The 14 fixture outputs compared byte for byte |
| [negatives.csv](negatives.csv) | Permanent-negatives output per build per machine |
| [raw/](raw/) | Verify records, logs, candidate files and packed-run logs per machine |

Report copies have private identifiers removed (home paths, host names other than the three machine names, working repository names, agent and process ids). Nothing else was changed.
