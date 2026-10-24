# Glossary and roles — Satoshi Hike / PCOC split-six

**Status.** Framing vocabulary for the six-document pack. Locked 2026-10-24 with Christian. Boundary relocation the same day moved §14 into Measurement and marked K HOLD — see [INTENT.md](./INTENT.md). See also [FRAMING-REVIEW.md](./FRAMING-REVIEW.md) and [README.md](./README.md).

**Language rule.** Prefer **valid** → **measured** → **admitted**. Do not say “verified” when you mean only one of those three.

---

## Core outcomes

| Term | Meaning | Who decides |
|---|---|---|
| **Valid** | The object matches Construction; Conformance rules and vectors agree. Yes/no. | Conformance (design-time for tools; runtime check before measure) |
| **Measured** | Quantities reported for a valid object: beacons folded, legs, anchored paths, times, etc. Facts only. No thresholds. | Measurement (verifier at runtime) |
| **Admitted** | A **policy instance** applied to a measurement record accepts (or refuses/holds) the submission. | Admission Process + policy instance |

---

## Documents and concepts

### Construction

The **definition** of a PCOC (Satoshi Hike object): cost, terrain, legs, stages, waypoint slots, encodings, and the **two-way Bitcoin commitment bind**.

- Into the hike: Bitcoin block headers (beacons) observed and folded into stages.
- Out of the hike: waypoint anchors that commit hike history into Bitcoin.

Construction does **not** set admission thresholds, does **not** own the §14 measure procedure (that is Measurement), and does **not** own live Bari operator steps (Waypoint Publication).

### Conformance

The **test specification** paired with Construction (specification + matching tests). Yes/no rules, permanent negatives, and the one conformance pack/vectors.

- **Design time:** producer authors prove outputs are valid; verifier authors prove they accept valids, reject invalids, and report the required measurements.
- **Runtime:** a verifier applies the same validity rules before reporting quantities.
- An **admission service is not a conformance target.** Policy numbers are not vectors.

Construction + Conformance travel as the **definitional pair**.

### Producer / Producer Process

A **producer** is software that builds a hike object. **Producer Process** is the live sequence that realises Construction (tip observation, stages, async plant/ask, close). Design-time proof that a producer is correct uses Construction + Conformance.

### Verifier

Software that **measures** a finished object: confirm it is **valid** (Conformance), then emit a **measurement record** of quantities. It does not admit. It does not say accept/refuse.

### Waypoint Publication (Process)

Our **scoped** open channel for the anchor side of the Bitcoin bind: Hashcash payment → pool → batch/tree on beacon → confirm → path fetch window (Bari-shaped).

- Required for security is the **Construction** bind, not this particular operator.
- This document scopes **batching** as the path we specify.
- **Direct-to-Bitcoin** plant/confirm/fetch with different timing is allowed as a different mechanism; it is out of scope for this process document.

Bari **fetchability** (path served or not in the anchor window) is the real include window for anchored paths. A tip-vs-last-beacon **submit deadline (K)** is not that window.

### Measurement / Measurement Record

**Measurement** reads a valid object against Construction (and publication evidence when the Bari-shaped channel was used) and reports facts. This pack’s Measurement Record document now holds the full §14 procedure plus the record shape and verify CLI. No judgment, no policy levers.

### Admission Process vs policy instance

| | Admission Process (in pack) | Policy instance (outside pack) |
|---|---|---|
| Role | Generic consumer: take measurement record + policy → decide/log | Named values for one relying party |
| Contents | How to read fields, apply a policy file, outcomes vocabulary | e.g. `gl-admit-v*` — floors, `min_anchored_paths`, born height, whether any tip deadline exists |
| Values | None (value-free) | All thresholds |

groundinglab Gate **service** docs and runbooks are neither the Process nor the instance; they are ops.

### Bitcoin bind (both directions)

Always owned by **Construction**. Observing headers into the object and anchoring waypoints into Bitcoin are both part of what the object *is*. Waypoint Publication is how *we* batch the anchor direction efficiently and openly.

---

## Roles (reader paths)

### Builder (producer implementer)

Read **Construction + Conformance**. Implement **Producer Process**. If planting via our channel, also **Waypoint Publication**. Prove with the conformance pack that emitted objects are **valid**.

### Measurer / verifier implementer

Read **Construction + Conformance**. Implement measurement against Construction; emit the **Measurement Record**. Use **Waypoint Publication** only to interpret evidence produced through that channel (do not re-fetch Bari for plant facts in the specified verify path). Prove with the pack that the verifier accepts valids, rejects invalids, and reports the same quantities.

### Gate / admission implementer

Read **Construction** (object shape), **Measurement Record** (inputs), and **Admission Process** (generic consume/decide). Plug in **your** policy instance (for groundinglab, `gl-admit-*` outside this pack). Do not treat lever values in a deployment as part of Construction.

### Conformance claimant

Read **Conformance** plus every document you claim to implement (always Construction; Producer and/or Measurement and/or Waypoint Publication as claimed). Admission policy values are not a conformance claim.

---

## Design time vs runtime (short)

| Phase | Producer | Verifier / measurer | Gate |
|---|---|---|---|
| Design time | Build against Construction + Conformance until outputs pass the pack | Build against Construction + Conformance until behaviour matches vectors | Build against Admission Process; configure a policy instance |
| Runtime | Emit a PCOC (trusted valid via design-time conformance) | Validate (Conformance rules) then **measure** → record | Apply policy instance → **admit** or not |

---

## Terms to avoid (loose use)

- **Verified** — say **valid**, **measured**, or **admitted** as appropriate.
- **Optional anchors** — anchors are part of the Construction bind; only the *batching channel* is a scoped mechanism.
- **SPEC deadline (K)** — do not treat tip-vs-last-beacon as an object or measurement rule; see framing review.
