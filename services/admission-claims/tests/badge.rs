//! Badge acceptance checks that can run offline.
//! A fixture clock, a fixture gate line server, and a directory publisher.

use admission_claims::nostr::{self, Event, KIND_HTTP_AUTH};
use admission_claims::{Config, ManualClock, RelaySender, Service};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use http_body_util::BodyExt;
use secp256k1::{Message, Secp256k1, XOnlyPublicKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tower::ServiceExt;

const NOW: i64 = 1_700_000_000;
const POLICY: &str = "gl-admit-v1";
type Lines = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct World {
    svc: Service,
    clock: Arc<ManualClock>,
    lines: Lines,
    sent: Arc<Mutex<Vec<String>>>,
    fails: Arc<AtomicUsize>,
    dir: PathBuf,
    issuer_pub: String,
    user_secret: [u8; 32],
    user_pub: String,
    issuer_secret_hex: String,
}

async fn world() -> World {
    let dir =
        std::env::temp_dir().join(format!("claims-badge-{}-{}", std::process::id(), unique()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let issuer_secret_hex = format!("{:064x}", unique());
    let issuer_secret = hex_key(&issuer_secret_hex);
    let issuer_pub = nostr::pubkey_hex(&issuer_secret).unwrap();
    write_key(&dir.join("issuer.key"), &issuer_secret_hex, 0o600);
    let user_secret_hex = format!("{:064x}", unique());
    let user_secret = hex_key(&user_secret_hex);
    let user_pub = nostr::pubkey_hex(&user_secret).unwrap();
    let lines: Lines = Arc::new(Mutex::new(HashMap::new()));
    let gate = spawn_gate(lines.clone()).await;
    let clock = Arc::new(ManualClock::new(NOW));
    let sent = Arc::new(Mutex::new(Vec::new()));
    let fails = Arc::new(AtomicUsize::new(0));
    let sent_relay = sent.clone();
    let fails_relay = fails.clone();
    let relay = RelaySender::new(move |body: String| {
        let sent_relay = sent_relay.clone();
        let fails_relay = fails_relay.clone();
        async move {
            sent_relay.lock().unwrap().push(body);
            if fails_relay.load(Ordering::SeqCst) > 0
                && fails_relay.fetch_sub(1, Ordering::SeqCst) > 0
            {
                return Err("forced".into());
            }
            Ok(())
        }
    });
    let cfg = load_config(&dir, &gate, &issuer_pub, 33_554_432, 5);
    let svc = Service::open_with(cfg, clock.clone(), relay).unwrap();
    svc.prepare().await.unwrap();
    World {
        svc,
        clock,
        lines,
        sent,
        fails,
        dir,
        issuer_pub,
        user_secret,
        user_pub,
        issuer_secret_hex,
    }
}

fn load_config(
    dir: &FsPath,
    gate: &str,
    issuer_pub: &str,
    max_log_bytes: usize,
    gate_timeout: u64,
) -> Config {
    let path = dir.join("claims.toml");
    let toml = format!(
        r#"
role = "writer"
bind = "127.0.0.1:8793"
claims_origin = "https://claims.groundinglab.io"
gate_base = "{gate}"
gate_timeout_secs = {gate_timeout}
max_log_bytes = {max_log_bytes}
db_path = "{db}"
issuer_key_file = "{key}"
issuer_pubkey_hex = "{issuer_pub}"
artifact_dir = "{artifacts}"
backup_dir = "{backup}"
relay_url = "ws://127.0.0.1:9"
log_filter = "info"
claim_per_ip_per_min = 0
claim_per_pubkey_per_min = 0
r2_account_id = ""
blocked_names_file = "{blocked}"
"#,
        blocked = dir.join("blocked-names.txt").display(),
        db = dir.join("claims.sqlite").display(),
        key = dir.join("issuer.key").display(),
        artifacts = dir.join("artifacts").display(),
        backup = dir.join("backups").display(),
    );
    fs::write(&path, toml).unwrap();
    Config::load(&path).unwrap()
}

fn write_key(path: &FsPath, hex_body: &str, mode: u32) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)
        .unwrap();
    file.write_all(hex_body.as_bytes()).unwrap();
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(mode);
    fs::set_permissions(path, perms).unwrap();
}

fn hex_key(text: &str) -> [u8; 32] {
    let bytes = hex::decode(text).unwrap();
    bytes.try_into().unwrap()
}

fn unique() -> u64 {
    static N: AtomicUsize = AtomicUsize::new(1);
    // Keep the value inside the scalar range. A counter in the low bytes is a valid key.
    N.fetch_add(1, Ordering::Relaxed) as u64 + 10
}

async fn spawn_gate(lines: Lines) -> String {
    let app = Router::new()
        .route("/log/{day}/{seq}", get(gate_line))
        .with_state(lines);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

async fn gate_line(
    State(lines): State<Lines>,
    Path((day, seq)): Path<(String, String)>,
) -> Response {
    let key = format!("{day}/{seq}");
    match lines.lock().unwrap().get(&key) {
        Some(body) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            body.clone(),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            [("content-type", "text/plain")],
            Vec::new(),
        )
            .into_response(),
    }
}

fn put_line(world: &World, day: &str, seq: i64, body: Vec<u8>) {
    world
        .lines
        .lock()
        .unwrap()
        .insert(format!("{day}/{seq}"), body);
}

fn accept_line(seq: i64, time: i64, pubkey: &str, object: &str, policy: Option<&str>) -> Vec<u8> {
    let mut value = json!({
        "seq": seq,
        "time_unix_s": time,
        "phase": "thorough",
        "decision": "accept",
        "public_key": pubkey,
        "object_sha256": object,
    });
    if let Some(policy) = policy {
        value["policy_id"] = json!(policy);
    }
    serde_json::to_vec(&value).unwrap()
}

fn return_line(
    seq: i64,
    time: i64,
    pubkey: &str,
    object: &str,
    policy: &str,
    ref_day: &str,
    ref_seq: i64,
) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "seq": seq,
        "time_unix_s": time,
        "phase": "return",
        "decision": "return",
        "public_key": pubkey,
        "object_sha256": object,
        "policy_id": policy,
        "access_ref_day": ref_day,
        "access_ref_seq": ref_seq,
    }))
    .unwrap()
}

