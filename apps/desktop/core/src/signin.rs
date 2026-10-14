//! The section 15.1 sign-in wire, locked 2026-10-10.
//!
//! Paste only. The client shows the origin, signs `SHA-256(challenge_bytes)`
//! once with BIP-340, POSTs once to the fixed door, and shows one of three
//! sentences. It does not poll, does not read the result route, does not
//! open `/welcome/`, and does not enforce the challenge window.

use crate::keys::{PublicKey, SecretKey};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::time::Duration;

/// The only signature origin, WebAuthn spelling.
pub const ORIGIN: &str = "https://groundinglab.io";
/// 19 ASCII bytes, no trailing NUL in the tag.
pub const DOMAIN: &str = "pcoc-gate-signin-v1";
/// The door. The link does not name it. Defined with the other endpoints.
pub use crate::endpoints::SIGN_IN as DOOR;
pub const LIFETIME_S: u64 = 120;
pub const SKEW_S: u64 = 5;

pub const SUBMITTED: &str = "Sign-in submitted";
pub const BAD_SIGNATURE: &str = "bad signature";
pub const MALFORMED: &str = "malformed";

const LINK_PREFIX: &str = "https://groundinglab.io/sign-in/?v=1&nonce=";
const RETIRED_PREFIX: &str = "https://groundinglab.io/sign-in/?h=";

/// One parsed challenge link. The link carries a nonce and an expiry.
/// The client binds `object_sha256` from the hike object it chooses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    pub nonce: [u8; 32],
    pub expiry_unix_s: u64,
}

/// A challenge bound to the object the client chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundChallenge {
    pub nonce: [u8; 32],
    pub expiry_unix_s: u64,
    pub object_sha256: [u8; 32],
}

/// Why a paste is not a link. The client stops before any POST.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotALink {
    /// `https://groundinglab.io/sign-in/?h=` plus 64 hex: the retired form.
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
    pub fn link(&self) -> String {
        format!(
            "{LINK_PREFIX}{}&exp={}",
            crate::hex::encode(&self.nonce),
            self.expiry_unix_s
        )
    }

    /// The link broken on its query parameters, for reading.
    pub fn link_lines(&self) -> String {
        format!(
            "https://groundinglab.io/sign-in/?v=1\n&nonce={}\n&exp={}",
            crate::hex::encode(&self.nonce),
            self.expiry_unix_s
        )
    }

    /// Bind the SHA-256 of the hike object this computer chose.
    pub fn bind(&self, object_sha256: [u8; 32]) -> BoundChallenge {
        BoundChallenge {
            nonce: self.nonce,
            expiry_unix_s: self.expiry_unix_s,
            object_sha256,
        }
    }

    /// Seconds until `expiry_unix_s`, negative once passed. Display only:
    /// the signing client does not enforce the window.
    pub fn seconds_left(&self, now_unix_s: u64) -> i128 {
        self.expiry_unix_s as i128 - now_unix_s as i128
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

/// Why a parsed link is not signed. Local stops, not Gate tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CannotSign {
    /// No local object has the link's SHA-256.
    ObjectNotLocal,
    /// The private key is not in memory.
    Locked,
    /// The object's `public_key` is not the unlocked key.
    OtherKey,
}

impl CannotSign {
    pub fn sentence(&self) -> &'static str {
        match self {
            CannotSign::ObjectNotLocal => "Fail. Choose a PCOC. Nothing was signed.",
            CannotSign::Locked => "Fail. Paste an nsec. Nothing was signed.",
            CannotSign::OtherKey => {
                "Fail. This nsec does not match the PCOC's public key. Nothing was signed."
            }
        }
    }
}

/// A signed submission, ready for the one POST.
pub struct Submission {
    pub body: String,
    pub signature: [u8; 64],
}

/// Sign the challenge for the object this computer chose, under `secret`.
///
/// `object_sha256` in `challenge_bytes` and in the body is SHA-256 of
/// `object_bytes`. The link does not name that object. `object_public_key`
/// is that object's `public_key`. `aux` is fresh randomness in the app and
/// fixed in tests.
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

/// The client class (section 15.1, Client response).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientClass {
    /// Any completed response other than a redirect and the two HTTP 400 bodies.
    Submitted,
    BadSignature,
    Malformed,
    /// A redirect, another HTTP 400, or no response. Not delivery OK and not a Gate outcome.
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

