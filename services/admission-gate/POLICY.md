# Admission policy companion

This is the policy companion to the admission gate service. It holds groundinglab's argued defaults for one policy, `gl-admit-v4` ([policy/gl-admit-v4.toml](policy/gl-admit-v4.toml)).

## Where policy sits

| Layer | What it holds | Where |
|---|---|---|
| Specification | Measurements (Part A, §1–§14) and the admission service model (Part B, §15): parameter names, comparisons, states. It may discuss thresholds. **It never sets a threshold value.** It defers values to a policy. | `spec/satoshi-hike-v2.0/` |
| Policy companion (this file) | One argued default policy. The gate ships tuned to it so that gates interoperate out of the box. **It is a recommendation, not authority.** Anyone may choose their own. A policy's name encodes its values: change any value and the id changes. | `services/admission-gate/POLICY.md`, `policy/` |
| Operations (private) | groundinglab's decided, deployed instance config: which policy id is active, origins, hosts, secrets. Opinionated. | Not in this repository |

The admission gate is a service. groundinglab's gate is one configured instance of it. While it is the only public gate, its public page states the live policy it runs. That position is temporary and confers no authority on these values.

## gl-admit-v4

| Parameter (§15.4 name) | Value | Argument |
|---|---|---|
| `leg_rate_floor` (B2) | `1/5` legs per second: one leg every 5 seconds | If you cannot do a leg in under 5 seconds, you are too slow. The comparison is integer-only on the bookend pair, `5·N ≥ S`. Threshold evidence is in the gate's evidence note. |
| `min_anchored_paths` (B4) | 6 anchored among `W_1` … `W_16` | Enough waypoints to time the hike without punishing a weak connection. |
| First and last anchored | `W_1` and `W_16` must both be anchored (bookends `a = 1`, `b = 16`) | The timed span `S` then covers the whole hike, not a chosen slice. This is a policy parameter beyond the §15.4 candidates (`min_last_bookend` names only the last). |
| `thorough_six_leg_runs` (§15.4, Rule A4.4) | 3 runs, 18 leg results | Each run is a fresh draw of 6 distinct legs, uniform over every leg, from the gate's own OS CSPRNG (A4.4). Stop at the first mismatch (a B9 invariant). A hike that skipped 25% of its legs is caught with probability about `1 − 0.75^18 ≈ 99.4%`. The figure is approximate because runs may repeat an index. |
| Draw commitment | log `H(draw ‖ salt)` at draw, reveal with the result | Anyone can check afterwards that the draw was not chosen against the submitter. This is a decision-log extension, not part of the specification. |
| `provisional` (§15.3) | `on` | Cheap checks first, then a fast `provisional` grant. It is never `accept`. `provisional_action_cap` is 0 by the specification. Only the thorough phase accepts. |
| Not read (§15.3) | `held`, then retry | A missing waypoint read is likely a connection problem: "try again", not a refusal or a ban. A count settled by the object's own bytes is a plain refusal. |

### Gate rule of this policy: a fake leg is a banned key

A six-leg or checkpoint-digest `mismatch` is an invariant `revoke` in the specification (Rule B9), scoped to `(public_key, object_sha256)`. This policy adds one rule at this gate: **the public key is banned at this gate, and a later submission from a banned key is ignored without verification.** The specification has no ban concept. This rule is groundinglab's policy, not specification. Another gate may choose otherwise.

## Choosing your own

Copy `policy/gl-admit-v4.toml`, change the values, and give it a new id that encodes them. The specification's §15.5 lists other informative examples (for instance leg-rate floors `7/20`, `1/4`, `1/8`).

## What gate v0.4.1 enforces today

This pass changes documentation only. The running build reads `policy_id`, `min_leg_rate` and `min_waypoints` from `gate.toml` (code default `gl-admit-v1`: 0.25, 6). It does not yet implement the first/last rule, three runs with early stop, the draw commitment, `held` for an unread closing header (it refuses `bookends_not_read`), or the key ban. See [STATUS.md](STATUS.md).
