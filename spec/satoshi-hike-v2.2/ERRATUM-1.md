# SPEC 2.2 erratum 1: exact changes

**22 Oct 2026 (ACDT).** 29 clarifications to `spec/satoshi-hike-v2.2/SPEC.md` (main 2b50df7), from the six-build interop round. Christian approved the changes in principle; White Hat reviewed them (one MUST-FIX, C-S12, applied by withdrawing E22–E24 to the separate 2.3 queue; everything else READY). Paths below to `tools/`, `ref/`, `round1/`, `review/` and `2.3-QUEUE.md` name files in the interop working tree; they are not part of this repository.

**Base and result**
- Base: pcoc `main` 2b50df7, `spec/satoshi-hike-v2.2/SPEC.md`, sha256 `3055ef488bf761c198894a13b433c3f2d67a26c9ad73e937abfa4b1619e66319` (2181 lines).
- Result: `interop/erratum/SPEC.md`, sha256 `8e591ea02e7c73ac3d1a4e61602f25baaae88cabdc482240d310d2629dfa5889` (2197 lines). Unified diff: `SPEC.diff`.
- Every edit below is an exact string operation applied by `tools/build.py` from `tools/edits.py`. Each `old` string occurs exactly once in main; the build aborts otherwise. Line numbers are main's (L…) and the erratum's (E-L…).
- The SPEC stays a standalone v2.2 document. No edit mentions a previous version; §19 stays the only section that names earlier defects.

**Christian's calls, as applied.** C-S14 option A (delete the median-time previous-hash clause). C-S9 + C-S18: `final_stage_hoist` added as a new negative and `prefold_hoist` redefined as the per-slot summary. C-S8 dropped. C-S12 (each shape fault once, under its own string) is Christian's call too, but it is normative and moves to 2.3 (`2.3-QUEUE.md` Q01); C-S10 also moves there (Q02).

**White Hat review, applied.** E22–E24 (C-S12) are withdrawn: main L1303 (“A condition that is violated is not dropped because another condition is also violated.”) fixes `shape_item` here, so single reporting changes bytes a conforming 2.2 build must emit. Their IDs stay unused so E01–E32 match the review. The E22 checkpoint and leg-result width exclusions are withdrawn with them. **For 2.2, builds follow main L1303 and report both strings** (stage-0 `path` key: `field set` and `stage-0 path key`; a `last_interrupt` that is not 32 bytes: `field set` and `last_interrupt`). No other erratum edit depended on E22–E24 (checked: §14 shape table, §14.1 Rule A4.1 bullets, E14, E21, E25, and CASES-v2.md).

## Version call

All 29 applied edits are **clarifications** (White Hat: READY) under the class rule (no byte or outcome of any conforming output changes): no edit touches §10–§13, every honest construction value from round 1 (266/266 keys) stays valid, and each edit either defines something the SPEC named but did not define, removes a contradiction, or picks one of two readings the old text allowed. **Nothing here forces 2.3.** It can land as SPEC 2.2 erratum 1 before the `spec-2.2.0` tag (L2073: no tag until every SDK passes the pack).

Two items sit closest to the line; White Hat ruled both erratum-class:
1. **C-S11 follow-ons (E19–E21).** For one input, malformed CBOR that still carries a non-`KIND` kind text, the door's website outcome is now `rejected` (Rule B9 shape refuse), where a reader of old step 8 could have answered `unknown_kind`. Both are refusals; only the word on the website differs.
2. **C-S3 (E12).** Rule I2 now requires SDKs to expose the thorough draw and the integer rate and anchor comparisons. No byte changes; it adds interface obligations the pack already needed.

Everything normative is in `2.3-QUEUE.md`: C-S12, C-S10 with White Hat's fail-closed condition, the two optional negative siblings, and the three vectorisation items.

## Summary

