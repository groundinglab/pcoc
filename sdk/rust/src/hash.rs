//! SPEC §3. `H` is SHA-256 (FIPS 180-4), via the `sha2` crate.
//! On a host with SHA-NI the crate selects those instructions.

use sha2::{Digest, Sha256};

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
