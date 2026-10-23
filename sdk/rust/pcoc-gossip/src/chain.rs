//! Header chain for the live source.
//!
//! Mainnet uses the SDK's `mainnet_next_bits` (SPEC §9 retarget, including the
//! proof-of-work-limit clamp). A fixed-target chain is the in-process test
//! plane: every height expects one `nBits`, so a fork can be mined quickly.
//! It is not the live arm.

use std::collections::HashMap;

use pcoc::br1::{mainnet_next_bits, MAINNET_POW_LIMIT_BITS, SYNTH_NBITS};

use super::wire::sha256d;
use crate::TipTick;

pub const RETARGET_INTERVAL: u32 = 2016;
pub const HEADER_PEERS_MIN: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    BadAnchor,
    BadPow,
    UnexpectedDifficulty {
        height: u32,
        expected: u32,
        got: u32,
    },
    UnknownParent,
    Fork {
        height: u32,
    },
    BadCompact,
}

impl Fault {
    pub fn message(&self) -> String {
        match self {
            Fault::BadAnchor => "inconsistent chain".to_owned(),
            Fault::BadPow => "bad proof of work".to_owned(),
            Fault::UnexpectedDifficulty { .. } => "unexpected difficulty".to_owned(),
            Fault::UnknownParent => "inconsistent chain".to_owned(),
            Fault::Fork { .. } => "peer disagreement".to_owned(),
            Fault::BadCompact => "unexpected difficulty".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Mainnet,
    /// Every next header must carry this compact target.
    Fixed {
        bits: u32,
    },
}

#[derive(Clone, Copy)]
struct Node {
    header: [u8; 80],
    hash: [u8; 32],
    prev: [u8; 32],
    height: u32,
    bits: u32,
    time: u32,
}

/// Headers that link to one anchor. The tip is the most recently accepted
/// extension of that anchor, which is the only chain this clock follows.
#[derive(Clone)]
pub struct Chain {
    mode: Mode,
    nodes: HashMap<[u8; 32], Node>,
    by_height: HashMap<u32, [u8; 32]>,
    best: [u8; 32],
    anchor_height: u32,
    /// `nTime` of the first block of the anchor's difficulty period.
    period_start_time: u32,
}

impl Chain {
    pub fn open_mainnet(
        header: [u8; 80],
        height: u32,
        period_start_time: u32,
    ) -> Result<Self, Fault> {
        Self::open(Mode::Mainnet, header, height, period_start_time, None)
    }

    pub fn open_fixed(header: [u8; 80], height: u32, bits: u32) -> Result<Self, Fault> {
        Self::open(
            Mode::Fixed { bits },
            header,
            height,
            header_time(&header),
            Some(bits),
        )
    }

    fn open(
        mode: Mode,
        header: [u8; 80],
        height: u32,
        period_start_time: u32,
        fixed_bits: Option<u32>,
    ) -> Result<Self, Fault> {
        let bits = header_bits(&header);
        if fixed_bits.is_some_and(|want| want != bits) {
            return Err(Fault::BadAnchor);
        }
        let target = compact_target(bits)?;
        let limit = compact_target(match mode {
            Mode::Mainnet => MAINNET_POW_LIMIT_BITS,
            Mode::Fixed { bits } => bits,
        })?;
        if target.as_slice() > limit.as_slice() {
            return Err(Fault::BadAnchor);
        }
        if !hash_meets(&header_hash(&header), &target) {
            return Err(Fault::BadAnchor);
        }
        let hash = header_hash(&header);
        let node = Node {
            header,
            hash,
            prev: header_prev(&header),
            height,
            bits,
            time: header_time(&header),
        };
        let mut nodes = HashMap::new();
        nodes.insert(hash, node);
        let mut by_height = HashMap::new();
        by_height.insert(height, hash);
        Ok(Self {
            mode,
            nodes,
            by_height,
            best: hash,
            anchor_height: height,
            period_start_time,
        })
    }

    pub fn tip_hash(&self) -> [u8; 32] {
        self.best
    }

    pub fn tip_height(&self) -> u32 {
        self.nodes[&self.best].height
    }

    pub fn contains(&self, hash: &[u8; 32]) -> bool {
        self.nodes.contains_key(hash)
    }

    pub fn locator(&self) -> Vec<[u8; 32]> {
        let mut out = vec![self.best];
        let mut hash = self.best;
        let mut height = self.nodes[&hash].height;
        let mut step = 1u32;
        while height > self.anchor_height && out.len() < 32 {
            let goal = height.saturating_sub(step).max(self.anchor_height);
            while height > goal {
                let prev = self.nodes[&hash].prev;
                if !self.nodes.contains_key(&prev) {
                    break;
                }
                hash = prev;
                height -= 1;
            }
            if out.last() != Some(&hash) {
                out.push(hash);
            }
            if out.len() > 10 {
                step = step.saturating_mul(2);
            }
            if height <= self.anchor_height {
                break;
            }
        }
        out
    }

    pub fn append(&mut self, header: [u8; 80]) -> Result<(), Fault> {
        let hash = header_hash(&header);
        if self.nodes.contains_key(&hash) {
            return Ok(());
        }
        let node = self.validated_node(header)?;
        if self
            .by_height
            .get(&node.height)
            .is_some_and(|have| *have != node.hash)
        {
            return Err(Fault::Fork {
                height: node.height,
            });
        }
        self.by_height.insert(node.height, node.hash);
        self.best = node.hash;
        self.nodes.insert(node.hash, node);
        Ok(())
    }

    /// Check `header` and remember it without moving the adopted tip.
    ///
    /// A second valid child of one parent is kept so a later header can link
    /// to it. Callers stage on a scratch copy. The adopted chain uses `append`.
    pub fn stage(&mut self, header: [u8; 80]) -> Result<(), Fault> {
        let node = self.validated_node(header)?;
        self.nodes.insert(node.hash, node);
        Ok(())
    }

    pub fn prev_of(&self, hash: &[u8; 32]) -> Option<[u8; 32]> {
        self.nodes.get(hash).map(|node| node.prev)
    }

    fn validated_node(&self, header: [u8; 80]) -> Result<Node, Fault> {
        let hash = header_hash(&header);
        if let Some(node) = self.nodes.get(&hash).copied() {
            return Ok(node);
        }
        let prev = header_prev(&header);
        let parent = self.nodes.get(&prev).copied().ok_or(Fault::UnknownParent)?;
        let bits = header_bits(&header);
        let height = parent.height.saturating_add(1);
        let expected = self.expected_bits(&parent)?;
        if bits != expected {
            return Err(Fault::UnexpectedDifficulty {
                height,
                expected,
                got: bits,
            });
        }
        let target = compact_target(bits)?;
        if !hash_meets(&hash, &target) {
            return Err(Fault::BadPow);
        }
        Ok(Node {
            header,
            hash,
            prev,
            height,
            bits,
            time: header_time(&header),
        })
    }

    pub fn tick(&self, hash: [u8; 32]) -> Result<TipTick, Fault> {
        let node = self.nodes.get(&hash).ok_or(Fault::UnknownParent)?;
        let period_first_time = if node.height.wrapping_add(1) % RETARGET_INTERVAL == 0 {
            self.period_first_time(node)?
        } else {
            0
        };
        Ok(TipTick {
            hash: node.hash,
            height: u64::from(node.height),
            previous: Some(node.prev),
            n_time: node.time,
            header: node.header,
            period_first_time,
        })
    }

    /// Hashes from `after` (exclusive) up to `tip`, oldest first.
    pub fn path_after(&self, after: [u8; 32], tip: [u8; 32]) -> Result<Vec<TipTick>, Fault> {
        let mut out = Vec::new();
        let mut hash = tip;
        loop {
            if hash == after {
                break;
            }
            let tick = self.tick(hash)?;
            let prev = tick.previous.ok_or(Fault::UnknownParent)?;
            out.push(tick);
            hash = prev;
            if out.len() > 10_000 {
                return Err(Fault::UnknownParent);
            }
        }
        out.reverse();
        Ok(out)
    }

    fn expected_bits(&self, parent: &Node) -> Result<u32, Fault> {
        match self.mode {
            Mode::Fixed { bits } => Ok(bits),
            Mode::Mainnet => {
                let next = parent.height.saturating_add(1);
                if next % RETARGET_INTERVAL != 0 {
                    return Ok(parent.bits);
                }
                let first = self.period_first_time(parent)?;
                Ok(mainnet_next_bits(
                    parent.height,
                    parent.bits,
                    parent.time,
                    first,
                ))
            }
        }
    }

    fn period_first_time(&self, parent: &Node) -> Result<u32, Fault> {
        let target = parent
            .height
            .checked_sub(RETARGET_INTERVAL - 1)
            .ok_or(Fault::UnknownParent)?;
        let mut hash = parent.hash;
        let mut height = parent.height;
        while height > target {
            let node = self.nodes.get(&hash).ok_or(Fault::UnknownParent)?;
            if height == self.anchor_height || !self.nodes.contains_key(&node.prev) {
                break;
            }
            hash = node.prev;
            height -= 1;
        }
        if height == target {
            return Ok(self.nodes[&hash].time);
        }
        if target < self.anchor_height {
            return Ok(self.period_start_time);
        }
        Err(Fault::UnknownParent)
    }
}

pub fn header_hash(header: &[u8; 80]) -> [u8; 32] {
    // Same SHA-256d the SDK uses for Rule BR1. `sha256d` here is the wire
    // checksum helper; both are SHA-256 twice.
    let via_sdk = pcoc::header_hash(header);
    debug_assert_eq!(via_sdk, sha256d(header));
    via_sdk
}

pub fn header_prev(header: &[u8; 80]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&header[4..36]);
    out
}

pub fn header_time(header: &[u8; 80]) -> u32 {
    u32::from_le_bytes(header[68..72].try_into().expect("nTime"))
}

pub fn header_bits(header: &[u8; 80]) -> u32 {
    u32::from_le_bytes(header[72..76].try_into().expect("nBits"))
}

/// Mine one header. `bits` must be an easy target (`SYNTH_NBITS` in tests).
pub fn mine_header(prev: [u8; 32], time: u32, bits: u32) -> Result<[u8; 80], Fault> {
    let target = compact_target(bits)?;
    for nonce in 0..100_000u32 {
        let header = pcoc::br1::assemble_header(&prev, time, bits, nonce);
        if hash_meets(&header_hash(&header), &target) {
            return Ok(header);
        }
    }
    Err(Fault::BadPow)
}

pub fn synth_bits() -> u32 {
    SYNTH_NBITS
}

/// Change the nonce until the header misses its own target. `nBits` stays put.
#[cfg(test)]
pub fn spoil_proof(header: &mut [u8; 80]) {
    let bits = header_bits(header);
    let target = compact_target(bits).expect("compact target");
    for nonce in 0..200_000u32 {
        header[76..80].copy_from_slice(&nonce.to_le_bytes());
        if !hash_meets(&header_hash(header), &target) {
            return;
        }
    }
    panic!("no nonce missed the target");
}

fn compact_target(bits: u32) -> Result<[u8; 32], Fault> {
    let size = (bits >> 24) as usize;
    let word = bits & 0x007f_ffff;
    let negative = word != 0 && (bits & 0x0080_0000) != 0;
    let overflow =
        word != 0 && (size > 34 || (word > 0xff && size > 33) || (word > 0xffff && size > 32));
    if negative || overflow || word == 0 || size == 0 || size > 32 {
        return Err(Fault::BadCompact);
    }
    let mut be = [0u8; 32];
    if size <= 3 {
        let value = word >> (8 * (3 - size));
        be[31] = value as u8;
        if size > 1 {
            be[30] = (value >> 8) as u8;
        }
        if size > 2 {
            be[29] = (value >> 16) as u8;
        }
    } else {
        let start = 32 - size;
        be[start] = ((word >> 16) & 0xff) as u8;
        be[start + 1] = ((word >> 8) & 0xff) as u8;
        be[start + 2] = (word & 0xff) as u8;
    }
    Ok(be)
}

fn hash_meets(hash_le: &[u8; 32], target_be: &[u8; 32]) -> bool {
    let mut hash_be = *hash_le;
    hash_be.reverse();
    hash_be.as_slice() <= target_be.as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorded::{
        ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME, GENESIS_TIME, HEADER_2015, HEADER_2016,
        HEADER_30240_TIME, HEADER_32255, HEADER_32256, HEADER_971713,
    };

    #[test]
    fn recorded_anchor_and_child_pass_and_a_flipped_nonce_does_not() {
        let mut chain = Chain::open_mainnet(ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME).unwrap();
        assert_eq!(chain.tip_height(), ANCHOR_HEIGHT);
        assert_eq!(ANCHOR_HEIGHT % RETARGET_INTERVAL, 0);
        chain.append(HEADER_971713).unwrap();
        assert_eq!(chain.tip_height(), ANCHOR_HEIGHT + 1);
        let mut bad = HEADER_971713;
        bad[79] ^= 0x01;
        assert_eq!(chain.append(bad).unwrap_err(), Fault::BadPow);
    }

    #[test]
    fn wrong_nbits_is_unexpected_difficulty_before_proof_of_work() {
        let mut chain = Chain::open_mainnet(ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME).unwrap();
        let mut header = HEADER_971713;
        header[72..76].copy_from_slice(&0x1d00ffffu32.to_le_bytes());
        match chain.append(header).unwrap_err() {
            Fault::UnexpectedDifficulty { expected, got, .. } => {
                assert_eq!(expected, header_bits(&HEADER_971713));
                assert_eq!(got, 0x1d00ffff);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn first_retarget_clamps_and_height_32256_changes_difficulty() {
        let mut early = Chain::open_mainnet(HEADER_2015, 2015, GENESIS_TIME).unwrap();
        early.append(HEADER_2016).unwrap();
        assert_eq!(header_bits(&HEADER_2016), 0x1d00ffff);

        let mut chain = Chain::open_mainnet(HEADER_32255, 32_255, HEADER_30240_TIME).unwrap();
        chain.append(HEADER_32256).unwrap();
        assert_eq!(header_bits(&HEADER_32256), 0x1d00d86a);
        let tick = chain.tick(chain.tip_hash()).unwrap();
        // The next height is not a boundary, so the tick does not carry the anchor time.
        assert_eq!(tick.period_first_time, 0);
        let mut same_bits = HEADER_32256;
        same_bits[72..76].copy_from_slice(&0x1d00ffffu32.to_le_bytes());
        // The parent already occupies this height, and the bits are wrong first.
        assert!(matches!(
            chain.append(same_bits),
            Err(Fault::UnexpectedDifficulty { .. })
        ));
    }

    #[test]
    fn two_valid_children_of_one_parent_are_a_fork() {
        let bits = synth_bits();
        let anchor = mine_header([0x44; 32], 1_700_000_000, bits).unwrap();
        let mut chain = Chain::open_fixed(anchor, 50, bits).unwrap();
        let parent = header_hash(&anchor);
        let a = mine_header(parent, 1_700_000_100, bits).unwrap();
        let b = mine_header(parent, 1_700_000_200, bits).unwrap();
        assert_ne!(header_hash(&a), header_hash(&b));
        chain.append(a).unwrap();
        assert_eq!(chain.append(b).unwrap_err(), Fault::Fork { height: 51 });
    }

    #[test]
    fn a_header_that_does_not_link_is_an_inconsistent_chain() {
        let bits = synth_bits();
        let anchor = mine_header([0x45; 32], 1_700_000_000, bits).unwrap();
        let mut chain = Chain::open_fixed(anchor, 7, bits).unwrap();
        let orphan = mine_header([0x99; 32], 1_700_000_050, bits).unwrap();
        assert_eq!(chain.append(orphan).unwrap_err(), Fault::UnknownParent);
    }
}
