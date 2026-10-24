//! Signed flush of one identity's cached login state.
//!
//! A good BIP-340 signature over `pcoc-gate-flush-v1` clears that key.
//! A signature under another key does not. The decision log is not rewritten.

#![forbid(unsafe_code)]

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use pcoc_admission::door::{LockedThorough, PendingAdmit};
use pcoc_admission::wire::{self, LockedBody};
use pcoc_admission::{Config, Door, Service};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tower::ServiceExt;

const ORIGIN: &str = "https://groundinglab.io";
const NOW: u64 = 1_700_000_000;

struct Lab {
    router: Router,
    door: Door,
    dir: PathBuf,
    peer: Mutex<u16>,
}

impl Drop for Lab {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn scratch(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "pcoc-gate-flush-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn lab_with(edit: impl FnOnce(&mut Config)) -> Lab {
    let dir = scratch("door");
    let mut cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
    edit(&mut cfg);
    let service = Service::open(cfg).expect("gate");
    let door = service.door();
    door.set_now_unix_s(Some(NOW));
    door.test_set_store_start(0);
    Lab {
        router: service.router(),
        door,
        dir,
        peer: Mutex::new(41_000),
    }
}

fn lab() -> Lab {
    lab_with(|_| {})
}

impl Lab {
    fn peer(&self) -> SocketAddr {
        let mut peer = self.peer.lock().expect("peer");
        *peer = peer.wrapping_add(1);
        SocketAddr::from(([127, 0, 0, 1], *peer))
    }
}

async fn send(lab: &Lab, mut request: Request<Body>) -> (StatusCode, Value) {
    request.extensions_mut().insert(ConnectInfo(lab.peer()));
    let response = lab.router.clone().oneshot(request).await.expect("router");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

async fn post(lab: &Lab, path: &str, body: &Value) -> (StatusCode, Value) {
    send(
        lab,
        Request::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
}

fn sign(secret: &[u8; 32], message: &[u8; 32]) -> [u8; 64] {
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &SecretKey::from_slice(secret).unwrap());
    secp.sign_schnorr_no_aux_rand(&Message::from_digest(*message), &keypair)
        .serialize()
}

fn pubkey(secret: &[u8; 32]) -> [u8; 32] {
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, &SecretKey::from_slice(secret).unwrap());
    keypair.x_only_public_key().0.serialize()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn hex32(bytes: &[u8; 32]) -> String {
    hex::encode(bytes)
}

/// Kind and key only, so the door can read the x-only key.
fn object_for(secret: &[u8; 32]) -> Vec<u8> {
    let key = pubkey(secret);
    let mut out = vec![0xa2];
    out.extend(cbor_text("kind"));
    out.extend(cbor_text("satoshi-hike-v2.0-object"));
    out.extend(cbor_text("public_key"));
    out.push(0x58);
    out.push(32);
    out.extend_from_slice(&key);
    out
}

fn cbor_text(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    if bytes.len() < 24 {
        out.push(0x60 | bytes.len() as u8);
    } else {
        out.push(0x78);
        out.push(u8::try_from(bytes.len()).expect("short text"));
    }
    out.extend_from_slice(bytes);
    out
}

fn job(secret: &[u8; 32], nonce_byte: u8) -> LockedThorough {
    let object = object_for(secret);
    let object_sha = sha256(&object);
    let mut nonce = [nonce_byte; 32];
    nonce[1] = 7;
    LockedThorough {
        nonce,
        origin: ORIGIN.to_owned(),
        object,
        submission: [nonce_byte; 32],
        public_key: pubkey(secret),
        object_sha,
        prior_accept: None,
        pending_epoch: 0,
        flush_ticket: 0,
    }
}

fn plant_accept(door: &Door, secret: &[u8; 32]) -> LockedThorough {
    let planted = job(secret, 1);
    let seq = door.test_grant_provisional(&planted);
    let out = pcoc_admission::thorough::ThoroughOut {
        seq,
        matched: true,
        runs_done: 1,
        legs_matched: 6,
        legs_total: 6,
        failed: None,
        digest_mismatch: false,
        salt: "ab".to_owned(),
        indices: vec![vec![0, 1, 2, 3, 4, 5]],
        commitment: "cd".to_owned(),
    };
    door.settle_thorough(&planted, out);
    planted
}

fn sign_in(door: &Door, secret: &[u8; 32], nonce_byte: u8) -> pcoc_admission::door::LockedSignIn {
    let object = object_for(secret);
    let object_sha = sha256(&object);
    let mut nonce = [nonce_byte; 32];
    nonce[2] = 9;
    let exp = NOW + 60;
    let message = wire::bip340_message(ORIGIN, &nonce, exp, &object_sha);
    let signature = sign(secret, &message);
    door.sign_in_locked(LockedBody {
        origin: ORIGIN.to_owned(),
        nonce,
        exp,
        object_sha,
        object,
        signature,
    })
}

async fn mint(lab: &Lab) -> Value {
    let (status, body) = post(lab, "/v1/flush/challenge", &json!({ "origin": ORIGIN })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["domain"], "pcoc-gate-flush-v1");
    assert_eq!(body["action"], "flush-claims");
    assert_eq!(body["origin"], ORIGIN);
    let link = body["link"].as_str().unwrap();
    assert!(
        link.starts_with(&format!("{ORIGIN}/flush/?v=1&nonce=")),
        "{link}"
    );
    assert!(!link.contains("public_key"), "{link}");
    body
}

fn flush_body(secret: &[u8; 32], challenge: &Value, public_key: &[u8; 32]) -> Value {
    let nonce = hex::decode(challenge["nonce"].as_str().unwrap()).unwrap();
    let nonce: [u8; 32] = nonce.try_into().unwrap();
    let exp = challenge["exp"].as_u64().unwrap();
    let origin = challenge["origin"].as_str().unwrap();
    let message = wire::flush_bip340_message(origin, &nonce, exp, public_key);
    let signature = sign(secret, &message);
    json!({
        "origin": origin,
        "nonce": hex32(&nonce),
        "exp": exp,
        "public_key": hex32(public_key),
        "signature": hex::encode(signature),
    })
}

#[tokio::test]
async fn a_good_signature_flushes_one_key_and_leaves_the_log() {
    let lab = lab();
    let holder = [4u8; 32];
    let other = [5u8; 32];
    let planted = plant_accept(&lab.door, &holder);
    let key = planted.public_key;
    let other_key = pubkey(&other);
    let source = pcoc_admission::door::pending_key(&key);
    assert!(matches!(
        lab.door.try_begin_pending(&key, &source),
        PendingAdmit::Begun { .. }
    ));
    assert_eq!(lab.door.pending_counts(&source).1, 1);
    lab.door.ban_key(&key, 1);
    lab.door.ban_key(&other_key, 2);
    assert!(lab.door.is_banned(&key));
    assert!(lab
        .door
        .prior_accept_ref(&key, &planted.object_sha)
        .is_some());
    let friendly = lab.dir.join("friendly").join(hex32(&key));
    assert!(friendly.is_file(), "accept writes the friendly file");

    let nonce = [8u8; 32];
    lab.door.consume_for_test(&nonce, planted.object_sha, NOW);
    lab.door.nonce_rows().note_holder(&nonce, key);
    lab.door
        .nonce_rows()
        .settle(&nonce, wire::OUTCOME_ACCEPTED, NOW);
    let (status, before_row) = send(
        &lab,
        Request::get(format!("/v1/sign-in/result/{}", hex32(&nonce)))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(before_row["outcome"], "accepted");

    let seq = lab.door.last_seq();
    let chain = std::fs::read(lab.dir.join("chain.jsonl")).unwrap();
    let latest = std::fs::read(lab.dir.join("latest.json")).unwrap();

    let challenge = mint(&lab).await;
    let (status, flushed) = post(&lab, "/v1/flush", &flush_body(&holder, &challenge, &key)).await;
    assert_eq!(status, StatusCode::OK, "{flushed}");
    assert_eq!(flushed, json!({ "flushed": true }));
    assert_eq!(lab.door.last_seq(), seq);
    assert_eq!(std::fs::read(lab.dir.join("chain.jsonl")).unwrap(), chain);
    assert_eq!(std::fs::read(lab.dir.join("latest.json")).unwrap(), latest);

    assert!(!lab.door.is_banned(&key));
    assert!(lab.door.is_banned(&other_key), "another key stays banned");
    assert!(lab
        .door
        .prior_accept_ref(&key, &planted.object_sha)
        .is_none());
    assert!(!friendly.exists());
    assert_eq!(lab.door.pending_counts(&source).1, 0);
    let (status, after_row) = send(
        &lab,
        Request::get(format!("/v1/sign-in/result/{}", hex32(&nonce)))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(after_row, json!({ "outcome": "pending" }));

    let banned = std::fs::read_to_string(lab.dir.join("banned_keys.json")).unwrap();
    assert!(!banned.contains(&hex32(&key)), "{banned}");
    assert!(banned.contains(&hex32(&other_key)), "{banned}");

    let again = sign_in(&lab.door, &holder, 3);
    assert_eq!(again.status, StatusCode::OK, "{:?}", again.body);
    let thorough = again.thorough.expect("fresh async sign-in");
    assert!(
        thorough.prior_accept.is_none(),
        "the old accept is not a claim"
    );
    lab.door.end_pending(&thorough.pending_key());

    let seq_after_flush = lab.door.last_seq();
    let provisional = job(&holder, 6);
    let new_seq = lab.door.test_grant_provisional(&provisional);
    assert_eq!(new_seq, seq_after_flush + 1);
    assert_eq!(
        lab.door.test_decisions().last().map(String::as_str),
        Some("provisional")
    );
    assert!(lab.door.test_decisions().len() > 2);

    let reopened = Door::open(
        &lab.dir,
        lab.door.policy().clone(),
        "http://127.0.0.1:9",
        std::time::Duration::from_secs(1),
        lab.door.served_origins().to_vec(),
    )
    .expect("reopen");
    assert!(!reopened.is_banned(&key));
    assert!(reopened.is_banned(&other_key));
    assert!(reopened
        .prior_accept_ref(&key, &planted.object_sha)
        .is_none());
    assert!(reopened.last_seq() >= new_seq);
}

#[tokio::test]
async fn a_wrong_key_is_refused_and_flushes_nothing() {
    let lab = lab();
    let holder = [6u8; 32];
    let other = [7u8; 32];
    let planted = plant_accept(&lab.door, &holder);
    let key = planted.public_key;
    let other_key = pubkey(&other);
    lab.door.ban_key(&key, 1);
    lab.door.ban_key(&other_key, 2);
    let seq = lab.door.last_seq();
    let chain = std::fs::read(lab.dir.join("chain.jsonl")).unwrap();

    let challenge = mint(&lab).await;
    let (status, body) = post(&lab, "/v1/flush", &flush_body(&other, &challenge, &key)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body, json!({ "error": { "code": "bad_signature" } }));
    assert!(lab.door.is_banned(&key));
    assert!(lab.door.is_banned(&other_key));
    assert!(lab
        .door
        .prior_accept_ref(&key, &planted.object_sha)
        .is_some());
    assert_eq!(lab.door.last_seq(), seq);

    let nonce = hex::decode(challenge["nonce"].as_str().unwrap()).unwrap();
    let nonce: [u8; 32] = nonce.try_into().unwrap();
    let exp = challenge["exp"].as_u64().unwrap();
    let sign_in_message = wire::bip340_message(ORIGIN, &nonce, exp, &planted.object_sha);
    let (status, body) = post(
        &lab,
        "/v1/flush",
        &json!({
            "origin": ORIGIN,
            "nonce": hex32(&nonce),
            "exp": exp,
            "public_key": hex32(&key),
            "signature": hex::encode(sign(&holder, &sign_in_message)),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body, json!({ "error": { "code": "bad_signature" } }));
    assert!(lab.door.is_banned(&key));
    assert_eq!(std::fs::read(lab.dir.join("chain.jsonl")).unwrap(), chain);

    let (status, flushed) = post(&lab, "/v1/flush", &flush_body(&holder, &challenge, &key)).await;
    assert_eq!(status, StatusCode::OK, "{flushed}");
    assert_eq!(flushed["flushed"], true);
    assert!(!lab.door.is_banned(&key));
    assert!(lab.door.is_banned(&other_key));

    let (status, again) = post(&lab, "/v1/flush", &flush_body(&holder, &challenge, &key)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{again}");
    assert_eq!(again, json!({ "error": { "code": "malformed" } }));
    assert!(lab.door.is_banned(&other_key));
}

#[tokio::test]
async fn flush_clears_the_door_session_cap_for_that_key() {
    let lab = lab_with(|cfg| {
        cfg.policy.verify_cap_per_door_session = Some(1);
        cfg.policy.pending_per_source = 8;
    });
    let holder = [8u8; 32];
    let first = sign_in(&lab.door, &holder, 1);
    assert!(first.thorough.is_some(), "{:?}", first.body);
    lab.door.end_pending(&first.thorough.unwrap().pending_key());
    let second = sign_in(&lab.door, &holder, 2);
    assert!(
        second.thorough.is_none(),
        "the door-session cap blocks a second verify"
    );
    assert_eq!(client_code(&second.body), "delivery", "{:?}", second.body);
    assert!(lab
        .door
        .test_reasons()
        .iter()
        .any(|reason| reason == "verify_cap_per_door_session"));
    let seq = lab.door.last_seq();
    let challenge = mint(&lab).await;
    let key = pubkey(&holder);
    let (status, flushed) = post(&lab, "/v1/flush", &flush_body(&holder, &challenge, &key)).await;
    assert_eq!(status, StatusCode::OK, "{flushed}");
    assert_eq!(lab.door.last_seq(), seq);
    let third = sign_in(&lab.door, &holder, 3);
    assert!(
        third.thorough.is_some(),
        "after flush the same key can verify again: {:?}",
        third.body
    );
    assert!(third.thorough.unwrap().prior_accept.is_none());
}

#[tokio::test]
async fn flush_routes_are_404_when_the_flag_is_off() {
    let lab = lab_with(|cfg| cfg.flush_claims = false);
    for path in ["/v1/flush/challenge", "/v1/flush"] {
        let (status, body) = post(&lab, path, &json!({ "origin": ORIGIN })).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path} {body}");
        assert_eq!(body["error"]["code"], "not_found");
    }
}

fn client_code(body: &Value) -> &'static str {
    if body.get("delivery").and_then(|item| item.as_str()) == Some("ok") {
        "delivery"
    } else if body.pointer("/error/code").and_then(|item| item.as_str()) == Some("bad_signature") {
        "bad_signature"
    } else if body.pointer("/error/code").and_then(|item| item.as_str()) == Some("malformed") {
        "malformed"
    } else {
        "other"
    }
}
