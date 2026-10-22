# Integrate a producer

For someone implementing produce from the specification. This page is explanatory. It is not normative. Where it disagrees with the specification, the specification wins.

Read section 7.3 for the byte encodings, then section 13 for the producer procedure. The pack is [spec/conformance/pack](../../spec/conformance/pack/). SPEC §16 says what that pack publishes. It does not publish a hike-object encoding. A producer conforms when its outputs equal the published pack vectors.

Only hash-bound values are evidence. A text disagreement with a vector is a defect to report. Do not pick a side.

A worked example is any of the three SDKs. Start at [sdk/README.md](../../sdk/README.md). Rust is the program that can regenerate the vectors. Go and Java check the bytes already published.

Do not call groundinglab to learn the construction. Do not read the services to learn the construction. The services add no Part A rule.