| Edit | Candidate | Section | Main line | Class | Round-1 status |
|---|---|---|---|---|---|
| E01 | C-S1, C-S9, C-S18, C-S19 | §16 Vectors, Permanent negatives | L2067 | clarification | CONFIRMED |
| E02 | C-S9 | §19 row F3 | L2168 | clarification | CONFIRMED |
| E03 | C-S1 | §19 row F4 | L2169 | clarification | CONFIRMED |
| E04 | C-S2 | §4 Glossary (new row after “Outcome token”) | L323 | clarification | CONFIRMED |
| E05 | C-S2 | §15.4 Rule B2 | L1973 | clarification | CONFIRMED |
| E06 | C-S2 | §15.4 Rule B4 | L1985 | clarification | CONFIRMED |
| E07 | C-S2 | §15.4 Rule B10 | L1977 | clarification | CONFIRMED |
| E08 | C-S17 | §16.1 Legs (last sentence) | L2096 | clarification | CONFIRMED |
| E09 | C-S3 | §16.1 (new paragraph **Thorough rounds**) | new text (after E08) | clarification | CONFIRMED |
| E10 | C-S7 | §16.1 (new paragraph **Anchor comparison**) | new text (after E09) | clarification | CONFIRMED |
| E11 | C-S4, C-S3, C-S17 | §16.1 Draw bytes | L2098 | clarification | CONFIRMED, UNTESTED |
| E12 | C-S3 | §16.2 Rule I2 | L2116 | clarification | CONFIRMED |
| E13 | C-S5 | §7.3 CBOR | L494 | clarification | CONFIRMED |
| E14 | C-S11 | §7.3 CBOR (decode order) | L496 | clarification | CONFIRMED |
| E15 | C-S6 | §14.7 Rule A5 (serialization) | L1670 | clarification | CONFIRMED |
| E16 | C-S11 | §14.7 Rule A5 (last sentence) | L1670 | clarification | CONFIRMED |
| E17 | C-S11 | §18 Verify CLI | L2158 | clarification | CONFIRMED |
| E18 | C-S11 | §1 Part A rule classes, row A5 | L228 | clarification | CONFIRMED |
| E19 | C-S11 | §15.1 Check order, step 8 | L1801 | clarification | CONFIRMED |
| E20 | C-S11 | §15.1 Check order, step 9 | L1802 | clarification | CONFIRMED |
| E21 | C-S11 | §15.1 Check order, step 10 | L1803 | clarification | CONFIRMED |
| E25 | C-S13 | §14.1 Rule A4.1 (height_range bullet) | L1359 | clarification | UNTESTED |
| E26 | C-S14 (option A) | §14.4 Rule A4.10 Median-time-past | L1586 | clarification | CONFIRMED |
| E27 | C-S15 | §14.4 Rule A4.11 step 1 | L1602 | clarification | UNTESTED |
| E28 | C-S15 | §14.4 Rule A4.11 step 2 | L1604 | clarification | UNTESTED |
| E29 | C-S15 | §1 Part A rule classes, row A4.11 | L223 | clarification | UNTESTED |
| E30 | C-S15 | §14 Verify record, `fixed_start` (first use) | L1333 | clarification | UNTESTED |
| E31 | C-S15 | §14 Verify record, `fixed_start` (second use) | L1333 | clarification | UNTESTED |
| E32 | C-S16 | §14 Verify record, `index_comparison_P_b` | L1331 | clarification | UNTESTED |
| — | C-S8 | §10.4 L788 slot 0 wording | L788 | (dropped) | SUPPORTED |
| (E22–E24) | C-S12 | §14 shape row 2, L1303 rule, Rule A4.1 bullet | L1289, L1303, L1348 | withdrawn: normative (White Hat MUST-FIX) → 2.3-QUEUE Q01 | UNTESTED |
| — | C-S10 | §15.4 defaults → POLICY.md | L14, L24, L316, L1652, L1675, L1942–L1963, L2049 | normative (policy reading) → 2.3-QUEUE Q02 | UNTESTED |

By candidate: C-S1 (E01, E03), C-S2 (E04–E07), C-S3 (E09, E11, E12), C-S4 (E11), C-S5 (E13), C-S6 (E15), C-S7 (E10), C-S9 (E01, E02), C-S11 (E14, E16–E21), C-S13 (E25), C-S14 A (E26), C-S15 (E27–E31), C-S16 (E32), C-S17 (E08, E11), C-S18 (E01), C-S19 (E01). 16 candidates applied (12 CONFIRMED, 4 UNTESTED), 1 dropped (C-S8), 2 moved to 2.3 (C-S12, C-S10). Status words are from the six-way comparison (`round1/CANDIDATE-CHANGES.md`).

## §10–§13 are byte-identical

Each section runs from its `## N.` heading to the line before the next `## ` heading; the hash is SHA-256 of those lines joined with LF plus a final LF (`tools/frozen.py`). Line numbers shift by one because E04 adds a glossary row before §10.

| Section | Main lines | Main sha256 | Erratum lines | Erratum sha256 | Result |
|---|---|---|---|---|---|
| §10 | L683–L791 | `567480ac67a1e0b39aa89ff19aba4bdd65266fbbb2bb87b06f05d5fe2cb8fec0` | L684–L792 | `567480ac67a1e0b39aa89ff19aba4bdd65266fbbb2bb87b06f05d5fe2cb8fec0` | **IDENTICAL** |
| §11 | L792–L887 | `6fab8bfd5d8536c357ecf8e5ce563d186430daf68d1b1c49e716f0d89d3b6bc6` | L793–L888 | `6fab8bfd5d8536c357ecf8e5ce563d186430daf68d1b1c49e716f0d89d3b6bc6` | **IDENTICAL** |
| §12 | L888–L977 | `5fa3eb6327c2e1c5db1a427eee3c9376d5365b7269db564555be67147153e201` | L889–L978 | `5fa3eb6327c2e1c5db1a427eee3c9376d5365b7269db564555be67147153e201` | **IDENTICAL** |
| §13 | L978–L1251 | `1b08040c7010d9cf739bb7f842b07d0474fceb82551607e848c9f0391c83c2d6` | L979–L1252 | `1b08040c7010d9cf739bb7f842b07d0474fceb82551607e848c9f0391c83c2d6` | **IDENTICAL** |
| §10–§13 | L683–L1251 | `30deb16248bbf3d4d4ccdaf97e69dda8499e4b076257dc88b9f572179ae66a50` | L684–L1252 | `30deb16248bbf3d4d4ccdaf97e69dda8499e4b076257dc88b9f572179ae66a50` | **IDENTICAL** |

ALL FROZEN SECTIONS IDENTICAL. No hunk in `SPEC.diff` falls inside L683–L1251. The informative §10.2 sentence at L723 (“a terrain digest made from a squeezed seed as a negative”) was deliberately left unedited for this reason; it still reads correctly next to the new name `squeezed_seed_terrain`.

## Changes, one by one

### E01. C-S1, C-S9, C-S18, C-S19: §16 Vectors, Permanent negatives

