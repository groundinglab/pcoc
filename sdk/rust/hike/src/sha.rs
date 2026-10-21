use sha2::{Digest, Sha256};

use crate::consts::HASH_BYTES;

pub fn sha256(data: &[u8]) -> [u8; HASH_BYTES] {
    let digest = Sha256::digest(data);
    let mut out = [0u8; HASH_BYTES];
    out.copy_from_slice(&digest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_abc() {
        let digest = sha256(b"abc");
        assert_eq!(
            digest,
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    /// The walk binary contains the hardware SHA op this host can execute.
    /// x86_64: SHA-NI `sha256rnds2`. aarch64 (`sha2` feature `asm`): `sha256h`.
    #[test]
    fn walk_binary_contains_hardware_sha() {
        let exe = std::fs::read("/proc/self/exe").expect("read own executable");
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        {
            let sha_ni = exe.windows(3).any(|window| window == [0x0f, 0x38, 0xcb]);
            assert!(sha_ni, "SHA-NI sha256rnds2 is not linked into the walk");
            assert!(
                std::is_x86_feature_detected!("sha"),
                "this host has no SHA-NI, so the hardware path would not run"
            );
        }
        #[cfg(target_arch = "aarch64")]
        {
            let sha256h = exe.windows(4).any(|window| {
                let insn = u32::from_le_bytes(window.try_into().unwrap());
                // SHA256H Qd, Qn, Vm.4S. Register fields are masked off.
                (insn & 0xFFE0_FC00) == 0x5E00_5800
            });
            assert!(sha256h, "sha256h is not linked into the walk");
        }
    }
}
