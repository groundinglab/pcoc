# Changes

## Unreleased

- SPEC 2.3 retired the tip-versus-last-beacon deadline (old Rule B10). The gate does not refuse `submission_deadline` when the index tip is past `last_interrupt_height`. `submission_deadline_blocks` is not a required lever. A file that still has the key (`gl-admit-v9`, `gl-admit-v10`) loads, and the value is ignored. A file that omits the key loads. This note does not issue a new policy id and does not deploy.
- Sticky refuse is off. A resubmit of a refused `(public_key, object_sha256, policy_id)` runs the fast phase again and may append a new refuse line. `[policy_rules] sticky_refuse = true` turns the stored-outcome short circuit back on. An absent key means off, including `gl-admit-v10`. The key is not a §15.4 lever and is not part of the policy bind hash. `gl-admit-v11` sets `false`. This note does not deploy.

## 0.5.8

Crate `pcoc-admission` is 0.5.8. Tag `gate-v0.5.7` was not moved. This build links Rust SDK 2.3.1 on pin `spec-2.3.0`.

- Policy `spec` `2.2` loads on a spec-2.2.x pin and on a spec-2.3.x pin. Policy `spec` `2.3` loads on a spec-2.3.x pin and is refused on a spec-2.2.x pin. Any other spec string, and any other pin, is refused.
- `GET /v1/health` `spec` and `spec_kind` follow the pin. On `spec-2.2.0` they stay `Satoshi Hike 2.2` and `2.2`. Object kind stays `v2.2-pcoc-object`. Decision-log lines stay `Satoshi Hike 2.2`.
- A policy that omits `min_anchored_paths`, `leg_rate_floor`, `submission_deadline_blocks`, `held_retry_seconds`, `thorough_rounds`, `challenge_lifetime_seconds`, `challenge_clock_skew_seconds`, or `result_retention_seconds` is refused. The error names the missing key. `gl-admit-v10` sets all eight, so it loads unchanged.
- An absent `verify_cap_per_door_session` is unset (no session cap). The gate no longer fills 3. `gl-admit-v10` sets 3. `AdmissionPolicy::default` mirrors that file for tests only. A release build does not compile it, or `Config::for_tests`.
- A policy file must set `policy_id`, `draw_commitment`, and `born_height`. A missing key refuses the file and the error names it. There is no fallback id, commitment sentence, or born height of 0. `expected_network` may be absent, which means no index-network constraint. `gl-admit-v9.toml` does not set `born_height`, so the gate refuses that file. The file was not edited. `gl-admit-v10.toml` sets the key and still loads on a spec-2.2.0 pin and on a spec-2.3.0 pin.

## 0.5.7

Crate `pcoc-admission` is 0.5.7. Tag `gate-v0.5.6` is unchanged. This number is the tree that serves policy `gl-admit-v10`.
