# Contributing

`main` takes pull requests. CI is self-hosted. Fork pull requests do not run the workflows. A maintainer re-runs CI from a branch on this repository.

One subject per branch.

## Spec changes

A semantic change to Construction must update the corresponding Conformance rules and vectors in the same pull request. When the vector pack or its manifest changes, update the pin accordingly. Run the full SDK suites and conformance checks. Editorial changes that do not alter normative behaviour need not change vectors. A path move of the vectors is a spec change for this reason.

## Wording

Measurement tokens stay out of policy sentences. The closed set defined in §4 is:

`match`, `mismatch`, `present`, `skip`, `not defined`, `not read`, `not fetched by design`, `n/a`, `anchored`, `not anchored`, `unknown kind`.

Policy words are `accept`, `refuse`, `revoke`, and `held`. They are not verifier outputs.

## Stable pages

Stable pages do not carry lab hostnames, LAN addresses, living issue numbers, or process meta.

## Risk tier

Each ticket and pull request carries a `Risk tier:` line set to `Quick`, `Normal`, or `High`. Quick work merges on green CI, Normal work merges after both reviewers pass, and High work also waits for the owner's go. A tier can be raised at any time and only the owner lowers it. A ticket is checked before work starts and is marked `Lauren checked: OK` or `Lauren checked: changes needed`. The full process is in [`devops/TICKET_TO_MERGE.md`](https://github.com/groundinglab/ops/blob/main/devops/TICKET_TO_MERGE.md) in `groundinglab/ops`.

## Code comments

This is guidance, not a rule, and no check enforces it. Pull requests, including ones written by coding agents, should not add code comments. A comment is often used to justify a shortcut, and the shortcut then gets copied along with the comment. Put the reason in the pull request description, the commit message, or a test that pins the behaviour, and let names and structure carry the rest. A link to an upstream issue or RFC that explains a workaround is fine, and so is legal or licence text.

## Licence and sign-off

Contributions are Apache-2.0 for code and CC BY 4.0 for prose, as [LICENSE-DOCS.md](LICENSE-DOCS.md) states. Conformance vector bytes are Apache-2.0. Sign-off is a `Signed-off-by` trailer (Developer Certificate of Origin).

## Depth

| Depth | Home | Job |
|---|---|---|
| What it is | [README.md](README.md) | the project in one sitting |
| Briefs | [docs/briefs/](docs/briefs/) | the questions, in plain language |
| Rationale | [docs/rationale/](docs/rationale/) | why it is built this way |
| Specification | [spec/](spec/) | normative |

The specification wins. Where a page and the specification disagree, the specification is the construction.
