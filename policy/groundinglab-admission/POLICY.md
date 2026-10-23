# groundinglab admission policy gl-admit-v10

- **Status.** A policy file's `status` is one of `draft`, `active`, or `superseded`. `gl-admit-v10` is `active`. Live from 2026-10-23. `gl-admit-v9` is `superseded`: it was live until 2026-10-23, and `gl-admit-v10` supersedes it.
- **Repo path (agreed layout).** `policy/groundinglab-admission/POLICY.md`, next to `gl-admit-v10.toml`. `gl-admit-v9.toml` stays in this directory with `status` `superseded`.
- **Targets.** Satoshi Hike 2.2, tag `spec-2.2.0` (`spec/satoshi-hike-v2.2/SPEC.md`).
- **Policy version.** The `policy_version` is the id `gl-admit-v10` plus the SHA-256 of `gl-admit-v10.toml`. Ops pins that SHA-256 in the deployment manifest at deploy time.

A policy sets the SPEC's §15.4 levers to values inside their ranges. It never changes a SPEC rule. Section 3 holds the behaviour that SPEC 2.2 leaves to policy. Changing any value or rule here needs a new id; ids are never reused. `gl-admit-v9.toml` is the previous id. Its `status` is `superseded`. Earlier ids are kept only in ops `policy-bind.json`.

## 1. Levers this policy sets

| Lever (SPEC §15.4) | Value | Source | Argument |
|---|---|---|---|
| `min_anchored_paths` | 6 | SPEC default | Enough anchored waypoints to time the hike without punishing a weak connection. They can be anywhere among `W_1` … `W_16`. |
| `leg_rate_floor` | `1/5` | SPEC default (`p ≥ 1`, `q ≥ 1`) | One leg per 5 s over the MTP span. It must be re-measured under 2.2 with async producers (SPEC §15.5). It will rise once the 15–45 s wait no longer sits inside the span. |
| `submission_deadline_blocks` (K) | 3 | SPEC default (range 0–6) | Submit within 3 blocks of the last beacon. |
| `held_retry_seconds` | 600 | SPEC default | Retry a held submission every 10 minutes. |
| `thorough_rounds` | 3 | SPEC default | 18 legs in 3 rounds of 6, which detects a 25 % fake fraction 99.44 % of the time. |
| `challenge_lifetime_seconds` | 120 | SPEC default | The website mints with the same value. |
| `challenge_clock_skew_seconds` | 5 | SPEC default | |
| `result_retention_seconds` | 600 | SPEC default | |
| `reverify_max_attempts` | 3 | policy (no SPEC default) | Earlier `default`-profile figure. |
| `max_held_blocks` | 6 | policy (no SPEC default) | Earlier `default`-profile figure. |
| `provisional` | `on` | policy | Cheap checks first, then a short read-only grant that is never `accept`. |
| `fast_accept_bound` | 2 s | policy | Earlier `default`-profile figure. |
| `async_catch_budget` | 22.05 s | policy | Times the parallel checkpoint-digest pass. It does not include recomputing legs. |
| `verify_cap_per_object` | unset | SPEC default | The earlier candidate was 1. Set it if compute abuse appears. |
| `verify_cap_per_door_session` | 3 | policy | SPEC default is unset. This file sets 3. The gate reads the key. |
| `fake_leg_ignore_seconds` | unset (no expiry) | SPEC default | A fake leg ignores that key at this gate permanently. |

## 2. Fixed by the SPEC, not set here

`provisional_action_cap`, `revoke_scope`, the 6-leg round, the Rule B9 invariant refuses, and the fake-leg ignore itself are fixed by SPEC 2.2. So are: admission reads only a `production`-profile record, and a live-plane gate compares against the mainnet index only (SPEC Rule B9); the consumed-nonce row outlives the challenge window whatever `result_retention_seconds` is (SPEC §15.1 **Single-use**); and the policy-file encoding (SPEC 2.3 §15.4 **Policy file encoding**), which `gl-admit-v10.toml` follows: exact lever names, `leg_rate_floor` as `{ p, q }`, and decimal seconds. Under SPEC 2.3 D2 the SPEC has no lever defaults. A policy must set every lever. An absent key means unset only for `verify_cap_per_object`, `verify_cap_per_door_session` and `fake_leg_ignore_seconds`, whose ranges list unset. Any other absent key refuses the whole file, and so do an absent `policy_id`, `draw_commitment` or `born_height`. With no policy file, or a refused one, the gate admits nothing.

## 3. Behaviour SPEC 2.2 leaves to policy

These moved out of the draft SPEC at 11:32 (Rules B1 and B5, and the friendly-list custody sentences). They apply only under this policy id.

### P1. Integrity (was SPEC Rule B1)

