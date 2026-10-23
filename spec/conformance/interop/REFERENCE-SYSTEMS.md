# Reference systems

The timings in this evidence were taken on three commodity machines, owned and run by groundinglab. They are described by configuration, not by name. Each is ordinary, widely sold hardware: two laptops and a mini PC. None has a cache large enough to hold a 256 MiB terrain, so every figure is bound by main memory.

| | Role | CPU | Cores / threads | L3 | SHA-NI | Vector | RAM | OS |
|---|---|---|---|---|---|---|---|---|
| **Reference system 1** | primary reference | AMD Ryzen 5 5600H (Zen 3), laptop | 6 / 12 | 16 MiB | yes | AVX2, no AVX-512 | 15 GiB usable | Ubuntu 24.04, Linux 6.8 |
| **Reference system 2** | low-end host (SPEC section 15.5); the leg-rate floor is calibrated on it | Intel Core i7-10610U, mini PC | 4 / 8 | 8 MiB | no | AVX2, no AVX-512 | 31 GiB usable | Debian 12, Linux 6.1 |
| **Reference system 3** | second commodity system | AMD Ryzen 5 7430U (Zen 3), laptop | 6 / 12 | 16 MiB | yes | AVX2, no AVX-512 | 15 GiB usable | Ubuntu 24.04, Linux 7.0 |

RAM is the usable total that `free` reports. The full `lscpu` and memory output for each system is in [round-2/results/raw/](round-2/results/raw/) (`ref1`, `ref2`, `ref3`).

Toolchains, the same versions on all three: gcc (system), Zig 0.15.2, Temurin JDK 21.0.12.1 with Kotlin 2.0.21 (Gradle 8.10.2 wrapper), Node 22.23.4, Swift 6.1.2, .NET SDK 8.0.426.

Load during the runs:
- **Reference system 1:** desktop session only.
- **Reference system 2:** a batch job (about 40 % of one thread) was running and was paused for the packed runs. Single-hike bench figures with it paused match the loaded ones.
- **Reference system 3:** a browser and desktop apps were open, and some swap was already in use. Every instance kept its full 256 MiB resident and leg times were steady.

The 0.2 legs/s floor is calibrated on reference system 2 (low-end), with reference system 1 as the primary reference ([calibration note](calibration/leg-rate-floor-spec-2.2.md)).

Results from other machines are welcome: see [community/](community/).
