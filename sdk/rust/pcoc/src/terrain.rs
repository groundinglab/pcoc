//! SPEC §10. Seed lanes and the dag2 terrain fill.
//! The terrain is filled once per seed. The step does not write it.

use crate::constants::{PARENT_MUL, SLOT_MUL, WORD_XOR};
use crate::hash::sha256;
use crate::mix::{bitrev_parent, mix64, mul_parent, rotl};
use crate::profile::Profile;

/// SPEC §10. `seed_preimage = public_key ‖ seed_beacon`, then `D = SHA-256(seed_preimage)`.
pub fn seed(public_key: &[u8; 32], seed_beacon: &[u8; 32]) -> [u8; 32] {
    let mut pre = [0u8; 64];
    pre[..32].copy_from_slice(public_key);
    pre[32..].copy_from_slice(seed_beacon);
    sha256(&pre)
}

/// SPEC §10. The 64-byte seed preimage, no label and no separator.
pub fn seed_preimage(public_key: &[u8; 32], seed_beacon: &[u8; 32]) -> [u8; 64] {
    let mut pre = [0u8; 64];
    pre[..32].copy_from_slice(public_key);
    pre[32..].copy_from_slice(seed_beacon);
    pre
}

/// SPEC §10. `P[k] = u64_le(D[8k : 8k+8])` for `k = 0 … 3`.
pub fn lanes(seed_d: &[u8; 32]) -> [u64; 4] {
    let mut p = [0u64; 4];
    for k in 0..4 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&seed_d[8 * k..8 * k + 8]);
        p[k] = u64::from_le_bytes(b);
    }
    p
}

/// SPEC §10.4. The NL4 base word at slot `i`, word `j`. This is the base, before parents.
pub fn nl4_base_word(lane: &[u64; 4], i: u64, j: u64) -> u64 {
    let c = (i << 32) ^ j ^ WORD_XOR;
    let mut t = mix64(lane[(j % 4) as usize] ^ c);
    t = mix64(t ^ rotl(lane[((j + 1) % 4) as usize], 17));
    t = mix64(t ^ rotl(lane[((j + 2) % 4) as usize], 31));
    t = mix64(t ^ rotl(lane[((j + 3) % 4) as usize], 47));
    t
}

fn read_stored_word(terrain: &[u8], slot_bytes: usize, slot: usize, j: usize) -> u64 {
    let off = slot * slot_bytes + j * 8;
    let mut b = [0u8; 8];
    b.copy_from_slice(&terrain[off..off + 8]);
    u64::from_le_bytes(b)
}

/// SPEC §10.4. Apply the three parents (when `i > 0`) and `slot_mix` (when `i > 0`),
/// then write `le64(w[j])` into slot `i`. `words` arrives as the base words.
/// Parents are read from terrain bytes already stored, not from `words`.
pub fn commit_slot(terrain: &mut [u8], slot_bytes: usize, i: usize, words: &mut [u64]) {
    let n_words = words.len();
    if i > 0 {
        let iu = i as u64;
        let parents = [iu - 1, bitrev_parent(iu), mul_parent(iu)];
        for k in 1u64..=3 {
            let p = parents[(k as usize) - 1] as usize;
            let k_term = k.wrapping_mul(PARENT_MUL);
            for j in 0..n_words {
                let pw = read_stored_word(terrain, slot_bytes, p, j);
                let rot = ((j as u32) + (k as u32)) % 64;
                let mixed = (words[j] ^ rotl(pw, rot)) ^ k_term;
                words[j] = mix64(mixed);
            }
        }
        let slot_mix = iu.wrapping_mul(SLOT_MUL);
        for j in 0..n_words {
            words[j] = mix64(words[j] ^ slot_mix);
        }
    }
    let base = i * slot_bytes;
    for j in 0..n_words {
        let off = base + j * 8;
        terrain[off..off + 8].copy_from_slice(&words[j].to_le_bytes());
    }
}

/// SPEC §10.4. Fill `terrain` once from `lane`. `terrain.len()` is the profile terrain size.
/// Slot 0 keeps the NL4 words: no parents and no slot mix.
pub fn fill_terrain(terrain: &mut [u8], lane: &[u64; 4], profile: &Profile) {
    assert_eq!(terrain.len(), profile.terrain_bytes);
    terrain.fill(0);
    let n_slots = profile.n_slots as usize;
    let slot_bytes = profile.slot_bytes;
    let n_words = profile.words_per_slot;
    let mut words = vec![0u64; n_words];
    for i in 0..n_slots {
        for j in 0..n_words {
            words[j] = nl4_base_word(lane, i as u64, j as u64);
        }
        commit_slot(terrain, slot_bytes, i, &mut words);
    }
}

/// SPEC §10. Build the terrain for one seed. One fill for this PCOC.
pub fn terrain_from_seed(
    public_key: &[u8; 32],
    seed_beacon: &[u8; 32],
    profile: &Profile,
) -> (Vec<u8>, [u8; 32], [u64; 4]) {
    let d = seed(public_key, seed_beacon);
    let lane = lanes(&d);
    let mut terrain = vec![0u8; profile.terrain_bytes];
    fill_terrain(&mut terrain, &lane, profile);
    (terrain, d, lane)
}
