# Known divergences between the SDKs

`sdk/test.sh` runs the Rust, Go and Java verify programs on the same inputs (`sdk/tools/crosscheck.py`). On every conformance-pack object, all three write byte-identical section 14 records. On some **malformed** objects, Rust disagrees with Go and Java. Go and Java agree with each other in every case.

These divergences already existed before the SDK move: the reference Rust build at `565add5` (main, gate 0.5.4) writes the same bytes. The move does not fix them, because tonight's push allows no behaviour changes. The harness lists them by case name. If a listed case starts to agree, or a new case disagrees, the harness fails. The list can only shrink.

| Case (kat-small object, mutated) | What differs |
|---|---|
| `kat-flip@2`, `kat-head@2` (top-level `fill` key renamed) | Rust: `shape_item` `["field set"]`, and every later measurement is `not defined` or `not read`. Go/Java: `["field set", "constant"]`, and they go on to measure headers, checkpoints, waypoints, paths and the six-leg draw. |
| `kat-flip@562`, `kat-head@562` (a key in a stage map renamed) | Rust: `["field set"]` and stops measuring. Go/Java: `["field set", "header height order"]` and go on measuring. |
| `kat-flip@3938` (one bit in a stored header height) | Rust: `["last_interrupt"]`. Go/Java: `["last_interrupt", "header height order"]`. |

To reproduce: `bash sdk/test.sh`, or run `python3 sdk/tools/crosscheck.py` with the three binaries. Each case prints both record digests.

**Open question for the spec owner.** Section 14.1 (the `shape_item` table, and the rule that a present map with the wrong keys adds `field set`) has to decide which reading is right: does a `field set` mismatch still let the other Rule A3 conditions and measurements run? The statement in section 14.1 that a missing `last_interrupt` gives exactly `["field set", "last_interrupt", "header height order", …]` suggests that items keep accumulating, which points towards Go/Java. The SDKs do not decide this. The spec does, followed by a fix with a new vector in a `spec` PR.
