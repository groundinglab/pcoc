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

/// Flush challenge domain. Distinct from [`DOMAIN`] so a sign-in signature
/// is not a flush, and a flush signature is not a sign-in.
pub const FLUSH_DOMAIN_STR: &str = "pcoc-gate-flush-v1";
pub const FLUSH_DOMAIN: &[u8] = FLUSH_DOMAIN_STR.as_bytes();

/// Label on the mint response. It is not part of `challenge_bytes`.
pub const FLUSH_ACTION: &str = "flush-claims";

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

/// SPEC §15.1. Three comparisons, in wide integers so a short `exp` does not wrap:
/// `now < exp + skew`, `exp <= now + lifetime + skew`, and
/// `exp − lifetime >= nonce_store_start + skew`.
pub fn window_accepts(
    now: u64,
    exp: u64,
    lifetime_s: u64,
    skew_s: u64,
    store_start_unix_s: u64,
) -> bool {
    let now = i128::from(now);
    let exp = i128::from(exp);
    let lifetime = i128::from(lifetime_s);
    let skew = i128::from(skew_s);
    let start = i128::from(store_start_unix_s);
    now < exp + skew && exp <= now + lifetime + skew && exp - lifetime >= start + skew
}

pub fn challenge_bytes(origin: &str, nonce: &[u8; 32], exp: u64, object: &[u8; 32]) -> Vec<u8> {
    tagged_challenge(DOMAIN, origin, nonce, exp, object)
}

pub fn bip340_message(origin: &str, nonce: &[u8; 32], exp: u64, object: &[u8; 32]) -> [u8; 32] {
    digest32(&challenge_bytes(origin, nonce, exp, object))
}

/// Flush `challenge_bytes`. Same layout as sign-in, with [`FLUSH_DOMAIN`]
/// and the signer's 32-byte x-only public key in the slot sign-in uses for
/// `object_sha256`. The mint response does not carry the key. The signer
/// appends it. The link does not carry it.
pub fn flush_challenge_bytes(
    origin: &str,
    nonce: &[u8; 32],
    exp: u64,
    public_key: &[u8; 32],
) -> Vec<u8> {
    tagged_challenge(FLUSH_DOMAIN, origin, nonce, exp, public_key)
}

pub fn flush_bip340_message(
    origin: &str,
    nonce: &[u8; 32],
    exp: u64,
    public_key: &[u8; 32],
) -> [u8; 32] {
    digest32(&flush_challenge_bytes(origin, nonce, exp, public_key))
}

fn digest32(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn tagged_challenge(
    domain: &[u8],
    origin: &str,
    nonce: &[u8; 32],
    exp: u64,
    tail: &[u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(domain.len() + 2 + origin.len() + 32 + 8 + 32);
    out.extend_from_slice(domain);
    out.push(0x00);
    let n = u8::try_from(origin.len()).unwrap_or(u8::MAX);
    out.push(n);
    out.extend_from_slice(&origin.as_bytes()[..usize::from(n).min(origin.len())]);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&exp.to_be_bytes());
    out.extend_from_slice(tail);
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

/// Body of `POST /v1/flush`. Five keys, each once. No object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlushSubmit {
    pub origin: String,
    pub nonce: [u8; 32],
    pub exp: u64,
    pub public_key: [u8; 32],
    pub signature: [u8; 64],
}

const FLUSH_KEYS: [&str; 5] = ["origin", "nonce", "exp", "public_key", "signature"];

/// `None` is malformed: not one JSON object, or the five-field grammar missed.
pub fn parse_flush_submit(body: &[u8]) -> Option<FlushSubmit> {
    let parsed: Value = serde_json::from_slice(body).ok()?;
    let map = parsed.as_object()?;
    if map.len() != FLUSH_KEYS.len() || FLUSH_KEYS.iter().any(|key| !map.contains_key(*key)) {
        return None;
    }
    let origin = map.get("origin").and_then(Value::as_str)?.to_owned();
    let nonce = decode_fixed(map.get("nonce").and_then(Value::as_str).unwrap_or("")).ok()?;
    let exp = json_u64(map.get("exp")).ok()?;
    let public_key =
        decode_fixed(map.get("public_key").and_then(Value::as_str).unwrap_or("")).ok()?;
    let signature =
        decode_fixed(map.get("signature").and_then(Value::as_str).unwrap_or("")).ok()?;
    Some(FlushSubmit {
        origin,
        nonce,
        exp,
        public_key,
        signature,
    })
}

/// Mint body. `{"origin":"..."}` names the site. `{}` asks the door to use
/// its sole served origin. Any other shape is malformed.
pub enum FlushMint {
    Origin(String),
    SoleOrigin,
}

pub fn parse_flush_mint(body: &[u8]) -> Option<FlushMint> {
    if body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Some(FlushMint::SoleOrigin);
    }
    let parsed: Value = serde_json::from_slice(body).ok()?;
    let map = parsed.as_object()?;
    if map.is_empty() {
        return Some(FlushMint::SoleOrigin);
    }
    if map.len() != 1 || !map.contains_key("origin") {
        return None;
    }
    let origin = map.get("origin").and_then(Value::as_str)?.to_owned();
    if origin.is_empty() {
        return None;
    }
    Some(FlushMint::Origin(origin))
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
        assert!(window_accepts(1_000_000, 1_000_120, 120, 5, 0));
        assert!(!window_accepts(1_000_000, 1_000_126, 120, 5, 0));
        assert!(!window_accepts(1_793_159_040, 1_893_456_000, 120, 5, 0));
        // Issued at store_start + skew passes. One second earlier does not.
        assert!(window_accepts(1_125, 1_125, 120, 5, 1_000));
        assert!(!window_accepts(1_124, 1_124, 120, 5, 1_000));
    }

    #[test]
    fn flush_domain_is_not_the_sign_in_domain() {
        assert_ne!(FLUSH_DOMAIN, DOMAIN);
        assert_eq!(FLUSH_DOMAIN, b"pcoc-gate-flush-v1");
        let nonce = [0x11; 32];
        let key = [0x22; 32];
        let origin = "https://groundinglab.io";
        let flush = flush_challenge_bytes(origin, &nonce, 1_893_456_000, &key);
        let sign_in = challenge_bytes(origin, &nonce, 1_893_456_000, &key);
        assert_ne!(flush, sign_in);
        assert_eq!(flush.len(), 115);
        assert!(flush.starts_with(FLUSH_DOMAIN));
        assert_eq!(flush[FLUSH_DOMAIN.len()], 0x00);
    }
}
