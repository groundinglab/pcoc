//! Send to my phone: one PCOC `.cbor` to the object's own npub.
//!
//! The wire is one NIP-04 kind-4 direct message, the form the Android app
//! reads (`PcocDm.kt`). The attachment is the raw object bytes: they are the
//! AES-256-CBC plaintext. The event `content` is
//! `base64(ciphertext)?iv=base64(iv)`, with one `p` tag naming the object's
//! public key. The sender is the nsec unlocked in this session. The message
//! carries no nsec. The send speaks to Nostr relays only. It never POSTs to
//! the admission gate.

use crate::hex;
use crate::keys::{PublicKey, SecretKey};
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes::Aes256;
use base64::Engine;
use cbc::{Decryptor, Encryptor};
use secp256k1::{ecdh, Parity, XOnlyPublicKey};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fmt;
use std::time::Duration;
use zeroize::{Zeroize, Zeroizing};

/// NIP-04 encrypted direct message.
pub const KIND: u32 = 4;

/// Where the message goes. One acceptance is enough. damus and Primal's
/// relay take messages up to 1 MB; nos.lol caps them at 128 KiB, so it
/// refuses most real objects, and that refusal does not fail the send.
pub const RELAYS: [&str; 3] = [
    "wss://relay.damus.io",
    "wss://relay.primal.net",
    "wss://nos.lol",
];

const RELAY_TIMEOUT: Duration = Duration::from_secs(10);

/// Why nothing was sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DmError {
    /// No nsec is unlocked in this session.
    Locked,
    /// The file has no 32-byte `public_key`.
    NoPublicKey,
    /// That field is not a secp256k1 x-only key, so it has no npub.
    NotACurvePoint,
    /// The file could not be read.
    Read(String),
    /// The event could not be built.
    Build(String),
    /// Every relay refused or did not answer. One line per relay.
    NoRelayAccepted(Vec<RelayResult>),
}

impl fmt::Display for DmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DmError::Locked => f.write_str(
                "Not sent. Paste your nsec on Produce or Sign-in first. The message is encrypted with it.",
            ),
            DmError::NoPublicKey => f.write_str(
                "Not sent. This file has no public key, so there is no npub to send it to.",
            ),
            DmError::NotACurvePoint => f.write_str(
                "Not sent. This file's public key is not a valid Nostr key, so there is no npub to send it to.",
            ),
            DmError::Read(err) => write!(f, "Not sent. The file could not be read: {err}."),
            DmError::Build(err) => write!(f, "Not sent. The message could not be built: {err}."),
            DmError::NoRelayAccepted(results) => {
                f.write_str("Not sent. No relay accepted the message, so nothing will reach your phone. ")?;
                f.write_str(&relay_summary(results))?;
                f.write_str(" Try again later, or copy pcoc.cbor to the phone and use Import PCOC.")
            }
        }
    }
}

/// One relay's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayResult {
    pub relay: String,
    /// `Ok(())` when the relay answered `OK true`. Otherwise its reason.
    pub outcome: Result<(), String>,
}

impl RelayResult {
    pub fn accepted(&self) -> bool {
        self.outcome.is_ok()
    }

    /// `relay.damus.io accepted` or `nos.lol refused (reason)`.
    pub fn line(&self) -> String {
        let host = self.relay.trim_start_matches("wss://");
        match &self.outcome {
            Ok(()) => format!("{host} accepted"),
            Err(reason) => format!("{host} refused ({reason})"),
        }
    }
}

fn relay_summary(results: &[RelayResult]) -> String {
    if results.is_empty() {
        return "No relay was tried.".into();
    }
    let lines: Vec<String> = results.iter().map(RelayResult::line).collect();
    format!("{}.", lines.join("; "))
}

/// A sent message. At least one relay accepted it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
    pub recipient_npub: String,
    pub event_id: String,
    /// Bytes of the event JSON, the size a relay checks.
    pub event_bytes: usize,
    pub results: Vec<RelayResult>,
}

