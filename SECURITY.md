# Security

Please report a vulnerability privately. Do not open a public issue.

Use GitHub's private reporting for `groundinglab/pcoc`: the **Security** tab, then **Report a vulnerability**. Include the affected path (the specification, a reference implementation, a service, or an app), the version or commit, and steps to reproduce.

We aim to acknowledge a report within seven days.

## Scope

- The Satoshi Hike specification and the conformance vectors.
- The Rust, C, Go, and Java reference programs.
- Bari, the index, the Admission Gate, the claims service, and the Claims Worker.
- The desktop app.

The Android app is built from a separate repository (`groundinglab/primal-pcoc`). Report issues there. Deployments outside this repository are out of scope for this file.

Test vectors and demo keys in this repository are public on purpose. They are not secrets and carry no value.

## Worth reporting

- An object that measures as honest for materially less producer work than the construction requires.
- A disagreement on a §14 Measurement Record (document 06) that the Rust, C, Go, and Java references do not agree on for the same vector.
- A sentence of the specification and a published vector that disagree. That is a specification defect under §16 Conformance (document 05). Report it. Do not pick a side.

## Out of scope

- The absence of a guarantee the specification does not claim.
- Cost at scale.
- Bitcoin itself.

There is no bounty. A report is a report.
