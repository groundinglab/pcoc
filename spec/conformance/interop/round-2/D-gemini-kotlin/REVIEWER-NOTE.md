# Reviewer note: build D

Byte-identical outputs and all-match cross-verify on all three machines; the JIT vectorises its leg loop (2.75 s on reference system 1). Its 512-bit Vector API path is emulated on AVX2 and is about ten times slower, so it is not used. Its RESULTS.md lists possible SPEC flaws. They were reviewed and none is adopted as a SPEC flaw: the cases that fake legs are the cheat that the deep check prices (see the calibration note), and the rest are policy or timing matters the floor and the submission deadline already bound. It is also the independent library used to recompute sampled legs from the packed runs.

`RESULTS.md` is the builder's own report, copied unedited. `code/` is the builder's repository at the commit above, unedited. Cloud-VM timings in `RESULTS.md` only show that the build works; the timings that count are in [../results/](../results/).
