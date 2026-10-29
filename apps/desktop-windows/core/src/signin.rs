//! Sign-in door constants for the Windows producer.
//!
//! W1 locks the origin and the POST target. The client under test does not
//! read a sign-in result URL. It builds one submission, posts it once, and
//! classifies that response. It does not poll.

use crate::endpoints::{SIGN_IN, SIGN_IN_ORIGIN};
use crate::keys::{PublicKey, SecretKey};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::time::Duration;

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

/// 19 ASCII bytes, no trailing NUL in the tag.
pub const DOMAIN: &str = "pcoc-gate-signin-v1";

pub const SUBMITTED: &str = "Sign-in submitted";
pub const BAD_SIGNATURE: &str = "bad signature";
pub const MALFORMED: &str = "malformed";

const LINK_PREFIX: &str = "https://groundinglab.io/sign-in/?v=1&nonce=";
const RETIRED_PREFIX: &str = "https://groundinglab.io/sign-in/?h=";

/// One parsed challenge link. The website mints it. This module does not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    pub nonce: [u8; 32],
    pub expiry_unix_s: u64,
}

/// A challenge bound to the object this computer chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundChallenge {
    pub nonce: [u8; 32],
    pub expiry_unix_s: u64,
    pub object_sha256: [u8; 32],
}

/// Why a paste is not a link. The client stops before any POST.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotALink {
    /// `https://groundinglab.io/sign-in/?h=` plus hex: the retired form.
    RetiredHashLink,
    Other,
}

impl NotALink {
    pub fn sentence(self) -> &'static str {
        match self {
            NotALink::RetiredHashLink => {
                "Fail. This is the retired ?h= link. It is not signed. Ask the sign-in page for a new link."
            }
            NotALink::Other => {
                "Fail. This is not a sign-in link. Paste the link from the sign-in page."
            }
        }
    }
}

/// Section 15.1 paste rule: trim leading and trailing space, tab, CR and LF,
/// then the remainder must equal the canonical link.
pub fn parse_link(input: &str) -> Result<Challenge, NotALink> {
    let text = input.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    if text.starts_with(RETIRED_PREFIX) {
        return Err(NotALink::RetiredHashLink);
    }
    let rest = text.strip_prefix(LINK_PREFIX).ok_or(NotALink::Other)?;
    let (nonce_hex, rest) = split_at_checked(rest, 64).ok_or(NotALink::Other)?;
    let nonce = crate::hex::decode32_lower(nonce_hex).ok_or(NotALink::Other)?;
    let rest = rest.strip_prefix("&exp=").ok_or(NotALink::Other)?;
    if rest.contains('&') || rest.contains('#') || rest.contains('?') {
        return Err(NotALink::Other);
    }
    let expiry_unix_s = shortest_u64(rest).ok_or(NotALink::Other)?;
    Ok(Challenge {
        nonce,
        expiry_unix_s,
    })
}

fn split_at_checked(text: &str, at: usize) -> Option<(&str, &str)> {
    if text.len() < at || !text.is_char_boundary(at) {
        return None;
    }
    Some(text.split_at(at))
}

/// Shortest decimal: digits only, no sign, no leading zero except `0`, in `u64`.
fn shortest_u64(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    text.parse().ok()
}

impl Challenge {
    /// Bind the SHA-256 of the hike object this computer chose.
    pub fn bind(&self, object_sha256: [u8; 32]) -> BoundChallenge {
        BoundChallenge {
            nonce: self.nonce,
            expiry_unix_s: self.expiry_unix_s,
            object_sha256,
        }
    }
}

