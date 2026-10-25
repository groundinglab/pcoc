# Short prompt for the webpage

Paste this into the coding agent:

```
You are integrating PCOC into this codebase. Make the smallest change that adds the capability the human names.

Clone https://github.com/groundinglab/pcoc as a full clone. Read ai/README.md and follow its role map for the side the human named (producer, verifier, admission gate, or both). Prefer a released SDK when one exists for the language. Where an SDK and the specification disagree, follow the specification and report the conflict.

Keep production, measurement, and admission separate. Do not hard-code policy values. Do not invent fields or rules the specification does not define.

Run the applicable tests and conformance checks. Report the actual results, the files changed, and any conflicts or limitations. Flag changes that touch key material, signatures, path validation, or admission policy.
```

The detailed rules live in ai/README.md and the specification documents. This prompt only points at them.
