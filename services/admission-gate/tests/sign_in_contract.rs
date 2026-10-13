//! Locked sign-in wire, SPEC section 15.1.
//!
//! Loads the pinned `tests/vectors/sign-in.json` (a copy of
//! `conformance/v2.0/sign-in.json`; `tests/vectors.rs` checks its hash). The live door still checks a
//! display-order index header hash. This test does not call that door.

#![forbid(unsafe_code)]

use base64::Engine as _;
use secp256k1::{Keypair, Message, Parity, Secp256k1, SecretKey};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn vector() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/sign-in.json");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&text).expect("sign-in.json")
}

fn hex_32(name: &str, hex_text: &str) -> [u8; 32] {
    let bytes = hex::decode(hex_text).unwrap_or_else(|err| panic!("{name}: {err}"));
    bytes
        .try_into()
        .unwrap_or_else(|v: Vec<u8>| panic!("{name} is {} bytes", v.len()))
}

fn lower_hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn trim_ascii_ws(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && matches!(bytes[start], b' ' | b'\t' | b'\r' | b'\n') {
        start += 1;
    }
    while end > start && matches!(bytes[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
        end -= 1;
    }
    &s[start..end]
}

/// Canonical link, after the paste trim. Anything else is not a link.
fn is_link(raw: &str) -> bool {
    let text = trim_ascii_ws(raw);
    let Some(rest) = text.strip_prefix("https://groundinglab.io/sign-in/?v=1&nonce=") else {
        return false;
    };
    let Some((nonce, exp)) = rest.split_once("&exp=") else {
        return false;
    };
    if !lower_hex(nonce, 64) || exp.contains('&') || exp.contains('#') {
        return false;
    }
    if exp != "0"
        && (exp.is_empty() || exp.as_bytes()[0] == b'0' || !exp.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    exp.parse::<u64>().is_ok()
}

fn challenge_bytes(origin: &str, nonce: &[u8; 32], exp: u64, object: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(93 + origin.len());
    out.extend_from_slice(b"pcoc-gate-signin-v1");
    out.push(0x00);
    out.push(u8::try_from(origin.len()).expect("origin length"));
    out.extend_from_slice(origin.as_bytes());
    out.extend_from_slice(nonce);
    out.extend_from_slice(&exp.to_be_bytes());
    out.extend_from_slice(object);
    out
}

fn window_ok(now: u64, exp: u64, lifetime_s: u64, skew_s: u64) -> bool {
    let now = u128::from(now);
    let exp = u128::from(exp);
    let lifetime_s = u128::from(lifetime_s);
    let skew_s = u128::from(skew_s);
    now < exp + skew_s && exp <= now + lifetime_s + skew_s
}

const BODY_KEYS: [&str; 6] = [
    "origin",
    "nonce",
    "exp",
    "object",
    "object_b64",
    "signature",
];

fn grammar_ok(body: &Value) -> bool {
    let Some(map) = body.as_object() else {
        return false;
    };
    if map.len() != BODY_KEYS.len() || BODY_KEYS.iter().any(|key| !map.contains_key(*key)) {
        return false;
    }
    if map.get("origin").and_then(Value::as_str) != Some("https://groundinglab.io") {
        return false;
    }
    let nonce = map.get("nonce").and_then(Value::as_str).unwrap_or("");
    let object = map.get("object").and_then(Value::as_str).unwrap_or("");
    let signature = map.get("signature").and_then(Value::as_str).unwrap_or("");
    if !lower_hex(nonce, 64) || !lower_hex(object, 64) || !lower_hex(signature, 128) {
        return false;
    }
    if map.get("exp").and_then(Value::as_u64).is_none() {
        return false;
    }
    let encoded = map.get("object_b64").and_then(Value::as_str).unwrap_or("");
    if encoded.bytes().any(|b| b.is_ascii_whitespace()) {
        return false;
    }
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .is_ok()
}

#[test]
fn sign_in_links_match_the_canonical_grammar() {
    let doc = vector();
    assert_eq!(doc["origin"], "https://groundinglab.io");
    assert_eq!(doc["door"], "https://admission.groundinglab.io/v1/sign-in");
    assert_eq!(doc["method"], "POST");
    assert_eq!(doc["lifetime_s"], 120);
    assert_eq!(doc["skew_s"], 5);
    assert_eq!(doc["client_ok"], "Sign-in submitted");
    assert_eq!(
        doc["client_codes"],
        serde_json::json!(["bad_signature", "malformed"])
    );
    for item in doc["links"].as_array().expect("links") {
        let input = item["input"].as_str().expect("input");
        let class = item["class"].as_str().expect("class");
        let note = item["note"].as_str().unwrap_or("");
        match class {
            "link" => assert!(is_link(input), "{note} should be a link: {input:?}"),
            "malformed" => assert!(!is_link(input), "{note} should not be a link: {input:?}"),
            other => panic!("unknown link class {other}"),
        }
    }
}

#[test]
fn sign_in_challenge_bytes_and_digest() {
    let doc = vector();
    let example = &doc["example"];
    let origin = example["origin"].as_str().unwrap();
    let nonce = hex_32("nonce", example["nonce_hex"].as_str().unwrap());
    let object = hex_32("object", example["object_sha256_hex"].as_str().unwrap());
    let exp = example["expiry_unix_s"].as_u64().unwrap();
    let bytes = challenge_bytes(origin, &nonce, exp, &object);
    assert_eq!(
        bytes.len(),
        example["challenge_bytes_len"].as_u64().unwrap() as usize
    );
    assert_eq!(
        hex::encode(&bytes),
        example["challenge_bytes_hex"].as_str().unwrap()
    );
    assert_eq!(bytes.len(), 116);
    let digest = Sha256::digest(&bytes);
    assert_eq!(
        hex::encode(digest),
        example["bip340_message_32_hex"].as_str().unwrap()
    );
    let twice = Sha256::digest(digest);
    assert_eq!(
        hex::encode(twice),
        example["double_hash_hex"].as_str().unwrap()
    );

    let encoded = example["object_b64"].as_str().unwrap();
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("object_b64");
    assert_eq!(raw, vec![0xaa; 32]);
    let file_hash = hex::encode(Sha256::digest(&raw));
    assert_eq!(file_hash, example["file_of_0xaa_sha256"].as_str().unwrap());
    assert_ne!(file_hash, example["object_sha256_hex"].as_str().unwrap());
    assert_eq!(example["hash_matches_object_field"].as_bool(), Some(false));

    let link = format!(
        "https://groundinglab.io/sign-in/?v=1&nonce={}&exp={exp}",
        example["nonce_hex"].as_str().unwrap(),
    );
    assert_eq!(example["link"].as_str().unwrap(), link);
    assert!(!link.contains("object="));
    assert!(!is_link(
        &(link.clone() + "&object=" + example["object_sha256_hex"].as_str().unwrap())
    ));
    // The object stays in challenge_bytes. The link does not carry it.
    assert!(bytes.ends_with(&object));
}

/// The door never reads the link. An object-free link plus `object` in the
/// body is the sign-in: challenge_bytes still bind that body field.
#[test]
fn sign_in_object_free_link_binds_the_body_object() {
    let doc = vector();
    let example = &doc["example"];
    let link = example["link"].as_str().unwrap();
    assert!(is_link(link), "{link}");
    assert!(!link.contains("object="));
    assert!(!link.contains('&') || link.matches('&').count() == 2);
    let body = &doc["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["class"] == "grammar_ok")
        .unwrap()["body"];
    assert!(grammar_ok(body));
    assert_eq!(
        body["nonce"].as_str().unwrap(),
        example["nonce_hex"].as_str().unwrap()
    );
    assert_eq!(
        body["object"].as_str().unwrap(),
        example["object_sha256_hex"].as_str().unwrap()
    );
    let origin = example["origin"].as_str().unwrap();
    let nonce = hex_32("nonce", example["nonce_hex"].as_str().unwrap());
    let object = hex_32("object", body["object"].as_str().unwrap());
    let exp = body["exp"].as_u64().unwrap();
    let bytes = challenge_bytes(origin, &nonce, exp, &object);
    assert_eq!(
        hex::encode(&bytes),
        example["challenge_bytes_hex"].as_str().unwrap()
    );
}

#[test]
fn sign_in_signature_verifies_once() {
    let doc = vector();
    let example = &doc["example"];
    assert_eq!(example["aux_hex"].as_str().unwrap(), "00".repeat(32));
    assert_eq!(example["parity"], "even");
    let secret =
        SecretKey::from_slice(&hex::decode(example["secret_hex"].as_str().unwrap()).unwrap())
            .expect("secret");
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let (public, parity) = keypair.x_only_public_key();
    assert_eq!(parity, Parity::Even);
    assert_eq!(
        hex::encode(public.serialize()),
        example["public_key_hex"].as_str().unwrap()
    );
    let digest = hex_32("digest", example["bip340_message_32_hex"].as_str().unwrap());
    let message = Message::from_digest(digest);
    let signed = secp.sign_schnorr_no_aux_rand(&message, &keypair);
    assert_eq!(
        hex::encode(signed.serialize()),
        example["signature_hex"].as_str().unwrap()
    );
    secp.verify_schnorr(&signed, &message, &public)
        .expect("signature over the digest");
    let twice = hex_32("double", example["double_hash_hex"].as_str().unwrap());
    let hashed_again = Message::from_digest(twice);
    assert!(
        secp.verify_schnorr(&signed, &hashed_again, &public)
            .is_err(),
        "SHA-256 of the digest is not the BIP-340 message"
    );
}

#[test]
fn sign_in_window_uses_wide_integers() {
    let doc = vector();
    let lifetime_s = doc["lifetime_s"].as_u64().unwrap();
    let skew_s = doc["skew_s"].as_u64().unwrap();
    assert_eq!(lifetime_s, 120);
    assert_eq!(skew_s, 5);
    for item in doc["window"].as_array().expect("window") {
        let now = item["now"].as_u64().unwrap();
        let exp = item["exp"].as_u64().unwrap();
        let accept = item["accept"].as_bool().unwrap();
        assert_eq!(
            window_ok(now, exp, lifetime_s, skew_s),
            accept,
            "now {now} exp {exp}"
        );
    }
    let example = &doc["example"];
    let now = example["window_now_2026_10_10"].as_u64().unwrap();
    let exp = example["expiry_unix_s"].as_u64().unwrap();
    assert_eq!(
        window_ok(now, exp, lifetime_s, skew_s),
        example["window_accepts_example_at_2026_10_10"]
            .as_bool()
            .unwrap()
    );
    assert!(!window_ok(now, exp, lifetime_s, skew_s));
}

#[test]
fn sign_in_body_grammar() {
    let doc = vector();
    assert_eq!(
        doc["responses"]["delivery_ok"],
        serde_json::json!({"delivery": "ok"})
    );
    assert_eq!(
        doc["responses"]["bad_signature"],
        serde_json::json!({"error": {"code": "bad_signature"}})
    );
    assert_eq!(
        doc["responses"]["malformed"],
        serde_json::json!({"error": {"code": "malformed"}})
    );
    assert!(doc["responses"]["bad_signature"].get("message").is_none());
    assert!(doc["responses"]["bad_signature"]["error"]
        .get("message")
        .is_none());
    for item in doc["bodies"].as_array().expect("bodies") {
        let class = item["class"].as_str().unwrap();
        let note = item["note"].as_str().unwrap_or("");
        let ok = grammar_ok(&item["body"]);
        match class {
            "grammar_ok" => assert!(ok, "{note}"),
            "malformed" => assert!(!ok, "{note}"),
            other => panic!("unknown body class {other}"),
        }
    }
}
