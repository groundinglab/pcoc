//! SPEC §13.1 Hashcash challenge, digest, and solve. No POST.

use crate::constants::{HASHCASH_DIFFICULTY, HASHCASH_PREFIX};
use crate::hash::sha256_parts;

/// SPEC §13.1. `challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)`.
pub fn hashcash_challenge(bari_origin: &str, header_hash: &[u8; 32]) -> [u8; 32] {
    sha256_parts(&[HASHCASH_PREFIX, bari_origin.as_bytes(), header_hash])
}

/// SPEC §13.1. `digest = SHA-256(challenge ‖ waypoint ‖ le64(nonce))`.
pub fn hashcash_digest(challenge: &[u8; 32], waypoint: &[u8; 32], nonce: u64) -> [u8; 32] {
    let nb = nonce.to_le_bytes();
    sha256_parts(&[challenge, waypoint, &nb])
}

/// SPEC §13.1. Leading zero bits from the first digest byte. Inside a byte the high bit is first.
pub fn leading_zero_bits(digest: &[u8]) -> u32 {
    let mut n = 0u32;
    for &b in digest {
        if b == 0 {
            n += 8;
        } else {
            n += b.leading_zeros();
            break;
        }
    }
    n
}

/// SPEC §13.1. The first nonce from 0 whose digest has `difficulty` leading zero bits.
/// Returns `(nonce, digest)`.
pub fn hashcash_solve(
    challenge: &[u8; 32],
    waypoint: &[u8; 32],
    difficulty: u32,
) -> Option<(u64, [u8; 32])> {
    let mut nonce = 0u64;
    loop {
        let dig = hashcash_digest(challenge, waypoint, nonce);
        if leading_zero_bits(&dig) >= difficulty {
            return Some((nonce, dig));
        }
        nonce = nonce.checked_add(1)?;
    }
}

/// SPEC §13.1. Solve at the one difficulty in this file, 21.
pub fn hashcash_solve_spec(
    challenge: &[u8; 32],
    waypoint: &[u8; 32],
) -> Option<(u64, [u8; 32])> {
    hashcash_solve(challenge, waypoint, HASHCASH_DIFFICULTY)
}

/// SPEC §13.1. Nonce header spelling: shortest decimal, zero is `0`.
pub fn nonce_decimal(nonce: u64) -> String {
    nonce.to_string()
}
