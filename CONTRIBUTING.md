# Contributing

`main` takes pull requests. CI is self-hosted. Fork pull requests do not run the workflows. A maintainer re-runs CI from a branch on this repository.

One subject per branch. Future work stays one subject. The publication branch already in flight is the exception.

## Spec changes

A Part A change updates the vectors in the same pull request, recomputes `reference/SPEC_PIN`, and runs the full suites. A path move of the vectors is a spec change for this reason.

## Wording

Measurement tokens (`match`, `mismatch`, `present`, `skip`, `not read`, `not defined`, `n/a`) stay out of policy sentences. Policy words are `accept`, `refuse`, `revoke`, and `held`. They are not verifier outputs.

## Stable pages

Stable pages do not carry lab hostnames, LAN addresses, living issue numbers, or process meta.

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
