# Reviewer note: build A

All 14 fixture outputs and the three objects are byte-identical to every other build on all three reference machines, and it verifies every other build's objects (all match). It is the slowest optimised build: Node gives no access to the SHA-256 instructions, so its fast leg uses a WebAssembly fold with Node's crypto SHA-256 (6.32 s per leg on reference system 1). Its RESULTS.md lists parts it did not finish; those are its own words and are left as written.

`RESULTS.md` is the builder's own report, copied unedited. `code/` is the builder's repository at the commit above, unedited. Cloud-VM timings in `RESULTS.md` only show that the build works; the timings that count are in [../results/](../results/).
