//! SPEC §13.1 Hashcash and §13.6 Rule BR1 window.
//!
//! The puzzle is the Rust SDK (`pcoc::header_hash`, `pcoc::hashcash_challenge`,
//! `pcoc::hashcash_digest`). This module does not keep a second copy.
//!
//! Wire: `X-Bari-Hashcash-Nonce` (shortest decimal) and
//! `X-Bari-Hashcash-Header` (160 lowercase hex digits, the 80 fee-header bytes).
//! Challenge: `SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)`.
//! `header_hash` is SHA-256d of the fee header. Difficulty is 21 leading zero bits.

use crate::Hash32;
use url::Url;

/// Algorithm tag mixed into the challenge. SPEC §13.1, ten ASCII bytes.
pub const HASHCASH_TAG: &[u8] = pcoc::constants::HASHCASH_PREFIX;

/// Difficulty: 21 leading zero bits. SPEC P13.5.
pub const HASHCASH_BITS: u32 = pcoc::constants::HASHCASH_DIFFICULTY;

/// HTTP header name (canonical).
pub const HASHCASH_NONCE_HEADER: &str = "x-bari-hashcash-nonce";

/// HTTP header name for the 80-byte fee header (canonical).
pub const HASHCASH_HEADER_HEADER: &str = "x-bari-hashcash-header";

/// Fixed synth `nBits`. SPEC §9.
pub const SYNTH_NBITS: u32 = 0x207f_ffff;

/// Where a fee header sits against Bari's tip. SPEC §13.6 Rule BR1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeeWindow {
    Current,
    Before,
    After,
    Outside,
}

/// RFC 6454 §6.2 serialization of `raw` (SPEC §13.1).
///
/// Lowercase scheme, `://`, lowercase host (A-label), and `:port` only when
/// the port is not the scheme default. No userinfo, path, query, or fragment.
pub fn bari_origin(raw: &str) -> Result<String, String> {
    let url = Url::parse(raw).map_err(|error| format!("BARI_PUBLIC_ORIGIN: {error}"))?;
    let origin = url.origin();
    let url::Origin::Tuple(scheme, host, port) = origin else {
        return Err("BARI_PUBLIC_ORIGIN must be an http or https URL".to_owned());
    };
    if scheme != "http" && scheme != "https" {
        return Err("BARI_PUBLIC_ORIGIN scheme must be http or https".to_owned());
    }
    let host = host.to_string();
    let default = (scheme == "https" && port == 443) || (scheme == "http" && port == 80);
    if default {
        Ok(format!("{scheme}://{host}"))
    } else {
        Ok(format!("{scheme}://{host}:{port}"))
    }
}

/// 80-byte header. Version is 1. `previous` is natural order at offset 4.
/// `nTime` is at 68, `nBits` at 72, nonce at 76. All multi-byte fields are
/// little-endian, the Bitcoin header layout.
pub fn assemble_header(previous: &[u8; 32], n_time: u32, bits: u32, nonce: u32) -> [u8; 80] {
    let mut header = [0u8; 80];
    header[0..4].copy_from_slice(&1u32.to_le_bytes());
    header[4..36].copy_from_slice(previous);
    header[68..72].copy_from_slice(&n_time.to_le_bytes());
    header[72..76].copy_from_slice(&bits.to_le_bytes());
    header[76..80].copy_from_slice(&nonce.to_le_bytes());
    header
}

/// SHA-256d of an 80-byte header. One SHA-256 is not this value.
pub fn header_hash(fee_header: &[u8]) -> Hash32 {
    pcoc::header_hash(fee_header)
}

/// `SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)`.
pub fn challenge(bari_origin: &str, header_hash: &Hash32) -> Hash32 {
    pcoc::hashcash_challenge(bari_origin, header_hash)
}

/// `SHA-256(challenge ‖ waypoint ‖ le64(nonce))`.
pub fn hashcash_digest(challenge: &Hash32, waypoint: &Hash32, nonce: u64) -> Hash32 {
    pcoc::hashcash_digest(challenge, waypoint, nonce)
}

/// Leading zero bits. The high bit of the first byte is first.
pub fn leading_zero_bits(digest: &[u8]) -> u32 {
    pcoc::leading_zero_bits(digest)
}

pub fn verify(challenge: &Hash32, waypoint: &Hash32, nonce: u64, bits: u32) -> bool {
    leading_zero_bits(&hashcash_digest(challenge, waypoint, nonce)) >= bits
}

/// First nonce from 0 that meets `bits`. Test difficulties only; production
/// posts carry a nonce the caller already solved.
pub fn solve(challenge: &Hash32, waypoint: &Hash32, bits: u32) -> u64 {
    pcoc::hashcash_solve(challenge, waypoint, bits)
        .expect("hashcash search exhausted u64")
        .0
}

/// The one difficulty. Always [`HASHCASH_BITS`]. The environment is not read.
pub fn effective_bits() -> u32 {
    HASHCASH_BITS
}

