//! Sign-in door constants for the Windows producer.
//!
//! W1 locks the origin and the POST target. The client under test does not
//! read a sign-in result URL. Building a submission and classifying a gate
//! body belong to a later wave; this module only names the door.

use crate::endpoints::{SIGN_IN, SIGN_IN_ORIGIN};

/// Signature origin. Same spelling the Linux desktop posts.
pub const ORIGIN: &str = SIGN_IN_ORIGIN;

/// `POST` target. Same door the Linux desktop posts to.
pub const DOOR: &str = SIGN_IN;

/// Whether `url` is the sign-in result URL. The Windows client must not
/// request that route. Callers use this to refuse one.
pub fn is_result_route(url: &str) -> bool {
    const MARKER: &str = concat!("/v1/sign-in/", "result/");
    url.contains(MARKER)
}

/// The POST the client is allowed to send. A result-route URL is refused.
pub fn sign_in_post_target() -> &'static str {
    DOOR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_and_door_are_locked() {
        assert_eq!(ORIGIN, "https://groundinglab.io");
        assert_eq!(DOOR, "https://admission.groundinglab.io/v1/sign-in");
        assert_eq!(sign_in_post_target(), DOOR);
        assert!(!is_result_route(DOOR));
        assert!(!is_result_route(ORIGIN));
    }

    #[test]
    fn client_source_does_not_poll_the_result_route() {
        let source = include_str!("signin.rs");
        let client = source.split("#[cfg(test)]").next().unwrap();
        let marker = concat!("/v1/sign-in/", "result");
        assert!(
            !client.contains(marker),
            "sign-in client must not name the result route"
        );
        assert!(!client.contains("sign_in_result"));
        let sample = crate::endpoints::sign_in_result_url(&"ab".repeat(32));
        assert!(is_result_route(&sample));
        assert_ne!(sample, DOOR);
        assert_eq!(sign_in_post_target(), DOOR);
    }
}