- **Where:** main L2067; erratum E-L2068.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** DIFF-6WAY R4; QUESTIONS Q8, Q9; Rust Q10; B1, Go (final-stage hoist); Rust, C, Java (per-slot summary).

**Old (verbatim):**

```text
**Permanent negatives.** The pack carries, and every SDK must reject as non-matching, at least: `slot_summary_fold`, `xor_accumulator_fold`, `xor_w_fold`, `s_plus_w_fold`, `prefold_hoist`, `partial_slot_read`, `partial_slot_read_2047`, `one_lane_flip`, `all4_kernel`, `f3_shared_terrain`, a cached leg, and a terrain digest made from a squeezed seed. A negative is never removed while the construction it guards stands.
```

**New:**

```text
**Permanent negatives.** The pack carries, and every SDK must reject as non-matching, at least the thirteen negatives below. Each negative is computed at the vector’s profile integers, for the vector’s public key and seed beacon, with the section 10.4 terrain and the section 11.1 step except where its entry says otherwise. A leg negative is the first leg of stage 0, from its Rule A2.2 start (the seed), and is published as `legs_le_hex` of that leg. A terrain negative is published as the SHA-256 of the whole terrain array. Each entry is a construction this file forbids; it is defined here only so that the pack can pin it as a non-match. The pack generator compares each negative with the honest value of the same vector and stops with an error when they are equal; QA then chooses other fixture inputs. A negative is never removed while the construction it guards stands.

- `slot_summary_fold`, `xor_accumulator_fold`, `xor_w_fold`, `s_plus_w_fold`: the four folds that section 11.1 names. Leg negatives.
- `prefold_hoist`: the fold that computes once per slot `pre(i) = (sum over w of mix64(word(w) XOR w)) mod 2^64` and, on every step that reads slot `i`, uses `S_folded = (pre(i) + S) mod 2^64` in place of the section 11.1 accumulator, then the section 11.1 seal. Leg negative. It is the per-slot summary that section 11.1 forbids: `word(w) XOR w` does not depend on `S`.
- `final_stage_hoist`: the fold whose `S_folded` is `x XOR (x >> 31)`, where `x` is the XOR, over every word of the slot, of `mix64` applied to `word(w) XOR S` without its last line (the value `y` just before `return y XOR (y >> 31)` in section 10.1). Leg negative. It is the hoist that section 11.1 describes as informative. Its value equals an XOR accumulator of `mix64(word(w) XOR S)`.
- `partial_slot_read`: the section 11.1 fold over word 0 only of the slot each step reads. Leg negative.
- `partial_slot_read_2047`: the section 11.1 fold over words `0 … WORDS_PER_SLOT − 2` of the slot each step reads, leaving out the last word. Under section 6 that is 2047 of the 2048 words. Leg negative.
- `one_lane_flip`: the section 10.4 fill with the one-lane map of section 10.4 in place of the NL4 base word. Parents and the slot mix are unchanged. Terrain negative.
- `all4_kernel`: the section 10.4 fill with the linear map of section 10.4 in place of the NL4 base word. Parents and the slot mix are unchanged. Terrain negative.
- `f3_shared_terrain`: the first leg of stage 0 of the vector’s public key, from that key’s own start, run on the terrain of a second public key that the vector states, with the same seed beacon. Leg negative. The `f3` in the name is not row F3 of section 19. This negative guards row F4, shared terrain.
- `cached_leg`: the first leg of stage 0 in which a step that reads a slot already read earlier in the same leg reuses the `S_folded` of that earlier read instead of folding the slot with the current `S`. Leg negative. The vector’s leg reads at least one slot twice.
- `squeezed_seed_terrain`: the section 10.4 fill with each of the four lanes replaced by the one 64-bit value `P[0] XOR P[1] XOR P[2] XOR P[3]`, the squeeze that section 10.2 forbids. Terrain negative.
```

**Why:** L2067 names eight negatives it never defines, yet L2065 makes reproducing every negative a conformance condition, so nobody can conform as written. Round 1: 7 of the 8 named-only negatives split across the six builds (DIFF-6WAY R4); `prefold_hoist` had 5 values in 6 builds. Four builds published an honest terrain digest as `f3_shared_terrain`, and C/B2 published honest or near-honest legs as `cached_leg` (hence the generator-stops clause). `prefold_hoist` follows the per-slot-summary reading that Rust, C and Java chose independently (C-S18; L830 forbids a per-slot summary by name; F2 wording). `final_stage_hoist` is the L822 hoist that Go and B1 both built as their `prefold_hoist` (C-S9). The `f3` note answers Rust Q10 (C-S19). Other seven definitions: the erratum author's proposal; agreement in round 1 was partial_slot_read 3/6, partial_slot_read_2047 4/5, one_lane_flip 4/6, all4_kernel 4/6, squeezed 2/6, f3 1/6, cached_leg 1/6. Reference values for every entry are in ref/neg_values.out.

### E02. C-S9: §19 row F3

- **Where:** main L2168; erratum E-L2184.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** Go, B1.

**Old (verbatim):**

```text
Section 11.1 wrapping-add fold; negative `xor_accumulator_fold`. |
```

**New:**

```text
Section 11.1 wrapping-add fold; negatives `xor_accumulator_fold`, `final_stage_hoist` (section 16). |
```

**Why:** `final_stage_hoist` is an F3-family construction (xorshift linearity). §19 must list it beside `xor_accumulator_fold`.

### E03. C-S1: §19 row F4

