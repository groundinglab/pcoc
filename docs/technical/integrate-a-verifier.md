# Integrate a verifier

For someone implementing verify from the specification. This page is explanatory. It is not normative. Where it disagrees with the specification, the specification wins.

Read section 14 for the measurement record, and section 16 for conformance. The pack is [spec/conformance/pack](../../spec/conformance/pack/). SPEC §16 says the pack does not publish a section 14 record. A verifier conforms when its outputs equal the published pack vectors.

Only hash-bound values are evidence. A text disagreement with a vector is a defect to report. Do not pick a side. Admission policy is not part of this check.

A worked example is any of the three SDKs. Start at [sdk/README.md](../../sdk/README.md). The verify programs write the section 14 record. They do not apply section 15.

Do not call groundinglab to learn the construction. Do not read the services to learn the construction. The services add no Part A rule.
