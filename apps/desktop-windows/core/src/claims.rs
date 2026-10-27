//! Ask A1. No auto NIP-05 and no auto badge in v0.1.
//!
//! Claims happen when the user logs in manually elsewhere. This module has
//! no HTTP client and no request builder. Calling the omitted step fails closed.

/// Why the auto claims step does not run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimsOmitted {
    /// Ask A1: fail closed. Do not invent a claims POST.
    AskA1,
}

impl ClaimsOmitted {
    pub const SENTENCE: &'static str =
        "Auto NIP-05 and badge are omitted. Claims happen when the user logs in manually elsewhere.";
}

/// The only auto-claims entry. It refuses. It does not build a URL.
pub fn auto_claims() -> Result<(), ClaimsOmitted> {
    Err(ClaimsOmitted::AskA1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_claims_fails_closed_with_no_post() {
        let err = auto_claims().unwrap_err();
        assert_eq!(err, ClaimsOmitted::AskA1);
        assert!(ClaimsOmitted::SENTENCE.contains("omitted"));
        // The sentence names the product decision. It is not a request.
        assert!(!ClaimsOmitted::SENTENCE.contains("POST"));
        assert!(!ClaimsOmitted::SENTENCE.contains("https://"));
    }
}