/// Where the one POST goes. The app always uses [`DOOR`]; tests point at a
/// local listener.
pub fn post_once(door: &str, body: &str) -> ClientClass {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(30))
        .user_agent("pcoc-hike-desktop")
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

/// A challenge this app mints for test admission. The website link is the
/// same shape. Expiry is 60 seconds ahead, inside the gate's 120 second window.
pub fn mint_challenge(now_unix_s: u64) -> Challenge {
    let mut nonce = [0u8; 32];
    getrandom::getrandom(&mut nonce).expect("OS randomness");
    Challenge {
        nonce,
        expiry_unix_s: now_unix_s.saturating_add(60),
    }
}

/// Plain words for a finished sign-in. `outcome` is the gate's result token
/// after a delivery-OK post. `None` means the result was not read.
pub fn admission_words(class: ClientClass, outcome: Option<&str>) -> String {
    match class {
        ClientClass::NotDelivered => "Fail. The gate did not answer.".to_owned(),
        ClientClass::BadSignature => "Fail. The gate rejected the signature.".to_owned(),
        ClientClass::Malformed => "Fail. The gate called this sign-in malformed.".to_owned(),
        ClientClass::Submitted => match outcome.unwrap_or("pending") {
            "accepted" => "Pass. The gate accepted this PCOC.".to_owned(),
            "pending" => "Waiting. The gate has not decided yet.".to_owned(),
            "rejected" => "Fail. The gate rejected this PCOC.".to_owned(),
            "malformed" => "Fail. The gate called this sign-in malformed.".to_owned(),
            "bad_signature" => "Fail. The gate rejected the signature.".to_owned(),
            "too_short" => "Fail. The gate said this object is too short.".to_owned(),
            "earlier_version" => {
                "Fail. This PCOC is an earlier version. The gate checks Satoshi Hike v2.0 only."
                    .to_owned()
            }
            "too_few_waypoints_anchored" => {
                "Fail. The gate said too few waypoints are anchored.".to_owned()
            }
            other => format!("Fail. The gate said {other}."),
        },
    }
}

/// Read `outcome` from a sign-in result URL. `None` when the gate does not answer.
pub fn fetch_outcome(url: &str) -> Option<String> {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(15))
        .user_agent("pcoc-hike-desktop")
        .build();
    let response = match agent.get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(_, _)) | Err(ureq::Error::Transport(_)) => return None,
    };
    if response.status() != 200 {
        return None;
    }
    let mut bytes = Vec::new();
    use std::io::Read;
    if response
        .into_reader()
        .take(8192)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value
        .get("outcome")
        .and_then(|item| item.as_str())
        .map(str::to_owned)
}

/// Post once, then read the result until it leaves `pending`.
pub fn submit_and_read(submission: &Submission, nonce: &[u8; 32]) -> String {
    read_admission(
        DOOR,
        &crate::endpoints::sign_in_result(&crate::hex::encode(nonce)),
        submission,
    )
}

