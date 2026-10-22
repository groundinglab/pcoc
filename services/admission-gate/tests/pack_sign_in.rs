//! Pack sign-in vectors, checked with this gate's §15.1 parser.
//!
//! `spec/conformance/pack/sign-in.json` and `sign-in-origins.json` are
//! produced by the Rust SDK. This test does not regenerate them. Each row
//! is accepted or refused exactly as the vector says.

#![forbid(unsafe_code)]

use base64::Engine as _;
use pcoc_admission::wire::{self, LockedParse};
use secp256k1::{Keypair, Parity, Secp256k1, SecretKey};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn pack(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/conformance/pack")
        .join(name);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn hex32(name: &str, text: &str) -> [u8; 32] {
    let bytes = hex::decode(text).unwrap_or_else(|err| panic!("{name}: {err}"));
    bytes
        .try_into()
        .unwrap_or_else(|v: Vec<u8>| panic!("{name} is {} bytes", v.len()))
}

fn lower_hex(text: &str, n: usize) -> bool {
    text.len() == n
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn trim_ascii(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && matches!(bytes[start], b' ' | b'\t' | b'\r' | b'\n') {
        start += 1;
    }
    while end > start && matches!(bytes[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
        end -= 1;
    }
    &text[start..end]
}

/// The example-origin link grammar the vector classifies. Same sentences as §15.1.
fn is_example_link(raw: &str) -> bool {
    let text = trim_ascii(raw);
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

#[test]
fn pack_sign_in_matches_the_gate_parser() {
    let doc = pack("sign-in.json");
    assert_eq!(doc["origin"], "https://groundinglab.io");
    assert_eq!(doc["door"], "https://admission.groundinglab.io/v1/sign-in");
    assert_eq!(doc["lifetime_s"], 120);
    assert_eq!(doc["skew_s"], 5);
    assert_eq!(doc["domain"], "pcoc-gate-signin-v1");
    assert_eq!(
        doc["responses"]["malformed"],
        serde_json::json!({"error": {"code": "malformed"}})
    );
    assert_eq!(
        doc["responses"]["bad_signature"],
        serde_json::json!({"error": {"code": "bad_signature"}})
    );
    assert_eq!(
        doc["responses"]["delivery_ok"],
        serde_json::json!({"delivery": "ok"})
    );

    for item in doc["links"].as_array().unwrap() {
        let input = item["input"].as_str().unwrap();
        let class = item["class"].as_str().unwrap();
        match class {
            "link" => assert!(is_example_link(input), "{}", item["note"]),
            "malformed" => assert!(!is_example_link(input), "{}", item["note"]),
            other => panic!("unknown link class {other}"),
        }
    }

    let example = &doc["example"];
    let origin = example["origin"].as_str().unwrap();
    let nonce = hex32("nonce", example["nonce_hex"].as_str().unwrap());
    let object = hex32("object", example["object_sha256_hex"].as_str().unwrap());
    let exp = example["expiry_unix_s"].as_u64().unwrap();
    let bytes = wire::challenge_bytes(origin, &nonce, exp, &object);
    assert_eq!(
        bytes.len(),
        example["challenge_bytes_len"].as_u64().unwrap() as usize
    );
    assert_eq!(
        hex::encode(&bytes),
        example["challenge_bytes_hex"].as_str().unwrap()
    );
    let message = wire::bip340_message(origin, &nonce, exp, &object);
    assert_eq!(
        hex::encode(message),
        example["bip340_message_32_hex"].as_str().unwrap()
    );
    let twice = Sha256::digest(message);
    assert_eq!(
        hex::encode(twice),
        example["double_hash_hex"].as_str().unwrap()
    );
    assert_ne!(
        hex::encode(twice),
        example["bip340_message_32_hex"].as_str().unwrap()
    );

    let secret =
        SecretKey::from_slice(&hex::decode(example["secret_hex"].as_str().unwrap()).unwrap())
            .unwrap();
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let (public, parity) = keypair.x_only_public_key();
    assert_eq!(parity, Parity::Even);
    assert_eq!(
        hex::encode(public.serialize()),
        example["public_key_hex"].as_str().unwrap()
    );
    let sig = hex::decode(example["signature_hex"].as_str().unwrap()).unwrap();
    let sig: [u8; 64] = sig.try_into().unwrap();
    assert!(wire::verify_signature(&public.serialize(), &message, &sig));
    let again = hex32("double", example["double_hash_hex"].as_str().unwrap());
    assert!(!wire::verify_signature(&public.serialize(), &again, &sig));

    let raw = base64::engine::general_purpose::STANDARD
        .decode(example["object_b64"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        hex::encode(Sha256::digest(&raw)),
        example["file_of_0xaa_sha256"].as_str().unwrap()
    );
    assert_ne!(example["file_of_0xaa_sha256"], example["object_sha256_hex"]);

    let lifetime = doc["lifetime_s"].as_u64().unwrap();
    let skew = doc["skew_s"].as_u64().unwrap();
    for item in doc["window"].as_array().unwrap() {
        let now = item["now"].as_u64().unwrap();
        let exp = item["exp"].as_u64().unwrap();
        let accept = item["accept"].as_bool().unwrap();
        let got = {
            let now = u128::from(now);
            let exp = u128::from(exp);
            now < exp + u128::from(skew) && exp <= now + u128::from(lifetime) + u128::from(skew)
        };
        assert_eq!(
            wire::window_accepts(now, exp, lifetime, skew, 0),
            accept,
            "now {now} exp {exp}"
        );
        assert_eq!(got, accept);
    }
    let now = example["window_now_2026_10_10"].as_u64().unwrap();
    assert_eq!(
        wire::window_accepts(now, exp, lifetime, skew, 0),
        example["window_accepts_example_at_2026_10_10"]
            .as_bool()
            .unwrap()
    );
    assert!(!wire::window_accepts(now, exp, lifetime, skew, 0));

    for item in doc["bodies"].as_array().unwrap() {
        let class = item["class"].as_str().unwrap();
        let text = serde_json::to_vec(&item["body"]).unwrap();
        let parsed = wire::classify_json(&text).expect("json");
        match class {
            "grammar_ok" => assert!(matches!(parsed, LockedParse::Ready(_)), "{}", item["note"]),
            "malformed" => assert!(!matches!(parsed, LockedParse::Ready(_)), "{}", item["note"]),
            other => panic!("unknown body class {other}"),
        }
    }
}

#[test]
fn pack_sign_in_origins_match_the_gate() {
    let doc = pack("sign-in-origins.json");
    let served: Vec<&str> = doc["served"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(served, vec!["https://example.com"]);
    let nonce = hex32("nonce", doc["nonce_hex"].as_str().unwrap());
    let object = hex32("object", doc["object_sha256_hex"].as_str().unwrap());
    let key = hex32("key", doc["public_key_hex"].as_str().unwrap());
    let exp = doc["expiry_unix_s"].as_u64().unwrap();
    for case in doc["cases"].as_array().unwrap() {
        let origin = case["origin"].as_str().unwrap();
        let bytes = wire::challenge_bytes(origin, &nonce, exp, &object);
        assert_eq!(
            hex::encode(&bytes),
            case["challenge_bytes_hex"].as_str().unwrap(),
            "{origin}"
        );
        assert_eq!(bytes.len(), 93 + origin.len());
        let message = wire::bip340_message(origin, &nonce, exp, &object);
        assert_eq!(
            hex::encode(message),
            case["bip340_message_32_hex"].as_str().unwrap()
        );
        let sig: [u8; 64] = hex::decode(case["signature_hex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            wire::verify_signature(&key, &message, &sig),
            case["signature_valid"].as_bool().unwrap(),
            "{origin}"
        );
        assert_eq!(
            wire::origin_served(&served, origin),
            case["served_accepts"].as_bool().unwrap(),
            "{origin}"
        );
        let client = case["client"].as_str().unwrap();
        if case["served_accepts"].as_bool().unwrap() {
            assert_eq!(client, "delivery_ok_past_step_2");
        } else {
            assert_eq!(client, "malformed");
        }
    }
}
