# Service status

Admission gate service, crate `pcoc-admission` 0.5.4. This page is the service's status. Evidence and withdrawn claims are in [EVIDENCE.md](EVIDENCE.md). Policy is in [POLICY.md](../../policy/groundinglab-admission/POLICY.md). The wire is in [WIRE.md](WIRE.md).

## Implemented

- Gate 0.5.4 (fix-forward of gate review r2 findings 1, 2, 3, 9, 24/25, 26): a restart resumes an unsettled provisional with its stored draw and never draws again (no stored draw is held); a thorough error is held, with no fallback check; the thorough ticket is the job's own provisional seq; a banned key takes no pending slot and the per-source pending cap is keyed on the public key; the served sign-in origins are the required config key `served_origins`; `pending/<seq>.draw` and `banned_keys.json` are 0600 in code. Every new decision line carries `index_network`. The groundinglab policy document is [gl-admit-v9.toml](../../policy/groundinglab-admission/gl-admit-v9.toml). The service code still names the removed policy files; those references are listed and are not edited here.

- Sign-in door `POST /v1/sign-in` (§15.1 locked wire), result poll, public hash-chained decision log, `POST /v1/verify` (section 14 record, no admission), `GET /v1/policy`, `GET /v1/health`.
- The policy file is the source of truth. The groundinglab policy is [gl-admit-v9.toml](../../policy/groundinglab-admission/gl-admit-v9.toml). `config.rs` still has a policy path of its own; that line is listed with the code references. Unknown keys are an error. `policy-bind.json` binds the id to a hash of the full table.
- The leg-rate comparison is SPEC §15.4 Rule B2 on `(hike_legs, hike_span)` from the §14 analysis record: `hike_span` > 0 and `q · hike_legs ≥ p · hike_span`. The comparison is the Rust SDK `leg_rate_floor`. `hike_span` ≤ 0 is `refuse`. `hike_span` `not read` is `held`. Bookend `N` and `S` are still shown and drive no admission comparison (Rule B6). Anchored waypoints are counted among `W_1` … `W_16`; no specific waypoint is required. The too-few refusal line carries the count (`N of 16 anchored, need 6`).
- A fast phase issues `provisional` before thorough. A settled miss, including too few anchored waypoints, does not build terrain. A `hike_span` of `not read` is held. The website outcome for that hold stays the locked `pending` token.
- Thorough draws three runs of six distinct legs, commits the draw before walking, stops at the first mismatch, then recomputes the Rule A4.3 digests. A mismatch revokes the grant and bans the public key. A pending draw file is reused on restart.
- `GET /v1/policy` returns the full table plus `source` `recommendation, not specification`. `/v1/health` echoes the policy id.

## Not on the website wire

`provisional`, `held`, `revoked`, and `banned` are admission states. They are not tokens in the locked §15.1 website table, so this build does not add them to `sign-in.json`. The website keeps `pending` while admission is unsettled, `accepted` on a thorough accept, `rejected` on a refuse or a revoke, and `too_few_waypoints_anchored` for that settled shortfall.
