# Leg-rate floor under SPEC 2.2: the calibration

**Policy:** groundinglab admission policy `gl-admit-v9` (draft), lever `leg_rate_floor = { p = 1, q = 5 }`, which is 0.2 legs per second.
**Evidence:** [../round-2/results/](../round-2/results/) (SPEC 2.2 + erratum 1, pcoc `7b40353`).
**Status:** a judgment call based on the evidence so far, not a proof. The full beacon-paced hike confirmation is open (below).

## What the SPEC fixes and what the policy chooses

- SPEC section 15.4 defines the lever: `q · hike_legs ≥ p · hike_span`, integers only, `p ≥ 1`, `q ≥ 1`, labelled default `1/5`.
- SPEC section 15.5 gives the measuring method. Measure on owned commodity hosts with a conforming implementation. Calibrate on the best legal packed rate per machine (N parallel PCOCs with distinct keys and terrains). Set the floor a little below the low-end host's rate. The low-end host is the Intel Core i7-10610U, which is reference system 2.
- The policy picks the value. Another service can pick another value inside the range.

## The numbers

Best single hike (one hike on the machine, legs/s): Reference system 1 0.402, reference system 3 0.375, reference system 2 0.242.

Packed (N independent hikes at once; per-hike median / min, aggregate):

| Machine | N | Per hike | Aggregate | At or above 0.2? |
|---|---|---|---|---|
| Reference system 1 | 1 | 0.402 / 0.402 | 0.402 | yes |
| Reference system 1 | 4 | 0.283 / 0.282 | 1.133 | yes |
| Reference system 1 | 6 | 0.190 / 0.184 | 1.140 | no |
| Reference system 3 | 1 | 0.375 / 0.375 | 0.375 | yes |
| Reference system 3 | 4 | 0.246 / 0.242 | 0.985 | yes |
| Reference system 3 | 6 | 0.170 / 0.163 | 1.015 | no |
| Reference system 2 | 1 | 0.242 / 0.242 | 0.242 | yes |
| Reference system 2 | 2 | 0.238 / 0.238 | 0.477 | yes |

Producer: build C (Zig), its normal fast engine, except on reference system 2, where its scalar engine is faster (fast reads 0.134). Every counted instance's last leg was recomputed by build D (Kotlin) and matched.

## Derivation

1. **Low-end host.** Reference system 2's best legal rate is 0.242 single and 0.238 per hike at N = 2. A floor of 0.2 is about 17 % below the single rate and still below the N = 2 rate. It meets the section 15.5 rule "a little below the low-end host's rate". Reference system 2 is the older, pricier machine, and it can make two PCOCs at once and stay above the floor.
2. **Packing hits memory bandwidth.** Each hike needs its own 256 MiB terrain, which no cache on these machines holds. On both six-core Ryzen laptops the aggregate levels off at about 1.0–1.14 legs/s from N = 4 on. Adding hikes only splits a fixed bandwidth. At N = 6 both fall below 0.2.
3. **Hikes per machine at the floor.** 1.14 / 0.2 = 5.7, so reference system 1 supports at most 5 hikes at once at the floor; reference system 3 (1.015 / 0.2 = 5.1) the same.
4. **Value.** Keep `1/5`, calibrated on reference system 2, the SPEC's low-end host, with reference system 1 as the primary reference.

## Faking legs

A fake leg is a leg whose result was never computed. The deep check redoes 18 randomly drawn legs (3 rounds of 6). The draw comes after the object is committed, and every leg starts from the previous checkpoint, so a fake cannot be patched afterwards. The chance of passing with a fraction `f` of legs faked is about `(1 − f)^18`:

| Fraction faked | Work saved, at best | Caught |
|---|---|---|
| 1 % | about 1 % | 16.5 % |
| 5 % | about 5 % | 60.3 % |
| 10 % | about 10 % | 85.0 % |
| 25 % | about 25 % (at best one extra PCOC for every three) | 99.44 % |
| 50 % | about 50 % | 99.9996 % |

One catch refuses the object, and that gate ignores the key from then on (`fake_leg_ignore_seconds` unset: no expiry). The pace is also measured against Bitcoin's block times, which no one can predict, so the saving from a few fakes cannot be banked ahead of time. Optimising, or running real hikes in parallel, fulfils the contract; faking does not.

## Open

- Confirm the floor with full, real beacon-paced hikes (`hike_legs / hike_span` per SPEC Rule A4.10) on reference system 2 and reference system 1. The figures above are steady 60-second leg-rate windows. This is open, not blocking.
- Publish the policy under its final id and pin this note to that id's TOML SHA-256.
