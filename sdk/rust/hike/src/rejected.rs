//! Constructions verify and the gate must reject.
//!
//! Produce does not call this module. Each function is a known break: a terrain
//! that does not depend on all 256 seed bits, a fold that drops `MIX_ADD`, the
//! XOR accumulator, the v2.0 fold, a partial slot read, or a per-slot cache.

use crate::consts::*;
use crate::terrain::{self, fill_from_bases, lanes_of, mix64};
use crate::walk::seal;

const OLD_SEED_DOMAIN: [u8; 8] = *b"PSEED64\0";

/// 64-bit `fill_pin` terrain. The deleted `SEED_DOMAIN` label is inside this function only.
pub fn fill_pin_terrain(preimage: &[u8; 64], sizes: Sizes) -> Vec<u8> {
    let pin = fill_pin(preimage);
    fill_from_bases(sizes, move |i, j| {
        mix64(pin ^ (i << 32) ^ j ^ WORD_XOR)
    })
}

fn fill_pin(preimage: &[u8; 64]) -> u64 {
    let mut x = u64::from_le_bytes(OLD_SEED_DOMAIN);
    for i in 0..8 {
        let lane = u64::from_le_bytes(preimage[i * 8..i * 8 + 8].try_into().unwrap());
        x = mix64(x ^ lane ^ (i as u64).wrapping_mul(MIX_ADD));
    }
    x
}

/// REF word map: one lane only.
pub fn fill_ref(seed: &[u8; 32], sizes: Sizes) -> Vec<u8> {
    let lanes = lanes_of(seed);
    fill_from_bases(sizes, move |i, j| {
        let c = (i << 32) ^ j ^ WORD_XOR;
        mix64(lanes[(j as usize) & 3] ^ c)
    })
}

/// ALL4 word map: one `mix64` of the XOR of all four lanes. The two-lane kernel is invisible to it.
pub fn fill_all4(seed: &[u8; 32], sizes: Sizes) -> Vec<u8> {
    let lanes = lanes_of(seed);
    fill_from_bases(sizes, move |i, j| {
        let c = (i << 32) ^ j ^ WORD_XOR;
        let r = (j as usize) & 3;
        mix64(
            lanes[r]
                ^ lanes[(r + 1) & 3].rotate_left(17)
                ^ lanes[(r + 2) & 3].rotate_left(31)
                ^ lanes[(r + 3) & 3].rotate_left(47)
                ^ c,
        )
    })
}

/// Fold that uses the bare splitmix finalizer. `MIX_ADD` is missing. That is wrong.
/// The accumulator is still wrapping addition. Only the mixer is bare.
pub fn step_bare_finalizer(terrain: &[u8], state: u64, sizes: Sizes) -> u64 {
    fold_add(terrain, state, sizes, |word, _w, state| {
        bare_mix(word ^ state)
    })
}

/// Withdrawn per-word input `word XOR w XOR S`, still with wrapping addition.
/// `word XOR w` does not depend on `S`. Vector key `xor_w_fold`.
pub fn step_xor_w(terrain: &[u8], state: u64, sizes: Sizes) -> u64 {
    fold_add(terrain, state, sizes, |word, w, state| {
        mix64(word ^ w ^ state)
    })
}

/// Withdrawn c886ef2 fold: `mix64(word XOR (S + w))` with wrapping addition.
/// Vector key `s_plus_w_fold`. There is no such `w` term in §11.1.
pub fn step_s_plus_w(terrain: &[u8], state: u64, sizes: Sizes) -> u64 {
    fold_add(terrain, state, sizes, |word, w, state| {
        mix64(word ^ state.wrapping_add(w))
    })
}

/// Replaces each stored word with `word XOR w`. That array is not the §10.4
/// terrain. The honest §11.1 step on it does not match the real slot.
/// Vector key `prefold_hoist`.
pub fn terrain_word_xor_w(terrain: &[u8], sizes: Sizes) -> Vec<u8> {
    let mut out = terrain.to_vec();
    let words = sizes.words_per_slot();
    for slot in 0..sizes.n_slots() {
        let start = slot * sizes.slot_bytes;
        for w in 0..words {
            let off = start + w * 8;
            let word = u64::from_le_bytes(out[off..off + 8].try_into().unwrap());
            out[off..off + 8].copy_from_slice(&(word ^ (w as u64)).to_le_bytes());
        }
    }
    out
}

