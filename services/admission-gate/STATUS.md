# Service status

Admission gate service, crate `pcoc-admission` 0.5.2. This page is the service's status. Evidence and withdrawn claims are in [EVIDENCE.md](EVIDENCE.md). Policy is in [POLICY.md](POLICY.md). The wire is in [WIRE.md](WIRE.md).

## Implemented

- Sign-in door `POST /v1/sign-in` (§15.1 locked wire), result poll, public hash-chained decision log, `POST /v1/verify` (section 14 record, no admission), `GET /v1/policy`, `GET /v1/health`.
- The policy file is the source of truth. `config.rs` reads `policy = "policy/gl-admit-v6.toml"`, or an inline `[policy_table]`. Unknown keys are an error. `policy-bind.json` binds the id to a hash of the full table.
- The code default is `gl-admit-v6`. The leg-rate comparison is the integer `q·N ≥ p·S` on the bookend pair. Anchored waypoints are counted among `W_1` … `W_16`; no specific waypoint is required. The too-few refusal line carries the count (`N of 16 anchored, need 6`).
- A fast phase issues `provisional` before thorough. A settled miss, including too few anchored waypoints, does not build terrain. A `not read` is held. The website outcome for that hold stays the locked `pending` token.
- Thorough draws three runs of six distinct legs, commits the draw before walking, stops at the first mismatch, then recomputes the Rule A4.3 digests. A mismatch revokes the grant and bans the public key. A pending draw file is reused on restart.
- `GET /v1/policy` returns the full table plus `source` `recommendation, not specification`. `/v1/health` echoes the policy id.

## Not on the website wire

`provisional`, `held`, `revoked`, and `banned` are admission states. They are not tokens in the locked §15.1 website table, so this build does not add them to `sign-in.json`. The website keeps `pending` while admission is unsettled, `accepted` on a thorough accept, `rejected` on a refuse or a revoke, and `too_few_waypoints_anchored` for that settled shortfall.