fn object() -> String {
    "ab".repeat(32)
}

fn claim(secret: &[u8; 32], created_at: i64, nonce: &str, day: &str, seq: i64) -> Event {
    signed(
        secret,
        created_at,
        tags(
            nonce,
            day,
            seq,
            "https://claims.groundinglab.io/badge",
            "badge",
            None,
        ),
    )
}

fn signed(secret: &[u8; 32], created_at: i64, tags: Vec<Vec<String>>) -> Event {
    nostr::sign_event(secret, created_at, KIND_HTTP_AUTH, tags, "").unwrap()
}

fn tags(
    nonce: &str,
    day: &str,
    seq: i64,
    url: &str,
    action: &str,
    name: Option<&str>,
) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["u".into(), url.into()],
        vec!["method".into(), "POST".into()],
        vec!["action".into(), action.into()],
        vec!["nonce".into(), nonce.into()],
        vec!["access".into(), day.into(), seq.to_string()],
    ];
    if let Some(name) = name {
        tags.push(vec!["name".into(), name.into()]);
    }
    tags
}

/// The no-extension body: the npub and the gate line from the access tag.
fn request_body(event: &Event) -> String {
    let access = event.tags.iter().find(|t| t[0] == "access").unwrap();
    json!({
        "npub": nostr::npub_from_hex(&event.pubkey).unwrap(),
        "log_day": access[1],
        "log_seq": access[2].parse::<i64>().unwrap(),
    })
    .to_string()
}