impl BoundChallenge {
    /// `domain ‖ 0x00 ‖ uint8(n) ‖ origin ‖ nonce ‖ uint64_be(exp) ‖ object`.
    pub fn challenge_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(93 + ORIGIN.len());
        out.extend_from_slice(DOMAIN.as_bytes());
        out.push(0x00);
        out.push(ORIGIN.len() as u8);
        out.extend_from_slice(ORIGIN.as_bytes());
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&self.expiry_unix_s.to_be_bytes());
        out.extend_from_slice(&self.object_sha256);
        out
    }

    /// `bip340_message_32 = SHA-256(challenge_bytes)`. The signer does not hash it again.
    pub fn bip340_message(&self) -> [u8; 32] {
        Sha256::digest(self.challenge_bytes()).into()
    }
}

/// Why a parsed link is not signed. Local stops, not gate tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CannotSign {
    /// The private key is not the object's public key.
    OtherKey,
}

impl CannotSign {
    pub fn sentence(&self) -> &'static str {
        match self {
            CannotSign::OtherKey => {
                "Fail. This nsec does not match the PCOC's public key. Nothing was signed."
            }
        }
    }
}

/// A signed submission, ready for the one POST.
#[derive(Debug)]
pub struct Submission {
    pub body: String,
    pub signature: [u8; 64],
}

/// Sign the challenge for `object_bytes` under `secret`.
///
/// `object_public_key` is that object's public key. `aux` is fresh randomness
/// in the app and fixed in tests.
pub fn sign(
    challenge: &Challenge,
    object_bytes: &[u8],
    object_public_key: PublicKey,
    secret: &SecretKey,
    aux: &[u8; 32],
) -> Result<Submission, CannotSign> {
    if secret.public_key() != object_public_key {
        return Err(CannotSign::OtherKey);
    }
    let bound = challenge.bind(Sha256::digest(object_bytes).into());
    let signature = secret.sign_digest(&bound.bip340_message(), aux);
    Ok(Submission {
        body: body_json(&bound, object_bytes, &signature),
        signature,
    })
}

/// The six-key body, in the section 15.1 order. Lowercase hex, standard
/// base64 with padding, `exp` a JSON integer.
pub fn body_json(bound: &BoundChallenge, object_bytes: &[u8], signature: &[u8; 64]) -> String {
    format!(
        "{{\"origin\":\"{ORIGIN}\",\"nonce\":\"{}\",\"exp\":{},\"object\":\"{}\",\"object_b64\":\"{}\",\"signature\":\"{}\"}}",
        crate::hex::encode(&bound.nonce),
        bound.expiry_unix_s,
        crate::hex::encode(&bound.object_sha256),
        base64::engine::general_purpose::STANDARD.encode(object_bytes),
        crate::hex::encode(signature)
    )
}

/// The client class (section 15.1, client response).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientClass {
    /// Any completed response other than a redirect and the two HTTP 400 bodies.
    Submitted,
    BadSignature,
    Malformed,
    /// A redirect, another HTTP 400, or no response. Not delivery OK and not a gate outcome.
    NotDelivered,
}

impl ClientClass {
    /// The sign-in sentence. `None` when the client shows no sign-in sentence.
    pub fn sentence(self) -> Option<&'static str> {
        match self {
            ClientClass::Submitted => Some(SUBMITTED),
            ClientClass::BadSignature => Some(BAD_SIGNATURE),
            ClientClass::Malformed => Some(MALFORMED),
            ClientClass::NotDelivered => None,
        }
    }
}

/// Classify one completed HTTP response. The body is read only for HTTP 400.
pub fn classify(status: u16, body: &[u8]) -> ClientClass {
    match status {
        300..=399 => ClientClass::NotDelivered,
        400 => {
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
                return ClientClass::NotDelivered;
            };
            if value == serde_json::json!({"error": {"code": "bad_signature"}}) {
                ClientClass::BadSignature
            } else if value == serde_json::json!({"error": {"code": "malformed"}}) {
                ClientClass::Malformed
            } else {
                ClientClass::NotDelivered
            }
        }
        _ => ClientClass::Submitted,
    }
}

