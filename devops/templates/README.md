# Copyable task, acceptance and evidence records

These are original minimal examples, not mandatory operational schemas and not copies of the historical private ticket template. They add no permission or gate. Replace placeholders honestly; an unresolved value is not a pass.

## Task

```text
Task ID / purpose:
Output / excluded work:
Specification version + digest:
Code base/head or content digest:
Allowed inputs, files, tools and environment:
Dependencies / integration owner:
Governing policy version + digest + applicable keys:
Existing permissions / budget / stop rule:
Completion evidence destination:
Unresolved ambiguity:
```

## Acceptance criterion (repeat per requirement)

```text
AC ID:
Must: one falsifiable behavior
Why: intent and failure prevented
Push-back: what to challenge if the contract fights reality
Check: exact command, fixture/input pin, assertion and expected result
Negative: deliberate defect, expected failure and why it reaches this property
Evidence: output path/digest, code/check revision, outcome
Not covered:
```

A requirement to reject malformed input should not pass merely because a compiler or earlier pin check failed. Identify the intended assertion and retain evidence it was reached. Do not silently rewrite the criterion when implementation is difficult.

## Attempt/evidence

```text
Attempt / task / start-end with time zone:
Input/specification/fixture/code/check/prompt/recipe digests:
Builder requested and reported settings (or unknown):
Environment/toolchain and isolation limitations:
Exact command / exit status / raw output digest:
Outcome: passed | failed | cancelled | skipped | not run | unknown
Failure classification / case and inputs:
Hypothesis / falsifier before next authorised attempt:
Human intervention / uncertainty / next bounded action:
Supersedes/invalidates which earlier evidence:
Permission to publish: separate from technical results
```

## Review and release summary

Name the head, criteria covered, findings, source/log locations, deliberate negatives, independent reproduction versus cited reports, limitations and stale evidence. Record each required check by name, not only a green count. A moved head needs a delta review. Approval of technical evidence is not permission to publish; apply the existing task/release policy. Never include secrets or private histories in public evidence.
