# Status

What is specified, what has been measured, and what is open. This page is not the specification. Figures without a label do not belong here. Date of this page: 2026-10-26.

## Specified

| Item | Label | Where |
|---|---|---|
| Satoshi Hike 2.3 | the specification (six documents plus GUIDE). The six documents are a responsibility split of the same specification, pinned at tag `spec-2.3.0`. GUIDE is navigation and implementation guidance, not a seventh specification. | [spec/](spec/) — start at [spec/GUIDE.md](spec/GUIDE.md) |
| Tag `spec-2.3.0` | cut by the maintainers on the merge commit | `sdk/SPEC_PIN` |
| Pack `pack-2.2.0` | complete; the pack id is unchanged on the 2.3 pin | [spec/conformance/pack/](spec/conformance/pack/) |
| Satoshi Hike 2.2 | prior release, available under tag `spec-2.2.0`. The current `spec/` tree contains 2.3. | tag `spec-2.2.0` |
| Wire kind | object label `v2.2-pcoc-object` on the `spec-2.3.0` pin (not a SPEC version) | [services/admission-gate/WIRE.md](services/admission-gate/WIRE.md) |
| groundinglab policy | policy, not the specification; current example id `gl-admit-v12` | [policy/groundinglab-admission/](policy/groundinglab-admission/) |

## Measured

No cost figure is published. Measured produce and verify cost on bare metal is open (see services/admission-gate/STATUS.md).

## One relying party's log

The Admission Gate publishes a decision log (`GET /latest.json` and `GET /log/{YYYY-MM-DD}.jsonl`, [services/README.md](services/README.md)). Those entries are groundinglab's policy decisions on measurements in the §14 Measurement Record: accept, refuse, or revoke. They are not a conformance result. Another relying party can decide differently on the same measurements. The log is not evidence that a measurement token occurred, except where the entry quotes a hash-bound value.

## Open

- Cost at scale. The design aims to impose a new production cost per object. Linear cost across volume ("many cost many times one", as a measured result), deployed cost, and resistance to well-funded attackers are unmeasured.
- Catch rates. Not established.
- Economic security. Not established.
- Many-channel parallelism. The question of whether a host with many memory channels changes the cost is open. No such measurement is published.
- Android build reproducibility. Not achieved. See [apps/android/VERIFY.md](apps/android/VERIFY.md).

## Not claimed

Bitcoin does not secure or pay for a PCOC. The index can be bypassed. Verify measures. Policy decides.
