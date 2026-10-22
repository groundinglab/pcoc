//! Permanent negatives. This module is the generator only. The SDK does not call it.
//! Each shortcut replaces the named part of the construction and keeps the seal.

use pcoc_sdk::constants::{PARENT_MUL, SEAL_DOMAIN, SLOT_MUL, WORD_XOR};
use pcoc_sdk::hash::sha256;
use pcoc_sdk::mix::{bitrev_parent, mix64, mul_parent, rotl};
use pcoc_sdk::profile::Profile;
use pcoc_sdk::terrain::{commit_slot, fill_terrain};

fn seal(s_folded: u64) -> u64 {
    let mut pre = [0u8; 20];
    pre[..12].copy_from_slice(SEAL_DOMAIN);
    pre[12..].copy_from_slice(&s_folded.to_le_bytes());
    let e = sha256(&pre);
    let mut head = [0u8; 8];
    head.copy_from_slice(&e[..8]);
    u64::from_le_bytes(head)
}

fn word_at(terrain: &[u8], slot_bytes: usize, slot: usize, w: usize) -> u64 {
    let off = slot * slot_bytes + w * 8;
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&terrain[off..off + 8]);
    u64::from_le_bytes(raw)
}

fn opening(previous: &[u8; 32]) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&previous[..8]);
    u64::from_le_bytes(raw)
}

/// SPEC §11.1 negative `slot_summary_fold`.
/// `S_folded = S XOR (XOR over w of (word(w) XOR w))`, then the seal.
fn step_slot_summary(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut x = 0u64;
    for w in 0..profile.words_per_slot {
        x ^= word_at(terrain, profile.slot_bytes, i, w) ^ (w as u64);
    }
    seal(s ^ x)
}

/// SPEC §11.1 negative `xor_accumulator_fold`.
fn step_xor_accumulator(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut acc = 0u64;
    for w in 0..profile.words_per_slot {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        acc ^= mix64(word ^ (w as u64) ^ s);
    }
    seal(acc)
}

/// SPEC §11.1 negative `xor_w_fold`.
fn step_xor_w(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut acc = 0u64;
    for w in 0..profile.words_per_slot {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        acc = acc.wrapping_add(mix64(word ^ (w as u64) ^ s));
    }
    seal(acc)
}

/// SPEC §11.1 negative `s_plus_w_fold`.
fn step_s_plus_w(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut acc = 0u64;
    for w in 0..profile.words_per_slot {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        acc = acc.wrapping_add(mix64(word ^ s.wrapping_add(w as u64)));
    }
    seal(acc)
}

/// SPEC §11.1 / F2 negative `prefold_hoist`.
/// Once per slot, `summary[i] = sum_w mix64(word(w) XOR w)` with no `S`.
/// Each step uses `S_folded = summary[i] + S`, then the seal.
/// The table lives only in this module.
fn prefold_summaries(terrain: &[u8], profile: &Profile) -> Vec<u64> {
    let n = profile.n_slots as usize;
    let mut summary = vec![0u64; n];
    for i in 0..n {
        let mut acc = 0u64;
        for w in 0..profile.words_per_slot {
            let word = word_at(terrain, profile.slot_bytes, i, w);
            acc = acc.wrapping_add(mix64(word ^ (w as u64)));
        }
        summary[i] = acc;
    }
    summary
}

fn step_prefold(summary: &[u64], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    seal(summary[i].wrapping_add(s))
}

/// SPEC §16 negative `partial_slot_read`. Folds every word except the last.
fn step_partial(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut acc = 0u64;
    let words = profile.words_per_slot;
    for w in 0..words.saturating_sub(1) {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        acc = acc.wrapping_add(mix64(word ^ s));
    }
    seal(acc)
}

fn run_leg(profile: &Profile, previous: &[u8; 32], mut one: impl FnMut(u64) -> u64) -> u64 {
    let mut s = opening(previous);
    for _ in 0..profile.steps_per_leg {
        s = one(s);
    }
    s
}

pub struct S1Legs {
    pub slot_summary_fold: u64,
    pub xor_accumulator_fold: u64,
    pub xor_w_fold: u64,
    pub s_plus_w_fold: u64,
    pub prefold_hoist: u64,
    pub partial_slot_read: u64,
    pub cached_leg: u64,
}

