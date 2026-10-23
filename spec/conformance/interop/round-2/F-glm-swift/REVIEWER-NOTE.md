# Reviewer note: build F

Objects and verify records are byte-identical to every other build and cross-verify is all match on all three machines (3.17 s per leg on reference system 1, AVX2 fold). One slip: its `hashcash.txt` hashes the fee header once where CASES.md asks for SHA-256(SHA-256(fee header)); this touches no object and no verify record. Its RESULTS.md also says the SPEC section 7.4 example seed digest has a typo; that was checked and the SPEC value is correct. One test and its own bench call the AVX-512 kernel directly and stop with SIGILL on machines without AVX-512; the timings here come from a small harness (`kit/glm_bench.c`) that links its unchanged compiled kernels.

`RESULTS.md` is the builder's own report, copied unedited. `code/` is the builder's repository at the commit above, unedited. Cloud-VM timings in `RESULTS.md` only show that the build works; the timings that count are in [../results/](../results/).
