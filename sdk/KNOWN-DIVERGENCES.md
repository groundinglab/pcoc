# Known divergences between the SDKs

`sdk/test.sh` cross-checks the three SDKs on the same inputs (`sdk/tools/crosscheck.py`), at two levels:

- **Programs:** the Rust, Go and Java `satoshi-hike-verify` programs must write byte-identical stdout and exit with the same code.
- **Libraries:** each SDK's verify call must return a byte-identical record. The calls are Rust `verify_sized` with `verify_record_json`, Go `Verify` with `Record.Marshal`, and Java `Measure.record`. The `librecord` programs in `examples/` print that record even when the verify program would withhold it.

On every conformance-pack object, all three SDKs write byte-identical section 14 records. The inputs are the pack objects, mutants of `kat-small`, targeted section 14.1 edits, one-byte CBOR items that are not a map, and (from rc.2) the structural break cases in `sdk/tools/hikecases.py`: a wrong checkpoint or leg-result width in several stages, each stage field absent or mistyped, absent or mistyped top-level fields, 0/1/17/19 stages, a stage that is not a map, absent beacons combined with bad headers, mixed edits, out-of-range integers, six-leg previous values of the wrong width, top levels that are not a map, and trailing bytes after maps with and without other faults. The harness pins each open case below by name and by the first 16 hex digits of the Rust, Go and Java output digests. The run fails in three situations: a listed case starts to agree, its bytes change, or an unlisted case disagrees. At rc.3 the open pin is Q-b only (four rows, at both levels).

## Open

