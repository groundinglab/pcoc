//! Locked §15.1 sign-in wire.
//!
//! Loads the pinned `tests/vectors/sign-in.json`. Every request is in-process.
//! Nothing here dials a deployed gate, a live host, or Bari.
//!
//! The clock is pinned. The published example expiry `1893456000` is outside
//! a window near 2026-10-10. A fresh `exp = now + 120` is inside it.

#![forbid(unsafe_code)]

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use base64::Engine as _;
use http_body_util::BodyExt;
use pcoc_admission::{Config, Door, Service};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;

const NOW_2026_10_10: u64 = 1_791_590_400;

fn vector() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/sign-in.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn scratch(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "pcoc-gate-locked-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

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

fn lab_at(now: u64) -> Lab {
    let dir = scratch("door");
    let cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
    let service = Service::open(cfg).expect("gate");
    let door = service.door();
    door.set_now_unix_s(Some(now));
    Lab {
        router: service.router(),
        door,
        dir,
        peer: Mutex::new(40_000),
    }
}

fn lab() -> Lab {
    lab_at(NOW_2026_10_10)
}

impl Lab {
    fn peer(&self) -> SocketAddr {
        let mut peer = self.peer.lock().expect("peer");
        *peer = peer.wrapping_add(1);
        SocketAddr::from(([127, 0, 0, 1], *peer))
    }
}

async fn send(lab: &Lab, mut request: Request<Body>) -> (StatusCode, Value, String) {
    request.extensions_mut().insert(ConnectInfo(lab.peer()));
    let response = lab.router.clone().oneshot(request).await.expect("router");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value, text)
}

async fn post_raw(lab: &Lab, content_type: &str, body: String) -> (StatusCode, Value, String) {
    send(
        lab,
        Request::post("/v1/sign-in")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .unwrap(),
    )
    .await
}

async fn post_json(lab: &Lab, body: &Value) -> (StatusCode, Value, String) {
    post_raw(lab, "application/json", body.to_string()).await
}

