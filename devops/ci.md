# CI posture (groundinglab)

Canonical order for ordinary tickets (9 Nov 2026 posture). No credentials or host secrets live in this file.

## Order of work

1. **Owned builder / local on the development machine.** Run the tests and clean builds that the change's acceptance criteria require. Record commands, exit status, failures and skips. A local green is not the independent CI result.
2. **Push the same commit.** Open or update the PR at that SHA.
3. **Same-SHA CI on the retained self-hosted pool.** `groundinglab/pcoc` `.github/workflows/ci.yml` reruns the **relevant** suites on runners labelled `pcoc-heavy` only. Path gate: `.github/scripts/changes.sh`. Required rollup: **`ci-ok`**. There is **no** GitHub-hosted fallback and **no** nightly cron; a full suite is `workflow_dispatch`.
4. **Red + White.** Independent review against the ticket AC and the same-SHA evidence. Shared-test limits stay recorded; a second model alone is not an independent correctness oracle.
5. **Merge** only when the owner's merge rules and required checks (`ci-ok`) are satisfied.

## What runs where

| Surface | Where | What |
|---|---|---|
| pcoc code / spec / SDKs / services / apps / docs | Dev machine, then `pcoc-heavy` self-hosted pool | Path-gated jobs in `ci.yml`; gitleaks (v8.30.1 digest-pinned), g1, retired-term always in the light set; `ci-ok` aggregates |
| ops (manifests, secure prose, infra docs, www content) | **Dev machine only** | Ops has **no** Actions workflows and **no** runners. Before push: gitleaks v8.30.1, nsec/bare-64-hex grep, deployment manifest parse, advisory editorial-terms (see ops README Staging gate) |
| Website (`ops/www`) | Build + **published preview** + **manual** content/navigation/sign-in checks + **direct** `wrangler pages deploy` | No website Actions; no browser CI; merge ≠ deploy. Live stamp: `https://groundinglab.io/build.txt` |

## Map the change to checks

| Change | Relevant checks in both local and self-hosted passes |
|---|---|
| Component logic | Affected unit, negative and boundary tests; clean build where applicable |
| Service or CLI | Unit tests plus affected service/CLI harnesses (including BARI and admission gate where affected); clean builds |
| SDK | Affected SDK tests and builds; interoperability/conformance when the contract warrants them |
| Shared contract or dependency | Name affected SDK/service/application consumers; include their regression checks and clean builds; justify breadth |
| Application | Affected application tests and clean application build |
| Documentation | Accuracy and changed-link checks; website route below when rendered content changes |

For each acceptance criterion: intended property, concrete observable check, affected consumers, expected result, commit, and which checks belong to each pass. Explain exclusions.

## Website route

Require a successful build, a published preview and recorded manual checks of content consistency, navigation and applicable sign-in behaviour. Record preview reference and source commit/build identity, who checked what, observations, outcome and failures/unknowns. Do **not** introduce automated browser tests or browser-equipped CI runners. A static or HTTP smoke check does not replace the manual review.

Ops local www gate (named list only): render `--check`; node tests `stamp-assets`, `sign-in/locked-link`, `guides/pages`, `verify/real-object`; `check-www-site.py`; `check-links`. Headless-Chrome tests under `www/tests/` are not part of the gate. `copy.test.mjs` is retired.

## Review and posture oversight

Before launch, posture oversight reviews the concrete per-change test plan and its intent/AC mapping. At readiness, supply actual local and same-commit self-hosted results (commands, failures, skips, unresolved coverage) and the website preview/manual record where applicable.

Oversight checks ticket stamps and acceptance of CI configuration, runner sets, workflow triggers and matrices against this posture. New hosted fallback, unrelated fan-out or browser CI is nonconforming unless a scoped owner exception is explicitly recorded. Withhold a stamp only for genuine consequential ambiguity or failure of an applicable owner-approved criterion; name that ground and the smallest correction.

## Dormant built-ins

GitHub Copilot cloud-agent and Dependabot dependency-graph workflows may appear in the Actions registry. They are dormant built-ins, not part of this model; do not treat them as required gates.

## Version relationship

This document states the **implemented** CI model as of 9 Nov 2026 (owner cards ~20:22 / ~20:33 / ~20:42 Australia/Adelaide). It complements the ops v0.5.4 playbook text; governing operational process policy remains v0.5.2. This file is not a blanket v0.5.4 stamp, policy revision, auto-merge permission or normative PCOC specification change.