- **Where:** main L2169; erratum E-L2185.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** naming only.

**Old (verbatim):**

```text
`f3_shared_terrain`, a cached leg, and a terrain digest from a squeezed seed. |
```

**New:**

```text
`f3_shared_terrain`, `cached_leg`, and `squeezed_seed_terrain` (section 16). |
```

**Why:** The same names as the §16 list. The informative §10.2 sentence at L723 (“a terrain digest made from a squeezed seed”) is NOT edited, because §10–§13 stay byte-identical; it still reads correctly.

### E04. C-S2: §4 Glossary (new row after “Outcome token”)

- **Where:** main L323; erratum E-L324.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** DIFF-6WAY R1 (45 keys); Rust Q2, C Q2, Go Q3, Java Q1, B1-11.

**Anchor (unchanged, verbatim):**

```text
Those `not defined` values are settled by the object's own bytes. |
```

**Inserted after it:**

```text
| Comparison result | The result of a Part B integer comparison (Rules B2, B4, and B10). It is `meets` when the comparison holds. When it does not hold, the rule states the outcome: `refuse`, or `not read` and `held` where that rule says so. `meets` is not an outcome token of this table, not an admission outcome, and not a section 15.2 state. It means only that this comparison does not refuse; every other section 15 check still runs. Part A does not use these words. |
```

**Why:** Rule B2 names only the failing side. The pack publishes “the integer rate … comparisons” (L2065), so the pass side needs one spelling. Round 1 produced four pass spellings (`meets the floor`, `meet` ×3, `match`, `meets`) and Java used `mismatch` for a shortfall. A separate row keeps the closed §4 outcome-token set (P4.1) unchanged.

### E05. C-S2: §15.4 Rule B2

- **Where:** main L1973; erratum E-L1974.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** DIFF-6WAY R1.

**Anchor (unchanged, verbatim):**

```text
a ratio `p/q` meets the floor when `hike_span` is an integer greater than 0 and `q · hike_legs ≥ p · hike_span`.
```

**Inserted after it:**

```text
 The comparison result is `meets` when that holds and `refuse` when it does not (section 4, **Comparison result**).
```

**Why:** See E04. Decisions agreed 45/45 in round 1; only the spelling differed.

### E06. C-S2: §15.4 Rule B4

- **Where:** main L1985; erratum E-L1986.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** text audit (no vector).

**Anchor (unchanged, verbatim):**

```text
with `min_anchored_paths` the lever in section 15.4.
```

**Inserted after it:**

```text
 When `anchored_total ≥ min_anchored_paths`, the comparison result is `meets` (section 4, **Comparison result**).
```

**Why:** Same one-sided gap as B2. The failing side keeps its existing settled-refuse / `not read` rule unchanged.

### E07. C-S2: §15.4 Rule B10

- **Where:** main L1977; erratum E-L1978.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** text audit (no vector).

**Anchor (unchanged, verbatim):**

```text
A gap of `K` is not refused by this rule and a gap of `K + 1` refuses.
```

**Inserted after it:**

```text
 A gap of `K` or less is the comparison result `meets` (section 4, **Comparison result**).
```

**Why:** Same one-sided gap as B2.

### E08. C-S17: §16.1 Legs (last sentence)

- **Where:** main L2096; erratum E-L2108.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** Rust Q4; DIFF-6WAY R2.

**Old (verbatim):**

```text
because fewer than 6 leaves `six_leg` `not defined`. The draw count stays 6.
```

**New:**

```text
because fewer than 6 leaves `six_leg` `not defined`. The Rule A4.4 draw count stays 6. The thorough-round draw vector is the section 15.5 draw and draws `D` indices (**Thorough rounds**, below).
```

**Why:** Rust (Q4) read “The draw count stays 6” as fixing every pack draw at 6 and so emitted no thorough draw; no high-effort core build emitted the thorough draw L2065 lists. B1 was the only build that did.

### E09. C-S3: §16.1 (new paragraph **Thorough rounds**)

- **Where:** anchored on text added by the previous edit; erratum E-L2110.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** DIFF-6WAY R2; Java Q16; C gen.c L493–511; B1.

**Anchor (unchanged, verbatim):**

```text
(**Thorough rounds**, below).
```

**Inserted after it:**

```text
**Thorough rounds.** The thorough-round index draw vector states its own `thorough_rounds`, as vector fixture metadata like `legs_per_stage`. That value is not a policy value, it is not read from section 15.4, and it sets no policy. The vector draws `D = 6 · thorough_rounds` indices under section 15.5 from its own byte source (**Draw bytes**, below), read from the first byte. A vector that publishes both a six-leg draw and a thorough draw, or draws for more than one leg total, reads each draw from the first byte of its byte source. No draw continues where another stopped.
```

**Why:** `D = 6 · thorough_rounds` depends on a §15.4 lever, so a pack vector would otherwise depend on a policy value (the F13 separation). No high-effort core build emitted the thorough draw (Java Q16, C and Go treat `thorough_rounds` as admission-only); C continued one byte stream across the 6/18/54 draws where the other five restarted at `b_0` (the reference script `ref/draw_ref.py` reproduces both readings exactly).

### E10. C-S7: §16.1 (new paragraph **Anchor comparison**)

- **Where:** anchored on text added by the previous edit; erratum E-L2112.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** Rust Q3, C Q3, Go Q4, Java Q2, B1 Q12.

**Anchor (unchanged, verbatim):**

