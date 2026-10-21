use crate::consts::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// How many times this process has built a terrain.
///
/// The fast phase must not increment it. A test reads it before and after a
/// settled refuse. Production does not branch on the count.
static TERRAIN_BUILDS: AtomicU64 = AtomicU64::new(0);

pub fn terrain_builds() -> u64 {
    TERRAIN_BUILDS.load(Ordering::Relaxed)
}

pub fn mix64(x: u64) -> u64 {
    let mut y = x.wrapping_add(MIX_ADD);
    y = (y ^ (y >> 30)).wrapping_mul(MIX_MUL1);
    y = (y ^ (y >> 27)).wrapping_mul(MIX_MUL2);
    y ^ (y >> 31)
}

/// Seed preimage: `public_key ‖ seed_beacon`, 64 bytes, no label and no hash.
pub fn production_seed(public_key: &[u8; 32], seed_beacon: &[u8; 32]) -> [u8; 64] {
    seed_preimage(public_key, seed_beacon)
}

/// The 64 raw bytes `public_key ‖ seed_beacon`. This is not the seed.
pub fn seed_preimage(public_key: &[u8; 32], seed_beacon: &[u8; 32]) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(public_key);
    out[32..].copy_from_slice(seed_beacon);
    out
}

/// The seed `D = SHA-256(public_key ‖ seed_beacon)`. No domain label.
///
/// `stages[0].waypoint` is this digest. That equality is intentional.
pub fn seed(public_key: &[u8; 32], seed_beacon: &[u8; 32]) -> [u8; 32] {
    crate::sha::sha256(&seed_preimage(public_key, seed_beacon))
}

/// SHA-256 of a 64-byte seed preimage. Same bytes as [`seed`].
pub fn seed_from_preimage(preimage: &[u8; 64]) -> [u8; 32] {
    crate::sha::sha256(preimage)
}

/// Four little-endian lanes of the 32-byte seed. Every terrain word reads all four.
pub fn lanes_of(seed: &[u8; 32]) -> [u64; 4] {
    let mut lanes = [0u64; 4];
    for (k, lane) in lanes.iter_mut().enumerate() {
        *lane = u64::from_le_bytes(seed[k * 8..k * 8 + 8].try_into().unwrap());
    }
    lanes
}

/// NL4 base word. `c` mixes the coordinates; each `mix64` folds in one rotated lane.
pub fn base_word_nl4(lanes: &[u64; 4], i: u64, j: u64) -> u64 {
    let c = (i << 32) ^ j ^ WORD_XOR;
    let r = (j as usize) & 3;
    let mut t = mix64(lanes[r] ^ c);
    t = mix64(t ^ lanes[(r + 1) & 3].rotate_left(17));
    t = mix64(t ^ lanes[(r + 2) & 3].rotate_left(31));
    t = mix64(t ^ lanes[(r + 3) & 3].rotate_left(47));
    t
}

pub(crate) fn bitrev_parent(i: usize) -> usize {
    let i = i as u32;
    let bits = i.ilog2().max(1);
    let rev = i.reverse_bits() >> (32 - bits);
    (rev % i) as usize
}

pub(crate) fn mul_parent(i: usize) -> usize {
    let h = (i as u64).wrapping_mul(MIX_ADD);
    ((h >> 32) % (i as u64)) as usize
}

/// dag2 terrain from the 32-byte seed. `TERRAIN_BYTES` long. Filled once per terrain.
pub fn fill_terrain(seed: &[u8; 32]) -> Vec<u8> {
    fill_terrain_sized(seed, Sizes::production())
}

/// dag2 terrain at `sizes`. Slot 0 is NL4 base words only. Parents and `slot_mix` are unchanged.
///
/// Every base word depends on all 256 bits of `seed`. The fill reads no smaller digest.
pub fn fill_terrain_sized(seed: &[u8; 32], sizes: Sizes) -> Vec<u8> {
    let lanes = lanes_of(seed);
    fill_from_bases(sizes, |i, j| base_word_nl4(&lanes, i, j))
}

