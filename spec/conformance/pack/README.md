# Conformance pack (`pack-2.2.0`)

Status: complete. These bytes are valid for Satoshi Hike **2.3** (tag `spec-2.3.0`). The manifest id `pack-2.2.0` is historical and is left unchanged so the manifest hash pinned beside `spec-2.3.0` stays put. The draw fixture label `pcoc-pack-2.2/draw` inside `draw.json` is part of that vector, not the specification version.

The construction files below are the candidate lines on which Rust, C, Go, and Java already agree. A value was written only when those four were byte-identical. This pack does not invent a hash. `sign-in.json` and `sign-in-origins.json` are produced by the Rust SDK (plan decision D7) and checked by the admission gate.

`manifest.json` names this pack, sets `"status": "complete"`, and lists the files present. `sdk/SPEC_PIN` records the SHA-256 of that manifest beside the tag `spec-2.3.0`. This pack does not create that tag.

Where a published vector and a sentence of the specification conflict, the conflict is a defect in the specification. Report it and do not pick a side (SPEC §16, Precedence). Evidence is the hash-bound set (SPEC §8). The portable index stands in for Bitcoin. Anyone may bypass the index and read Bitcoin directly (SPEC §9 and §14). Vector bytes in this directory are Apache-2.0. Prose here is CC BY 4.0.

## Present

### `blockhash-display.json`

SPEC §9 states one reversal rule: `natural[i] = display[31 − i]`, and Bitcoin display order is `display[i] = natural[31 − i]`. The loose display-order file was checked against that same reversal sentence. `stored_hash` is natural order. `block_hash` on `P_g`, `P_b`, and `claimed_block` is display order.

Shown, before this file was written, with SHA-256 over the 80-byte `header_hex` (Bitcoin mainnet genesis, `bitcoin_height` 0):

- SHA-256(SHA-256(header)) equals `natural_block_hash` `6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000`.
- Reversing those 32 bytes equals `display_block_hash` `000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f`.
- `display[i] = natural[31 − i]` holds for every byte.

The id, header, height, and both hashes are the fields of the loose display-order file this pack replaces. Two note sentences were not kept: one named the previous pack, and one claimed an inclusion height for a hike object that was not checked.

This file is the display spelling of one header. It is not a Bari paid-rule vector and it is not a 402 answer.

### `manifest.json` and this README

SPEC §16 says the pack's files are listed in `manifest.json` and `README.md`. Both are present. The manifest does not list itself. It lists no hash.

## Present from the four SDKs

Each JSON file other than `blockhash-display.json` is the tab-separated candidate of that name, agreed by all four SDKs. `legs_le_hex` is the hex of `le64(leg_result)`. No file here carries `object_hex` or `draw_bytes_hex`.

| File | What it pins |
|---|---|
| `seed.json` | Seed preimage, `D`, and the four lanes, for two public keys. |
| `nl4.json` | NL4 base words and the stored words. |
| `terrain.json` | Terrain digests for S1, S2, and the production profile. |
| `fold.json` | The §11.1 fold on the first four S1 steps, and the leg result. |
| `kat-small.json` | `conformance-small` S1 terrain digest and per-stage leg, checkpoint, and waypoint bytes. |
| `test-3leg.json` | The same bytes for S2, three legs a stage. |
| `prod.json` | One production-size leg: `legs_le_hex` `88a04ac5beacb8a7`, its checkpoint, and the terrain digest. |
| `wes.json` | `waypoint_equals_seed`. |
| `rate.json` | Integer rate comparisons (`meets` or `refuse`). |
| `anchor.json` | Integer anchor comparisons (`anchored` or `not anchored`). |
| `draw.json` | The thorough-round label `pcoc-pack-2.2/draw` and the index lists. Not the draw bytes. |
| `hashcash.json` | Hashcash challenge and digest for origin `https://example.com` and the stated fee header. |
| `br1.json` | Rule BR1: current, before, after, outside, wrong origin, three display-order mismatches, and live-plane and test-plane after and wrong `nBits`. Status is 204 or 402. |
| `negatives.json` | The 13 permanent negatives, each unequal to the honest value. |
| `unknown-kind.json` | The Rule A5 line: `kind` and `kind_text` only. |
| `blockhash-display.json` | The §9 display reversal of the Bitcoin genesis header. |
| `sign-in.json` | §15.1 sign-in wire for the example origin. Rust SDK. Not a section 14 record. |
| `sign-in-origins.json` | §15.1 origin comparison for a service that serves only `https://example.com`. Rust SDK. |

`stage0-start.json` is not in the §16 publish list. It was not copied.

The previous `constants.json` names a different `KIND`, includes a seed-domain label, and carries leg-rate pairs. §6 sets `KIND` to `v2.2-pcoc-object`, says there is no seed domain label, and does not put those pairs in the constant table. It was not copied.

## What this directory is

Pinned vector files only. Generators and interop kits are not part of this pack.
