# Satoshi Hike 2.3 — Six Document Split

**Status.** Branch `spec/split-six-documents-2026-10-24`. Text relocation 2026-10-24 (boundaries only; no new object rules). Not merged. See [INTENT.md](./INTENT.md), [GLOSSARY.md](./GLOSSARY.md), [FRAMING-REVIEW.md](./FRAMING-REVIEW.md), [GB-REVIEW-BRIEF.md](./GB-REVIEW-BRIEF.md).

## Definitional pair

**Construction + Conformance** = specification + matching test specification. That pair is fundamental: object cost, encodings, and the two-way Bitcoin commitment bind, plus the yes/no rules and vectors that prove an object (and a tool) matches Construction.

Producer and Measurement are **process** documents constrained by that pair. Waypoint Publication is our **scoped** Bitcoin-anchor batching channel (Construction still owns the bind). Admission Process is **downstream** and value-free; policy instances (`gl-admit-*`) stay outside this pack.

**Language:** **valid** → **measured** → **admitted**. Do not use “verified” loosely.

## Documents

1. **Satoshi Hike Construction** — object, terrain, leg, stage/waypoint slots, encodings, two-way Bitcoin bind. Definitional.
2. **Producer Process** — live produce sequence that realises Construction (pointer to Waypoint Publication for Bari operator rules).
3. **Waypoint Publication Process** — scoped batching channel (BR1–BR4). Bind required in Construction; this is how we batch the anchor side.
4. **Admission Process** — generic consume measurement + policy → decide. Value-free. K tip-deadline marked HOLD (wrong job; Bari fetch window is the real include window).
5. **Conformance Process** — test specification paired with Construction. F16 (K-defence) flagged for revisit when K retires.
6. **Measurement Record Process** — full §14 measure procedure + verify-record shape + verify CLI. Fact-only.

## Dependencies

```
Construction  ↔  Conformance     ← definitional pair
       ↑                ↑
   Producer        Measurement     ← realise / read the object
       \                |
        Waypoint Publication       ← scoped anchor channel (batching)
                        |
                 Admission Process ← generic consumer
                        ↑
              policy instance (outside pack, e.g. gl-admit-*)
```

## Reader paths

- **Builder (producer implementer):** Construction + Conformance; Producer Process; Waypoint Publication if planting via our channel.
- **Measurer / verifier implementer:** Construction + Conformance; Measurement Record; Waypoint Publication only to interpret that channel’s evidence.
- **Gate / admission implementer:** Construction (shape), Measurement Record (inputs), Admission Process (generic consume/decide), then **your** policy instance outside this pack.
- **Conformance claimant:** Conformance + every document claimed (always Construction).

Bodies remain section lifts from monolith 2.3; this pass relocates homes and adds FRAMING/HOLD markers only. The original 2.3 specification remains source of truth until these documents are confirmed complete and merged.
