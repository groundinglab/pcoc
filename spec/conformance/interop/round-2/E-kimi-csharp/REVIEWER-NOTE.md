# Reviewer note: build E

Byte-identical outputs and all-match cross-verify on all three machines (3.94 s per leg on reference system 1, AVX2 engine). .NET has no direct SHA-256 instructions, so the seal hash goes through the system crypto library. Two of its tests call the AVX-512 engine directly and fail on machines without AVX-512 (33 of 35 pass; 33 of 33 with those two excluded); that is test portability, not an output difference. Its bench label names SHA-NI on reference system 2, which has none; the label is cosmetic.

`RESULTS.md` is the builder's own report, copied unedited. `code/` is the builder's repository at the commit above, unedited. Cloud-VM timings in `RESULTS.md` only show that the build works; the timings that count are in [../results/](../results/).
