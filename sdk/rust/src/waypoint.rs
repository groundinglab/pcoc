//! SPEC §12.2 waypoint fold and waypoint.

use crate::hash::sha256_parts;
use crate::profile::Profile;

/// SPEC §12.2. `F_0 = 0^32`, then `F_{j+1} = SHA-256(F_j ‖ C[j])`.
/// `n = 0` leaves `0^32` and performs no hash.
pub fn fold_checkpoints(checkpoints: &[[u8; 32]]) -> [u8; 32] {
    let mut f = [0u8; 32];
    for c in checkpoints {
        f = sha256_parts(&[&f, c]);
    }
    f
}

/// SPEC §12.2. The full slot selected by `u64_le(B[0:8]) mod N_SLOTS`.
pub fn slot_sample<'a>(terrain: &'a [u8], profile: &Profile, beacon: &[u8; 32]) -> &'a [u8] {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&beacon[..8]);
    let idx = (u64::from_le_bytes(raw) % profile.n_slots) as usize;
    let start = idx * profile.slot_bytes;
    &terrain[start..start + profile.slot_bytes]
}

/// SPEC §12.2. `W = SHA-256(B ‖ slot_sample ‖ W ‖ fold)`.
pub fn waypoint(
    beacon_ig: &[u8; 32],
    terrain: &[u8],
    profile: &Profile,
    stage_waypoint: &[u8; 32],
    fold: &[u8; 32],
) -> [u8; 32] {
    let slot = slot_sample(terrain, profile, beacon_ig);
    sha256_parts(&[beacon_ig, slot, stage_waypoint, fold])
}
