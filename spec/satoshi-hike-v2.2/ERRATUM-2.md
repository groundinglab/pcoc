# SPEC 2.2 erratum 2: exact changes

**23 Oct 2026 (ACDT).** 3 clarifications to `spec/satoshi-hike-v2.2/SPEC.md` (main 7b40353, after erratum 1), in 4 edits. Christian approved the content on 23 Oct at 01:11 ACDT (option A for the malformed record; White Hat's D2 wording for vectorisation). None of them changes the bytes of any valid object, any vector, or any accept/reject outcome. Paths below to `sdk/conformance/` name the cross-verify gate on the Phase A integration branch (draft PR #78); it is not on main yet.

**Base and result**
- Base: pcoc `main` 7b40353, `spec/satoshi-hike-v2.2/SPEC.md`, sha256 `2ae522f7034bb7adbc9a78e2e685631cb5f2c059d9f88200ba7c504b0f492956` (2201 lines).
- Result: this branch's `spec/satoshi-hike-v2.2/SPEC.md`, sha256 `2ae522f7034bb7adbc9a78e2e685631cb5f2c059d9f88200ba7c504b0f492956` (2201 lines).
- Every edit below is an exact string operation: an anchor that occurs exactly once in main, with text inserted after it (E01–E03) or a new subsection inserted before the `## 18. Verify CLI` heading (E04). Line numbers are main's (L…) and the erratum's (E-L…).
- The SPEC stays a standalone v2.2 document. No edit mentions a previous version.

**Christian's calls, as applied.**
1. Vectorisation within one slot (2.3-QUEUE Q04(a), plan decision D2): land it now as a clarification, in Rule I6 (§16.2) and a new informative §17.4, **not** in the frozen §11.1. The wording is White Hat's D2 text, with the slot load spelled out: every step loads all `SLOT_BYTES` bytes (16 KiB) and mixes every word with that step's `S`.
2. Invalid UTF-8 in `kind` (erratum 1 open point 4): a malformed CBOR value.
3. Malformed-record path and count fields: **option A** (the record the C SDK writes today). Options B and C are recorded under *Dropped and not taken*.

**White Hat review.** White Hat proposed the E03/E04 wording (review of the SDK 2.2 plan, D2) and recommended point 2 be settled as malformed CBOR (Q10). White Hat has not yet reviewed this document as a whole; it is a draft PR.

## Version call

All four edits are **clarifications** under the class rule (no byte or outcome of any conforming output changes):
- **E01** picks one of two readings the old text allowed for an input no valid object contains. All four SDK verifiers already record `malformed cbor`, exit 0, for `a1 64 6b 69 6e 64 61 ff`.
- **E02** defines fields that main left without a value. main L1320 gives `path_state` a closed set (`present`, `skip`), and main L1346 says a value left without a usable object value is `not defined`; for a malformed CBOR value those two sentences do not pick one record, and the four SDKs wrote three different ones. A malformed value is not a valid object, so no valid object's record changes.
- **E03/E04** state that the §11.1 loop may be evaluated several words at a time. main L831 already requires every step to load all `SLOT_BYTES` bytes and mix each of the `WORDS_PER_SLOT` words with the current `S` by the §11.1 loop; a vectorised evaluation of that loop does exactly that and gives identical bytes, because the §11.1 sum is a wrapping sum mod 2^64 (associative and commutative). main L2148 (Rule I6) already routes a rate gain through review against the negatives before release. No construction value, vector, or outcome changes. The text sits outside §10–§13.

**Nothing here forces 2.3.**

## Summary

| Edit | Item | Section | Main line | Class | Status |
|---|---|---|---|---|---|
| E01 | Invalid UTF-8 `kind` (erratum 1 open point 4) | §7.3 CBOR (decoding rules) | L497 | clarification | CONFIRMED (four SDKs) |
| E02 | Malformed-record path and count fields, option A | §14 Verify record (`paths`) | L1320 | clarification | CONFIRMED (C); Rust, Go, Java change |
| E03 | Several words of one slot at once (D2) | §16.2 Rule I6 | L2148 | clarification | wording only |
| E04 | Same, restated | §17.4 (new, informative) | new text (before §18) | clarification | wording only |

## §10–§13 are byte-identical

Lines L684–L1252 of main (`## 10. Terrain` through the line before `## 14. Verify`) are identical in the result (sha256 of that range `30deb16248bbf3d4d4ccdaf97e69dda8499e4b076257dc88b9f572179ae66a50` on both). §11.1 is not touched; E03 and E04 refer to it.

## Changes, one by one

### E01. Invalid UTF-8 in `kind`: §7.3 CBOR (decoding rules)

- **Where:** main L497; erratum E-L497.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Status:** CONFIRMED. **Evidence:** cross-verify gate, malformed cells: Rust, C, Go and Java all exit 0 and record `shape` `mismatch`, `shape_item` `["malformed cbor"]` for `a1 64 6b 69 6e 64 61 ff` (a one-entry map whose `kind` value is the one byte `ff`).

**Anchor (unchanged, verbatim):**

```text
Indefinite lengths, tags, floating-point values, simple values, negative integers, a map key that is not a text string, a duplicate key, and any byte after that one data item are a malformed CBOR value.
```

**Inserted after it:**

```text
 A `kind` value whose text-string bytes are not valid UTF-8 (RFC 3629) is also a malformed CBOR value. It is not Rule A5 and it is not a decode failure.
```

**Why:** §7.3 lists the malformed values and did not list a text string that is not UTF-8, while L495 requires text values to be UTF-8 text strings and RFC 8785 cannot spell such a value in the record. Without this sentence one reader could take the input as Rule A5 (a `kind` that is not exactly `KIND`) and another as a decode failure (exit 1). Treating it as malformed keeps the decode order of L497 (malformed first, then decode failure, then Rule A5) and matches what every SDK already does. Rule A5 is not applied to a malformed CBOR value (L497), so no `unknown_kind` record arises. Map keys are not covered by this sentence (open point 1 below).

### E02. Malformed-record path and count fields (option A): §14 Verify record, `paths`

- **Where:** main L1320; erratum E-L1320.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Status:** CONFIRMED for C; Rust, Go and Java are brought to it on the Phase A branch. **Evidence:** cross-verify full tier on draft PR #78 before this erratum: 160/160 cells match, 3 malformed cells pending under `PENDING_ERRATUM_2` (empty input, `58 05 01`, invalid UTF-8 `kind`). The SDKs agreed on exit code, `shape` and `shape_item`, and split only on the six fields below.

**Anchor (unchanged, verbatim):**

```text
`plant` is `n/a` only when `path_state` is `skip`. Then `P_g`, `height_P_g`, `d_g`, and `claimed_block` are `not defined`.
```

**Inserted after it:**

```text
 On a malformed CBOR value, `paths[g].path_state` and `paths[g].plant` for g in 1…16 are `not defined`. That is the one exception to the closed set in this paragraph. `skip_list` is `[]`. `analysis.per_stage[g].checkpoint_count` for g in 0…17, `analysis.whole_hike_legs`, and `analysis.hike_legs` are `not defined`. A missing stage is not a checkpoint count of zero, and `skip` is only an empty path byte string (Rule A4.5), which this input is not.
```

**Why:** main L1320 says `path_state` is `present` or `skip` and `plant` is `n/a` only when `path_state` is `skip`; main L1346 (Rule A4.1) says a malformed CBOR value is `shape` `mismatch` with `shape_item` exactly `["malformed cbor"]`, and that a value left without a usable object value is `not defined`. An empty file and `58 05 01` are not one CBOR data item, so there is no stage, no empty path byte string (the only `skip`, L1419) and no path body (`present`, L1420). L961 already leaves `whole_hike_legs` `not defined` when a stage is missing, so a missing stage is not a count of zero. L1328 then forces `skip_list` `[]`, since no `g` is `skip`. Under this edit the record of every malformed CBOR value has `paths[g].path_state` and `paths[g].plant` `not defined` for g in 1…16, `skip_list` `[]`, every `analysis.per_stage[g].checkpoint_count` `not defined`, and `analysis.whole_hike_legs` and `analysis.hike_legs` `not defined`. Every other field was already agreed. Before this edit: C wrote this record; Rust wrote `present`/`mismatch` and counts `0`; Go wrote `skip`/`n/a`, counts `0` and hike totals `0` (`skip` with `skip_list` `[]` also contradicted L1328); Java wrote `skip`/`n/a` and `skip_list` `[1…16]`.

### E03. Several words of one slot at once: §16.2 Rule I6

- **Where:** main L2148; erratum E-L2148.
- **Kind:** insert after the anchor (anchor unchanged).
- **Class:** clarification.
- **Status:** wording only. **Evidence:** White Hat's exactness check, 0 mismatches over 200,000 slots across scalar, AVX2, AVX-512 and reversed 8-lane folds; the Java timing note (exact JIT vectorisation).

**Anchor (unchanged, verbatim):**

```text
It is reviewed against the permanent negatives before release, and it is reported, not merged as an improvement.
```

**Inserted after it:**

```text
 Processing several words of one slot at once (SIMD, or compiler or JIT vectorisation) is the section 11.1 loop and is not an efficiency or a speed-up in the sense of section 11.1, provided that on every step all `SLOT_BYTES` bytes of the selected slot (16 KiB under the section 6 constants) are loaded, each of the `WORDS_PER_SLOT` words is mixed with the current `S` by `mix64` with `MIX_ADD`, and the sum is the section 11.1 wrapping sum. Such a kernel’s rate gain is reported under this rule with equality, mutation and load evidence, and after that review it may be released. Skipping, caching, summarising or reusing any word or value across steps remains a break.
```

**Why:** §11.1 (main L829, Christian's requirement) calls a producer speed-up a suspected break, and L831 says "flag it, and do not merge it". A reader could take that to forbid evaluating the §11.1 loop with SIMD, which loads the same bytes, mixes every word with the same `S` and gives the same bytes. L831's load rule is the substance: every step, all `SLOT_BYTES` bytes of the selected slot, each of the `WORDS_PER_SLOT` words mixed with the current `S` by the §11.1 loop including `mix64` with `MIX_ADD`. This sentence says that a vectorised evaluation meeting that rule is the loop, keeps every shortcut a break, and keeps the Rule I6 review (equality, mutation and load evidence) before release. §11.1 is frozen for errata, so the sentence sits in Rule I6.

### E04. Several words of one slot at once: §17.4 (new, informative)

- **Where:** new text before main L2172 (`## 18. Verify CLI`); erratum E-L2172–E-L2175.
- **Kind:** insert a new subsection before the anchor (anchor unchanged).
- **Class:** clarification (informative).
- **Status:** wording only.

**Anchor (unchanged, verbatim):**

```text
## 18. Verify CLI
```

**Inserted before it:**

```text
### 17.4 Several words of one slot at once

Section 11.1 folds the `WORDS_PER_SLOT` words of the selected slot with a wrapping sum mod 2^64. That sum is associative and commutative, so a kernel that loads all `SLOT_BYTES` bytes of the slot (16 KiB under the section 6 constants) on every step and mixes several of its words with the current `S` at once, by SIMD or by compiler or JIT vectorisation, gives the same `S_folded`, seal, leg, checkpoint, and waypoint bytes as the one-word-at-a-time loop. It is the section 11.1 loop, not an efficiency (Rule I6). Every step still loads every byte of the slot and mixes every word with that step’s `S`. A kernel that skips, caches, summarises, or reuses any word or value across steps is a break whatever its output on a vector, and the permanent negatives of section 16 stay non-matching.
```

**Why:** §17 restates parts of the construction without adding a second one. The restatement gives the reason SIMD is exact (the wrapping sum) next to the rule that every byte of the slot is loaded and every word mixed with that step's `S`, so an implementer finds both in one place.

## Dropped and not taken

- **E02 option B** (missing paths read as `skip`, `plant` `n/a`, `skip_list` `[1…16]`, counts `not defined`; Java's record). Not taken: L1419's `skip` is an empty path byte string on a decoded stage, which a malformed value does not have.
- **E02 option C** (`present`, `plant` `mismatch`, counts `0`; Rust's record). Not taken: L1420 and L1430 talk about path bytes that were read, and L961 forbids summing a missing stage as zero.
- **Vectorisation inside §11.1 itself.** Not an erratum (§11.1 is frozen for errata); 2.3 if wanted.
- **2.3-QUEUE Q04(b) and Q04(c)** (SHA-NI rule informative; floor calibration method) stay in the 2.3 queue.

## Reference values (for checking builds, never for CASES)

The record that `verify <file>` (no other arguments, `production` constants) prints for each of the empty input, `58 05 01`, and `a1 64 6b 69 6e 64 61 ff` is one line, identical for all three inputs, sha256 `eb7bff4d55ddcc47e47c600276693db8a406a0083a47c2526462651ffa5bdb8c` (C SDK on the Phase A branch). After this erratum the cross-verify gate (`sdk/conformance/harness.py`) has `PENDING_ERRATUM_2 = ()` and requires those three records byte-identical across Rust, C, Go and Java.

## Internal consistency

- §10–§13 byte-identical (above). No rule ID, P-row ID, section heading or `shape_item` string is added or removed, apart from the new heading `17.4`.
- `path_state` and `plant`: the closed sets of L1320 stand for every well-formed input; E02 names its one exception. L1328 (`skip_list`) and L1419–L1420 (Rule A4.5) are unchanged and consistent with E02.
- Counts: L961 and the `analysis` paragraph (L1338) are unchanged; E02 applies L961's missing-stage rule to every stage of a malformed value.
- E01 and L497's decode order: malformed first, so Rule A5 and the exit-1 decode failure do not apply.
- E03 and L831: E03 restates L831's load rule and adds no other condition.
- `groundinglab` case violations: 0.

## Open points noticed while drafting (not proposed)

1. **Invalid UTF-8 in a map key.** E01 covers the `kind` value only, as approved. All four SDKs already read every major-type-3 string whose bytes are not valid UTF-8, map keys included, as a malformed CBOR value (their `RESULTS.md` notes), but the cross-verify gate tests only the `kind` value. Widening E01 to every text string would be one more clarification of the same kind; it is left for Christian's call (or 2.3).
2. **Erratum 1 open point 3** (an empty input is a `malformed cbor` record, not exit 1) is unchanged; E02 now fixes every field of that record.
