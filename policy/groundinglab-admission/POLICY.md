# groundinglab admission policy gl-admit-v11

- **Status.** A policy file's `status` is one of `draft`, `active`, or `superseded`. `gl-admit-v11` is `active` in-repo once this file ships; **the live gate stays on gl-admit-v10 until Christian says go** (pcoc #115). `gl-admit-v10` remains live until that swap, then becomes `superseded`.
- **Repo path (agreed layout).** `policy/groundinglab-admission/POLICY.md`, next to `gl-admit-v11.toml`.
- **Targets.** Satoshi Hike 2.3, tag `spec-2.3.0` (`spec/satoshi-hike-v2.3/SPEC.md`).
- **Policy version.** The `policy_version` is the id `gl-admit-v11` plus the SHA-256 of `gl-admit-v11.toml`. Ops pins that SHA-256 in the deployment manifest at deploy time.

## Changes from gl-admit-v10 (Christian, 24 Oct 2026)

| Lever | v10 | v11 |
|---|---|---|
| `leg_rate_floor` | `{ p = 1, q = 5 }` (0.2) | `{ p = 1, q = 10 }` (0.1) for testing; retune after more produces (#115) |
| `spec` label | `"2.2"` | `"2.3"` |
| `submission_deadline_blocks` | 3 | **still 3** — SPEC 2.3 requires the lever (range 0..6); true disable is #117 |

A policy sets the SPEC's §15.4 levers to values inside their ranges. It never changes a SPEC rule. Section 3 holds the behaviour that SPEC leaves to policy. Changing any value or rule here needs a new id; ids are never reused.

## 1. Levers this policy sets

| Lever (SPEC §15.4) | Value | Source | Argument |
|---|---|---|---|
| `min_anchored_paths` | 6 | policy | Enough anchored waypoints among `W_1` … `W_16`, anywhere. Matches Christian's admission minima. |
| `leg_rate_floor` | `1/10` | measured hike ≈0.14; floor 0.1 for testing (#115) | Integer compare `q · hike_legs ≥ p · hike_span`. Retune after more produces. |
| `submission_deadline_blocks` (K) | 3 | still required by SPEC 2.3 | Intent withdrawn (#117). Cannot unset under SPEC 2.3; K=0 tightens; K=6 still refuses multi-hour-late submits. |
| `held_retry_seconds` | 600 | carried from v10 | Retry a held submission every 10 minutes. |
| `thorough_rounds` | 3 | carried from v10 | 18 legs in 3 rounds of 6. |
| `challenge_lifetime_seconds` | 120 | carried from v10 | |
| `challenge_clock_skew_seconds` | 5 | carried from v10 | |
| `result_retention_seconds` | 600 | carried from v10 | |
| `reverify_max_attempts` | 3 | carried from v10 | |
| `max_held_blocks` | 6 | carried from v10 | |
| `provisional` | `on` | carried from v10 | |
| `fast_accept_bound` | 2 s | carried from v10 | |
| `async_catch_budget` | 22.05 s | carried from v10 | |
| `verify_cap_per_object` | unset | carried from v10 | |
| `verify_cap_per_door_session` | 3 | carried from v10 | |
| `fake_leg_ignore_seconds` | unset (no expiry) | carried from v10 | |

## 2. Fixed by the SPEC, not set here

Same as v10: `provisional_action_cap`, `revoke_scope`, the 6-leg round, Rule B9, fake-leg ignore, production-profile-only, mainnet index on live plane, policy-file encoding under SPEC 2.3 D2. Under SPEC 2.3 an absent key means unset only for `verify_cap_per_object`, `verify_cap_per_door_session` and `fake_leg_ignore_seconds`.

## 3. Behaviour SPEC leaves to policy

Unchanged from gl-admit-v10 sections P1–P5 (integrity, plant gap record-only, friendly-entry custody, draw commitment, `born_height` = **972685**).

## 4. Open

- Deploy only after Christian's go (#115 AC3).
- Land #117 (SPEC) before claiming submission_deadline is off.
- Retune `leg_rate_floor` after further produces.
