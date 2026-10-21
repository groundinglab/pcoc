use crate::consts::*;
use crate::sha::sha256;
use crate::terrain::mix64;

/// Next leg state after one step. `s_folded` is the integer folded into the seal.
pub fn seal(s_folded: u64) -> u64 {
    let mut pre = [0u8; 20];
    pre[..12].copy_from_slice(SEAL_DOMAIN);
    pre[12..].copy_from_slice(&s_folded.to_le_bytes());
    debug_assert_eq!(pre.len(), 20);
    let digest = sha256(&pre);
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

pub fn step(terrain: &[u8], state: u64) -> u64 {
    step_sized(terrain, state, Sizes::production())
}

pub fn step_sized(terrain: &[u8], state: u64, sizes: Sizes) -> u64 {
    let n_slots = sizes.n_slots() as u64;
    let i = (state % n_slots) as usize;
    let start = i * sizes.slot_bytes;
    let slot = &terrain[start..start + sizes.slot_bytes];
    // C1: acc starts at 0. Every word of the slot is mixed with w and S.
    // mix64 includes MIX_ADD. The seal is the existing SHA-256 of that acc.
    let words = sizes.words_per_slot();
    debug_assert_eq!(slot.len(), words * 8);
    let mut acc = 0u64;
    for w in 0..words {
        let word = u64::from_le_bytes(slot[w * 8..w * 8 + 8].try_into().unwrap());
        acc ^= mix64(word ^ (w as u64) ^ state);
    }
    seal(acc)
}

pub fn leg_result(terrain: &[u8], previous_checkpoint: &[u8; 32]) -> u64 {
    let mut state = u64::from_le_bytes(previous_checkpoint[..8].try_into().unwrap());
    for _ in 0..STEPS_PER_LEG {
        state = step(terrain, state);
    }
    state
}

pub fn leg_result_sized(terrain: &[u8], previous_checkpoint: &[u8; 32], sizes: Sizes) -> u64 {
    let mut state = u64::from_le_bytes(previous_checkpoint[..8].try_into().unwrap());
    for _ in 0..sizes.steps_per_leg {
        state = step_sized(terrain, state, sizes);
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Section 11.1. The seal preimage is `SEAL_DOMAIN ‖ le64(S_folded)`, 20 bytes.
    #[test]
    fn p11_1_seal_preimage_is_domain_then_le64() {
        let folded = 0x0102u64;
        let mut pre = Vec::new();
        pre.extend_from_slice(SEAL_DOMAIN);
        pre.extend_from_slice(&folded.to_le_bytes());
        assert_eq!(pre.len(), 20);
        assert_eq!(&pre[..12], b"pcoc-walk-v1");
        let digest = crate::sha::sha256(&pre);
        assert_eq!(
            seal(folded),
            u64::from_le_bytes(digest[..8].try_into().unwrap())
        );
    }

    #[test]
    fn seal_zero_matches_spec() {
        // SHA-256(SEAL_DOMAIN ‖ le64(0)) begins 4816bdaf23240476.
        assert_eq!(
            seal(0).to_le_bytes(),
            [0x48, 0x16, 0xbd, 0xaf, 0x23, 0x24, 0x04, 0x76]
        );
        assert_eq!(seal(0), 0x76042423afbd1648);
    }

    /// Section 11. A leg is exactly `steps_per_leg` steps. The small profile uses 32, not 2_000_000.
    #[test]
    fn p11_2_small_leg_runs_steps_per_leg() {
        let sizes = Sizes::conformance_small();
        let terrain = vec![0x7u8; sizes.terrain_bytes];
        let previous = [0x10u8; 32];
        let mut state = u64::from_le_bytes(previous[..8].try_into().unwrap());
        let start = state;
        for _ in 0..sizes.steps_per_leg {
            state = step_sized(&terrain, state, sizes);
        }
        assert_eq!(sizes.steps_per_leg, 32);
        assert_ne!(state, start);
        assert_eq!(leg_result_sized(&terrain, &previous, sizes), state);
    }

    /// C1 on four words. mix64 includes MIX_ADD. The bare finalizer is a different acc.
    #[test]
    fn c1_fold_includes_mix_add() {
        let words = [
            0x1111_1111_1111_1111u64,
            0x2222_2222_2222_2222,
            0x3333_3333_3333_3333,
            0x4444_4444_4444_4444,
        ];
        let state = 0x0123_4567_89ab_cdefu64;
        let mut acc = 0u64;
        for (w, word) in words.iter().enumerate() {
            acc ^= mix64(word ^ (w as u64) ^ state);
        }
        assert_eq!(acc, 0x2af1_8a27_e8cd_3106);
        let mut bare = 0u64;
        for (w, word) in words.iter().enumerate() {
            let x = word ^ (w as u64) ^ state;
            let mut y = x;
            y = (y ^ (y >> 30)).wrapping_mul(crate::consts::MIX_MUL1);
            y = (y ^ (y >> 27)).wrapping_mul(crate::consts::MIX_MUL2);
            bare ^= y ^ (y >> 31);
        }
        assert_ne!(bare, acc);
        assert_eq!(bare, 0x1b1f_889a_704d_a59d);
    }
}