- An `accepted` submission rests on mainnet headers and mainnet waypoints (`expected_network = "main"`). A `match` on a `synthetic` document is not a mainnet fact. SPEC §14 already records `plant` `mismatch` for a `network` that is neither `main` nor `synthetic`.
- `index_comparison` `not read`, `T` `not read` and `plant` `not read` are not, by themselves, a refusal. The response is to re-verify: `held`, then retry, inside the SPEC held bound (`reverify_max_attempts`, `max_held_blocks`, reason `held_bound`).
- A six-leg draw that is `not defined` because fewer than six legs exist is settled by the object's own bytes, so it is refused at once. A short-byte six-leg draw is `not read` and is `held`.
- A `skip` is not, by itself, a refusal. SPEC Rule B4 (`min_anchored_paths`) is the paths comparison.
- `profile` `conformance-small` is never offered to the production gate. This is now also SPEC Rule B9 (production profile only), so it no longer rests on this policy.
- The invariant refuses (shape, seed value, waypoint rehash, confirmed header or plant mismatch, checkpoint digest, six-leg) stay SPEC Rule B9. They are not policy.

### P2. Plant gap (was SPEC Rule B5)

- `d_g` is recorded and never compared under this policy. A plant `mismatch` confirmed by a fresh `{index-url}` read is a SPEC B9 refusal, not a plant-gap outcome.
- The earlier candidate `plant_gap_floor` of 1 block is not used. Making it a comparison would need a SPEC lever and a defined outcome first.

### P3. Friendly-entry custody (was the SPEC "custody candidate" sentences)

- One friendly entry per key, holding one accepted object.
- Only a thorough `accept` of a newer object replaces the entry.
  - "Newer" means a higher decision-log `seq` for that thorough `accept`. It is not the submission time or a hike's Bitcoin time.
  - A `refuse` or `revoke` of any other object does not change the entry.
  - A re-verify `accept` keeps the entry only when its object is already the key's current entry.
  - If a newer accept B later loses its accept, the older A is not restored. The replaced pair's logged decision stays unchanged.
- **Old-policy resubmit.** A pair `refused` or `revoked` under an old `policy_version` and submitted under this one starts a new admission cycle. That cycle may issue `provisional` under the same rules as any fresh cycle, and the pair may be accepted. A key on the gate's fake-leg ignore record never starts that cycle.
- **Replaced object.** A new submission of an `accepted` pair whose object a newer accept has replaced returns the logged `accepted` decision and states that this object is not the key's friendly entry. It gets no grant, no new run and no signature-only sign-in, and it does not displace the current entry.
- **Removal.** A logged manual operator strike of the key is the only other removal. Who edits the list by hand is a deployment concern. The list has no time expiry.

### P4. Draw commitment

The gate logs `H(draw ‖ salt)` at the draw and reveals it with the result, so anyone can check afterwards that the draw was not chosen against the submitter. This is a decision-log extension, not part of the SPEC.

### P5. Born height

`born_height` is **972685**. It lives in `[policy_rules]` of `gl-admit-v10.toml`. It is not a SPEC §15.4 lever, and it is not a change to Satoshi Hike 2.2. The number can change by issuing a new policy id. It does not need a spec release.

A height at or above `born_height` is production. A height below it is testing. If any stored height in the object (`beacon_height`, `last_interrupt_height`) or any claimed path block height is below `born_height`, the gate rejects the object with `before_born_height`. That decision is final. The job is terminal. It is not `held`, and waiting on the index will not change it.

The verify output shows `policy_id`, the `born_height` that applied, and `lowest_height` (the lowest offending height), next to the reason `before_born_height`. Health shows `born_height` next to `policy_id`. An object whose stored and claimed heights are all at or above `born_height` is judged as it was before this rule.

Median-time-past reads `MTP_WINDOW` headers ending at the beacon. `MTP_WINDOW` is 11 in the verifier (`n = min(MTP_WINDOW, h + 1)`). The beacon is one of those headers, so the lookback is `MTP_LOOKBACK` = `MTP_WINDOW - 1` = 10. The index and Bari serve the headers from `born_height - MTP_LOOKBACK` through `born_height - 1` so that window can be read. Those headers are for the timestamp only. They are not production heights: tip selection ignores them, and `GET /index/since` omits them. A height below `born_height - MTP_LOOKBACK` is not served. An object anchored below `born_height` is still `before_born_height`.

A caller who receives `before_born_height` should stop. The object is a testing artefact. Submit an object anchored at or above the born height. Do not retry this object.

## 4. Open

- Re-measure `leg_rate_floor` under 2.2 (SPEC §15.5 method) and issue the result as a new id.
- Confirm the carried-over figures for `reverify_max_attempts`, `max_held_blocks`, `fast_accept_bound` and `async_catch_budget`.
- Gate code must read every §15.4 lever and the §3 rules from this file. Today it reads `thorough_six_leg_runs`, and that key is renamed `thorough_rounds`.