```text
No draw continues where another stopped.
```

**Inserted after it:**

```text
**Anchor comparison.** The integer anchor vector gives a stored stage height `H` and a candidate inclusion height. Its answer is `anchored` when the inclusion height is `H` plus one of `ANCHOR_WINDOW_OFFSETS`, and `not anchored` otherwise, as if that header were an interrupt stored on the object and its plant comparison were `match`. It does not test those two conditions. Rule A4.5 still requires both on a real object. The integer anchor vector is not the anchor-window vector of sections 9 and 14.4, which is a full object.
```

**Why:** Rule A4.5 makes `anchored` also need a stored interrupt and plant `match`; the integer vector in L2065 has neither, so a careful build could answer `not anchored` everywhere. All four high-effort core builds raised it and all six builds chose the window-only reading (7/7 agree). The paragraph writes that reading down and keeps the full-object anchor-window vector distinct.

### E11. C-S4, C-S3, C-S17: §16.1 Draw bytes

- **Where:** main L2098; erratum E-L2114.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED, UNTESTED. **Evidence:** text audit; all six builds used the label chain.

**Old (verbatim):**

```text
**Draw bytes.** Injected draw bytes are rig-only, for a deterministic Rule A4.4 vector. The rig supplies them in order as the section 14.6 byte source. A published vector names those bytes `draw_bytes_hex`. The verifier consumes them in that order. It does not expand them again. They are never a production input. Production verify reads the operating-system CSPRNG. The draw count stays 6. Fewer than six legs leaves the draw unconsumed and `six_leg` `not defined`.
```

**New:**

```text
**Draw bytes.** Injected draw bytes are rig-only, for a deterministic draw vector: the Rule A4.4 six-leg draw, or the section 15.5 thorough-round draw. A published draw vector names its byte source as a SHA-256 chain from a stated ASCII label: `b_0 = SHA-256(label)` and `b_(n+1) = SHA-256(b_n)`, concatenated in that order. The pack publishes the label, not the bytes. The rig expands the label into that chain and supplies the bytes in order as the section 14.6 byte source. The verifier consumes them in that order and does not hash them again. They are never a production input. Production verify reads the operating-system CSPRNG. The Rule A4.4 draw count stays 6; the thorough-round draw vector draws `D` indices (**Thorough rounds**, above). Fewer than six legs leaves the draw unconsumed and `six_leg` `not defined`.
```

**Why:** L2065 says the pack does not publish `draw_bytes_hex`; L2098 said a published vector names its bytes `draw_bytes_hex`. A direct contradiction. All six builds used the CASES SHA-256 chain, which this text adopts; L2065 stays as written. Also carries the C-S17 scoping and names the thorough draw as a draw vector (C-S3).

### E12. C-S3: §16.2 Rule I2

- **Where:** main L2116; erratum E-L2132.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** text audit; Rust Q4.

**Old (verbatim):**

```text
including Rule A5; and the section 14.6 draw from a supplied byte source.
```

**New:**

```text
including Rule A5; the section 14.6 draw and the section 15.5 thorough draw, each from a supplied byte source; the Rule B2 integer comparison; and the integer anchor comparison (section 16.1, **Anchor comparison**).
```

**Why:** L2065 publishes vectors for the thorough draw and the integer rate and anchor comparisons, but Rule I2 did not require an SDK to expose them, so the pack could not drive them. No output byte changes; it names interfaces the pack already needs.

### E13. C-S5: §7.3 CBOR

- **Where:** main L494; erratum E-L495.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** Rust Q1, Java Q14, B2 (Q5).

**Old (verbatim):**

```text
`KIND` is longer than 23 bytes, and the `kind` value is still one definite-length text string.
```

**New:**

```text
`KIND` is 16 bytes, so the `kind` value is the head byte `0x70` and those 16 bytes.
```

**Why:** `v2.2-pcoc-object` is 16 bytes; the sentence is false on arithmetic. All six builds encode head `0x70`.

### E14. C-S11: §7.3 CBOR (decode order)

- **Where:** main L496; erratum E-L497.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** B1 Q3 (QUESTIONS Q12); Java AC-C5; Rust Q13.

**Anchor (unchanged, verbatim):**

```text
Later measurements are still recorded. Produce is what fixes the canonical bytes.
```

**Inserted after it:**

```text
 Verification sorts the input bytes in this order. First, input that is not one definite-length CBOR data item under these rules is a malformed CBOR value, whatever its first data item holds. That includes an empty input, bytes that are not CBOR, bytes that end inside an item, and a malformed item that is not a map. Rule A5 is not applied to a malformed CBOR value, even when a `kind` text can be read from it. Second, one well-formed data item that is not a map, or a map with no `kind` key whose value is a text string, is a decode failure: there is no section 14 record, and the verify CLI exits with code `1` (section 18). Third, a map whose `kind` text is not exactly `KIND` is Rule A5. Every other input is measured by section 14.
```

**Why:** §7.3 / the shape table row 1 make malformed bytes a `malformed cbor` record, while Rule A5 and §18 make “cannot be … decoded” exit 1. Not-CBOR, truncated, malformed-non-map and malformed-map-with-a-foreign-kind inputs fell under both or neither. Java’s AC-C5 treats a trailing byte on a non-KIND map as A5; Rust (Q13) refuses an oversize count with no record outcome stated. The order picks B1’s reading, which shape row 1 supports.

### E15. C-S6: §14.7 Rule A5 (serialization)