async fn post_raw(svc: &Service, body: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/badge")
        .header("content-type", "application/json")
        .header("cf-connecting-ip", "203.0.113.10")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let response = svc.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn post(svc: &Service, event: &Event) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/badge")
        .header("content-type", "application/json")
        .header("cf-connecting-ip", "203.0.113.10")
        .body(Body::from(request_body(event)))
        .unwrap();
    let response = svc.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn open_err(cfg: Config, clock: Arc<ManualClock>) -> String {
    match Service::open_with(cfg, clock, RelaySender::new(|_| async { Ok(()) })) {
        Ok(_) => panic!("service started"),
        Err(err) => err,
    }
}

fn code(value: &Value) -> &str {
    value["error"]["code"].as_str().unwrap_or("")
}

fn day_of(unix: i64) -> String {
    admission_claims::timeutil::utc_day(unix)
}

fn award_id(value: &Value) -> String {
    value["award"]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn accept_and_return_inside_the_window() {
    let world = world().await;
    let day = day_of(NOW);
    put_line(
        &world,
        &day,
        4,
        accept_line(4, NOW - 10, &world.user_pub, &object(), Some(POLICY)),
    );
    let event = claim(&world.user_secret, NOW, &format!("{:064x}", 11u64), &day, 4);
    let (status, body) = post(&world.svc, &event).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let award = &body["award"];
    assert_eq!(award["kind"], 8);
    assert_eq!(award["content"], "");
    assert_eq!(award["pubkey"], world.issuer_pub);
    assert_eq!(award["created_at"], NOW);
    assert_eq!(award["tags"][0][0], "a");
    assert_eq!(
        award["tags"][0][1],
        format!("30009:{}:groundinglab-admitted", world.issuer_pub)
    );
    assert_eq!(award["tags"][1], json!(["p", world.user_pub]));
    assert_eq!(award["tags"][2], json!(["policy", POLICY]));
    assert_eq!(
        award["tags"][3][1],
        format!("https://admission.groundinglab.io/log/{day}.jsonl#4")
    );
    assert_eq!(body["admitted_unix_s"], NOW - 10);
    assert!(body["badge_url"]
        .as_str()
        .unwrap()
        .starts_with("https://groundinglab.io/badge/npub1"));
    verify_independently(&world, award["id"].as_str().unwrap());

    let accept_day = day_of(NOW - 86_400);
    put_line(
        &world,
        &accept_day,
        2,
        accept_line(2, NOW - 86_400, &world.user_pub, &object(), Some(POLICY)),
    );
    let return_day = day_of(NOW);
    put_line(
        &world,
        &return_day,
        9,
        return_line(9, NOW, &world.user_pub, &object(), POLICY, &accept_day, 2),
    );
    // A second npub so this is a new badge, not the stored one.
    let other_hex = format!("{:064x}", unique());
    let other = hex_key(&other_hex);
    let other_pub = nostr::pubkey_hex(&other).unwrap();
    world.lines.lock().unwrap().insert(
        format!("{accept_day}/2"),
        accept_line(2, NOW - 86_400, &other_pub, &object(), Some(POLICY)),
    );
    world.lines.lock().unwrap().insert(
        format!("{return_day}/9"),
        return_line(9, NOW, &other_pub, &object(), POLICY, &accept_day, 2),
    );
    let event = claim(&other, NOW, &format!("{:064x}", 12u64), &return_day, 9);
    let (status, body) = post(&world.svc, &event).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["award"]["tags"][3][1],
        format!("https://admission.groundinglab.io/log/{accept_day}.jsonl#2")
    );
    assert_eq!(body["admitted_unix_s"], NOW - 86_400);
    assert_eq!(body["award"]["created_at"], NOW);
}

#[tokio::test]
async fn window_edges_and_refusals() {
    let world = world().await;
    let cases = [(NOW - 300, "edge-old"), (NOW + 5, "edge-new")];
    for (index, (time, label)) in cases.into_iter().enumerate() {
        let day = day_of(time);
        let seq = 20 + index as i64;
        let secret = hex_key(&format!("{:064x}", unique()));
        let pubkey = nostr::pubkey_hex(&secret).unwrap();
        put_line(
            &world,
            &day,
            seq,
            accept_line(seq, time, &pubkey, &object(), Some(POLICY)),
        );
        let event = claim(
            &secret,
            NOW,
            &format!("{:064x}", 100 + index as u64),
            &day,
            seq,
        );
        let (status, body) = post(&world.svc, &event).await;
        assert_eq!(status, StatusCode::OK, "{label} {body}");
    }

    let secret = hex_key(&format!("{:064x}", unique()));
    let pubkey = nostr::pubkey_hex(&secret).unwrap();
    let day = day_of(NOW);
    let missing = claim(&secret, NOW, &format!("{:064x}", 201u64), &day, 404);
    let (status, body) = post(&world.svc, &missing).await;
    assert_eq!(code(&body), "no_access", "{status}");

    put_line(
        &world,
        &day,
        30,
        serde_json::to_vec(&json!({
            "seq": 30,
            "time_unix_s": NOW,
            "phase": "provisional",
            "decision": "accept",
            "public_key": pubkey,
            "object_sha256": object(),
            "policy_id": POLICY,
        }))
        .unwrap(),
    );
    let event = claim(&secret, NOW, &format!("{:064x}", 202u64), &day, 30);
    let (_, body) = post(&world.svc, &event).await;
    assert_eq!(code(&body), "provisional");

    put_line(
        &world,
        &day,
        31,
        accept_line(31, NOW, &pubkey, &object(), None),
    );
    let event = claim(&secret, NOW, &format!("{:064x}", 203u64), &day, 31);
    let (_, body) = post(&world.svc, &event).await;
    assert_eq!(code(&body), "no_access");

    put_line(
        &world,
        &day,
        32,
        accept_line(32, NOW - 301, &pubkey, &object(), Some(POLICY)),
    );
    let event = claim(
        &secret,
        NOW,
        &format!("{:064x}", 204u64),
        &day_of(NOW - 301),
        32,
    );
    let (_, body) = post(&world.svc, &event).await;
    assert_eq!(code(&body), "window");

    let future = NOW + 6;
    put_line(
        &world,
        &day_of(future),
        33,
        accept_line(33, future, &pubkey, &object(), Some(POLICY)),
    );
    let event = claim(
        &secret,
        NOW,
        &format!("{:064x}", 205u64),
        &day_of(future),
        33,
    );
    let (_, body) = post(&world.svc, &event).await;
    assert_eq!(code(&body), "window");

    let demo = "42a34e98196b4a1b11c3a5ed3357be7358d98c98176b414efa99f23fb4f7fe06";
    put_line(
        &world,
        &day,
        34,
        accept_line(34, NOW, &pubkey, demo, Some(POLICY)),
    );
    let event = claim(&secret, NOW, &format!("{:064x}", 206u64), &day, 34);
    let (_, body) = post(&world.svc, &event).await;
    assert_eq!(code(&body), "demo");
}

#[tokio::test]
async fn no_signature_needed_and_key_must_match_the_line() {
    let world = world().await;
    let day = day_of(NOW);
    put_line(
        &world,
        &day,
        1,
        accept_line(1, NOW, &world.user_pub, &object(), Some(POLICY)),
    );
    for bad in [
        "{}".to_owned(),
        json!({"npub": "npub1nope", "log_day": day, "log_seq": 1}).to_string(),
        json!({"npub": nostr::npub_from_hex(&world.user_pub).unwrap(), "log_day": "x", "log_seq": 1}).to_string(),
        json!({"npub": nostr::npub_from_hex(&world.user_pub).unwrap(), "log_day": day, "log_seq": 0}).to_string(),
    ] {
        let (status, body) = post_raw(&world.svc, &bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(code(&body), "malformed");
    }
    let other = hex_key(&format!("{:064x}", unique()));
    let other_pub = nostr::pubkey_hex(&other).unwrap();
    let wrong = json!({"npub": nostr::npub_from_hex(&other_pub).unwrap(), "log_day": day, "log_seq": 1}).to_string();
    let (_, body) = post_raw(&world.svc, &wrong).await;
    assert_eq!(code(&body), "no_access");

    let good = json!({"npub": nostr::npub_from_hex(&world.user_pub).unwrap(), "log_day": day, "log_seq": 1}).to_string();
    let (status, first) = post_raw(&world.svc, &good).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, again) = post_raw(&world.svc, &good).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(award_id(&first), award_id(&again));
}

#[tokio::test]
async fn one_badge_per_npub_and_the_same_event_on_retry() {
    let world = world().await;
    let day = day_of(NOW);
    put_line(
        &world,
        &day,
        7,
        accept_line(7, NOW, &world.user_pub, &object(), Some(POLICY)),
    );
    world.fails.store(1, Ordering::SeqCst);
    let first = claim(
        &world.user_secret,
        NOW,
        &format!("{:064x}", 401u64),
        &day,
        7,
    );
    let (status, body) = post(&world.svc, &first).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let id = award_id(&body);
    let second = claim(
        &world.user_secret,
        NOW,
        &format!("{:064x}", 402u64),
        &day,
        7,
    );
    world.clock.set(NOW + 2);
    let (status, again) = post(&world.svc, &second).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(award_id(&again), id);
    let sent = world.sent.lock().unwrap().clone();
    let badge_sends: Vec<_> = sent.into_iter().filter(|body| body.contains(&id)).collect();
    assert!(
        badge_sends.len() >= 2,
        "retry did not send the stored event"
    );
    for body in &badge_sends {
        let event: Value = serde_json::from_str(body).unwrap();
        assert_eq!(event["id"], id);
    }

    let left = claim(
        &world.user_secret,
        NOW + 2,
        &format!("{:064x}", 501u64),
        &day,
        7,
    );
    let right = claim(
        &world.user_secret,
        NOW + 2,
        &format!("{:064x}", 502u64),
        &day,
        7,
    );
    let (one, two) = tokio::join!(post(&world.svc, &left), post(&world.svc, &right));
    assert_eq!(one.0, StatusCode::OK, "{}", one.1);
    assert_eq!(two.0, StatusCode::OK, "{}", two.1);
    assert_eq!(award_id(&one.1), id);
    assert_eq!(award_id(&two.1), id);
    let db = rusqlite::Connection::open(world.dir.join("claims.sqlite")).unwrap();
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM badge", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn award_record_and_page_have_no_measurement_keys() {
    let world = world().await;
    let day = day_of(NOW);
    put_line(
        &world,
        &day,
        8,
        accept_line(8, NOW - 3, &world.user_pub, &object(), Some(POLICY)),
    );
    let event = claim(
        &world.user_secret,
        NOW,
        &format!("{:064x}", 601u64),
        &day,
        8,
    );
    let (status, body) = post(&world.svc, &event).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let id = award_id(&body);
    let record_path = world
        .dir
        .join("artifacts")
        .join("claims")
        .join(format!("{id}.json"));
    let record: Value = serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
    let index: Value =
        serde_json::from_slice(&fs::read(world.dir.join("artifacts/claims/index.json")).unwrap())
            .unwrap();
    let npub = body["badge_url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap();
    let html = fs::read_to_string(
        world
            .dir
            .join("artifacts")
            .join("badge")
            .join(format!("{npub}.html")),
    )
    .unwrap();
    let nostr = fs::read(world.dir.join("artifacts/.well-known/nostr.json")).unwrap();
    assert_eq!(nostr, br#"{"names":{}}"#);
    assert_eq!(index["feed"], "groundinglab-claims-v1");
    let mut found = false;
    for item in index["items"].as_array().unwrap() {
        if item["id"] == id {
            found = true;
            assert_eq!(item["type"], "badge");
            assert_eq!(item["pubkey"], world.user_pub);
            assert!(item["name"].is_null());
        }
    }
    assert!(found);
    for value in [&body["award"], &record, &index] {
        let mut names = Vec::new();
        collect_keys(value, &mut names);
        for name in names {
            assert!(!forbidden_key(&name), "{name} in {value}");
        }
    }
    for word in ["legs", "span_s", "leg_rate", "verify_summary", "waypoint"] {
        assert!(!html.contains(word), "{word} in the badge page");
        assert!(!record.to_string().contains(word));
    }
    assert!(html.contains(&world.issuer_pub));
    assert!(html.contains(&world.user_pub));
    assert!(html.contains(POLICY));
    assert!(html.contains(&id));
    assert!(!html.to_ascii_lowercase().contains("<script"));
}

#[tokio::test]
async fn key_mode_pin_and_logs() {
    let dir = std::env::temp_dir().join(format!(
        "claims-key-ac8-{}-{}",
        std::process::id(),
        unique()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let secret_hex = format!("{:064x}", unique());
    let secret = hex_key(&secret_hex);
    let pin = nostr::pubkey_hex(&secret).unwrap();
    let key_path = dir.join("issuer.key");
    write_key(&key_path, &secret_hex, 0o644);
    let gate = "http://127.0.0.1:9";
    let wide = load_config(&dir, gate, &pin, 1024, 1);
    let err = open_err(wide, Arc::new(ManualClock::new(NOW)));
    assert!(err.contains("mode"), "{err}");
    assert!(!err.contains(&secret_hex));

    write_key(&key_path, &secret_hex, 0o600);
    let wrong = load_config(&dir, gate, &"ab".repeat(32), 1024, 1);
    let err = open_err(wrong, Arc::new(ManualClock::new(NOW)));
    assert!(err.contains("does not match"), "{err}");
    assert!(!err.contains(&secret_hex));

    let logs = log_buf();
    let world = world().await;
    let day = day_of(NOW);
    put_line(
        &world,
        &day,
        1,
        accept_line(1, NOW, &world.user_pub, &object(), Some(POLICY)),
    );
    let event = claim(
        &world.user_secret,
        NOW,
        &format!("{:064x}", 701u64),
        &day,
        1,
    );
    let (status, body) = post(&world.svc, &event).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let health = world
        .svc
        .router()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let health_bytes = health.into_body().collect().await.unwrap().to_bytes();
    let health_text = String::from_utf8(health_bytes.to_vec()).unwrap();
    assert!(
        health_text.contains("\"key\":\"loaded\"") || health_text.contains("\"key\": \"loaded\"")
    );
    assert!(!health_text.contains(&world.issuer_secret_hex));
    assert!(!body.to_string().contains(&world.issuer_secret_hex));
    let text = logs.lock().unwrap().clone();
    assert!(
        text.contains("issuer public key"),
        "startup did not log the public key: {text}"
    );
    assert!(
        !text.contains(&world.issuer_secret_hex),
        "issuer secret reached the log"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_second_writer_does_not_take_the_lock() {
    let world = world().await;
    let cfg = world.svc.config().clone();
    let err = open_err(cfg, world.clock.clone());
    assert!(err.contains("lock"), "{err}");
}

#[test]
fn blocked_names_file_is_shipped_and_well_formed() {
    let path = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("deploy/blocked-names.txt");
    let text = fs::read_to_string(&path).unwrap();
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        assert!(
            line.len() >= 2
                && line.len() <= 30
                && line
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                && !line.starts_with('_')
                && !line.ends_with('_'),
            "{line}"
        );
        names.push(line.to_owned());
    }
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len());
    assert_eq!(names.len(), 985);
    assert!(!names.iter().any(|n| n == "christian" || n == "groundinglab"));
}

fn verify_independently(world: &World, id: &str) {
    let path = world
        .dir
        .join("artifacts")
        .join("claims")
        .join(format!("{id}.json"));
    let raw = fs::read(&path).unwrap();
    let value: Value = serde_json::from_slice(&raw).unwrap();
    let pubkey = value["pubkey"].as_str().unwrap();
    let created_at = value["created_at"].as_i64().unwrap();
    let kind = value["kind"].as_u64().unwrap() as u32;
    let content = value["content"].as_str().unwrap();
    let tags: Vec<Vec<String>> = serde_json::from_value(value["tags"].clone()).unwrap();
    let preimage = serde_json::to_vec(&(0_u8, pubkey, created_at, kind, &tags, content)).unwrap();
    let digest = Sha256::digest(preimage);
    assert_eq!(hex::encode(digest), value["id"].as_str().unwrap());
    let pk = XOnlyPublicKey::from_slice(&hex::decode(pubkey).unwrap()).unwrap();
    let sig = secp256k1::schnorr::Signature::from_slice(
        &hex::decode(value["sig"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    let secp = Secp256k1::verification_only();
    secp.verify_schnorr(&sig, &Message::from_digest(digest.into()), &pk)
        .unwrap();
    assert_eq!(pubkey, world.issuer_pub);
    let index = fs::read_to_string(world.dir.join("artifacts/claims/index.json")).unwrap();
    assert!(index.contains(id));
}

fn collect_keys(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                out.push(key.clone());
                collect_keys(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_keys(item, out);
            }
        }
        _ => {}
    }
}

fn forbidden_key(name: &str) -> bool {
    matches!(
        name,
        "legs"
            | "span_s"
            | "leg_rate"
            | "leg_rate_floor"
            | "waypoints"
            | "waypoint"
            | "verify_summary"
            | "measurement"
            | "fast_seq"
            | "bitcoin_tip_height"
    )
}

fn log_buf() -> Arc<Mutex<String>> {
    static LOGS: OnceLock<Arc<Mutex<String>>> = OnceLock::new();
    LOGS.get_or_init(|| {
        let buf = Arc::new(Mutex::new(String::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(Mem(buf.clone()))
            .without_time()
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("log subscriber");
        buf
    })
    .clone()
}

#[derive(Clone)]
struct Mem(Arc<Mutex<String>>);

impl Write for Mem {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap()
            .push_str(&String::from_utf8_lossy(data));
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Mem {
    type Writer = Mem;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

async fn post_name(svc: &Service, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/name")
        .header("content-type", "application/json")
        .header("cf-connecting-ip", "203.0.113.10")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = svc.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn available(svc: &Service, name: &str) -> String {
    let request = Request::builder()
        .uri(format!("/name/available?name={name}"))
        .body(Body::empty())
        .unwrap();
    let response = svc.router().oneshot(request).await.unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    value["status"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn names_one_each_blocked_and_published() {
    let world = world().await;
    fs::write(world.dir.join("blocked-names.txt"), "# list\nadmin\n").unwrap();
    let day = day_of(NOW);
    put_line(&world, &day, 1, accept_line(1, NOW, &world.user_pub, &object(), Some(POLICY)));
    let npub = nostr::npub_from_hex(&world.user_pub).unwrap();
    let body = |name: &str, seq: i64| json!({"npub": npub, "name": name, "log_day": day, "log_seq": seq});

    assert_eq!(available(&world.svc, "admin").await, "reserved");
    assert_eq!(available(&world.svc, "Bad").await, "invalid");
    assert_eq!(available(&world.svc, "ada_9").await, "free");

    let (_, v) = post_name(&world.svc, body("admin", 1)).await;
    assert_eq!(code(&v), "reserved");
    let (_, v) = post_name(&world.svc, body("a-b", 1)).await;
    assert_eq!(code(&v), "invalid");

    // Edits to the block list apply on the next request.
    fs::write(world.dir.join("blocked-names.txt"), "admin\nada_9\n").unwrap();
    let (_, v) = post_name(&world.svc, body("ada_9", 1)).await;
    assert_eq!(code(&v), "reserved");
    fs::write(world.dir.join("blocked-names.txt"), "admin\n").unwrap();

    let (status, v) = post_name(&world.svc, body("ada_9", 1)).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["nip05"], "ada_9@groundinglab.io");
    assert_eq!(v["record"]["kind"], 1742);
    let (status, again) = post_name(&world.svc, body("ada_9", 1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["record"]["id"], v["record"]["id"]);
    let (_, v2) = post_name(&world.svc, body("other", 1)).await;
    assert_eq!(code(&v2), "has_name");
    assert_eq!(available(&world.svc, "ada_9").await, "taken");

    let other = hex_key(&format!("{:064x}", unique()));
    let other_pub = nostr::pubkey_hex(&other).unwrap();
    put_line(&world, &day, 2, accept_line(2, NOW, &other_pub, &object(), Some(POLICY)));
    let other_body = json!({"npub": nostr::npub_from_hex(&other_pub).unwrap(), "name": "ada_9", "log_day": day, "log_seq": 2});
    let (_, v3) = post_name(&world.svc, other_body).await;
    assert_eq!(code(&v3), "taken");

    let nostr_json = fs::read_to_string(world.dir.join("artifacts/.well-known/nostr.json")).unwrap();
    assert_eq!(nostr_json, format!("{{\"names\":{{\"ada_9\":\"{}\"}}}}", world.user_pub));
}
