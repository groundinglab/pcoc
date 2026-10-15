//! NIP-01 event id and BIP-340. No Nostr stack.
//!
//! The id preimage is the JSON array `[0, pubkey, created_at, kind, tags, content]`.
//! `serde_json` writes that array with no extra space and does not escape `/`.

use crate::error::Fail;
use crate::timeutil::valid_day;
use bech32::{FromBase32, ToBase32, Variant};
use secp256k1::schnorr::Signature;
use secp256k1::{Keypair, Message, Secp256k1, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const KIND_HTTP_AUTH: u32 = 27_235;
pub const KIND_BADGE_AWARD: u32 = 8;
pub const KIND_BADGE_DEFINITION: u32 = 30_009;
pub const KIND_NAME: u32 = 1_742;

/// The gate host that serves the public log line. The award `r` tag cites it.
pub const ADMISSION_LOG_ORIGIN: &str = "https://admission.groundinglab.io";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    pub created_at: i64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

impl Event {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("event json")
    }

    pub fn from_json(raw: &str) -> Result<Self, Fail> {
        serde_json::from_str(raw).map_err(|_| Fail::malformed())
    }
}

/// A verified kind 27235 badge claim. The bytes are the caller's event.
#[derive(Clone, Debug)]
pub struct BadgeClaim {
    pub event: Event,
    pub nonce: String,
    pub day: String,
    pub seq: i64,
}

