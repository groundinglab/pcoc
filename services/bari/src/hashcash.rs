//! Tip-bound Hashcash for `POST /waypoint`.
//!
//! Wire: HTTP header `X-Bari-Hashcash-Nonce: <u64 decimal>`.
//! Message: `SHA256(challenge ‖ waypoint ‖ nonce_le_u64)`.
//! Challenge: `SHA256(b"bari-hc-v1" ‖ tip_hash)` from the live tip.
//! Difficulty: 21 leading zero bits. That is the one difficulty. Produce uses
//! the same count. `BARI_HASHCASH_BITS` is not a dial.

use crate::Hash32;
use sha2::{Digest, Sha256};

/// Algorithm tag mixed into the challenge (not on the wire).
pub const HASHCASH_TAG: &[u8] = b"bari-hc-v1";

/// Difficulty: 21 leading zero bits.
pub const HASHCASH_BITS: u32 = 21;

/// HTTP header name (canonical).
pub const HASHCASH_NONCE_HEADER: &str = "x-bari-hashcash-nonce";

/// Derive the 32-byte challenge from the live tip hash.
pub fn challenge_from_tip(tip_hash: &Hash32) -> Hash32 {
    let mut hasher = Sha256::new();
    hasher.update(HASHCASH_TAG);
    hasher.update(tip_hash);
    hasher.finalize().into()
}

/// `SHA256(challenge ‖ waypoint ‖ nonce_le_u64)`.
pub fn hashcash_digest(challenge: &Hash32, waypoint: &Hash32, nonce: u64) -> Hash32 {
    let mut hasher = Sha256::new();
    hasher.update(challenge);
    hasher.update(waypoint);
    hasher.update(nonce.to_le_bytes());
    hasher.finalize().into()
}

/// Count leading zero bits in a SHA-256 digest (big-endian bit order within bytes).
pub fn leading_zero_bits(digest: &Hash32) -> u32 {
    let mut bits = 0u32;
    for byte in digest {
        if *byte == 0 {
            bits += 8;
            continue;
        }
        bits += byte.leading_zeros();
        break;
    }
    bits
}

pub fn verify(challenge: &Hash32, waypoint: &Hash32, nonce: u64, bits: u32) -> bool {
    let digest = hashcash_digest(challenge, waypoint, nonce);
    leading_zero_bits(&digest) >= bits
}

/// Brute-force a nonce that satisfies `bits` leading zero bits.
pub fn solve(challenge: &Hash32, waypoint: &Hash32, bits: u32) -> u64 {
    let mut nonce = 0u64;
    loop {
        if verify(challenge, waypoint, nonce, bits) {
            return nonce;
        }
        nonce = nonce.wrapping_add(1);
        if nonce == 0 {
            panic!("hashcash search wrapped without finding a nonce");
        }
    }
}

/// The one difficulty. Always [`HASHCASH_BITS`]. The environment is not read.
pub fn effective_bits() -> u32 {
    HASHCASH_BITS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_zero_bits_counts_prefix() {
        let mut d = [0xffu8; 32];
        assert_eq!(leading_zero_bits(&d), 0);
        d[0] = 0x00;
        d[1] = 0x0f;
        assert_eq!(leading_zero_bits(&d), 8 + 4);
        d = [0u8; 32];
        assert_eq!(leading_zero_bits(&d), 256);
    }

    #[test]
    fn solve_and_verify_roundtrip_low_bits() {
        let tip = [0x11; 32];
        let wp = [0x22; 32];
        let challenge = challenge_from_tip(&tip);
        let nonce = solve(&challenge, &wp, 8);
        assert!(verify(&challenge, &wp, nonce, 8));
        assert_eq!(HASHCASH_BITS, 21);
        let other_waypoint = [0x33; 32];
        assert_ne!(
            hashcash_digest(&challenge, &wp, nonce),
            hashcash_digest(&challenge, &other_waypoint, nonce)
        );
        let other_tip = challenge_from_tip(&[0x99; 32]);
        assert_ne!(
            hashcash_digest(&challenge, &wp, nonce),
            hashcash_digest(&other_tip, &wp, nonce)
        );
    }

    #[test]
    fn tip_bound_challenge_rotates() {
        let a = challenge_from_tip(&[1; 32]);
        let b = challenge_from_tip(&[2; 32]);
        assert_ne!(a, b);
    }

    #[test]
    fn difficulty_stays_21_when_the_removed_dial_is_set() {
        let previous = std::env::var("BARI_HASHCASH_BITS").ok();
        std::env::set_var("BARI_HASHCASH_BITS", "1");
        assert_eq!(effective_bits(), HASHCASH_BITS);
        assert_eq!(HASHCASH_BITS, 21);
        match previous {
            Some(value) => std::env::set_var("BARI_HASHCASH_BITS", value),
            None => std::env::remove_var("BARI_HASHCASH_BITS"),
        }
        let tip = [0x41; 32];
        let waypoint = [0x42; 32];
        let challenge = challenge_from_tip(&tip);
        let weak = solve(&challenge, &waypoint, 1);
        if !verify(&challenge, &waypoint, weak, HASHCASH_BITS) {
            assert!(!verify(&challenge, &waypoint, weak, effective_bits()));
        }
    }

    /// The one 21-bit check. The stamp is committed. This test does not search.
    #[test]
    fn committed_21_bit_stamp_verifies_without_mining() {
        let fixture = include_str!("../tests/vectors/hashcash-21.txt");
        let mut tip = None;
        let mut waypoint = None;
        let mut nonce = None;
        let mut digest = None;
        for line in fixture.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once(' ').expect("fixture field");
            match key {
                "tip" => tip = Some(parse_hex32(value)),
                "waypoint" => waypoint = Some(parse_hex32(value)),
                "nonce" => nonce = Some(value.parse::<u64>().expect("nonce")),
                "digest" => digest = Some(parse_hex32(value)),
                other => panic!("unknown fixture field {other}"),
            }
        }
        let tip = tip.expect("tip");
        let waypoint = waypoint.expect("waypoint");
        let nonce = nonce.expect("nonce");
        let digest = digest.expect("digest");
        assert_eq!(HASHCASH_BITS, 21);
        assert_eq!(effective_bits(), HASHCASH_BITS);
        let challenge = challenge_from_tip(&tip);
        let got = hashcash_digest(&challenge, &waypoint, nonce);
        assert_eq!(got, digest);
        assert!(verify(&challenge, &waypoint, nonce, HASHCASH_BITS));
        assert!(leading_zero_bits(&got) >= HASHCASH_BITS);
        assert!(nonce > 0);
        assert!(!verify(&challenge, &waypoint, nonce - 1, HASHCASH_BITS));
    }

    fn parse_hex32(value: &str) -> Hash32 {
        let bytes = hex::decode(value).expect("hex");
        bytes.try_into().expect("32 bytes")
    }
}
