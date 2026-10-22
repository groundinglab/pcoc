//! SPEC §13.6 Rule BR1 and the §9 header-hash rules that rule uses.
//! The producer posts a waypoint. This module is the paid-rule check a Bari
//! operator runs on that POST. It does not walk a leg.

use crate::constants::HASHCASH_DIFFICULTY;
use crate::hash::sha256;
use crate::hashcash::{hashcash_challenge, hashcash_digest, leading_zero_bits, nonce_decimal};
use crate::hexutil::{hex_decode_exact, reverse32};

/// SPEC §13. The default plane is the test plane. Live is explicit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    Test,
    Live,
}

impl Plane {
    pub fn label(self) -> &'static str {
        match self {
            Plane::Test => "test",
            Plane::Live => "live",
        }
    }
}

/// SPEC §9. Synth headers carry this fixed `nBits`.
pub const SYNTH_NBITS: u32 = 0x207f_ffff;
/// Bitcoin mainnet proof-of-work limit compact (`0x1d00ffff`).
pub const MAINNET_POW_LIMIT_NBITS: u32 = 0x1d00_ffff;
/// SPEC §9 / Bitcoin mainnet. Retarget every 2016 blocks.
pub const RETARGET_INTERVAL: u64 = 2016;
/// Bitcoin mainnet target timespan: 2016 blocks × 10 minutes.
pub const TARGET_TIMESPAN: i64 = 1_209_600;

/// SPEC §13.6. One waypoint POST as Bari sees it.
#[derive(Clone, Debug)]
pub struct WaypointSubmission<'a> {
    pub body: &'a [u8; 32],
    /// `X-Bari-Hashcash-Nonce`, the decimal spelling on the wire.
    pub nonce_header: &'a str,
    /// `X-Bari-Hashcash-Header`, 160 hexadecimal digits.
    pub fee_header_hex: &'a str,
}

/// SPEC §13.6. Bari's view of its tip when the POST arrives.
#[derive(Clone, Debug)]
pub struct BariTip {
    pub header: [u8; 80],
    pub height: u64,
    /// Natural-order tip hash used for current / before / after.
    /// A correct caller passes [`header_hash`] of `header` after reversing a
    /// display-hex tip once (SPEC §9). Passing the display bytes here is the
    /// mismatch that answers 402.
    pub hash_natural: [u8; 32],
    pub plane: Plane,
    /// `nTime` of the block at `height - 2015`. Required on the live plane when
    /// `height + 1` is a multiple of 2016. Ignored otherwise.
    pub period_first_time: Option<u32>,
}

/// SPEC §13.6 Rule BR1. `204` when the POST is paid, otherwise `402`.
pub fn br1_status(bari_origin: &str, post: &WaypointSubmission<'_>, tip: &BariTip) -> u16 {
    if fee_header_class(post, tip).is_none() {
        return 402;
    }
    let Some(parsed) = parse_fee(post) else {
        return 402;
    };
    let header_hash = header_hash(&parsed.header);
    let challenge = hashcash_challenge(bari_origin, &header_hash);
    let digest = hashcash_digest(&challenge, post.body, parsed.nonce);
    if leading_zero_bits(&digest) < HASHCASH_DIFFICULTY {
        return 402;
    }
    204
}

