//! Rig only. Release verify does not take a byte source.
//! The named `*_from_bytes` functions are the injected-draw variants (SPEC §14.6, §15.5, §16.1).
#![deny(unsafe_code)]

use pcoc::draw::{self, DrawStop};
use pcoc::hash::sha256;
use pcoc::hexutil::{hex_encode, reverse32};
use pcoc::profile::Profile;
use pcoc::verify::{self, HeightIndex, Verified};
use std::collections::HashMap;

/// SPEC §16.1 Draw bytes. `b_0 = SHA-256(label)`, `b_{n+1} = SHA-256(b_n)`.
/// The verifier consumes these bytes. It does not hash them again.
#[derive(Clone, Debug)]
pub struct LabelChain {
    block: [u8; 32],
    pos: usize,
}

impl LabelChain {
    pub fn new(label: &str) -> Self {
        Self {
            block: sha256(label.as_bytes()),
            pos: 0,
        }
    }

    pub fn next8(&mut self) -> Option<[u8; 8]> {
        if self.pos == 32 {
            self.block = sha256(&self.block);
            self.pos = 0;
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.block[self.pos..self.pos + 8]);
        self.pos += 8;
        Some(b)
    }
}

/// SPEC §14.6. Six-leg draw from a caller-supplied byte source. Rig only.
pub fn six_leg_draw_from_bytes(
    leg_total: u64,
    next8: &mut dyn FnMut() -> Option<[u8; 8]>,
) -> Result<Vec<u64>, DrawStop> {
    draw::draw_six(leg_total, next8)
}

/// SPEC §15.5. Thorough draw from a caller-supplied byte source. Rig only.
pub fn thorough_draw_from_bytes(
    leg_total: u64,
    thorough_rounds: u32,
    next8: &mut dyn FnMut() -> Option<[u8; 8]>,
) -> Result<Vec<u64>, DrawStop> {
    draw::draw_thorough(leg_total, thorough_rounds, next8)
}

/// SPEC §14. Verify with an injected byte source. Rig only.
/// Release `pcoc::verify` reads the OS CSPRNG and does not call this.
pub fn verify_from_bytes(
    bytes: &[u8],
    profile: &Profile,
    index: &mut dyn HeightIndex,
    next8: &mut dyn FnMut() -> Option<[u8; 8]>,
) -> Verified {
    verify::verify_rig(bytes, profile, index, next8)
}

/// A clock that does not sleep. Tests advance it.
#[derive(Clone, Debug)]
pub struct MockClock {
    now: u64,
}

impl MockClock {
    pub fn new(start: u64) -> Self {
        Self { now: start }
    }

    pub fn now(&self) -> u64 {
        self.now
    }

    pub fn advance(&mut self, seconds: u64) {
        self.now = self.now.saturating_add(seconds);
    }
}

fn label_digest(label: &str) -> [u8; 32] {
    sha256(format!("pcoc-pack-2.2/{label}").as_bytes())
}

/// CASES.md natural header bytes at height `h`.
pub fn cases_natural(height: u64) -> [u8; 32] {
    if height == 1000 {
        return label_digest("beacon/seed");
    }
    if (1001..=1018).contains(&height) {
        let k = height - 1001;
        return label_digest(&format!("beacon/{k}"));
    }
    label_digest(&format!("index/filler/{height}"))
}

/// CASES.md compact index document for height `h`.
pub fn cases_document(height: u64) -> Vec<u8> {
    let natural = cases_natural(height);
    let display = hex_encode(&reverse32(&natural));
    let time = 1_700_000_000u64 + 600 * height;
    format!(
        "{{\"network\":\"synthetic\",\"header\":{{\"hash\":\"{display}\",\"height\":{height},\"time\":{time}}}}}"
    )
    .into_bytes()
}

/// CASES.md synthetic index. Serves heights 989 through 1018.
pub struct CasesIndex {
    docs: HashMap<u64, Vec<u8>>,
}

impl CasesIndex {
    pub fn new() -> Self {
        let mut docs = HashMap::new();
        for h in 989u64..=1018 {
            docs.insert(h, cases_document(h));
        }
        Self { docs }
    }
}

impl Default for CasesIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl HeightIndex for CasesIndex {
    fn read_height(&mut self, height: u64) -> Option<Vec<u8>> {
        self.docs.get(&height).cloned()
    }
}
