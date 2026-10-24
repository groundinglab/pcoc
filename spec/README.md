# Satoshi Hike 2.3 — Specification pack

**Status.** Branch `spec/split-six-documents-2026-10-24`. Standalone pack layout (2026-10-24): six process/definition documents plus guidance under `spec/`. Text relocation and framing only; **no new object rules**. **Not merged.**

**Start here if you are building something:** [GUIDE.md](./GUIDE.md) — which documents to use for a producer, a verifier, an admission gate, or policy tuning.

Also: [INTENT.md](./INTENT.md) · [GLOSSARY.md](./GLOSSARY.md). Process notes for reviewers live under [`docs/internal/spec-split-six/`](../docs/internal/spec-split-six/) (not build inputs).

## Definitional pair

**Construction + Conformance** = specification + matching test specification. That pair is fundamental: object cost, encodings, and the two-way Bitcoin commitment bind, plus the yes/no rules and vectors that prove an object (and a tool) matches Construction.

Producer and Measurement are **process** documents constrained by that pair. Waypoint Publication is our **scoped** Bitcoin-anchor batching channel (Construction still owns the bind). Admission Process is **downstream** and value-free; policy instances (`gl-admit-*`) stay outside this pack.

**Language:** **valid** → **measured** → **admitted**. Do not use “verified” loosely.

## Documents

| # | File | Role |
|---|---|---|
| 01 | [Satoshi Hike Construction](./01-Satoshi_Hike_Construction.md) | Object, terrain, leg, stage/waypoint slots, encodings, two-way Bitcoin bind. Definitional. |
| 02 | [Producer Process](./02-Producer_Process.md) | Live produce sequence that realises Construction. |
| 03 | [Waypoint Publication Process](./03-Waypoint_Publication_Process.md) | Scoped batching channel (BR1–BR4). |
| 04 | [Admission Process](./04-Admission_Process.md) | Generic consume measurement + policy → decide. Value-free. K HOLD. |
| 05 | [Conformance Process](./05-Conformance_Process.md) | Test specification paired with Construction. |
| 06 | [Measurement Record Process](./06-Measurement_Record_Process.md) | Full measure procedure + record shape + verify CLI. Fact-only. |

**Vectors.** The one conformance pack of pinned bytes is [conformance/pack/](./conformance/pack/). Doc 05 defines how that pack is used.

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

Full developer instructions: **[GUIDE.md](./GUIDE.md)**.

- **Builder (producer implementer):** Construction + Conformance; Producer Process; Waypoint Publication if planting via our channel. (Path-body layout for produce-time path checks: Measurement §14.3 — see Producer FRAMING stub.)
- **Measurer / verifier implementer:** Construction + Conformance; Measurement Record; Waypoint Publication only to interpret that channel’s evidence.
- **Gate / admission implementer:** Construction (shape), Measurement Record (inputs), Admission Process (generic consume/decide), then **your** policy instance outside this pack.
- **Policy tuner:** policy instance only — do not edit Admission Process to change thresholds.
- **Conformance claimant:** Conformance + every document claimed (always Construction).

SDKs and services elsewhere should follow the same paths. This restructure alone does not require implementation changes; K remains HOLD.

Bodies remain section lifts from monolith 2.3 (tag `spec-2.3.0`); this pack relocates homes and adds FRAMING/HOLD markers only. On this branch, **`spec/` is the public face** for readers and ratification agents — do not look for a competing monolith tree here.
