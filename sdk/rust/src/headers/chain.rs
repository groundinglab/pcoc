//! Bitcoin header chain. SPEC §9: the chain with the most work, natural byte order (P9.17).
//!
//! Live difficulty is Bitcoin mainnet: expected `nBits` at each height, retarget every 2016
//! blocks. The test plane uses the fixed synth `nBits` `0x207fffff` (section 9).

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::hash::sha256;

/// Bitcoin mainnet wire magic `F9 BE B4 D9`.
pub const MAINNET_MAGIC: [u8; 4] = [0xF9, 0xBE, 0xB4, 0xD9];
/// Bitcoin regtest wire magic `FA BF B5 DA`. SPEC §9: test-plane gossip magic.
pub const REGTEST_MAGIC: [u8; 4] = [0xFA, 0xBF, 0xB5, 0xDA];

/// Difficulty-1 compact target on Bitcoin mainnet (`powLimit`).
pub const BITCOIN_DIFF1_NBITS: u32 = 0x1d00ffff;
/// SPEC §9. Every synth header carries this `nBits`.
pub const SYNTH_NBITS: u32 = 0x207fffff;

/// Bitcoin retarget interval. Not a SPEC lever and not an SDK option.
pub const RETARGET_INTERVAL: u64 = 2016;
/// Bitcoin target timespan, 14 days in seconds.
pub const TARGET_TIMESPAN: u64 = 14 * 24 * 60 * 60;

/// Why a header was not accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeaderFault {
    /// Compact `nBits` is negative, overflowing, or zero.
    BadCompact,
    /// The target is easier than the plane's proof-of-work limit.
    PowLimit,
    /// `nBits` is not the expected compact target for this height.
    UnexpectedNBits {
        height: u64,
        expected: u32,
        got: u32,
    },
    /// The header hash is above the target its `nBits` encodes.
    BadPow,
    /// `hashPrevBlock` is not a header this chain already holds.
    UnknownParent,
    /// The configured genesis header does not meet the plane rules.
    BadGenesis,
    /// The peer's message did not follow the header wire.
    Wire,
}

/// Result of offering a header to the chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appended {
    Stored,
    AlreadyHad,
}

/// Network parameters for one plane. Retarget width and timespan are the Bitcoin constants.
#[derive(Clone, Debug)]
pub struct ConsensusParams {
    pub magic: [u8; 4],
    pub pow_limit_bits: u32,
    /// When set, every height expects this `nBits` (SPEC §9 synth headers). Live is `None`.
    pub fixed_nbits: Option<u32>,
    pub genesis: [u8; 80],
    pub port: u16,
}

impl ConsensusParams {
    /// Live plane: Bitcoin mainnet genesis, magic, and proof-of-work limit.
    pub fn mainnet() -> Self {
        Self {
            magic: MAINNET_MAGIC,
            pow_limit_bits: BITCOIN_DIFF1_NBITS,
            fixed_nbits: None,
            genesis: MAINNET_GENESIS,
            port: 8333,
        }
    }

    /// Test plane: regtest magic and the fixed synth `nBits` (SPEC §9).
    pub fn regtest() -> Self {
        Self {
            magic: REGTEST_MAGIC,
            pow_limit_bits: SYNTH_NBITS,
            fixed_nbits: Some(SYNTH_NBITS),
            genesis: REGTEST_GENESIS,
            port: 18444,
        }
    }
}

/// Mainnet genesis header, wire order.
pub const MAINNET_GENESIS: [u8; 80] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e,
    0x67, 0x76, 0x8f, 0x61, 0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa,
    0x4b, 0x1e, 0x5e, 0x4a, 0x29, 0xab, 0x5f, 0x49, 0xff, 0xff, 0x00, 0x1d, 0x1d, 0xac, 0x2b, 0x7c,
];

/// Regtest genesis header, wire order.
pub const REGTEST_GENESIS: [u8; 80] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e,
    0x67, 0x76, 0x8f, 0x61, 0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa,
    0x4b, 0x1e, 0x5e, 0x4a, 0xda, 0xe5, 0x49, 0x4d, 0xff, 0xff, 0x7f, 0x20, 0x02, 0x00, 0x00, 0x00,
];

/// SPEC §9 P9.17. `SHA-256(SHA-256(header))` in natural byte order.
pub fn header_hash(header: &[u8; 80]) -> [u8; 32] {
    sha256(&sha256(header))
}

