# PCOC principles

Approved by Christian 2026-10-06. If a proposal conflicts with these principles, the proposal is wrong.

1. **Bitcoin is the clock and the ground truth.** Anything that passes Bitcoin consensus is treated as true. Block times are Bitcoin's own header timestamps. Wall-clock hours are for presentations only.

2. **The portable index stands in for Bitcoin.** groundinglab publishes it well formed, usable, timestamped, correctly attributed and faithful to Bitcoin: height, header hash and timestamp, plus optional waypoint anchoring. Anyone can bypass it by checking Bitcoin directly. Verify measures a PCOC against the index.

3. **The PCOC producer is a notary.** It iterates through memory, gets interrupted by Bitcoin, and records according to the SPEC. That record covers the calculations, the bindings and its dealings with the Bitcoin commitment service, which is Bari: legs, interrupts, waypoints, and which waypoints were anchored and fetched in. Production is best effort. The environment causes variation, and some of it can be gamed, so the SPEC must say exactly what is recorded and when. That gives the Gate something clear to assess. The record contains no pass, fail or deny.

4. **Verify reads and surfaces measurements.** Verify reads the PCOC, recomputes it, compares it against the index, and surfaces outcomes such as match, mismatch, present or skip, plus raw values. Odd values, such as a span of zero or less, are recorded as they are.

5. **Policy judges measurements.** The groundinglab Admission Gate judges verify's measurements by groundinglab policy, such as a leg-rate floor or a paths minimum. Each variation is treated on its merits. The policy is moved around as research and development show what works as a fair defence. "Fail", "deny" and "admit" are policy words and appear nowhere else. Policy should lean toward admitting honest objects with minor distortions.

6. **Event order.** The seed (public key plus the latest header) opens stage 0, and leg 0 starts from SHA-256(seed). Stage 0 runs its legs and records checkpoints like any stage. It has no waypoint of its own and posts nothing, because the first waypoint is built only when the first interrupt arrives, and stage 0's checkpoints are folded into it. An unpredictable Bitcoin header, the interrupt, ends each stage. The waypoint is then built from that header, the terrain sample it indexes, the fold of the stage's checkpoints and the previous waypoint, and posted to Bari with a fee. The waypoint is the pivot that ends one stage and opens the next. Never say "the header arrives during the waypoint post".

7. **Bookends are a way of talking, mainly useful to policy.** The bookends are the first and last waypoints that were anchored and fetched into the proof, wherever they sit. Between them, verify can count the waypoints, the stages and the legs, and measure the Bitcoin time. Policy may judge whether that is enough. Gaps in the middle don't break the chain, because it is sequential.

8. **Skipping is recorded, not punished.** A producer may skip waypoints, whether by miss or by choice. Whether too many were skipped is a policy question. Best effort is the producer's job.

9. **Legs are a measurement.** There is no per-stage minimum or maximum leg count. Per-stage rates are for analysis only.

10. **Language. Verify surfaces measurements and policy judges them.** Be clear and explicit, and keep measurement and policy wording apart. Don't put binary pass/fail words into the record or measurement layers. Proposals get a bot round table before any SPEC change.