//! The hosts this app dials. Every URL is defined here.
//!
//! Admission, Bari, the portable index, and the public verifier. Sign-in is
//! the admission host plus the locked path.

/// groundinglab admission gate.
pub const ADMISSION: &str = "https://admission.groundinglab.io";
/// Live Bari. Produce posts waypoints here.
pub const BARI: &str = "https://bari.groundinglab.io";
/// Portable index. Verify reads stored headers from here.
pub const INDEX: &str = "https://index.groundinglab.io";
/// Public verifier. The desktop measures in-process and names this host.
pub const VERIFY: &str = "https://verify.groundinglab.io";

/// `POST` target for a sign-in. The link does not name this door.
pub const SIGN_IN: &str = "https://admission.groundinglab.io/v1/sign-in";

/// `GET` target for one sign-in result. `nonce_hex` is 64 lowercase hex digits.
pub fn sign_in_result(nonce_hex: &str) -> String {
    format!("{ADMISSION}/v1/sign-in/result/{nonce_hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_hosts_and_the_sign_in_path() {
        assert_eq!(ADMISSION, "https://admission.groundinglab.io");
        assert_eq!(BARI, "https://bari.groundinglab.io");
        assert_eq!(INDEX, "https://index.groundinglab.io");
        assert_eq!(VERIFY, "https://verify.groundinglab.io");
        assert_eq!(SIGN_IN, format!("{ADMISSION}/v1/sign-in"));
        assert!(SIGN_IN.starts_with(ADMISSION));
        let nonce = "ab".repeat(32);
        assert_eq!(
            sign_in_result(&nonce),
            format!("{ADMISSION}/v1/sign-in/result/{nonce}")
        );
    }
}
