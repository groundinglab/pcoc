# SPEC-FINDINGS — Satoshi Hike v2.1

## Closed by the final text

The draft at `ba434c1` had a stale §16 catalogue (then line 2032, and later sentences of the vectors paragraph) that still described the v2.0 object pack: `object_hex`, `draw_bytes_hex`, `anchor-window`. The final spec at `3ee7bc0` replaces that catalogue. It names the v2.1 files, says `legs_le_hex` is the little-endian wire spelling, and says this pack does not publish a hike-object encoding, a section 14 record, `object_hex`, or `draw_bytes_hex`.

That draft finding is closed. FIX5-i (`aeb13dc`) sets the §11.1 per-word input to `word XOR S`. There is no `w` term. The reference follows that text. `xor_w_fold`, `s_plus_w_fold`, `prefold_hoist`, and the production leg match the vectors. This resync found no new conflict.
