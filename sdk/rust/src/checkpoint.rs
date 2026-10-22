//! SPEC §11.3 terrain sample and §11.4 checkpoint.

use crate::hash::sha256_parts;
use crate::profile::Profile;

/// SPEC §11.3. `offset = u64_le(previous[0:8]) mod OFFSET_MODULUS`.
pub fn sample_offset(previous: &[u8; 32], profile: &Profile) -> usize {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&previous[..8]);
    let off = u64::from_le_bytes(raw) % profile.offset_modulus;
    off as usize
}

/// SPEC §11.3. The `SAMPLE_BYTES` window at `previous`. Not slot-aligned.
pub fn terrain_sample<'a>(
    terrain: &'a [u8],
    profile: &Profile,
    previous: &[u8; 32],
) -> &'a [u8] {
    let off = sample_offset(previous, profile);
    &terrain[off..off + profile.sample_bytes]
}

/// SPEC §11.4. Checkpoint digest.
/// Preimage is previous ‖ stage limb ‖ terrain sample ‖ le64(leg_result).
pub fn checkpoint(
    previous: &[u8; 32],
    stage_limb: &[u8; 32],
    terrain: &[u8],
    profile: &Profile,
    leg_result: u64,
) -> [u8; 32] {
    let sample = terrain_sample(terrain, profile, previous);
    let limb = leg_result.to_le_bytes();
    sha256_parts(&[previous, stage_limb, sample, &limb])
}