pub fn header_prev(header: &[u8; 80]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&header[4..36]);
    out
}

pub fn header_time(header: &[u8; 80]) -> u32 {
    u32::from_le_bytes(header[68..72].try_into().expect("ntime"))
}

pub fn header_nbits(header: &[u8; 80]) -> u32 {
    u32::from_le_bytes(header[72..76].try_into().expect("nbits"))
}

/// True when the header hash is at or below the target in its own `nBits`.
/// This is the check that let diff-1 headers through. Callers that accept a
/// header also require [`HeaderChain::append`]'s expected-`nBits` test.
pub fn header_meets_own_target(header: &[u8; 80]) -> bool {
    let Ok(target) = compact_to_target(header_nbits(header)) else {
        return false;
    };
    if target.is_zero() {
        return false;
    }
    U256::from_le_bytes(&header_hash(header)) <= target
}

#[derive(Clone, Copy, Debug)]
struct Node {
    header: [u8; 80],
    hash: [u8; 32],
    prev: [u8; 32],
    height: u64,
    nbits: u32,
    time: u32,
    work: U256,
}

/// Headers keyed by natural hash. The tip is the most-work header, not the tallest.
#[derive(Clone, Debug)]
pub struct HeaderChain {
    params: ConsensusParams,
    nodes: HashMap<[u8; 32], Node>,
    best: [u8; 32],
    best_work: U256,
    best_height: u64,
}

impl HeaderChain {
    pub fn open(params: ConsensusParams) -> Result<Self, HeaderFault> {
        let genesis = params.genesis;
        let nbits = header_nbits(&genesis);
        if params.fixed_nbits.is_some_and(|f| f != nbits) || nbits != params.pow_limit_bits {
            return Err(HeaderFault::BadGenesis);
        }
        let target = compact_to_target(nbits)?;
        let limit = compact_to_target(params.pow_limit_bits)?;
        if target > limit {
            return Err(HeaderFault::PowLimit);
        }
        if !header_meets_own_target(&genesis) {
            return Err(HeaderFault::BadGenesis);
        }
        let hash = header_hash(&genesis);
        let work = block_work(nbits)?;
        let node = Node {
            header: genesis,
            hash,
            prev: header_prev(&genesis),
            height: 0,
            nbits,
            time: header_time(&genesis),
            work,
        };
        let mut nodes = HashMap::new();
        nodes.insert(hash, node);
        Ok(Self {
            params,
            nodes,
            best: hash,
            best_work: work,
            best_height: 0,
        })
    }

    pub fn params(&self) -> &ConsensusParams {
        &self.params
    }

    pub fn tip_hash(&self) -> [u8; 32] {
        self.best
    }

    pub fn tip_height(&self) -> u64 {
        self.best_height
    }

    pub fn tip_header(&self) -> [u8; 80] {
        self.nodes[&self.best].header
    }

    pub fn contains(&self, hash: &[u8; 32]) -> bool {
        self.nodes.contains_key(hash)
    }

    pub fn header(&self, hash: &[u8; 32]) -> Option<[u8; 80]> {
        self.nodes.get(hash).map(|n| n.header)
    }

    pub fn height_of(&self, hash: &[u8; 32]) -> Option<u64> {
        self.nodes.get(hash).map(|n| n.height)
    }

    /// Expected compact `nBits` for the block that builds on `parent`.
    pub fn expected_next_nbits(&self, parent: &[u8; 32]) -> Result<u32, HeaderFault> {
        let node = self.nodes.get(parent).ok_or(HeaderFault::UnknownParent)?;
        self.expected_after(node)
    }

    fn expected_after(&self, parent: &Node) -> Result<u32, HeaderFault> {
        if let Some(fixed) = self.params.fixed_nbits {
            return Ok(fixed);
        }
        let next_height = parent.height + 1;
        if next_height % RETARGET_INTERVAL != 0 {
            return Ok(parent.nbits);
        }
        let mut first = parent.hash;
        for _ in 0..(RETARGET_INTERVAL - 1) {
            first = self
                .nodes
                .get(&first)
                .ok_or(HeaderFault::UnknownParent)?
                .prev;
        }
        let first_time = self
            .nodes
            .get(&first)
            .ok_or(HeaderFault::UnknownParent)?
            .time;
        retarget_for(
            parent.nbits,
            parent.time,
            first_time,
            self.params.pow_limit_bits,
        )
    }

