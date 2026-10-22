# Site update for SDK 2.2.0 (generated 2026-10-23 ACDT from release-manifest.json a3ef7ffa455a347743d8c07fa69c5e701a3b8db20abbe1c225335e2b8857da87)

## Apply on a preview branch, then Christian reviews the preview

- [x] technical/files/sdk.md: status 2.2.0; four languages; install and quickstart from the manifest; pack link; no performance table (no timing run) (V22 #15)
- [x] technical/index.html: version 2.2.0; Rust, C, Go, and Java; quickstart lines in place of the old harness summary (V22 #16)
- [x] make/index.html: quickstart commands on `spec` via `pcoc quickstart` (the 2.2 pack, not `spec/conformance/v2.0`); expected six lines; tested on reference system 3 (V22 #17)
- [ ] build/index.html: left as the existing setup page. The manifest install steps are the quickstart commands above (V22 #17, partial)
- [x] technical/reference-{rust,go,java}: already retired on www/site-candidate (V22 #19)
- [x] technical/files/integrate-a-producer.md and integrate-a-verifier.md: pack links already point at the single pack on the candidate (V22 #20, #21)
- [x] technical/files/sign-in-wire.md: already a 2.2 section 15.1 page on the candidate (V22 #22)
- [x] policy page: erratum 2 named; `verify_cap_per_door_session` = 3 called out as a policy value the gate reads; evidence links stay on `interop/evidence-2.2`
- [ ] re-render technical pages after the markdown edit
- [ ] site checks

## Not this release (blocked on other tickets)

- admission-gate/ example output (#18): gate on SDK 2.2.0 is not deployed
- apps/, guides/ (#42–43): desktop and Android builds on SDK 2.2.0
- verify/ (#1–7): a 2.2 PCOC from reference system 1
- pcoc-at-scale, by-audience (#40–41): no timing run, so no new legs/s
- Bari paid-rule vectors and the sign-in wire are in the pack; manifest `pack_status` is `complete`
- Performance table: `performance` is an empty list
