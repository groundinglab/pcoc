//! Locked §15.1 sign-in wire.
//!
//! The six-field JSON body rebuilds `challenge_bytes` and verifies
//! `SHA-256(challenge_bytes)` once. This module does not read the index and
//! does not run policy.

use base64::Engine as _;
use secp256k1::schnorr::Signature;
use secp256k1::{Message, Secp256k1, XOnlyPublicKey};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

/// SPEC section 15.1 **Origin**: exact byte equality with a served origin.
/// No case folding, trimming, decoding, or default-port completion.
///
/// The served origins come from the required config key `served_origins`.
/// There is no origin in code and no default.
pub fn origin_served<S: AsRef<str>>(served: &[S], origin: &str) -> bool {
    served
        .iter()
        .any(|candidate| candidate.as_ref().as_bytes() == origin.as_bytes())
}
pub const DOMAIN: &[u8] = b"pcoc-gate-signin-v1";
pub const LIFETIME_S: u64 = 120;
pub const SKEW_S: u64 = 5;
/// A settled result row stays readable this long after it leaves `pending`.
pub const RESULT_KEEP_S: u64 = 600;

pub const OUTCOME_PENDING: &str = "pending";
pub const OUTCOME_ACCEPTED: &str = "accepted";
pub const OUTCOME_MALFORMED: &str = "malformed";
pub const OUTCOME_BAD_SIGNATURE: &str = "bad_signature";
pub const OUTCOME_TOO_SHORT: &str = "too_short";
pub const OUTCOME_REJECTED: &str = "rejected";
pub const OUTCOME_EARLIER: &str = "earlier_version";
pub const OUTCOME_TOO_FEW: &str = "too_few_waypoints_anchored";

/// Words the current gate already writes for fewer than two of `W_1`–`W_16`.
///
/// This build does not enforce that candidate bar, so the new wire never
/// emits [`OUTCOME_TOO_FEW`]. The constant stays so a later build that does
/// enforce the row can map it without inventing a second phrase.
pub const TOO_FEW_WAYPOINTS_REASONS: &[&str] = &["too_few_waypoints_anchored"];

const KEYS: [&str; 6] = [
    "origin",
    "nonce",
    "exp",
    "object",
    "object_b64",
    "signature",
];

/// What the signing client is told. Delivery OK is every completed response
/// other than the two HTTP 400 bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientClass {
    Malformed,
    BadSignature,
    DeliveryOk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockedBody {
    pub origin: String,
    pub nonce: [u8; 32],
    pub exp: u64,
    pub object_sha: [u8; 32],
    pub object: Vec<u8>,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockedParse {
    /// Step 1. The value is not one JSON object, or `nonce` is not 64
    /// lowercase hex. No result row.
    NotJsonOrNonce,
    /// `nonce` is 64 lowercase hex and the six fields were read. Origin,
    /// base64, and the window are checked by the door.
    Ready(LockedBody),
    /// `nonce` is 64 lowercase hex, and a field breaks the Submit grammar.
    /// The object bytes are present when base64 decoded, so step 2 can store
    /// `malformed` when they hash to `object`.
    Grammar {
        nonce: [u8; 32],
        object: Vec<u8>,
        object_sha: Option<[u8; 32]>,
    },
}

/// `now < exp + skew` and `exp <= now + lifetime + skew`, in wide integers.
pub fn window_accepts(now: u64, exp: u64) -> bool {
    let now = u128::from(now);
    let exp = u128::from(exp);
    now < exp + u128::from(SKEW_S) && exp <= now + u128::from(LIFETIME_S) + u128::from(SKEW_S)
}

pub fn challenge_bytes(origin: &str, nonce: &[u8; 32], exp: u64, object: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(93 + origin.len());
    out.extend_from_slice(DOMAIN);
    out.push(0x00);
    let n = u8::try_from(origin.len()).unwrap_or(u8::MAX);
    out.push(n);
    out.extend_from_slice(&origin.as_bytes()[..usize::from(n).min(origin.len())]);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&exp.to_be_bytes());
    out.extend_from_slice(object);
    out
}

