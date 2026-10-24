# GB review brief — six-doc boundary relocation

**Branch.** `spec/split-six-documents-2026-10-24` on `groundinglab/pcoc`  
Reviewed on #119.  
**Date.** 2026-10-24 (ACDT).

## What we did

We relocated existing monolith-lift wording into the six document homes so the pack matches the locked framing. We did **not** invent new Construction rules, new vectors, or new thresholds. Prefer cut-paste; framing notes mark intent.

Read first: [INTENT.md](../../../spec/INTENT.md), [GLOSSARY.md](../../../spec/GLOSSARY.md), [README.md](../../../spec/README.md).

## Intent (frame GB must interpret)

1. **Construction + Conformance** = definitional pair (spec + test spec). Construction / Bitcoin bind is fundamental.
2. **Producer** and **Measurement** = process docs constrained by that pair.
3. **Waypoint Publication** = scoped Bitcoin-anchor **batching** channel. Construction owns the two-way bind (required for a well-formed PCOC; constrains produce and measure). Direct bind is out of scope here but allowed as a different mechanism.
4. **Admission Process** = downstream, value-free consume-then-decide. Policy instances (`gl-admit-*`) outside the pack.
5. Language: **valid → measured → admitted**. No loose “verified.”
6. **Tip-versus-last-beacon submit deadline** is not part of the specification. Bari’s anchor window is the include constraint. A later commit on this branch removed it from the public pack. No replacement deadline.

## What to check

| Check | Pass if |
|---|---|
| No broken dependencies | Cross-doc pointers resolve; Producer → Waypoint for BR; Construction → Measurement for §14; BR4’s “section 14.3” noted as now in Measurement. |
| No lost Construction rules | Object/terrain/leg/stage/waypoint/encodings/bind still in Construction; rule-index still lists A* IDs; §14 body intact in Measurement (not deleted). |
| Bitcoin bind still clear | Construction owns both directions; Waypoint Publication FRAMING states scoped batching + bind required. |
| Produce/measure constrained | Producer and Measurement headers state constraint by Construction + Conformance; Measurement is fact-only. |
| Admission value-free | Header states value-free; lever *values* → policy; Gate/sign-in flagged ops-shaped. |
| Tip deadline | Removed from Admission and from Conformance. No replacement deadline and no new vectors. |
| Conformance pair | Still the test-spec half. The old F16 row is gone. |
| No new rules | Diff is relocation, framing notes, and the deadline deletion — not new INVARIANTs or thresholds. |

## Deadline

- Removed from `04-Admission_Process.md` and `05-Conformance_Process.md`.
- No replacement submit deadline.
- Ops note on Gate/sign-in in Admission (body preserved).

## Out of scope for this review

Merging to main. Writing `gl-admit-*`. Retiring K for real. Rewriting rule wording. Inventing new conformance vectors.
