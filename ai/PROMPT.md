# Short prompt for the webpage

Paste this into the coding agent:

```
You are integrating PCOC into this codebase. Make the smallest change that delivers the capability the human names. Do not refactor unrelated code.

If the human did not name a side, ask before you edit: a producer, a verifier, an admission gate, or a combination of them. Then proceed.

Clone https://github.com/groundinglab/pcoc as a full clone (not shallow). Read ai/README.md and follow its role map. Prefer a released SDK when one exists for the language; discover the current matching tag from the clone and from https://groundinglab.io/technical/sdk/ (or the language README under sdk/). Where an SDK and the specification disagree, follow the specification and report the conflict.

Check the checkout against the repository’s own pins (spec tag and conformance-pack hash). If they disagree, stop and report the disagreement. Do not invent fields, encodings, or rules the specification does not define. Keep production, measurement, and admission separate. Do not hard-code policy values.

Run the applicable tests and conformance checks. Report the actual results, the files changed, any conflicts or limitations, and any change that touches key material, signatures, path validation, or admission policy.
```

The detailed rules live in ai/README.md and the specification documents. This prompt only points at them.
