//! SPEC §11.1 step and §11.2 leg result.
//! Every step loads the whole slot and folds every word with `S`.

use crate::constants::SEAL_DOMAIN;
use crate::hash::sha256;
use crate::mix::mix64;
use crate::profile::Profile;

/// SPEC §11.1. One step. Returns `(S_folded, S')`.
/// `S_folded` is the accumulator after every word. `S'` is the next state.
pub fn step(terrain: &[u8], profile: &Profile, s: u64) -> (u64, u64) {
    let n_slots = profile.n_slots;
    let slot_bytes = profile.slot_bytes;
    let i = (s % n_slots) as usize;
    let start = i * slot_bytes;
    let slot = &terrain[start..start + slot_bytes];
    let words = profile.words_per_slot;
    let mut acc: u64 = 0;
    for w in 0..words {
        let off = w * 8;
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&slot[off..off + 8]);
        let word = u64::from_le_bytes(raw);
        acc = acc.wrapping_add(mix64(word ^ s));
    }
    let s_folded = acc;
    let mut pre = [0u8; 20];
    pre[..12].copy_from_slice(SEAL_DOMAIN);
    pre[12..].copy_from_slice(&s_folded.to_le_bytes());
    let e = sha256(&pre);
    let mut head = [0u8; 8];
    head.copy_from_slice(&e[..8]);
    let s_prime = u64::from_le_bytes(head);
    (s_folded, s_prime)
}

/// SPEC §11.1. The slot index a step reads: `S mod N_SLOTS`.
pub fn slot_index(profile: &Profile, s: u64) -> u64 {
    s % profile.n_slots
}

/// SPEC §11.2. `leg_result` after `STEPS_PER_LEG` steps from `previous_checkpoint`.
pub fn leg_result(terrain: &[u8], profile: &Profile, previous_checkpoint: &[u8; 32]) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&previous_checkpoint[..8]);
    let mut s = u64::from_le_bytes(raw);
    for _t in 0..profile.steps_per_leg {
        s = step(terrain, profile, s).1;
    }
    s
}
