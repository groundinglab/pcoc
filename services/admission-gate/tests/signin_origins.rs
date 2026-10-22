//! SPEC section 15.1 **Origin**: the signed origin is the requesting site's
//! own origin, compared exactly.
//!
//! Loads the pinned `tests/vectors/sign-in-origins.json` (a copy of
//! `spec/conformance/v2.0/sign-in-origins.json`). Each case is a challenge
//! signed with BIP-340 (aux all zero) over that case's origin. The check
//! runs the Gate's own origin rule with the served list set to the vector's
//! `served`, so it tests the rule a gate built for `https://example.com`
//! applies. A gate serves the origins in its config key `served_origins`.
//!
//! Regenerate both copies with `PCOC_REGEN_SIGNIN_ORIGINS=1 cargo test --test signin_origins`.

#![forbid(unsafe_code)]

use pcoc_admission::wire;
use serde_json::{json, Value};
use std::path::PathBuf;

const SERVED: &str = "https://example.com";
const SECRET: [u8; 32] = {
    let mut s = [0u8; 32];
    s[31] = 1;
    s
};
const NONCE_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const EXP: u64 = 1_893_456_000;

fn cases() -> Vec<(&'static str, &'static str, bool)> {
    vec![
        ("https://example.com", "served origin, exact", true),
        (
            "https://groundinglab.io",
            "another deployment's origin",
            false,
        ),
        ("https://example.com/", "trailing slash", false),
        ("http://example.com", "http instead of https", false),
        ("https://example.com:8443", "different port", false),
        (
            "https://example.com:443",
            "explicit default port is not the serialization",
            false,
        ),
        ("https://EXAMPLE.com", "uppercase host", false),
        ("HTTPS://example.com", "uppercase scheme", false),
    ]
}

fn build() -> Value {
    let nonce: [u8; 32] = hex::decode(NONCE_HEX).unwrap().try_into().unwrap();
    let object = [0xaa_u8; 32];
    let public_key = {
        let secp = secp256k1::Secp256k1::new();
        let key = secp256k1::SecretKey::from_slice(&SECRET).unwrap();
        secp256k1::Keypair::from_secret_key(&secp, &key)
            .x_only_public_key()
            .0
            .serialize()
    };
    let items: Vec<Value> = cases()
        .into_iter()
        .map(|(origin, note, accept)| {
            let bytes = wire::challenge_bytes(origin, &nonce, EXP, &object);
            let message = wire::bip340_message(origin, &nonce, EXP, &object);
            let secp = secp256k1::Secp256k1::new();
            let keypair = secp256k1::Keypair::from_secret_key(
                &secp,
                &secp256k1::SecretKey::from_slice(&SECRET).unwrap(),
            );
            let signature = secp
                .sign_schnorr_with_aux_rand(
                    &secp256k1::Message::from_digest(message),
                    &keypair,
                    &[0u8; 32],
                )
                .serialize();
            json!({
                "origin": origin,
                "note": note,
                "origin_len": origin.len(),
                "challenge_bytes_len": bytes.len(),
                "challenge_bytes_hex": hex::encode(&bytes),
                "bip340_message_32_hex": hex::encode(message),
                "signature_hex": hex::encode(signature),
                "signature_valid": true,
                "served_accepts": accept,
                "client": if accept { "delivery_ok_past_step_2" } else { "malformed" },
            })
        })
        .collect();
    json!({
        "id": "sign-in-origins",
        "spec": "Satoshi Hike v2.0 section 15.1 Origin",
        "label": "The signed origin is the requesting site's own origin, compared byte for byte. Not a section 14 record. Not produced by the hike pack generator. Not in manifest.json.",
        "generator": "services/admission-gate: PCOC_REGEN_SIGNIN_ORIGINS=1 cargo test --test signin_origins (BIP-340, aux 32 zero bytes)",
        "domain": "pcoc-gate-signin-v1",
        "served": [SERVED],
        "served_note": "What a service that serves only https://example.com does at check-order step 2. A mismatch is client malformed and the nonce is not consumed, as in the check order.",
        "nonce_hex": NONCE_HEX,
        "expiry_unix_s": EXP,
        "object_sha256_hex": hex::encode(object),
        "secret_hex": hex::encode(SECRET),
        "public_key_hex": hex::encode(public_key),
        "aux_hex": hex::encode([0u8; 32]),
        "test_key_note": "Not a production key. Do not use it on a deployed gate.",
        "cases": items,
    })
}

fn vector_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/sign-in-origins.json")
}

#[test]
fn signin_origin_vectors() {
    // The pinned file stays. A fresh secp256k1 signing does not have to
    // reproduce its signature bytes. Each pinned signature is checked with
    // the gate's BIP-340 verify.
    if std::env::var("PCOC_REGEN_SIGNIN_ORIGINS").ok().as_deref() == Some("1") {
        let built = build();
        let text = serde_json::to_string_pretty(&built).unwrap() + "\n";
        std::fs::write(vector_path(), &text).unwrap();
        let spec = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../spec/conformance/v2.0/sign-in-origins.json");
        std::fs::write(spec, &text).unwrap();
    }
    let stored: Value =
        serde_json::from_str(&std::fs::read_to_string(vector_path()).unwrap()).unwrap();

    let served: Vec<&str> = stored["served"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let nonce: [u8; 32] = hex::decode(stored["nonce_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let object: [u8; 32] = hex::decode(stored["object_sha256_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let key: [u8; 32] = hex::decode(stored["public_key_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let exp = stored["expiry_unix_s"].as_u64().unwrap();
    for case in stored["cases"].as_array().unwrap() {
        let origin = case["origin"].as_str().unwrap();
        let bytes = wire::challenge_bytes(origin, &nonce, exp, &object);
        assert_eq!(
            hex::encode(&bytes),
            case["challenge_bytes_hex"].as_str().unwrap(),
            "{origin}"
        );
        assert_eq!(bytes.len(), 93 + origin.len());
        let message = wire::bip340_message(origin, &nonce, exp, &object);
        let sig: [u8; 64] = hex::decode(case["signature_hex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        assert!(
            wire::verify_signature(&key, &message, &sig),
            "{origin}: signature"
        );
        // The signature is bound to its origin: it does not verify under the served origin.
        if origin != served[0] {
            let other = wire::bip340_message(served[0], &nonce, exp, &object);
            assert!(
                !wire::verify_signature(&key, &other, &sig),
                "{origin}: cross-origin"
            );
        }
        assert_eq!(
            wire::origin_served(&served, origin),
            case["served_accepts"].as_bool().unwrap(),
            "{origin}: {}",
            case["note"]
        );
    }
}

#[test]
fn the_groundinglab_deploy_serves_only_its_configured_origin() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("deploy/gate.toml");
    let cfg = pcoc_admission::Config::load(&path).unwrap();
    let served = &cfg.served_origins;
    assert_eq!(served, &vec!["https://groundinglab.io".to_owned()]);
    assert!(wire::origin_served(served, "https://groundinglab.io"));
    assert!(!wire::origin_served(served, "https://example.com"));
    assert!(!wire::origin_served(served, "https://groundinglab.io/"));
}