pub(crate) fn fill_from_bases(sizes: Sizes, mut base_word: impl FnMut(u64, u64) -> u64) -> Vec<u8> {
    TERRAIN_BUILDS.fetch_add(1, Ordering::Relaxed);
    let words_per_slot = sizes.words_per_slot();
    let slot_bytes = sizes.slot_bytes;
    let mut terrain = vec![0u8; sizes.terrain_bytes];
    let mut words = vec![0u64; words_per_slot];
    for i in 0..sizes.n_slots() {
        for j in 0..words_per_slot {
            words[j] = base_word(i as u64, j as u64);
        }
        if i > 0 {
            let parents = [i - 1, bitrev_parent(i), mul_parent(i)];
            for (p_idx, parent) in parents.into_iter().enumerate() {
                let k = (p_idx as u64) + 1;
                let k_term = k.wrapping_mul(PARENT_MUL);
                let base = parent * slot_bytes;
                for j in 0..words_per_slot {
                    let pw = u64::from_le_bytes(
                        terrain[base + j * 8..base + j * 8 + 8].try_into().unwrap(),
                    );
                    let rot = ((j as u32).wrapping_add(k as u32)) & 63;
                    let x = words[j] ^ pw.rotate_left(rot);
                    words[j] = mix64(x ^ k_term);
                }
            }
            let slot_mix = (i as u64).wrapping_mul(SLOT_MUL);
            for j in 0..words_per_slot {
                words[j] = mix64(words[j] ^ slot_mix);
            }
        }
        let base = i * slot_bytes;
        for (j, word) in words.iter().enumerate() {
            terrain[base + j * 8..base + j * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }
    }
    terrain
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seed preimage is the key followed by the beacon. The seed is SHA-256 of that, with no label.
    #[test]
    fn p10_1_seed_is_sha256_of_key_then_beacon() {
        let key = [0x11u8; 32];
        let beacon = [0x22u8; 32];
        let preimage = seed_preimage(&key, &beacon);
        assert_eq!(&preimage[..32], &key);
        assert_eq!(&preimage[32..], &beacon);
        assert_eq!(preimage.len(), 64);
        assert_eq!(production_seed(&key, &beacon), preimage);
        let digest = seed(&key, &beacon);
        assert_eq!(
            digest,
            [
                0x51, 0x89, 0xc7, 0x7d, 0x29, 0xfe, 0x5d, 0x54, 0x6a, 0x04, 0x5e, 0xc4, 0x69, 0x86,
                0x85, 0x27, 0x85, 0xfe, 0xa5, 0xc1, 0x3a, 0xc7, 0xda, 0x9c, 0x11, 0x5f, 0xf5, 0xfb,
                0x6e, 0xdf, 0x81, 0x7c,
            ]
        );
        assert_eq!(seed_from_preimage(&preimage), digest);
        let lanes = lanes_of(&digest);
        assert_eq!(
            lanes,
            [
                0x545dfe297dc78951,
                0x27858669c45e046a,
                0x9cdac73ac1a5fe85,
                0x7c81df6efbf55f11,
            ]
        );
    }

    /// NL4 sample base words from VECTORS.md (`pk1` / `beacon0`).
    #[test]
    fn nl4_base_words_match_vectors() {
        let lanes = lanes_of(&seed(&[0x11; 32], &[0x22; 32]));
        let samples = [
            (0u64, 0u64, 0xc7582082a632c2c8u64),
            (0, 1, 0x6395f245d5934a68),
            (0, 2, 0xdd3aaa1f4cc23a0b),
            (0, 3, 0x23da68c5377ed50a),
            (1, 0, 0xa9d24682c24f67c9),
            (1, 1, 0x6f4529b850deed8f),
            (7, 7, 0x38ac197bd39defe0),
            (255, 255, 0x9e61ddf478cdd5c7),
        ];
        for (i, j, word) in samples {
            assert_eq!(base_word_nl4(&lanes, i, j), word, "i={i} j={j}");
        }
    }

    /// Slot 0 is 2048 NL4 base words. SHA-256 matches VECTORS.md.
    #[test]
    fn nl4_slot0_sha256_matches_vectors() {
        let digest = seed(&[0x11; 32], &[0x22; 32]);
        let sizes = Sizes::conformance_small_checked(1024, 16384, 16384, 1).unwrap();
        let terrain = fill_terrain_sized(&digest, sizes);
        assert_eq!(terrain.len(), 16384);
        assert_eq!(
            crate::sha::sha256(&terrain),
            [
                0x30, 0x7a, 0xd6, 0x85, 0xfb, 0x99, 0x9e, 0x9a, 0x51, 0x35, 0xad, 0x89, 0xa4, 0x3e,
                0xa1, 0xf0, 0xb8, 0x09, 0x99, 0x6f, 0x47, 0xb8, 0x41, 0xf0, 0x4d, 0x12, 0x5c, 0xe0,
                0x53, 0xe0, 0xe2, 0x41,
            ]
        );
    }

    /// One base word moves when any lane bit flips. j = 0 depends on all four lanes.
    #[test]
    fn nl4_base_word_depends_on_all_four_lanes() {
        let lanes = lanes_of(&seed(&[0x11; 32], &[0x22; 32]));
        let word = base_word_nl4(&lanes, 0, 0);
        for lane in 0..4 {
            let mut flipped = lanes;
            flipped[lane] ^= 0x8000_0000_0000_0001;
            assert_ne!(base_word_nl4(&flipped, 0, 0), word, "lane {lane}");
        }
    }

    #[test]
    fn parent_indices_match_section_10() {
        assert_eq!(bitrev_parent(1), 0);
        assert_eq!(bitrev_parent(5), 2);
        assert_eq!(mul_parent(5), 4);
        assert_eq!(bitrev_parent(16383), 8191);
        assert_eq!(mul_parent(16383), 9976);
    }

    /// `stages[0].waypoint` is the seed. Intentional.
    #[test]
    fn waypoint_equals_seed() {
        let hike = crate::object::structural_object();
        assert_eq!(
            hike.stages[0].waypoint,
            seed(&hike.public_key, &hike.seed_beacon)
        );
    }
}
