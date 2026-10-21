# v2.1 fill and solo leg — commodity laptop, 22 Oct 2026

Host: AMD Ryzen 5 7430U (6 cores, 12 threads, SHA-NI `sha_ni`). `nice -n 10`. Release build of `sdk/rust/hike`
at the final v2.1 fold (§11.1: `acc = (acc + mix64(word(w) XOR S)) mod 2^64`).

| measurement | result |
|---|---|
| terrain fill (256 MiB, NL4, parents and slot_mix) | 0.565 s |
| one production leg (2_000_000 steps, full 16 KiB slot each step) | 6.173 s |

Inputs: the `spec/conformance/v2.1/prod-leg0.json` case. Leg result `0fde828f950c2c62` (the pack value).
One run shortly after other builds on the host (load average still high). Not a ratified floor; the floor method is SPEC §15.5.
