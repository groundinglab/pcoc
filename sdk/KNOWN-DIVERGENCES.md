# Known divergences between the SDKs

`sdk/test.sh` cross-checks the three SDKs on the same inputs (`sdk/tools/crosscheck.py`), at two levels:

- **Programs:** the Rust, Go and Java `satoshi-hike-verify` programs must write byte-identical stdout and exit with the same code.
- **Libraries:** each SDK's verify call must return a byte-identical record. The calls are Rust `verify_sized` with `verify_record_json`, Go `Verify` with `Record.Marshal`, and Java `Measure.record`. The `librecord` programs in `examples/` print that record even when the verify program would withhold it.

On every conformance-pack object, all three SDKs write byte-identical section 14 records. The inputs are the pack objects, mutants of `kat-small`, targeted section 14.1 edits (one field absent, mistyped or out of order) and one-byte CBOR items that are not a map. The harness pins each case below by name and by the first 16 hex digits of the Rust, Go and Java output digests. The run fails in three situations: a listed case starts to agree, its bytes change, or an unlisted case disagrees.

## Open

| Case | Rust | Go | Java | Kind | Tracked by |
|---|---|---|---|---|---|
| `cbor-uint` (`01`), `cbor-array` (`80`), `cbor-text` (`60`): one well-formed CBOR item that is not a map | Library: a record with `shape_item` `["field set"]`. Program: exit 0, that record on stdout | Library: `Verify` returns `decoded == false` and a record with `["malformed cbor"]`. Program: exit 1, `undecodable` on stderr, no record | Library: `Measure.record` returns `null`. Program: exit 1, no record | **Spec question 1** below | this file; not filed |
| `field-s2-cpwidth`: stage 2's checkpoint is 31 bytes, not 32 | `W_3` `mismatch`, the stage-2 leg in the six-leg draw `mismatch`, and `checkpoint_count`/`whole_hike_legs` `not defined` | `W_3` `not defined`, that leg `match`, counts are numbers, `legs_outside` `not defined` | Same as Go, except `legs_outside` is `17` | **Bugs, not yet fixed.** Section 14 says a recompute with unusable stage bytes is `not defined` (Rust wrong). A `j = 0` leg reads only the waypoint and `leg_results[0]` (Rust wrong). `whole_hike_legs` is a JSON number (Rust wrong). `legs_outside` is `not defined` only when the bookend fields are, and here they are defined (Go wrong) | this file; not filed |

All three SDKs agree on every other case, at both levels. That includes the 11 truncated or broken-head inputs, where no program writes a record; see spec question 2.

## Fixed after rc.1

The tag `sdk-v2.0.0-rc.1` (c9b160ea) still has the bug below. It is fixed on `sdk/phase-a` after the tag.

**Rust did not record every section 14.1 condition, and stopped measuring.** Section 14.1 (normative) says: *"On `mismatch`, the array contains every string whose condition is violated … A condition that is violated is not dropped because another condition is also violated. A condition that tests a field’s value is violated when that field is absent."* It also says *"Later measurements are still recorded."* Rust departed from this in two ways:

- `hike/src/object.rs`, `read_measured_map` and `parse_stage`, used `?` on a required field. When `kind`, `fill`, a size constant, a stage key set, a stage `beacon`, `beacon_height` or `leg_count` was absent or of the wrong type, the whole object was refused. The record was then the no-object record, byte-identical to the one for the single byte `01`.
- The header-order loop stopped at the first failed pair (`break`). The closing-header check recorded `last_interrupt` but not `header height order` or `header hash differ` for the same pair.

A related bug: the six-leg draw indexed legs by `leg_count`, but section 14.6 says *"The draw does not read `leg_count`"*.

Go and Java followed the text. Rust now does too, and its records are byte-identical to theirs for every case below.

| Case | Rust before | All three now |
|---|---|---|
| `kat-flip@2`, `kat-head@2`: top-level `fill` key renamed | `["field set"]`, then nothing measured | `["field set", "constant"]`, and everything measured |
| `kat-flip@562`, `kat-head@562`: stage 1 `beacon_height` key renamed | `["field set"]`, then nothing measured | `["field set", "header height order"]`. `headers[1]` is `not defined` / `not read` |
| `kat-flip@3938`: `last_interrupt_height` below stage 17's height | `["last_interrupt"]` | `["last_interrupt", "header height order"]` |
| `field-slot-text`: `slot_bytes` is text | no-object record | `["field set", "constant"]` |
| `field-s0-beacon`, `field-s5-beacon`: a stage `beacon` key renamed | no-object record | `["field set", "seed beacon", "header hash differ"]` (stage 0), `["field set", "header hash differ"]` (stage 5, and `W_5` is `not defined`) |
| `field-s1-height`: stage 1 `beacon_height` renamed | no-object record | as `kat-flip@562` |
| `field-s3-legcount`, `field-s3-legcount-2`: stage 3 `leg_count` absent, or 2 with 1 checkpoint | no-object record; a draw over 19 legs | `["field set", "leg_count"]` / `["leg_count"]`; the draw is over 18 checkpoints |
| `field-li-equal`: `last_interrupt` equals stage 17's beacon | `["last_interrupt"]` | `["last_interrupt", "header hash differ"]` |
| `field-order-hash`: a height-order failure at stage 4 and a repeated hash at stage 10 | `["header height order"]` | `["header height order", "header hash differ"]` |

Section 14.1 is the only basis for this fix. No vector covers these inputs yet. Adding them is a spec PR.

## Questions for the spec owner

There are two. The SDKs do not decide them. Each fix lands as a new vector in a `spec` PR, and then every SDK follows.

1. **Which `shape_item` does a well-formed CBOR item that is not a map get?** Row 1, `malformed cbor`, applies when *"the input is not one definite-length CBOR data item under section 7.3"*. `01`, `80` and `60` each are one such item, which points away from `malformed cbor`. Row 2, `field set`, covers *"a map that is present"*, but here no top-level map is present. Rust records `field set`, Go records `malformed cbor`, and Java returns no record. Section 14.1 does not settle which is right, and no vector covers it.
2. **Does a verify program write the `["malformed cbor"]` record, or exit 1?** Sections 7.3 and 14.1 define a record for malformed CBOR: *"`shape_item` is exactly `["malformed cbor"]` … Later measurements are still recorded."* All three libraries return that record, byte-identical, for the 11 truncated or broken-head inputs (`kat-cut@*`, `kat-flip@1`, `kat-head@1`, `kat-head@9`, `kat-flip@17`, `kat-head@17`). The locked verify-CLI rule (section 18, item 9) says: *"Exit code `0` means the record was produced … Exit code `1` means the object file cannot be read or decoded."* All three programs exit 1 with no record on those inputs. But for `kat-trailing`, also malformed CBOR under section 7.3, they exit 0 and write the record. So each program draws the line at whether a map decodes. The spec does not say whether "cannot be decoded" means "is malformed CBOR" (then `kat-trailing` should exit 1) or "is not CBOR at all" (then the 11 inputs should exit 0 with the record). No behaviour changes until this is settled.

To reproduce, run `bash sdk/test.sh`, or run `python3 sdk/tools/crosscheck.py` with the binaries. Each divergent case prints its exit codes and all three digests.
