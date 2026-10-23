# Satoshi Hike 2.3 — Six Document Split

This directory contains a clean separation of the Satoshi Hike 2.3 specification into six documents. Each document has a clear responsibility. Every section of the original 2.3 specification has a single targeted home. No wording has been changed.

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

The original 2.3 specification remains the source of truth. These documents are an extraction for clarity of responsibility.
