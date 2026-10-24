# Intent — Satoshi Hike 2.3 six-document pack

Locked frame for the public pack under `spec/` (tag `spec-2.3.0`). No new object rules beyond Satoshi Hike 2.3.

## Locked frame

- **Construction + Conformance** = definitional pair (spec + test spec). Construction and the Bitcoin bind are fundamental.
- **Producer** and **Measurement** are process docs constrained by Construction + Conformance.
- **Waypoint Publication** = our scoped Bitcoin-anchor channel (batching). Construction owns the two-way bind; the bind is required for a well-formed PCOC and constrains produce/measure. Direct-to-Bitcoin remains an out-of-scope similar mechanism.
- **Admission Process** is downstream and value-free; policy instances (`gl-admit-*`) stay outside the pack. Gate/sign-in service detail is ops-shaped. The core is: measurement record + policy → decide.
- **Language:** valid → measured → admitted. No loose “verified.” A verifier records tokens, including mismatch, and does not admit or refuse. Validity is not a reject gate that skips measurement.
- **Include window:** Bari’s anchor-window offsets 2, 3, and 4. This specification defines no tip-versus-last-beacon submit deadline.

## Where the text lives

1. Construction: object + Bitcoin bind. §14 measure lives in Measurement.
2. Producer: live sequence. Bari BR1–BR4 live in Waypoint Publication. Path-body layout for the produce-time check is Measurement §14.3 only.
3. Waypoint Publication: batching / BR / fetch.
4. Measurement Record: full §14 procedure + record shape + §18 CLI. Fact-only.
5. Admission Process: value-free consume-then-decide. Lever values belong in a policy instance.
6. Conformance: test-spec half of the pair with Construction.