/// Rule BR1 step 2. `current` and `before` use hashes Bari already holds.
/// `after` on the test plane (`network == "synthetic"`) uses the fixed synth
/// `nBits` and the little-endian proof-of-work check (SPEC §9). A live-plane
/// retarget is not in this build, so `after` on any other network is `Outside`.
pub fn fee_window(
    header: &[u8; 80],
    tip_hash: &Hash32,
    previous_hash: Option<&Hash32>,
    network: &str,
) -> FeeWindow {
    let hash = header_hash(header);
    if hash == *tip_hash {
        return FeeWindow::Current;
    }
    if previous_hash.is_some_and(|previous| hash == *previous) {
        return FeeWindow::Before;
    }
    let mut previous = [0u8; 32];
    previous.copy_from_slice(&header[4..36]);
    if previous == *tip_hash && network == "synthetic" && synth_after_pow(header) {
        return FeeWindow::After;
    }
    FeeWindow::Outside
}

/// Shortest decimal, no sign, no leading zeros. Zero is `0`. SPEC §13.1.
pub fn parse_nonce(raw: &str) -> Option<u64> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if raw.len() > 1 && raw.starts_with('0') {
        return None;
    }
    raw.parse().ok()
}

/// 160 lowercase hex digits, 80 bytes.
pub fn parse_fee_header(raw: &str) -> Option<[u8; 80]> {
    if raw.len() != 160
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return None;
    }
    let bytes = hex::decode(raw).ok()?;
    bytes.try_into().ok()
}

fn synth_after_pow(header: &[u8; 80]) -> bool {
    let bits = u32::from_le_bytes(header[72..76].try_into().expect("4 bytes"));
    bits == SYNTH_NBITS && pow_meets(header, bits)
}

/// `SHA-256d`, as a little-endian integer, is at most the target `bits` encodes.
fn pow_meets(header: &[u8; 80], bits: u32) -> bool {
    let hash = header_hash(header);
    let target = compact_target(bits);
    for index in (0..32).rev() {
        if hash[index] < target[index] {
            return true;
        }
        if hash[index] > target[index] {
            return false;
        }
    }
    true
}

