Build an independent, well-optimised implementation of Satoshi Hike SPEC 2.2 (with erratum 1) in {LANGUAGE}, working ONLY from these public files, all pinned to one commit:

- SPEC: https://raw.githubusercontent.com/groundinglab/pcoc/7b40353ed68cbdc7319f06d09f6ed178e7f7809e/spec/satoshi-hike-v2.2/SPEC.md (sha256 8e591ea02e7c73ac3d1a4e61602f25baaae88cabdc482240d310d2629dfa5889)
- Policy: https://raw.githubusercontent.com/groundinglab/pcoc/7b40353ed68cbdc7319f06d09f6ed178e7f7809e/policy/groundinglab-admission/POLICY.md and https://raw.githubusercontent.com/groundinglab/pcoc/7b40353ed68cbdc7319f06d09f6ed178e7f7809e/policy/groundinglab-admission/gl-admit-v9.toml
- Fixtures and exact output keys: https://raw.githubusercontent.com/groundinglab/pcoc/8c40cf855241d6fbfdb508a2a2c1727342df0755/spec/conformance/interop/round2/CASES.md (save a copy in this repo)

Why: this is a public interoperability test of the SPEC. Several models from different providers each build independently in a different language. We publish every build, its prompt, model and effort, and a cross-verification matrix as evidence that the SPEC alone is enough to build interoperable implementations. Your RESULTS.md is part of that evidence.

Rules:
- Do NOT read, fetch or copy any other implementation, repository, branch or code about PCOC or Satoshi Hike. Use only the URLs above.
- Implement the SPEC exactly. Every output must be byte-identical to what the SPEC defines; interoperability is the point.
- Where the SPEC is unclear, don't guess silently: record each question with its section and line, the readings you considered and the one you implemented.

Optimisation (please take this seriously):
- Optimise the producer and verifier as far as your language allows, within the SPEC. SIMD, vectorisation, compiler flags, memory layout and better SHA-256 are all welcome, as long as the bytes are identical.
- The SPEC's cost is real work: every hike step loads every word of the full slot and mixes it with the state, because that is the only way to compute the correct result. An optimisation that keeps that work but does it faster is good.
- If you find any way to produce correct, verifying outputs while doing LESS work than the SPEC intends (skipping or caching slot reads, reusing work across steps or legs, partial reads, precomputation, or a verifier that can't tell), that is a flaw in the SPEC. Do not hide it and do not ship it as an optimisation: describe it prominently at the top of RESULTS.md under "Possible SPEC flaw", with how it works and what it saves.
- Report whether your language gave an advantage or a disadvantage, and why.

Build (under impl/):
1. The construction (SPEC sections 6–12).
2. Object encode and decode (sections 7–8).
3. The section 14 verifier, including the section 16.1 conformance-small profile, reading the fixture index the CASES file defines.
4. A deterministic candidate generator that writes every case in CASES.md with exactly its keys, into candidates/. Negatives live only in a generator module named negatives.
5. An offline test-plane producer that writes O1 (S1), O2 (S2) and O3 (production profile, one leg per stage) to objects/, plus a CLI `verify <object>` that prints the section 14 verifier record.
6. Tests that re-run the generator and check that the committed candidates are unchanged.

RESULTS.md (commit it; it is required): the model and effort you ran as, if known; what you built; the exact commands; the machine (CPU model and flags such as avx2, avx512 and sha_ni); timings for the terrain fill and for one production leg, both your optimised version and a plain scalar one if you made both; every optimisation and what it gained; any language advantage; every SPEC question; any possible SPEC flaw; and anything you could not finish.

Commit everything, including RESULTS.md, to main. In your final message, give the repo's Origin owner/name and the HEAD commit.
