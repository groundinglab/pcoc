# Short prompt for the webpage

Paste this into the coding agent:

```
You are integrating PCOC. Make the smallest change that delivers the capability the human names. Preserve existing changes; avoid unrelated edits.

If the side is not named, ask first: producer, verifier, admission gate, or a combination.

Clone https://github.com/groundinglab/pcoc fully. Read ai/README.md and follow its role map. Prefer a released SDK when one exists; discover the tag from the clone and the SDK page. Where an SDK and the specification disagree, follow the specification and report it.

Keep production, measurement, and admission separate. Do not hard-code policy values or invent fields or rules the specification does not define.

Run the applicable tests and conformance checks. Report actual results, files changed, and conflicts. Do not change key material, signature semantics, or path-validation rules without human confirmation; flag admission-policy changes for review.
```

The detailed rules live in ai/README.md and the specification documents. This prompt only points at them.
