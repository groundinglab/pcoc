# RESULTS

CASES.md v2 candidates from `gen-candidates` (`sdk/rust/gen`). Keys and inputs follow CASES.md. No deviation from that file.

The generator writes the candidate files, including `br1.txt`. It does not write this note. `header_hash` in `hashcash.txt` is SHA-256(SHA-256(fee header)), natural order, as CASES.md states for that key.

## SPEC questions

### Invalid UTF-8 in `kind` (erratum 1 open point 4; SPEC §7.3)

§7.3 requires a text string, and a CBOR text string is UTF-8. This SDK's decoder rejects a text item that is not UTF-8 as a malformed item (`cbor.rs`, `str::from_utf8`). That input is the E14 first sort: a `malformed cbor` record, exit 0. It is not Rule A5, because Rule A5 is not applied to a malformed value (E14). Erratum 1 leaves this open for erratum 2.

### Empty input (erratum 1 open point 3; SPEC §7.3, E14)

An empty input is not one definite-length CBOR item. The record is `shape` `mismatch` and `shape_item` exactly `["malformed cbor"]`. The verify CLI exits 0. It does not fill a terrain.

### Over-range stored height and the index GET (erratum 1 open point 1; E25, Rule A4.2)

E25 records an integer above `2^53 − 1` as shape `height_range` and the stored value as `not defined`. This SDK does not GET that height. `index_comparison` and `T` are `not read`.

### Bytes after a map (SPEC §7.3, E14)

Input that is not one definite-length item is a malformed CBOR value, and later measurements are still recorded. A map with bytes after it is measured: `shape_item` is exactly `["malformed cbor"]`, and the section 14 checks still run. A well-formed non-map with bytes after it has no hike object, so the record is the malformed skeleton and the terrain is not filled. A well-formed non-map with nothing after it, or a map with no `kind` text and nothing after it, is a decode failure: no record, CLI exit 1 (`object.rs` `DecodeError::DecodeFailure`).
