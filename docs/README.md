# Docs

Plain-language briefs and rationale for why PCOC is built this way. Explanatory, not normative. Where a page and the specification disagree, the specification wins. CC BY 4.0.

New here: start with [Where does the trouble start?](briefs/where-does-the-trouble-start.md), then [Foundations](rationale/foundations.md).

To run the pieces for your own site, see [Run it under your own name](run-under-your-own-name.md).

The normative pack is [spec/](../spec/) — start with [GUIDE.md](../spec/GUIDE.md). Vectors are [spec/conformance/pack](../spec/conformance/pack/). The tested SDKs are [sdk/](../sdk/).

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

The two-plane note is for someone running a test plane and a live plane side by side. It assumes those four parameters. It is not required reading for the specification.

## Technical

- [Integrate a producer](technical/integrate-a-producer.md)
- [Integrate a verifier](technical/integrate-a-verifier.md)

Verification measures the object. Whether to accept it is a separate policy decision.
