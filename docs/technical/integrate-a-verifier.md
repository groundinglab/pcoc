# Integrate a verifier

For someone implementing verify from the specification. This page is explanatory. It is not normative. Where it disagrees with the specification, the specification wins.

Read section 14 for the measurement record, and section 16 for conformance. The pack is [spec/conformance/pack](../../spec/conformance/pack/). SPEC §16 says the pack does not publish a section 14 record. A verifier conforms when its outputs equal the published pack vectors.

Only hash-bound values are evidence. A text disagreement with a vector is a defect to report. Do not pick a side. Admission policy is not part of this check.

A worked example is any of the three SDKs. Start at [sdk/README.md](../../sdk/README.md). The verify programs write the section 14 record. They do not apply section 15.

Do not call groundinglab to learn the construction. Do not read the services to learn the construction. The services add no Part A rule.

`before_born_height` is not a section 14 result. A SPEC verifier does not emit it. It is an admission-policy decision from the gate. The policy value is `born_height` (972685 in `gl-admit-v10`). It is not part of Satoshi Hike 2.3. A caller of the gate who receives `before_born_height` has a final answer: the object carries a stored or claimed height below the born height, so it is testing history. Do not retry it, and do not treat the answer as an index outage or a hold. The verify output names the `born_height` that applied and the lowest offending height. A median-time header below `born_height` is not that rejection. The index serves the `MTP_LOOKBACK` headers before `born_height` (`MTP_WINDOW` 11, lookback 10) for the timestamp read only. Those headers are not production heights.
