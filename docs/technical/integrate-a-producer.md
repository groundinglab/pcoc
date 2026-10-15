# Integrate a producer

For someone implementing produce from the specification. This page is explanatory. It is not normative. Where it disagrees with the specification, the specification wins.

Read section 7.3 for the byte encodings, then section 13 for the producer procedure. The byte-equality target is `kat-small.json` under [spec/conformance/v2.0](../../spec/conformance/v2.0/). A producer conforms when its object matches that vector and the other Part A vectors, byte for byte.

Only hash-bound values are evidence. A text disagreement with a vector is a defect to report. Do not pick a side.

A worked example is any of the three references. Start at [reference/README.md](../../reference/README.md). Rust is the program that can regenerate the vectors. Go and Java check the bytes already published.

Do not call groundinglab to learn the construction. Do not read the services to learn the construction. The services add no Part A rule.
