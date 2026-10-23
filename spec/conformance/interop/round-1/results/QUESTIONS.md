# Round 1 SPEC questions, merged and deduplicated (B1 Sonnet TS + B2 Sol Py)

Sources:
- **B2:** `b2-sol-py/RESULTS.md` § "SPEC questions and implemented readings" (4 items).
- **B1:** "SPEC questions and the reading I implemented" (16 items, plus "Not finished or not covered"), recovered from the B1 transcript into `b1-questions-from-transcript.md`.
  - B1 did write RESULTS.md, but the file never reached its commit, so a3ac71d has no RESULTS.md. That is a commit gap, not a deviation from PROMPT-trackB.md.
  - B1's numbers are given as B1-1 … B1-16 below.
- **Reviewer:** issues found in the diff and cross-verify (DIFF.md) that neither build raised.

SPEC line numbers are for pcoc `2b50df7` `spec/satoshi-hike-v2.2/SPEC.md`.

| # | Topic | SPEC / CASES location | Raised by | Reading(s) taken | Byte mismatch? | Judgement |
|---|---|---|---|---|---|---|
| Q1 | NL4 words j = 0…15 at S1, where `WORDS_PER_SLOT` = 8 | CASES case 2; SPEC §6 P6.23 L126, §10.4 L749 | B1, B2 | B1: base words for all j = 0…15, stored words for 0…7 only. B2: j = 0…7 only. | Yes: 24 B1-only keys. The 48 shared keys all match. | **CASES fixture bug.** The SPEC is clear. |
| Q2 | `rate` outcome word for the passing side | SPEC Rule B2 L1973 ("meets the floor" … "The outcome is `refuse`"); §15 vocabulary L1683; CASES case 9 | B1, B2 | B1 `meets`, B2 `meet`; both `refuse` otherwise | Yes: 17 keys (spelling only; all 45 decisions agree) | **Genuine SPEC gap.** There is a token for refuse and none for pass. Define a comparison token pair, or have CASES define vector tokens. |
| Q3 | Does slot 0 get `slot_mix`? | SPEC §10.4 L766 ("Slot 0 … has no parent lanes and no slot mix") and L788 ("For every slot, including slot 0, write …") | B1 | Both: no mix on slot 0 | No (terrain digests match) | **SPEC clear.** L788 might read as applying to the mix, so it could be reworded to "write (slot 0 unmixed)", but it is not ambiguous. |
| Q4 | Do the unknown-kind record bytes include the CLI's trailing LF? | SPEC Rule A5 L1667–1670; CASES case 12 | B2 | Both: compact JSON, no LF | No (4/4 match) | Mildly ambiguous in CASES only. CASES should state "no LF". |
| Q5 | §7.3 says "`KIND` is longer than 23 bytes", but `v2.2-pcoc-object` is 16 bytes | SPEC §7.3 L494 | B2 | Both: encode the real 16-byte length (`0x70` head) | No | **SPEC error** (a stale sentence, probably from a longer earlier KIND). Delete it or correct it. |
| Q6 | The CASES key template is underspecified (padding, separators, `pk_A`, field names `S_in`/`S`, `final`/`stored`, `sha256`/`digest`) | CASES L14 | Reviewer (diff) | Each build invented its own layout | Yes: 12 of ~380 keys match raw; after normalisation, 304 match | **CASES fault.** Publish exact key names per case. |
| Q7 | Which draw does the pack pin, and at what `thorough_rounds`? | SPEC §16 L2065 ("thorough-round index draw"), L2073; §15.5 L2005–2007 (`thorough_rounds` is a §15.4 lever); CASES case 11 | Reviewer (diff) | B1: six-leg + thorough at 3 rounds, each restarting the byte source. B2: six-leg only. | Yes: 5 B1-only keys. Six-leg agrees 3/3. | **SPEC gap** (the vector depends on a policy lever with no fixture value) plus CASES wording. |
| Q8 | Eight permanent negatives are named but not defined: `prefold_hoist`, `partial_slot_read`, `partial_slot_read_2047`, `one_lane_flip`, `all4_kernel`, `f3_shared_terrain`, cached leg, squeezed-seed terrain | SPEC §16 L2067; §19 F2/F4 L2167, L2169; §10.2 L721–723 (squeeze not defined); §10.4 L762–764 (one-lane/linear maps informative only); §11.1 L822 (hoist informative only) | Reviewer (diff) | B1: concrete constructions for each (see DIFF C5). B2: placeholders or honest values (defects). | Yes: 8 value mismatches. The 4 negatives the SPEC defines (§11.1 L824) all match. | **Genuine SPEC gap.** "Reproduces every permanent negative" is a conformance condition (§16 L2065), but 8 of the 12 have no normative construction. |
| Q9 | `partial_slot_read_2047` on an S1 case ("at PROD `WORDS_PER_SLOT` semantics") | CASES case 14 | Reviewer (diff) | B1: wps−1 words (7 of 8). B2: first 2047 words = the whole slot = the honest value. | Yes (part of Q8) | **CASES fault** (meaningless at S1). Run it at PROD or define it as `WORDS_PER_SLOT − 1`. |
| Q10 | Hashcash fixture inputs (origin, fee header, waypoint) are left to each build | CASES case 13, L23; SPEC §13.1 L1051–1055 clear | Reviewer (diff) | B1: derived origin `https://<16 hex>.invalid` plus published fee header. B2: `https://example.com` (not derived per CASES L23), fee header not published. | Yes: 7 + 4 keys | **CASES fault.** Both are SPEC-correct by independent recompute. Fix the inputs. |
| Q11 | Object beacon heights for the test-plane objects | CASES (none) | Reviewer (cross-verify) | B1: 1000…1018. B2: 100…118. | Objects differ only in heights; re-encoding gives identical bytes | **CASES fault** (not fixed). Fix the heights for O1–O3. |
| Q12 | Malformed CBOR (record) vs decode failure (exit 1) vs Rule A5 | §7.3 L496; table row 1 L1288; A5 L1670; §18 L2158; §15.1 L1802 | B1 (B1-3) | B1: anything malformed → `["malformed cbor"]` record; one well-formed non-map or no text `kind` → exit 1 | Not tested | **Genuine SPEC ambiguity.** Not-CBOR or truncated bytes, malformed non-maps, and malformed maps with a non-`KIND` kind fall under both rules or neither. B1's reading fits row 1. → C-S11 |
| Q13 | Stage-0 `path` key: also `field set`? | L1348 (field set bullet says "Stage 0 has no `path` key"), L1349, L1289, L1303 ("not dropped") | B1 (B1-4) | B1: only `stage-0 path key` | Not tested | **Genuine ambiguity.** The literal text gives both strings; the `stages length` precedent suggests exclusive. → C-S12 |
| Q14 | Widths and value types as `field set`; `last_interrupt` length only by `last_interrupt` | §8 L519–531 ("byte string, length 32"); rows 2 and 11 | B1 (B1-5) | B1: `public_key`, `seed_beacon`, beacon and waypoint length or type → `field set`; `last_interrupt` length → `last_interrupt` only | Not tested | Widths as `field set`: **B1's is the only sensible reading** (§8 types include the length). `last_interrupt`: same overlap as Q13. → C-S12 |
| Q15 | Absent or out-of-range height in `header height order` | L1303, L1307, L1359, §8 L579 | B1 (B1-6) | B1: both violate `header height order` (plus `height_range` for out-of-range) | Not tested | Absent height: **SPEC clear** (L1303/L1307), B1 right. Over-range height: **ambiguous** (compare the integer, or treat it as absent). → C-S13 |
| Q16 | A4.10 MTP "previous-hash" names no JSON key | L1586; §9 L615 (header measurement uses only hash/height/time) | B1 (B1-7) | B1: no link check | No (no index carries one) | **Genuine gap.** The clause is inoperative, and a core might invent a key. → C-S14 |
| Q17 | A4.11 "present path" vs A4.5 `path_state` `present` | L1419, L1481, L1602, L1604, L223 | B1 (B1-8) | B1: present = decodes + `validate_for` succeeds (L1602) | Not tested (all paths empty) | B1 follows L1602, the only sensible reading of step 1. But the term clashes with A4.5, and steps 2 and L223 are unclear. → C-S15 |
| Q18 | `index_comparison_P_b` when the reused plant document has no `header` | L1331, L1551, L615, L617, step 4 L1610 | B1 (B1-9) | B1: `not read`, then every bookend field `not read` | Not tested | B1 follows L615 and step 4, the consistent reading. L1331 says "`not read` when the read does not return a document" on its face. → C-S16 |
| Q19 | Anchor vector words and test scope | CASES case 10; A4.5 L1414; §16 L2065; the anchor-window object vector L617/L1606 | B1 (B1-12) | B1: `anchored` / `not anchored`, integer window only | No (7/7 match B2) | Same as C-S7. Also a name clash with the full-object "anchor-window vector". → C-S7 (amended) |
| Q20 | Test-plane objects are not full producer runs | §13.3, §16.1 Legs L2096 | B1 (B1-16) | B1: rig fixes the tip schedule at `legs_per_stage`; no Observe/Bari/events | n/a | **SPEC clear** (L2096). No change. |

