# Java SDK 2.2 candidate results

Generated from SPEC 2.2 with erratum 1 and the CASES v2 inputs. The candidate files are the generator's output. This note records the readings. No deviation from `CASES.md` keys or inputs.

## Readings

- **Beacon heights.** CASES fixes `seed_beacon` at height 1000 and `I_k` at `1001 + k`. Those heights are stored on the object and are not inputs to SHA-256 (SPEC §9). The scripted rig uses that schedule for O1–O3.
- **`header_hash`.** SPEC §13.1: `SHA-256(SHA-256(fee header))`, natural order. A single SHA-256 of the same header differs. The generator stops if the two are equal.
- **Rate tokens.** Rule B2, as clarified by erratum E04 and E05: `meets` when `hike_span > 0` and `q · hike_legs ≥ p · hike_span`, otherwise `refuse`.
- **Anchor vector.** §16.1 **Anchor comparison**: `anchored` only at `H + {2, 3, 4}`. The integer vector does not test a stored interrupt or a plant.
- **Draws.** Each of the six draws reads the label chain from `b_0` (§16.1 **Thorough rounds**). The thorough draw is §15.5 with the fixture's `thorough_rounds = 3`, not a policy value.
- **Unknown kind.** Rule A5 is the two-key JSON object, no newline in the pack bytes. The CLI adds one trailing LF (§14.7, §18).
- **Negatives.** Computed as §16 **Permanent negatives** (erratum E01), only in the generator source set. The generator stops if any equals the honest value or if two are equal. `final_stage_hoist` is checked equal to an XOR accumulator of `mix64(word XOR S)`.

## SPEC questions

1. **Invalid UTF-8 in `kind` (erratum 1 open point 4; erratum 2 E01; SPEC §7.3 line 495).** §7.3 requires definite-length UTF-8 text. RFC 8949 does not allow an invalid UTF-8 sequence in a text string, and §7.3 lists that class of input as not a text string. Reading implemented: the value is a malformed CBOR item, so the record is `shape` `mismatch` with `shape_item` exactly `["malformed cbor"]`, and the verify CLI exits 0. Rule A5 is not applied, because there is no kind text. Erratum 2 E02 records `paths[g].path_state` and `plant` for g in 1…16 as `not defined`, `skip_list` as `[]`, and each `checkpoint_count`, `whole_hike_legs`, and `hike_legs` as `not defined`. The CLI test covers a one-byte `0xFF` kind value and the full record bytes.
2. **Over-range stored height and the index GET (erratum 1 open point 1; SPEC §14.1 / E25).** A height above `2^53 − 1` is `not defined` and is not a usable unsigned value. Reading implemented: the verifier does not GET that height. `index_comparison` and `T` are `not read` because the read is not made. White Hat's note that the record is identical either way is why this does not change pack bytes.
3. **Empty input (SPEC §7.3 line 497; §18; erratum 2 E02).** An empty input is not one CBOR data item. Reading implemented: the same malformed-CBOR record as item 1, exit 0. Bytes that end inside an item (`58 05 01`) use that record too. Same sentence, second clause: one well-formed data item that is not a map, or a map with no `kind` text, is a decode failure, exit 1, and no record is written. The CLI test uses `0x00` and an empty map `0xa0` for that clause. An empty path byte string on a decoded stage stays `path_state` `skip` (Rule A4.5). A decoded empty checkpoint list stays count 0 (SPEC line 961).
4. **`--profile live` (SPEC §18).** Phase A has no live index client (that is Phase C). `--profile live` and `--profile test` measure with the section 6 constants, the same as an omitted profile. The index is whatever `--index` supplies. No URL is filled in (SPEC §9: the default profile does not fill a groundinglab index URL). With no index, header reads are `not read`.
5. **`UseSHA256Intrinsics` is a diagnostic flag.** The JDK reports it to the process only when the VM is started with `-XX:+UnlockDiagnosticVMOptions`. The flag's default is already on; unlocking does not change it. `selftest` reports `sha-ni` only when that flag, `UseSHA`, and CPU SHA-NI are all reported on. SHA-256 itself is `MessageDigest` `SHA-256`, which is the JDK intrinsic.
6. **Median-time-past previous-hash (erratum E26).** The portable index has no previous-hash member. The verifier does not invent one and does not drop a window time for a missing link.

No other deviation from `CASES.md`.
