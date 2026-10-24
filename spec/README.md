# Satoshi Hike 2.3 — Specification pack

**Status.** Public specification for Satoshi Hike **2.3**, tag `spec-2.3.0`, standalone under `spec/`.

**Start here if you are building something:** [GUIDE.md](./GUIDE.md) — which documents to use for a producer, a verifier, an admission gate, or policy tuning.

Also: [INTENT.md](./INTENT.md) · [GLOSSARY.md](./GLOSSARY.md).

**Versions.** This pack is specification **2.3**. SDK packages are `2.3.x` and are not the specification version. The object kind wire label remains `v2.2-pcoc-object` (same bytes). The conformance manifest id `pack-2.2.0` is a historical name; the pinned bytes are 2.3-valid. See [conformance/pack/README.md](./conformance/pack/README.md).

## Definitional pair

**Construction + Conformance** = specification + matching test specification. That pair is fundamental: object cost, encodings, and the two-way Bitcoin commitment bind, plus the yes/no rules and vectors that prove an object (and a tool) matches Construction.

Producer and Measurement are **process** documents constrained by that pair. Waypoint Publication is our **scoped** Bitcoin-anchor batching channel (Construction still owns the bind). Admission Process is **downstream** and value-free; policy instances (`gl-admit-*`) stay outside this pack.

**Language:** **valid** → **measured** → **admitted**. Do not use “verified” loosely.

Anchored-path inclusion uses Bari’s anchor window (offsets 2, 3, and 4). This specification defines no tip-versus-last-beacon submit deadline.

## Documents

| # | File | Role |
|---|---|---|
| 01 | [Satoshi Hike Construction](./01-Satoshi_Hike_Construction.md) | Object, terrain, leg, stage/waypoint slots, encodings, two-way Bitcoin bind. Definitional. Sections 1–4, 6–12. |
| 02 | [Producer Process](./02-Producer_Process.md) | Live produce sequence that realises Construction. Section 13 (except 13.6). |
| 03 | [Waypoint Publication Process](./03-Waypoint_Publication_Process.md) | Scoped batching channel (BR1–BR4). Section 13.6. |
| 04 | [Admission Process](./04-Admission_Process.md) | Generic consume measurement + policy → decide. Value-free. Section 15. |
| 05 | [Conformance Process](./05-Conformance_Process.md) | Test specification paired with Construction. Sections 16, 17, 19. |
| 06 | [Measurement Record Process](./06-Measurement_Record_Process.md) | Full measure procedure + record shape + verify CLI. Fact-only. Sections 14 and 18. |

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

- **Builder (producer implementer):** Construction + Conformance; Producer Process; Waypoint Publication if planting via our channel; Measurement **§14.3 only** for the path-body / `validate_for` check (Producer §13.1 cites “section 14.3”).
- **Measurer / verifier implementer:** Construction + Conformance; Measurement Record (including §14.3); Waypoint Publication only to interpret that channel’s evidence. Record tokens, including `mismatch`. Do not admit or refuse, and do not treat validity as a reject gate before measuring.
- **Gate / admission implementer:** Construction (shape), Measurement Record (inputs), Admission Process (measurement record + policy → decide), then **your** policy instance outside this pack.
- **Policy tuner:** policy instance only — do not edit Admission Process to change thresholds.
- **Conformance claimant:** Conformance + every document claimed (always Construction).
