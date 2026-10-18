# Known divergences between the SDKs

`sdk/test.sh` runs the Rust, Go and Java verify programs on the same inputs (`sdk/tools/crosscheck.py`). On every conformance-pack object, all three write byte-identical section 14 records. On some **malformed** inputs, Rust disagrees with Go and Java. Go and Java agree with each other in every case.

This release changes no behaviour. The Rust build from before the SDK move writes the same bytes for every case below. The harness pins each case by name and by the first 16 hex digits of the Rust and the Go/Java output digests. The run fails in three situations: a listed case starts to agree, its bytes change, or an unlisted case disagrees.

| Case | Rust | Go and Java | Impact | Tracked by |
|---|---|---|---|---|
| `kat-flip@2`, `kat-head@2`: top-level `fill` key renamed | `shape_item` `["field set"]`. Every later measurement is `not defined` or `not read` | `["field set", "constant"]`, and they go on to measure headers, checkpoints, waypoints, paths and the six-leg draw | Malformed objects only. `shape` is `mismatch` in all three, so section 15 refuses in all three. A Rust verifier writes different record bytes from a Go or Java verifier | not yet filed: spec-owner question below |
| `kat-flip@562`, `kat-head@562`: a key in a stage map renamed | `["field set"]`, then stops measuring | `["field set", "header height order"]`, and go on measuring | same | same |
| `kat-flip@3938`: one bit in a stored header height | `["last_interrupt"]` | `["last_interrupt", "header height order"]` | same | same |
| `cbor-uint` (`01`), `cbor-array` (`80`), `cbor-text` (`60`): one well-formed CBOR item that is not a map | Library: `verify` writes a record with `shape_item` `["field set"]`, the same bytes as the `kat-flip@2` record. Program: exit 0, record on stdout | Library: Go `Verify` returns `decoded == false` with a `["malformed cbor"]` record, and Java `Measure.record` returns `null`. Programs: exit 1, `undecodable` on stderr, no record | Input that is not a hike object. Every SDK refuses it, but Rust writes a record and the others do not | same |

To reproduce, run `bash sdk/test.sh`, or run `python3 sdk/tools/crosscheck.py` with the three binaries. Each divergent case prints its exit codes and all three record digests.

## Questions for the spec owner

These are spec decisions. The SDKs do not decide them. The fix lands as a new vector in a `spec` PR, and then every SDK follows.

1. **Does `field set` stop the other Rule A3 conditions?** Section 14.1 (normative) says: *"On `mismatch`, the array contains every string whose condition is violated, each string once, in the order 1…14. … A condition that is violated is not dropped because another condition is also violated."* Section 14.1 also says *"Later measurements are still recorded."* On that text, the Rust records for `kat-flip@2`, `kat-flip@562` and `kat-flip@3938` look **non-conforming**: Rust drops conditions and measurements after `field set`, or omits `header height order`. Go and Java follow the text. This needs the spec owner's confirmation before any SDK changes.
2. **Which `shape_item` does a well-formed CBOR item that is not a map get?** Row 1, `malformed cbor`, applies when *"the input is not one definite-length CBOR data item under section 7.3"*. `01`, `80` and `60` each are one such item, which points away from `malformed cbor`. Row 2, `field set`, covers *"a map that is present"*, but here no top-level map is present. Rust records `field set`. Go records `malformed cbor` and Java writes no record. Section 14.1 does not settle which is right, and no vector covers it.
