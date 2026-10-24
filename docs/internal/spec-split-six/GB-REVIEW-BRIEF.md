# GB review brief — six-doc boundary relocation

**Branch.** `spec/split-six-documents-2026-10-24` on `groundinglab/pcoc`  
**Not merged.** Review only.  
**Date.** 2026-10-24 (ACDT).

## What we did

We relocated existing monolith-lift wording into the six document homes so the pack matches the locked framing. We did **not** invent new Construction rules, new vectors, or new thresholds. Prefer cut-paste; FRAMING/HOLD notes mark intent.

Read first: [INTENT.md](../../../spec/INTENT.md), [GLOSSARY.md](../../../spec/GLOSSARY.md), [README.md](../../../spec/README.md).

## Intent (frame GB must interpret)

1. **Construction + Conformance** = definitional pair (spec + test spec). Construction / Bitcoin bind is fundamental.
2. **Producer** and **Measurement** = process docs constrained by that pair.
3. **Waypoint Publication** = scoped Bitcoin-anchor **batching** channel. Construction owns the two-way bind (required for a well-formed PCOC; constrains produce and measure). Direct bind is out of scope here but allowed as a different mechanism.
4. **Admission Process** = downstream, value-free consume-then-decide. Policy instances (`gl-admit-*`) outside the pack.
5. Language: **valid → measured → admitted**. No loose “verified.”
6. **K tip submit-deadline** is the wrong job. Marked HOLD / to retire pending Christian. **Bari fetch window is the real include window.** Do not treat a silent SPEC-without-K as the outcome of this pass.

## What to check

| Check | Pass if |
|---|---|
| No broken dependencies | Cross-doc pointers resolve; Producer → Waypoint for BR; Construction → Measurement for §14; BR4’s “section 14.3” noted as now in Measurement. |
| No lost Construction rules | Object/terrain/leg/stage/waypoint/encodings/bind still in Construction; rule-index still lists A* IDs; §14 body intact in Measurement (not deleted). |
| Bitcoin bind still clear | Construction owns both directions; Waypoint Publication FRAMING states scoped batching + bind required. |
| Produce/measure constrained | Producer and Measurement headers state constraint by Construction + Conformance; Measurement is fact-only. |
| Admission value-free | Header states value-free; lever *values* → policy; Gate/sign-in flagged ops-shaped. |
| K marked, not silently deleted | Rule B10 body preserved under HOLD; K lever row marked HOLD; F16 flagged revisit — no invented replacement vectors. |
| Conformance pair | Still the test-spec half; F16 HOLD note present. |
| No new rules | Diff is relocation + FRAMING/HOLD/README/INTENT/glossary sync — not new INVARIANTs or thresholds. |

## HOLD markers (expected)

- `04-Admission_Process.md` — Rule B10 block + `submission_deadline_blocks` (K) row.
- `05-Conformance_Process.md` — F16 revisit when K retires.
- Ops note on Gate/sign-in in Admission (body preserved).

## Out of scope for this review

Merging to main. Writing `gl-admit-*`. Retiring K for real. Rewriting rule wording. Inventing new conformance vectors.
