# Satoshi Hike 2.3: changes from 2.2

**Informative.** Base: `spec/satoshi-hike-v2.2/SPEC.md` on main (sha256 `0300373e…7cf2`). That file already carries errata 1 and 2 (#79). The v2.2 files are not touched.

## What 2.3 is

2.3 is 2.2 with errata 1 and 2 folded in, plus clarification-only edits. **No byte changes:** the same valid objects, pack, vectors, verify records, reason codes and admission outcomes.

- `KIND` stays `v2.2-pcoc-object`.
- `SEAL_DOMAIN`, `bari-hc-v2`, `pcoc-gate-signin-v1` and `/v1` are unchanged.
- §10–§13 are byte-identical to 2.2 (sha256 of the range: `30deb162…6a50` in both).

## Numbering

Section, rule (A/B/I/P/BR) and §19 F-label numbering is unchanged. The one exception is a new informative §17.5, added at the end of §17. Nothing is renumbered, so no old → new map is needed.

## Changes (each its own commit)

| # | Where | Change | Why |
|---|---|---|---|
| 1 | whole file | Plain copy of 2.2 SPEC.md. | So the diff is readable. |
| 2 | title, Status, §1, §2, §4 row, §15.4 policy encoding, §16 pack paragraph, Rule I5 | Version 2.2 → 2.3. The Status line says no byte changes and `KIND` is unchanged. The policy `spec` is `"2.3"`, and a `"2.2"` file is read the same way. The tag is `spec-2.3.0`. SDK tags are `sdk-<language>-v2.3.<patch>`. | Version identity. The `"2.2"` policy reading keeps existing policy files (gl-admit-v10) admissible unchanged. |
| 3 | §3 | BCP 14 boilerplate for the file's lowercase and capitalised `must` / `must not` / `may`. These words are requirements only in Normative text, and plain statements in Normative text are requirements too. | Without it, RFC 8174 would read lowercase "must" as non-normative. One informative line (§1, "must be measured under this file") restates §15.5 and is covered. |
| 4 | §15.2 table (two `received` rows), §15.2 B9 list, Rule B7 | "The profile sets a figure" → "the policy in force sets" the named cap. | Cap values live in the policy file. "Profile" collides with the verifier profiles `production` and `conformance-small`. |
| 4 | Rule I6 | "loads all 16 KiB" → "loads all `SLOT_BYTES` bytes of the selected slot". | Under `conformance-small`, `SLOT_BYTES` is not 16 KiB. §17.4 already says it this way. |
| 5 | §14 | A three-line Informative summary before the Normative text. | §14 is the longest normative section and had no overview. |
| 5 | new §17.5 (Informative) | Reconciles the §11.1 baseline class with the §15.5 low-end owned host. | ERRATUM-1 open point 6. Neither section's text changes. |
| 6 | whole file | Explicit anchors. No text changes. | Lets a rule be quoted and linked by a stable ID that does not depend on heading wording. |
| 7 | §7, §8, §9, §16 | One-to-three-sentence Informative summaries. | Major sections with no overview. §10–§13 are left without one so their normative text stays byte-identical. §15 already opens with one. |
| 7 | P4.2, P10.1, P13.10 (§1 class table) | Quotability fixes from a 10-rule random check. P4.2 links "Part A" to §1. P10.1 gives the lane width ("lanes of 8 bytes each, `P[0]` … `P[3]`"). P13.10 says `leg_count` and `len(checkpoints)` are per stage. | Each read ambiguously when quoted alone. Meaning is unchanged; each restates §1, §10 or §8. |

## Anchors

| Anchor | Placed | Count |
|---|---|---|
| `sec-<n>` (dots → hyphens), for example `#sec-11-1` | own line before each numbered heading | 59 |
| `rule-<id>`, for example `#rule-i6`, `#rule-br1`, `#rule-a4-1` | own line before each rule heading (A, B, BR, I) | 49 |
| `rule-<id>`, for example `#rule-p7-5`, `#rule-a4-2-2` | in the ID cell of a Part A class-table row whose ID has no heading | 108 |
| `class-<id>`, for example `#class-a2-2` | in the ID cell of a class-table row that shares its ID with a rule heading | 23 |
| `fixed-<f>`, for example `#fixed-f1` | in the §19 label cell | 16 |

All 255 IDs are unique. Inside §10–§13 the only additions are anchor lines and blank lines. With those removed, the range hashes the same as 2.2.

## Diff vs 2.2 SPEC.md

- With anchors: +392 / −160 lines (2201 → 2433). Of the −160, 147 are table rows that gained an anchor in their first cell.
- Text only, anchors stripped: +32 / −18 lines.

## Not in 2.3 (normative; for Christian)

See the PR description: 2.3-QUEUE Q01–Q04 and the lever-default removal (C-S10).
