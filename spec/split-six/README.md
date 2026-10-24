# Satoshi Hike 2.3 — Six Document Split

Framing vocabulary (locked 2026-10-24): see [GLOSSARY.md](./GLOSSARY.md) and the drift notes in [FRAMING-REVIEW.md](./FRAMING-REVIEW.md). SPEC body text in 01–06 is unchanged in that framing pass.

These six documents separate the Satoshi Hike 2.3 specification by responsibility. Each document inherits its content verbatim from the listed sections of `spec/satoshi-hike-v2.3/SPEC.md`. No wording has been changed.

## Documents

1. **Satoshi Hike Construction** — object, terrain, leg, stage and waypoint binding, encodings, and measurement procedure.
2. **Producer Process** — reference live sequence for a producer.
3. **Waypoint Publication Process** — tree-batching process for waypoints.
4. **Admission Process** — process an admission gate follows after measurement.
5. **Conformance Process** — process for claiming pack-level conformance.
6. **Measurement Record Process** — fixed serialisation of the measurement record and tooling.

## Dependencies

All documents depend on Satoshi Hike Construction.

## Roles

- A producer implementer uses Construction (and Producer Process and/or Waypoint Publication Process if those purposes are required).
- A verifier implementer uses Construction (and Waypoint Publication Process and/or Measurement Record Process if those purposes are required).
- An admission-gate implementer uses Construction and Admission Process.
- Conformance Process is used by any implementer who wants to claim pack-level conformance.

The original 2.3 specification remains the source of truth until these documents are confirmed complete.