    /// Bitcoin `GetLocator` from the most-work tip. Hashes are natural order.
    pub fn locator(&self) -> Vec<[u8; 32]> {
        let mut out = Vec::new();
        let mut step: u64 = 1;
        let mut current = self.best;
        loop {
            out.push(current);
            let node = &self.nodes[&current];
            if node.height == 0 || out.len() >= 64 {
                break;
            }
            let next_h = node.height.saturating_sub(step);
            let mut cur_hash = current;
            let mut cur_h = node.height;
            while cur_h > next_h {
                let n = &self.nodes[&cur_hash];
                cur_hash = n.prev;
                cur_h -= 1;
            }
            current = cur_hash;
            if out.len() > 10 {
                step = step.saturating_mul(2);
            }
        }
        out
    }

    pub fn append(&mut self, header: [u8; 80]) -> Result<Appended, HeaderFault> {
        let hash = header_hash(&header);
        if self.nodes.contains_key(&hash) {
            return Ok(Appended::AlreadyHad);
        }
        let prev = header_prev(&header);
        let parent = self
            .nodes
            .get(&prev)
            .cloned()
            .ok_or(HeaderFault::UnknownParent)?;
        let nbits = header_nbits(&header);
        let target = compact_to_target(nbits)?;
        let limit = compact_to_target(self.params.pow_limit_bits)?;
        if target.is_zero() {
            return Err(HeaderFault::BadCompact);
        }
        if target > limit {
            return Err(HeaderFault::PowLimit);
        }
        let height = parent.height + 1;
        let expected = self.expected_after(&parent)?;
        if nbits != expected {
            return Err(HeaderFault::UnexpectedNBits {
                height,
                expected,
                got: nbits,
            });
        }
        if U256::from_le_bytes(&hash) > target {
            return Err(HeaderFault::BadPow);
        }
        let step = block_work(nbits)?;
        let work = parent.work.wrapping_add(step);
        let node = Node {
            header,
            hash,
            prev,
            height,
            nbits,
            time: header_time(&header),
            work,
        };
        if work > self.best_work {
            self.best = hash;
            self.best_work = work;
            self.best_height = height;
        }
        self.nodes.insert(hash, node);
        Ok(Appended::Stored)
    }
}

