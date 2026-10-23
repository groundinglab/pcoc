# Changes

## Unreleased

Crate version stays 0.5.7. Gate change #86 did not bump the patch. #89 bumped 0.5.6 to 0.5.7 as version housekeeping, not as the rule for every gate change. `sdk/SPEC_PIN` is unchanged (`spec-2.2.0`).

- Policy `spec` `2.2` loads on a spec-2.2.x pin and on a spec-2.3.x pin. Policy `spec` `2.3` loads on a spec-2.3.x pin and is refused on a spec-2.2.x pin. Any other spec string, and any other pin, is refused.
- `GET /v1/health` `spec` and `spec_kind` follow the pin. On `spec-2.2.0` they stay `Satoshi Hike 2.2` and `2.2`. Object kind stays `v2.2-pcoc-object`. Decision-log lines stay `Satoshi Hike 2.2`.
- A policy that omits `min_anchored_paths`, `leg_rate_floor`, `submission_deadline_blocks`, `held_retry_seconds`, `thorough_rounds`, `challenge_lifetime_seconds`, `challenge_clock_skew_seconds`, or `result_retention_seconds` is refused. The error names the missing key. `gl-admit-v10` sets all eight, so it loads unchanged.
- An absent `verify_cap_per_door_session` is unset (no session cap). The gate no longer fills 3. `gl-admit-v10` sets 3. `AdmissionPolicy::default` still mirrors that file and is not a load path.

## 0.5.7

Crate `pcoc-admission` is 0.5.7. Tag `gate-v0.5.6` is unchanged. This number is the tree that serves policy `gl-admit-v10`.
