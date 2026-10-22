//! SPEC §15.1 sign-in wire. Rust only (plan decision D7).
//!
//! Challenge bytes, the example-origin link grammar, the challenge window,
//! exact origin comparison, the six-field submit body, and one BIP-340
//! verify over `SHA-256(challenge_bytes)`. The digest is not hashed again.

use secp256k1::schnorr::Signature;
use secp256k1::{Keypair, Message, Parity, Secp256k1, SecretKey, XOnlyPublicKey};
use sha2::{Digest, Sha256};

/// SPEC §15.1. The 19 ASCII bytes `pcoc-gate-signin-v1`. No trailing NUL.
pub const DOMAIN: &[u8; 19] = b"pcoc-gate-signin-v1";

/// Labelled defaults from SPEC §15.4. A policy may set other values.
pub const LIFETIME_S: u64 = 120;
pub const SKEW_S: u64 = 5;

/// Example deployment origin. Not a requirement (SPEC §15.1).
pub const EXAMPLE_ORIGIN: &str = "https://groundinglab.io";

/// Fixed test secret, 32 bytes, value 1. Not a production key.
pub const TEST_SECRET: [u8; 32] = {
    let mut s = [0u8; 32];
    s[31] = 1;
    s
};

const BODY_KEYS: [&str; 6] = [
    "origin",
    "nonce",
    "exp",
    "object",
    "object_b64",
    "signature",
];

/// SPEC §15.1 **Bytes**. Length is `93 + origin.len()`.
pub fn challenge_bytes(origin: &str, nonce: &[u8; 32], exp: u64, object: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(93 + origin.len());
    out.extend_from_slice(DOMAIN);
    out.push(0x00);
    let n = u8::try_from(origin.len()).unwrap_or(u8::MAX);
    let take = usize::from(n).min(origin.len());
    out.push(n);
    out.extend_from_slice(&origin.as_bytes()[..take]);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&exp.to_be_bytes());
    out.extend_from_slice(object);
    out
}

