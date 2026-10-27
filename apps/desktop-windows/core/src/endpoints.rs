//! Hosts this producer dials. Every default URL is defined here.
//!
//! These match the Linux desktop (`apps/desktop/core/src/endpoints.rs`).
//! Lab mirrors are overrides at the call site. Defaults stay production
//! groundinglab. There is no claims host: Ask A1 omits auto NIP-05 / badge.

/// groundinglab admission gate.
pub const ADMISSION: &str = "https://admission.groundinglab.io";
/// Live Bari. Produce posts waypoints here.
pub const BARI: &str = "https://bari.groundinglab.io";
/// Portable index. Measurement may read stored headers from here.
pub const INDEX: &str = "https://index.groundinglab.io";
/// Public verifier. This core measures in-process and names this host.
pub const VERIFY: &str = "https://verify.groundinglab.io";

/// `POST` target for a sign-in. The link does not name this door.
pub const SIGN_IN: &str = "https://admission.groundinglab.io/v1/sign-in";

/// The only signature origin, WebAuthn spelling. Locked with the Linux app.
pub const SIGN_IN_ORIGIN: &str = "https://groundinglab.io";

/// Linux desktop names this result URL. The Windows sign-in module does not
/// call it. The helper exists so a test can tell that URL from the POST door.
#[cfg(test)]
pub(crate) fn sign_in_result_url(nonce_hex: &str) -> String {
    format!(
        "{ADMISSION}{}{nonce_hex}",
        concat!("/v1/sign-in/", "result/")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_match_the_linux_desktop_defaults() {
        assert_eq!(ADMISSION, "https://admission.groundinglab.io");
        assert_eq!(BARI, "https://bari.groundinglab.io");
        assert_eq!(INDEX, "https://index.groundinglab.io");
        assert_eq!(VERIFY, "https://verify.groundinglab.io");
        assert_eq!(SIGN_IN, "https://admission.groundinglab.io/v1/sign-in");
        assert_eq!(SIGN_IN, format!("{ADMISSION}/v1/sign-in"));
        assert_eq!(SIGN_IN_ORIGIN, "https://groundinglab.io");
        assert!(SIGN_IN.starts_with("https://"));
        assert!(!ADMISSION.contains("claims"));
        assert!(!BARI.contains("claims"));
    }
}
