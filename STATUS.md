# Status

What is specified, what has been measured, and what is open. This page is not the specification. Figures without a label do not belong here. Date of this page: 2026-10-15.

## Specified

| Item | Label | Where |
|---|---|---|
| Satoshi Hike v2.0 construction | locked in the specification, 2026-10-08 | `spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md` |
| `STEPS_PER_LEG` 2000000, eighteen stages, anchor offsets 2, 3, 4 | locked, Part A | specification §6 and §13.1 |
| Leg-rate floor | a Part B parameter; the specification sets no value (amended 2026-10-16). Example figures 7/20, 1/4, 1/8 are informative | specification §15.4, §15.5 |
| groundinglab's recommended default policy `gl-admit-v6` | a recommendation, not specification | [services/admission-gate/POLICY.md](services/admission-gate/POLICY.md) |
| Part A conformance | the vectors under `spec/conformance/v2.0/` | §16 |

## Measured

No cost figure is published for Satoshi Hike v2.0. Measured produce and verify cost on bare metal is open (see services/admission-gate/STATUS.md).

## One relying party's log

The Admission Gate publishes a decision log (`GET /latest.json` and `GET /log/{YYYY-MM-DD}.jsonl`, [services/README.md](services/README.md)). Those entries are groundinglab's policy decisions on Part A measurements: accept, refuse, or revoke. They are not a conformance result. Another relying party can decide differently on the same measurements. The log is not evidence that a measurement token occurred, except where the entry quotes a hash-bound value.

## Open

- Cost at scale. Linear cost ("many cost many times one", as a measured result) is unmeasured for v2.0.
- Catch rates. Not established.
- Economic security. Not established.
- Many-channel parallelism. The question of whether a host with many memory channels changes the cost is open. No such measurement is published.
- Android build reproducibility. Not achieved. See [apps/android/VERIFY.md](apps/android/VERIFY.md).

## Not claimed

Bitcoin does not secure or pay for a PCOC. The index can be bypassed. Verify measures. Policy decides.