/// Bitcoin compact target as a 32-byte little-endian integer.
/// A target that does not fit in 256 bits is all `0xff` (the check still runs).
fn compact_target(bits: u32) -> [u8; 32] {
    let exponent = (bits >> 24) as usize;
    let mantissa = bits & 0x007f_ffff;
    let mut target = [0u8; 32];
    if mantissa == 0 || (bits & 0x0080_0000) != 0 {
        return target;
    }
    if exponent <= 3 {
        let word = mantissa >> (8 * (3 - exponent));
        target[0] = (word & 0xff) as u8;
        if exponent > 1 {
            target[1] = ((word >> 8) & 0xff) as u8;
        }
        if exponent > 2 {
            target[2] = ((word >> 16) & 0xff) as u8;
        }
        return target;
    }
    let shift = exponent - 3;
    if shift >= 32 {
        return [0xff; 32];
    }
    let bytes = mantissa.to_le_bytes();
    for (offset, byte) in bytes.iter().take(3).enumerate() {
        let index = shift + offset;
        if index < 32 {
            target[index] = *byte;
        }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn origin_serialization_drops_default_port_and_lowercases() {
        assert_eq!(
            bari_origin("https://Example.COM/path?q=1").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            bari_origin("https://example.com:443").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            bari_origin("http://example.com:80/x").unwrap(),
            "http://example.com"
        );
        assert_eq!(
            bari_origin("https://example.com:8443").unwrap(),
            "https://example.com:8443"
        );
        assert_eq!(
            bari_origin("http://example.com:8080").unwrap(),
            "http://example.com:8080"
        );
    }

    #[test]
    fn leading_zero_bits_counts_prefix() {
        let mut digest = [0xffu8; 32];
        assert_eq!(leading_zero_bits(&digest), 0);
        digest[0] = 0x00;
        digest[1] = 0x0f;
        assert_eq!(leading_zero_bits(&digest), 8 + 4);
        assert_eq!(leading_zero_bits(&[0u8; 32]), 256);
    }

    #[test]
    fn solve_and_verify_roundtrip_low_bits() {
        let header = assemble_header(&[0x11; 32], 1_700_000_000, SYNTH_NBITS, 1);
        let hash = header_hash(&header);
        let origin = "https://bari.test";
        let bound = challenge(origin, &hash);
        let waypoint = [0x22; 32];
        let nonce = solve(&bound, &waypoint, 8);
        assert!(verify(&bound, &waypoint, nonce, 8));
        assert_eq!(HASHCASH_BITS, 21);
        assert_eq!(HASHCASH_TAG, b"bari-hc-v2");
        let other = challenge(origin, &[0x99; 32]);
        assert_ne!(
            hashcash_digest(&bound, &waypoint, nonce),
            hashcash_digest(&other, &waypoint, nonce)
        );
    }

    #[test]
    fn a_different_header_rotates_the_challenge() {
        let origin = "https://bari.test";
        let a = challenge(
            origin,
            &header_hash(&assemble_header(&[1; 32], 1, SYNTH_NBITS, 1)),
        );
        let b = challenge(
            origin,
            &header_hash(&assemble_header(&[2; 32], 1, SYNTH_NBITS, 1)),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn difficulty_stays_21_when_the_removed_dial_is_set() {
        let previous = std::env::var("BARI_HASHCASH_BITS").ok();
        std::env::set_var("BARI_HASHCASH_BITS", "1");
        assert_eq!(effective_bits(), HASHCASH_BITS);
        match previous {
            Some(value) => std::env::set_var("BARI_HASHCASH_BITS", value),
            None => std::env::remove_var("BARI_HASHCASH_BITS"),
        }
    }

    #[test]
    fn nonce_and_header_spellings_reject_padding() {
        assert_eq!(parse_nonce("0"), Some(0));
        assert_eq!(parse_nonce("2400506"), Some(2_400_506));
        assert_eq!(parse_nonce("01"), None);
        assert_eq!(parse_nonce(" 1"), None);
        assert_eq!(parse_nonce(""), None);
        let header = assemble_header(&[0; 32], 1, SYNTH_NBITS, 1);
        let hex = hex::encode(header);
        assert_eq!(parse_fee_header(&hex), Some(header));
        assert_eq!(parse_fee_header(&hex.to_ascii_uppercase()), None);
        assert_eq!(parse_fee_header(&hex[..159]), None);
    }

    #[test]
    fn window_is_current_before_after_or_outside() {
        let current = assemble_header(&[0x10; 32], 1_700_000_000, SYNTH_NBITS, 4);
        let current_hash = header_hash(&current);
        assert_eq!(
            fee_window(&current, &current_hash, None, "signet"),
            FeeWindow::Current
        );
        let before = assemble_header(&[0x10; 32], 1_700_000_000, SYNTH_NBITS, 5);
        let before_hash = header_hash(&before);
        assert_eq!(
            fee_window(&before, &current_hash, Some(&before_hash), "signet"),
            FeeWindow::Before
        );
        let mut after = None;
        for nonce in 0..64u32 {
            let header = assemble_header(&current_hash, 1_700_000_010, SYNTH_NBITS, nonce);
            if fee_window(&header, &current_hash, None, "synthetic") == FeeWindow::After {
                after = Some(header);
                break;
            }
        }
        let after = after.expect("synth nBits finds an after header");
        assert_eq!(
            fee_window(&after, &current_hash, None, "signet"),
            FeeWindow::Outside,
            "live-plane after is not checked in this build"
        );
        let outside = assemble_header(&[0x99; 32], 1, SYNTH_NBITS, 9);
        assert_eq!(
            fee_window(&outside, &current_hash, Some(&before_hash), "synthetic"),
            FeeWindow::Outside
        );
    }

    /// Every `hashcash.*` key in the SDK candidate file, through Bari's wrappers.
    #[test]
    fn candidate_hashcash_keys_match_bari() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../sdk/rust/candidates/hashcash.txt");
        let text = std::fs::read_to_string(&path).expect("candidate file");
        let mut fields = std::collections::HashMap::new();
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('\t').expect("tab");
            fields.insert(key.to_owned(), value.to_owned());
        }
        let origin = fields["hashcash.bari_origin"].as_str();
        assert_eq!(bari_origin(origin).unwrap(), origin);
        let header = parse_fee_header(&fields["hashcash.fee_header"]).unwrap();
        let hash = header_hash(&header);
        assert_eq!(hex::encode(hash), fields["hashcash.header_hash"]);
        let once = sha2::Sha256::digest(&header);
        assert_ne!(hex::encode(once), fields["hashcash.header_hash"]);
        let bound = challenge(origin, &hash);
        assert_eq!(hex::encode(bound), fields["hashcash.challenge"]);
        let once_hash: Hash32 = once.into();
        assert_ne!(
            hex::encode(challenge(origin, &once_hash)),
            fields["hashcash.challenge"]
        );
        let waypoint = parse32(&fields["hashcash.waypoint"]);
        let nonce: u64 = fields["hashcash.nonce"].parse().unwrap();
        let digest = hashcash_digest(&bound, &waypoint, nonce);
        assert_eq!(hex::encode(digest), fields["hashcash.digest"]);
        assert_eq!(
            leading_zero_bits(&digest).to_string(),
            fields["hashcash.leading_zero_bits"]
        );
        assert_eq!(fields["hashcash.difficulty_bits"], "21");
        assert_eq!(HASHCASH_BITS, 21);
        assert!(verify(&bound, &waypoint, nonce, HASHCASH_BITS));
        let other = challenge("https://other.example", &hash);
        assert!(!verify(&other, &waypoint, nonce, HASHCASH_BITS));
        assert!(!verify(
            &challenge(origin, &once_hash),
            &waypoint,
            nonce,
            HASHCASH_BITS
        ));
    }

    fn parse32(value: &str) -> Hash32 {
        let bytes = hex::decode(value).unwrap();
        bytes.try_into().unwrap()
    }
}