/// Withdrawn XOR accumulator: `acc ^= mix64(word(w) ^ w ^ S)`. The v2.1 fold adds.
pub fn step_xor_acc(terrain: &[u8], state: u64, sizes: Sizes) -> u64 {
    fold_xor(terrain, state, sizes, |word, w, state| {
        mix64(word ^ w ^ state)
    })
}

/// The v2.0 fold: `S XOR (XOR of (word XOR w))`. The slot collapses to one value independent of `S`.
/// Vector key `v2_0_fold`.
pub fn step_v2_0_fold(terrain: &[u8], state: u64, sizes: Sizes) -> u64 {
    let slot = slot_of(terrain, state, sizes);
    let mut folded = state;
    for (w, word) in slot.iter().enumerate() {
        folded ^= word ^ (w as u64);
    }
    seal(folded)
}

/// Reads the first word and ignores the rest of the slot.
pub fn step_partial(terrain: &[u8], state: u64, sizes: Sizes) -> u64 {
    step_partial_words(terrain, state, sizes, 1)
}

/// Reads `keep` words and drops the rest. `keep` of 2047 is still a partial read.
pub fn step_partial_words(terrain: &[u8], state: u64, sizes: Sizes, keep: usize) -> u64 {
    let slot = slot_of(terrain, state, sizes);
    let mut acc = 0u64;
    for word in slot.iter().take(keep) {
        acc = acc.wrapping_add(mix64(*word ^ state));
    }
    seal(acc)
}

/// Builds one u64 per slot, then steps from that table. The slot is not read again.
pub fn leg_result_cached(terrain: &[u8], previous: &[u8; 32], sizes: Sizes) -> u64 {
    let words = sizes.words_per_slot();
    let mut table = vec![0u64; sizes.n_slots()];
    for (i, acc) in table.iter_mut().enumerate() {
        let start = i * sizes.slot_bytes;
        let mut folded = 0u64;
        for w in 0..words {
            let off = start + w * 8;
            let word = u64::from_le_bytes(terrain[off..off + 8].try_into().unwrap());
            folded ^= word ^ (w as u64);
        }
        *acc = folded;
    }
    let mut state = u64::from_le_bytes(previous[..8].try_into().unwrap());
    for _ in 0..sizes.steps_per_leg {
        let i = (state % sizes.n_slots() as u64) as usize;
        state = seal(state ^ table[i]);
    }
    state
}

pub fn leg_result_with(
    terrain: &[u8],
    previous: &[u8; 32],
    sizes: Sizes,
    step: fn(&[u8], u64, Sizes) -> u64,
) -> u64 {
    let mut state = u64::from_le_bytes(previous[..8].try_into().unwrap());
    for _ in 0..sizes.steps_per_leg {
        state = step(terrain, state, sizes);
    }
    state
}

fn bare_mix(x: u64) -> u64 {
    let mut y = x;
    y = (y ^ (y >> 30)).wrapping_mul(MIX_MUL1);
    y = (y ^ (y >> 27)).wrapping_mul(MIX_MUL2);
    y ^ (y >> 31)
}

/// Rejected accumulator. The honest step uses wrapping add, not this XOR.
fn fold_xor(terrain: &[u8], state: u64, sizes: Sizes, mix: impl Fn(u64, u64, u64) -> u64) -> u64 {
    let slot = slot_of(terrain, state, sizes);
    let mut acc = 0u64;
    for (w, word) in slot.iter().enumerate() {
        acc ^= mix(*word, w as u64, state);
    }
    seal(acc)
}

fn fold_add(terrain: &[u8], state: u64, sizes: Sizes, mix: impl Fn(u64, u64, u64) -> u64) -> u64 {
    let slot = slot_of(terrain, state, sizes);
    let mut acc = 0u64;
    for (w, word) in slot.iter().enumerate() {
        acc = acc.wrapping_add(mix(*word, w as u64, state));
    }
    seal(acc)
}

fn slot_of(terrain: &[u8], state: u64, sizes: Sizes) -> Vec<u64> {
    let i = (state % sizes.n_slots() as u64) as usize;
    let start = i * sizes.slot_bytes;
    let mut words = Vec::with_capacity(sizes.words_per_slot());
    for w in 0..sizes.words_per_slot() {
        let off = start + w * 8;
        words.push(u64::from_le_bytes(
            terrain[off..off + 8].try_into().unwrap(),
        ));
    }
    words
}

/// Honest NL4 terrain, so a rejected fold is the only difference.
pub fn honest_terrain(public_key: &[u8; 32], seed_beacon: &[u8; 32], sizes: Sizes) -> Vec<u8> {
    terrain::fill_terrain_sized(&terrain::seed(public_key, seed_beacon), sizes)
}