- **Where:** main L1670; erratum E-L1671.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** Rust Q5, Java Q12, B2 Q4.

**Anchor (unchanged, verbatim):**

```text
`kind_text` is the kind text the object carried, as a JSON string.
```

**Inserted after it:**

```text
 The record is that two-key object, keys in that order, with no insignificant whitespace, and `kind_text` spelled as RFC 8785 spells a JSON string; that is also the RFC 8785 encoding of the object. A conforming verify CLI writes it as it writes the section 14 record, with one trailing newline (U+000A). The newline is not part of the record. The pack’s unknown-kind vector is the record bytes without the newline.
```

**Why:** L1274 fixes the full record’s CLI serialization and newline; Rule A5 gave only a template. All six builds emitted identical bytes with no LF; Rust and Java asked. Escaping was untested (ASCII labels only), so RFC 8785 string rules are named.

### E16. C-S11: §14.7 Rule A5 (last sentence)

- **Where:** main L1670; erratum E-L1671.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** as E14.

**Old (verbatim):**

```text
Bytes that are not a CBOR map, or a map that does not yield a `kind` text, are still a decode failure.
```

**New:**

```text
One well-formed data item that is not a map, or a map that does not yield a `kind` text, is still a decode failure (section 7.3). A malformed CBOR value is not a decode failure and is not Rule A5.
```

**Why:** Aligns Rule A5 with the E14 order.

### E17. C-S11: §18 Verify CLI

- **Where:** main L2158; erratum E-L2174.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** as E14.

**Old (verbatim):**

```text
Exit code `1` means the object file cannot be read or decoded.
```

**New:**

```text
Exit code `1` means the object file cannot be read, or its bytes are a decode failure under section 7.3. A malformed CBOR value is a record, exit code `0`.
```

**Why:** Aligns §18 with the E14 order.

### E18. C-S11: §1 Part A rule classes, row A5

- **Where:** main L228; erratum E-L228.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** as E14.

**Old (verbatim):**

```text
Sections 14.1–14.6 are not applied. | §14.7 Rule A5 |
```

**New:**

```text
Sections 14.1–14.6 are not applied. A malformed CBOR value is section 7.3 `malformed cbor`, not this record. | §14.7 Rule A5 |
```

**Why:** Consistency follow-on to E14 (the class row restates Rule A5).

### E19. C-S11: §15.1 Check order, step 8

- **Where:** main L1801; erratum E-L1802.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** internal consistency.

**Old (verbatim):**

```text
8. The object’s kind text is not `KIND`.
```

**New:**

```text
8. The object is Rule A5 under the section 7.3 order: one well-formed CBOR map whose `kind` text is not `KIND`.
```

**Why:** Consistency follow-on to E14. Without it the door could read a kind text out of malformed bytes and answer `unknown_kind` where verify records `malformed cbor`. Flag for White Hat: for that one input (malformed bytes carrying a non-KIND kind text) the website outcome is now `rejected`, not `unknown_kind`.

### E20. C-S11: §15.1 Check order, step 9

- **Where:** main L1802; erratum E-L1803.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** internal consistency.

**Old (verbatim):**

```text
9. The object does not decode as a hike object (no CBOR map, or no `kind` text).
```

**New:**

```text
9. The object does not decode as a hike object: a section 7.3 decode failure (one well-formed data item that is not a map, or a map with no `kind` text).
```

**Why:** Consistency follow-on to E14.

### E21. C-S11: §15.1 Check order, step 10

- **Where:** main L1803; erratum E-L1804.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** internal consistency.

**Old (verbatim):**

```text
10. The object is a hike object of `KIND`.
```

**New:**

```text
10. The object is a hike object of `KIND`, or a malformed CBOR value (section 7.3), which section 14 records as shape `mismatch` (Rule B9).
```

**Why:** Consistency follow-on to E14: a malformed value otherwise matched no step. It reaches the existing Rule B9 shape-mismatch refuse (website `rejected`).

### E25. C-S13: §14.1 Rule A4.1 (height_range bullet)

- **Where:** main L1359; erratum E-L1360.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Round-1 status:** UNTESTED. **Evidence:** B1 Q6 (QUESTIONS Q15); Rust Q12 (no GET); untested.

**Anchor (unchanged, verbatim):**

```text
is shape `mismatch`. `shape_item` includes `height_range`. The value is `not defined`. It is not `not read`.
```

**Inserted after it:**

```text
 A height that is `not defined` this way is treated as absent by every other shape condition: `header height order` is violated for each comparison that height takes part in, and `last_interrupt` is violated when that height is `last_interrupt_height`.
```

**Why:** L1359 makes an over-range height `not defined` but did not say whether it still takes part in the order tests. B1 treats it as absent, which matches “not a usable unsigned value” (L579) and the existing field-absent rule (L1303). It adds violations, never removes one.

### E26. C-S14 (option A): §14.4 Rule A4.10 Median-time-past

- **Where:** main L1586; erratum E-L1587.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** CONFIRMED. **Evidence:** Rust Q11, Java Q15, Go Q2, B1 Q7.

**Old (verbatim):**

```text
Take `header.time` from each other document in the window when the read returned a document, `header.height` equals the requested height, `header.time` follows section 9, and, where the index body carries a previous-hash, that header’s previous-hash links to the next header toward the beacon. A carried previous-hash that does not link that way leaves that header’s time missing.
```

**New:**

