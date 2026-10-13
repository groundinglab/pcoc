//! Job identifiers.
//!
//! A job id is a bearer token for the result of someone's upload, so it must not
//! be a counter. One 32-byte seed is drawn from `/dev/urandom` at startup and
//! each id is `SHA-256(seed || counter)` truncated to 128 bits of hex.
//! If that read fails, the seed is SHA-256 of the clock and the process id.
//! That fallback is not the OS CSPRNG, and it is not the `audit-k` draw.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct IdGen {
    seed: [u8; 32],
    counter: AtomicU64,
}

impl IdGen {
    pub fn new() -> Self {
        Self {
            seed: os_seed(),
            counter: AtomicU64::new(0),
        }
    }

    pub fn next_id(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut hasher = Sha256::new();
        hasher.update(self.seed);
        hasher.update(n.to_le_bytes());
        hex::encode(&hasher.finalize()[..16])
    }
}

impl Default for IdGen {
    fn default() -> Self {
        Self::new()
    }
}

fn os_seed() -> [u8; 32] {
    // `/dev/urandom` never reaches EOF, so ask for exactly 32 bytes.
    let mut seed = [0u8; 32];
    match File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut seed)) {
        Ok(()) => seed,
        Err(_) => fallback_seed(),
    }
}

fn fallback_seed() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    hasher.update(std::process::id().to_le_bytes());
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&hasher.finalize());
    seed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct_and_hex() {
        let gen = IdGen::new();
        let a = gen.next_id();
        let b = gen.next_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn two_generators_do_not_collide() {
        assert_ne!(IdGen::new().next_id(), IdGen::new().next_id());
    }
}
