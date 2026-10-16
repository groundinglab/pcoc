# Admission policy companion

This is the policy companion to the admission gate service. It holds groundinglab's argued defaults for one policy, `gl-admit-v4` ([policy/gl-admit-v4.toml](policy/gl-admit-v4.toml)).

## Where policy sits

| Layer | What it holds | Where |
|---|---|---|
| Specification | Measurements (Part A, §1–§14) and the admission service model (Part B, §15): parameter names, comparisons, states. It may discuss thresholds. **It never sets a threshold value.** It defers values to a policy. | `spec/satoshi-hike-v2.0/` |
| Policy companion (this file) | One argued default policy. It is the out-of-the-box default for this gate build. **It is a recommendation, not authority.** Anyone may choose their own. A policy's name encodes its values: change any value and the id changes. | `services/admission-gate/POLICY.md`, `policy/` |
| Operations (private) | groundinglab's decided, deployed instance config: which policy id is active, origins, hosts, secrets. Opinionated. | Not in this repository |

The admission gate is a service. groundinglab's gate is one configured instance of it. While it is the only public gate, its public page states the live policy it runs. That position is temporary and confers no authority on these values.

## gl-admit-v4

| Parameter (§15.4 name) | Value | Argument |
|---|---|---|
| `leg_rate_floor` (B2) | `1/5` legs per second: one leg every 5 seconds | This default asks for at least one leg every 5 seconds on average across the timed span `S`, and refuses slower hikes. The comparison is integer-only on the bookend pair, `5·N ≥ S`. Threshold evidence is a follow-up (see [EVIDENCE.md](EVIDENCE.md)). |
| `min_anchored_paths` (B4) | 6 anchored among `W_1` … `W_16` | Enough waypoints to time the hike without punishing a weak connection. |
| First and last anchored | `W_1` and `W_16` must both be anchored (bookends `a = 1`, `b = 16`) | The timed span `S` then is the widest span the construction anchors (`W_1` to `W_16`), not a chosen slice. This is a policy parameter beyond the §15.4 candidates (`min_last_bookend` names only the last). |
| `thorough_six_leg_runs` (§15.4, Rule A4.4) | 3 runs, 18 leg results | Each run is a fresh draw of 6 distinct legs, uniform over every leg, from the gate's own OS CSPRNG (A4.4). Stop at the first mismatch (a B9 invariant). A hike missing 25% of its leg results is caught with probability about `1 − 0.75^18 ≈ 99.4%` (derived from the draw size; not a measured catch rate). The figure is approximate because runs may repeat an index. |
| Draw commitment | log `H(draw ‖ salt)` at draw, reveal with the result | Anyone can check afterwards that the draw was not chosen against the submitter. This is a decision-log extension, not part of the specification. |
| `provisional` (§15.3) | `on` | Cheap checks first, then a fast `provisional` grant. It is never `accept`. `provisional_action_cap` is 0 by the specification. Only the thorough phase accepts. |
| Not read (§15.3) | `held`, then retry | A missing waypoint read is likely a connection problem: "try again", not a refusal or a ban. A count settled by the object's own bytes is a plain refusal. |

### Ban rule: a leg mismatch bans the key at this gate

A six-leg or checkpoint-digest `mismatch` is an invariant `revoke` in the specification (Rule B9), scoped to `(public_key, object_sha256)`. This policy adds one rule at this gate: **the public key is banned at this gate, and a later submission from a banned key is ignored without verification.** The specification has no ban concept. This rule is groundinglab's policy, not specification. Another gate may choose otherwise.

## Choosing your own

Copy `policy/gl-admit-v4.toml`, change the values, and give it a new id that encodes them. The specification's §15.5 lists other informative examples (for instance leg-rate floors `7/20`, `1/4`, `1/8`).

## Where this policy runs

This build reads `policy/gl-admit-v4.toml`, or an inline `policy_table` with the same keys. The code default is `gl-admit-v4`. The comparison is the integer `q·N ≥ p·S` on the bookend pair. Anchored waypoints are counted among `W_1` … `W_16`, and `W_1` and `W_16` must both be anchored. The fast phase issues `provisional`. A `not read` is held and retried. Thorough runs three six-leg draws, commits the draw before walking, recomputes the checkpoint digests, and bans the public key on a fake leg. See [STATUS.md](STATUS.md).

v1 history. Through gate 0.4.1 the code default was `gl-admit-v1`: a float `min_leg_rate` of 0.25 leg/s, `min_waypoints` 6 of 18, and a named `leg_rate_floor` (`strict` 7/20, `default` 1/4, `lenient` 1/8) used only when a provisional line resumed. Those keys are not read. A resumed provisional is re-judged under the active policy id. The startup log records that move.