/// Leg-level shortcuts on one S1 terrain, from the stage-0 opening checkpoint.
pub fn s1_legs(terrain: &[u8], profile: &Profile, previous: &[u8; 32]) -> S1Legs {
    let summary = prefold_summaries(terrain, profile);
    S1Legs {
        slot_summary_fold: run_leg(profile, previous, |s| step_slot_summary(terrain, profile, s)),
        xor_accumulator_fold: run_leg(profile, previous, |s| {
            step_xor_accumulator(terrain, profile, s)
        }),
        xor_w_fold: run_leg(profile, previous, |s| step_xor_w(terrain, profile, s)),
        s_plus_w_fold: run_leg(profile, previous, |s| step_s_plus_w(terrain, profile, s)),
        prefold_hoist: run_leg(profile, previous, |s| step_prefold(&summary, profile, s)),
        partial_slot_read: run_leg(profile, previous, |s| step_partial(terrain, profile, s)),
        cached_leg: opening(previous),
    }
}

/// SPEC §11.1 negative `partial_slot_read_2047` at production width.
/// The production loop is `w = 0 … 2047` (2048 words). This reads `w = 0 … 2046`
/// (2047 words) and skips word index 2047, then applies the seal.
pub fn partial_slot_read_2047(terrain: &[u8], previous: &[u8; 32]) -> u64 {
    const SLOT_BYTES: usize = 16384;
    const WORDS: usize = 2048;
    const STEPS: u64 = 2_000_000;
    let n_slots = (terrain.len() / SLOT_BYTES) as u64;
    let mut s = opening(previous);
    for _ in 0..STEPS {
        let i = (s % n_slots) as usize;
        let mut acc = 0u64;
        for w in 0..2047 {
            let word = word_at(terrain, SLOT_BYTES, i, w);
            acc = acc.wrapping_add(mix64(word ^ s));
        }
        let _ = WORDS;
        s = seal(acc);
    }
    s
}

fn one_lane_word(lane: &[u64; 4], i: u64, j: u64) -> u64 {
    let c = (i << 32) ^ j ^ WORD_XOR;
    mix64(lane[(j % 4) as usize] ^ c)
}

fn all4_word(lane: &[u64; 4], i: u64, j: u64) -> u64 {
    let c = (i << 32) ^ j ^ WORD_XOR;
    let x = lane[(j % 4) as usize]
        ^ rotl(lane[((j + 1) % 4) as usize], 17)
        ^ rotl(lane[((j + 2) % 4) as usize], 31)
        ^ rotl(lane[((j + 3) % 4) as usize], 47)
        ^ c;
    mix64(x)
}

fn fill_with(profile: &Profile, lane: &[u64; 4], base: fn(&[u64; 4], u64, u64) -> u64) -> Vec<u8> {
    let mut terrain = vec![0u8; profile.terrain_bytes];
    let n_slots = profile.n_slots as usize;
    let mut words = vec![0u64; profile.words_per_slot];
    for i in 0..n_slots {
        for j in 0..profile.words_per_slot {
            words[j] = base(lane, i as u64, j as u64);
        }
        commit_slot(&mut terrain, profile.slot_bytes, i, &mut words);
    }
    let _ = (PARENT_MUL, SLOT_MUL, bitrev_parent, mul_parent);
    terrain
}

/// SPEC §10.4 informative one-lane map, then the honest parent mix and `slot_mix`.
pub fn one_lane_flip(profile: &Profile, lane: &[u64; 4]) -> [u8; 32] {
    sha256(&fill_with(profile, lane, one_lane_word))
}

/// SPEC §10.4 informative linear all4 map, then the honest parent mix and `slot_mix`.
pub fn all4_kernel(profile: &Profile, lane: &[u64; 4]) -> [u8; 32] {
    sha256(&fill_with(profile, lane, all4_word))
}

/// F4 shared terrain. The shortcut reports the terrain of a different seed.
pub fn f3_shared_terrain(profile: &Profile, other_lanes: &[u64; 4]) -> [u8; 32] {
    let mut terrain = vec![0u8; profile.terrain_bytes];
    fill_terrain(&mut terrain, other_lanes, profile);
    sha256(&terrain)
}

/// SPEC §10.2. A squeeze of the seed to one lane, then an otherwise honest dag2 fill.
pub fn squeezed_seed_terrain(profile: &Profile, lane: &[u64; 4]) -> [u8; 32] {
    let squeezed = [lane[0], lane[0], lane[0], lane[0]];
    let mut terrain = vec![0u8; profile.terrain_bytes];
    fill_terrain(&mut terrain, &squeezed, profile);
    sha256(&terrain)
}
