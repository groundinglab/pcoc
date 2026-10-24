# Framing review — split-six vs locked frame (2026-10-24)

**Status.** Framing locked with Christian 2026-10-24. **Boundary relocation applied** the same day (cut-paste homes; framing notes; no new object rules). GB review on #119: **PASS-with-nits** (framing only). Developer reader paths: [GUIDE.md](../../../spec/GUIDE.md). See [INTENT.md](../../../spec/INTENT.md) and [GB-REVIEW-BRIEF.md](./GB-REVIEW-BRIEF.md).

**How to read this file.** Sections **01–06** below keep historical **(b)/(c)** notes from *before* the relocate commit `8115a6c`. They are **pre-relocation drift notes**, not a claim that the current pack still mixes those homes. **Post-relocation status** for each doc is the short line under the section heading. Cross-cutting and nits at the bottom describe the pack **after** relocate (+ GUIDE / FRAMING nits).

**Locked frame (summary).**

- **Construction + Conformance** = definitional pair (specification + matching test specification).
- **Design time:** build a producer and a verifier against that pair.
- **Runtime:** producer emits → verifier measures (validate with Conformance, then report quantities) → admission applies a **policy instance**.
- **Language:** **valid** → **measured** → **admitted** (do not use “verified” loosely).
- **Waypoint Publication** = our scoped Bitcoin-anchor channel (batching). **Construction** owns the two-way Bitcoin bind; direct-to-Bitcoin is out of scope here but allowed as a different mechanism.
- **Admission Process** = generic, value-free consumer of a measurement record + a policy; `gl-admit-*` stays **outside** the pack.
- **Tip-versus-last-beacon submit deadline** is not part of this specification. Bari fetchability is the include window. That deadline was removed from the public pack.

---

## 01 — Satoshi Hike Construction

**Post-relocation.** Object + Bitcoin bind; §14 measure moved to Measurement; status prose points admission out. Hardcoded “6” in classification A4.5.2 kept with FRAMING nit (B4 lever owns the value).

**(a) Locked frame says.** Definition of the object and its cost: terrain, legs, stages, waypoint *slots*, encodings, and the two-way Bitcoin commitment bind (headers observed into the hike; anchors that make history observe the hike). No live produce steps, no thresholds, no judgment.

**(b) Pre-relocation note (historical).** Responsibility line already said object + encodings **and the measurement procedure**, and the source list includes **§14 Verify**. Status prose still summarises Part B admission levers and judgment. So Construction currently mixes **definition** with **how to measure**.

**(c) Pre-relocation recommended boundary (done).** Keep Construction as the object definition only. Move §14 measurement procedure out (into Measurement / verifier design-time material). Leave Bitcoin bind here; leave Bari *mechanism* to Waypoint Publication. Rename optional: drop “Satoshi Hike” from the pack title later if the pack becomes “PCOC Construction” — not required for framing.

**Design-time vs runtime blur.** Low if §14 is extracted; high today because “measure” lives inside the definitional doc.

---

## 02 — Producer Process

**Post-relocation.** Live sequence retained; §13.6 points at Waypoint; FRAMING stub also points path-body layout to Measurement §14.3; Bari-skip sentences have FRAMING pointers to out-of-scope direct-to-Bitcoin.

**(a) Locked frame says.** Live sequence that *realises* Construction: observe tip → hike → plant/collect when the publication channel allows → close. Depends on Construction. Does not redefine the object.

**(b) Pre-relocation note (historical).** Matched the live sequence well (§13 Produce, Rule A1, close at `I_17`, async posts/asks). It also inlines Bari rules (§13.6) and Hashcash in full — overlap with Waypoint Publication. Anchor window offsets (2, 3, 4) correctly sit on the produce side as “when to ask.”

**(c) Pre-relocation recommended boundary (done).** Keep produce-side event order and ask timing here; point to Waypoint Publication for operator/pool/serve rules instead of duplicating §13.6. No rename needed.

**Design-time vs runtime blur.** Mostly runtime process text used at design time to implement a producer — acceptable if Conformance (not this file) is the design-time pass/fail pack.

---

## 03 — Waypoint Publication Process

**Post-relocation.** BR1–BR4 only; plant-measure removed; FRAMING states Construction bind + batching scope + Measurement path-body home.

**(a) Locked frame says.** Our **scoped** open batching path: hashcash → pool → batch → confirm → fetch window (Bari-shaped). Sibling under Construction for the **anchor** direction of the Bitcoin bind. Not optional security; scoped mechanism. Direct bind is out of scope but not forbidden.

**(b) Pre-relocation note (historical).** Responsibility fitted (tree-batching). Body correctly has BR1–BR4. **Drift:** it also pulls large chunks of **§14.3** path/plant *measurement* (verifier procedure against the portable index). That is Measure, not Publication.

**(c) Pre-relocation recommended boundary (done).** Trim to accept / pool / seal / serve (and path *wire* format if needed for producers and Bari). Move plant-vs-index measurement to Measurement (or keep only in Construction until Measurement owns §14). Add a short scope note (when body edits are allowed): Construction requires the bind; this document is the batching channel we specify.