/// Which of the three Rule BR1 blocks this fee header is, if it is one of them.
pub fn fee_header_class(post: &WaypointSubmission<'_>, tip: &BariTip) -> Option<&'static str> {
    let parsed = parse_fee(post)?;
    let fee_hash = header_hash(&parsed.header);
    if fee_hash == tip.hash_natural {
        return Some("current");
    }
    if fee_hash == header_prev(&tip.header) {
        return Some("before");
    }
    if header_prev(&parsed.header) == tip.hash_natural && after_pow(&parsed.header, tip) {
        return Some("after");
    }
    None
}

struct ParsedFee {
    header: [u8; 80],
    nonce: u64,
}

fn parse_fee(post: &WaypointSubmission<'_>) -> Option<ParsedFee> {
    if post.fee_header_hex.len() != 160 || !post.fee_header_hex.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let header = hex_decode_exact::<80>(post.fee_header_hex)?;
    let nonce = parse_nonce(post.nonce_header)?;
    Some(ParsedFee { header, nonce })
}

/// Shortest decimal spelling of a `u64`. Rejects a sign, a space, and leading zeros.
pub fn parse_nonce(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if text.len() > 1 && text.as_bytes()[0] == b'0' {
        return None;
    }
    let n: u64 = text.parse().ok()?;
    if nonce_decimal(n) != text {
        return None;
    }
    Some(n)
}

fn after_pow(fee: &[u8; 80], tip: &BariTip) -> bool {
    let required = match tip.plane {
        Plane::Test => SYNTH_NBITS,
        Plane::Live => match mainnet_next_nbits(tip.height, header_nbits(&tip.header), header_time(&tip.header), tip.period_first_time) {
            Some(n) => n,
            None => return false,
        },
    };
    if header_nbits(fee) != required {
        return false;
    }
    let hash = header_hash(fee);
    hash_meets_nbits(&hash, required)
}

/// SPEC §13.6 live plane. Mainnet `GetNextWorkRequired`, including the retarget
/// clamp to the mainnet proof-of-work limit. A height that is not a boundary
/// returns the tip's own `nBits` (no clamp on that copy).
pub fn mainnet_next_nbits(tip_height: u64, tip_nbits: u32, tip_time: u32, period_first_time: Option<u32>) -> Option<u32> {
    if (tip_height + 1) % RETARGET_INTERVAL != 0 {
        return Some(tip_nbits);
    }
    let first = period_first_time?;
    let mut span = tip_time as i64 - first as i64;
    let min_span = TARGET_TIMESPAN / 4;
    let max_span = TARGET_TIMESPAN * 4;
    if span < min_span {
        span = min_span;
    }
    if span > max_span {
        span = max_span;
    }
    let mut target = compact_to_target(tip_nbits)?;
    target = mul_div_target(&target, span as u64, TARGET_TIMESPAN as u64);
    let limit = compact_to_target(MAINNET_POW_LIMIT_NBITS)?;
    if target_cmp(&target, &limit) == std::cmp::Ordering::Greater {
        target = limit;
    }
    Some(target_to_compact(&target))
}

/// `SHA-256(SHA-256(header))`, natural order. SPEC §13.1 `header_hash`.
pub fn header_hash(header: &[u8; 80]) -> [u8; 32] {
    sha256(&sha256(header))
}

pub fn header_prev(header: &[u8; 80]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&header[4..36]);
    out
}

pub fn header_time(header: &[u8; 80]) -> u32 {
    u32::from_le_bytes([header[68], header[69], header[70], header[71]])
}

pub fn header_nbits(header: &[u8; 80]) -> u32 {
    u32::from_le_bytes([header[72], header[73], header[74], header[75]])
}

pub fn build_header(version: u32, prev: &[u8; 32], merkle: &[u8; 32], time: u32, nbits: u32, nonce: u32) -> [u8; 80] {
    let mut h = [0u8; 80];
    h[0..4].copy_from_slice(&version.to_le_bytes());
    h[4..36].copy_from_slice(prev);
    h[36..68].copy_from_slice(merkle);
    h[68..72].copy_from_slice(&time.to_le_bytes());
    h[72..76].copy_from_slice(&nbits.to_le_bytes());
    h[76..80].copy_from_slice(&nonce.to_le_bytes());
    h
}

/// First nonce from 0 whose header meets `nbits`, or `None` if none exists in `0 .. limit`.
pub fn mine_nonce(prev: &[u8; 32], merkle: &[u8; 32], time: u32, nbits: u32, limit: u32) -> Option<[u8; 80]> {
    let mut nonce = 0u32;
    loop {
        let header = build_header(0x2000_0000, prev, merkle, time, nbits, nonce);
        if hash_meets_nbits(&header_hash(&header), nbits) {
            return Some(header);
        }
        if nonce + 1 >= limit {
            return None;
        }
        nonce += 1;
    }
}

