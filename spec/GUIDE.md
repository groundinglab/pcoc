# Developer guide — which documents to use

**Status.** Developer guidance for the standalone pack under `spec/` on branch `spec/split-six-documents-2026-10-24`. Not merged. Matches [INTENT.md](./INTENT.md) and [GLOSSARY.md](./GLOSSARY.md). Pack overview: [README.md](./README.md).

This directory is a **framing and boundary** split of Satoshi Hike 2.3. It does not invent new object rules. Implementations that already match 2.3 **need not change** because of this restructure alone. The tip submit-deadline (**K** / Rule B10) remains **HOLD** (wrong job; Bari fetch window is the real include window) — preserved, not silently removed.

**Language:** **valid** → **measured** → **admitted**. Do not say “verified” when you mean only one of those.

**Definitional pair:** **Construction + Conformance** = specification + matching test specification. Every build below starts from that pair.

SDKs and services elsewhere in this repository should follow the same reader paths.

---

## If you want to…

### Produce valid PCOCs (build a producer)

| Need | Document |
|---|---|
| What the object is; two-way Bitcoin bind | [01 — Construction](./01-Satoshi_Hike_Construction.md) |
| Yes/no rules + vectors that prove validity | [05 — Conformance](./05-Conformance_Process.md) |
| Live produce sequence (tip → hike → plant/ask → close) | [02 — Producer Process](./02-Producer_Process.md) |
| Our batching / anchor channel (when planting via Bari-shaped path) | [03 — Waypoint Publication](./03-Waypoint_Publication_Process.md) |

**Design time:** implement against Construction + Conformance until emitted objects pass the conformance pack. Use Producer for the live sequence. Use Waypoint Publication when planting through our scoped channel.

**Note.** Canonical path-body layout used when checking a fetched path at produce time lives in Measurement §14.3 (Producer cites “section 14.3”; see Producer FRAMING stub). Do not copy that layout into Producer.

**Out of scope here, still allowed:** direct-to-Bitcoin plant/confirm/fetch with different timing. Construction’s bind still holds; that mechanism is not defined in this pack’s Waypoint Publication document.

### Build a verifier (measure)

| Need | Document |
|---|---|
| What the object is; Bitcoin bind | [01 — Construction](./01-Satoshi_Hike_Construction.md) |
| Validity rules + vectors | [05 — Conformance](./05-Conformance_Process.md) |
| Full measure procedure + record shape + verify CLI | [06 — Measurement Record](./06-Measurement_Record_Process.md) |
| Interpreting our channel’s plant/path evidence (if the object used it) | [03 — Waypoint Publication](./03-Waypoint_Publication_Process.md) |

**Design time:** accept valids, reject invalids, report the same quantities the rules require.

**Runtime:** confirm the object is still **valid** (same Conformance rules), then **measure** → emit a measurement record. No thresholds. No admit/refuse.

### Build an admission gate

| Need | Document |
|---|---|
| Object shape (inherited) | [01 — Construction](./01-Satoshi_Hike_Construction.md) |
| Validity context | [05 — Conformance](./05-Conformance_Process.md) |
| Measurement record inputs | [06 — Measurement Record](./06-Measurement_Record_Process.md) |
| Generic consume measurement + policy → decide/log | [04 — Admission Process](./04-Admission_Process.md) |
| Thresholds and lever values | **Policy instance outside this pack** (e.g. `gl-admit-*`) |

Admission Process is **value-free**. Do not put groundinglab numbers into the SPEC process document.

### Tune admission (change what a gate accepts)

Edit or swap the **policy instance** only. Do **not** edit Admission Process to change floors, `min_anchored_paths`, born height, or whether any tip deadline exists. Those belong in the policy file your gate loads.

### Claim conformance for an implementation

Read [05 — Conformance](./05-Conformance_Process.md) plus every document you claim (always Construction; plus Producer and/or Measurement and/or Waypoint Publication as claimed). Admission policy values are **not** a conformance claim.

---

## Short map

```
Construction  ↔  Conformance     ← definitional pair (always)
       ↑                ↑
   Producer        Measurement     ← realise / read
       \                |
        Waypoint Publication       ← scoped anchor channel (batching)
                        |
                 Admission Process ← generic consumer
                        ↑
              policy instance (outside pack)
```

| Role | Documents |
|---|---|
| Producer author | Construction + Conformance + Producer (+ Waypoint if planting on our channel) |
| Verifier author | Construction + Conformance + Measurement (+ Waypoint if interpreting that evidence) |
| Gate author | Construction + Conformance + Measurement + Admission Process + **your** policy instance |
| Policy tuner | Policy instance only |

---

## Related framing files (reviewers, not build inputs)

- [INTENT.md](./INTENT.md) — relocation intent and locked frame
- [GLOSSARY.md](./GLOSSARY.md) — terms and roles
- [`docs/internal/spec-split-six/FRAMING-REVIEW.md`](../docs/internal/spec-split-six/FRAMING-REVIEW.md) — drift notes (reviewers)
- [`docs/internal/spec-split-six/GB-REVIEW-BRIEF.md`](../docs/internal/spec-split-six/GB-REVIEW-BRIEF.md) — GB review brief for #119


On this branch, this directory is the standalone public face. Implementations already shipping against tag `spec-2.3.0` need not change because of doc moves alone; K remains HOLD.
