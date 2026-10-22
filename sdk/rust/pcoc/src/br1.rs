//! SPEC §13.6 Rule BR1, the Bari paid-waypoint check.
//!
//! Block-hash comparisons are natural order on both sides (SPEC §9, P9.17).
//! This module does not reverse a hash. A caller that learned a tip hash as
//! Bitcoin display hex reverses that hash once before calling in.

use crate::constants::HASHCASH_DIFFICULTY;
use crate::hashcash::{hashcash_challenge, hashcash_digest, header_hash, leading_zero_bits};

/// SPEC §9. Every synth header carries this compact `nBits`.
pub const SYNTH_NBITS: u32 = 0x207f_ffff;

/// Bitcoin mainnet proof-of-work limit, compact form (`powLimit`).
pub const MAINNET_POW_LIMIT_BITS: u32 = 0x1d00_ffff;

const RETARGET_INTERVAL: u32 = 2016;
const TARGET_TIMESPAN: i64 = 14 * 24 * 60 * 60;

/// SPEC §9. Which plane's `nBits` rule Rule BR1 applies to the "after" header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Br1Plane {
    /// Fixed synth `nBits` `0x207fffff` and the little-endian proof-of-work check.
    Test,
    /// Bitcoin mainnet consensus target for `tip.height + 1`, including a retarget.
    Live,
}

/// Bari's tip, in the form Rule BR1 compares.
///
/// `hash` and `previous` are natural order. `hash` is `SHA-256(SHA-256(tip header))`.
/// `previous` is the 32 bytes at offset 4 of the tip header. `bits` and `time`
/// are the tip header's `nBits` and `nTime`. `period_first_time` is the `nTime`
/// of the block at height `height - 2015`, and is read only when
/// `(height + 1)` is a multiple of 2016.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Br1Tip {
    pub hash: [u8; 32],
    pub previous: [u8; 32],
    pub height: u32,
    pub time: u32,
    pub bits: u32,
    pub period_first_time: u32,
}

/// SPEC §13.6. Where the fee header sits against the tip. `Outside` is not paid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Br1Window {
    Current,
    Before,
    After,
    Outside,
}

/// SPEC §13.6 Rule BR1 step 2, before the Hashcash check.
///
/// `header` is the 80-byte fee header. A short or long slice is `Outside`.
pub fn br1_window(header: &[u8], tip: &Br1Tip, plane: Br1Plane) -> Br1Window {
    let Some(header) = header_80(header) else {
        return Br1Window::Outside;
    };
    let hash = header_hash(&header);
    if hash == tip.hash {
        return Br1Window::Current;
    }
    if hash == tip.previous {
        return Br1Window::Before;
    }
    let mut previous = [0u8; 32];
    previous.copy_from_slice(&header[4..36]);
    if previous == tip.hash && bits_of(&header) == required_bits(tip, plane) && pow_meets(&header)
    {
        return Br1Window::After;
    }
    Br1Window::Outside
}

/// SPEC §9 and §13.6. The compact target an "after" header must carry.
pub fn required_bits(tip: &Br1Tip, plane: Br1Plane) -> u32 {
    match plane {
        Br1Plane::Test => SYNTH_NBITS,
        Br1Plane::Live => mainnet_next_bits(tip.height, tip.bits, tip.time, tip.period_first_time),
    }
}

/// SPEC §13.6 Rule BR1. `204` when the post is a paid waypoint, `402` otherwise.
///
/// `nonce_decimal` is the `X-Bari-Hashcash-Nonce` spelling. `fee_header_hex` is
/// the `X-Bari-Hashcash-Header` spelling: 160 lowercase hex digits. `bari_origin`
/// is Bari's own public origin, already serialized as SPEC §13.1 states.
pub fn br1_status(
    body: &[u8],
    nonce_decimal: &str,
    fee_header_hex: &str,
    bari_origin: &str,
    tip: &Br1Tip,
    plane: Br1Plane,
) -> u16 {
    let Some(header) = parse_fee_header(fee_header_hex) else {
        return 402;
    };
    let Some(nonce) = parse_nonce(nonce_decimal) else {
        return 402;
    };
    if body.len() != 32 {
        return 402;
    }
    if br1_window(&header, tip, plane) == Br1Window::Outside {
        return 402;
    }
    let mut waypoint = [0u8; 32];
    waypoint.copy_from_slice(body);
    let hash = header_hash(&header);
    let challenge = hashcash_challenge(bari_origin, &hash);
    let digest = hashcash_digest(&challenge, &waypoint, nonce);
    if leading_zero_bits(&digest) < HASHCASH_DIFFICULTY {
        return 402;
    }
    204
}

