# Building from a specification — experimental WIP kit

**Public snapshot:** 2026-11-08 · **Kit revision:** WIP-1. This is non-normative development guidance, not a PCOC specification, SDK release or permission to operate any service.

[Start manually](#manual-quickstart) · [CI posture](ci.md) · [Copyable templates](templates/README.md) · [Portable recipes](skills/README.md) · [Source reconciliation and omissions](sources/README.md) · [Observed results](evidence/README.md) · [History and proposals](history/README.md)

## What is being tested

Write an explicit requirement, run a named check against pinned inputs, and keep its result—including failures. A person or their own coding agent can follow the same files. The immediate experiment is whether **one complete run can meet its agreed quality bar before moving on**. This is not proof of reliability, zero repairs, superior productivity or security. More agents and retries are not inherently better.

The specification controls behavior, interfaces and invariants. A small dependency graph controls execution order and integration boundaries; it does not replace that contract. Agents investigate, implement, test and review bounded tasks. Verification checks explicit criteria. Humans own objectives, permissions, risk thresholds, ambiguity and consequential decisions.

## Version and authority map

| Material | Snapshot status | What it means |
|---|---|---|
| Operational playbook v0.5.3 | Current, in operational testing | Published running body has internal revision r6c; not a completed experiment |
| Process policy v0.5.2 | Active operational policy | Separate version axis; proposed v0.5.3 policy keys and auto-merge are not in force |
| Playbook r6b stamp | Historical, different bytes | Does not stamp r6c or the producer ticket |
| v0.5.4 corrections and stamp recipe | Prepared proposals only | Not assembled/tested/adopted by this snapshot; public WIP does not wait for them |
| This kit | Manual, non-normative public extraction | No operational policy or installed skill is changed by reading it |

Use **one governing policy** chosen by the task owner. Record its version/digest and cite its keys; do not copy numeric budgets into every task or recipe. This kit introduces no global retry counts, new approval gates or permission grants. The original operational policy is not republished here: [source rights and omissions](sources/README.md). A reader supplies their own policy; the example route below needs no private service or account.

## Manual quickstart

Prerequisites: a text editor, a local project and its normal test tools. Python 3 is used only for the small evidence-file example below. Git is optional for this smoke; a real task should record its code commit or content hash. No bot account, hosted builder, deployment token or lab host is needed.

1. Copy the task and AC blocks from [templates](templates/README.md). Name the output and excluded work. Pin specification, fixtures, code and check versions. Resolve consequential ambiguity before implementation; do not invent missing behavior.
2. Name the observable check and one deliberate negative that reaches the relevant assertion. Freeze expected values independently of the implementation when that matters. A setup crash is not a successful negative.
3. Order tasks by dependency. Parallelize only independent writes; reserve one integration task for the combined artifact at a single head.
4. Run the baseline, make the bounded change, then run applicable checks. Keep exact commands, exit status and output; do not label an unrun check passed. A second model alone does not establish independence.
5. On failure, classify before consulting budgets. Record the failing case/inputs, hypothesis and falsifier before an authorised retry. Follow the governing stop rule; do not extend caps or treat a mutation hang/OOM as a flake.
6. Review the artifact and evidence against the actual contract. Retain unresolved requirements and human interventions. Merge/deploy only under the owner's existing permissions and required checks.

Try the evidence-file mechanics in a fresh scratch directory (not the project checkout):

```sh
mkdir evidence-smoke
cd evidence-smoke
python3 - <<'PYTHON'
import json, pathlib
p = pathlib.Path('record.json')
p.write_text(json.dumps({
    'task': 'evidence-file-smoke',
    'scope': 'file mechanics only; not a producer or workflow validation',
    'checks': [{'id': 'json-roundtrip', 'status': 'passed'}],
    'unrun': ['project tests', 'independent review', 'deployment']
}, indent=2) + '\n')
assert json.loads(p.read_text())['checks'][0]['status'] == 'passed'
print('PASS json-roundtrip; project checks NOT RUN')
PYTHON
python3 - <<'PYTHON'
import hashlib, pathlib
print(hashlib.sha256(pathlib.Path('record.json').read_bytes()).hexdigest())
PYTHON
```

Expected first line: `PASS json-roundtrip; project checks NOT RUN`. The second command prints the file's digest, not a fixed universal answer. This smoke only demonstrates recording and hashing a result. It does not validate a real task or the operational workflow. [Recorded execution status](evidence/README.md).

## Evidence limits and boundaries

Passing finite checks supports only their stated coverage at that revision/environment. Incomplete specifications, circular expected values and correlated reviewers can miss the same defect. Separate observed evidence, reports and hypotheses; distinguish passed, failed, cancelled, skipped, not run and unknown. Preserve initial failures, repairs, environment changes and exhausted attempts, not just the final success.

For normal PCOC integration, prefer a [released SDK](https://groundinglab.io/technical/sdk/) and follow [ai/README.md](../ai/README.md). An independent SPEC-only experiment needs its own permitted-input, isolation/provenance and interop evidence packet. The number of SDK languages does not prove independent SPEC-only success. Workflow guidance must not modify [the normative specification](../spec/GUIDE.md), conformance vectors, production/measurement/admission separation or SDK behavior.

[Website method guide](https://groundinglab.io/technical/building-from-a-specification/) explains the method; this directory is the public kit's canonical home. Its source snapshot is pcoc `53792d57caab78e9821bc817929507517321ce71` before these documentation additions. Original writing in these per-folder READMEs falls within the existing per-folder README prose scope of [LICENSE-DOCS.md](../LICENSE-DOCS.md). No private or third-party source is relicensed here.
