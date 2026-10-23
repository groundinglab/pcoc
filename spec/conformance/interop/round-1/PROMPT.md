Build an independent implementation of Satoshi Hike SPEC 2.2 in {LANGUAGE}, working ONLY from the public specification:

- SPEC: https://raw.githubusercontent.com/groundinglab/pcoc/2b50df71afc842080bee77c6a3efd64d55603d7f/spec/satoshi-hike-v2.2/SPEC.md (commit 2b50df7, sha256 3055ef488bf761c198894a13b433c3f2d67a26c9ad73e937abfa4b1619e66319)
- Policy: https://raw.githubusercontent.com/groundinglab/pcoc/2b50df71afc842080bee77c6a3efd64d55603d7f/policy/groundinglab-admission/POLICY.md and .../gl-admit-v9.toml

Why: this build is part of a public interoperability test. We want to show that anyone can point an AI agent at the SPEC, build directly from it at low effort, and get an implementation that is byte-for-byte interoperable with independent implementations in other languages built by other models. Your result and your SPEC questions are published as evidence.

Rules:
- Do NOT read, fetch or copy any other implementation, repository, branch or code about PCOC or Satoshi Hike (including anything else in groundinglab/pcoc). Only the two URLs above, plus the attached uploads/CASES.md.
- Every hike step reads and folds the FULL slot mixed with the state. No cache, table, partial read or shortcut of any kind (SPEC section 11.1, Rule I6, section 19). Correctness over speed.
- Where the SPEC is unclear, don't guess silently: list each question with section and line and say which reading you implemented.

Build (in this repo, under impl/):
1. The construction (SPEC sections 6-12): seed, terrain, hike, fold, checkpoints, waypoints.
2. Object encode and decode (sections 7-8), with the v2.2-pcoc-object kind; any other label is an unknown kind.
3. The section 14 verifier, including the section 16.1 conformance-small profile.
4. A candidate generator that writes every case in uploads/CASES.md, in exactly the format it specifies, into candidates/ (instead of sdk/<lang>/candidates/). Negatives live only in a generator module named negatives.
5. A small offline test-plane producer that builds full objects from the CASES fixtures (synthetic beacons, empty path bytes, posts nothing) and writes O1 (profile S1, pk_A), O2 (profile S2, pk_A) and O3 (production profile, pk_A, one leg per stage) to objects/, plus a CLI `verify <object>` that prints the section 14 verifier record.
6. Tests that re-run the generator and check the committed candidates are unchanged.

Write RESULTS.md with: the model and effort setting you ran as (if known), what you built, the exact commands, the build and run timings (including legs per second for O3), every SPEC question, and anything you could not finish. Keep it simple and finish; commit everything to the branch.
