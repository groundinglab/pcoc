# Service status

Admission gate service, crate `pcoc-admission` 0.4.1, tag `gate-v0.4.1`. This page is the service's status. Evidence and withdrawn claims are in [EVIDENCE.md](EVIDENCE.md). Policy is in [POLICY.md](POLICY.md). The wire is in [WIRE.md](WIRE.md).

## Implemented

- Sign-in door `POST /v1/sign-in` (§15.1 locked wire), result poll, public hash-chained decision log, `POST /v1/verify` (section 14 record, no admission), `GET /v1/policy`, `GET /v1/health`.
- Policy keys read from `gate.toml`: `policy_id`, `min_leg_rate` (a float, leg/s), `min_waypoints`. The code defaults are `gl-admit-v1`, 0.25 and 6. groundinglab's live gate is set to `gl-admit-v4` in its `gate.toml` and enforces two of its values today: `min_leg_rate` 0.2 legs/s and `min_waypoints` 6. A changed value needs a new id (`policy-bind.json`).
- Legacy `leg_rate_floor` (`strict`/`default`/`lenient`) is used only to settle a provisional line resumed at restart.

## Not yet implemented (relative to `gl-admit-v4` and §15)

- A separate fast phase issuing `provisional` before thorough. The thorough checks currently run before the decision.
- `held` for a `not read` pair. An unread closing header is refused as `bookends_not_read`.
- The refuse-at-once for fewer than two anchored of `W_1` … `W_16` (`too_few_waypoints_anchored` is not emitted).
- `min_waypoints` is counted "of 18" in code (`WAYPOINTS_TOTAL`). §14.5 counts anchored among 16.
- First and last anchored, three six-leg runs with early stop, the draw commitment, and the key ban.
- An integer leg-rate comparison (`q·N ≥ p·S`). The build compares a float.

## Monitor

The gate monitor page has no code in this repository. It reads the gate's public routes.
