# Conformance vectors

Shared test vectors for the PCOC specification. They live in this tree because they are part of the specification, not a side folder. Current pack: [v2.0](v2.0/). The spec tag that covers this tree is named in [spec/README.md](../README.md) and in `reference/SPEC_PIN`.

Any producer or verifier that reproduces the Part A vectors agrees with the specification. "Conforms" means Part A only: every Part A output byte for byte, and the same section 14 measurement tokens. Part B (§15) is policy. The leg-rate figures in `v2.0/constants.json` are informative §15.5 examples. They are not a conformance target. `v2.0/sign-in.json` is the locked §15.1 wire. Matching it is not Part A conformance.

Where a vector and a sentence of the specification disagree, that is a specification defect. Open an issue. Do not pick a side.

Only hash-bound values are evidence. The portable index stands in for Bitcoin. Anyone can bypass it by checking Bitcoin directly.

## What an implementer reproduces

1. Read the specification, Part A, then run the pack in `v2.0/`.
2. `kat-small.json` is the producer byte-equality target: a section 13 replay equals `object_hex`.
3. Each verify vector's `record` is the section 14 record, keys in the order section 14 lists, no insignificant whitespace, one trailing newline inside `record`.
4. The Rust, Go, and Java references are the first tested examples. They are a guide. The specification wins if a reference disagrees.

The file table, the regenerate command, and the reason the short objects are short are in [v2.0/README.md](v2.0/README.md).

`blockhash-display.json` is the §9 display-order pin for a Bitcoin header hash. It is not part of the regenerated v2.0 pack. The vector hash in `reference/SPEC_PIN` covers it, because the hash walks every file under `spec/conformance/` except `spec/conformance/tools/`.

## Licence

Vector bytes in this tree are Apache-2.0: `v2.0/*.json`, `v2.0/earlier-kind.cbor`, and `blockhash-display.json`. Prose in this tree (`*.md`) is CC BY 4.0. That split is the rule recorded in LICENSE-DOCS.md.
