# SDK 2.0.0 release-candidate notes

## rc.1 (`sdk-v2.0.0-rc.1`, `sdk/go/v2.0.0-rc.1`, commit c9b160ea)

The SDKs moved from `reference/` to `sdk/`. All three now share one harness (`sdk/test.sh`), a version (`sdk/VERSION`) and a cross-check. The Rust code is `ref-rust-v2.0.6` plus two fixes:

- **Untrusted input:** `hike/src/cbor.rs` and `hike/src/pathbody.rs` now return a typed error (`truncated`) where they used to `unwrap` on short input. Bytes that used to panic now give a record.
- **nsec:** `nip06::DemoKey.nsec` is a private field with an `nsec()` accessor. `Debug` does not print the secret, and there is no `Display`.

### What the consumers get

- **Admission gate:** it was on `ref-rust-v2.0.6`, so it gets exactly 2.0.6 plus the two fixes. The gate's version stays 0.5.4. `--version` and `/v1/health` report `sdk=2.0.0-rc.1 (path)`.
- **Desktop (PCOC Hike for Linux):** it was on `ref-rust-v2.0.5`, so for the desktop **this is a behaviour step, not just a move**. The app version stays 0.1.4, and there is no new metainfo release entry. The desktop gets:
  - The 2.0.6 additions: the gate fast-phase functions (`screen_fast`, `verify_fast`), `canonical_object` and `decoded_hike`, a terrain-build counter, and comments marking the section 15.5 leg-rate floors as informative. All are additive: the desktop does not call the new functions, and the records it writes for a given object do not change.
  - The two fixes above. A malformed object that used to panic in the Rust SDK now gets a record.
  - The SDK comes by path from `sdk/rust`, not from a git tag. About shows the SDK version that was resolved.

## After rc.1 (on `sdk/phase-a`, not tagged)

- **Rust section 14.1 recording (behaviour change).** For an object with an absent or mistyped field, the Rust verify used to write the no-object record (`["field set"]` and nothing measured). It also left out conditions that a later header pair or the closing header violated. Now it lists every violated condition and goes on measuring, as section 14.1 requires. The six-leg draw counts checkpoints, not `leg_count`, as section 14.6 requires. Rust records now match Go and Java on every such input. See [KNOWN-DIVERGENCES.md](KNOWN-DIVERGENCES.md#fixed-after-rc1). Records for well-formed objects and for the conformance pack do not change. The impact by consumer:
  - **Gate:** `shape` is still `mismatch` for every affected object, so it still refuses. Only the logged measures change.
  - **Desktop:** checking a malformed object now shows the full condition list and the later measurements, not `field set` alone.
- **Cross-check:** targeted section 14.1 cases, and a library-level comparison through the `librecord` examples.

## Backlog

- **QA B2:** the informative `LEG_RATE_FLOOR_*` constants (`hike/src/consts.rs`) and `hike-net::judge_test_policy` are still in the SDKs. Section 15.5 makes these figures informative, and a policy sets the floor, so the code belongs with the policy library and the gate. The READMEs already say this. Removing the constants is an API change, so it is deferred to PLAN-v3.
- **Bad-width checkpoint:** a three-way divergence (`field-s2-cpwidth`), pinned in KNOWN-DIVERGENCES.md. Rust and Go need fixes.
- **Stale `reference/rust/nip06`** in the gate's `sign-in-origins.json` generator string. That file is a spec vector under `SPEC_PIN`, and `tests/signin_origins.rs` regenerates it byte for byte. So both change together in the next spec release.
- **`--chain-rpc` in Go and Java** is accepted and ignored. This predates the SDK move and is documented. **`/v1/health`** omits `served_origins`, which also predates the move.