## B1 question map (B1-n → merged Q)
| B1 | Topic | Merged as | CANDIDATE-CHANGES |
|---|---|---|---|
| B1-1 | slot 0, L766 vs L788 | Q3 | C-S8 |
| B1-2 | NL4 j = 0…15 | Q1 | C-F2 |
| B1-3 | malformed CBOR vs decode failure | Q12 | C-S11 |
| B1-4 | stage-0 path key vs field set | Q13 | C-S12 |
| B1-5 | widths and types as field set; `last_interrupt` length | Q14 | C-S12 |
| B1-6 | absent or out-of-range height order | Q15 | C-S13 (out-of-range only) |
| B1-7 | MTP previous-hash key | Q16 | C-S14 |
| B1-8 | A4.11 "present" | Q17 | C-S15 |
| B1-9 | `P_b` index comparison from the plant document | Q18 | C-S16 |
| B1-10 | draw: six-leg and thorough, `thorough_rounds` | Q7 | C-S3, C-F7 |
| B1-11 | rate pass word | Q2 | C-S2, C-F8 |
| B1-12 | anchor words | Q19 | C-S7 |
| B1-13 | negatives definitions | Q8 | C-S1, C-F5 |
| B1-14 | hashcash fixture inputs | Q10 | C-F3 |
| B1-15 | key naming | Q6 | C-F1 |
| B1-16 | test-plane objects not full runs | Q20 | none (SPEC clear) |

