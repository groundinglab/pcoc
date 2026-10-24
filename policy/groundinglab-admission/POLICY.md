# groundinglab admission policy gl-admit-v11

- **Status.** A policy file's `status` is one of `draft`, `active`, or `superseded`. `gl-admit-v11` is `active` in this repository. The live gate stays on `gl-admit-v10` until an explicit go (#115). This change does not deploy. `gl-admit-v10.toml` keeps `status` `active` until that swap, because that file is the live id and its bytes stay pinned. After the swap it becomes `superseded`. `gl-admit-v9` is `superseded`: it was live until 2026-10-23.
- **Repo path (agreed layout).** `policy/groundinglab-admission/POLICY.md`, next to [`gl-admit-v11.toml`](gl-admit-v11.toml). [`gl-admit-v10.toml`](gl-admit-v10.toml) and [`gl-admit-v9.toml`](gl-admit-v9.toml) stay in this directory.
- **Targets.** Satoshi Hike 2.3, tag `spec-2.3.0`. The public face is the six-doc pack under [`spec/`](../../spec/README.md) ([GUIDE](../../spec/GUIDE.md)). Admission levers are [Admission Process §15.4](../../spec/04-Admission_Process.md#sec-15-4). This document does not target `spec/satoshi-hike-v2.2/SPEC.md` or `spec/satoshi-hike-v2.3/SPEC.md`.
- **Policy version.** The `policy_version` is the id `gl-admit-v11` plus the SHA-256 of `gl-admit-v11.toml`. Ops pins that SHA-256 in the deployment manifest at deploy time. There is no deploy in this change.

## Changes from gl-admit-v10 (Christian, 24 Oct 2026)

| Field | v10 | v11 |
|---|---|---|
| `leg_rate_floor` | `{ p = 1, q = 5 }` (0.2 legs/s) | `{ p = 1, q = 10 }` (0.1 legs/s) for testing; retune after more produces (#115) |
| `spec` | `"2.2"` | `"2.3"` |
| `submission_deadline_blocks` | set on v10 | omitted. [Admission Process](../../spec/04-Admission_Process.md) defines no tip-versus-last-beacon submit deadline (#119). It is not a §15.4 lever. |
| `sticky_refuse` | no key (the gate always returned a stored refuse) | `false` in `[policy_rules]`. Off for testing. An absent key means the same. |

`min_anchored_paths` stays 6. `born_height` stays 972685. Every other §15.4 lever is carried from v10.

A policy sets the Admission Process §15.4 levers to values inside their ranges. It never changes a SPEC rule. Section 3 holds the behaviour that process leaves to policy. Changing any value or rule here needs a new id; ids are never reused. `gl-admit-v9.toml` is the id before v10. Its `status` is `superseded`. Earlier ids are kept only in ops `policy-bind.json`.

## 1. Levers this policy sets

| Lever ([Admission Process §15.4](../../spec/04-Admission_Process.md#sec-15-4)) | Value | Source | Argument |
|---|---|---|---|
| `min_anchored_paths` | 6 | carried from v10 | Enough anchored waypoints to time the hike without punishing a weak connection. They can be anywhere among `W_1` … `W_16`. |
| `leg_rate_floor` | `1/10` | measured hike ≈0.14; floor 0.1 for testing (#115) | Integer compare `q · hike_legs ≥ p · hike_span`. Retune after more produces. |
| `held_retry_seconds` | 600 | carried from v10 | Retry a held submission every 10 minutes. |
| `thorough_rounds` | 3 | carried from v10 | 18 legs in 3 rounds of 6, which detects a 25 % fake fraction 99.44 % of the time. |
| `challenge_lifetime_seconds` | 120 | carried from v10 | The website mints with the same value. |
| `challenge_clock_skew_seconds` | 5 | carried from v10 | |
| `result_retention_seconds` | 600 | carried from v10 | |
| `reverify_max_attempts` | 3 | carried from v10 | Earlier `default`-profile figure. |
| `max_held_blocks` | 6 | carried from v10 | Earlier `default`-profile figure. |
| `provisional` | `on` | carried from v10 | Cheap checks first, then a short read-only grant that is never `accept`. |
| `fast_accept_bound` | 2 s | carried from v10 | Earlier `default`-profile figure. |
| `async_catch_budget` | 22.05 s | carried from v10 | Times the parallel checkpoint-digest pass. It does not include recomputing legs. |
| `verify_cap_per_object` | unset | carried from v10 | The earlier candidate was 1. Set it if compute abuse appears. |
| `verify_cap_per_door_session` | 3 | carried from v10 | The range lists unset. This file sets 3. The gate reads the key. |
| `fake_leg_ignore_seconds` | unset (no expiry) | carried from v10 | A fake leg ignores that key at this gate with no expiry. |

`submission_deadline_blocks` is not a lever v11 sets. SPEC 2.3 dropped the tip-versus-last-beacon deadline (old Rule B10), and this file omits the key. `gl-admit-v9.toml` and `gl-admit-v10.toml` still contain it, so those files stay the bytes of the ids they already are. The gate ignores the key when it is present and does not require it when it is absent. It does not refuse with `submission_deadline`.

## 2. Fixed by the SPEC, not set here

`provisional_action_cap`, `revoke_scope`, the 6-leg round, the Rule B9 invariant refuses, and the fake-leg ignore itself are fixed by [Admission Process](../../spec/04-Admission_Process.md). So are: admission reads only a `production`-profile record, and a live-plane gate compares against the mainnet index only (Rule B9); the consumed-nonce row outlives the challenge window whatever `result_retention_seconds` is (§15.1 **Single-use**); and the policy-file encoding (§15.4 **Policy file encoding**), which `gl-admit-v11.toml` follows: exact lever names, `leg_rate_floor` as `{ p, q }`, and decimal seconds. Admission Process gives no lever a default. A policy must set every lever the SPEC still lists. An absent key means unset only for `verify_cap_per_object`, `verify_cap_per_door_session` and `fake_leg_ignore_seconds`, whose ranges list unset. `submission_deadline_blocks` is not in that list and is not required: the gate ignores it. Any other absent lever refuses the whole file, and so do an absent `policy_id`, `draw_commitment` or `born_height`. A key in `[levers]` that is not a §15.4 name refuses the whole file, except that retired key, which the gate ignores so `gl-admit-v9` and `gl-admit-v10` still load. With no policy file, or a refused one, the gate admits nothing.

Anchored-path inclusion is Bari's anchor window (offsets 2, 3, and 4). This policy sets no tip-versus-last-beacon submit deadline.

## 3. Behaviour the admission process leaves to policy

These moved out of the draft SPEC at 11:32 (Rules B1 and B5, and the friendly-list custody sentences). They apply only under this policy id.

### P1. Integrity (was SPEC Rule B1)

- An `accepted` submission rests on mainnet headers and mainnet waypoints (`expected_network = "main"`). A `match` on a `synthetic` document is not a mainnet fact. [Measurement Record](../../spec/06-Measurement_Record_Process.md) section 14 already records `plant` `mismatch` for a `network` that is neither `main` nor `synthetic`.
- `index_comparison` `not read`, `T` `not read` and `plant` `not read` are not, by themselves, a refusal. The response is to re-verify: `held`, then retry, inside the held bound (`reverify_max_attempts`, `max_held_blocks`, reason `held_bound`).
- A six-leg draw that is `not defined` because fewer than six legs exist is settled by the object's own bytes, so it is refused at once. A short-byte six-leg draw is `not read` and is `held`.
- A `skip` is not, by itself, a refusal. [Admission Process Rule B4](../../spec/04-Admission_Process.md#rule-b4) (`min_anchored_paths`) is the paths comparison.
- `profile` `conformance-small` is never offered to the production gate. This is also [Admission Process Rule B9](../../spec/04-Admission_Process.md#rule-b9) (production profile only), so it no longer rests on this policy.
- The invariant refuses (shape, seed value, waypoint rehash, confirmed header or plant mismatch, checkpoint digest, six-leg) stay Admission Process Rule B9. They are not policy.

### P2. Plant gap (was SPEC Rule B5)

- `d_g` is recorded and never compared under this policy. A plant `mismatch` confirmed by a fresh `{index-url}` read is a Rule B9 refusal, not a plant-gap outcome.
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

`born_height` is **972685**. It lives in `[policy_rules]` of `gl-admit-v11.toml`. It is not an Admission Process §15.4 lever, and it is not a change to the Satoshi Hike 2.3 pack. The number can change by issuing a new policy id. It does not need a spec release.

A height at or above `born_height` is production. A height below it is testing. If any stored height in the object (`beacon_height`, `last_interrupt_height`) or any claimed path block height is below `born_height`, the gate rejects the object with `before_born_height`. That decision is final. The job is terminal. It is not `held`, and waiting on the index will not change it.

The verify output shows `policy_id`, the `born_height` that applied, and `lowest_height` (the lowest offending height), next to the reason `before_born_height`. Health shows `born_height` next to `policy_id`. An object whose stored and claimed heights are all at or above `born_height` is judged as it was before this rule.

Median-time-past reads `MTP_WINDOW` headers ending at the beacon. `MTP_WINDOW` is 11 in the verifier (`n = min(MTP_WINDOW, h + 1)`). The beacon is one of those headers, so the lookback is `MTP_LOOKBACK` = `MTP_WINDOW - 1` = 10. The index and Bari serve the headers from `born_height - MTP_LOOKBACK` through `born_height - 1` so that window can be read. Those headers are for the timestamp only. They are not production heights: tip selection ignores them, and `GET /index/since` omits them. A height below `born_height - MTP_LOOKBACK` is not served. An object anchored below `born_height` is still `before_born_height`.

A caller who receives `before_born_height` should stop. The object is a testing artefact. Submit an object anchored at or above the born height. Do not retry this object.

### P6. Sticky refuse (off for testing)

[Admission Process](../../spec/04-Admission_Process.md) §15.2 says a pair closed `refused` or `revoked` under this policy version returns that outcome and does not open a new cycle. The gate's short circuit for that rule scans the chain for the same `public_key`, `object_sha256`, and `policy_id` with decision `refuse` or `revoke`, settles the nonce with the stored reason, and writes no new line.

That short circuit is off. `[policy_rules] sticky_refuse` is `false` in [`gl-admit-v11.toml`](gl-admit-v11.toml). An absent key means the same. `gl-admit-v9.toml` and `gl-admit-v10.toml` do not name it, and their bytes stay pinned. While it is off, a later submission of a refused pair runs the fast phase again. If the object is still below the floor, the gate appends a new refuse line. The same reason text is a fresh decision.

Re-enable later by setting `sticky_refuse = true` in `[policy_rules]` of the policy file the process loads. The key is not a §15.4 lever. A value that is not a boolean refuses the file. The value is not part of the policy bind hash, so the flip does not need a new id and does not make a start fail against an existing `policy-bind.json`. This change does not deploy.

## 4. Open

- Re-measure `leg_rate_floor` after further produces and issue the result as a new id (#115). The testing value is `{ p = 1, q = 10 }`.
- Confirm the carried-over figures for `reverify_max_attempts`, `max_held_blocks`, `fast_accept_bound` and `async_catch_budget`.
- Swap the live gate only after an explicit go. This repository change does not deploy, and it does not edit ops.
- `sticky_refuse` stays `false` while the same object is resubmitted for testing. Set it `true` when the stored-refuse short circuit should return.
- The gate strip landed in #120. The loader accepts this file with the key omitted, and it ignores the key on v9 and v10. The live process keeps `gl-admit-v10` until an explicit go.
