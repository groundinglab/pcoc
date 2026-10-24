# Evidence status (Admission Gate)

This note records evidence and withdrawn claims. It is not the service status ([STATUS.md](STATUS.md)), not policy ([POLICY.md](../../policy/groundinglab-admission/POLICY.md)), and does not change the wire ([WIRE.md](WIRE.md)).

## Produce and verify measured evidence

**Not fully closed.** Measured produce and verify cost on bare metal is a follow-up. This item is not waived, and it is not treated as unmeasured.

The specification is the [Satoshi Hike 2.3 pack](../../spec/README.md). The program is `pcoc` in `sdk/rust/` (`pcoc verify`, `pcoc produce --rig`).

## Killed slogans

These sentences are not claims of this gate.

- “K=2 catches 90% of 10% shaves” — killed. On `N = 18`, a 10% shave is not an integer count of unpaid leg results (`0.10 × 18 = 1.8`). The nearest row is `F = 2` unpaid leg results and `K = 2`, catch `11/51` (about 22%). Catch at or above 90% on that row needs `K = 12`.
- “Forges failed ⇒ economic security / cost won’t amortize proven” — killed as a public claim. A red-team run is a miss mechanism. It does not prove economic security, and it does not prove that a cost will not amortize.
- Farm-scale dollar deterrence — not established.
- “A tip-hold or a hike-CPU shave is a ≤0.9× produce-dollar win” — killed. Two unpaid leg results are a negligible share of a tip-dominated wall; that is not a produce-dollar win.