**Design-time vs runtime blur.** Publication is an operational channel; measurement text inside it blurs Measure into Publication.

---

## 04 — Admission Process

**Post-relocation.** Value-free framing. The tip-versus-last-beacon submit deadline was removed from the public pack. Lever values stay in policy.

**(a) Locked frame says.** Generic, **value-free**: how a gate consumes a measurement record + a policy instance → accept / refuse / log. No groundinglab numbers. Policy instances (`gl-admit-*`) live outside the SPEC pack. Gate *service* runbooks stay ops.

**(b) Pre-relocation note (historical).** Already stated that this file sets **no lever defaults** and that a policy is a separate document — good. **Drift:** it still embeds the full Gate state machine, sign-in wire, groundinglab Gate as the running example, lever **ranges** including **`submission_deadline_blocks` (K) / Rule B10**, and decision vocabulary tied to that service shape. That is far thicker than “generic consumer.”

**(c) Pre-relocation recommended boundary (deadline later removed from the public pack; thin split still later).** Split mentally: (1) thin Admission Process in the pack; (2) policy instance outside; (3) Gate deployment docs in ops. **Tip-versus-last-beacon submit deadline:** under the locked frame this is the wrong job (Bari fetchability is the include window; ≥6 anchors is a policy value). It was later removed from Admission and from Conformance (the old F16 row). No replacement deadline.

**Design-time vs runtime blur.** High: normative Gate runtime behaviour and design-time “how to build a gate” are the same huge §15.

---

## 05 — Conformance Process

**Post-relocation.** FRAMING states the definitional pair. The old F16 row left with the tip deadline. Responsibility line aligned to “test-specification half of the pair”.

**(a) Locked frame says.** Test specification paired with Construction: yes/no rules + vectors. Design-time for **both** producer and verifier authors; same rules used at runtime by a verifier to confirm validity before measuring quantities. Admission is **not** a conformance target.

**(b) Pre-relocation note (historical).** Aligned well: layers producer / verifier / “admission not a target”; one pack; permanent negatives; Rules I1–I6. **Drift:** §17 clarifications and §19 fixed issues still narrate admission (including **F16**, which treats “no submission deadline” as a defect and points at Rule B10). That couples the test pack to the contested Judge rule. Also titled “Process” while it is really the **test specification** half of the definitional pair.

**(c) Pre-relocation recommended boundary (mostly done).** Keep as Conformance (or rename to **Conformance** / **Conformance Pack** without “Process”). Present explicitly as hand-in-hand with Construction. When K is retired from Judge, revisit F16 so Conformance does not permanently encode the tip deadline.

**Design-time vs runtime blur.** Document already covers both audiences; README should say so explicitly (glossary/roles do).

---

## 06 — Measurement Record Process

**Post-relocation.** Owns full §14 procedure + verify-record shape + §18 CLI; fact-only.

**(a) Locked frame says.** After validity: report quantities only (beacons folded, legs, anchored paths, times, etc.). No thresholds. Serialisation of that fact record. Depends on Construction; may interpret Waypoint Publication evidence when that channel was used.

**(b) Pre-relocation note (historical).** Was mostly §18 Verify CLI + verify-record JSON shape. The **procedure** that fills the record still lives in Construction (§14) and spills into Waypoint Publication. So this file is “record + CLI,” not the full Measure job.

**(c) Pre-relocation recommended boundary (done).** Own both procedure and serialisation under Measurement (pull §14 from Construction). Rename toward **Measurement** / **Measurement Record** (drop “Process” if it reads as Gate-like). Keep: no accept/refuse language (already true for CLI).

**Design-time vs runtime blur.** CLI is runtime tooling; vectors that pin measurements are design-time — Conformance should remain the place vectors are claimed.

---

## Cross-cutting (post-relocation + GUIDE)

| Topic | Finding |
|---|---|
| Definitional pair | README and GUIDE state Construction + Conformance as the definitional pair. |
| Reader paths | [GUIDE.md](../../../spec/GUIDE.md) gives producer / verifier / gate / policy-tuner document lists. README points at GUIDE. |
| Tip deadline | Removed from the public pack. Bari’s anchor window is the include constraint. No replacement deadline. |
| Hardcoded “6” vs B4 | Pre-existing in ratified 2.3. Construction A4.5.2 and one Admission `received` state cell say “6”; Rule B4 uses lever `min_anchored_paths`. **FRAMING nits only** on those sentences — wording left. |
| Bari-skip vs direct Bitcoin | Producer §13.1 / §13.5 “does not define a path that skips Bari” + Waypoint/glossary out-of-scope direct-to-Bitcoin agree; FRAMING pointers link them. Inherited sentences unchanged. |
| Verbatim split | Bodies remain section lifts; relocation is cut-paste of homes plus framing notes. Glossary/INTENT/README/GUIDE state the locked frame. |

## Pack picture (locked)

```
Construction  ↔  Conformance     ← definitional pair
       ↑                ↑
   Producer        Measurement     ← realise / read the object
       \                |
        Waypoint Publication       ← scoped anchor channel (batching)
                        |
                 Admission Process ← generic consumer
                        ↑
              policy instance (outside pack, e.g. gl-admit-*)
```