/// Where the one POST goes. The app uses [`DOOR`]; tests point at a local listener.
pub fn post_once(door: &str, body: &str) -> ClientClass {
    if is_result_route(door) {
        return ClientClass::NotDelivered;
    }
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(30))
        .user_agent("pcoc-windows")
        .build();
    let response = agent
        .post(door)
        .set("Content-Type", "application/json")
        .send_string(body);
    let (status, response) = match response {
        Ok(response) => (response.status(), response),
        Err(ureq::Error::Status(status, response)) => (status, response),
        Err(ureq::Error::Transport(_)) => return ClientClass::NotDelivered,
    };
    let mut bytes = Vec::new();
    if status == 400 {
        use std::io::Read;
        let _ = response.into_reader().take(4096).read_to_end(&mut bytes);
    }
    classify(status, &bytes)
}

/// The app's submit: the fixed door, once.
pub fn submit(submission: &Submission) -> ClientClass {
    post_once(DOOR, &submission.body)
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

    #[test]
    fn client_classes_match_the_linux_sentences() {
        let bad = br#"{"error":{"code":"bad_signature"}}"#;
        let mal = br#"{"error":{"code":"malformed"}}"#;
        assert_eq!(
            classify(200, br#"{"delivery":"ok"}"#),
            ClientClass::Submitted
        );
        assert_eq!(classify(400, bad), ClientClass::BadSignature);
        assert_eq!(classify(400, mal), ClientClass::Malformed);
        assert_eq!(classify(400, b"not json"), ClientClass::NotDelivered);
        assert_eq!(classify(302, b""), ClientClass::NotDelivered);
        assert_eq!(ClientClass::Submitted.sentence(), Some(SUBMITTED));
        assert_eq!(ClientClass::BadSignature.sentence(), Some(BAD_SIGNATURE));
        assert_eq!(ClientClass::Malformed.sentence(), Some(MALFORMED));
        assert_eq!(ClientClass::NotDelivered.sentence(), None);
    }

    #[test]
    fn one_post_sends_the_locked_origin_and_does_not_follow_redirects() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::{Duration, Instant};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let door = format!("http://127.0.0.1:{port}/v1/sign-in");
        let server = std::thread::spawn(move || {
            let start = Instant::now();
            let mut sock = loop {
                match listener.accept() {
                    Ok((sock, _)) => break sock,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        if start.elapsed() > Duration::from_secs(10) {
                            return "timeout".to_owned();
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => return format!("accept {err}"),
                }
            };
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 2048];
            loop {
                match sock.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let req = String::from_utf8_lossy(&buf).to_string();
            let body = b"{\"delivery\":\"ok\"}";
            let ok = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = sock.write_all(ok.as_bytes());
            let _ = sock.write_all(body);
            req
        });

        let secret = SecretKey::generate();
        let link = format!(
            "https://groundinglab.io/sign-in/?v=1&nonce={}&exp=1893456000",
            "ab".repeat(32)
        );
        let challenge = parse_link(&link).unwrap();
        let object = b"object-bytes";
        let submission =
            sign(&challenge, object, secret.public_key(), &secret, &[3u8; 32]).unwrap();
        assert!(!submission.body.contains("nsec"));
        assert!(submission
            .body
            .contains("\"origin\":\"https://groundinglab.io\""));
        let class = post_once(&door, &submission.body);
        assert_eq!(class, ClientClass::Submitted);
        let req = server.join().unwrap();
        assert_eq!(req.matches("POST ").count(), 1, "{req}");
        assert!(req.starts_with("POST /v1/sign-in HTTP/1.1"), "{req}");
        assert!(
            req.contains("\"origin\":\"https://groundinglab.io\""),
            "{req}"
        );
        let marker = concat!("/v1/sign-in/", "result");
        assert!(!req.contains(marker), "{req}");
        assert_eq!(
            parse_link("https://groundinglab.io/sign-in/?h=ab"),
            Err(NotALink::RetiredHashLink)
        );
        let other = SecretKey::generate();
        assert_eq!(
            sign(&challenge, object, other.public_key(), &secret, &[3u8; 32]).unwrap_err(),
            CannotSign::OtherKey
        );
    }
}
