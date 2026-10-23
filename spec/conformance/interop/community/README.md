# Submit your results

This is an open test. Run the builds, or your own build from the SPEC, on whatever hardware you have, and tell us what you measured. A different machine, a faster conforming build, or a result that disagrees with ours is exactly the evidence we want.

## Two ways to submit

1. **Issue.** Open a [Reference results](https://github.com/groundinglab/pcoc/issues/new?template=reference-results.yml) issue and paste your files. This is the easy route.
2. **Pull request.** Add a folder `community/<yyyy-mm-dd>-<handle>-<machine>/` with the files in [SCHEMA.md](SCHEMA.md), and open a pull request. [example/](example/) shows `machine.json`.

## What happens next

- We check that your objects match the published SHA-256 sums. A result that comes with mismatching bytes is reported as a possible bug, not as a timing.
- Accepted submissions are listed here as **community-measured**, kept apart from groundinglab's reference runs, and considered at the next calibration.
- Don't include secrets, private keys, home paths or anything that identifies your machine beyond what the schema asks for.

## Submissions

None yet.