pub fn hash_meets_nbits(hash: &[u8; 32], nbits: u32) -> bool {
    let Some(target) = compact_to_target(nbits) else {
        return false;
    };
    if target == [0u8; 32] {
        return false;
    }
    target_cmp(hash, &target) != std::cmp::Ordering::Greater
}

/// Bitcoin compact `nBits` to a 32-byte little-endian target. `None` on negative or overflow.
pub fn compact_to_target(nbits: u32) -> Option<[u8; 32]> {
    if nbits & 0x0080_0000 != 0 {
        return None;
    }
    let size = (nbits >> 24) as usize;
    let word = nbits & 0x007f_ffff;
    if size > 32 {
        return None;
    }
    let mut out = [0u8; 32];
    if size <= 3 {
        let word = word >> (8 * (3 - size));
        let bytes = word.to_le_bytes();
        out[0] = bytes[0];
        if size > 1 {
            out[1] = bytes[1];
        }
        if size > 2 {
            out[2] = bytes[2];
        }
    } else {
        let start = size - 3;
        if start + 2 >= 32 {
            return None;
        }
        out[start] = (word & 0xff) as u8;
        out[start + 1] = ((word >> 8) & 0xff) as u8;
        out[start + 2] = ((word >> 16) & 0xff) as u8;
    }
    Some(out)
}

pub fn target_to_compact(target: &[u8; 32]) -> u32 {
    let mut nsize = 0usize;
    for i in (0..32).rev() {
        if target[i] != 0 {
            nsize = i + 1;
            break;
        }
    }
    if nsize == 0 {
        return 0;
    }
    let mut compact: u32 = if nsize <= 3 {
        let mut word = 0u32;
        for i in 0..nsize {
            word |= (target[i] as u32) << (8 * i);
        }
        word << (8 * (3 - nsize))
    } else {
        let start = nsize - 3;
        (target[start] as u32) | ((target[start + 1] as u32) << 8) | ((target[start + 2] as u32) << 16)
    };
    if compact & 0x0080_0000 != 0 {
        compact >>= 8;
        nsize += 1;
    }
    compact |= (nsize as u32) << 24;
    compact
}

fn target_cmp(a: &[u8; 32], b: &[u8; 32]) -> std::cmp::Ordering {
    for i in (0..32).rev() {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    std::cmp::Ordering::Equal
}

fn mul_div_target(target: &[u8; 32], mul: u64, div: u64) -> [u8; 32] {
    let mut limbs = [0u64; 5];
    let mut carry = 0u128;
    for i in 0..4 {
        let limb = u64::from_le_bytes(target[i * 8..i * 8 + 8].try_into().unwrap());
        let prod = (limb as u128) * (mul as u128) + carry;
        limbs[i] = prod as u64;
        carry = prod >> 64;
    }
    limbs[4] = carry as u64;
    let mut rem = 0u128;
    let mut out_limbs = [0u64; 5];
    for i in (0..5).rev() {
        let cur = (rem << 64) | (limbs[i] as u128);
        out_limbs[i] = (cur / (div as u128)) as u64;
        rem = cur % (div as u128);
    }
    let mut out = [0u8; 32];
    if out_limbs[4] != 0 {
        return [0xff; 32];
    }
    for i in 0..4 {
        out[i * 8..i * 8 + 8].copy_from_slice(&out_limbs[i].to_le_bytes());
    }
    out
}

/// SPEC §13. `{bari-url}` to the RFC 6454 origin. No path, query, fragment, or trailing slash.
pub fn origin_from_bari_url(url: &str) -> Result<String, String> {
    if url.ends_with('/') {
        return Err("bari-url has a trailing slash".into());
    }
    let (scheme, rest) = url.split_once("://").ok_or("bari-url is not an absolute URL")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("bari-url scheme is not http or https".into());
    }
    if rest.is_empty() || rest.contains('/') || rest.contains('?') || rest.contains('#') || rest.contains('@') {
        return Err("bari-url is not an origin".into());
    }
    let (host, port) = if let Some(rest) = rest.strip_prefix('[') {
        let (host, after) = rest.split_once(']').ok_or("bad IPv6 host")?;
        if host.is_empty() {
            return Err("empty host".into());
        }
        let port = if let Some(p) = after.strip_prefix(':') {
            Some(parse_port(p)?)
        } else if after.is_empty() {
            None
        } else {
            return Err("junk after IPv6 host".into());
        };
        (host.to_ascii_lowercase(), port)
    } else {
        if let Some((host, port)) = rest.rsplit_once(':') {
            if host.is_empty() {
                return Err("empty host".into());
            }
            (host.to_ascii_lowercase(), Some(parse_port(port)?))
        } else {
            (rest.to_ascii_lowercase(), None)
        }
    };
    if host.is_empty() {
        return Err("empty host".into());
    }
    let default = if scheme == "http" { 80 } else { 443 };
    let mut origin = format!("{scheme}://");
    if host.contains(':') && !host.starts_with('[') {
        origin.push('[');
        origin.push_str(&host);
        origin.push(']');
    } else {
        origin.push_str(&host);
    }
    if let Some(port) = port {
        if port != default {
            origin.push(':');
            origin.push_str(&port.to_string());
        }
    }
    Ok(origin)
}

