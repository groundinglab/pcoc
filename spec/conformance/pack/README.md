# pack-2.2.0

Status: incomplete. The pack-2.2.0 vectors are generated in the SDK phase. This directory does not invent hashes or vector values.

`manifest.json` names this pack, sets `"status": "incomplete"`, and lists the files present. `sdk/SPEC_PIN` records the SHA-256 of that manifest beside the tag `spec-2.2.0`. The maintainers cut that tag on the merge commit. This pack does not create the tag.

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

## Missing

Each row is something SPEC §16 says the pack publishes, or a permanent negative the pack carries. Nothing below was copied. The previous pack, the gate test vectors, and the Bari Hashcash file were inspected and left out. The gate and Bari copies stay next to those tests; they are not on the delete list for this change.

| Item | State | Reason |
|---|---|---|
| seed | missing | No seed vector was shown against §10. |
| NL4 base words | missing | No NL4 vector file is in the tree. |
| terrain digests | missing | §16 assigns these to `kat-small.json` and `test-3leg.json`, which are not in this pack. |
| §11.1 fold | missing | No fold vector was generated. |
| `conformance-small` leg bytes | missing | Not generated. The SDK phase publishes them. |
| `conformance-small` checkpoint bytes | missing | The previous checkpoint-preimage file hashes its own 1096-byte hex, which matches the §11.4 length at section 6 sizes, but those bytes are not the `conformance-small` checkpoint this pack publishes. |
| `conformance-small` waypoint bytes | missing | Not generated. |
| one production-size leg | missing | Not generated. |
| `waypoint_equals_seed` | missing | No vector. |
| integer rate comparisons | missing | No vector. |
| anchor comparisons | missing | The previous anchor files publish `object_hex` and a section 14 record. §16 says this pack publishes neither. |
| thorough-round index draw | missing | No vector. §16 also says this pack does not publish `draw_bytes_hex`. |
| Hashcash challenge and digest for a stated origin and fee header | missing | No generated vector. §13.1 binds the Bari origin and the double SHA-256 of the fee header. Not copied. |
| Bari paid rule, current fee header | missing | No vector. |
| Bari paid rule, before fee header | missing | No vector. |
| Bari paid rule, after fee header | missing | No vector. |
| Bari paid rule, outside fee header | missing | No vector. |
| Bari paid rule, wrong origin | missing | No vector. |
| display-order mismatch, current, answered 402 | missing | `blockhash-display.json` does not answer a POST. |
| display-order mismatch, before, answered 402 | missing | No vector. |
| display-order mismatch, after, answered 402 | missing | No vector. |
| after case, live plane (mainnet target, including a retarget boundary) | missing | No vector, and no plane label. |
| after case, test plane (fixed synth `nBits`, §9) | missing | No vector, and no plane label. |
| wrong `nBits`, live plane | missing | No vector, and no plane label. |
| wrong `nBits`, test plane | missing | No vector, and no plane label. |
| plane label on each vector | missing | No generated Bari or Hashcash vector carries a plane label. |
| unknown-kind record | missing | The previous earlier-run record is `version` and `kind`. §16 says an unknown kind is the Rule A5 line: `kind` and `kind_text` only. |
| `sign-in.json` | missing | The example `challenge_bytes` hex and `bip340_message_32` in the previous file match the hex printed in §15.1, and SHA-256 of those challenge bytes is that digest. The file's website outcomes include `earlier_version` and omit `unknown_kind`, which §15.1 names. Not copied. |
| `sign-in-origins.json` | missing | §15.1 names this path. The previous file's signatures were not re-checked, and the file describes the previous specification. Not copied. |
| `kat-small.json` | missing | The previous file publishes `object_hex` and a section 14 `record`, which §16 says this pack does not publish, and the embedded kind is not `v2.2-pcoc-object`. When generated, it pins terrain digests and per-stage leg, checkpoint, and waypoint bytes only. |
| `test-3leg.json` | missing | Same as `kat-small.json`: the previous file publishes `object_hex` and a section 14 record, and the embedded kind is not `v2.2-pcoc-object`. |
| `legs_le_hex` spelling | missing as a field | No vector carries it yet. §16: it is the hex of the little-endian wire bytes `le64(leg_result)`, not the `%016llx` spelling of the integer. |
| negative `slot_summary_fold` | missing | No non-match vector. |
| negative `xor_accumulator_fold` | missing | No non-match vector. |
| negative `xor_w_fold` | missing | No non-match vector. |
| negative `s_plus_w_fold` | missing | No non-match vector. |
| negative `prefold_hoist` | missing | No non-match vector. |
| negative `partial_slot_read` | missing | No non-match vector. |
| negative `partial_slot_read_2047` | missing | No non-match vector. |
| negative `one_lane_flip` | missing | No non-match vector. |
| negative `all4_kernel` | missing | No non-match vector. |
| negative `f3_shared_terrain` | missing | No non-match vector. |
| negative cached leg | missing | No non-match vector. |
| negative terrain digest from a squeezed seed | missing | No non-match vector. §6 says there is no seed domain label and no squeeze of the seed. |

`stage0-start.json` is not in the §16 publish list. It was not shown byte-correct under §13 and was not copied.

The previous `constants.json` names a different `KIND`, includes a seed-domain label, and carries leg-rate pairs. §6 sets `KIND` to `v2.2-pcoc-object`, says there is no seed domain label, and does not put those pairs in the constant table. It was not copied.

## `tools/`

`spec/conformance/tools/` stays where it is. The only file is `ring1_checks.py`. The filename is not version-named. The script hardcodes the previous specification path and the previous pack path, so it is specific to that tree. It is not edited in this change.