```text
Take `header.time` from each other document in the window when the read returned a document, `header.height` equals the requested height, and `header.time` follows section 9. The portable index carries no previous-hash member (section 9), and this measurement does not link the window’s headers to each other.
```

**Why:** §9 defines no previous-hash member and L615 says members other than hash/height/time change nothing, so the clause could never fire on a conforming index, yet Rust and Java invented `header.previous_hash` and checked it while Go and B1 ignored it (2–2). Christian’s call: option A (delete). Option B (define the member) would be normative and is not taken.

### E27. C-S15: §14.4 Rule A4.11 step 1

- **Where:** main L1602; erratum E-L1603.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** UNTESTED. **Evidence:** B1 Q8 (QUESTIONS Q17); untested (all paths empty).

**Old (verbatim):**

```text
1. A present path is a non-empty path on `W_1` … `W_16` that decodes and whose `validate_for` succeeds. When fewer than two paths among `W_1` … `W_16` are present, record every bookend field as `not defined`.
```

**New:**

```text
1. A valid path is a path on `W_1` … `W_16` whose `path_state` is `present` (Rule A4.5), that decodes as `CanonicalPathBody`, and whose `validate_for` succeeds. A `present` path that does not validate is not a valid path. When fewer than two paths among `W_1` … `W_16` are valid, record every bookend field as `not defined`.
```

**Why:** Rule A4.5 uses “present” for any non-empty path; step 1 redefined it locally as a validated path, and step 2, the A4.11 class row and the `fixed_start` text used it without saying which. B1 follows the step-1 sense; the new term keeps the two apart.

### E28. C-S15: §14.4 Rule A4.11 step 2

- **Where:** main L1604; erratum E-L1605.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** UNTESTED. **Evidence:** as E27.

**Old (verbatim):**

```text
2. An issued plant GET is a plant read for a present path whose inclusion height
```

**New:**

```text
2. An issued plant GET is a plant read for a valid path (step 1) whose inclusion height
```

**Why:** As E27. Only a validated path reaches the plant GET anyway (L1493).

### E29. C-S15: §1 Part A rule classes, row A4.11

- **Where:** main L223; erratum E-L223.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** UNTESTED. **Evidence:** as E27.

**Old (verbatim):**

```text
| A4.11 | INVARIANT | Fewer than two present paths among `W_1` … `W_16` records
```

**New:**

```text
| A4.11 | INVARIANT | Fewer than two valid paths (Rule A4.11 step 1) among `W_1` … `W_16` records
```

**Why:** As E27.

### E30. C-S15: §14 Verify record, `fixed_start` (first use)

- **Where:** main L1333; erratum E-L1334.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** UNTESTED. **Evidence:** as E27.

**Old (verbatim):**

```text
when two or more paths are present, every read returned, and fewer than two `anchors` tokens are `anchored`
```

**New:**

```text
when two or more paths are valid (Rule A4.11 step 1), every read returned, and fewer than two `anchors` tokens are `anchored`
```

**Why:** As E27. This sentence restates steps 1 and 3, so it uses the step-1 sense.

### E31. C-S15: §14 Verify record, `fixed_start` (second use)

- **Where:** main L1333; erratum E-L1334.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** UNTESTED. **Evidence:** as E27.

**Old (verbatim):**

```text
The two-or-more present paths case is index-settled.
```

**New:**

```text
The two-or-more valid paths case is index-settled.
```

**Why:** As E30.

### E32. C-S16: §14 Verify record, `index_comparison_P_b`

- **Where:** main L1331; erratum E-L1332.
- **Kind:** replace.
- **Class:** clarification.
- **Round-1 status:** UNTESTED. **Evidence:** B1 Q9 (QUESTIONS Q18); untested.

**Old (verbatim):**

```text
It is `mismatch` when the document was read and either the 32 bytes differ or the height differs. It is `not read` when the read does not return a document.
```

**New:**

```text
It is `mismatch` when the read is a successful header read under section 9 and either the 32 bytes differ or the height differs. It is `not read` when the read does not return a document, or returns a document that is an unsuccessful header read under section 9, including a plant document with no usable `header` that Rule A4.8 reuses.
```

**Why:** Rule A4.8 reuses the §14.3 plant document, which may have no `header` (the SPEC’s own anchor-window vector). L615 calls that an unsuccessful header read (`not read`), and A4.11 step 4 then gives every bookend field `not read`; L1331 on its face said `mismatch`. B1 follows L615.

## Dropped and not taken

- **C-S8 (slot 0 wording, §10.4 L788): dropped.** All six builds produced identical terrain at S1, S2 and PROD with slot 0 unmixed, and no build raised it in the high-effort round (status SUPPORTED). The edit would sit inside §10, which stays byte-identical.
- **C-S14 option B (define `header.previous_hash`): not taken.** It would change §9 body rules and MTP outcomes on indexes that carry the member, which is normative (2.3). Option A is applied as E26.
- **C-S9 / C-S18 alternatives.** Java's `pre + mix64(S)` per-slot-summary combination is not pinned; C's `pre XOR S` is queued as the optional sibling `prefold_hoist_xor` (2.3-QUEUE Q03). Every variant is already forbidden by §11.1 L830 (“a per-slot summary … does not satisfy the load”).

## Moved to 2.3 (`2.3-QUEUE.md`)