fn parse_port(text: &str) -> Result<u16, String> {
    if text.is_empty() || (text.len() > 1 && text.as_bytes()[0] == b'0') || !text.bytes().all(|c| c.is_ascii_digit()) {
        return Err("bad port".into());
    }
    text.parse::<u16>().map_err(|_| "bad port".into())
}

/// Display order of a natural hash. SPEC §9. `display[i] = natural[31 − i]`.
pub fn display_hash(natural: &[u8; 32]) -> [u8; 32] {
    reverse32(natural)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hexutil::hex_decode_exact;

    #[test]
    fn genesis_header_hash_matches_the_pack() {
        let raw: [u8; 80] = hex_decode_exact(
            "0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c",
        )
        .unwrap();
        let natural = header_hash(&raw);
        let expect: [u8; 32] = hex_decode_exact("6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000").unwrap();
        assert_eq!(natural, expect);
        assert_eq!(
            display_hash(&natural),
            hex_decode_exact::<32>("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f").unwrap()
        );
        assert_eq!(header_nbits(&raw), 0x1d00ffff);
        assert_eq!(header_time(&raw), 1_231_006_505);
    }

    #[test]
    fn compact_roundtrip_genesis_and_synth() {
        for n in [0x1d00ffff, 0x207fffff, 0x1e00ffff, 0x1f00ffff] {
            let t = compact_to_target(n).unwrap();
            assert_eq!(target_to_compact(&t), n, "{n:08x}");
        }
    }

    #[test]
    fn retarget_copies_off_boundary_and_clamps_on_boundary() {
        assert_eq!(mainnet_next_nbits(100, 0x1f00ffff, 0, None), Some(0x1f00ffff));
        let same = mainnet_next_nbits(2015, 0x1d00ffff, 1_231_006_505 + 1_209_600, Some(1_231_006_505));
        assert_eq!(same, Some(0x1d00ffff));
        let clamped = mainnet_next_nbits(2015, 0x207fffff, 1_231_006_505 + 1_209_600, Some(1_231_006_505));
        assert_eq!(clamped, Some(0x1d00ffff));
    }

    #[test]
    fn origin_spelling() {
        assert_eq!(origin_from_bari_url("https://bari.example").unwrap(), "https://bari.example");
        assert_eq!(origin_from_bari_url("HTTPS://Bari.Example:443").unwrap(), "https://bari.example");
        assert_eq!(origin_from_bari_url("http://bari.example:80").unwrap(), "http://bari.example");
        assert_eq!(origin_from_bari_url("http://bari.example:8080").unwrap(), "http://bari.example:8080");
        assert!(origin_from_bari_url("https://bari.example/").is_err());
        assert!(origin_from_bari_url("https://user@bari.example").is_err());
    }
}
