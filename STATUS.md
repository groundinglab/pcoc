# Status

What is specified, what has been measured, and what is open. This page is not the specification. Figures without a label do not belong here. Date of this page: 2026-10-22.

## Specified

| Item | Label | Where |
|---|---|---|
| Satoshi Hike 2.2 | the specification | [spec/satoshi-hike-v2.2/SPEC.md](spec/satoshi-hike-v2.2/SPEC.md) |
| Tag `spec-2.2.0` | cut by the maintainers on the merge commit | `sdk/SPEC_PIN` |
| Pack `pack-2.2.0` | incomplete until the SDK phase | [spec/conformance/pack/](spec/conformance/pack/) |
| groundinglab policy | policy, not the specification | [policy/groundinglab-admission/](policy/groundinglab-admission/) |

## Measured

No cost figure is published for Satoshi Hike 2.2. Measured produce and verify cost on bare metal is open (see services/admission-gate/STATUS.md).

## One relying party's log

The Admission Gate publishes a decision log (`GET /latest.json` and `GET /log/{YYYY-MM-DD}.jsonl`, [services/README.md](services/README.md)). Those entries are groundinglab's policy decisions on Part A measurements: accept, refuse, or revoke. They are not a conformance result. Another relying party can decide differently on the same measurements. The log is not evidence that a measurement token occurred, except where the entry quotes a hash-bound value.

## Open

- Cost at scale. Linear cost ("many cost many times one", as a measured result) is unmeasured.
- Catch rates. Not established.
- Economic security. Not established.
- Many-channel parallelism. The question of whether a host with many memory channels changes the cost is open. No such measurement is published.
- Android build reproducibility. Not achieved. See [apps/android/VERIFY.md](apps/android/VERIFY.md).

## Not claimed

Bitcoin does not secure or pay for a PCOC. The index can be bypassed. Verify measures. Policy decides.