/// SPEC §13.1. Shortest decimal, no sign, no leading zeros. Zero is `0`.
pub fn parse_nonce(raw: &str) -> Option<u64> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if raw.len() > 1 && raw.as_bytes()[0] == b'0' {
        return None;
    }
    raw.parse().ok()
}

/// SPEC §13.1. 160 lowercase hex digits, the 80 fee-header bytes, byte 0 first.
pub fn parse_fee_header(raw: &str) -> Option<[u8; 80]> {
    if raw.len() != 160 {
        return None;
    }
    if !raw
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let bytes = crate::hexutil::hex_decode(raw)?;
    let mut out = [0u8; 80];
    out.copy_from_slice(&bytes);
    Some(out)
}

/// 80-byte header. Version is 1. `previous` is natural order at offset 4.
/// `nTime` is at 68, `nBits` at 72, nonce at 76. Multi-byte fields are little-endian.
pub fn assemble_header(previous: &[u8; 32], n_time: u32, bits: u32, nonce: u32) -> [u8; 80] {
    let mut header = [0u8; 80];
    header[0..4].copy_from_slice(&1u32.to_le_bytes());
    header[4..36].copy_from_slice(previous);
    header[68..72].copy_from_slice(&n_time.to_le_bytes());
    header[72..76].copy_from_slice(&bits.to_le_bytes());
    header[76..80].copy_from_slice(&nonce.to_le_bytes());
    header
}

/// SPEC §9. Bitcoin mainnet `GetNextWorkRequired` for the block after `height`.
///
/// Between retargets the compact target is the tip's `nBits`. On a 2016-block
/// boundary the actual timespan is `tip_time - period_first_time`, clamped to
/// a factor of four around two weeks, and the new target is clamped to the
/// mainnet proof-of-work limit.
pub fn mainnet_next_bits(
    height: u32,
    tip_bits: u32,
    tip_time: u32,
    period_first_time: u32,
) -> u32 {
    if (height + 1) % RETARGET_INTERVAL != 0 {
        return tip_bits;
    }
    let mut span = i64::from(tip_time) - i64::from(period_first_time);
    let min_span = TARGET_TIMESPAN / 4;
    let max_span = TARGET_TIMESPAN * 4;
    if span < min_span {
        span = min_span;
    }
    if span > max_span {
        span = max_span;
    }
    let mut target = set_compact(tip_bits).0;
    target = target.mul_u64(span as u64).div_u64(TARGET_TIMESPAN as u64);
    let limit = set_compact(MAINNET_POW_LIMIT_BITS).0;
    if target.cmp_u256(&limit).is_gt() {
        target = limit;
    }
    target.get_compact()
}

fn header_80(header: &[u8]) -> Option<[u8; 80]> {
    if header.len() != 80 {
        return None;
    }
    let mut out = [0u8; 80];
    out.copy_from_slice(header);
    Some(out)
}

fn bits_of(header: &[u8; 80]) -> u32 {
    u32::from_le_bytes(header[72..76].try_into().expect("4 bytes"))
}

/// SPEC §9. `SHA-256d` as a little-endian integer is at most the target `nBits` encodes.
/// A negative compact or an overflow compact does not encode a usable target.
fn pow_meets(header: &[u8; 80]) -> bool {
    let bits = bits_of(header);
    let (target, negative, overflow) = set_compact(bits);
    if negative || overflow || target.is_zero() {
        return false;
    }
    let hash = header_hash(header);
    let got = U256::from_le_bytes(hash);
    got.cmp_u256(&target).is_le()
}

/// Bitcoin `arith_uint256::SetCompact`. The boolean pair is `(negative, overflow)`.
fn set_compact(bits: u32) -> (U256, bool, bool) {
    let size = (bits >> 24) as i32;
    let word = bits & 0x007f_ffff;
    let value = if word == 0 {
        U256::ZERO
    } else if size <= 3 {
        U256::from_u64(u64::from(word) >> (8 * (3 - size)))
    } else {
        U256::from_u64(u64::from(word)).shl_bytes((size - 3) as u32)
    };
    let negative = word != 0 && (bits & 0x0080_0000) != 0;
    let overflow = word != 0
        && (size > 34 || (word > 0xff && size > 33) || (word > 0xffff && size > 32));
    (value, negative, overflow)
}

/// 256-bit little-endian integer. Limb 0 is the least significant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct U256([u64; 4]);

impl U256 {
    const ZERO: Self = Self([0; 4]);

