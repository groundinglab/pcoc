# v2.1 fill and solo leg — commodity laptop, 22 Oct 2026

Host: AMD Ryzen 5 7430U (6 cores, 12 threads, SHA-NI `sha_ni`). `nice -n 10`. Release build of `sdk/rust/hike`
at the final v2.1 fold (wrapping-add accumulator, §11.1).

| measurement | result |
|---|---|
| terrain fill (256 MiB, NL4, parents and slot_mix) | 0.547 s |
| one production leg (2_000_000 steps, full 16 KiB slot each step) | 7.000 s |

Inputs: the `spec/conformance/v2.1/prod-leg0.json` case. Leg result `112a630861105796` (the pack value).
One run with other work on the host (load average about 3). This is not a ratified floor; the floor method is SPEC §15.5.
