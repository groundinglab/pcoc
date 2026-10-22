//! SPEC §3 and §11.1. `H` is SHA-256 (FIPS 180-4), via the `sha2` crate.
//! The crate selects SHA-NI at runtime when the host has it. The binary is
//! built for the baseline ISA; this probe is not a compile-time ISA choice.

use sha2::{Digest, Sha256};

/// Same runtime feature `sha2` 0.10 consults (`cpufeatures` feature `sha` on x86-64).
#[allow(unsafe_code, deprecated)]
mod sha_probe {
    // Same feature set as sha2 0.10.8 `sha256/x86.rs` (`shani_cpuid`).
    cpufeatures::new!(probe, "sha", "sse2", "ssse3", "sse4.1");

    pub fn sha_ni() -> bool {
        probe::get()
    }
}

/// SPEC §11.1. The SHA-256 backend this process is actually using.
/// `sha-ni` when the runtime probe is set, otherwise `software`.
pub fn sha_backend() -> &'static str {
    if sha_probe::sha_ni() {
        "sha-ni"
    } else {
        "software"
    }
}

/// SPEC §3. SHA-256 of `data`.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let dig = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&dig);
    out
}

/// SPEC §3. SHA-256 of the concatenation of the slices, in order.
pub fn sha256_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for p in parts {
        hasher.update(p);
    }
    let dig = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&dig);
    out
}

/// SPEC §3. SHA-256 of a byte stream that may be larger than a digest preimage.
pub fn sha256_reader(mut read: impl FnMut(&mut [u8]) -> Option<usize>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        match read(&mut buf) {
            Some(0) | None => break,
            Some(n) => hasher.update(&buf[..n]),
        }
    }
    let dig = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&dig);
    out
}

/// SPEC §3. SHA-256 over one contiguous buffer, updated in chunks.
pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    sha256(data)
}
