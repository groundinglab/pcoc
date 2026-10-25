# AI integration guide

This directory holds the detailed instructions a coding agent should follow when integrating PCOC. The webpage at groundinglab.io/apps/ai/ keeps only a short prompt that points here.

## What the agent must do

1. Identify the scope from the human: producer, verifier, admission gate, or a combination. If the scope is ambiguous, ask before editing.
2. Use a full clone of this repository (not a shallow clone). Read `spec/README.md`, `spec/GUIDE.md`, and `sdk/SPEC_PIN`.
3. Follow the role map below. Read only the documents required for the named side.
4. Prefer a released SDK for the language when one exists. Discover the current tag and quickstart from the clone and from https://groundinglab.io/technical/sdk/. Where an SDK and the specification disagree, follow the specification and report the conflict.
5. Keep production, measurement, and admission separate. Do not hard-code policy values. Do not invent fields, encodings, or rules the specification does not define.
6. Run the applicable tests and the conformance checks. Report the actual results.
7. Finish with a summary of changed files, checks run, and any remaining limitations. Flag changes that touch key material, signatures, path validation, or admission policy.

## Role map

- **Producer:** Construction, Conformance, Producer Process. Waypoint Publication only when using that channel. Measurement Record section 14.3 only for the path-body check.
- **Verifier:** Construction, Conformance, Measurement Record. Waypoint Publication only when interpreting evidence from that channel.
- **Admission gate:** the verifier documents plus Admission Process, `policy/README.md`, and the policy file the human names.

The specification and its matching conformance pack define interoperability. Check the tag and pinned pack using the repository's own records. Do not assume SDK and specification versions match. If the tagged specification and the published copy at https://groundinglab.io/technical/spec/ disagree, stop and report the disagreement.

## Boundaries

- Producing or measuring must not decide who is accepted, refused, or revoked.
- Signature checks belong to admission, not production or measurement.
- Policy thresholds belong in a human-editable policy file.
- Do not add a tip-versus-last-beacon submission deadline, `submission_deadline_blocks`, or a lever named K unless the pinned specification explicitly defines it.
- Bitcoin supplies the clock and timestamp. Bari is an optional waypoint-publication channel.

## What to show the human

- The files changed.
- The checks that were actually run and their results.
- Any conflict between an SDK and the specification.
- Any change that touches key material, signatures, path validation, or admission policy.