- **Q01 C-S12** (withdrawn E22–E24, narrowed to the stage-0 `path` key and the `last_interrupt` length; no width exclusions).
- **Q02 C-S10** (defaults out of §15.4) with White Hat's condition sentence and the explicit three-lever unset list.
- **Q03** White Hat's optional negative siblings `mix64_without_mix_add` and `prefold_hoist_xor`.
- **Q04** vectorisation: (a) SIMD within one slot permitted in §11.1 (needs Christian's OK), (b) the §11.1 SHA-NI rule made informative, (c) the §15.5 floor calibration method.

Each has exact old/new text against this erratum's SPEC.md, applied by `tools/queue.py` to `2.3-queue/SPEC-2.3-candidate.md`.

## Reference values (from SPEC text; for checking builds, never for CASES)

`ref/neg_values.py` computes all 13 negatives exactly as E01 words them (S1, `pk_A`, CASES-v2 seed beacon). Every value differs from honest, all 13 are pairwise distinct, `final_stage_hoist` equals an XOR accumulator of `mix64(word(w) XOR S)` as E01 states, and the S1 leg first revisits slot 101 at step 34 (first read at step 20), so `cached_leg` is well defined there. `prefold_hoist` equals Rust's round-1 value and `final_stage_hoist` equals Go's and B1's; the other eleven equal the round-1 reference values.

```text
honest.legs_le_hex	4c9a6a98574cbd31
honest.terrain_sha256	366e6d1d27adb715d5c96c64e6b9d881e1701a0151e6e08d49df08d2996bd97f
slot_summary_fold.legs_le_hex	b87ce6fc485dd57c
xor_accumulator_fold.legs_le_hex	346188d2d8cf98e8
xor_w_fold.legs_le_hex	73a4c7fbd105aae1
s_plus_w_fold.legs_le_hex	07262df2d90e8ecb
prefold_hoist.legs_le_hex	a652562037b465fc
final_stage_hoist.legs_le_hex	b1808c8d3927a7d1
partial_slot_read.legs_le_hex	e1ae647ee5a5d7c5
partial_slot_read_2047.legs_le_hex	738755dabac824ab
f3_shared_terrain.legs_le_hex	a73019b8a2c43cd1
cached_leg.legs_le_hex	3b715e2a251a44d9
one_lane_flip.terrain_sha256	5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608
all4_kernel.terrain_sha256	2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e
squeezed_seed_terrain.terrain_sha256	ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22
# count 13
# all differ from honest: True
# all pairwise distinct: True
# final_stage_hoist equals XOR-accumulator of mix64(word XOR S): True
# cached_leg first revisit (step, slot, first read step): (34, 101, 20)
```

## Internal consistency of the erratum SPEC (`tools/consistency.py`)


- Rule references: 49 distinct IDs, 0 unresolved.
- P-row IDs: 87 distinct, 0 unresolved.
- Section references: 52 distinct; the only two that match no heading (6.2, 4.2) are RFC 6454 §6.2 and RFC 8949 §4.2.1, external references already in main.
- Bold paragraph cross-references: 10 distinct, all resolve (`Binding` is a `####` heading; `Comparison result` is the new §4 glossary row). New ones: **Thorough rounds**, **Anchor comparison**, **Draw bytes**, **Comparison result**.
- `shape_item`: still exactly 14 strings; every string named in “includes …” or “reported only by …” is in the table.
- Permanent negatives: §16 defines 13 (“at least the thirteen”); every negative named in §11.1 and §19 is defined in §16 and every §16 negative appears in §19 (main had 10 §19 names and 4 §11.1 names with no §16 definition).
- Outcome words: `meets` appears 5 times, always as a Part B comparison result tied to the §4 row; the §4 outcome-token set (P4.1) and the §14 token list are unchanged.
- §19 labels F1–F16 (16), contiguous.
- Leftover-phrase checks: “The draw count stays 6” unscoped: 0 (main 2). “present path” in the A4.11 sense: 0 (main 3: L223, L1333, L1602). `draw_bytes_hex`: 1, the L2065 “does not publish” sentence (main 2, contradicting). “previous-hash”: 1, the new “carries no previous-hash member” sentence. `groundinglab` case violations: 0.

## Open points noticed while drafting (not proposed; White Hat raised no objection to leaving them)

1. **Over-range stored heights and the index read.** E25 settles the shape side. Whether Rule A4.2 issues a GET for an over-range `beacon_height` (Rust: no GET, `not read`) is still only implied.
2. **L1606 “three paths are present”** in the anchor-window vector sentence was left as is: it gives `not defined` whether step 1 or step 3 fires, but it names step 3. If any of those three paths fails to validate, the sentence names the wrong step.
3. **E14 “an empty input”** is the literal reading of shape row 1 (not one data item), so it is a `malformed cbor` record and not exit 1. White Hat may prefer exit 1.
4. **A `kind` text that is not valid UTF-8.** §7.3 does not list invalid UTF-8 among the malformed values, and RFC 8785 cannot spell it. Untouched here.
5. **Speed-up flags.** The Java 5.2 s timing is now explained as exact vectorisation (White Hat, JAVA-TIMING.md); the SPEC-side consequences are 2.3-QUEUE Q04. B1's WASM kernel is still flagged, not merged.
6. **Baseline host wording (pre-existing, §11.1 vs §15.5).** §11.1 (informative) names the floor baseline as commodity hardware with SHA-256 instructions (Ryzen 5 5600H); §15.5 names the low-end owned host as an i7-10610U with no SHA-NI. Not a contradiction (one is the baseline class, the other the low end), but Q04b/Q04c touch both; worth one consistent sentence in 2.3.