/// Post `submission` to `door`, then poll `result_url`. Tests point both at a local listener.
pub fn read_admission(door: &str, result_url: &str, submission: &Submission) -> String {
    let class = post_once(door, &submission.body);
    if class != ClientClass::Submitted {
        return admission_words(class, None);
    }
    for attempt in 0..8 {
        if let Some(outcome) = fetch_outcome(result_url) {
            if outcome != "pending" {
                return admission_words(ClientClass::Submitted, Some(&outcome));
            }
        }
        if attempt + 1 == 8 {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    admission_words(ClientClass::Submitted, Some("pending"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::{schnorr, Message, Secp256k1, XOnlyPublicKey};
    use serde_json::Value;

    fn vector() -> Value {
        serde_json::from_str(include_str!("../../../../spec/conformance/v2.0/sign-in.json")).unwrap()
    }

    fn hex32(value: &Value) -> [u8; 32] {
        crate::hex::decode32(value.as_str().unwrap()).unwrap()
    }

    #[test]
    fn locked_constants_match_the_vector() {
        let v = vector();
        assert_eq!(v["domain"], DOMAIN);
        assert_eq!(v["origin"], ORIGIN);
        assert_eq!(v["door"], DOOR);
        assert_eq!(v["lifetime_s"], LIFETIME_S);
        assert_eq!(v["skew_s"], SKEW_S);
        assert_eq!(v["method"], "POST");
        assert_eq!(v["content_type"], "application/json");
        assert_eq!(v["client_ok"], SUBMITTED);
        let codes: Vec<&str> = v["client_codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        assert_eq!(codes, ["bad_signature", "malformed"]);
        assert_eq!(BAD_SIGNATURE, "bad_signature".replace('_', " "));
    }

    #[test]
    fn every_link_case_in_the_vector() {
        let v = vector();
        for case in v["links"].as_array().unwrap() {
            let input = case["input"].as_str().unwrap();
            let parsed = parse_link(input);
            match case["class"].as_str().unwrap() {
                "link" => assert!(parsed.is_ok(), "{}: {input}", case["note"]),
                "malformed" => assert!(parsed.is_err(), "{}: {input}", case["note"]),
                other => panic!("unknown class {other}"),
            }
        }
        let retired = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["note"] == "retired ?h= form")
            .unwrap();
        assert_eq!(
            parse_link(retired["input"].as_str().unwrap()),
            Err(NotALink::RetiredHashLink)
        );
    }

    #[test]
    fn example_link_round_trips_and_builds_challenge_bytes() {
        let v = vector();
        let ex = &v["example"];
        let challenge = parse_link(ex["link"].as_str().unwrap()).unwrap();
        assert_eq!(challenge.nonce, hex32(&ex["nonce_hex"]));
        assert_eq!(
            challenge.expiry_unix_s,
            ex["expiry_unix_s"].as_u64().unwrap()
        );
        assert!(!challenge.link().contains("object="));
        assert_eq!(challenge.link(), ex["link"].as_str().unwrap());
        let bound = challenge.bind(hex32(&ex["object_sha256_hex"]));

        let bytes = bound.challenge_bytes();
        assert_eq!(
            bytes.len() as u64,
            ex["challenge_bytes_len"].as_u64().unwrap()
        );
        assert_eq!(
            crate::hex::encode(&bytes),
            ex["challenge_bytes_hex"].as_str().unwrap()
        );
        assert_eq!(bound.bip340_message(), hex32(&ex["bip340_message_32_hex"]));
        let double: [u8; 32] = Sha256::digest(bound.bip340_message()).into();
        assert_eq!(double, hex32(&ex["double_hash_hex"]));
    }

    /// The link minted by https://groundinglab.io/sign-in/ on 2026-10-12.
    /// Headless Chrome pressed Sign in with PCOC and read `#sign-in-payload`.
    #[test]
    fn live_minted_link_parses() {
        let link = "https://groundinglab.io/sign-in/?v=1&nonce=c939a2a1747ec2c7b06c03e6ee00019c289e1c45f8595d1e3af82e29c9e26f6b&exp=1791778962";
        let challenge = parse_link(link).expect("live minted link");
        assert_eq!(challenge.link(), link);
        assert_eq!(challenge.expiry_unix_s, 1_791_778_962);
        assert_eq!(
            crate::hex::encode(&challenge.nonce),
            "c939a2a1747ec2c7b06c03e6ee00019c289e1c45f8595d1e3af82e29c9e26f6b"
        );
        assert!(challenge
            .link()
            .starts_with("https://groundinglab.io/sign-in/?v=1"));
        assert!(!challenge.link().contains("object="));
    }

    #[test]
    fn exp_zero_is_the_one_leading_zero_exception() {
        let v = vector();
        let case = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["note"] == "exp 0 is the one leading-zero exception")
            .unwrap();
        assert_eq!(
            parse_link(case["input"].as_str().unwrap())
                .unwrap()
                .expiry_unix_s,
            0
        );
    }

    #[test]
    fn signature_matches_the_vector_and_signs_the_digest_once() {
        let v = vector();
        let ex = &v["example"];
        let challenge = parse_link(ex["link"].as_str().unwrap())
            .unwrap()
            .bind(hex32(&ex["object_sha256_hex"]));
        let secret_bytes = crate::hex::decode32(ex["secret_hex"].as_str().unwrap()).unwrap();
        let nsec = bech32::encode(
            "nsec",
            bech32::ToBase32::to_base32(&secret_bytes),
            bech32::Variant::Bech32,
        )
        .unwrap();
        let secret = SecretKey::from_nsec(&nsec).unwrap();
        assert_eq!(
            secret.public_key().hex(),
            ex["public_key_hex"].as_str().unwrap()
        );

        let aux = hex32(&ex["aux_hex"]);
        let sig = secret.sign_digest(&challenge.bip340_message(), &aux);
        assert_eq!(
            crate::hex::encode(&sig),
            ex["signature_hex"].as_str().unwrap()
        );

        let secp = Secp256k1::verification_only();
        let xonly = XOnlyPublicKey::from_slice(&secret.public_key().0).unwrap();
        let sig = schnorr::Signature::from_slice(&sig).unwrap();
        assert!(secp
            .verify_schnorr(
                &sig,
                &Message::from_digest(challenge.bip340_message()),
                &xonly
            )
            .is_ok());
        let double: [u8; 32] = Sha256::digest(challenge.bip340_message()).into();
        assert!(secp
            .verify_schnorr(&sig, &Message::from_digest(double), &xonly)
            .is_err());
    }

    #[test]
    fn body_has_the_six_keys_in_order_and_matches_the_grammar_body() {
        let v = vector();
        let ex = &v["example"];
        let challenge = parse_link(ex["link"].as_str().unwrap())
            .unwrap()
            .bind(hex32(&ex["object_sha256_hex"]));
        let object = base64::engine::general_purpose::STANDARD
            .decode(ex["object_b64"].as_str().unwrap())
            .unwrap();
        let sig: [u8; 64] = crate::hex::decode(ex["signature_hex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let body = body_json(&challenge, &object, &sig);
        let parsed: Value = serde_json::from_str(&body).unwrap();
        let grammar_ok = v["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["class"] == "grammar_ok")
            .unwrap();
        assert_eq!(parsed, grammar_ok["body"]);
        let keys: Vec<&str> = [
            "origin",
            "nonce",
            "exp",
            "object",
            "object_b64",
            "signature",
        ]
        .to_vec();
        let mut at = 0;
        for key in keys {
            let found = body[at..].find(&format!("\"{key}\":")).expect(key) + at;
            at = found;
        }
        assert!(parsed["exp"].is_u64());
    }

    #[test]
    fn sign_binds_the_sha256_of_the_object_this_computer_chose() {
        let v = vector();
        let ex = &v["example"];
        // The example field is 32 bytes of 0xaa. It is not the hash of those bytes.
        assert_eq!(ex["hash_matches_object_field"], false);
        let challenge = parse_link(ex["link"].as_str().unwrap()).unwrap();
        let secret = SecretKey::generate();
        let object = b"a local object".to_vec();
        let other = SecretKey::generate().public_key();
        assert_eq!(
            sign(&challenge, &object, other, &secret, &[0; 32]).err(),
            Some(CannotSign::OtherKey)
        );
        let submission = sign(&challenge, &object, secret.public_key(), &secret, &[0; 32])
            .ok()
            .unwrap();
        let parsed: Value = serde_json::from_str(&submission.body).unwrap();
        let object_sha = crate::hex::encode(Sha256::digest(&object).as_slice());
        assert_eq!(parsed["object"].as_str().unwrap(), object_sha);
        assert_ne!(
            parsed["object"].as_str().unwrap(),
            ex["object_sha256_hex"].as_str().unwrap()
        );
        assert!(submission
            .body
            .contains(&crate::hex::encode(&submission.signature)));
        assert!(CannotSign::ObjectNotLocal.sentence().starts_with("Fail."));
        assert!(CannotSign::Locked.sentence().contains("nsec"));
        assert!(CannotSign::OtherKey.sentence().contains("public key"));
    }

    #[test]
    fn client_classes() {
        let v = vector();
        let ok = serde_json::to_vec(&v["responses"]["delivery_ok"]).unwrap();
        let bad = serde_json::to_vec(&v["responses"]["bad_signature"]).unwrap();
        let mal = serde_json::to_vec(&v["responses"]["malformed"]).unwrap();
        assert_eq!(classify(200, &ok), ClientClass::Submitted);
        assert_eq!(classify(400, &bad), ClientClass::BadSignature);
        assert_eq!(classify(400, &mal), ClientClass::Malformed);
        assert_eq!(
            classify(
                400,
                b"{\"error\":{\"code\":\"malformed\",\"message\":\"x\"}}"
            ),
            ClientClass::NotDelivered
        );
        assert_eq!(classify(400, b"not json"), ClientClass::NotDelivered);
        for status in [200, 202, 404, 413, 415, 429, 500, 503] {
            assert_eq!(
                classify(status, b"{\"decision\":\"deny\"}"),
                ClientClass::Submitted,
                "{status}"
            );
        }
        for status in [301, 302, 303, 307, 308] {
            assert_eq!(classify(status, b""), ClientClass::NotDelivered);
        }
        assert_eq!(ClientClass::Submitted.sentence(), Some("Sign-in submitted"));
        assert_eq!(ClientClass::BadSignature.sentence(), Some("bad signature"));
        assert_eq!(ClientClass::Malformed.sentence(), Some("malformed"));
        assert_eq!(ClientClass::NotDelivered.sentence(), None);
    }

    /// One local listener, one request. The client POSTs once and does not follow a redirect.
    #[test]
    fn post_once_sends_one_json_post_and_does_not_follow_redirects() {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;

        fn serve(
            reply: &'static str,
        ) -> (String, std::thread::JoinHandle<(String, String, usize)>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let handle = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let mut length = 0usize;
                let mut content_type = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    if let Some(v) = lower.strip_prefix("content-type:") {
                        content_type = v.trim().to_owned();
                    }
                }
                let mut body = vec![0u8; length];
                reader.read_exact(&mut body).unwrap();
                let mut stream = stream;
                stream.write_all(reply.as_bytes()).unwrap();
                stream.flush().unwrap();
                // A second request would block here; count what arrives.
                listener.set_nonblocking(true).unwrap();
                std::thread::sleep(Duration::from_millis(200));
                let extra = usize::from(listener.accept().is_ok());
                (
                    format!("{} {content_type}", request_line.trim()),
                    String::from_utf8(body).unwrap(),
                    extra,
                )
            });
            (format!("http://{addr}/v1/sign-in"), handle)
        }

        let (url, handle) = serve("HTTP/1.1 400 Bad Request\r\nContent-Length: 34\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"bad_signature\"}}");
        assert_eq!(post_once(&url, "{\"k\":1}"), ClientClass::BadSignature);
        let (line, body, extra) = handle.join().unwrap();
        assert_eq!(line, "POST /v1/sign-in HTTP/1.1 application/json");
        assert_eq!(body, "{\"k\":1}");
        assert_eq!(extra, 0);

        let (url, handle) = serve("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:9/x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        assert_eq!(post_once(&url, "{}"), ClientClass::NotDelivered);
        assert_eq!(handle.join().unwrap().2, 0);

        let (url, handle) = serve("HTTP/1.1 200 OK\r\nContent-Length: 17\r\nConnection: close\r\n\r\n{\"delivery\":\"ok\"}");
        assert_eq!(post_once(&url, "{}"), ClientClass::Submitted);
        handle.join().unwrap();
    }

    #[test]
    fn minted_challenge_is_a_sign_in_link() {
        let now = 1_700_000_000;
        let challenge = mint_challenge(now);
        assert_eq!(challenge.expiry_unix_s, now + 60);
        assert_eq!(parse_link(&challenge.link()).unwrap(), challenge);
        assert!(!challenge.link().contains("object="));
    }

    #[test]
    fn admission_words_are_pass_or_fail() {
        assert_eq!(
            admission_words(ClientClass::Submitted, Some("accepted")),
            "Pass. The gate accepted this PCOC."
        );
        assert_eq!(
            admission_words(ClientClass::Submitted, Some("rejected")),
            "Fail. The gate rejected this PCOC."
        );
        assert_eq!(
            admission_words(ClientClass::Submitted, Some("pending")),
            "Waiting. The gate has not decided yet."
        );
        assert_eq!(
            admission_words(ClientClass::BadSignature, None),
            "Fail. The gate rejected the signature."
        );
        assert_eq!(
            admission_words(ClientClass::NotDelivered, None),
            "Fail. The gate did not answer."
        );
        assert!(admission_words(ClientClass::Submitted, Some("nope")).starts_with("Fail."));
    }

    #[test]
    fn read_admission_reports_a_pass_from_the_result() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let door = format!("http://{addr}/v1/sign-in");
        let result = format!("http://{addr}/v1/sign-in/result/ab");
        let handle = std::thread::spawn(move || {
            for reply in [
                "HTTP/1.1 200 OK\r\nContent-Length: 17\r\nConnection: close\r\n\r\n{\"delivery\":\"ok\"}",
                "HTTP/1.1 200 OK\r\nContent-Length: 22\r\nConnection: close\r\n\r\n{\"outcome\":\"accepted\"}",
            ] {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header == "\r\n" {
                        break;
                    }
                }
                let mut stream = stream;
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        let words = read_admission(
            &door,
            &result,
            &Submission {
                body: "{}".into(),
                signature: [0; 64],
            },
        );
        assert_eq!(words, "Pass. The gate accepted this PCOC.");
        handle.join().unwrap();
    }
}
