# Round 2

- **SPEC:** Satoshi Hike 2.2 with erratum 1 at groundinglab/pcoc `7b40353`, `spec/satoshi-hike-v2.2/SPEC.md`, sha256 `8e591ea02e7c73ac3d1a4e61602f25baaae88cabdc482240d310d2629dfa5889`.
- **Policy given to builders:** `policy/groundinglab-admission/POLICY.md` and `gl-admit-v9.toml` at the same commit.
- **Fixtures:** [CASES.md](CASES.md), identical to `spec/conformance/interop/round2/CASES.md` at pcoc `8c40cf8` (sha256 `0e6de698e468138ba6503b55a876cee94a24cc84bdc649adfa5af643db5a5290`). No expected values.
- **Prompt:** [PROMPT.md](PROMPT.md), sent inline; only `{LANGUAGE}` differed.
- **Date:** 22 Oct 2026, launched about 16:20 ACDT. Reference-machine runs the same evening.
- **Effort:** high. Builders were asked to optimise within the SPEC, report any language advantage, and flag any less-work shortcut as a possible SPEC flaw.

| Build | Provider | Model | Effort | Language | Commit |
|---|---|---|---|---|---|
| [A-sonnet-ts](A-sonnet-ts/) | Anthropic | Claude Sonnet 5.5 | high | TypeScript / Node 22 + WebAssembly | `b3d7b02` |
| [B-sol-py](B-sol-py/) | OpenAI | GPT-5.6 Sol | high (see its MODEL.md) | Python 3 + C extension | `b18a5f6` |
| [C-grok-zig](C-grok-zig/) | xAI | Grok 4.7 | high | Zig 0.15.2 | `ffcfc22` |
| [D-gemini-kotlin](D-gemini-kotlin/) | Google | Gemini 4 Argon | high | Kotlin / JVM 21 | `fd24932` |
| [E-kimi-csharp](E-kimi-csharp/) | Moonshot | Kimi K3 | high | C# / .NET 8 | `d9fa6ea` |
| [F-glm-swift](F-glm-swift/) | Zhipu | GLM 5p3 | high | Swift 6.1 + C kernels | `6d976f4` |

Each build folder has `code/` (the build as written), `PROMPT.md`, `MODEL.md`, `RESULTS.md` (the builder's own report, unedited), `REVIEWER-NOTE.md` and `objects/` (O1–O3 with SHA256SUMS).

## Headline results (details in [results/](results/))

- On reference system 1, reference system 2 and reference system 3, all six builds produce byte-identical O1, O2 and O3, and each matches the build's committed copy.
- Cross-verify: 324 of 324 cells are all match (6 verifiers × 6 producers × 3 objects × 3 machines).
- Fixture outputs: 13 of 14 files are identical across all six builds. `hashcash.txt` from F-glm is the one exception: a single-hash slip in that test vector.
- Every build reports every permanent negative as a non-match (`negatives.txt` identical across builds).
- Best single hike: Reference system 1 0.402 legs/s, reference system 3 0.375, reference system 2 0.242.