impl Sent {
    /// The sentence the app shows.
    pub fn sentence(&self) -> String {
        format!(
            "Sent to {}. {} On the phone app signed in with this npub, press Approve on \"PCOC .cbor DM received.\"",
            self.recipient_npub,
            relay_summary(&self.results)
        )
    }
}

/// One prepared kind-4 event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedDm {
    pub recipient: PublicKey,
    pub content: String,
    pub event_id: String,
    pub event_json: String,
}

/// The npub of the object's `public_key`. Not a typed npub, and not the
/// session key unless that key is the one in the object.
pub fn recipient(object_bytes: &[u8]) -> Result<PublicKey, DmError> {
    let raw = crate::object_key::public_key(object_bytes).ok_or(DmError::NoPublicKey)?;
    XOnlyPublicKey::from_slice(&raw).map_err(|_| DmError::NotACurvePoint)?;
    Ok(PublicKey(raw))
}

/// Build the signed event with a fresh IV.
pub fn prepare(
    object_bytes: &[u8],
    sender: &SecretKey,
    created_at: u64,
) -> Result<PreparedDm, DmError> {
    let mut iv = [0u8; 16];
    getrandom::getrandom(&mut iv).map_err(|err| DmError::Build(err.to_string()))?;
    prepare_with_iv(object_bytes, sender, created_at, iv)
}

/// Build the signed event with the given IV. Tests freeze the IV.
pub fn prepare_with_iv(
    object_bytes: &[u8],
    sender: &SecretKey,
    created_at: u64,
    iv: [u8; 16],
) -> Result<PreparedDm, DmError> {
    let to = recipient(object_bytes)?;
    let key = shared_x(sender, &to.0)?;
    let ciphertext = encrypt_pkcs7(&key, &iv, object_bytes);
    let engine = base64::engine::general_purpose::STANDARD;
    let content = format!("{}?iv={}", engine.encode(ciphertext), engine.encode(iv));
    let (event_id, event_json) = sign_kind4(sender, &to, &content, created_at)?;
    Ok(PreparedDm {
        recipient: to,
        content,
        event_id,
        event_json,
    })
}

/// Read the bytes, build the event, and publish it to [`RELAYS`].
/// Blocking: call it off the GTK thread.
pub fn send(object_bytes: &[u8], sender: &SecretKey, created_at: u64) -> Result<Sent, DmError> {
    let prepared = prepare(object_bytes, sender, created_at)?;
    publish(&prepared, &RELAYS)
}

/// Publish one prepared event. One acceptance is a send.
pub fn publish(prepared: &PreparedDm, relays: &[&str]) -> Result<Sent, DmError> {
    let event: serde_json::Value = serde_json::from_str(&prepared.event_json)
        .map_err(|err| DmError::Build(err.to_string()))?;
    let frame = serde_json::to_string(&json!(["EVENT", event]))
        .map_err(|err| DmError::Build(err.to_string()))?;
    let results: Vec<RelayResult> = relays
        .iter()
        .filter(|url| url.starts_with("wss://") && !url.contains(ADMISSION_HOST))
        .map(|url| RelayResult {
            relay: (*url).to_owned(),
            outcome: publish_one(url, &frame, &prepared.event_id),
        })
        .collect();
    if results.iter().any(RelayResult::accepted) {
        Ok(Sent {
            recipient_npub: prepared.recipient.npub(),
            event_id: prepared.event_id.clone(),
            event_bytes: prepared.event_json.len(),
            results,
        })
    } else {
        Err(DmError::NoRelayAccepted(results))
    }
}

const ADMISSION_HOST: &str = "admission.";

fn publish_one(url: &str, frame: &str, event_id: &str) -> Result<(), String> {
    let (mut socket, _) = connect(url)?;
    socket
        .send(tungstenite::Message::Text(frame.to_owned().into()))
        .map_err(|err| short(&err.to_string()))?;
    let result = read_ok(&mut socket, event_id);
    let _ = socket.close(None);
    result
}

type Socket = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

