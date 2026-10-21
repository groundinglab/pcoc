# Service status

Admission gate service, crate `pcoc-admission` 0.5.4. This page is the service's status. Evidence and withdrawn claims are in [EVIDENCE.md](EVIDENCE.md). Policy is in [POLICY.md](POLICY.md). The wire is in [WIRE.md](WIRE.md).

## Implemented

- Gate 0.5.4 (fix-forward of gate review r2 findings 1, 2, 3, 9, 24/25, 26): a restart resumes an unsettled provisional with its stored draw and never draws again (no stored draw is held); a thorough error is held, with no fallback check; the thorough ticket is the job's own provisional seq; a banned key takes no pending slot and the per-source pending cap is keyed on the public key; the served sign-in origins are the required config key `served_origins`; `pending/<seq>.draw` and `banned_keys.json` are 0600 in code. Every new decision line carries `index_network`. `policy/gl-admit-v7.toml` (v6 plus `[index] expected_network = "main"`, refusal `index_network_not_expected`) is prepared; groundinglab's deploy file stays on `gl-admit-v6`.

- Sign-in door `POST /v1/sign-in` (§15.1 locked wire), result poll, public hash-chained decision log, `POST /v1/verify` (section 14 record, no admission), `GET /v1/policy`, `GET /v1/health`.
- The policy file is the source of truth. `config.rs` reads `policy = "policy/gl-admit-v6.toml"`, or an inline `[policy_table]`. Unknown keys are an error. `policy-bind.json` binds the id to a hash of the full table.
- The code default is `gl-admit-v6`. SPEC v2.1 withdraws bookend admission: the rate is total legs across all stages divided by Bitcoin clock time from the first beacon to the last beacon, and at least 6 waypoints planted anywhere, with no first-or-last-planted requirement. This build still compares the withdrawn pair until `impl/v2.1-rust`. Anchored waypoints are counted among `W_1` … `W_16`; no specific waypoint is required. The too-few refusal line carries the count (`N of 16 anchored, need 6`).
- A fast phase issues `provisional` before thorough. A settled miss, including too few anchored waypoints, does not build terrain. A `not read` is held. The website outcome for that hold stays the locked `pending` token.
- Thorough draws three runs of six distinct legs, commits the draw before walking, stops at the first mismatch, then recomputes the Rule A4.3 digests. SPEC v2.1 runs those three rounds in parallel and withdraws the candidates of 48 and 90 legs. This build still walks the drawn legs one after another until `impl/v2.1-rust`. A mismatch revokes the grant and bans the public key. A pending draw file is reused on restart.
- `GET /v1/policy` returns the full table plus `source` `recommendation, not specification`. `/v1/health` echoes the policy id.

## Not on the website wire

`provisional`, `held`, `revoked`, and `banned` are admission states. They are not tokens in the locked §15.1 website table, so this build does not add them to `sign-in.json`. The website keeps `pending` while admission is unsettled, `accepted` on a thorough accept, `rejected` on a refuse or a revoke, and `too_few_waypoints_anchored` for that settled shortfall.