/// Bitcoin `CalculateNextWorkRequired`: clamp the timespan to 1/4…4×, scale the
/// parent target, and do not let the result get easier than `pow_limit_bits`.
fn retarget_for(
    parent_bits: u32,
    parent_time: u32,
    first_time: u32,
    pow_limit_bits: u32,
) -> Result<u32, HeaderFault> {
    let mut span = parent_time as i64 - first_time as i64;
    let min_span = (TARGET_TIMESPAN / 4) as i64;
    let max_span = (TARGET_TIMESPAN.saturating_mul(4)) as i64;
    if span < min_span {
        span = min_span;
    }
    if span > max_span {
        span = max_span;
    }
    let target = compact_to_target(parent_bits)?;
    let wide = target.mul_u64(span as u64);
    let (mut quot, overflow) = wide.div_u64(TARGET_TIMESPAN);
    let limit = compact_to_target(pow_limit_bits)?;
    if overflow || quot > limit {
        quot = limit;
    }
    Ok(target_to_compact(&quot))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct U256 {
    d: [u64; 4],
}

impl U256 {
    const ZERO: Self = Self { d: [0, 0, 0, 0] };

    fn from_le_bytes(b: &[u8; 32]) -> Self {
        let mut d = [0u64; 4];
        for i in 0..4 {
            let mut limb = [0u8; 8];
            limb.copy_from_slice(&b[i * 8..i * 8 + 8]);
            d[i] = u64::from_le_bytes(limb);
        }
        Self { d }
    }

    fn is_zero(&self) -> bool {
        self.d.iter().all(|&x| x == 0)
    }

    fn bit_len(&self) -> u32 {
        for i in (0..4).rev() {
            if self.d[i] != 0 {
                return i as u32 * 64 + (64 - self.d[i].leading_zeros());
            }
        }
        0
    }

    fn bit(&self, i: u32) -> bool {
        let limb = (i / 64) as usize;
        let off = i % 64;
        ((self.d[limb] >> off) & 1) == 1
    }

    fn wrapping_add(self, other: Self) -> Self {
        let mut d = [0u64; 4];
        let mut c = 0u128;
        for i in 0..4 {
            c += self.d[i] as u128 + other.d[i] as u128;
            d[i] = c as u64;
            c >>= 64;
        }
        Self { d }
    }

    fn add_one(self) -> Self {
        self.wrapping_add(Self { d: [1, 0, 0, 0] })
    }

    fn not(self) -> Self {
        Self {
            d: [!self.d[0], !self.d[1], !self.d[2], !self.d[3]],
        }
    }

    fn shl_bytes(self, n: usize) -> Self {
        if n >= 32 {
            return Self::ZERO;
        }
        let b = self.to_le_bytes();
        let mut o = [0u8; 32];
        o[n..].copy_from_slice(&b[..32 - n]);
        Self::from_le_bytes(&o)
    }

    fn shr_bytes(self, n: usize) -> Self {
        if n >= 32 {
            return Self::ZERO;
        }
        let b = self.to_le_bytes();
        let mut o = [0u8; 32];
        o[..32 - n].copy_from_slice(&b[n..]);
        Self::from_le_bytes(&o)
    }

    fn to_le_bytes(self) -> [u8; 32] {
        let mut o = [0u8; 32];
        for i in 0..4 {
            o[i * 8..i * 8 + 8].copy_from_slice(&self.d[i].to_le_bytes());
        }
        o
    }

    fn low_u64(self) -> u64 {
        self.d[0]
    }

    fn mul_u64(self, m: u64) -> Wide {
        let mut d = [0u64; 5];
        let mut carry = 0u128;
        for i in 0..4 {
            let p = self.d[i] as u128 * m as u128 + carry;
            d[i] = p as u64;
            carry = p >> 64;
        }
        d[4] = carry as u64;
        Wide { d }
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for U256 {
    fn cmp(&self, other: &Self) -> Ordering {
        for i in (0..4).rev() {
            match self.d[i].cmp(&other.d[i]) {
                Ordering::Equal => {}
                o => return o,
            }
        }
        Ordering::Equal
    }
}

struct Wide {
    d: [u64; 5],
}

impl Wide {
    fn div_u64(self, m: u64) -> (U256, bool) {
        if m == 0 {
            return (U256::ZERO, true);
        }
        let mut rem = 0u128;
        let mut q = [0u64; 5];
        for i in (0..5).rev() {
            let cur = (rem << 64) | self.d[i] as u128;
            q[i] = (cur / m as u128) as u64;
            rem = cur % m as u128;
        }
        let overflow = q[4] != 0;
        (
            U256 {
                d: [q[0], q[1], q[2], q[3]],
            },
            overflow,
        )
    }
}

fn compact_to_target(compact: u32) -> Result<U256, HeaderFault> {
    let n_size = compact >> 24;
    let mut n_word = compact & 0x007f_ffff;
    let negative = n_word != 0 && (compact & 0x0080_0000) != 0;
    let overflow = n_word != 0
        && (n_size > 34 || (n_word > 0xff && n_size > 33) || (n_word > 0xffff && n_size > 32));
    if negative || overflow {
        return Err(HeaderFault::BadCompact);
    }
    let value = if n_size <= 3 {
        n_word >>= 8 * (3 - n_size);
        U256 {
            d: [u64::from(n_word), 0, 0, 0],
        }
    } else {
        U256 {
            d: [u64::from(n_word), 0, 0, 0],
        }
        .shl_bytes((n_size as usize).saturating_sub(3))
    };
    if value.is_zero() {
        return Err(HeaderFault::BadCompact);
    }
    Ok(value)
}

fn target_to_compact(target: &U256) -> u32 {
    if target.is_zero() {
        return 0;
    }
    let mut n_size = (target.bit_len() + 7) / 8;
    let mut n_compact: u32 = if n_size <= 3 {
        (target.low_u64() << (8 * (3 - n_size))) as u32
    } else {
        target.shr_bytes((n_size as usize) - 3).low_u64() as u32
    };
    if n_compact & 0x0080_0000 != 0 {
        n_compact >>= 8;
        n_size += 1;
    }
    n_compact |= n_size << 24;
    n_compact
}

fn block_work(nbits: u32) -> Result<U256, HeaderFault> {
    let target = compact_to_target(nbits)?;
    // floor(2^256 / (target + 1)) == (~target / (target + 1)) + 1
    let denom = target.add_one();
    if denom.is_zero() {
        return Err(HeaderFault::BadCompact);
    }
    let numer = target.not();
    Ok(div_u256(&numer, &denom).add_one())
}

/// Binary long division. The remainder uses an extra limb so an easy target
/// near 2^255 does not overflow when it is doubled.
fn div_u256(numer: &U256, denom: &U256) -> U256 {
    if denom.is_zero() {
        return U256::ZERO;
    }
    let mut q = [0u64; 4];
    let mut r = [0u64; 5];
    for i in (0..256).rev() {
        shl1_5(&mut r);
        if numer.bit(i) {
            r[0] |= 1;
        }
        if cmp5_u256(&r, denom) != Ordering::Less {
            sub5_u256(&mut r, denom);
            let limb = (i / 64) as usize;
            let off = i % 64;
            q[limb] |= 1u64 << off;
        }
    }
    U256 { d: q }
}

fn shl1_5(r: &mut [u64; 5]) {
    let mut carry = 0u64;
    for limb in r.iter_mut() {
        let next = *limb >> 63;
        *limb = (*limb << 1) | carry;
        carry = next;
    }
}

fn cmp5_u256(r: &[u64; 5], denom: &U256) -> Ordering {
    if r[4] != 0 {
        return Ordering::Greater;
    }
    for i in (0..4).rev() {
        match r[i].cmp(&denom.d[i]) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    Ordering::Equal
}

fn sub5_u256(r: &mut [u64; 5], denom: &U256) {
    let mut borrow = 0i128;
    for i in 0..4 {
        borrow += r[i] as i128 - denom.d[i] as i128;
        r[i] = borrow as u64;
        borrow >>= 64;
    }
    borrow += r[4] as i128;
    r[4] = borrow as u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_and_regtest_genesis_hashes() {
        let main = header_hash(&MAINNET_GENESIS);
        assert_eq!(
            hex_of(&main),
            "6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000"
        );
        let reg = header_hash(&REGTEST_GENESIS);
        assert_eq!(
            hex_of(&reg),
            "06226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f"
        );
        assert!(header_meets_own_target(&MAINNET_GENESIS));
        assert!(header_meets_own_target(&REGTEST_GENESIS));
        HeaderChain::open(ConsensusParams::mainnet()).unwrap();
        HeaderChain::open(ConsensusParams::regtest()).unwrap();
    }

    #[test]
    fn compact_roundtrip_diff1_and_synth() {
        for bits in [BITCOIN_DIFF1_NBITS, SYNTH_NBITS, 0x1b0404cb] {
            let t = compact_to_target(bits).unwrap();
            assert_eq!(target_to_compact(&t), bits, "{bits:#x}");
        }
    }

    #[test]
    fn retarget_identity_and_clamps() {
        let same = retarget_for(
            SYNTH_NBITS,
            1_000_000 + TARGET_TIMESPAN as u32,
            1_000_000,
            SYNTH_NBITS,
        )
        .unwrap();
        assert_eq!(same, SYNTH_NBITS);
        let harder = retarget_for(
            SYNTH_NBITS,
            1_000_000 + (TARGET_TIMESPAN / 4) as u32,
            1_000_000,
            SYNTH_NBITS,
        )
        .unwrap();
        let easy_t = compact_to_target(SYNTH_NBITS).unwrap();
        let hard_t = compact_to_target(harder).unwrap();
        assert!(hard_t < easy_t);
        let work_easy = block_work(SYNTH_NBITS).unwrap();
        let work_hard = block_work(harder).unwrap();
        assert!(work_hard > work_easy.wrapping_add(work_easy).wrapping_add(work_easy));
        // Already at the pow limit, a slow interval cannot get easier.
        let easier = retarget_for(
            SYNTH_NBITS,
            1_000_000 + (TARGET_TIMESPAN * 4) as u32,
            1_000_000,
            SYNTH_NBITS,
        )
        .unwrap();
        assert_eq!(easier, SYNTH_NBITS);
    }

    #[test]
    fn natural_prev_matches_parent_hash() {
        let chain = HeaderChain::open(ConsensusParams::regtest()).unwrap();
        let g = chain.tip_header();
        assert_eq!(header_hash(&g), chain.tip_hash());
        let mut reversed = chain.tip_hash();
        reversed.reverse();
        assert_ne!(header_prev(&g), reversed);
    }

    fn hex_of(bytes: &[u8]) -> String {
        const D: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::new();
        for b in bytes {
            s.push(D[(b >> 4) as usize] as char);
            s.push(D[(b & 0xf) as usize] as char);
        }
        s
    }
}