/// SPEC §15.1. `bip340_message_32 = SHA-256(challenge_bytes)`.
pub fn bip340_message(origin: &str, nonce: &[u8; 32], exp: u64, object: &[u8; 32]) -> [u8; 32] {
    let bytes = challenge_bytes(origin, nonce, exp, object);
    let digest = Sha256::digest(&bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// SPEC §15.1. `now < exp + skew` and `exp <= now + lifetime + skew`, wide integers.
///
/// The nonce-store comparison is [`store_accepts`]. These two are the window
/// rows in `sign-in.json`.
pub fn window_accepts(now: u64, exp: u64, lifetime_s: u64, skew_s: u64) -> bool {
    let now = u128::from(now);
    let exp = u128::from(exp);
    let lifetime_s = u128::from(lifetime_s);
    let skew_s = u128::from(skew_s);
    now < exp + skew_s && exp <= now + lifetime_s + skew_s
}

/// SPEC §15.1. `exp - lifetime >= nonce_store_start + skew`.
pub fn store_accepts(exp: u64, lifetime_s: u64, skew_s: u64, nonce_store_start: u64) -> bool {
    let exp = u128::from(exp);
    let start = u128::from(nonce_store_start);
    let lifetime_s = u128::from(lifetime_s);
    let skew_s = u128::from(skew_s);
    exp >= start + skew_s + lifetime_s
}

/// SPEC §15.1 **Origin**. Exact bytes. Nothing is case-folded or completed.
pub fn origin_served(served: &[&str], origin: &str) -> bool {
    served.iter().any(|candidate| candidate.as_bytes() == origin.as_bytes())
}

/// SPEC §15.1 challenge link for one origin.
///
/// A paste trims only leading and trailing ASCII space, tab, CR, and LF.
/// The remainder is `<origin>/sign-in/?v=1&nonce=<64 lowercase hex>&exp=<decimal>`.
pub fn is_canonical_link(raw: &str, origin: &str) -> bool {
    let text = trim_ascii(raw);
    let prefix = format!("{origin}/sign-in/?v=1&nonce=");
    let Some(rest) = text.strip_prefix(&prefix) else {
        return false;
    };
    let Some((nonce, exp)) = rest.split_once("&exp=") else {
        return false;
    };
    if !lower_hex(nonce, 64) || exp.contains('&') || exp.contains('#') {
        return false;
    }
    shortest_u64(exp)
}

/// `link` or `malformed`, for the example origin `https://groundinglab.io`.
pub fn example_link_class(raw: &str) -> &'static str {
    if is_canonical_link(raw, EXAMPLE_ORIGIN) {
        "link"
    } else {
        "malformed"
    }
}

/// SPEC §15.1 **Submit**. One JSON object, the six keys, the spellings in that section.
pub fn submit_grammar_ok(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    let Some(map) = value.as_object() else {
        return false;
    };
    if map.len() != BODY_KEYS.len() || BODY_KEYS.iter().any(|key| !map.contains_key(*key)) {
        return false;
    }
    let nonce = map.get("nonce").and_then(|v| v.as_str()).unwrap_or("");
    let object = map.get("object").and_then(|v| v.as_str()).unwrap_or("");
    let signature = map.get("signature").and_then(|v| v.as_str()).unwrap_or("");
    if !lower_hex(nonce, 64) || !lower_hex(object, 64) || !lower_hex(signature, 128) {
        return false;
    }
    if map.get("origin").and_then(|v| v.as_str()).is_none() {
        return false;
    }
    if map.get("exp").and_then(|v| v.as_u64()).is_none() {
        return false;
    }
    let encoded = map.get("object_b64").and_then(|v| v.as_str()).unwrap_or("");
    b64_decode(encoded).is_some()
}

/// BIP-340 verify of `message` under the x-only `public_key`. No second hash.
pub fn verify_bip340(public_key: &[u8; 32], message: &[u8; 32], signature: &[u8; 64]) -> bool {
    let Ok(key) = XOnlyPublicKey::from_slice(public_key) else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(signature) else {
        return false;
    };
    let msg = Message::from_digest(*message);
    Secp256k1::verification_only()
        .verify_schnorr(&sig, &msg, &key)
        .is_ok()
}

/// Sign `message` with the fixed test key. Aux is 32 zero bytes.
/// Returns `(public_key, signature)`.
pub fn sign_test_key(message: &[u8; 32]) -> ([u8; 32], [u8; 64]) {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_slice(&TEST_SECRET).expect("test secret");
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let (public, _parity) = keypair.x_only_public_key();
    let sig = secp.sign_schnorr_with_aux_rand(
        &Message::from_digest(*message),
        &keypair,
        &[0u8; 32],
    );
    (public.serialize(), sig.serialize())
}

/// Standard base64 of `bytes`, with `=` padding and no whitespace.
pub fn b64_encode(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | bytes[i + 2] as u32;
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        out.push(A[((n >> 6) & 63) as usize] as char);
        out.push(A[(n & 63) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = (bytes[i] as u32) << 16;
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        out.push(A[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn b64_decode(text: &str) -> Option<Vec<u8>> {
    if text.is_empty() || text.len() % 4 != 0 || text.bytes().any(|b| b.is_ascii_whitespace()) {
        return None;
    }
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let chunk = &bytes[i..i + 4];
        let pad = chunk.iter().filter(|b| **b == b'=').count();
        if pad > 2 || (pad > 0 && i + 4 != bytes.len()) {
            return None;
        }
        if chunk[..4 - pad].iter().any(|b| b64_val(*b).is_none()) {
            return None;
        }
        if pad == 1 && chunk[3] != b'=' {
            return None;
        }
        if pad == 2 && (chunk[2] != b'=' || chunk[3] != b'=') {
            return None;
        }
        let a = b64_val(chunk[0])?;
        let b = b64_val(chunk[1])?;
        let c = if pad == 2 { 0 } else { b64_val(chunk[2])? };
        let d = if pad >= 1 { 0 } else { b64_val(chunk[3])? };
        let n = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6) | d as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
        i += 4;
    }
    Some(out)
}

fn b64_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn trim_ascii(s: &str) -> &str {
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

fn lower_hex(text: &str, n: usize) -> bool {
    text.len() == n
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn shortest_u64(text: &str) -> bool {
    if text == "0" {
        return true;
    }
    if text.is_empty() || text.as_bytes()[0] == b'0' || !text.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    text.parse::<u64>().is_ok()
}

/// `sign-in.json` for the pack. Rust SDK only.
pub fn pack_sign_in_json() -> String {
    let nonce_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let nonce: [u8; 32] = crate::hexutil::hex_decode_exact(nonce_hex).unwrap();
    let object = [0xaa_u8; 32];
    let exp = 1_893_456_000_u64;
    let bytes = challenge_bytes(EXAMPLE_ORIGIN, &nonce, exp, &object);
    let message = bip340_message(EXAMPLE_ORIGIN, &nonce, exp, &object);
    let (public, signature) = sign_test_key(&message);
    let parity = test_key_parity();
    let object_b64 = b64_encode(&object);
    let file_hash = Sha256::digest(object);
    let double_hash = Sha256::digest(message);
    let link = format!(
        "{EXAMPLE_ORIGIN}/sign-in/?v=1&nonce={nonce_hex}&exp={exp}"
    );
    let links = sign_in_links(&link, nonce_hex);
    let window_rows = [
        (1_000_000_u64, 1_000_120_u64),
        (1_000_120, 1_000_120),
        (1_000_124, 1_000_120),
        (1_000_125, 1_000_120),
        (1_000_000, 1_000_125),
        (1_000_000, 1_000_126),
        (1_000_000, 999_996),
        (1_000_000, 999_995),
    ];
    let window: Vec<serde_json::Value> = window_rows
        .iter()
        .map(|(now, exp)| {
            serde_json::json!({
                "now": now,
                "exp": exp,
                "accept": window_accepts(*now, *exp, LIFETIME_S, SKEW_S),
            })
        })
        .collect();
    let sig_hex = crate::hexutil::hex_encode(&signature);
    let bodies = sign_in_bodies(&object_b64, &sig_hex);
    let doc = serde_json::json!({
        "id": "sign-in",
        "spec": "Satoshi Hike 2.2 section 15.1",
        "label": "Locked sign-in wire. Not a section 14 record. Produced by the Rust SDK.",
        "domain": "pcoc-gate-signin-v1",
        "origin": EXAMPLE_ORIGIN,
        "lifetime_s": LIFETIME_S,
        "skew_s": SKEW_S,
        "door": "https://admission.groundinglab.io/v1/sign-in",
        "method": "POST",
        "content_type": "application/json",
        "client_ok": "Sign-in submitted",
        "client_codes": ["bad_signature", "malformed"],
        "website_outcomes": [
            "pending",
            "accepted",
            "malformed",
            "bad_signature",
            "too_short",
            "rejected",
            "unknown_kind",
            "too_few_waypoints_anchored"
        ],
        "responses": {
            "delivery_ok": {"delivery": "ok"},
            "bad_signature": {"error": {"code": "bad_signature"}},
            "malformed": {"error": {"code": "malformed"}}
        },
        "example": {
            "origin": EXAMPLE_ORIGIN,
            "nonce_hex": nonce_hex,
            "expiry_unix_s": exp,
            "object_sha256_hex": crate::hexutil::hex_encode(&object),
            "object_sha256_note": "The 32-byte field of 0xaa. Not the hash of a file of 32 0xaa bytes.",
            "file_of_0xaa_sha256": crate::hexutil::hex_encode(&file_hash),
            "link": link,
            "challenge_bytes_hex": crate::hexutil::hex_encode(&bytes),
            "challenge_bytes_len": bytes.len(),
            "bip340_message_32_hex": crate::hexutil::hex_encode(&message),
            "double_hash_hex": crate::hexutil::hex_encode(&double_hash),
            "secret_hex": crate::hexutil::hex_encode(&TEST_SECRET),
            "public_key_hex": crate::hexutil::hex_encode(&public),
            "parity": parity,
            "aux_hex": "00".repeat(32),
            "signature_hex": sig_hex,
            "object_b64": object_b64,
            "hash_matches_object_field": false,
            "window_now_2026_10_10": 1_791_590_400_u64,
            "window_accepts_example_at_2026_10_10": window_accepts(1_791_590_400, exp, LIFETIME_S, SKEW_S),
            "test_key_note": "Not a production key. Do not use it on a deployed gate."
        },
        "links": links,
        "window": window,
        "bodies": bodies
    });
    serde_json::to_string_pretty(&doc).unwrap() + "\n"
}

/// `sign-in-origins.json`. One service serves only `https://example.com`.
pub fn pack_sign_in_origins_json() -> String {
    let nonce_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let nonce: [u8; 32] = crate::hexutil::hex_decode_exact(nonce_hex).unwrap();
    let object = [0xaa_u8; 32];
    let exp = 1_893_456_000_u64;
    let (public, parity_name) = {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&TEST_SECRET).unwrap();
        let keypair = Keypair::from_secret_key(&secp, &secret);
        let (public, parity) = keypair.x_only_public_key();
        let name = if parity == Parity::Even { "even" } else { "odd" };
        (public.serialize(), name)
    };
    let _ = parity_name;
    let served = "https://example.com";
    let cases = [
        (served, "served origin, exact", true),
        ("https://groundinglab.io", "another deployment's origin", false),
        ("https://example.com/", "trailing slash", false),
        ("http://example.com", "http instead of https", false),
        ("https://example.com:8443", "different port", false),
        ("https://example.com:443", "explicit default port is not the serialization", false),
        ("https://EXAMPLE.com", "uppercase host", false),
        ("HTTPS://example.com", "uppercase scheme", false),
    ];
    let mut items = Vec::new();
    for (origin, note, accept) in cases {
        let bytes = challenge_bytes(origin, &nonce, exp, &object);
        let message = bip340_message(origin, &nonce, exp, &object);
        let (_, signature) = sign_test_key(&message);
        assert!(verify_bip340(&public, &message, &signature));
        assert_eq!(origin_served(&[served], origin), accept);
        items.push(serde_json::json!({
            "origin": origin,
            "note": note,
            "origin_len": origin.len(),
            "challenge_bytes_len": bytes.len(),
            "challenge_bytes_hex": crate::hexutil::hex_encode(&bytes),
            "bip340_message_32_hex": crate::hexutil::hex_encode(&message),
            "signature_hex": crate::hexutil::hex_encode(&signature),
            "signature_valid": true,
            "served_accepts": accept,
            "client": if accept { "delivery_ok_past_step_2" } else { "malformed" },
        }));
    }
    let doc = serde_json::json!({
        "id": "sign-in-origins",
        "spec": "Satoshi Hike 2.2 section 15.1 Origin",
        "label": "The signed origin is the requesting site's own origin, compared byte for byte. Not a section 14 record. Produced by the Rust SDK.",
        "generator": "sdk/rust pcoc::signin (BIP-340, aux 32 zero bytes)",
        "domain": "pcoc-gate-signin-v1",
        "served": [served],
        "served_note": "What a service that serves only https://example.com does at check-order step 2. A mismatch is client malformed and the nonce is not consumed.",
        "nonce_hex": nonce_hex,
        "expiry_unix_s": exp,
        "object_sha256_hex": crate::hexutil::hex_encode(&object),
        "secret_hex": crate::hexutil::hex_encode(&TEST_SECRET),
        "public_key_hex": crate::hexutil::hex_encode(&public),
        "aux_hex": "00".repeat(32),
        "test_key_note": "Not a production key. Do not use it on a deployed gate.",
        "cases": items
    });
    serde_json::to_string_pretty(&doc).unwrap() + "\n"
}

fn test_key_parity() -> &'static str {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_slice(&TEST_SECRET).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    if keypair.x_only_public_key().1 == Parity::Even {
        "even"
    } else {
        "odd"
    }
}

fn sign_in_links(canonical: &str, nonce_hex: &str) -> Vec<serde_json::Value> {
    let exp = "1893456000";
    let rows = [
        (canonical.to_owned(), "canonical"),
        (format!(" \t{canonical}\r\n"), "ASCII trim"),
        (
            format!("{EXAMPLE_ORIGIN}/sign-in/?v=1&nonce={nonce_hex}&exp=0"),
            "exp 0 is the one leading-zero exception",
        ),
        (
            format!("{EXAMPLE_ORIGIN}/sign-in/?h=90e38d7538d543aff5148318b7cb310595def390dac199c99bb10bc9ead5e0c2"),
            "retired ?h= form",
        ),
        (
            format!("{EXAMPLE_ORIGIN}/sign-in/?v=1&nonce=0123456789Abcdef0123456789abcdef0123456789abcdef0123456789abcdef&exp={exp}"),
            "uppercase hex",
        ),
        (
            format!("{canonical}&door=https://admission.groundinglab.io/v1/sign-in"),
            "extra parameter",
        ),
        (
            format!("{EXAMPLE_ORIGIN}/sign-in/?v=1&exp={exp}&nonce={nonce_hex}"),
            "swapped parameter order",
        ),
        (
            format!("http://groundinglab.io/sign-in/?v=1&nonce={nonce_hex}&exp={exp}"),
            "http",
        ),
        (
            format!("https://www.groundinglab.io/sign-in/?v=1&nonce={nonce_hex}&exp={exp}"),
            "www host",
        ),
        (format!("{canonical}#x"), "fragment"),
        (
            format!("{EXAMPLE_ORIGIN}/sign-in/?v=1&nonce={nonce_hex}&exp=01893456000"),
            "leading zero on exp",
        ),
        (nonce_hex.to_owned(), "bare 64 hex"),
        (
            format!("{EXAMPLE_ORIGIN}/sign-in/?v=1&nonce={nonce_hex}&exp=18446744073709551616"),
            "exp above u64",
        ),
        (
            format!("\u{00a0}{canonical}"),
            "leading U+00A0",
        ),
        (
            format!("{canonical}&object=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            "object parameter",
        ),
        (format!("{canonical}&x=1"), "extra parameter x"),
    ];
    rows.into_iter()
        .map(|(input, note)| {
            serde_json::json!({
                "input": input,
                "class": example_link_class(&input),
                "note": note,
            })
        })
        .collect()
}

fn sign_in_bodies(object_b64: &str, signature: &str) -> Vec<serde_json::Value> {
    let nonce = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let object = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let good = serde_json::json!({
        "origin": EXAMPLE_ORIGIN,
        "nonce": nonce,
        "exp": 1_893_456_000_u64,
        "object": object,
        "object_b64": object_b64,
        "signature": signature,
    });
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("signature");
    let mut extra = good.clone();
    extra.as_object_mut().unwrap().insert(
        "door".into(),
        serde_json::json!("https://admission.groundinglab.io/v1/sign-in"),
    );
    let mut exp_string = good.clone();
    exp_string
        .as_object_mut()
        .unwrap()
        .insert("exp".into(), serde_json::json!("1893456000"));
    let mut upper = good.clone();
    let mut sig_chars: Vec<u8> = signature.as_bytes().to_vec();
    let last = sig_chars.len() - 1;
    sig_chars[last] = b'F';
    upper.as_object_mut().unwrap().insert(
        "signature".into(),
        serde_json::Value::String(String::from_utf8(sig_chars).unwrap()),
    );
    let mut bare = good.clone();
    bare.as_object_mut().unwrap().insert(
        "object_b64".into(),
        serde_json::json!(object_b64.trim_end_matches('=')),
    );
    let rows = [
        (good, "field grammar only; the example bytes do not hash to the object field"),
        (missing, "missing signature"),
        (extra, "unknown key"),
        (exp_string, "exp as a string"),
        (upper, "uppercase signature"),
        (bare, "base64 without padding"),
    ];
    rows.into_iter()
        .map(|(body, note)| {
            let text = serde_json::to_string(&body).unwrap();
            let class = if submit_grammar_ok(&text) {
                "grammar_ok"
            } else {
                "malformed"
            };
            serde_json::json!({"class": class, "note": note, "body": body})
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_digest_matches_the_specification_sentence() {
        let nonce = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef].repeat(4);
        let nonce: [u8; 32] = nonce.try_into().unwrap();
        let object = [0xaa; 32];
        let bytes = challenge_bytes(EXAMPLE_ORIGIN, &nonce, 1_893_456_000, &object);
        assert_eq!(bytes.len(), 116);
        assert_eq!(
            crate::hexutil::hex_encode(&bytes),
            "70636f632d676174652d7369676e696e2d7631001768747470733a2f2f67726f756e64696e676c61622e696f0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0000000070dbd880aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        let message = bip340_message(EXAMPLE_ORIGIN, &nonce, 1_893_456_000, &object);
        assert_eq!(
            crate::hexutil::hex_encode(&message),
            "90e38d7538d543aff5148318b7cb310595def390dac199c99bb10bc9ead5e0c2"
        );
        let (public, sig) = sign_test_key(&message);
        assert_eq!(
            crate::hexutil::hex_encode(&public),
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );
        assert_eq!(
            crate::hexutil::hex_encode(&sig),
            "05674eed5e01b51beea3a767dda78e6c94027524014525b57073d71da229463dd82d20deecab2874a9d55b43f52c1a8f48ce2072ced76cba5ce946fb3f36c778"
        );
        assert!(verify_bip340(&public, &message, &sig));
        let twice = Sha256::digest(message);
        let mut again = [0u8; 32];
        again.copy_from_slice(&twice);
        assert!(!verify_bip340(&public, &again, &sig));
        assert_eq!(b64_encode(&object), "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=");
    }

    #[test]
    fn example_links_and_window_match_the_published_rows() {
        let canonical = "https://groundinglab.io/sign-in/?v=1&nonce=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&exp=1893456000";
        assert_eq!(example_link_class(canonical), "link");
        assert_eq!(example_link_class(&format!(" \t{canonical}\r\n")), "link");
        assert_eq!(
            example_link_class(
                "https://groundinglab.io/sign-in/?v=1&nonce=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&exp=0"
            ),
            "link"
        );
        for bad in [
            "https://groundinglab.io/sign-in/?h=90e38d7538d543aff5148318b7cb310595def390dac199c99bb10bc9ead5e0c2",
            "https://groundinglab.io/sign-in/?v=1&nonce=0123456789Abcdef0123456789abcdef0123456789abcdef0123456789abcdef&exp=1893456000",
            "https://www.groundinglab.io/sign-in/?v=1&nonce=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&exp=1893456000",
            "https://groundinglab.io/sign-in/?v=1&nonce=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&exp=01893456000",
            "\u{00a0}https://groundinglab.io/sign-in/?v=1&nonce=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&exp=1893456000",
        ] {
            assert_eq!(example_link_class(bad), "malformed", "{bad}");
        }
        assert!(window_accepts(1_000_000, 1_000_120, LIFETIME_S, SKEW_S));
        assert!(!window_accepts(1_000_000, 1_000_126, LIFETIME_S, SKEW_S));
        assert!(!window_accepts(1_791_590_400, 1_893_456_000, LIFETIME_S, SKEW_S));
    }

    #[test]
    fn pack_json_matches_the_example_digest() {
        let sign_in = pack_sign_in_json();
        let origins = pack_sign_in_origins_json();
        assert!(sign_in.contains("90e38d7538d543aff5148318b7cb310595def390dac199c99bb10bc9ead5e0c2"));
        assert!(origins.contains("\"served_accepts\": true"));
        assert!(origins.contains("\"served_accepts\": false"));
    }
}