**B1 "Not finished or not covered":** no Bari, sign-in, network producer, header source or event log; no HTTP index reader (file only); the six legs run sequentially (allowed by §14.6); no §15 thorough rounds outside the generator; path/plant code is untested (all paths empty); hardware SHA is whatever Node's OpenSSL uses. All of this agrees with "Did NOT prove" in CANDIDATE-CHANGES.md.

## Cross-reference to CANDIDATE-CHANGES.md
- Q1 → C-F2
- Q2 → C-S2, C-F8
- Q3 → C-S8
- Q4 → C-S6, C-F6
- Q5 → C-S5
- Q6 → C-F1
- Q7 → C-S3, C-F7
- Q8 → C-S1, C-F5
- Q9 → C-F5
- Q10 → C-F3
- Q11 → C-F4
- Q12 → C-S11
- Q13, Q14 → C-S12
- Q15 → C-S13
- Q16 → C-S14
- Q17 → C-S15
- Q18 → C-S16
- Q19 → C-S7
- Q20 → none

New items found while preparing the candidates (raised by neither build):
- C-S4: the `draw_bytes_hex` contradiction, L2065 vs L2098.
- C-S7: the anchor vector's assumptions.
- C-S9: optional `final_stage_hoist`.
- C-S10: the lever-default note.

## Top ambiguities for the SPEC clarification pass (in priority order)
1. **Q8: define all 12 permanent negatives normatively.** 8 have names only. This is the biggest gap, because it is a conformance condition.
2. **Q2: name the rate-comparison tokens** (pass and refuse) in Rule B2 or §4.
3. **Q7: say which draw the pack pins and the `thorough_rounds` value for that vector.**
4. **Q5: remove the false "KIND is longer than 23 bytes" sentence** (§7.3 L494).
5. **Q12: one order for malformed CBOR, decode failure and Rule A5** (C-S11).
6. **Q13/Q14: dedicated `shape_item` strings exclude `field set`** (C-S12).
7. **Q16: the inoperative MTP previous-hash clause** (C-S14).
8. Q15, Q17, Q18: wording fixes (C-S13, C-S15, C-S16).
9. Wording only: Q3 (slot 0 at L788).

The CASES fixes needed before round 2 (no SPEC change) are Q1, Q4, Q6, Q9, Q10 and Q11.
