//! SPEC §10.1 `mix64` and §10.3 parent indices.

use crate::constants::{MIX_ADD, MIX_MUL1, MIX_MUL2};

/// SPEC §10.1. `mix64`, including the initial add of `MIX_ADD`.
pub fn mix64(x: u64) -> u64 {
    let mut y = x.wrapping_add(MIX_ADD);
    y = (y ^ (y >> 30)).wrapping_mul(MIX_MUL1);
    y = (y ^ (y >> 27)).wrapping_mul(MIX_MUL2);
    y ^ (y >> 31)
}

/// SPEC §10.3. Rotate left. `r` is in `0 … 63`. `rotl(w, 0) = w`.
pub fn rotl(w: u64, r: u32) -> u64 {
    w.rotate_left(r)
}

/// SPEC §10.3. Greatest `b` with `2^b ≤ n`, for `n ≥ 1`.
pub fn floor_log2(n: u64) -> u32 {
    debug_assert!(n >= 1);
    63 - n.leading_zeros()
}

/// SPEC §10.3. `bitrev_parent`. The result is in `0 … i − 1`.
pub fn bitrev_parent(i: u64) -> u64 {
    debug_assert!(i >= 1);
    let bits = floor_log2(i).max(1);
    let rev = (i as u32).reverse_bits() as u64;
    (rev >> (32 - bits)) % i
}

/// SPEC §10.3. `mul_parent`. The result is in `0 … i − 1`.
pub fn mul_parent(i: u64) -> u64 {
    debug_assert!(i >= 1);
    let h = i.wrapping_mul(MIX_ADD);
    (h >> 32) % i
}
