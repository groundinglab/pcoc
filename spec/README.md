# Satoshi Hike v2.0

The PCOC specification and its conformance vectors. Together they are the core deliverable of this project.

The point is interoperability. A PCOC made by one implementation must check the same way in every other. Everything else in this repository serves that. The references, the services, and the apps are built from this folder, not the other way round.

Anyone, or their AI agents, can build a producer, a verifier, a service, or an app from the specification and the vectors alone, with no groundinglab code, service, or account.

`reference/SPEC_PIN` names `spec-v2.0.3`, the current release (section 15.1 origin amendment and `sign-in-origins.json`). `spec-v2.0.2` is the release on the merge commit that moved the vectors under `spec/conformance/`. `spec-v2.0.1` is the previous release and still resolves at the old vector paths.

## What "conforms" means

Conformance is Part A only (§1–§14 and §16). A producer or a verifier conforms when it reproduces every Part A vector byte for byte and records the same measurement tokens.

Part A is the construction: the hike object, terrain, legs and leg results, stages, checkpoints, waypoints, the Bitcoin clock, how to produce (§13), and the verify record (§14). Part A has no pass, fail, accept, refuse, or revoke.

Part B (§15) is admission policy. It is groundinglab's example, with the locked leg-rate floors labelled: strict `7/20`, default `1/4`, lenient `1/8` (specification line that states the lock, and §15.5). Other relying parties may set their own policy on the same Part A measurements. Part B is not a conformance target. `spec/conformance/v2.0/sign-in.json` is the locked sign-in wire. It is not a section 14 record, and matching it is not Part A conformance.

The vectors are worked inputs and exact outputs. A text/vector disagreement is a defect in the specification (§16). Open an issue. Do not pick a side.

The Rust, Go, and Java implementations in [reference/](../reference/) are the first tested examples. They are a guide, not a requirement. They conform for the same reason another implementation would: they match the vectors. Where a reference disagrees with the specification, the specification wins and the reference has a bug. Each reference ships its own tests, including a run of the full vector pack, so it also shows how to wire the vectors into a test suite.

The intended use: give the specification and the vectors to your own developers, or your own AI agents, and have them implement PCOC production, verification, or interoperability in your system, service, or application. The references are an optional aid in that work. The vectors decide whether the result conforms.

Only hash-bound values are evidence. The portable index stands in for Bitcoin and can be bypassed by checking Bitcoin directly.

## What the file contains

| § | Title | Role |
|---|---|---|
| 1 | Status | Lock, changes from v1.35, rule classes |
| 2 | Scope | What the file defines |
| 3 | Interpretation | "must" / "must not", byte ranges, notation |
| 4 | Glossary | Terms, and the retired-wording table |
| 5 | Overview | The object before the machinery |
| 6 | Constants | `STEPS_PER_LEG` 2000000, `SAMPLE_BYTES` 1024, `SLOT_BYTES` 16384, `TERRAIN_BYTES` 268435456, `SEAL_DOMAIN` |
| 7 | Encodings | Byte encodings |
| 8 | Hike object | Normative object shape |
| 9 | Clock | Tip, seed beacon, interrupts |
| 10 | Terrain | Fill from key and seed |
| 11 | Leg | Steps, leg result |
| 12 | Stage and waypoint | Checkpoints, fold, waypoint |
| 13 | Produce | Producer procedure. Anchor offsets 2, 3, 4 |
| 14 | Verify | The verify record: keys, tokens, encoding |
| 15 | Admission service policy | **Part B.** Locked floors and `earlier_version`. Not a conformance target |
| 16 | Conformance | What "conforms" means, and the vectors |
| 17 | Closed | Settled items |
| 18 | Wording notes | Informative, except item 9 (verify CLI), which is locked |

This page is the map. It does not add a second construction. The file is [Satoshi-Hike-v2.0.md](satoshi-hike-v2.0/Satoshi-Hike-v2.0.md).

## How to build from it

1. Read Part A in order (§3 through §12, then §13 for a producer or §14 for a verifier).
2. Implement, then run every file in [spec/conformance/v2.0](conformance/v2.0/).
3. Compare verify records. `kat-small.json` is the producer byte-equality target. The `record` field is the §14 record.
4. Optionally compare with a reference: [reference/](../reference/). The references are not required.
5. The specification and the vectors change together, in one pull request. Implementations pin one spec tag through `reference/SPEC_PIN`. Part A changes need new vectors and a full CI run.

## Related, not normative here

`services/admission-gate/WIRE.md` is the Gate's decision-log contract. It is a companion to §15. It is not part of this folder and it is not a second construction.

[plant-attribution.md](plant-attribution.md) is a proposed note on plant identification. For the initial release, plants are identified by the 39-byte `BARI` tag only. Attributing a plant to an operator by a shared published key on the funding inputs is a planned, optional later step. Status: PROPOSED, not normative. It does not change Part A, the vectors, or the verifier.

## Licence

Prose in this folder, including this page: CC BY 4.0 ([LICENSE-DOCS.md](../LICENSE-DOCS.md)).
Vector bytes under `spec/conformance/`: Apache-2.0 ([LICENSE](../LICENSE)).
