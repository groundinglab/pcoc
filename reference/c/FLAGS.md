# FLAGS — GB-I2-C

No producer speed-up is implemented. None of the withdrawn constructions below are in the terrain fill, the fold, or the hike.

Withdrawn constructions that are not implemented. `make test` rejects each published forbidden value:

- fill pin on the terrain
- REF word map
- ALL4 word map
- `mix64` without `MIX_ADD`
- v2.0 fold (`S` XOR a per-slot value that ignores `S`)
- XOR accumulator (`acc ^= mix64(word XOR w XOR S)`), the negative `xor_accumulator_fold`
- `xor_w_fold` (`acc += mix64(word XOR w XOR S)`). `word XOR w` does not depend on `S`
- `s_plus_w_fold` (`acc += mix64(word XOR (S + w))`), the withdrawn FIX5 per-word mix
- `prefold_hoist`, which stores `word XOR w` in place of the terrain word
- partial slot read, including a read of 2047 words
- a shifted terrain window used as checkpoint limb 3
- a cached leg result, including one whose checkpoint digest was rebuilt from the stolen result
- one terrain shared by two public keys (`f3_shared_terrain`)

Also absent, because section 11.1 forbids them: a per-slot digest, a word lookup table, a leg-result cache, and any step that reads fewer than every word of the selected slot.

FLAG FOR CHRISTIAN: the last stage of `mix64` is `y XOR (y >> 31)`, which is linear over GF(2). An XOR of `mix64` outputs would let a producer apply that stage once to the XOR of the pre-final values. The final §11.1 accumulator is wrapping addition, so that hoist does not compute this fold. Not implemented.

FLAG FOR CHRISTIAN: `word XOR w` does not depend on `S`. A producer could store that term in place of the terrain word. That is `prefold_hoist` and the withdrawn `xor_w_fold`. Section 11.1 has no `w` term. The stored terrain is the §10.4 fill. Not implemented.

FLAG FOR CHRISTIAN: equal words do not cancel under wrapping addition. Skipping a repeated word, which an XOR accumulator would allow, does not compute this fold. Not implemented.

SHA-256 is a self-contained compress. The SHA-NI compress is compiled under a function target attribute (`sha,ssse3,sse4.1`) and is selected only when CPUID.7.0:EBX[29] is set and that compress matches the software compress on the empty-message block. This host is an Intel Core i7-10610U. Its flags do not include `sha_ni`. `hike_sha256_hw` is 0. Software SHA-256 is the path section 11.1 permits on this host. OpenSSL is not linked. The binary is not built with `-msha`.

The build is `-O2` and nothing else aimed at the hike. That is ordinary compilation of the specified loops. It does not replace a step with a stored slot value.

The FIPS K256 round constants and the SHA-256 message schedule belong to the compression function. They are not a hike table. `reverse32` is a shift-swap. During a fill, one slot of `w[j]` is held until that slot is stored, which is the section 10.4 word array, and then freed. A step does not read it. A step loads every word from the terrain bytes and mixes it with the state.
