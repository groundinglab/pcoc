# Policy

A policy sets the [Admission Process](../spec/04-Admission_Process.md#sec-15-4) §15.4 levers to values inside their ranges. It never changes a SPEC rule. The public specification is the six-doc pack under [`spec/`](../spec/README.md), tag `spec-2.3.0`.

The in-repo successor is [groundinglab-admission/gl-admit-v11.toml](groundinglab-admission/gl-admit-v11.toml) (`leg_rate_floor` `{ p = 1, q = 10 }`). It is not live. The live gate stays on [gl-admit-v10.toml](groundinglab-admission/gl-admit-v10.toml) until an explicit go. [gl-admit-v9.toml](groundinglab-admission/gl-admit-v9.toml) is the previous id. Its status is `superseded`. `sticky_refuse` is `false` on v11 (off for testing). An absent key means the same, so v10 is off without an edit. Set the key `true` to restore the stored-refuse short circuit. See [POLICY.md](groundinglab-admission/POLICY.md).

`born_height` in the v11 file is policy, not a SPEC lever. See [POLICY.md](groundinglab-admission/POLICY.md).
