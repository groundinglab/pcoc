# Gap — vector-level verifier

`spec/conformance/v2.1/README.md` and specification line 2030 say this pack is not a full hike-object encoding. This reference implements the construction those vectors test. It does not decode a section 7.3 CBOR object, and it does not write the section 14 JSON verify record.

Implemented:

- Seed `SHA-256(public_key ‖ seed_beacon)`, four little-endian lanes, no label.
- NL4 terrain fill, once per terrain, including parents and `slot_mix`.
- §11.1 step and §11.2 leg. Every step mixes every word of the selected slot with the state.
- Terrain sample, checkpoint preimage, checkpoint fold, and §12.2 waypoint.
- A producer that walks a schedule and writes checkpoint records.
- A verifier that re-walks chosen legs, checks the chain, the checkpoint, and the waypoint, and applies Rule B2 and Rule B4 as separate measurements (`hike rate`, `hike anchors`).
- Kind check: `satoshi-hike-v2.1-object` is current. The two earlier kinds return `earlier_version` and do not fill terrain.
- Median-time-past (Rule A4.10) and `hike_span`. A span of 0 or below is `refuse`. A missing time is `held`.
- Submission deadline comparison (Rule B10). Placeholder `K = 3`. A missing or stale tip, or a tip below the last beacon, is `held`. A gap that is not greater than `K` is `not refused by this rule`. The function does not read `{index-url}` and does not remember a receipt.
- The section 15.5 thorough-round index draw from injected `u64` draws (`rounds.json`). A draw list that ends before 18 indices is `not read`. An unfinished group is not a round.

Not implemented, because the v2.1 files do not carry the inputs:

- Section 7.3 deterministic object encoding and CBOR decode.
- Section 14 record keys (`shape`, `headers`, `paths`, `anchors`, bookends, `N`, `S`, `six_leg`, and the rest of that list).
- A live operating-system CSPRNG read. `rounds.json` supplies the draw integers. Production verify would read the CSPRNG itself. This pack does not publish `draw_bytes_hex`.
- Hashcash, Bari posts, and live portable-index GETs. `mtp.json` is a numeric replay of header times.

`hike_deadline` returns `not refused by this rule`, `refuse`, or `held` for that one comparison. It is not the receipt clock. D1 (fresh `{index-url}` tip, bypassing the local mirror) and D2 (once per client submission) are gate behaviour this pack does not drive with an index.
