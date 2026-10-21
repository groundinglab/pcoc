# Conformance vectors

Shared test vectors for the PCOC specification. They live in this tree because they are part of the specification, not a side folder. The v2.1 target for the corrected seed, NL4 word, §11.1 fold, small hikes, and the rate and anchor comparisons is [v2.1](v2.1/). The [v2.0](v2.0/) pack is historical. It was generated under the withdrawn fill pin and the withdrawn v2.0 fold. A byte match to v2.0 is not v2.1 conformance. The spec tag line in `sdk/SPEC_PIN` names `spec-v2.1.0`. CoS cuts that tag on the merge commit. No cutover until every port passes this pack and the production-size vectors. The tag half of `sdk/tools/check_spec_pin.py` cannot pass until that tag exists.

Any producer or verifier that reproduces the v2.1 vectors agrees with the corrected construction. "Conforms" to v2.1 means those outputs byte for byte. Part B (§15) is policy. The floor in `v2.1/rate.json` is the placeholder `1/5`, marked not ratified. The leg-rate figures in `v2.0/constants.json` are the withdrawn examples. They are not a conformance target. `v2.0/sign-in.json` is the locked §15.1 wire. Matching it is not Part A conformance.

Where a vector and a sentence of the specification disagree, that is a specification defect. Open an issue. Do not pick a side.

Only hash-bound values are evidence. The portable index stands in for Bitcoin. Anyone can bypass it by checking Bitcoin directly.

## What an implementer reproduces

1. Read the specification, Part A, then run the pack in `v2.1/`. `spec/conformance/tools/check_v21.c` recomputes that pack. The historical pack in `v2.0/` is not the v2.1 target.
2. `v2.1/kat-small.json` pins the seed, the terrain digest, and the leg, checkpoint, and waypoint bytes at the small profile. It is not an object-equality target. The historical `v2.0/kat-small.json` is the old `object_hex` target. A byte match to that file is not v2.1 conformance.
3. The v2.1 pack does not publish a section 14 record. The historical v2.0 pack's `record` field is the withdrawn construction. A v2.1 verifier follows section 14 of the specification.
4. The Rust, Go, and Java references are the first tested examples. They are a guide. The specification wins if a reference disagrees.

The file table, the regenerate command, and the reason the short objects are short are in [v2.0/README.md](v2.0/README.md).

`blockhash-display.json` is the §9 display-order pin for a Bitcoin header hash. It is not part of the regenerated v2.0 pack. The vector hash in `reference/SPEC_PIN` covers it, because the hash walks every file under `spec/conformance/` except `spec/conformance/tools/`.

## Licence

Vector bytes in this tree are Apache-2.0: `v2.0/*.json`, `v2.0/earlier-kind.cbor`, and `blockhash-display.json`. Prose in this tree (`*.md`) is CC BY 4.0. That split is the rule recorded in LICENSE-DOCS.md.
