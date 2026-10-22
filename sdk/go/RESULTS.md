# RESULTS

SPEC questions met while building this SDK, and the reading implemented.
The SPEC wins where this note and the SPEC differ.

## SPEC questions

1. **Invalid UTF-8 in a `kind` text.** SPEC §7.3 (the decode-order paragraph)
   says text values are definite-length UTF-8 text strings, and it does not
   list invalid UTF-8 among the malformed values. ERRATUM-1 open point 4
   leaves it untouched. Reading implemented: a major-type-3 string whose
   bytes are not well-formed UTF-8 is not a text string under §7.3, so the
   input is not one definite-length data item. It is a malformed CBOR value:
   `shape_item` is exactly `["malformed cbor"]` and the CLI exits 0.

2. **An over-range stored height and the index read.** SPEC §14.1 (the
   `height_range` bullet, as E25 states it) treats an integer above
   `2^53 − 1` as absent for the other shape conditions. Whether Rule A4.2
   issues a GET for that height is only implied (ERRATUM-1 open point 1).
   Reading implemented: no GET. `stored_height` is `not defined`,
   `index_comparison` is `not read`, and `T` is `not read`.

3. **A map key whose text is well-formed but whose definite-length integer
   is not shortest.** SPEC §7.3 says a non-shortest unsigned integer
   decodes, and that map key order is not a shape mismatch. Reading
   implemented: the key decodes and is looked up by its text. Produce still
   emits the shortest form.

## Deviations from CASES.md

None. The fourteen candidate files use the key templates in CASES.md, and
the three objects use the fixed beacons, heights, and empty paths.

## Layout note (plan Q-GB3)

The module is `github.com/groundinglab/pcoc/sdk/go/v2`, package `pcoc`,
command `pcoc`. The scalar fold lives in `internal/fold`. The scripted-rig
producer lives in `produce`. The generator lives in `gen`, and the thirteen
negatives live in `gen/negatives`, which no release package imports.
