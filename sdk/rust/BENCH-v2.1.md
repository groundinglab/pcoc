# v2.1 fill and solo leg — pepebook, 21 Oct 2026

Host: AMD Ryzen 5 7430U (6 cores, 12 threads, SHA-NI `sha_ni`). `nice -n 10`. Release build of `sdk/rust/hike`.

| measurement | result |
|---|---|
| terrain fill (256 MiB, NL4, parents and slot_mix) | 0.580 s |
| one production leg (2_000_000 steps, full 16 KiB slot each step) | 6.716 s |

Command: `cargo run -p hike --release --example bench_leg` from `sdk/rust`.

Public key `11`×32, seed beacon `22`×32. Leg result `c8658b2b05a93205`. This is one quiet run, not a ratified floor.
