# Reviewer note: build B

Byte-identical outputs and all-match cross-verify on all three machines. Its only fast path is AVX-512, chosen at compile time, so on the AVX2-only reference machines its optimised and scalar figures are the same scalar C code (5.74 s per leg on reference system 1). The "Possible SPEC flaw" at the top of its RESULTS.md describes the probabilistic deep check that SPEC section 15.5 already documents, with its odds; it is known and by design, and the economics are in [../../calibration/leg-rate-floor-spec-2.2.md](../../calibration/leg-rate-floor-spec-2.2.md).

`RESULTS.md` is the builder's own report, copied unedited. `code/` is the builder's repository at the commit above, unedited. Cloud-VM timings in `RESULTS.md` only show that the build works; the timings that count are in [../results/](../results/).
