# Security

Please report a vulnerability privately. Do not open a public issue.

Use GitHub's private reporting for `groundinglab/pcoc`: the **Security** tab, then **Report a vulnerability**. Include the affected path (the specification, a reference, a service, or an app), the version or commit, and steps to reproduce.

We aim to acknowledge a report within seven days.

## Scope

- The Satoshi Hike specification and the conformance vectors.
- The Rust, Go, and Java reference programs.
- Bari, the index, and the Admission Gate.
- The desktop app.

Test vectors and demo keys in this repository are public on purpose. They are not secrets and carry no value.

## Worth reporting

- An object that measures as honest for materially less producer work than the construction requires.
- A section 14 record that the Rust, Go, and Java references do not agree on for the same vector.
- A sentence of the specification and a published vector that disagree. That is a specification defect (section 16). Report it. Do not pick a side.

## Out of scope

- The absence of a guarantee the specification does not claim.
- Cost at scale.
- Bitcoin itself.

There is no bounty. A report is a report.