fn connect(url: &str) -> Result<(Socket, tungstenite::handshake::client::Response), String> {
    use std::net::{TcpStream, ToSocketAddrs};
    let host = url
        .trim_start_matches("wss://")
        .split('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    let addr = (host.as_str(), 443)
        .to_socket_addrs()
        .map_err(|err| short(&err.to_string()))?
        .next()
        .ok_or_else(|| "no address".to_owned())?;
    let tcp =
        TcpStream::connect_timeout(&addr, RELAY_TIMEOUT).map_err(|err| short(&err.to_string()))?;
    let _ = tcp.set_read_timeout(Some(RELAY_TIMEOUT));
    let _ = tcp.set_write_timeout(Some(RELAY_TIMEOUT));
    tungstenite::client_tls(url, tcp).map_err(|err| short(&err.to_string()))
}

fn read_ok(socket: &mut Socket, event_id: &str) -> Result<(), String> {
    for _ in 0..16 {
        match socket.read() {
            Ok(tungstenite::Message::Text(text)) => {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(text.as_ref()) else {
                    continue;
                };
                match value.get(0).and_then(|item| item.as_str()) {
                    Some("OK") if value.get(1).and_then(|v| v.as_str()) == Some(event_id) => {
                        return if value.get(2).and_then(|item| item.as_bool()) == Some(true) {
                            Ok(())
                        } else {
                            Err(short(
                                value
                                    .get(3)
                                    .and_then(|item| item.as_str())
                                    .unwrap_or("refused"),
                            ))
                        };
                    }
                    Some("NOTICE") => {
                        if let Some(notice) = value.get(1).and_then(|v| v.as_str()) {
                            return Err(short(notice));
                        }
                    }
                    _ => {}
                }
            }
            Ok(tungstenite::Message::Ping(payload)) => {
                let _ = socket.send(tungstenite::Message::Pong(payload));
            }
            Ok(tungstenite::Message::Close(_)) => return Err("closed the connection".into()),
            Err(err) => return Err(short(&err.to_string())),
            _ => {}
        }
    }
    Err("no answer".into())
}

/// Relay reasons are shown in one line. Long ones are cut.
fn short(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= 120 {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(120).collect::<String>())
    }
}

fn encrypt_pkcs7(key: &[u8; 32], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    Encryptor::<Aes256>::new(key.into(), iv.into()).encrypt_padded_vec_mut::<Pkcs7>(data)
}

/// Decrypt as the recipient: the inverse of the send, in the form the phone uses.
pub fn nip04_decrypt(recipient: &SecretKey, sender: &PublicKey, content: &str) -> Option<Vec<u8>> {
    let (ct_b64, iv_b64) = content.split_once("?iv=")?;
    let engine = base64::engine::general_purpose::STANDARD;
    let ct = engine.decode(ct_b64).ok()?;
    let iv: [u8; 16] = engine.decode(iv_b64).ok()?.try_into().ok()?;
    let key = shared_x(recipient, &sender.0).ok()?;
    Decryptor::<Aes256>::new((&*key).into(), (&iv).into())
        .decrypt_padded_vec_mut::<Pkcs7>(&ct)
        .ok()
}

/// The NIP-04 AES key: the x coordinate of the ECDH point, not hashed.
fn shared_x(secret: &SecretKey, their_x: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, DmError> {
    let sk = secp256k1::SecretKey::from_slice(secret.scalar()).map_err(|_| DmError::Locked)?;
    let xonly = XOnlyPublicKey::from_slice(their_x).map_err(|_| DmError::NotACurvePoint)?;
    // Either Y parity gives the same x coordinate, which is the key.
    let point = secp256k1::PublicKey::from_x_only_public_key(xonly, Parity::Even);
    let mut shared = ecdh::shared_secret_point(&point, &sk);
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&shared[..32]);
    shared.zeroize();
    Ok(key)
}