One row is an open spec question (below), not a known bug: the SPEC text does not settle which record is right, so no SDK was changed for it. Q-a and Q-i were resolved for rc.3; see [Fixed for rc.3](#fixed-for-rc3).

| Case | Rust | Go | Java | Kind |
|---|---|---|---|---|
| `stage0-uint`, `stage1-uint`, `stage5-uint`, `stage17-uint`: one element of `stages` is the unsigned integer 7, not a map | Read as a stage map with every field absent: `checkpoint_count` `0`, `whole_hike_legs` and `legs_outside` counted, `path_state` `skip` / `plant` `n/a`, `w17_path` `not fetched by design` | Read as an absent stage: `checkpoint_count`, `whole_hike_legs`, `legs_outside`, `path_state`, `plant` and `w17_path` `not defined` | Same as Rust | **Spec question Q-b**. `shape_item` agrees in all three |

All three SDKs agree on every other case, at both levels.

## Implementation resource limits

These are not divergences between the SDKs: all three apply them identically, and the cross-check pins that they agree. They are listed here because the SPEC does not state them (a one-line SPEC proposal is pending; see `FIX-FORWARD-RC3.md` in the review bundle).

| Limit | Value | Rust / Go / Java name | Effect | Derivation |
|---|---|---|---|---|
| CBOR nesting depth (rc.3, review findings F1/F3) | 64 | `hike::MAX_CBOR_NESTING` / `hike.MaxCBORNesting` / `Cbor.MAX_CBOR_NESTING` | An item inside more than 64 arrays and maps (counting itself) is malformed CBOR: `shape_item` `["malformed cbor"]`, the programs exit 1, `yielded_public_key` is `Undecodable`, Go `CBORIssue` and Java `.issue` are `nesting depth` | The deepest item a valid section 8 object can hold sits inside 4 containers: the top-level map, the `stages` array, a stage map, and a `checkpoints` or `leg_results` array (whose elements are byte strings; `path` is a byte string too, and no field is a nested map). The limit is max(4 × 4, 64) = 64, sixteen times the schema depth. |

Before rc.3 the decoders recursed without a bound: Rust aborted on stack overflow (about 8 000 levels on a 2 MiB thread, the gate's sign-in door reaching it before the signature check), and Java threw `StackOverflowError` at about 2 700 levels. Cross-check cases: `deep-arr-64`, `deep-arr-65`, `deep-arr-100000`, `deep-key-64`, `deep-key-65`, `deep-key-8000`, `deep-key-100000`, `deep-key-100000-cut`, `deep-cp-elem-64`, `deep-cp-elem-65`, `deep-cp-elem-100000`, `deep-map-64`, `deep-map-5000`. Tests: Rust `cbor::tests`, `object::tests::yielded_public_key_bounds_nesting_on_a_two_mib_thread`; Go `hike/nesting_test.go`; Java `V2Pack.nestingIsBounded`; gate `tests/locked_wire.rs` `an_8kb_nested_post_is_bad_signature_not_a_crash`.
## Fixed for rc.3

rc.2 (`72a99227`, plus the two commits to `3039067`) has every bug below. Each row names the ruling or the SPEC text that decides it. Each behaviour row is a cross-check case. The SDK tests pin the facts (Rust `hike/tests/break_cases.rs`, Go `hike/rc2_break_test.go`, Java `V2Pack.rc2Break`). Nesting depth is the resource limit above, not a row in this table.

| Case(s) | Wrong in | Was | Now (all three) | SPEC basis / ruling |
|---|---|---|---|---|
| `os-top-bstr-2pow63`, `os-top-text-2pow63`, `os-top-arr-2pow64-1`, `os-top-map-2pow32`, `os-top-arr-1e9`, `os-stages-arr-1e9`: a declared byte, text, array or map length past the bytes left | Go | panic or out of memory; no record; `Verify` panicked | `["malformed cbor"]`, exit 1; the library does not panic | §7.3 / §14.1 row 1. Ruling F2: compare the length as uint64 against the bytes remaining before allocating (a map needs 2n) |
| same heads, and an array or map count of `Integer.MAX_VALUE` (`9a 7fffffff`, `ba 7fffffff`, `9b 000000007fffffff`) | Java | `take` added the length to the cursor (overflow near 2^63); an array or map was sized from the declared count, up to `Integer.MAX_VALUE` | the same malformed record, no allocation from the declared count | Ruling F3, the F2 class: bound the count by the bytes left before sizing the collection |
| `s5-waypoint-31`, `s5-waypoint-33`, `s5-waypoint-8`: stage 5's waypoint is not 32 bytes, and seed `11…11` draws that stage's leg 0 | Go | that six-leg result `match` | `mismatch` | Ruling F4: the leg-0 start is the stored waypoint, which is 32 bytes (§14.6, §12.2), as Rust and Java already required |
| `li-height-2^53`, `li-height-2^64-1`: `last_interrupt_height` above 2^53 − 1 | Rust, Java | `shape_item` listed `height_range` only | `["header height order","height_range"]` | Ruling F5, resolving Q-a: header height order compares the decoded unsigned integer. A height above 2^53 − 1 is not a usable ordered height. 2^53 is still greater than stage 17's height, so `last_interrupt` is not listed |
| `stages19-dup-header`: 19 stages, and stage 3's header duplicates stage 2's | Rust | `shape_item` listed `stages length` only | `["stages length","header height order","header hash differ"]` | §14.1: a violated condition is not dropped because another is also violated. Ruling F6: evaluate the other conditions over the stages that are present, as the too-short case already did. The six-leg draw still indexes stages 0 … 17 |
| `text-last_interrupt`, `text-last_interrupt_height`: the value is text | all three | no `field set` | `field set`, with the conditions those fields already fail | §14.1 row 2: wrong value types. Ruling F7: field set only when `last_interrupt` is present and not a byte string, or `last_interrupt_height` is present and not an unsigned integer. `short-li` (a 31-byte byte string) stays `last_interrupt` with no `field set` |
| `cp-text-s4`, `cp-uint-s4`, `cp-map-s4`, `cp-arr-s4`, `lr-text-s4`, `lr-uint-s4`, `lr-map-s4`, `lr-arr-s4`: a checkpoint or leg-result element that is not bytes | all three | `checkpoint width` or `leg result width` only | also `field set` | §14.1 row 2 (line 1206) and line 1220. Ruling F9, resolving Q-i: the under-report was an implementation bug, not a spec question |
| verify CLI `--profile conformance-small` | all three programs | accepted with no size flags, and measured the built-in four integers | exit 2 unless `--sample-bytes`, `--slot-bytes`, `--terrain-bytes` and `--steps-per-leg` are all given; the record states those integers. Any of the four with another profile is exit 2 | §16.1 (line 1962) and §18 item 9 (line 2031). Ruling F8 |

## Fixed for rc.2

The rc.1 tags (c9b160ea) have every bug below. They are fixed on `sdk/phase-a` and in the rc.2 tags. Each row names the SPEC text that decides it; each is a cross-check case, and the SDK tests pin the facts (Rust `hike/tests/break_cases.rs`, Go `hike/rc2_break_test.go`, Java `V2Pack.rc2Break`).

| Case(s) | Wrong in | Was | Now (all three) | SPEC basis |
|---|---|---|---|---|
| `field-s2-cpwidth`, `cpw-s0` … `cpw-s17`, `cpw-s4-33`, `cpw-s4-0`: a checkpoint of the wrong width | Rust | `W_{g+1}` `mismatch` | `not defined` | §14.2 (line 1232): a recompute with unusable stage bytes is `not defined`, and `checkpoints[g].legs` is `not defined` "in that same case" |
| same | Rust | the stage's `j = 0` leg in the six-leg draw `mismatch` (the leg results were dropped with the checkpoints) | `match` | §14.6: leg 0 reads the waypoint and `leg_results[0]` only |
| same | Rust | `checkpoint_count`, `whole_hike_legs`, `legs_outside` `not defined` | the array length, the bad element counted | §14.4 (line 1254) `checkpoint_count` is the number of checkpoints; §14.6 `L_g = len(stages[g].checkpoints)` |
| same | Go | `N`, `fixed_start.N`, `legs_outside` `not defined` (counted only stages whose legs were defined) | numbers | Rule A4.7 (lines 295, 1479): `N` is "an integer when bookends exist"; line 1254: `legs_outside` is `not defined` only when the bookend fields are |
| `cpw-s3-lc2`, `lrw-cpw-s4`: a bad width and a wrong `leg_count` | Rust | `leg_count` dropped when a width was bad | listed | §14.1: "a condition that is violated is not dropped because another condition is also violated" |
| `absent-s5-*`, `text-s5-*`, `absent-s0-*`, `absent-top-public_key`, `absent-top-seed_beacon`, `absent-top-stages`, `text-top-stages`, `short-pubkey`, `short-s5-waypoint`, `mixed-2`, `stages-0`: an absent or mistyped `checkpoints`, `leg_results`, `waypoint`, `path`, `public_key`, `seed_beacon` or `stages` | Rust | the no-object record (`["field set"]`, nothing measured) | every violated condition listed, and the later measurements recorded | §14.1: "Later measurements are still recorded"; "a condition that tests a field's value is violated when that field is absent" (the rc.1 section 14.1 fix covered the other fields) |
| `absent-s17-path`, `text-s17-path`: stage 17's `path` absent or not bytes | Rust | no `W_17 path` | `["field set", "W_17 path"]` | Rule A4.1 (line 1262ff): an absent stage-17 path is `W_17 path` |
| `stages-19`: 19 stages | Rust | the six-leg draw indexed stage 18 too | stages 0 … 17 only | §14.6: `leg_total = start_STAGES` |
| `stages-0`, `absent-top-stages`: no stage 0 | Rust | `seed_value` `not defined` | `mismatch` | Rule A4.1: `stages[0].waypoint` is compared to SHA-256(seed): match, else mismatch |
| `short-s8-beacon`, `short-li`: a 31-byte stored hash | Go | `stored_hash` written as 62 hex digits | `not defined` | §14.1 line 1262: a value with no usable value from the object is `not defined` |
| `text-s5-checkpoints`, `text-s5-leg_results`: the value is not an array | Go, Java | no `field set` | `field set` | §14.1 row 2: wrong value types |
| `cpw-prev7-s3`, `cp-prev-text-s3`: the six-leg previous checkpoint is under 8 bytes, or text | Java | crashed (exit 1, stack trace, no record) | `mismatch` | §14.6 / line 1252: each result is `match` or `mismatch`; nothing recomputes from a short previous |
| `cp-prev-nowp-s3`: leg `j ≥ 1` drawn on a stage with no waypoint | Java | `mismatch` | `match` | §14.6: for `j ≥ 1` the previous is the stored `checkpoints[j − 1]`; the waypoint is not read |

`cpw-prev-s3` (a 31-byte previous checkpoint, leg `j = 1` drawn by seed `01…01`) agrees in all three: `match`, from the first 8 bytes (§11.2 `S_0 = u64_le(previous[0:8])`).

### Behaviour changes approved for rc.2 (closing spec questions 1 and 2 of rc.1)

| Change | Rust | Go | Java | Cases |
|---|---|---|---|---|
| 1. A top level that is not a map is recorded as `["malformed cbor"]`, the same record as bytes that do not decode | was `["field set"]`; now the undecoded record | the library wrote `["malformed cbor"]` on a partly measured skeleton; now the undecoded record | `Measure.record` returned `null`; now the undecoded record | `cbor-uint`, `cbor-array`, `cbor-text`, `nm-*` |
| 2. The programs exit 1 only when no top-level map decodes; every other input writes a record and exits 0 | `01` exited 0 with a record; now exit 1 | unchanged | unchanged | `nm-*`, `kat-trailing`, `map-then-map`, `map-then-01`, `trail-*` |
| 3. `decoded_hike` returns `Err` on trailing bytes | was `Ok` | n/a | n/a | Rust test `decoded_hike_refuses_trailing_bytes` |
| 4. `Cbor.Problem.getMessage()` returns `.issue` | n/a | n/a | was `null` | Java `V2Pack.rc2Break` |

Changes 1 and 2 also change SPEC wording (§14.1 row 1 and §18 item 9). That wording, with a vector, is drafted for the next spec tag (v2.0.6, after the SPEC_PIN tag); see [CHANGES.md](CHANGES.md). Until that tag exists the vectors are SDK cross-check cases.

## Fixed after rc.1 (before rc.2)

**Rust did not record every section 14.1 condition, and stopped measuring.** `hike/src/object.rs` used `?` on a required field, so an absent or mistyped `kind`, `fill`, size constant, stage key set, stage `beacon`, `beacon_height` or `leg_count` refused the whole object; the header-order loop stopped at the first failed pair; and the six-leg draw indexed legs by `leg_count`. Cases: `kat-flip@2`, `kat-head@2`, `kat-flip@562`, `kat-head@562`, `kat-flip@3938`, `field-slot-text`, `field-s0-beacon`, `field-s5-beacon`, `field-s1-height`, `field-s3-legcount`, `field-s3-legcount-2`, `field-li-equal`, `field-order-hash`. Go and Java followed the text; Rust now does too.

## Questions for the spec owner

The SDKs do not decide these. Where all three already agree, the cross-check holds them to that agreement; each answer lands as a vector in a `spec` PR, and then every SDK follows.

- **Q-a.** Resolved for rc.3. A `last_interrupt_height` above 2^53 − 1 lists `header height order` with `height_range`. The compared integer is exact. See [Fixed for rc.3](#fixed-for-rc3).
- **Q-b. A `stages` element that is not a map.** Is it a stage map with every field absent (Rust, Java: counts `0`, path `skip`), or an absent stage (Go: `not defined`)? §14.1 row 2 lists `field set` either way (pinned: `stage*-uint`).
- **Q-c. An absent `checkpoints` array: `checkpoint_count` `0` or `not defined`?** All three write `0`. Line 909 asks for a "usable count" for `whole_hike_legs`; line 1197 says a shape mismatch that leaves no usable value is `not defined`.
- **Q-d. A leg whose input is absent** (the stage's `waypoint`, or `public_key` / `seed_beacon` for the terrain): all three write `mismatch` (line 1252 allows only `match` or `mismatch`), where line 1197 could read as `not defined`.
- **Q-e. `stages` empty or absent.** All three add `checkpoint width`, `leg result width` and `seed beacon` (an absent field violates a condition on its value). With one stage (`stages-1`) the missing stages add no width items, as for `test-3leg`. Is that asymmetry intended?
- **Q-f. Six-leg previous of the wrong width.** All three read bytes 0 … 7 of a 31-byte previous checkpoint (`cpw-prev-s3`: `match`), literally §11.2 and §14.6. Should a previous of the wrong width make that leg `mismatch` instead?
- **Q-g. An absent or mistyped stage `path` (stages 1 … 16):** all three write `path_state` `skip`. Should it be `not defined`? And `w17_path` for an absent or mistyped stage-17 path is `not fetched by design` in all three.
- **Q-h. Trailing bytes after a map that is not strictly usable.** All three write the undecoded record (PORTS-219 item 17, `a0 00`) when a field is absent or mistyped, a stage key set is not exact, a width is wrong, or `leg_count` differs from the arrays; a strictly usable map with trailing bytes is measured (`kat-trailing`). P7.7 says "Later measurements are still recorded" for malformed CBOR. Should a map with trailing bytes always be measured?
- **Q-i.** Resolved for rc.3. A checkpoint or leg-result element that is not bytes lists `field set` with the width condition. A byte string of the wrong width stays the width condition only. See [Fixed for rc.3](#fixed-for-rc3).

To reproduce, run `bash sdk/test.sh`, or run `python3 sdk/tools/crosscheck.py` with the binaries. Each divergent case prints its exit codes and all three digests.
