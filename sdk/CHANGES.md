# SDK changes: rc.1 → rc.2

rc.1 is the rc.1 tag pair at c9b160ea. rc.2 is `sdk-v2.0.0-rc.2` and `sdk/go/v2.0.0-rc.2`, version `2.0.0-rc.2` in [VERSION](VERSION). Neither is published.

> **For Christian: a spec change is drafted, not merged.** CoS-approved changes 1 and 2 below alter SPEC wording (§14.1 row 1 and §18 item 9, which is marked *Locked 2026-10-07*). That wording and a new vector (`spec/conformance/v2.0/top-level.json`) are in **one separate commit on the local branch `spec/v2.0.6-draft`** (also `patches/spec-v2.0.6-draft.patch` in the bundle), **not on `sdk/phase-a` and not in rc.2**. Reason: `sdk/SPEC_PIN` pins the hash of `spec/conformance` *and* checks it against the tree of its spec tag (the current tag, v2.0.5). A new vector changes the hash, so the pin cannot be updated honestly until a new spec tag exists. The draft commit sets SPEC_PIN to the next tag, **v2.0.6** (repository tag `spec-` + `v2.0.6`), with the new hash; `check_spec_pin.py` fails on that branch until that tag is created on the draft commit. Until then, rc.2 carries the same inputs as SDK cross-check cases (`cbor-*`, `nm-*`, `kat-trailing`, `map-then-*`, `trail-*`), and all three SDKs already behave as the draft says. No file under `spec/` changed on `sdk/phase-a`.

## Behaviour changes (CoS-approved)

1. **A top level that is not a map is recorded as `["malformed cbor"]`** in all three SDKs, libraries and programs: the same record as bytes that do not decode. Before: Rust wrote a `["field set"]` record, Go's library a partly measured `["malformed cbor"]` record, Java's `Measure.record` returned `null`.
2. **The verify programs exit 1 only when no top-level map decodes** (the file cannot be read, the first CBOR item does not decode under section 7.3, or it is not a map). Every other input, trailing bytes after a map included, writes the record and exits 0. Before: Rust exited 0 with a record for a non-map item.
3. **Rust `decoded_hike` returns `Err` on trailing bytes.** New: `hike::top_level_map(bytes)`.
4. **Java `Cbor.Problem.getMessage()` returns the `.issue` text** (it was `null`).

## Fixes (the SPEC text is clear)

The full table, with the SPEC line for each row, is in [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md#fixed-for-rc2).

- **Wrong checkpoint width (`field-s2-cpwidth`, pinned in rc.1).** Rust: `W_{g+1}` is now `not defined` (it was `mismatch`), the stage's `j = 0` leg is recomputed from the waypoint and `leg_results[0]` (it was `mismatch`), and the counts are the array length (they were `not defined`). Go: `N`, `fixed_start.N` and `legs_outside` count the stage (they were `not defined`). Java was already right.
- **Rust measures on through every absent or mistyped field.** rc.1 fixed this for the constants and the stage header fields; rc.2 does it for `checkpoints`, `leg_results`, `waypoint`, `path`, `public_key`, `seed_beacon`, `stages` and a stage that is not a map. Rust also keeps `leg_count` when a width is bad, records `W_17 path` for an absent or mistyped stage-17 path, draws only over stages 0 … 17 when there are 19, and writes `seed_value` `mismatch` when there is no stage 0.
- **Go:** a 31-byte stored hash is `not defined` (it was written as 62 hex digits); a `checkpoints` or `leg_results` value that is not an array is `field set`.
- **Java:** a `checkpoints` or `leg_results` value that is not an array is `field set`; a six-leg previous checkpoint under 8 bytes or of text no longer crashes the program (it is `mismatch`); a drawn leg `j ≥ 1` no longer needs the stage's waypoint.
- Records for well-formed objects and for every conformance-pack vector are unchanged.

## Policy values out of the SDKs (QA B2)

The spec defines measurements; a policy sets the values. Removed from the SDKs:

- Rust: `hike::LEG_RATE_FLOOR_STRICT`, `LEG_RATE_FLOOR_DEFAULT`, `LEG_RATE_FLOOR_LENIENT`; `hike_net::judge_test_policy`, `GateJudgment`, `GateSkip`, `TestGateMeasurement`.
- Go: `LegRateFloorP`, `LegRateFloorQ`, `LegRateFloor`, `LegRateFloorStrict`, `LegRateFloorDefault`, `LegRateFloorLenient`.
- Java: `Hike.LEG_RATE_FLOOR_*`.

The section 15.5 example pairs (7/20, 1/4, 1/8) stay in `spec/conformance/v2.0/constants.json` (unchanged) and are test fixtures in each SDK's tests and in the Rust pack generator. The test-plane judgment is now a fixture in `services/admission-gate/tests/test_plane_policy.rs`. The admission door never read any of these: its floor is its policy file. Proof: for gl-admit-v4 … v7, the policy `bind_key`, the `/v1/policy` document (`policy::public_document`) and the fast-phase decision for each of the 155 cross-check inputs were dumped before and after the move and are byte-identical, and also identical to the rc.1 SDK. Bari takes `hike-net` from the git tag `ref-rust-v2.0.5`, so this change does not reach it; when Bari moves to this SDK it should take the fixture with it (backlog).

These removals are API changes. A caller that used the constants should read its floor from its own policy.

## Cross-check

`sdk/tools/crosscheck.py` now also runs the structural break cases in `sdk/tools/hikecases.py` (CBOR-value edits of `kat-small`): 157 inputs, 163 runs × 3 languages (135 write a record), and the same 163 runs × 3 at library level (`librecord`). All agree except 5 pinned rows, the open questions Q-a and Q-b in KNOWN-DIVERGENCES.md. The new code was mutation-checked: 28 mutants over the Rust, Go and Java changes, 27 killed by the unit tests or the cross-check; the survivor (Java's byte-string check on a six-leg previous) is equivalent, because a Java text value carries no bytes.

## Upgrading from rc.1

- Rust: stop using the removed constants and `judge_test_policy`. `decoded_hike` now refuses trailing bytes; use `verify` for the record.
- Go: stop using the removed `LegRateFloor*` names. `Verify` on a non-map top level returns `decoded == false` with the undecoded record (as before) but no longer a partly measured one.
- Java: `Measure.record` never returns `null` for a non-map top level; `Problem.getMessage()` is the issue text.
- Gate and desktop take the SDK by path. Their records for well-formed objects do not change. For malformed objects, the logged measures change as above; `shape` stays `mismatch`, so the gate still refuses them.
