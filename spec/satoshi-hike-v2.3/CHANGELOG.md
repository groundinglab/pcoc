# Satoshi Hike 2.3: changes from 2.2

**Informative.** Base: `spec/satoshi-hike-v2.2/SPEC.md` on main (sha256 `0300373e…7cf2`). That file already carries errata 1 and 2 (#79). The v2.2 files are not touched.

## What 2.3 is

2.3 is 2.2 with errata 1 and 2 folded in, clarification-only edits, and three decisions by Christian (D2, D5, D7). **No byte changes:** the same valid objects, pack, vectors, verify records and reason codes. Admission outcomes are the same under any policy file that sets every lever whose range does not list unset. D2 removes the lever defaults, so a policy file that relied on a 2.2 default is refused under 2.3. The Status line says so. gl-admit-v10 sets every such lever and loads unchanged.

- `KIND` stays `v2.2-pcoc-object`.
- `SEAL_DOMAIN`, `bari-hc-v2`, `pcoc-gate-signin-v1` and `/v1` are unchanged.
- §10–§13 are byte-identical to 2.2 after anchor stripping. From `## 10. Terrain` through the line before `## 14. Verify`, 2.2 hashes to `30deb162…6a50` and 2.3 to `4d7eb491…85d4`; the 2.3 span differs only by anchor lines and the blank lines around them. With anchor-only lines and blank lines removed, both spans hash to `80eb8317…a637`.

## Numbering

Section, rule (A/B/I/P/BR) and §19 F-label numbering is unchanged. The one exception is a new informative §17.5, added at the end of §17. Nothing is renumbered, so no old → new map is needed.

## Changes (each its own commit)

| # | Where | Change | Why |
|---|---|---|---|
| 1 | whole file | Plain copy of 2.2 SPEC.md. | So the diff is readable. |
| 2 | title, Status, §1, §2, §4 row, §15.4 policy encoding, §16 pack paragraph, Rule I5 | Version 2.2 → 2.3. The Status line says no byte changes and `KIND` is unchanged. The policy `spec` is `"2.3"`, and a `"2.2"` file is read by the 2.3 rules. The tag is `spec-2.3.0`. SDK tags are `sdk-<language>-v2.3.<patch>`. | Version identity. The `"2.2"` policy reading keeps existing policy files (gl-admit-v10) admissible unchanged. |
| 3 | §3 | BCP 14 boilerplate for the file's lowercase and capitalised `must` / `must not` / `may`. These words are requirements only in Normative text, and plain statements in Normative text are requirements too. | Without it, RFC 8174 would read lowercase "must" as non-normative. One informative line (§1, "must be measured under this file") restates §15.5 and is covered. |
| 4 | §15.2 table (two `received` rows), §15.2 B9 list, Rule B7 | "The profile sets a figure" → "the policy in force sets" the named cap. | Cap values live in the policy file. "Profile" collides with the verifier profiles `production` and `conformance-small`. |
| 4 | Rule I6 | "loads all 16 KiB" → "loads all `SLOT_BYTES` bytes of the selected slot". | Under `conformance-small`, `SLOT_BYTES` is not 16 KiB. §17.4 already says it this way. |
| 5 | §14 | A three-line Informative summary before the Normative text. | §14 is the longest normative section and had no overview. |
| 5 | new §17.5 (Informative) | Reconciles the §11.1 baseline class with the §15.5 low-end owned host. | ERRATUM-1 open point 6. Neither section's text changes. |
| 6 | whole file | Explicit anchors. No text changes. | Lets a rule be quoted and linked by a stable ID that does not depend on heading wording. |
| 7 | §7, §8, §9, §16 | One-to-three-sentence Informative summaries. | Major sections with no overview. §10–§13 get none, to keep their text as close to 2.2 as possible. §15 already opens with one. |
| 7 | P4.2, P10.1, P13.10 (§1 class table) | Quotability fixes from a 10-rule random check. P4.2 links "Part A" to §1. P10.1 gives the lane width ("lanes of 8 bytes each, `P[0]` … `P[3]`"). P13.10 says `leg_count` and `len(checkpoints)` are per stage. | Each read ambiguously when quoted alone. Meaning is unchanged; each restates §1, §10 or §8. |
| 8 | §7, §8, §9, §16 summaries, §17.5, P4.2, P13.10, intro, §14, §15.1, Rule B10, §18 | Second-review must-fixes 1–6 and nits N1–N5, in the reviewer's wording. §7 summary: "the order of the decode checks" (it does not sort). §17.5: rewritten (the 2.2 text did not say what the earlier draft claimed). §14 "(P14.1)", §15.1 challenge "might add", Rule B10 "can hit": the plain *may* sat under the new BCP 14 sentence. The two matching edits inside §10 and §13 (§10 "(section 16)" after "as a negative"; §13 "its answer may arrive" → "can arrive") are deferred to keep §10–§13 byte-identical after anchor stripping; see "Not in 2.3". Intro "(Rule A4.4.4)". §16 summary: matching the pack is part of conformance, not the whole of it. P4.2 and P13.10 wording, §18 "Profile judgment", and the §8 and §9 summary lists. N6 ("16 KB" in §11.1) is not applied: it is Christian's quote in frozen text. | Review findings. No byte, vector or outcome change. |
| 9 | Status, §1 overview, intro, glossary, §14, §15 intro, §15.4 (text, lever table, policy encoding), §15.5 | **D2.** The SPEC gives no lever a default. The lever table loses its Default column; names, units, ranges and comparison rules stay. A policy sets every lever. An absent key means unset only where the range lists unset; any other absent key refuses the whole file. An admission service with no policy file, or whose file is refused, admits nothing. It has no built-in lever values. §15.5 no longer states `1/5`. | Christian's rule: the SPEC holds levers and bounds, policy holds values. Outcome change only for a policy file that omitted a lever with a 2.2 default; the Status line says so. |
| 9 | §15.5 Measuring method | **D5.** Two sentences, no new number. Measure with the fastest conforming producer build available for the host, and record whether its fold ran vectorised or scalar and whether SHA-256 ran in hardware or software. A policy does not set the floor above what the released conforming SDKs reach on the low-end host. That constraint binds whoever sets or raises the floor; it is not a gate check. | Christian's decision. It records which SHA path ran and adds no second SHA requirement (D4 stays deferred). |
| 9 | Rule I5, §15.4 policy encoding | **D7.** This file's tag is `spec-2.3.0`. `sdk/SPEC_PIN` names it when the SDKs move to 2.3 (SDK tags `sdk-<language>-v2.3.<patch>`). An admission service pinned to `spec-2.3.0` reads a policy file whose `spec` is `"2.2"` or `"2.3"`. | Christian's decision. The gate code does not accept a 2.3 pin today, so a separate gate-code PR must land before any `spec-2.3.0` tag. This PR changes neither gate code nor `SPEC_PIN`. |

## Anchors

| Anchor | Placed | Count |
|---|---|---|
| `sec-<n>` (dots → hyphens), for example `#sec-11-1` | own line before each numbered heading | 59 |
| `rule-<id>`, for example `#rule-i6`, `#rule-br1`, `#rule-a4-1` | own line before each rule heading (A, B, BR, I) | 49 |
| `rule-<id>`, for example `#rule-p7-5`, `#rule-a4-2-2` | in the ID cell of a Part A class-table row whose ID has no heading | 108 |
| `class-<id>`, for example `#class-a2-2` | in the ID cell of a class-table row that shares its ID with a rule heading | 23 |
| `fixed-<f>`, for example `#fixed-f1` | in the §19 label cell | 16 |

All 255 IDs are unique. Inside §10–§13 the only additions are anchor lines and blank lines.

## Diff vs 2.2 SPEC.md

- With anchors: +423 / −191 lines (2201 → 2433). Of the −191, 169 are table rows: rows that gained an anchor in their first cell, and the §15.4 lever rows that lost the Default column.
- Text only, anchors stripped: +63 / −47 lines.

## Section map

| Section | 2.3 change |
|---|---|
| Title, Status | Version; no byte change; D2 outcome disclosure |
| Intro picture | Rule A4.4.4 reference; no `thorough_rounds` default |
| §1 | Version; no lever value or default; P4.2, P10.1, P13.10 |
| Glossary (leg-rate floor) | No default |
| §2, §4 row | Version |
| §3 | BCP 14 boilerplate |
| §7, §8, §9, §16 | Informative summaries |
| §10–§13 | Anchors only |
| §14 | Summary; "(P14.1)"; no "default policy" |
| §15 intro, §15.4 | D2; policy `spec` "2.2"/"2.3" (D7) |
| §15.1 | Challenge: "might add" |
| Rule B10 | "can hit" |
| §15.2, Rule B7 | Caps set by the policy in force |
| §15.5 | No default floor (D2); D5 sentences |
| §16.2, Rule I5, Rule I6 | Version and pin (D7); `SLOT_BYTES` |
| §17.5 | New, informative |
| §18 | "Profile judgment" |
| §19 | Unchanged (F-labels anchored only) |

## Not in 2.3

- Deferred: D1 (Q01, C-S12, one string per shape fault), D3 (Q03, two sibling permanent negatives), D4 (Q04b, demote the §11.1 SHA-NI requirement). Each changes records, the pack, or conformance.
- Dropped: D6 (C-S14 option B). It reopens a split erratum 1 closed.
- Deferred to keep §10–§13 byte-identical after anchor stripping: the §10 squeeze note's "may publish … as a negative" gains no "(section 16)" pointer, and §13's "its answer may arrive while later legs run" stays "may" (it means *might*, not a BCP 14 permission). Both are informative-reading clarifications with no outcome change.
- Also deferred: Q04a1 (SIMD sentence in §11.1), ERRATUM-1 open points 1 and 2. See the triage in the PR description.
