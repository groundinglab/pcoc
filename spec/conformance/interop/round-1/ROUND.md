# Round 1

- **SPEC:** Satoshi Hike 2.2 at groundinglab/pcoc `2b50df7`, `spec/satoshi-hike-v2.2/SPEC.md`, sha256 `3055ef488bf761c198894a13b433c3f2d67a26c9ad73e937abfa4b1619e66319`.
- **Fixtures:** [CASES.md](CASES.md) (round-1 version).
- **Prompt:** [PROMPT.md](PROMPT.md) is the shared prompt. Each build's `PROMPT.md` is the full message actually sent, inline, after attached files failed to reach the build machines.
- **Date:** 22 Oct 2026, launched 13:31–13:32 ACDT.
- **Effort:** low, on purpose: a low-effort build exposes where the SPEC is ambiguous.

| Build | Provider | Model | Effort | Language | Commit |
|---|---|---|---|---|---|
| [b1-sonnet-ts](b1-sonnet-ts/) | Anthropic | Claude Sonnet 5.5 | low | TypeScript (Node.js) | `a3ac71d` |
| [b2-sol-py](b2-sol-py/) | OpenAI | GPT-5.6 Sol | low | Python 3 | `14e8c50` |

## Result

- b1 against b2: 304 candidate keys match and 32 do not. None of the mismatches is on a SPEC-defined construction value (seed, terrain, fold, kat-small, test-3leg, prod, wes, anchor, unknown-kind). All 10 cross-verify runs match ([results/DIFF.md](results/DIFF.md), [results/xverify/](results/xverify/)).
- A six-way comparison with the four SDK cores then being built (Rust, C, Go, Java) found every SPEC-defined construction value byte-identical in all six builds: 266 of 266 keys ([results/DIFF-6WAY.md](results/DIFF-6WAY.md)).
- Every mismatch traced to a fixtures gap, a SPEC gap or a defect in one build's negatives. The SPEC gaps were fixed as 29 clarifications in SPEC 2.2 erratum 1 (pcoc `7b40353`), with no change to sections 10–13.
- Round-1 timings come from the builders' own cloud machines and only show that the builds work.

The six-way comparison's raw key dump (`sixway.json`) is not included: its hex test values trip the repository's secret scanner. [results/sixway-table.md](results/sixway-table.md) and [results/DIFF-6WAY.md](results/DIFF-6WAY.md) carry the same comparison.
