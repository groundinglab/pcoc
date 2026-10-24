# Developer guide — which documents to use

**Status.** Public guide for the Satoshi Hike **2.3** specification pack under `spec/` (tag `spec-2.3.0`). Matches [INTENT.md](./INTENT.md) and [GLOSSARY.md](./GLOSSARY.md). Pack overview: [README.md](./README.md).

Specification version **2.3** is not an SDK package version (`2.3.x`). The object kind wire label remains `v2.2-pcoc-object`.

**Language:** **valid** → **measured** → **admitted**. Do not say “verified” when you mean only one of those.

**Definitional pair:** **Construction + Conformance** = specification + matching test specification. Every build below starts from that pair.

Anchored-path inclusion uses Bari’s anchor window (offsets 2, 3, and 4). This specification defines no tip-versus-last-beacon submit deadline.

---

## Section numbers

Cites such as “section 14” use these homes:

| Sections | Document |
|---|---|
| 1–4, 6–12 | [Construction](./01-Satoshi_Hike_Construction.md) |
| 13, except 13.6 | [Producer Process](./02-Producer_Process.md) |
| 13.6 (BR1–BR4) | [Waypoint Publication](./03-Waypoint_Publication_Process.md) |
| 14, 18 | [Measurement Record](./06-Measurement_Record_Process.md) |
| 15 | [Admission Process](./04-Admission_Process.md) |
| 16, 17, 19 | [Conformance](./05-Conformance_Process.md) |

---

## If you want to…

### Produce valid PCOCs (build a producer)

| Need | Document |
|---|---|
| What the object is; two-way Bitcoin bind | [01 — Construction](./01-Satoshi_Hike_Construction.md) |
| Yes/no rules + vectors that prove validity | [05 — Conformance](./05-Conformance_Process.md) |
| Live produce sequence (tip → hike → plant/ask → close) | [02 — Producer Process](./02-Producer_Process.md) |
| Our batching / anchor channel (when planting via Bari-shaped path) | [03 — Waypoint Publication](./03-Waypoint_Publication_Process.md) |
| Path-body / `validate_for` check only (Producer §13.1 cites “section 14.3”) | [06 — Measurement](./06-Measurement_Record_Process.md) §14.3 only |

**Design time:** implement against Construction + Conformance until emitted objects pass the conformance pack. Use Producer for the live sequence. Use Waypoint Publication when planting through our scoped channel. Use Measurement §14.3 only for the canonical path-body layout. Do not copy that layout into Producer.

**Out of scope here, still allowed:** direct-to-Bitcoin plant/confirm/fetch with different timing. Construction’s bind still holds; that mechanism is not defined in this pack’s Waypoint Publication document.

### Build a verifier (measure)

| Need | Document |
|---|---|
| What the object is; Bitcoin bind | [01 — Construction](./01-Satoshi_Hike_Construction.md) |
| Validity rules + vectors | [05 — Conformance](./05-Conformance_Process.md) |
| Full measure procedure + record shape + verify CLI | [06 — Measurement Record](./06-Measurement_Record_Process.md) |
| Canonical path-body layout (the same §14.3 text Producer §13.1 checks) | [06 — Measurement](./06-Measurement_Record_Process.md) §14.3 |
| Interpreting our channel’s plant/path evidence (if the object used it) | [03 — Waypoint Publication](./03-Waypoint_Publication_Process.md) |

**Design time:** accept valids, reject invalids, report the same quantities the rules require.

**Runtime:** **valid → measured → admitted**. Measurement records tokens, including `mismatch`, and does not admit or refuse. “Confirm still valid, then measure” is one pass. It is not a separate reject gate before measuring. Cites of “section 15” inside Measurement are gate-owned (fast record and judgment). A verifier does not apply them.

### Build an admission gate

**Core:** a measurement record plus a policy instance → decide. The Gate/sign-in machine in Admission Process is the consume-then-decide procedure around that core. Lever values stay in the policy instance.

The hike pair the floor reads is `(hike_legs, hike_span)` from Measurement Rule A4.10: checkpoints over stages 1–17, and `nTime(I_17) − nTime(I_0)`. That span is raw Bitcoin header `nTime`: `header.time` of `last_interrupt` minus `header.time` of `stages[1].beacon`. It is the same integer as `whole_hike_span`. `I_0` is the first interrupt. `I_17` is `last_interrupt`. The seed beacon is not an endpoint, and stage 0 is not in `hike_legs`. Bookend `N` and `S` are not this pair. Median-time-past is not this span.

| Need | Document |
|---|---|
| Object shape (inherited) | [01 — Construction](./01-Satoshi_Hike_Construction.md) |
| Validity context | [05 — Conformance](./05-Conformance_Process.md) |
| Measurement record inputs | [06 — Measurement Record](./06-Measurement_Record_Process.md) |
| Generic consume measurement + policy → decide/log | [04 — Admission Process](./04-Admission_Process.md) |
| Thresholds and lever values | **Policy instance outside this pack** (e.g. `gl-admit-*`) |

Admission Process is **value-free**. Do not put groundinglab numbers into the SPEC process document.

### Tune admission (change what a gate accepts)

Edit or swap the **policy instance** only. Do **not** edit Admission Process to change floors, `min_anchored_paths`, or born height. Those belong in the policy file your gate loads.

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
| Producer author | Construction + Conformance + Producer (+ Waypoint if planting on our channel) + Measurement **§14.3 only** for the path-body check |
| Verifier author | Construction + Conformance + Measurement (+ Waypoint if interpreting that evidence). §14.3 is the path-body layout Producer §13.1 checks |
| Gate author | Construction + Conformance + Measurement + Admission Process + **your** policy instance |
| Policy tuner | Policy instance only |

---

## Framing inside this pack

- [INTENT.md](./INTENT.md) — locked frame for the six documents
- [GLOSSARY.md](./GLOSSARY.md) — terms and roles