pub fn bip340_message(origin: &str, nonce: &[u8; 32], exp: u64, object: &[u8; 32]) -> [u8; 32] {
    let bytes = challenge_bytes(origin, nonce, exp, object);
    let digest = Sha256::digest(&bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// SHA-256 of `object_sha256 ‖ public_key ‖ bip340_message_32 ‖ signature`.
pub fn submission_sha256(
    object_sha256: &[u8; 32],
    public_key: &[u8; 32],
    message: &[u8; 32],
    signature: &[u8; 64],
) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(160);
    preimage.extend_from_slice(object_sha256);
    preimage.extend_from_slice(public_key);
    preimage.extend_from_slice(message);
    preimage.extend_from_slice(signature);
    let digest = Sha256::digest(&preimage);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn verify_signature(pubkey: &[u8; 32], digest: &[u8; 32], signature: &[u8; 64]) -> bool {
    let Ok(key) = XOnlyPublicKey::from_slice(pubkey) else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(signature) else {
        return false;
    };
    let message = Message::from_digest(*digest);
    verify_only().verify_schnorr(&sig, &message, &key).is_ok()
}

fn verify_only() -> &'static Secp256k1<secp256k1::VerifyOnly> {
    static SECP: OnceLock<Secp256k1<secp256k1::VerifyOnly>> = OnceLock::new();
    SECP.get_or_init(Secp256k1::verification_only)
}

/// Six-field object.
///
/// A JSON value that is not an object, or whose `nonce` is not 64 lowercase
/// hex, is step 1. Any other six-field miss is carried as [`LockedParse`]
/// only when `nonce` itself is well formed; the caller applies the rest.
/// A `{object_b64, signature}` object has no nonce, so it is step 1.
pub fn classify_json(body: &[u8]) -> Result<LockedParse, ()> {
    let parsed: Value = serde_json::from_slice(body).map_err(|_| ())?;
    let Some(map) = parsed.as_object() else {
        return Ok(LockedParse::NotJsonOrNonce);
    };
    let nonce_text = map.get("nonce").and_then(Value::as_str).unwrap_or("");
    if !lower_hex(nonce_text, 64) {
        return Ok(LockedParse::NotJsonOrNonce);
    }
    match assemble(map) {
        Ok(body) => Ok(LockedParse::Ready(body)),
        Err(miss) => Ok(LockedParse::Grammar {
            nonce: miss.nonce,
            object: miss.object,
            object_sha: miss.object_sha,
        }),
    }
}

fn assemble(map: &serde_json::Map<String, Value>) -> Result<LockedBody, GrammarMiss> {
    let nonce =
        decode_fixed(map.get("nonce").and_then(Value::as_str).unwrap_or("")).map_err(|_| {
            GrammarMiss {
                nonce: [0u8; 32],
                object: Vec::new(),
                object_sha: None,
            }
        })?;
    let object = decode_object_b64(map.get("object_b64")).unwrap_or_default();
    let object_sha = decode_fixed(map.get("object").and_then(Value::as_str).unwrap_or("")).ok();
    let miss = GrammarMiss {
        nonce,
        object: object.clone(),
        object_sha,
    };
    if map.len() != KEYS.len() || KEYS.iter().any(|key| !map.contains_key(*key)) {
        return Err(miss);
    }
    let origin = match map.get("origin").and_then(Value::as_str) {
        Some(origin) => origin.to_owned(),
        None => return Err(miss),
    };
    let exp = json_u64(map.get("exp")).map_err(|_| miss.clone())?;
    let object_sha = object_sha.ok_or_else(|| miss.clone())?;
    let signature = decode_fixed(map.get("signature").and_then(Value::as_str).unwrap_or(""))
        .map_err(|_| miss.clone())?;
    if decode_object_b64(map.get("object_b64")).is_none() {
        return Err(miss);
    }
    Ok(LockedBody {
        origin,
        nonce,
        exp,
        object_sha,
        object,
        signature,
    })
}

#[derive(Clone)]
struct GrammarMiss {
    nonce: [u8; 32],
    object: Vec<u8>,
    object_sha: Option<[u8; 32]>,
}

fn decode_object_b64(value: Option<&Value>) -> Option<Vec<u8>> {
    let encoded = value.and_then(Value::as_str)?;
    if encoded.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()
}

/// Grammar of a six-field body that has already been assembled.
///
/// A wrong origin is a step-2 miss, not a step-1 miss, so this returns the
/// body even then. `None` means the JSON could not be a locked body whose
/// nonce is 64 lowercase hex: no row.
pub fn parse_for_row(body: &[u8]) -> Option<RowParse> {
    let parsed: Value = serde_json::from_slice(body).ok()?;
    let map = parsed.as_object()?;
    let nonce_text = map.get("nonce").and_then(Value::as_str)?;
    if !lower_hex(nonce_text, 64) {
        return None;
    }
    let nonce = decode_fixed(nonce_text).ok()?;
    if map.len() != KEYS.len() || KEYS.iter().any(|key| !map.contains_key(*key)) {
        return Some(RowParse::NonceOnly(nonce));
    }
    match assemble(map) {
        Ok(locked) => Some(RowParse::Body(locked)),
        Err(_) => Some(RowParse::NonceOnly(nonce)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowParse {
    /// `nonce` is 64 lowercase hex. The rest of the grammar missed, so a
    /// transport edge does not write a website outcome.
    NonceOnly([u8; 32]),
    Body(LockedBody),
}

fn json_u64(value: Option<&Value>) -> Result<u64, ()> {
    match value {
        Some(Value::Number(number)) => number.as_u64().ok_or(()),
        _ => Err(()),
    }
}

pub fn lower_hex(text: &str, n: usize) -> bool {
    text.len() == n
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn decode_fixed<const N: usize>(text: &str) -> Result<[u8; N], ()> {
    if !lower_hex(text, N * 2) {
        return Err(());
    }
    let raw = hex::decode(text).map_err(|_| ())?;
    let mut out = [0u8; N];
    out.copy_from_slice(&raw);
    Ok(out)
}

/// 64 lowercase hex in a result path. Anything else is not a nonce.
pub fn nonce_from_path(text: &str) -> Option<[u8; 32]> {
    decode_fixed(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_digest_is_one_hash() {
        let nonce = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef].repeat(4);
        let nonce: [u8; 32] = nonce.try_into().unwrap();
        let object = [0xaa; 32];
        let bytes = challenge_bytes("https://groundinglab.io", &nonce, 1_893_456_000, &object);
        assert_eq!(bytes.len(), 116);
        assert!(window_accepts(1_000_000, 1_000_120));
        assert!(!window_accepts(1_000_000, 1_000_126));
        assert!(!window_accepts(1_793_159_040, 1_893_456_000));
    }
}
