# Reviewer note: build C

Byte-identical outputs and all-match cross-verify on all three machines. Fastest leg on the SHA-NI machines (2.47 s on reference system 1, `@Vector` path). On reference system 2 (no SHA-NI) its fast path is slower than its scalar path (7.59 s against 4.21 s, same values); this is untuned and only flagged. Its CLI refuses a fixed draw label on the production profile and draws from the OS random source, so its O3 verify records differ from the others only in the six drawn indices; all results match. It is also the producer used for the packed runs.

`RESULTS.md` is the builder's own report, copied unedited. `code/` is the builder's repository at the commit above, unedited. Cloud-VM timings in `RESULTS.md` only show that the build works; the timings that count are in [../results/](../results/).
