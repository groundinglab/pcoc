//! SPEC §16 permanent negatives. This module is the generator only.
//! Each shortcut replaces the named part of the construction and keeps the seal.

use pcoc::constants::{MIX_ADD, MIX_MUL1, MIX_MUL2, PARENT_MUL, SEAL_DOMAIN, SLOT_MUL, WORD_XOR};
use pcoc::hash::sha256;
use pcoc::mix::{bitrev_parent, mix64, mul_parent, rotl};
use pcoc::profile::Profile;
use pcoc::step::leg_result;
use pcoc::terrain::{commit_slot, fill_terrain};
use std::collections::HashMap;

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

/// SPEC §10.1 `mix64` without its last line. Generator only.
fn mix64_before_last(x: u64) -> u64 {
    let mut y = x.wrapping_add(MIX_ADD);
    y = (y ^ (y >> 30)).wrapping_mul(MIX_MUL1);
    (y ^ (y >> 27)).wrapping_mul(MIX_MUL2)
}

fn step_slot_summary(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut x = 0u64;
    for w in 0..profile.words_per_slot {
        x ^= word_at(terrain, profile.slot_bytes, i, w) ^ (w as u64);
    }
    seal(s ^ x)
}

fn step_xor_accumulator(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut acc = 0u64;
    for w in 0..profile.words_per_slot {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        acc ^= mix64(word ^ (w as u64) ^ s);
    }
    seal(acc)
}

fn step_xor_w(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut acc = 0u64;
    for w in 0..profile.words_per_slot {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        acc = acc.wrapping_add(mix64(word ^ (w as u64) ^ s));
    }
    seal(acc)
}

fn step_s_plus_w(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut acc = 0u64;
    for w in 0..profile.words_per_slot {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        acc = acc.wrapping_add(mix64(word ^ s.wrapping_add(w as u64)));
    }
    seal(acc)
}

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

/// SPEC §16 `partial_slot_read`. Word 0 only.
fn step_word0(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let word = word_at(terrain, profile.slot_bytes, i, 0);
    seal(mix64(word ^ s))
}

/// SPEC §16 `partial_slot_read_2047`. Words `0 … WORDS_PER_SLOT − 2`.
fn step_all_but_last(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut acc = 0u64;
    let last = profile.words_per_slot.saturating_sub(1);
    for w in 0..last {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        acc = acc.wrapping_add(mix64(word ^ s));
    }
    seal(acc)
}

/// SPEC §16 `final_stage_hoist`.
fn step_final_stage_hoist(terrain: &[u8], profile: &Profile, s: u64) -> u64 {
    let i = (s % profile.n_slots) as usize;
    let mut x = 0u64;
    for w in 0..profile.words_per_slot {
        let word = word_at(terrain, profile.slot_bytes, i, w);
        x ^= mix64_before_last(word ^ s);
    }
    seal(x ^ (x >> 31))
}

fn run_leg(profile: &Profile, previous: &[u8; 32], mut one: impl FnMut(u64) -> u64) -> u64 {
    let mut s = opening(previous);
    for _ in 0..profile.steps_per_leg {
        s = one(s);
    }
    s
}

/// SPEC §16 `cached_leg`. Reuses the first `S_folded` of a slot already read.
/// Returns the leg and `(step, slot, first_step)` of the first revisit.
pub fn cached_leg(terrain: &[u8], profile: &Profile, previous: &[u8; 32]) -> (u64, Option<(u64, u64, u64)>) {
    let mut seen: HashMap<u64, (u64, u64)> = HashMap::new();
    let mut s = opening(previous);
    let mut revisit = None;
    for step_i in 0..profile.steps_per_leg {
        let slot = s % profile.n_slots;
        let folded = if let Some((prior, first)) = seen.get(&slot).copied() {
            if revisit.is_none() {
                revisit = Some((step_i, slot, first));
            }
            prior
        } else {
            let (acc, _) = pcoc::step::step(terrain, profile, s);
            seen.insert(slot, (acc, step_i));
            acc
        };
        s = seal(folded);
    }
    (s, revisit)
}

pub struct LegNegatives {
    pub slot_summary_fold: u64,
    pub xor_accumulator_fold: u64,
    pub xor_w_fold: u64,
    pub s_plus_w_fold: u64,
    pub prefold_hoist: u64,
    pub final_stage_hoist: u64,
    pub partial_slot_read: u64,
    pub partial_slot_read_2047: u64,
    pub cached_leg: u64,
    pub cached_revisit: Option<(u64, u64, u64)>,
}

pub fn leg_negatives(terrain: &[u8], profile: &Profile, previous: &[u8; 32]) -> LegNegatives {
    let summary = prefold_summaries(terrain, profile);
    let (cached, revisit) = cached_leg(terrain, profile, previous);
    LegNegatives {
        slot_summary_fold: run_leg(profile, previous, |s| step_slot_summary(terrain, profile, s)),
        xor_accumulator_fold: run_leg(profile, previous, |s| step_xor_accumulator(terrain, profile, s)),
        xor_w_fold: run_leg(profile, previous, |s| step_xor_w(terrain, profile, s)),
        s_plus_w_fold: run_leg(profile, previous, |s| step_s_plus_w(terrain, profile, s)),
        prefold_hoist: run_leg(profile, previous, |s| step_prefold(&summary, profile, s)),
        final_stage_hoist: run_leg(profile, previous, |s| step_final_stage_hoist(terrain, profile, s)),
        partial_slot_read: run_leg(profile, previous, |s| step_word0(terrain, profile, s)),
        partial_slot_read_2047: run_leg(profile, previous, |s| step_all_but_last(terrain, profile, s)),
        cached_leg: cached,
        cached_revisit: revisit,
    }
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

pub fn one_lane_flip(profile: &Profile, lane: &[u64; 4]) -> [u8; 32] {
    sha256(&fill_with(profile, lane, one_lane_word))
}

pub fn all4_kernel(profile: &Profile, lane: &[u64; 4]) -> [u8; 32] {
    sha256(&fill_with(profile, lane, all4_word))
}

/// SPEC §16 `f3_shared_terrain`. `pk_A`'s opening leg on `pk_B`'s terrain.
pub fn f3_shared_terrain(profile: &Profile, opening_a: &[u8; 32], lanes_b: &[u64; 4]) -> u64 {
    let mut terrain = vec![0u8; profile.terrain_bytes];
    fill_terrain(&mut terrain, lanes_b, profile);
    leg_result(&terrain, profile, opening_a)
}

pub fn squeezed_seed_terrain(profile: &Profile, lane: &[u64; 4]) -> [u8; 32] {
    let x = lane[0] ^ lane[1] ^ lane[2] ^ lane[3];
    let squeezed = [x, x, x, x];
    let mut terrain = vec![0u8; profile.terrain_bytes];
    fill_terrain(&mut terrain, &squeezed, profile);
    sha256(&terrain)
}