pub fn event_hash(
    pubkey: &str,
    created_at: i64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> [u8; 32] {
    let preimage =
        serde_json::to_vec(&(0_u8, pubkey, created_at, kind, tags, content)).expect("id preimage");
    let digest = Sha256::digest(preimage);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn sign_event(
    secret: &[u8; 32],
    created_at: i64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: &str,
) -> Result<Event, String> {
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_seckey_slice(&secp, secret)
        .map_err(|_| "issuer key is not a scalar".to_owned())?;
    let (xonly, _) = keypair.x_only_public_key();
    let pubkey = hex::encode(xonly.serialize());
    let id_bytes = event_hash(&pubkey, created_at, kind, &tags, content);
    let sig = secp.sign_schnorr_no_aux_rand(&Message::from_digest(id_bytes), &keypair);
    Ok(Event {
        id: hex::encode(id_bytes),
        pubkey,
        created_at,
        kind,
        tags,
        content: content.to_owned(),
        sig: hex::encode(sig.as_ref()),
    })
}

pub fn verify_event(event: &Event) -> Result<[u8; 32], Fail> {
    if !is_lower_hex(&event.id, 64)
        || !is_lower_hex(&event.pubkey, 64)
        || !is_lower_hex(&event.sig, 128)
    {
        return Err(Fail::bad_signature());
    }
    let id_bytes = event_hash(
        &event.pubkey,
        event.created_at,
        event.kind,
        &event.tags,
        &event.content,
    );
    if hex::encode(id_bytes) != event.id {
        return Err(Fail::bad_signature());
    }
    let pk_bytes = hex::decode(&event.pubkey).map_err(|_| Fail::bad_signature())?;
    let sig_bytes = hex::decode(&event.sig).map_err(|_| Fail::bad_signature())?;
    let pk = XOnlyPublicKey::from_slice(&pk_bytes).map_err(|_| Fail::bad_signature())?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| Fail::bad_signature())?;
    let secp = Secp256k1::verification_only();
    secp.verify_schnorr(&sig, &Message::from_digest(id_bytes), &pk)
        .map_err(|_| Fail::bad_signature())?;
    Ok(id_bytes)
}

pub fn pubkey_hex(secret: &[u8; 32]) -> Result<String, String> {
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_seckey_slice(&secp, secret)
        .map_err(|_| "issuer key is not a scalar".to_owned())?;
    let (xonly, _) = keypair.x_only_public_key();
    Ok(hex::encode(xonly.serialize()))
}

pub fn npub_from_hex(pubkey_hex: &str) -> Result<String, String> {
    let bytes = decode_32(pubkey_hex).ok_or_else(|| "public key is not 32 bytes".to_owned())?;
    bech32::encode("npub", bytes.to_base32(), Variant::Bech32)
        .map_err(|_| "npub encode failed".to_owned())
}

/// 64 hex characters, or a bech32 string with the `nsec` prefix. The returned scalar is zeroed on drop.
pub fn parse_secret(raw: &str) -> Option<Zeroizing<[u8; 32]>> {
    let text = raw.trim();
    if text.len() > 120 || text.chars().any(|c| c.is_control()) {
        return None;
    }
    let bytes = if is_lower_hex(text, 64) || is_hex(text, 64) {
        decode_32(text)?
    } else {
        secret_from_nsec(text)?
    };
    let secp = Secp256k1::signing_only();
    Keypair::from_seckey_slice(&secp, &bytes).ok()?;
    Some(Zeroizing::new(bytes))
}

fn secret_from_nsec(text: &str) -> Option<[u8; 32]> {
    let (hrp, data, variant) = bech32::decode(text).ok()?;
    if hrp != "nsec" || variant != Variant::Bech32 {
        return None;
    }
    let bytes = Vec::<u8>::from_base32(&data).ok()?;
    let out: [u8; 32] = bytes.as_slice().try_into().ok()?;
    Some(out)
}

/// The hex public key in an `npub1…` string, lower case.
pub fn hex_from_npub(text: &str) -> Option<String> {
    let (hrp, data, variant) = bech32::decode(text.trim()).ok()?;
    if hrp != "npub" || variant != Variant::Bech32 {
        return None;
    }
    let bytes = Vec::<u8>::from_base32(&data).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// A badge request: `{"npub","log_day","log_seq"}`. The sign-in that wrote
/// that gate line is the proof of key control; no second signature.
#[derive(Clone, Debug)]
pub struct BadgeRequest {
    pub pubkey: String,
    pub day: String,
    pub seq: i64,
}

pub fn parse_badge_request(body: &[u8]) -> Result<BadgeRequest, Fail> {
    if body.len() > 4 * 1024 {
        return Err(Fail::malformed());
    }
    let value: Value = serde_json::from_slice(body).map_err(|_| Fail::malformed())?;
    let obj = value.as_object().ok_or_else(Fail::malformed)?;
    let npub = obj.get("npub").and_then(Value::as_str).ok_or_else(Fail::malformed)?;
    let pubkey = hex_from_npub(npub).ok_or_else(Fail::malformed)?;
    let day = obj.get("log_day").and_then(Value::as_str).ok_or_else(Fail::malformed)?;
    let b = day.as_bytes();
    let day_ok = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if !day_ok {
        return Err(Fail::malformed());
    }
    let seq = obj.get("log_seq").and_then(Value::as_i64).ok_or_else(Fail::malformed)?;
    if seq < 1 {
        return Err(Fail::malformed());
    }
    Ok(BadgeRequest { pubkey, day: day.to_owned(), seq })
}

/// Parse and verify a badge claim. `now` and `skew` are the claims clock.
pub fn parse_badge_claim(
    body: &[u8],
    claims_origin: &str,
    now: i64,
    skew: i64,
) -> Result<BadgeClaim, Fail> {
    if body.len() > 64 * 1024 {
        return Err(Fail::malformed());
    }
    let value: Value = serde_json::from_slice(body).map_err(|_| Fail::malformed())?;
    let obj = value.as_object().ok_or_else(Fail::malformed)?;
    let kind = json_u32(obj.get("kind"))?;
    if kind != KIND_HTTP_AUTH {
        return Err(Fail::malformed());
    }
    let content = obj
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(Fail::malformed)?;
    if !content.is_empty() {
        return Err(Fail::malformed());
    }
    let created_at = json_i64(obj.get("created_at"))?;
    let pubkey = obj
        .get("pubkey")
        .and_then(Value::as_str)
        .ok_or_else(Fail::malformed)?;
    if !is_lower_hex(pubkey, 64) {
        return Err(Fail::malformed());
    }
    let tags = parse_tags(obj.get("tags"))?;
    let (nonce, day, seq) = read_badge_tags(&tags, claims_origin)?;
    if created_at < now.saturating_sub(skew) || created_at > now.saturating_add(skew) {
        return Err(Fail::expired());
    }
    let id = obj
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(Fail::malformed)?;
    let sig = obj
        .get("sig")
        .and_then(Value::as_str)
        .ok_or_else(Fail::malformed)?;
    if !is_lower_hex(id, 64) || !is_lower_hex(sig, 128) {
        return Err(Fail::bad_signature());
    }
    let event = Event {
        id: id.to_owned(),
        pubkey: pubkey.to_owned(),
        created_at,
        kind,
        tags,
        content: String::new(),
        sig: sig.to_owned(),
    };
    verify_event(&event)?;
    Ok(BadgeClaim {
        event,
        nonce,
        day,
        seq,
    })
}

fn read_badge_tags(
    tags: &[Vec<String>],
    claims_origin: &str,
) -> Result<(String, String, i64), Fail> {
    if tags
        .iter()
        .any(|tag| tag.first().map(String::as_str) == Some("name"))
    {
        return Err(Fail::wrong_action());
    }
    if tags.len() != 5 {
        return Err(Fail::malformed());
    }
    let names: Vec<&str> = tags.iter().map(|tag| tag[0].as_str()).collect();
    if names != ["u", "method", "action", "nonce", "access"] {
        return Err(Fail::malformed());
    }
    if tags[0].len() != 2
        || tags[1].len() != 2
        || tags[2].len() != 2
        || tags[3].len() != 2
        || tags[4].len() != 3
    {
        return Err(Fail::malformed());
    }
    let expected = format!("{claims_origin}/badge");
    let u = tags[0][1].as_str();
    if u != expected {
        match (origin_of(u), origin_of(claims_origin)) {
            (Some(got), Some(want)) if got != want => return Err(Fail::wrong_origin()),
            (None, _) => return Err(Fail::malformed()),
            _ => return Err(Fail::wrong_action()),
        }
    }
    if tags[1][1] != "POST" {
        return Err(Fail::malformed());
    }
    if tags[2][1] != "badge" {
        return Err(Fail::wrong_action());
    }
    let nonce = tags[3][1].as_str();
    if !is_lower_hex(nonce, 64) {
        return Err(Fail::malformed());
    }
    let day = tags[4][1].as_str();
    if !valid_day(day) {
        return Err(Fail::malformed());
    }
    let seq = parse_seq(&tags[4][2])?;
    Ok((nonce.to_owned(), day.to_owned(), seq))
}

fn parse_seq(raw: &str) -> Result<i64, Fail> {
    if raw.is_empty() || raw.len() > 18 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Fail::malformed());
    }
    if raw.starts_with('0') {
        return Err(Fail::malformed());
    }
    let seq: i64 = raw.parse().map_err(|_| Fail::malformed())?;
    if seq < 1 {
        return Err(Fail::malformed());
    }
    Ok(seq)
}

