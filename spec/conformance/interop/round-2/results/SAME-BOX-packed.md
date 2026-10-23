## Packed runs (N concurrent independent production hikes on one machine)

Producer: C Grok (Zig), normal optimised `.fast` engine, plus `.scalar` on reference system 2. Gemini was not used as a producer (trimmed plan); it is the cross-checker.
Driver: `kit/packed/grok_packed.zig`, a thin driver compiled against Grok's unchanged `hike` library module (`src/root.zig`; `zig build-exe -OReleaseFast --dep hike -Mroot=… -Mhike=src/root.zig`). Each instance is its own hike:
- its own pk = SHA-256(label `pcoc-packed/<machine>/grok/<engine>/N<n>/i<k>`), so its own seed (SPEC §10 with the CASES seed beacon) and its own 256 MiB terrain fill;
- chained 2,000,000-step production legs from its own checkpoints: previous = waypoint = seed D, then each leg's §11.4 checkpoint feeds the next leg (stage-0 chaining, SPEC §11.2–11.4);
- no shared terrain and no shortcuts.
Timing: all N start together. The steady window opens when every instance has finished leg 0 (fill and leg 0 discarded) and runs 60 s. Per-instance legs/s = (legs ending in the window − 1) / (time between the first and last of those leg ends). Instances keep walking until the window closes, so every counted leg ran under full N-way load.
Cross-check: the last counted leg of every instance was recomputed from its logged pk and previous checkpoint by Gemini's unchanged library (`kit/packed/GeminiPacked.java` against `satoshi-hike-all.jar`: computeSeed → fillTerrain → legResult → computeCheckpoint). Both leg result and checkpoint must match. As a sanity check, both drivers reproduce the CASES pk_A leg 0 (`88a04ac5beacb8a7`).
Memory: RSS per instance read mid-window; used/available from `free`. Raw per-instance data is in `packed-<machine>.csv`.

### Reference system 1 (primary)

Load: desktop session only (a desktop app, cinnamon); nothing heavy.

| engine | N | per-hike legs/s median | min | aggregate legs/s | s/leg (median) | RSS/instance | mem used / avail | swap used | load (1 min) | sampled legs vs Gemini |
|---|---|---|---|---|---|---|---|---|---|---|
| fast | 1 | 0.402 | 0.402 | 0.402 | 2.49 | 256 MiB | 4.43 / 10.60 GiB | 0.00 GiB | 0.74 | 1/1 match |
| fast | 4 | 0.283 | 0.282 | 1.133 | 3.54 | 256 MiB | 5.16 / 9.87 GiB | 0.00 GiB | 2.56 | 4/4 match |
| fast | 6 | 0.190 | 0.184 | 1.140 | 5.26 | 256 MiB | 5.65 / 9.38 GiB | 0.00 GiB | 4.58 | 6/6 match |

Below 0.2 legs/s per hike at **N=6** (median 0.190, min 0.184). N=4 holds 0.283. Aggregate throughput saturates at about 1.13–1.14 legs/s from N=4 on: the 256 MiB terrains are DRAM-bound (16 MiB L3), so adding hikes past 4 only splits a fixed bandwidth.

### Reference system 2 (low-end host per SPEC §15.5)

Load: a batch job (about 40% of one thread, not a groundinglab production service) was paused for the timed runs and resumed right after. The PCOC CI runner containers (ciheavy podman) were left untouched; they were idle (about 0.3% CPU).

| engine | N | per-hike legs/s median | min | aggregate legs/s | s/leg (median) | RSS/instance | mem used / avail | swap used | load (1 min) | sampled legs vs Gemini |
|---|---|---|---|---|---|---|---|---|---|---|
| fast | 1 | 0.134 | 0.134 | 0.134 | 7.44 | 256 MiB | 1.96 / 29.26 GiB | 0.00 GiB | 0.84 | 1/1 match |
| fast | 2 | 0.137 | 0.137 | 0.274 | 7.31 | 256 MiB | 2.20 / 29.02 GiB | 0.00 GiB | 1.35 | 2/2 match |
| scalar | 1 | 0.242 | 0.242 | 0.242 | 4.13 | 256 MiB | 1.95 / 29.27 GiB | 0.00 GiB | 1.36 | 1/1 match |
| scalar | 2 | 0.238 | 0.238 | 0.477 | 4.19 | 256 MiB | 2.20 / 29.02 GiB | 0.00 GiB | 1.56 | 2/2 match |

Only N=1 and N=2 were run (trimmed plan). Neither engine drops below its own single-hike rate at N=2 (4 cores). Best legal packed rate on the low-end host: **0.238 legs/s per hike at N=2 with Grok `.scalar`** (aggregate 0.477). Grok `.fast` is **below 0.2 even at N=1** (0.134 legs/s, 7.44 s/leg). That is Grok's no-SHA-NI anomaly: its `.fast` path is slower than its scalar path on this CPU.

**Single-hike `hike bench` with the worker paused** (3 runs, s, median / min): fill fast 0.263 / 0.263, fill scalar 0.382 / 0.382, leg fast 7.564 / 6.956, leg scalar 4.207 / 4.188. These are the same as the loaded figures in the per-machine table (7.59 / 6.99 and 4.21 / 4.20), so the worker's about 40% of one thread had no measurable effect on reference system 2 leg timing.

Swap: Reference system 2 has no swap device; the 1.8 MB reported is noise from `free`. Available memory stayed at 29 GiB.

### Reference system 3

Load: Chrome open (about 43 processes, about 12% CPU at start), plus a desktop app and gnome-system-monitor. 2 GiB of swap was already in use before the run (pre-existing); swap_used below is the total.

| engine | N | per-hike legs/s median | min | aggregate legs/s | s/leg (median) | RSS/instance | mem used / avail | swap used | load (1 min) | sampled legs vs Gemini |
|---|---|---|---|---|---|---|---|---|---|---|
| fast | 1 | 0.375 | 0.375 | 0.375 | 2.67 | 256 MiB | 6.88 / 8.14 GiB | 2.31 GiB | 0.51 | 1/1 match |
| fast | 4 | 0.246 | 0.242 | 0.985 | 4.06 | 256 MiB | 7.43 / 7.59 GiB | 2.51 GiB | 2.18 | 4/4 match |
| fast | 6 | 0.170 | 0.163 | 1.015 | 5.89 | 256 MiB | 7.77 / 7.25 GiB | 2.67 GiB | 3.93 | 6/6 match |

Below 0.2 legs/s per hike at **N=6** (median 0.170, min 0.163). N=4 holds 0.246 (min 0.242), so N=6 was run per the rule. Aggregate saturates at about 1.0 legs/s, the same DRAM-bound shape as reference system 1 and about 10% lower, consistent with Chrome load and the slower 7430U.

Swap: swap use rose from 2.31 to 2.67 GiB across the ladder, but 7.2 GiB stayed available throughout. That looks like the kernel paging out cold desktop/Chrome pages, not the hikes being swapped: each instance's RSS stayed at its full 256 MiB and leg times were steady. Flagged.

