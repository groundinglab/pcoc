# Results schema

A submission folder holds four files. CSV files are UTF-8, comma-separated, with a header row exactly as below.

## machine.json

| Key | Type | Example |
|---|---|---|
| `label` | string, your name for the machine | `"office-desktop"` |
| `cpu` | string, the CPU model as the OS reports it | `"AMD Ryzen 5 5600H with Radeon Graphics"` |
| `cores` / `threads` | integer | `6` / `12` |
| `l3_mib` | number | `16` |
| `ram_gib` | number | `15` |
| `flags` | list of strings: any of `avx2`, `avx512f`, `sha_ni`, `neon`, `sha2` | `["avx2","sha_ni"]` |
| `os` | string | `"Ubuntu 24.04"` |
| `spec` | string, SPEC version and pcoc commit | `"2.2+erratum1 @ 7b40353"` |
| `builds` | list: build ids (`A-sonnet` … `F-glm`), or `"own:<repo>@<commit>"` | `["C-grok"]` |
| `load` | string, what else was running | `"desktop session only"` |

## bench.csv

`machine,build,phase,run,seconds`

`phase` is one of `fill_optimised`, `fill_scalar`, `leg_optimised`, `leg_scalar`. One row per run. A fill is one 256 MiB production terrain fill; a leg is one 2,000,000-step production leg.

## packed.csv

`machine,build,engine,N,instance,legs_total,legs_in_window,window_s,legs_per_s,rss_kb,verify_leg,verify`

This is the format `kit/packed/` writes. `verify` is `MATCH` when the sampled leg was recomputed by a second build and matched.

## objects.sha256

`sha256sum` output for the O1-S1, O2-S2 and O3-PROD objects your build produced. They must equal [../round-2/A-sonnet-ts/objects/SHA256SUMS](../round-2/A-sonnet-ts/objects/SHA256SUMS).