async fn result(lab: &Lab, nonce: &str) -> (StatusCode, Value) {
    let (status, value, _) = send(
        lab,
        Request::get(format!("/v1/sign-in/result/{nonce}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    (status, value)
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
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

#[derive(Clone)]
struct Signed {
    secret: [u8; 32],
    nonce: [u8; 32],
    exp: u64,
    object: Vec<u8>,
}

impl Signed {
    fn fresh(object: Vec<u8>, exp: u64) -> Self {
        static N: AtomicU64 = AtomicU64::new(1);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let mut nonce = [0u8; 32];
        nonce[..8].copy_from_slice(&n.to_be_bytes());
        let mut secret = [0x11; 32];
        secret[31] = (n as u8).wrapping_add(1);
        Self {
            secret,
            nonce,
            exp,
            object,
        }
    }

    fn body_signed(&self, sign_object_field: bool) -> Value {
        let object_sha = if sign_object_field {
            sha256(&self.object)
        } else {
            [0xaa; 32]
        };
        let message = bip340(&self.nonce, self.exp, &object_sha);
        let signature = sign(&self.secret, &message);
        json!({
            "origin": "https://groundinglab.io",
            "nonce": hex32(&self.nonce),
            "exp": self.exp,
            "object": hex32(&object_sha),
            "object_b64": b64(&self.object),
            "signature": hex::encode(signature),
        })
    }

    fn body(&self) -> Value {
        self.body_signed(true)
    }
}

fn bip340(nonce: &[u8; 32], exp: u64, object: &[u8; 32]) -> [u8; 32] {
    let mut raw = Vec::new();
    raw.extend_from_slice(b"pcoc-gate-signin-v1");
    raw.push(0);
    let origin = b"https://groundinglab.io";
    raw.push(origin.len() as u8);
    raw.extend_from_slice(origin);
    raw.extend_from_slice(nonce);
    raw.extend_from_slice(&exp.to_be_bytes());
    raw.extend_from_slice(object);
    sha256(&raw)
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

fn cbor_text(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    // Additional info 0..=23 is the length. 24 means the next byte is the length.
    // `satoshi-hike-v2.0-object` is 24 bytes, so it takes that extra byte.
    if bytes.len() < 24 {
        out.push(0x60 | bytes.len() as u8);
    } else {
        out.push(0x78);
        out.push(u8::try_from(bytes.len()).expect("short text"));
    }
    out.extend_from_slice(bytes);
    out
}

/// Kind and key only. The reference yields the key. It is not a full hike,
/// so step 8 stores `rejected` without a section 14 run.
fn v2_object(secret: &[u8; 32]) -> Vec<u8> {
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

/// The earlier kind plus a 32-byte key, so step 7 can run after a signature.
fn earlier_kind_with_key(secret: &[u8; 32]) -> Vec<u8> {
    let key = pubkey(secret);
    let mut out = vec![0xa2];
    out.extend(cbor_text("kind"));
    out.extend(cbor_text("satoshi-hike-v1.35-object"));
    out.extend(cbor_text("public_key"));
    out.push(0x58);
    out.push(32);
    out.extend_from_slice(&key);
    out
}

fn v135_object() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/earlier-kind.cbor"),
    )
    .expect("earlier kind")
}

fn assert_only_code(status: StatusCode, body: &Value, text: &str, code: &str) {
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    assert_eq!(body, &json!({"error": {"code": code}}), "{text}");
    assert!(!text.contains("message"), "{text}");
    assert!(!text.contains("leg"), "{text}");
}

fn assert_delivery(status: StatusCode, body: &Value) {
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, &json!({"delivery": "ok"}), "{body}");
}

async fn outcome_of(lab: &Lab, nonce: &[u8; 32]) -> String {
    let (status, body) = result(lab, &hex32(nonce)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let obj = body.as_object().expect("result object");
    let outcome = body["outcome"].as_str().expect("outcome").to_owned();
    match outcome.as_str() {
        // Unknown nonce: one field. A consumed row still pending adds submitted.
        "pending" => match body.get("submitted") {
            None => assert_eq!(obj.len(), 1, "{body}"),
            Some(flag) => {
                assert_eq!(obj.len(), 2, "{body}");
                assert_eq!(flag, &json!(true), "{body}");
            }
        },
        // Accepted publishes measurements. It does not publish the floor.
        "accepted" => {
            assert!(obj.contains_key("object_sha256"), "{body}");
            assert!(obj.get("submitted").is_none(), "{body}");
            assert!(obj.get("leg_rate_floor").is_none(), "{body}");
            for key in obj.keys() {
                assert!(
                    matches!(
                        key.as_str(),
                        "outcome"
                            | "public_key"
                            | "object_sha256"
                            | "legs"
                            | "span_s"
                            | "log_day"
                            | "log_seq"
                    ),
                    "unexpected result field {key} in {body}"
                );
            }
        }
        _ => assert_eq!(obj.len(), 1, "{body}"),
    }
    outcome
}

/// Wait until the website token leaves `pending`, or return the token it has.
async fn settled(lab: &Lab, nonce: &[u8; 32]) -> String {
    for _ in 0..50 {
        let token = outcome_of(lab, nonce).await;
        if token != "pending" {
            return token;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    outcome_of(lab, nonce).await
}

#[tokio::test]
async fn a_fresh_six_field_post_is_delivery_ok() {
    let doc = vector();
    assert_eq!(doc["lifetime_s"], 120);
    assert_eq!(doc["skew_s"], 5);
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;
    let draft = Signed::fresh(Vec::new(), exp);
    let signed = Signed {
        object: v2_object(&draft.secret),
        ..draft
    };
    let posted = signed.body();
    let yielded = hike::yielded_public_key(&signed.object);
    let (status, body, text) = post_json(&lab, &posted).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "posted {posted} yielded {yielded:?} key {} response {text}",
        hex::encode(pubkey(&signed.secret))
    );
    assert_eq!(body, json!({"delivery": "ok"}), "{text}");
    assert!(!text.contains("outcome"), "{text}");
    assert!(!text.contains("provisional"), "{text}");
    let _ = doc;
}

/// The website link carries nonce and exp only. The body still carries
/// `object`. The door rebuilds challenge_bytes from the body and the result
/// leaves `pending`.
#[tokio::test]
async fn an_object_free_link_signs_in_with_the_object_in_the_body() {
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;
    let draft = Signed::fresh(Vec::new(), exp);
    let signed = Signed {
        object: v2_object(&draft.secret),
        ..draft
    };
    let link = format!(
        "https://groundinglab.io/sign-in/?v=1&nonce={}&exp={exp}",
        hex32(&signed.nonce)
    );
    assert!(!link.contains("object="), "{link}");
    assert!(link.starts_with("https://groundinglab.io/sign-in/?v=1&nonce="));
    let posted = signed.body();
    assert_eq!(posted["nonce"], json!(hex32(&signed.nonce)));
    assert_eq!(posted["exp"], json!(exp));
    let object_field = posted["object"].as_str().unwrap();
    assert_eq!(object_field.len(), 64);
    assert_ne!(object_field, "aa".repeat(32));
    assert!(posted["object_b64"].as_str().unwrap().ends_with('='));
    let (status, body, text) = post_json(&lab, &posted).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(body, json!({"delivery": "ok"}), "{text}");
    let token = settled(&lab, &signed.nonce).await;
    assert_ne!(token, "pending", "result stayed pending");
    assert_eq!(token, "rejected");
}

#[tokio::test]
async fn a_two_field_body_is_malformed() {
    let lab = lab();
    let body = json!({
        "object_b64": "AA==",
        "signature": "ab".repeat(64),
    });
    let (status, value, text) = post_json(&lab, &body).await;
    assert_only_code(status, &value, &text, "malformed");
    assert!(!text.contains("provisional"), "{text}");
}

#[tokio::test]
async fn retired_mint_routes_stay_404() {
    let lab = lab();
    for path in ["/v1/sign-in/challenge", "/v1/challenge", "/v1/admit"] {
        let (status, body, _) =
            send(&lab, Request::post(path).body(Body::from("{}")).unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path} {body}");
        assert_eq!(body["error"]["code"], "not_found");
    }
}

#[tokio::test]
async fn grammar_and_window_misses_are_malformed_with_no_message() {
    let doc = vector();
    let lab = lab();
    for item in doc["bodies"].as_array().unwrap() {
        if item["class"] != "malformed" {
            continue;
        }
        let (status, body, text) = post_json(&lab, &item["body"]).await;
        assert_only_code(status, &body, &text, "malformed");
        let note = item["note"].as_str().unwrap_or("");
        assert!(
            !text.contains("message"),
            "{note} carried a message: {text}"
        );
    }
    let example = &doc["example"];
    let published = json!({
        "origin": example["origin"],
        "nonce": example["nonce_hex"],
        "exp": example["expiry_unix_s"],
        "object": example["object_sha256_hex"],
        "object_b64": example["object_b64"],
        "signature": example["signature_hex"],
    });
    let (status, body, text) = post_json(&lab, &published).await;
    assert_only_code(status, &body, &text, "malformed");
    assert_eq!(example["window_accepts_example_at_2026_10_10"], false);

    let fresh = Signed::fresh(vec![0xaa; 32], NOW_2026_10_10 + 120);
    // The example object bytes do not hash to the object field. A fresh exp
    // still has to pass the window before that hash check.
    let mut body = fresh.body_signed(false);
    body["exp"] = json!(NOW_2026_10_10 + 120);
    let (status, value, text) = post_json(&lab, &body).await;
    assert_ne!(
        value["error"]["code"], "malformed",
        "window rejected a fresh exp: {text}"
    );
    let _ = status;
}

#[tokio::test]
async fn hash_miss_missing_key_and_bad_signature_are_one_code() {
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;

    let mismatch = Signed::fresh(vec![0xaa; 32], exp);
    let (status, body, text) = post_json(&lab, &mismatch.body_signed(false)).await;
    assert_only_code(status, &body, &text, "bad_signature");
    assert_eq!(outcome_of(&lab, &mismatch.nonce).await, "pending");

    let no_key = Signed::fresh(b"this object has no key in it at all!!".to_vec(), exp);
    let (status, body, text) = post_json(&lab, &no_key.body()).await;
    assert_only_code(status, &body, &text, "bad_signature");

    let mut bad = Signed::fresh(Vec::new(), exp);
    bad.object = v2_object(&[0x31; 32]);
    bad.secret = [0x32; 32];
    let (status, body, text) = post_json(&lab, &bad.body()).await;
    assert_only_code(status, &body, &text, "bad_signature");
    let joined = format!("{body}");
    assert!(!joined.contains("which"), "{joined}");
    assert!(body["error"].get("message").is_none());
}

#[tokio::test]
async fn a_short_object_that_hashes_is_delivery_ok_and_too_short() {
    let lab = lab();
    let object = vec![0x01, 0x02, 0x03];
    assert!(object.len() < 32);
    let signed = Signed::fresh(object, NOW_2026_10_10 + 120);
    let (status, body, _) = post_json(&lab, &signed.body()).await;
    assert_delivery(status, &body);
    assert_eq!(outcome_of(&lab, &signed.nonce).await, "too_short");
}

#[tokio::test]
async fn the_result_read_is_one_outcome_and_a_probe_creates_no_row() {
    let lab = lab();
    let nonce = "ab".repeat(32);
    let (status, body) = result(&lab, &nonce).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"outcome": "pending"}));
    let (status, body) = result(&lab, &nonce).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"outcome": "pending"}));
    let (status, _, _) = send(
        &lab,
        Request::get("/v1/sign-in/result/AB")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_hash_mismatch_does_not_replace_a_stored_outcome() {
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;
    let short = Signed::fresh(vec![0x07], exp);
    let (status, body, _) = post_json(&lab, &short.body()).await;
    assert_delivery(status, &body);
    assert_eq!(outcome_of(&lab, &short.nonce).await, "too_short");

    let mut again = short.body_signed(false);
    again["nonce"] = json!(hex32(&short.nonce));
    let (status, body, text) = post_json(&lab, &again).await;
    assert_only_code(status, &body, &text, "bad_signature");
    assert_eq!(outcome_of(&lab, &short.nonce).await, "too_short");
}

#[tokio::test]
async fn a_nonconsuming_failure_is_replaced_by_a_later_consume() {
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;
    let draft = Signed::fresh(Vec::new(), exp);
    let signed = Signed {
        object: v2_object(&draft.secret),
        ..draft
    };
    let mut early = signed.body();
    early["signature"] = json!("cd".repeat(64));
    let (status, body, text) = post_json(&lab, &early).await;
    assert_only_code(status, &body, &text, "bad_signature");
    assert_eq!(outcome_of(&lab, &signed.nonce).await, "bad_signature");

    let (status, body, _) = post_json(&lab, &signed.body()).await;
    assert_delivery(status, &body);
    let token = settled(&lab, &signed.nonce).await;
    assert_ne!(
        token, "bad_signature",
        "the consuming submit did not replace the miss"
    );
    assert_ne!(token, "pending");
}

#[tokio::test]
async fn a_consumed_replay_returns_the_original_class_and_does_not_rerun() {
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;
    let signed = Signed::fresh(vec![0x09], exp);
    let first = signed.body();
    let (status, body, _) = post_json(&lab, &first).await;
    assert_delivery(status, &body);
    assert_eq!(outcome_of(&lab, &signed.nonce).await, "too_short");

    let (status, body, _) = post_json(&lab, &first).await;
    assert_delivery(status, &body);
    assert_eq!(outcome_of(&lab, &signed.nonce).await, "too_short");

    let mut other = first.clone();
    other["signature"] = json!("11".repeat(64));
    let (status, body, text) = post_json(&lab, &other).await;
    assert_only_code(status, &body, &text, "malformed");
    assert_eq!(outcome_of(&lab, &signed.nonce).await, "too_short");
}

#[tokio::test]
async fn two_racing_first_submits_consume_the_nonce_once() {
    let lab = Arc::new(lab());
    let exp = NOW_2026_10_10 + 120;
    let first = Signed::fresh(vec![0x0a], exp);
    // Two objects, one nonce. Each signature is over its own object.
    let mut second = first.clone();
    second.object = vec![0x0b];
    let mut tasks = Vec::new();
    for body in [first.body(), second.body()] {
        let lab = Arc::clone(&lab);
        tasks.push(tokio::spawn(async move { post_json(&lab, &body).await }));
    }
    let mut deliveries = 0;
    let mut malformed = 0;
    for task in tasks {
        let (status, value, text) = task.await.unwrap();
        if status == StatusCode::OK && value["delivery"] == "ok" {
            deliveries += 1;
        } else if status == StatusCode::BAD_REQUEST && value["error"]["code"] == "malformed" {
            malformed += 1;
        } else {
            panic!("unexpected race response {status} {text}");
        }
    }
    assert_eq!(deliveries, 1, "both racers consumed the nonce");
    assert_eq!(malformed, 1);
    let token = outcome_of(&lab, &first.nonce).await;
    assert!(token == "too_short", "{token}");
    assert!(lab.door.consumed_nonce(&first.nonce));
    let _ = first;
}

#[tokio::test]
async fn the_new_wire_hides_the_floor_and_still_enforces_it() {
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;
    let draft = Signed::fresh(Vec::new(), exp);
    let signed = Signed {
        object: v2_object(&draft.secret),
        ..draft
    };
    let (status, body, text) = post_json(&lab, &signed.body()).await;
    assert_delivery(status, &body);
    assert!(!text.contains("1/5"), "{text}");
    assert!(!text.contains("leg"), "{text}");
    assert!(!text.contains("Submit again"), "{text}");
    let token = settled(&lab, &signed.nonce).await;
    assert_eq!(token, "rejected");
    let (status, value, raw) = send(
        &lab,
        Request::get(format!("/v1/sign-in/result/{}", hex32(&signed.nonce)))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value.as_object().unwrap().len(), 1, "{raw}");
    assert!(!raw.contains("1/5"), "{raw}");
    assert!(!raw.contains("floor"), "{raw}");
    assert!(!raw.contains("verifier"), "{raw}");
    assert!(!raw.contains("Submit again"), "{raw}");
    // Health echoes the policy id. The fraction stays off this wire.
    let (health_status, health, _) = send(
        &lab,
        Request::get("/v1/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(health_status, StatusCode::OK);
    assert_eq!(health["config"]["policy_id"], "gl-admit-v4");
    assert!(health["config"].get("leg_rate_floor").is_none(), "{health}");
    assert!(health["config"].get("log_dir").is_none(), "{health}");
    assert!(health["config"].get("config_path").is_none(), "{health}");

    // The public decision line names one reason_code. It does not write the
    // catch-all rejected, a floor sentence, a verifier note, or a retry instruction.
    let chain = std::fs::read_to_string(lab.dir.join("chain.jsonl")).expect("chain");
    assert!(
        !chain.contains("\"reason\":\"rejected\""),
        "the catch-all rejected is not a log reason: {chain}"
    );
    assert!(chain.contains("\"reason_code\":"), "{chain}");
    assert!(
        !chain.contains("leg per second")
            && !chain.contains("1/5")
            && !chain.contains("Submit again")
            && !chain.contains("N and S"),
        "the public log leaked a floor or a retry: {chain}"
    );
}

#[tokio::test]
async fn website_tokens_follow_the_table() {
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;

    let earlier_bytes = v135_object();
    assert!(
        hike::earlier_object_kind(&earlier_bytes).is_some(),
        "the vector is the earlier kind"
    );
    let earlier = Signed::fresh(earlier_bytes, exp);
    let (status, body, _) = post_json(&lab, &earlier.body()).await;
    // This fixture yields no 32-byte key, so step 6 is `bad_signature` and
    // the nonce is not consumed. A kind with a key is step 7 below.
    if hike::yielded_public_key(&earlier.object)
        == hike::YieldedKey::PublicKey(pubkey(&earlier.secret))
    {
        assert_delivery(status, &body);
        assert_eq!(outcome_of(&lab, &earlier.nonce).await, "earlier_version");
    } else {
        assert_only_code(status, &body, &body.to_string(), "bad_signature");
        assert_eq!(outcome_of(&lab, &earlier.nonce).await, "bad_signature");
    }

    let with_key = earlier_kind_with_key(&earlier.secret);
    let earlier_signed = Signed {
        object: with_key,
        ..earlier.clone()
    };
    // A second nonce: the fixture above may already have stored this one.
    let earlier_signed = Signed::fresh(earlier_signed.object, exp);
    // Re-stamp the key to this submission's secret.
    let earlier_signed = Signed {
        object: earlier_kind_with_key(&earlier_signed.secret),
        ..earlier_signed
    };
    let (status, body, _) = post_json(&lab, &earlier_signed.body()).await;
    assert_delivery(status, &body);
    assert_eq!(
        outcome_of(&lab, &earlier_signed.nonce).await,
        "earlier_version"
    );

    let garbage = Signed::fresh(b"not a hike object, just some bytes!!".to_vec(), exp);
    let (status, body, _) = post_json(&lab, &garbage.body()).await;
    assert_only_code(status, &body, &body.to_string(), "bad_signature");

    let draft = Signed::fresh(Vec::new(), exp);
    let shell = Signed {
        object: v2_object(&draft.secret),
        ..draft
    };
    let (status, body, _) = post_json(&lab, &shell.body()).await;
    assert_delivery(status, &body);
    assert_eq!(outcome_of(&lab, &shell.nonce).await, "pending");
    let token = settled(&lab, &shell.nonce).await;
    assert_eq!(token, "rejected");
    assert_ne!(token, "too_few_waypoints_anchored");
}

#[tokio::test]
async fn transport_edge_does_not_burn_a_nonce() {
    let lab = lab();
    let exp = NOW_2026_10_10 + 120;
    let object = vec![0x03];
    let signed = Signed::fresh(object, exp);
    let body = signed.body();

    let (status, value, text) = post_raw(&lab, "text/plain", body.to_string()).await;
    assert_ne!(status, StatusCode::BAD_REQUEST, "{text}");
    assert!(value.get("outcome").is_none());
    assert_eq!(outcome_of(&lab, &signed.nonce).await, "pending");

    let (status, value, _) = post_json(&lab, &body).await;
    assert_delivery(status, &value);
    assert_eq!(outcome_of(&lab, &signed.nonce).await, "too_short");
}

#[tokio::test]
async fn oversize_writes_rejected_without_consuming() {
    let dir = scratch("oversize");
    let mut cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
    cfg.max_object_bytes = 8;
    let service = Service::open(cfg).expect("gate");
    let door = service.door();
    door.set_now_unix_s(Some(NOW_2026_10_10));
    let lab = Lab {
        router: service.router(),
        door,
        dir,
        peer: Mutex::new(42_000),
    };
    let object = vec![0u8; 32];
    let signed = Signed::fresh(object, NOW_2026_10_10 + 120);
    let (status, _, text) = post_json(&lab, &signed.body()).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{text}");
    assert!(!text.contains("bad_signature"), "{text}");
    assert_eq!(outcome_of(&lab, &signed.nonce).await, "rejected");
    assert!(!lab.door.consumed_nonce(&signed.nonce));

    let mut later = signed;
    later.object = vec![0x01];
    let (status, body, _) = post_json(&lab, &later.body()).await;
    assert_delivery(status, &body);
    assert_eq!(outcome_of(&lab, &later.nonce).await, "too_short");
}

#[tokio::test]
async fn the_published_example_expiry_is_malformed_near_2026() {
    let doc = vector();
    let example = &doc["example"];
    assert_eq!(example["expiry_unix_s"], 1_893_456_000);
    assert_eq!(example["window_now_2026_10_10"], NOW_2026_10_10);
    assert_eq!(example["window_accepts_example_at_2026_10_10"], false);
    let lab = lab();
    let body = json!({
        "origin": "https://groundinglab.io",
        "nonce": example["nonce_hex"],
        "exp": example["expiry_unix_s"],
        "object": example["object_sha256_hex"],
        "object_b64": example["object_b64"],
        "signature": example["signature_hex"],
    });
    let (status, value, text) = post_json(&lab, &body).await;
    assert_only_code(status, &value, &text, "malformed");

    let fresh_exp = NOW_2026_10_10 + doc["lifetime_s"].as_u64().unwrap();
    let signed = Signed::fresh(vec![0x04], fresh_exp);
    let (status, value, text) = post_json(&lab, &signed.body()).await;
    assert_delivery(status, &value);
    assert!(!text.contains("malformed"), "{text}");
}

#[test]
fn window_rows_in_the_vector_match_the_wide_sum() {
    let doc = vector();
    let lifetime = doc["lifetime_s"].as_u64().unwrap();
    let skew = doc["skew_s"].as_u64().unwrap();
    assert_eq!(lifetime, 120);
    assert_eq!(skew, 5);
    for item in doc["window"].as_array().unwrap() {
        let now = item["now"].as_u64().unwrap();
        let exp = item["exp"].as_u64().unwrap();
        let now_w = u128::from(now);
        let exp_w = u128::from(exp);
        let accept = now_w < exp_w + u128::from(skew)
            && exp_w <= now_w + u128::from(lifetime) + u128::from(skew);
        assert_eq!(accept, item["accept"].as_bool().unwrap(), "{item}");
    }
}

#[tokio::test]
async fn the_rate_limit_is_delivery_ok_and_does_not_consume() {
    let dir = scratch("rate");
    let mut cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
    cfg.rate_per_min = 1;
    let service = Service::open(cfg).expect("gate");
    let door = service.door();
    door.set_now_unix_s(Some(NOW_2026_10_10));
    let lab = Lab {
        router: service.router(),
        door,
        dir,
        peer: Mutex::new(43_000),
    };
    // One peer, so the limiter sees one client. The first POST is the short
    // object and consumes. The helper advances the peer, so pin it.
    *lab.peer.lock().unwrap() = 43_100;
    let first = Signed::fresh(vec![0x05], NOW_2026_10_10 + 120);
    let (status, _, _) = post_json(&lab, &first.body()).await;
    assert_eq!(status, StatusCode::OK);

    let draft = Signed::fresh(Vec::new(), NOW_2026_10_10 + 120);
    let second = Signed {
        object: v2_object(&draft.secret),
        ..draft
    };
    let (status, value, text) = post_json(&lab, &second.body()).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{text}");
    assert_ne!(value["error"]["code"], "bad_signature");
    assert!(!text.contains("bad_signature"), "{text}");
    assert_eq!(outcome_of(&lab, &second.nonce).await, "rejected");
    assert!(!lab.door.consumed_nonce(&second.nonce));
}
