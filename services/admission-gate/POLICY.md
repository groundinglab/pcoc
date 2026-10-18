# Admission policy companion

This is the policy companion to the admission gate service. It holds groundinglab's argued defaults for one policy, `gl-admit-v6` ([policy/gl-admit-v6.toml](policy/gl-admit-v6.toml)).

## Where policy sits

| Layer | What it holds | Where |
|---|---|---|
| Specification | Measurements (Part A, §1–§14) and the admission service model (Part B, §15): parameter names, comparisons, states. It may discuss thresholds. **It never sets a threshold value.** It defers values to a policy. | `spec/satoshi-hike-v2.0/` |
| Policy companion (this file) | One argued default policy. It is the out-of-the-box default for this gate build. **It is a recommendation, not authority.** Anyone may choose their own. A policy's name encodes its values: change any value and the id changes. | `services/admission-gate/POLICY.md`, `policy/` |
| Operations (private) | groundinglab's decided, deployed instance config: which policy id is active, origins, hosts, secrets. Opinionated. | Not in this repository |

The admission gate is a service. groundinglab's gate is one configured instance of it. While it is the only public gate, its public page states the live policy it runs. That position is temporary and confers no authority on these values.

## gl-admit-v6

| Parameter (§15.4 name) | Value | Argument |
|---|---|---|
| `leg_rate_floor` (B2) | `1/5` legs per second: one leg every 5 seconds | This default asks for at least one leg every 5 seconds on average across the timed span `S`, and refuses slower hikes. The comparison is integer-only on the bookend pair, `5·N ≥ S`. Threshold evidence is a follow-up (see [EVIDENCE.md](EVIDENCE.md)). |
| `min_anchored_paths` (B4) | At least 6 anchored among `W_1` … `W_16`, anywhere, counted | Enough waypoints to time the hike without punishing a weak connection. No specific waypoint is required: insisting on the first or last one would refuse many honest hikes whose final anchors had not landed, and would cost more in fees and retry logic. The object carries 18 stages; 16 of them (`W_1` … `W_16`) can be anchored. |
| `thorough_six_leg_runs` (§15.4, Rule A4.4) | 3 runs, 18 leg results | Each run is a fresh draw of 6 distinct legs, uniform over every leg, from the gate's own OS CSPRNG (A4.4). Stop at the first mismatch (a B9 invariant). A hike missing 25% of its leg results is caught with probability about `1 − 0.75^18 ≈ 99.4%` (derived from the draw size; not a measured catch rate). The figure is approximate because runs may repeat an index. |
| Draw commitment | log `H(draw ‖ salt)` at draw, reveal with the result | Anyone can check afterwards that the draw was not chosen against the submitter. This is a decision-log extension, not part of the specification. |
| `provisional` (§15.3) | `on` | Cheap checks first, then a fast `provisional` grant. It is never `accept`. `provisional_action_cap` is 0 by the specification. Only the thorough phase accepts. |
| Not read (§15.3) | `held`, then retry | A missing waypoint read is likely a connection problem: "try again", not a refusal or a ban. A count settled by the object's own bytes is a plain refusal. |

### Ban rule: a leg mismatch bans the key at this gate

A six-leg or checkpoint-digest `mismatch` is an invariant `revoke` in the specification (Rule B9), scoped to `(public_key, object_sha256)`. This policy adds one rule at this gate: **the public key is banned at this gate, and a later submission from a banned key is ignored without verification.** The specification has no ban concept. This rule is groundinglab's policy, not specification. Another gate may choose otherwise.

## gl-admit-v7 (prepared, not deployed)

[policy/gl-admit-v7.toml](policy/gl-admit-v7.toml) is `gl-admit-v6` plus one key, `[index] expected_network = "main"`. Every other value is the same.

| Parameter | Value | Argument |
|---|---|---|
| `index.expected_network` | `main` | An accepted sign-in should rest on Bitcoin mainnet headers and mainnet waypoints. The index serves a `network` field on every document. Under this policy every document the decision rests on (header reads, plant reads, and the fresh reads that confirm a mismatch) must say `"network":"main"`. Another network, or no `network` field, is refused as `index_network_not_expected` (class `invalid`): "The index evidence is not from the Bitcoin mainnet, so it cannot support admission here." |

The specification gives the verifier no network input (P14.5), so this is gate policy, not specification. The key is part of the hashed table and is written on every decision line as `expected_network`. Since gate 0.5.4 every new line also carries `index_network`, the networks of the index documents it rests on, whatever the policy. A gate built with the cargo feature `testing` does not start under this policy. Past accepts stand: a policy change does not re-judge or rewrite an earlier line.

## Choosing your own

Copy `policy/gl-admit-v6.toml`, change the values, and give it a new id that encodes them. The specification's §15.5 lists other informative examples (for instance leg-rate floors `7/20`, `1/4`, `1/8`).

## Where this policy runs

This build reads `policy/gl-admit-v6.toml`, or an inline `policy_table` with the same keys. The code default is `gl-admit-v6`. The comparison is the integer `q·N ≥ p·S` on the bookend pair. Anchored waypoints are counted among `W_1` … `W_16`; any 6 will do. A shortfall is refused as `too_few_waypoints_anchored` with the count in the line, for example `4 of 16 anchored, need 6`. The fast phase issues `provisional`. A `not read` is held and retried. Thorough runs three six-leg draws, commits the draw before walking, recomputes the checkpoint digests, and bans the public key on a fake leg. See [STATUS.md](STATUS.md).

Why v6. `gl-admit-v5` was the same table plus a rule that `W_1` and `W_16` must both be anchored (optional keys `require_first_anchored`, `require_last_anchored`, which this build still reads). v6 drops that rule. v5 is bound on groundinglab's live gate, so the change takes a new id.

Why v5, not v4. On groundinglab's live gate the id `gl-admit-v4` is already bound (policy-bind.json) to gate 0.4.1's two-value table (0.2 leg/s, 6 waypoints of 18). The full table here binds differently, so it takes a new id; a bound id is never reused with different values.

v1 history. Through gate 0.4.1 the code default was `gl-admit-v1`: a float `min_leg_rate` of 0.25 leg/s, `min_waypoints` 6 of 18, and a named `leg_rate_floor` (`strict` 7/20, `default` 1/4, `lenient` 1/8) used only when a provisional line resumed. Those keys are not read. A resumed provisional is re-judged under the active policy id. The startup log records that move.