fn sign_kind4(
    sender: &SecretKey,
    to: &PublicKey,
    content: &str,
    created_at: u64,
) -> Result<(String, String), DmError> {
    let sender_hex = sender.public_key().hex();
    let tags = json!([["p", to.hex()]]);
    let commit = serde_json::to_string(&json!([0, sender_hex, created_at, KIND, tags, content]))
        .map_err(|err| DmError::Build(err.to_string()))?;
    let id: [u8; 32] = Sha256::digest(commit.as_bytes()).into();
    let mut aux = [0u8; 32];
    getrandom::getrandom(&mut aux).map_err(|err| DmError::Build(err.to_string()))?;
    let sig = sender.sign_digest(&id, &aux);
    aux.zeroize();
    let id_hex = hex::encode(&id);
    let event = json!({
        "id": id_hex,
        "pubkey": sender_hex,
        "created_at": created_at,
        "kind": KIND,
        "tags": tags,
        "content": content,
        "sig": hex::encode(&sig),
    });
    let json = serde_json::to_string(&event).map_err(|err| DmError::Build(err.to_string()))?;
    Ok((id_hex, json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoints::ADMISSION;
    use crate::nip19_vector::NIP19_NSEC;
    use secp256k1::{Message, Secp256k1};

    /// CBOR map `{ "public_key": <generator x> }`. The same bytes as the
    /// Android app's `PcocDmTest.SHARED_OBJECT_HEX`.
    const SHARED_OBJECT_HEX: &str = "a16a7075626c69635f6b6579582079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    /// NIP-04 content for that object, sender the NIP-19 vector key, IV 0x11 x 16.
    /// The same string as the Android app's `PcocDmTest.SHARED_NIP04_CONTENT`,
    /// which that app decrypts with secret key 1 back to `SHARED_OBJECT_HEX`.
    const SHARED_NIP04_CONTENT: &str =
        "UFcSxGbxDZHZadW7k1QFYO52XdoH747bAJ69t0LDpboA4OZ00306bFft3LgvZ+yv?iv=EREREREREREREREREREREQ==";
    const SENDER_PUB_HEX: &str = "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e";
    const CREATED_AT: u64 = 1_760_000_000;
    const IV: [u8; 16] = [0x11; 16];

    fn object() -> Vec<u8> {
        hex::decode(SHARED_OBJECT_HEX).unwrap()
    }

    fn sender() -> SecretKey {
        SecretKey::from_nsec(NIP19_NSEC).unwrap()
    }

    /// Secret key 1: its public key is the generator x, the object's key.
    fn phone() -> SecretKey {
        let mut one = Zeroizing::new([0u8; 32]);
        one[31] = 1;
        SecretKey::from_scalar(one).unwrap()
    }

    #[test]
    fn content_matches_the_android_vector_byte_for_byte() {
        let prepared = prepare_with_iv(&object(), &sender(), CREATED_AT, IV).unwrap();
        assert_eq!(sender().public_key().hex(), SENDER_PUB_HEX);
        assert_eq!(prepared.content, SHARED_NIP04_CONTENT);
    }

    #[test]
    fn the_phone_decrypts_the_android_vector_to_the_raw_object() {
        let plain = nip04_decrypt(&phone(), &sender().public_key(), SHARED_NIP04_CONTENT).unwrap();
        assert_eq!(plain, object());
    }

    #[test]
    fn a_real_sized_object_round_trips_with_a_fresh_iv() {
        // About the size of a real object: a public key and 100 KB of filler.
        let mut bytes = hex::decode("a26466696c6c5a000186a0").unwrap();
        bytes.extend((0..100_000u32).map(|i| (i % 251) as u8));
        bytes.extend_from_slice(&hex::decode("6a7075626c69635f6b65795820").unwrap());
        bytes.extend_from_slice(&phone().public_key().0);
        assert_eq!(recipient(&bytes).unwrap(), phone().public_key());
        let prepared = prepare(&bytes, &sender(), CREATED_AT).unwrap();
        let plain = nip04_decrypt(&phone(), &sender().public_key(), &prepared.content).unwrap();
        assert_eq!(plain, bytes);
        // Under the 1 MB message limit of the two main relays.
        assert!(prepared.event_json.len() < 1_000_000);
    }

    #[test]
    fn recipient_is_the_objects_npub_not_the_session_key() {
        let prepared = prepare_with_iv(&object(), &sender(), CREATED_AT, IV).unwrap();
        assert_eq!(prepared.recipient, phone().public_key());
        assert_ne!(prepared.recipient, sender().public_key());
        let event: serde_json::Value = serde_json::from_str(&prepared.event_json).unwrap();
        assert_eq!(event["kind"], 4);
        assert_eq!(event["tags"], json!([["p", phone().public_key().hex()]]));
        assert_eq!(event["pubkey"], SENDER_PUB_HEX);
    }

    #[test]
    fn the_event_id_and_signature_verify_and_no_secret_is_in_it() {
        let prepared = prepare_with_iv(&object(), &sender(), CREATED_AT, IV).unwrap();
        let event: serde_json::Value = serde_json::from_str(&prepared.event_json).unwrap();
        let commit = serde_json::to_string(&json!([
            0,
            event["pubkey"],
            event["created_at"],
            event["kind"],
            event["tags"],
            event["content"]
        ]))
        .unwrap();
        let id = hex::encode(&Sha256::digest(commit.as_bytes()));
        assert_eq!(event["id"], id);
        assert_eq!(prepared.event_id, id);
        let secp = Secp256k1::verification_only();
        let pk = XOnlyPublicKey::from_slice(&sender().public_key().0).unwrap();
        let sig = secp256k1::schnorr::Signature::from_slice(
            &hex::decode(event["sig"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        let digest = hex::decode32(&id).unwrap();
        secp.verify_schnorr(&sig, &Message::from_digest(digest), &pk)
            .unwrap();
        assert!(!prepared.event_json.contains(NIP19_NSEC));
        assert!(!prepared
            .event_json
            .contains(&hex::encode(sender().scalar())));
        assert!(!prepared.event_json.contains(ADMISSION));
    }

    #[test]
    fn a_file_without_a_public_key_is_not_sent() {
        assert_eq!(
            prepare(b"\xa0", &sender(), CREATED_AT).unwrap_err(),
            DmError::NoPublicKey
        );
        assert_eq!(
            prepare(b"not cbor", &sender(), CREATED_AT).unwrap_err(),
            DmError::NoPublicKey
        );
    }

    #[test]
    fn relays_are_nostr_and_never_the_gate() {
        assert!(RELAYS.iter().all(|url| url.starts_with("wss://")));
        assert!(RELAYS.iter().all(|url| !url.contains(ADMISSION_HOST)));
        assert!(RELAYS.contains(&"wss://relay.damus.io"));
        assert!(RELAYS.contains(&"wss://relay.primal.net"));
    }

    #[test]
    fn one_acceptance_is_a_send_and_all_refused_says_so() {
        let refused = |relay: &str, why: &str| RelayResult {
            relay: relay.into(),
            outcome: Err(why.into()),
        };
        let all = DmError::NoRelayAccepted(vec![
            refused("wss://relay.damus.io", "timed out"),
            refused("wss://relay.primal.net", "timed out"),
            refused("wss://nos.lol", "invalid: event too large"),
        ]);
        let words = all.to_string();
        assert!(words.starts_with("Not sent. No relay accepted the message"));
        assert!(words.contains("nos.lol refused (invalid: event too large)"));
        assert!(words.contains("Import PCOC"));
        let sent = Sent {
            recipient_npub: phone().public_key().npub(),
            event_id: "00".repeat(32),
            event_bytes: 1,
            results: vec![
                RelayResult {
                    relay: "wss://relay.damus.io".into(),
                    outcome: Ok(()),
                },
                refused("wss://nos.lol", "too large"),
            ],
        };
        let words = sent.sentence();
        assert!(words.starts_with("Sent to npub1"));
        assert!(words.contains("relay.damus.io accepted; nos.lol refused (too large)."));
    }

    #[test]
    fn publishing_to_no_reachable_relay_is_an_error_not_a_send() {
        let prepared = prepare_with_iv(&object(), &sender(), CREATED_AT, IV).unwrap();
        let err = publish(&prepared, &[]).unwrap_err();
        assert_eq!(err, DmError::NoRelayAccepted(vec![]));
        let err = publish(&prepared, &["https://admission.groundinglab.io"]).unwrap_err();
        assert_eq!(err, DmError::NoRelayAccepted(vec![]));
    }
}