    fn from_u64(v: u64) -> Self {
        Self([v, 0, 0, 0])
    }

    fn from_le_bytes(bytes: [u8; 32]) -> Self {
        let mut limbs = [0u64; 4];
        for (i, limb) in limbs.iter_mut().enumerate() {
            let mut raw = [0u8; 8];
            raw.copy_from_slice(&bytes[i * 8..i * 8 + 8]);
            *limb = u64::from_le_bytes(raw);
        }
        Self(limbs)
    }

    fn is_zero(self) -> bool {
        self.0 == [0; 4]
    }

    fn cmp_u256(self, other: &Self) -> core::cmp::Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }

    fn bit_length(self) -> u32 {
        for i in (0..4).rev() {
            if self.0[i] != 0 {
                return (i as u32) * 64 + (64 - self.0[i].leading_zeros());
            }
        }
        0
    }

    /// Shift left by `nbytes` whole bytes. Bits that leave the 256-bit width are dropped.
    fn shl_bytes(self, nbytes: u32) -> Self {
        if nbytes >= 32 {
            return Self::ZERO;
        }
        let mut bytes = [0u8; 32];
        for (i, limb) in self.0.iter().enumerate() {
            bytes[i * 8..i * 8 + 8].copy_from_slice(&limb.to_le_bytes());
        }
        let mut out = [0u8; 32];
        let n = nbytes as usize;
        out[n..].copy_from_slice(&bytes[..32 - n]);
        Self::from_le_bytes(out)
    }

    fn shr_bytes(self, nbytes: u32) -> Self {
        if nbytes >= 32 {
            return Self::ZERO;
        }
        let mut bytes = [0u8; 32];
        for (i, limb) in self.0.iter().enumerate() {
            bytes[i * 8..i * 8 + 8].copy_from_slice(&limb.to_le_bytes());
        }
        let mut out = [0u8; 32];
        let n = nbytes as usize;
        out[..32 - n].copy_from_slice(&bytes[n..]);
        Self::from_le_bytes(out)
    }

    /// Truncating 256-bit product with a `u64`, matching `arith_uint256`.
    fn mul_u64(self, factor: u64) -> Self {
        let mut out = [0u64; 4];
        let mut carry = 0u128;
        for i in 0..4 {
            let prod = u128::from(self.0[i]) * u128::from(factor) + carry;
            out[i] = prod as u64;
            carry = prod >> 64;
        }
        Self(out)
    }

    fn div_u64(self, divisor: u64) -> Self {
        debug_assert!(divisor != 0);
        let mut rem = 0u128;
        let mut out = [0u64; 4];
        for i in (0..4).rev() {
            let cur = (rem << 64) | u128::from(self.0[i]);
            out[i] = (cur / u128::from(divisor)) as u64;
            rem = cur % u128::from(divisor);
        }
        Self(out)
    }

    /// Bitcoin `arith_uint256::GetCompact` for a non-negative value.
    fn get_compact(self) -> u32 {
        if self.is_zero() {
            return 0;
        }
        let mut size = (self.bit_length() + 7) / 8;
        let mut compact = if size <= 3 {
            self.0[0] << (8 * (3 - size))
        } else {
            self.shr_bytes(size - 3).0[0]
        };
        if compact & 0x0080_0000 != 0 {
            compact >>= 8;
            size += 1;
        }
        (compact as u32 & 0x007f_ffff) | (size << 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashcash::hashcash_solve;

    fn tip_from(header: &[u8; 80], height: u32, period_first_time: u32) -> Br1Tip {
        Br1Tip {
            hash: header_hash(header),
            previous: header[4..36].try_into().unwrap(),
            height,
            time: u32::from_le_bytes(header[68..72].try_into().unwrap()),
            bits: bits_of(header),
            period_first_time,
        }
    }

    #[test]
    fn genesis_compact_round_trips_and_the_first_retarget_clamps() {
        let (target, neg, over) = set_compact(MAINNET_POW_LIMIT_BITS);
        assert!(!neg && !over);
        assert_eq!(target.get_compact(), MAINNET_POW_LIMIT_BITS);
        let (synth, _, _) = set_compact(SYNTH_NBITS);
        assert_eq!(synth.get_compact(), SYNTH_NBITS);
        // Block 2015 time 1233061996, block 0 time 1231006505, both nBits 0x1d00ffff.
        // The unclamped target would be easier than the pow limit, so the next
        // compact stays 0x1d00ffff. Block 2016's nBits is that value.
        assert_eq!(
            mainnet_next_bits(2015, 0x1d00ffff, 1_233_061_996, 1_231_006_505),
            0x1d00ffff
        );
        // A timespan of one quarter makes the target four times harder: 0x1c3fffc0.
        assert_eq!(
            mainnet_next_bits(2015, 0x1d00ffff, 1_231_006_505 + 302_400, 1_231_006_505),
            0x1c3fffc0
        );
        assert_eq!(mainnet_next_bits(100, 0x207fffff, 1, 0), 0x207fffff);
    }

    #[test]
    fn test_plane_after_meets_the_fixed_target_and_live_copy_uses_the_tip() {
        let tip_header = assemble_header(&[0x11; 32], 1_700_000_000, SYNTH_NBITS, 4);
        let tip = tip_from(&tip_header, 100, 0);
        assert_eq!(
            br1_window(&tip_header, &tip, Br1Plane::Test),
            Br1Window::Current
        );
        let mut after = None;
        for nonce in 0..64u32 {
            let header = assemble_header(&tip.hash, 1_700_000_010, SYNTH_NBITS, nonce);
            if br1_window(&header, &tip, Br1Plane::Test) == Br1Window::After {
                after = Some(header);
                break;
            }
        }
        let after = after.expect("synth target");
        assert_eq!(
            br1_window(&after, &tip, Br1Plane::Live),
            Br1Window::After,
            "a non-retarget live tip with synth nBits keeps that compact"
        );
        let wrong = assemble_header(&tip.hash, 1_700_000_010, 0x1d00ffff, 1);
        assert_eq!(
            br1_window(&wrong, &tip, Br1Plane::Test),
            Br1Window::Outside
        );
    }

    #[test]
    fn display_order_reversal_is_not_current_before_or_after() {
        let tip_header = assemble_header(&[0x10; 32], 1_700_000_000, SYNTH_NBITS, 4);
        let tip = tip_from(&tip_header, 50, 0);
        let mut reversed = tip;
        reversed.hash.reverse();
        assert_eq!(
            br1_window(&tip_header, &reversed, Br1Plane::Test),
            Br1Window::Outside
        );
        let before = assemble_header(&[0x12; 32], 1_700_000_000, SYNTH_NBITS, 5);
        let mut tip_before = tip;
        tip_before.previous = header_hash(&before);
        assert_eq!(
            br1_window(&before, &tip_before, Br1Plane::Test),
            Br1Window::Before
        );
        tip_before.previous.reverse();
        assert_eq!(
            br1_window(&before, &tip_before, Br1Plane::Test),
            Br1Window::Outside
        );
        let after = (0..64u32)
            .map(|nonce| assemble_header(&tip.hash, 1_700_000_010, SYNTH_NBITS, nonce))
            .find(|header| br1_window(header, &tip, Br1Plane::Test) == Br1Window::After)
            .unwrap();
        let mut flipped = after;
        flipped[4..36].reverse();
        assert_eq!(
            br1_window(&flipped, &tip, Br1Plane::Test),
            Br1Window::Outside
        );
    }

    #[test]
    fn paid_post_is_204_and_a_bad_spelling_or_origin_is_402() {
        let tip_header = assemble_header(&[0x11; 32], 1_700_000_000, SYNTH_NBITS, 4);
        let tip = tip_from(&tip_header, 100, 0);
        let origin = "https://example.com";
        let waypoint = [0x22u8; 32];
        let hash = header_hash(&tip_header);
        let challenge = hashcash_challenge(origin, &hash);
        let (nonce, _) = hashcash_solve(&challenge, &waypoint, HASHCASH_DIFFICULTY).unwrap();
        let hex_header = crate::hexutil::hex_encode(&tip_header);
        assert_eq!(
            br1_status(&waypoint, &nonce.to_string(), &hex_header, origin, &tip, Br1Plane::Test),
            204
        );
        assert_eq!(
            br1_status(
                &waypoint,
                &nonce.to_string(),
                &hex_header,
                "https://other.example",
                &tip,
                Br1Plane::Test
            ),
            402
        );
        assert_eq!(
            br1_status(&waypoint, "01", &hex_header, origin, &tip, Br1Plane::Test),
            402
        );
        assert_eq!(
            br1_status(
                &waypoint,
                &nonce.to_string(),
                &hex_header.to_ascii_uppercase(),
                origin,
                &tip,
                Br1Plane::Test
            ),
            402
        );
        assert_eq!(
            br1_status(&[0; 31], &nonce.to_string(), &hex_header, origin, &tip, Br1Plane::Test),
            402
        );
    }
}
