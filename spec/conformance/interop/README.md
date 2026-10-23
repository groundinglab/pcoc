# Interop evidence: independent builds of the Satoshi Hike specification

This folder holds the evidence behind groundinglab's admission policy numbers: independent builds of the SPEC, written by AI models from different providers in different languages, with their prompts, their own reports, and the results of building, cross-verifying and timing them on three commodity reference systems ([REFERENCE-SYSTEMS.md](REFERENCE-SYSTEMS.md)). The web page that argues from this evidence is <https://groundinglab.io/admission-gate/policy/>.

**If you are integrating PCOC, use the SDKs in the repository `sdk/` directory, not these builds.** The SDKs are the reference implementations (SPEC section 16.2) and the ones groundinglab's own services and apps run on. These builds are evidence that the SPEC alone is enough to build interoperable implementations, and a way to see how fast each language gets. They are published as they were written and are not maintained.

## What is here

| Path | What |
|---|---|
| [METHOD.md](METHOD.md) | How builds were commissioned and how they were measured |
| [round-1/](round-1/) | Two low-effort builds against SPEC 2.2 (pcoc 2b50df7). Their disagreements became SPEC 2.2 erratum 1 |
| [round-2/](round-2/) | Six high-effort builds against SPEC 2.2 + erratum 1 (pcoc 7b40353), built and timed on three reference machines |
| [round-2/results/](round-2/results/) | Timings, packed runs, the 6 × 6 cross-verify grid, output comparisons and raw logs |
| [spec-2.3/](spec-2.3/) | Round-2 evidence re-run on RS3 against SPEC 2.3 (spec-2.3.0). Same objects, fixtures, and negatives |
| [calibration/](calibration/) | How the leg-rate floor in the groundinglab admission policy follows from the measurements |
| [kit/](kit/) | The scripts used to build, test, cross-verify and time every build |
| [community/](community/) | How to submit your own results, and the file schema |

## Round 2 in one paragraph

Six models (Anthropic Claude Sonnet 5.5, OpenAI GPT-5.6 Sol, xAI Grok 4.7, Google Gemini 4 Argon, Moonshot Kimi K3, Zhipu GLM 5p3) each built the SPEC in a different language (TypeScript, Python with C, Zig, Kotlin, C#, Swift) from the pinned public SPEC, policy and fixtures alone, with one shared prompt. Each build was then compiled from its own source on reference system 1 (Ryzen 5 5600H, the primary reference), reference system 2 (Core i7-10610U, the SPEC's low-end host) and reference system 3 (Ryzen 5 7430U). On all three machines all six builds produce byte-identical objects, and every build verifies every other build's objects: 324 of 324 cross-verify cells match.

## Reproduce

1. Read [kit/README.md](kit/README.md). `kit/setup.sh` installs the toolchains in your user directory; no root is needed.
2. Build and test each build: `kit/run-build.sh <build-id>`.
3. Cross-verify every pair: `kit/xgrid.sh`.
4. Time packed hikes: `kit/packed/ladder.sh` (N = 1, 2, 4, 6).
5. Compare with [round-2/results/](round-2/results/), then send us your numbers ([community/README.md](community/README.md)).

## Versioning

- Each round names exactly one SPEC commit and SHA-256 and one fixtures file, in its `ROUND.md`. A published round is frozen: corrections go in a new round, not as edits.
- Each calibration note names the policy id it supports. Policy ids are never reused, so a new floor means a new id and a new note.

## Licence

Prose (Markdown) is CC BY 4.0 and code is Apache-2.0, as in the repository files `LICENSE-DOCS.md` and `LICENSE`. Each build under `code/` is published as its builder wrote it.
