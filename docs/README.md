# groundinglab docs

Plain-language background for PCOC and the Satoshi Hike construction. These pages teach; they do not amend the specification. The normative text is the [Satoshi Hike v2.0 SPEC](https://github.com/groundinglab/pcoc/tree/spec-v2.0.1/spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md) at tag `spec-v2.0.1`; the reference implementation is at [`ref-rust-v2.0.3`](https://github.com/groundinglab/pcoc/tree/ref-rust-v2.0.3/reference/rust).

## Briefs: the questions

Short essays, one question each. Start with [the briefs index](briefs/README.md).

## Rationale: the argument

1. [Foundations](rationale/foundations.md): the whole picture on one page.
2. [Family contract](rationale/family-contract.md): what any construction owes.
3. [How you make one](rationale/how-you-make-one.md): terrain, legs, leg results and checkpoints.
4. [Threat model](rationale/threat-model.md): the two ways it can fail.
5. [Cost and incidence](rationale/cost-and-incidence.md): who pays, and when.

## Architecture

- [Two-plane substrate](architecture/two-plane-substrate.md): the test and live planes as one table.

Verification measures the object. Whether to accept it is a separate policy decision.
