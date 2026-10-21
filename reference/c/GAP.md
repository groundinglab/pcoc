# Gap — vector-level verifier

`spec/conformance/v2.1/README.md` and specification line 2030 say this pack is not a full hike-object encoding. This reference implements the construction those vectors test. It does not decode a section 7.3 CBOR object, and it does not write the section 14 JSON verify record.

Implemented:

- Seed `SHA-256(public_key ‖ seed_beacon)`, four little-endian lanes, no label.
- NL4 terrain fill, once per terrain, including parents and `slot_mix`.
- §11.1 step and §11.2 leg. Every step mixes every word of the selected slot with the state.
- Terrain sample, checkpoint preimage, checkpoint fold, and §12.2 waypoint.
- A producer that walks a schedule and writes checkpoint records.
- A verifier that re-walks chosen legs, checks the chain, the checkpoint, and the waypoint, and applies Rule B2 and Rule B4 as separate measurements (`hike rate`, `hike anchors`).

Not implemented, because the v2.1 files do not carry the inputs:

- Section 7.3 deterministic object encoding and CBOR decode.
- Section 14 record keys (`shape`, `headers`, `paths`, `anchors`, bookends, `N`, `S`, `six_leg`, and the rest of that list).
- The section 14.6 six-index draw from the operating-system CSPRNG. The verifier re-walks the legs the caller names. With no `--leg`, it re-walks every stored leg. `kat-small` has 18 legs, so a draw could be defined, but the pack publishes no `draw_bytes_hex` and no expected six-leg record. Inventing that output is not a vector match.
- Hashcash, Bari posts, and portable-index reads.
- Admission outcomes. This program does not print `accept`, `refuse`, `revoke`, or `held`.

`SPEC-FINDINGS.md` records the §16 sentences that still describe the v2.0 object pack.
