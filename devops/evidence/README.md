# Evidence snapshot and limitations

**Remote status fetched 2026-11-08 about 04:25–04:26 ACDT (Australia/Adelaide, UTC+10:30).** These rows distinguish observed remote metadata, historical observations and reported diagnosis. This documentation release does not rerun or repair the producer experiment.

| Item | Outcome | Evidence and limit |
|---|---|---|
| Producer branch | Unfinished | `gb/csharp-producer-cutA`, last run head `bedc1d828337195c14de3e23722cc73d16b1f776` |
| Latest producer CI | Failed, completed | [42031616695](https://github.com/groundinglab/pcoc/actions/runs/42031616695), last update 2026-11-05 23:45:38 ACDT; refreshed metadata, not a new test |
| Prior same-head run | Failed | [42028918719](https://github.com/groundinglab/pcoc/actions/runs/42028918719); historical evidence, not a new execution |
| Detailed producer checks | C# and mutations failed; cross-verify succeeded | Earlier remote job inspection; no claim the whole producer passed |
| Pre-build measurement | Historical 20/20 green | [42005204389](https://github.com/groundinglab/pcoc/actions/runs/42005204389), pin/scratch stand-ins; NOT completed producer proof |
| Earlier repair CI runs | Cancelled | Historical runs 42027487004 and 42026128624; cancellation is not a pass |
| Mutation diagnosis | Reported, not reproduced by this release | Fixture ending at 1018 versus seed requiring 1019; separately production-comment tokens tripping a scan. No fix claimed |
| Full workflow / outsider reproduction | Not run by this release | No reliability or independent SPEC-only success claim |
| Per-bot skill attachments | Unknown | Available source/catalog does not establish attachment |
| New public recipes | WIP, not validated end to end | Manual instructions, no shared skill adoption |
| Deliberate negative / production experiment | Not run here | Out of scope; do not relabel as skipped or passed |
| Evidence-file quickstart | Passed locally 2026-11-08 (ACDT) | Exact README shell block exited 0; JSON roundtrip and digest only, not semantic verification |

The artifact quality bar for the producer is byte-identical pinned objects, fresh-key agreement and five-verifier/tamper evidence, SPEC-exact behavior, meaningful negatives and provenance—not merely a green count. Production SDK integration and independent SPEC-only implementation are different tracks. Neither conformance nor cross-language agreement establishes every security/economic property.

A future effectiveness report should include all attempts, repairs, cancellations, skipped/unrun checks, human intervention and measured resource use or unavailable values. One success is a demonstration, not a reliability estimate.