fn parse_tags(value: Option<&Value>) -> Result<Vec<Vec<String>>, Fail> {
    let Some(Value::Array(tags)) = value else {
        return Err(Fail::malformed());
    };
    let mut out = Vec::with_capacity(tags.len());
    for tag in tags {
        let Value::Array(parts) = tag else {
            return Err(Fail::malformed());
        };
        if parts.is_empty() {
            return Err(Fail::malformed());
        }
        let mut row = Vec::with_capacity(parts.len());
        for part in parts {
            let Some(text) = part.as_str() else {
                return Err(Fail::malformed());
            };
            row.push(text.to_owned());
        }
        out.push(row);
    }
    Ok(out)
}

fn origin_of(raw: &str) -> Option<String> {
    let url = reqwest::Url::parse(raw).ok()?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn json_i64(value: Option<&Value>) -> Result<i64, Fail> {
    match value {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(Fail::malformed),
        _ => Err(Fail::malformed()),
    }
}

fn json_u32(value: Option<&Value>) -> Result<u32, Fail> {
    let n = json_i64(value)?;
    u32::try_from(n).map_err(|_| Fail::malformed())
}

pub fn is_lower_hex(text: &str, chars: usize) -> bool {
    text.len() == chars
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn is_hex(text: &str, chars: usize) -> bool {
    text.len() == chars && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn decode_32(text: &str) -> Option<[u8; 32]> {
    let raw = hex::decode(text).ok()?;
    raw.try_into().ok()
}

pub fn r_tag(day: &str, seq: i64) -> String {
    format!("{ADMISSION_LOG_ORIGIN}/log/{day}.jsonl#{seq}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[31] = 1;
        bytes
    }

    #[test]
    fn signed_event_verifies_and_a_flipped_sig_does_not() {
        let tags = vec![vec!["d".into(), "groundinglab-admitted".into()]];
        let event = sign_event(&secret(), 1_700_000_000, KIND_BADGE_DEFINITION, tags, "").unwrap();
        assert!(verify_event(&event).is_ok());
        assert_eq!(event.pubkey, pubkey_hex(&secret()).unwrap());
        let mut bad = event.clone();
        bad.sig = "0".repeat(128);
        // 64 zero bytes is not a valid BIP-340 signature, or it is the wrong one.
        assert_eq!(verify_event(&bad), Err(Fail::bad_signature()));
        assert!(npub_from_hex(&event.pubkey).unwrap().starts_with("npub1"));
    }
}
