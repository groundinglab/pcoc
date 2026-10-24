//! Gate on the 2.2 SDK. Sign-in is the locked §15.1 wire, covered in `locked_wire`.
//! This file covers health, the verify job, the decision log, and restart.
//! Any other kind is `unknown_kind` on verify (Rule A5).
#![forbid(unsafe_code)]

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use pcoc_admission::{Config, Service};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tower::ServiceExt;

fn with_peer(mut request: Request<Body>) -> Request<Body> {
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50_002))));
    request
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let response = router
        .clone()
        .oneshot(with_peer(request))
        .await
        .expect("router response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = if bytes.is_empty() {
        serde_json::json!(null)
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|err| {
            panic!(
                "body was not JSON ({err}): {}",
                String::from_utf8_lossy(&bytes)
            )
        })
    };
    (status, value)
}

fn scratch(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("pcoc-gate-{label}-{}-{n}", std::process::id()))
}

fn service(index_url: &str) -> (Router, PathBuf) {
    let dir = scratch("svc");
    let _ = std::fs::remove_dir_all(&dir);
    let router = Service::open(Config::for_tests(dir.clone(), index_url))
        .expect("gate")
        .router();
    (router, dir)
}

fn v135_object() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/earlier-kind.cbor");
    std::fs::read(&path).expect("earlier kind object")
}

/// Kind text that is not `v2.2-pcoc-object`, plus a 32-byte key.
/// SPEC §14.7 / Rule A5. The verify job does not read the index.
fn other_kind(public_key: [u8; 32]) -> Vec<u8> {
    fn text(value: &str) -> Vec<u8> {
        let bytes = value.as_bytes();
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
    let mut out = vec![0xa2];
    out.extend(text("kind"));
    out.extend(text("satoshi-hike-v2.0-object"));
    out.extend(text("public_key"));
    out.push(0x58);
    out.push(32);
    out.extend_from_slice(&public_key);
    out
}

fn append_line(
    dir: &std::path::Path,
    object: Vec<u8>,
    tip: u64,
    submission: [u8; 32],
    phase: &'static str,
    decision: &'static str,
) {
    use pcoc_admission::log::{DecisionLog, DecisionWrite};
    let mut log = DecisionLog::open(dir).unwrap();
    let mut write = DecisionWrite::bare(1_700_000_000, Some(tip), phase, decision);
    write.fast_seq = if phase == "thorough" { Some(0) } else { None };
    write.submission_sha256 = submission;
    write.public_key = [3u8; 32];
    write.object_sha256 = sha256_bytes(&object);
    write.object = object;
    log.append(&write).unwrap();
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[derive(Clone)]
enum Reply {
    Body { status: String, body: String },
    Hang,
}

struct Plan {
    exact: HashMap<String, Reply>,
    default: Reply,
}

struct Index {
    base: String,
    stop: Arc<AtomicBool>,
}

impl Index {
    /// Every path returns this body. A beacon parse fails when it has no header.
    fn every(body: String) -> Self {
        Self::serve(
            HashMap::new(),
            Reply::Body {
                status: "200 OK".to_owned(),
                body,
            },
        )
    }

    /// Every path answers with this status and an empty body. No retry on the client.
    fn status_only(status: &str) -> Self {
        Self::serve(
            HashMap::new(),
            Reply::Body {
                status: status.to_owned(),
                body: String::new(),
            },
        )
    }

    /// Accepts the TCP connection and never writes a response.
    fn blackhole() -> Self {
        Self::serve(HashMap::new(), Reply::Hang)
    }

    fn serve(exact: HashMap<String, Reply>, default: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).expect("nonblocking");
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let plan = Arc::new(Plan { exact, default });
        thread::spawn(move || {
            while !stop_thread.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let plan = Arc::clone(&plan);
                        let stop = Arc::clone(&stop_thread);
                        thread::spawn(move || handle(stream, plan, stop));
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base: format!("http://{addr}"),
            stop,
        }
    }
}

impl Drop for Index {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn handle(mut stream: TcpStream, plan: Arc<Plan>, stop: Arc<AtomicBool>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let header_end = loop {
        let Ok(n) = stream.read(&mut tmp) else {
            return;
        };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let path = head
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_owned();
    let reply = plan
        .exact
        .get(&path)
        .cloned()
        .unwrap_or_else(|| plan.default.clone());
    match reply {
        Reply::Hang => {
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(20));
            }
        }
        Reply::Body { status, body } => {
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    }
}

async fn post_cbor(router: &Router, object: Vec<u8>) -> (StatusCode, serde_json::Value) {
    send(
        router,
        Request::post("/v1/verify")
            .header(header::CONTENT_TYPE, "application/cbor")
            .body(Body::from(object))
            .unwrap(),
    )
    .await
}

fn json_values(raw: &[u8]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for item in serde_json::Deserializer::from_slice(raw).into_iter() {
        match item {
            Ok(value) => out.push(value),
            Err(_) => break,
        }
    }
    out
}

/// The embedded record is `reference`, including its trailing newline.
fn assert_record_bytes(body: &str, marker: &str, reference: &str) {
    assert!(
        reference.ends_with('\n'),
        "reference record dropped the trailing newline"
    );
    let start = body
        .find(marker)
        .unwrap_or_else(|| panic!("no {marker} in {body}"))
        + marker.len();
    let rest = &body[start..];
    assert!(
        rest.starts_with(reference),
        "record bytes differ at {}",
        first_diff(rest, reference)
    );
    let after = rest.as_bytes().get(reference.len()).copied();
    assert!(
        matches!(after, Some(b'}') | Some(b',')),
        "record did not end at the reference boundary: {after:?}"
    );
}

fn first_diff(got: &str, expected: &str) -> String {
    let got_b = got.as_bytes();
    let expected_b = expected.as_bytes();
    let n = got_b.len().min(expected_b.len());
    for i in 0..n {
        if got_b[i] != expected_b[i] {
            let from = i.saturating_sub(24);
            let got_end = got.len().min(i + 40);
            let expected_end = expected.len().min(i + 40);
            return format!(
                "byte {i} got {:?} expected {:?}",
                &got[from..got_end],
                &expected[from..expected_end]
            );
        }
    }
    format!(
        "lengths differ: body {} reference {}",
        got.len(),
        expected.len()
    )
}

async fn poll_job(router: &Router, job_id: &str) -> serde_json::Value {
    for _ in 0..50 {
        let (status, body) = send(
            router,
            Request::get(format!("/v1/verify/{job_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        if body["terminal"] == true {
            return body;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("verify job did not finish");
}

async fn poll_job_for(router: &Router, job_id: &str, budget: Duration) -> serde_json::Value {
    let deadline = std::time::Instant::now() + budget;
    while std::time::Instant::now() < deadline {
        let (status, body) = send(
            router,
            Request::get(format!("/v1/verify/{job_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        if body["terminal"] == true {
            return body;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("verify job {job_id} did not finish within {budget:?}");
}

#[tokio::test]
async fn health_reports_version_commit_spec_and_config() {
    let (router, dir) = service("https://index.example");
    let (status, body) = send(
        &router,
        Request::get("/v1/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert_eq!(body["service"], "admission-service");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["version"], "0.5.8");
    assert!(body["commit"].as_str().unwrap().len() >= 7);
    // The gate builds the in-tree Rust SDK; health names its version.
    // A clean tree stamps "(path)"; uncommitted SDK changes stamp "(path, dirty)".
    let version = include_str!("../../../sdk/VERSION").trim();
    let sdk = pcoc_admission::http::SDK_VERSION;
    assert!(
        sdk == format!("{version} (path)") || sdk == format!("{version} (path, dirty)"),
        "{sdk}"
    );
    assert_eq!(body["sdk"], sdk, "{body}");
    assert!(pcoc_admission::http::version_line().ends_with(&format!("sdk={sdk}")));
    let (name, label) =
        pcoc_admission::verifier::spec_identity(&pcoc::build_info().spec_tag).expect("pin");
    assert_eq!(pcoc::build_info().spec_tag, "spec-2.3.0");
    assert_eq!(name, "Satoshi Hike 2.3");
    assert_eq!(label, "2.3");
    assert_eq!(body["spec"], name);
    assert_eq!(body["spec_kind"], label);
    assert_eq!(body["config"]["index_url"], "https://index.example");
    assert!(body["config"].get("log_dir").is_none(), "{body}");
    assert!(body["config"].get("config_path").is_none(), "{body}");
    assert!(!body.to_string().contains("log_dir"), "{body}");
    assert!(!body.to_string().contains("config_path"), "{body}");
    assert!(body["config"].get("leg_rate_floor").is_none(), "{body}");
    assert_eq!(body["config"]["policy_id"], "gl-admit-v10");
    assert_eq!(body["config"]["born_height"], 0);
    assert_eq!(body["config"]["bind_threads"], 6);
    let origins = body["config"]["allowed_origins"].as_array().unwrap();
    assert!(origins.is_empty(), "the default names no site");
    assert_eq!(body["config"]["bind"], "127.0.0.1:8787");
    let (policy_status, policy) = send(
        &router,
        Request::get("/v1/policy").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(policy_status, StatusCode::OK);
    assert_eq!(policy["policy_id"], "gl-admit-v10");
    assert_eq!(policy["leg_rate_floor"]["p"], 1);
    assert_eq!(policy["leg_rate_floor"]["q"], 5);
    assert_eq!(policy["min_anchored_paths"], 6);
    assert_eq!(policy["born_height"], 0);
    assert_eq!(policy["source"], "recommendation, not specification");
    assert_eq!(policy["spec"], name);
    let text = body.to_string();
    assert!(!text.contains("friendly_list"));
    assert!(!text.contains("GL_VERIFY_"));
    assert!(!text.contains("secret"));
    assert!(body.get("config").unwrap().get("friendly_list").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_real_v135_object_is_unknown_kind_on_verify() {
    let (router, dir) = service("http://127.0.0.1:1");
    let object = v135_object();
    let (status, posted) = post_cbor(&router, object).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let job = poll_job(&router, posted["job_id"].as_str().unwrap()).await;
    assert_eq!(job["status"], "error");
    assert_eq!(job["error"]["code"], "unknown_kind");
    let message = job["error"]["message"].as_str().unwrap();
    assert!(!message.to_ascii_lowercase().contains("could not be read"));
    assert!(message.contains("v2.2"));
    assert_eq!(job["reasons"][0], "unknown_kind");

    let (status, _) = send(
        &router,
        Request::get("/latest.json").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!dir.join("chain.jsonl").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn legacy_routes_are_404() {
    let (router, dir) = service("https://index.example");
    for (method, path) in [
        ("POST", "/v1/admit"),
        ("POST", "/v1/challenge"),
        ("GET", "/v1/challenge"),
        ("POST", "/v1/sign-in/challenge"),
        ("GET", "/v1/friendly"),
        ("POST", "/admit"),
    ] {
        let request = match method {
            "POST" => Request::post(path),
            _ => Request::get(path),
        };
        let (status, body) = send(&router, request.body(Body::from("{}")).unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path} -> {body}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_fresh_log_directory_is_created_empty() {
    let dir = scratch("fresh");
    assert!(!dir.exists());
    let router = Service::open(Config::for_tests(dir.clone(), "https://index.example"))
        .unwrap()
        .router();
    assert!(dir.is_dir());
    let (status, body) = send(
        &router,
        Request::get("/latest.json").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    assert!(!dir.join("chain.jsonl").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_v135_decision_log_is_not_served() {
    let dir = scratch("oldlog");
    std::fs::create_dir_all(&dir).unwrap();
    let chain = dir.join("chain.jsonl");
    let original = b"{\"event\":\"gate.decision\",\"seq\":0,\"decision\":\"accept\",\"policy_profile_id\":\"default\"}\n";
    std::fs::write(&chain, original).unwrap();
    let error = match Service::open(Config::for_tests(dir.clone(), "https://index.example")) {
        Err(error) => error,
        Ok(_) => panic!("a v1.35 decision log was opened"),
    };
    assert!(error.contains("not a Satoshi Hike 2.2"), "{error}");
    assert_eq!(std::fs::read(&chain).unwrap(), original);
    assert!(!dir.join("latest.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

fn opened(dir: PathBuf, index_url: &str, edit: impl FnOnce(&mut Config)) -> Router {
    let mut cfg = Config::for_tests(dir, index_url);
    edit(&mut cfg);
    Service::open(cfg).expect("gate").router()
}

#[tokio::test]
async fn thorough_verify_returns_when_the_index_answers_500() {
    let index = Index::status_only("500 Internal Server Error");
    let dir = scratch("idx500");
    let router = opened(dir.clone(), &index.base, |cfg| {
        cfg.max_concurrent_verify = 1;
        cfg.index_timeout = Duration::from_millis(400);
    });
    let started = std::time::Instant::now();
    let (status, posted) = post_cbor(&router, other_kind([8u8; 32])).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job_for(
        &router,
        posted["job_id"].as_str().unwrap(),
        Duration::from_secs(3),
    )
    .await;
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "verify did not return"
    );
    assert_eq!(job["terminal"], true);
    assert_eq!(job["status"], "error");
    let reasons = job["reasons"].as_array().unwrap();
    assert!(
        reasons.iter().any(|reason| reason == "unknown_kind"),
        "{job}"
    );
    let (status, second) = post_cbor(&router, other_kind([9u8; 32])).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{second}");
    let job = poll_job_for(
        &router,
        second["job_id"].as_str().unwrap(),
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(job["terminal"], true);
    let (_, health) = send(
        &router,
        Request::get("/v1/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(health["jobs"]["in_flight"], 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn thorough_verify_returns_when_the_index_hangs() {
    let index = Index::blackhole();
    let dir = scratch("idxhang");
    let router = opened(dir.clone(), &index.base, |cfg| {
        cfg.max_concurrent_verify = 1;
        cfg.index_timeout = Duration::from_millis(400);
    });
    let started = std::time::Instant::now();
    let (status, posted) = post_cbor(&router, other_kind([8u8; 32])).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job_for(
        &router,
        posted["job_id"].as_str().unwrap(),
        Duration::from_secs(3),
    )
    .await;
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(job["terminal"], true);
    assert!(job["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "unknown_kind"));
    let (_, health) = send(
        &router,
        Request::get("/v1/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(health["jobs"]["in_flight"], 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_restart_does_not_wait_on_a_hung_index() {
    let index = Index::blackhole();
    let dir = scratch("resume");
    let submission = [4u8; 32];
    append_line(
        &dir,
        other_kind([3u8; 32]),
        9,
        submission,
        "fast",
        "provisional",
    );
    let router = opened(dir.clone(), &index.base, |cfg| {
        cfg.max_concurrent_verify = 1;
        cfg.index_timeout = Duration::from_millis(400);
    });
    // Gate 0.5.4: the resume reuses the stored draw and reads no index. This
    // provisional has no stored draw, so it is held: no thorough line, and
    // never a fresh six-leg draw against the (hung) index.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let raw = std::fs::read(dir.join("chain.jsonl")).unwrap();
    assert!(
        json_values(&raw)
            .iter()
            .all(|value| value["phase"] != "thorough"),
        "a provisional with no stored draw is held"
    );
    let (status, posted) = post_cbor(&router, vec![0x80]).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job_for(
        &router,
        posted["job_id"].as_str().unwrap(),
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(job["terminal"], true);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn latest_json_matches_the_chain_after_restart() {
    let dir = scratch("latest");
    append_line(&dir, b"one".to_vec(), 5, [1u8; 32], "thorough", "accept");
    append_line(&dir, b"two".to_vec(), 4, [2u8; 32], "thorough", "accept");
    std::fs::write(dir.join("latest.json"), br#"{"last_seq":0}"#).unwrap();
    let router = opened(dir.clone(), "http://127.0.0.1:9", |_| {});
    let (status, latest) = send(
        &router,
        Request::get("/latest.json").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(latest["last_seq"], 1);
    drop(router);
    std::fs::remove_file(dir.join("latest.json")).unwrap();
    let router = opened(dir.clone(), "http://127.0.0.1:9", |_| {});
    let (status, latest) = send(
        &router,
        Request::get("/latest.json").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(latest["last_seq"], 1);
    assert_eq!(latest["feed"], "pcoc-gate-feed-v2");
    assert_eq!(latest["policy"]["policy_id"], "gl-admit-v10");
    assert_eq!(latest["policy"]["leg_rate_floor"]["p"], 1);
    assert_eq!(latest["policy"]["leg_rate_floor"]["q"], 5);
    assert_eq!(
        latest["policy"]["source"],
        "recommendation, not specification"
    );
    let day = pcoc_admission::log::utc_day(1_700_000_000);
    let (line_status, line) = send(
        &router,
        Request::get(format!("/log/{day}/1"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(line_status, StatusCode::OK, "{line}");
    assert_eq!(line["seq"], 1);
    assert_eq!(line["decision"], "accept");
    let (miss_status, _) = send(
        &router,
        Request::get("/log/2023-11-15/1")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(miss_status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn verify_over_the_object_cap_is_refused() {
    let dir = scratch("objcapp");
    let router = opened(dir.clone(), "http://127.0.0.1:9", |cfg| {
        cfg.max_object_bytes = 260_000;
    });
    let (status, body) = post_cbor(&router, vec![0u8; 261_000]).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
    assert_eq!(body["error"]["code"], "oversize");
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("260000"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A shape match used to invent `hike_span` `not read` without calling the
/// index, so every current object finished `held`. SPEC §14 reads the index.
/// When those headers are readable and the pair meets gl-admit-v10's 1/5 floor,
/// the job is `done`.
#[tokio::test]
async fn a_readable_hike_is_done_not_held() {
    let object = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/objects/O3-PROD.cbor"),
    )
    .expect("O3");
    // Christian relocated the admit start off the seed beacon to I_0
    // (stages[1].beacon). The old pair was checkpoints over stages 0–17 and
    // MTP(seed)→MTP(last_interrupt). O3's index times are 5 s per height, so
    // the span is 5 * (end − start). Stage 0 is not in hike_legs.
    let parsed = pcoc::decode_object(&object).expect("O3");
    let seed = parsed.stages[0].beacon_height;
    let i0 = parsed.stages[1].beacon_height;
    let last = parsed.last_interrupt_height;
    let legs_admit: usize = parsed.stages[1..]
        .iter()
        .map(|stage| stage.checkpoints.len())
        .sum();
    let legs_with_seed: usize = parsed
        .stages
        .iter()
        .map(|stage| stage.checkpoints.len())
        .sum();
    assert!(i0 > seed, "the seed sits before the first interrupt");
    assert!(legs_with_seed > legs_admit, "stage 0 has checkpoints");
    let index = index_for_hike(&object);
    let dir = scratch("hike-read");
    let router = opened(dir.clone(), &index.base, |cfg| {
        cfg.index_timeout = Duration::from_secs(5);
        // This test owns its boundary. O3 is below the production value.
        cfg.policy.born_height = 0;
    });
    let (status, posted) = post_cbor(&router, object).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job_for(
        &router,
        posted["job_id"].as_str().unwrap(),
        Duration::from_secs(600),
    )
    .await;
    assert_eq!(job["status"], "done", "{job}");
    assert_eq!(job["record"]["shape"], "match", "{job}");
    assert_eq!(
        job["record"]["analysis"]["hike_legs"], legs_admit as i64,
        "{job}"
    );
    assert_ne!(
        job["record"]["analysis"]["hike_legs"], legs_with_seed as i64,
        "stage 0 is not an admission leg"
    );
    assert_eq!(
        job["record"]["analysis"]["hike_span"],
        5 * (last as i64 - i0 as i64),
        "{job}"
    );
    assert_ne!(
        job["record"]["analysis"]["hike_span"],
        5 * (last as i64 - seed as i64),
        "seed→last is not the admit span"
    );
    assert!(job["record"]["headers"].is_array(), "{job}");
    assert!(job.get("error").is_none(), "{job}");
    assert_eq!(job["policy_id"], "gl-admit-v10");
    assert_eq!(job["born_height"], 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every stored height is below the boundary this test sets. The index is a
/// black hole. The job is terminal `before_born_height` and is not `held`.
#[tokio::test]
async fn a_preborn_object_is_rejected_without_the_index() {
    let object = o3_bytes();
    let (min, max) = height_span(&object);
    let index = Index::blackhole();
    let dir = scratch("preborn");
    let born = max + 1;
    let router = opened(dir.clone(), &index.base, |cfg| {
        cfg.policy.born_height = born;
        cfg.index_timeout = Duration::from_millis(200);
    });
    let started = std::time::Instant::now();
    let (status, posted) = post_cbor(&router, object).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job_for(
        &router,
        posted["job_id"].as_str().unwrap(),
        Duration::from_secs(3),
    )
    .await;
    assert!(started.elapsed() < Duration::from_secs(3), "{job}");
    assert_eq!(job["terminal"], true, "{job}");
    assert_eq!(job["status"], "error", "{job}");
    assert_ne!(job["status"], "held", "{job}");
    assert_eq!(job["error"]["code"], "before_born_height", "{job}");
    assert_eq!(job["reasons"][0], "before_born_height", "{job}");
    assert_eq!(job["policy_id"], "gl-admit-v10", "{job}");
    assert_eq!(job["born_height"], born, "{job}");
    assert_eq!(job["lowest_height"], min, "{job}");
    assert_eq!(job["record"]["reason"], "before_born_height", "{job}");
    assert_eq!(job["record"]["born_height"], born, "{job}");
    assert_eq!(
        job["record"]["lowest_height"], job["lowest_height"],
        "{job}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The lowest stored height equals the boundary this test sets. The object
/// is accepted, the same way a readable hike is accepted at born height 0.
#[tokio::test]
async fn an_object_exactly_at_born_height_is_accepted() {
    let object = o3_bytes();
    let (min, max) = height_span(&object);
    assert!(min < max, "O3 spans more than one height");
    let index = index_for_hike(&object);
    let dir = scratch("born-exact");
    let router = opened(dir.clone(), &index.base, |cfg| {
        cfg.policy.born_height = min;
        cfg.index_timeout = Duration::from_secs(5);
    });
    let (status, posted) = post_cbor(&router, object).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job_for(
        &router,
        posted["job_id"].as_str().unwrap(),
        Duration::from_secs(600),
    )
    .await;
    assert_eq!(job["status"], "done", "{job}");
    assert_eq!(job["terminal"], true, "{job}");
    assert!(job.get("error").is_none(), "{job}");
    assert_eq!(job["policy_id"], "gl-admit-v10", "{job}");
    assert_eq!(job["born_height"], min, "{job}");
    assert!(job.get("lowest_height").is_none(), "{job}");
    assert_eq!(job["record"]["shape"], "match", "{job}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The anchor is exactly this test's born height, so the median-time window
/// starts below that height. The fake index uses the same header floor as
/// the production index: it serves `born_height - MTP_LOOKBACK` and up, and
/// it 404s anything lower. The job reaches `done`.
#[tokio::test]
async fn an_object_at_born_height_is_done_when_lookback_is_below_born() {
    let object = o3_bytes();
    let (min, max) = height_span(&object);
    assert!(min < max, "O3 spans more than one height");
    let lookback = pcoc::constants::MTP_LOOKBACK;
    assert!(min > lookback, "the window reaches below the anchor");
    let born = min;
    let window = born + 1 - born.saturating_add(1).min(pcoc::constants::MTP_WINDOW);
    assert!(window < born, "lookback lies below born_height");
    assert_eq!(window, born - lookback);
    let index = index_for_hike_from(&object, window);
    let dir = scratch("born-lookback");
    let router = opened(dir.clone(), &index.base, |cfg| {
        cfg.policy.born_height = born;
        cfg.index_timeout = Duration::from_secs(5);
    });
    let (status, posted) = post_cbor(&router, object).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job_for(
        &router,
        posted["job_id"].as_str().unwrap(),
        Duration::from_secs(600),
    )
    .await;
    assert_eq!(job["status"], "done", "{job}");
    assert_ne!(job["status"], "held", "{job}");
    assert_eq!(job["terminal"], true, "{job}");
    assert!(job.get("error").is_none(), "{job}");
    assert_eq!(job["policy_id"], "gl-admit-v10", "{job}");
    assert_eq!(job["born_height"], born, "{job}");
    assert!(job.get("lowest_height").is_none(), "{job}");
    assert_eq!(job["record"]["shape"], "match", "{job}");
    assert!(
        job["record"]["analysis"]["hike_span"].as_i64().is_some(),
        "{job}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// One stored height is below the boundary and another is not.
#[tokio::test]
async fn a_mixed_object_is_rejected_before_born_height() {
    let object = o3_bytes();
    let (min, max) = height_span(&object);
    assert!(min < max, "O3 spans more than one height");
    let index = Index::blackhole();
    let dir = scratch("born-mixed");
    let born = min + 1;
    let router = opened(dir.clone(), &index.base, |cfg| {
        cfg.policy.born_height = born;
        cfg.index_timeout = Duration::from_millis(200);
    });
    let (status, posted) = post_cbor(&router, object).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job_for(
        &router,
        posted["job_id"].as_str().unwrap(),
        Duration::from_secs(3),
    )
    .await;
    assert_eq!(job["terminal"], true, "{job}");
    assert_eq!(job["status"], "error", "{job}");
    assert_ne!(job["status"], "held", "{job}");
    assert_eq!(job["error"]["code"], "before_born_height", "{job}");
    assert_eq!(job["born_height"], born, "{job}");
    assert_eq!(job["lowest_height"], min, "{job}");
    assert!(job["lowest_height"].as_u64().unwrap() < born);
    assert!(max >= born, "the object also reaches the boundary");
    let _ = std::fs::remove_dir_all(&dir);
}

fn o3_bytes() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/objects/O3-PROD.cbor"),
    )
    .expect("O3")
}

fn height_span(object: &[u8]) -> (u64, u64) {
    let heights = pcoc_admission::policy::object_heights(object);
    let min = heights.iter().copied().min().expect("heights");
    let max = heights.iter().copied().max().expect("heights");
    (min, max)
}

/// Headers whose natural hash is the object's beacon, 5 s apart, so
/// `MTP(end) − MTP(start)` is `5 * (end − start)`.
fn index_for_hike(object: &[u8]) -> Index {
    index_for_hike_from(object, 0)
}

/// Same documents as [`index_for_hike`], served only at or above `serve_from`.
/// A lower height is the default 404. That is the production header floor:
/// `born_height - MTP_LOOKBACK`.
fn index_for_hike_from(object: &[u8], serve_from: u64) -> Index {
    let obj = pcoc::decode_object(object).expect("object");
    let mut beacons = HashMap::<u64, [u8; 32]>::new();
    for stage in &obj.stages {
        beacons.insert(stage.beacon_height, stage.beacon);
    }
    if obj.has_last_height {
        beacons.insert(obj.last_interrupt_height, obj.last_interrupt);
    }
    let mut lo = u64::MAX;
    let mut hi = 0u64;
    for &height in beacons.keys() {
        let n = height.saturating_add(1).min(pcoc::constants::MTP_WINDOW);
        let start = height + 1 - n;
        lo = lo.min(start);
        hi = hi.max(height);
    }
    let mut exact = HashMap::new();
    for height in lo..=hi {
        if height < serve_from {
            continue;
        }
        let natural = beacons.get(&height).copied().unwrap_or([0x11; 32]);
        let display = pcoc::hexutil::hex_encode(&pcoc::hexutil::reverse32(&natural));
        let time = 1_600_000_000u64 + 5 * height;
        let body = format!(
            "{{\"network\":\"synthetic\",\"header\":{{\"hash\":\"{display}\",\"height\":{height},\"time\":{time}}}}}"
        );
        exact.insert(
            format!("/index/{height}"),
            Reply::Body {
                status: "200 OK".to_owned(),
                body,
            },
        );
    }
    Index::serve(
        exact,
        Reply::Body {
            status: "404 Not Found".to_owned(),
            body: String::new(),
        },
    )
}

/// Rule A5. A verify job for any other kind is `unknown_kind`.
/// SPEC §14.7. The 2.0 pack record is not spliced.
#[tokio::test]
async fn an_other_kind_verify_job_is_unknown_kind() {
    let (router, dir) = service("http://127.0.0.1:1");
    let (status, posted) = post_cbor(&router, other_kind([7u8; 32])).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{posted}");
    let job = poll_job(&router, posted["job_id"].as_str().unwrap()).await;
    assert_eq!(job["terminal"], true, "{job}");
    assert_eq!(job["error"]["code"], "unknown_kind");
    assert_eq!(job["reasons"][0], "unknown_kind");
    assert_eq!(job["record"]["kind"], "unknown kind");
    let _ = std::fs::remove_dir_all(&dir);
}

fn plant_document(waypoint: &[u8; 32]) -> String {
    let mut pre = [0u8; 33];
    pre[0] = 0x1A;
    pre[1..].copy_from_slice(waypoint);
    let root = hex::encode(Sha256::digest(pre));
    format!(
        r#"{{"network":"synthetic","plant":{{"root":"{root}","txid":"{txid}","vout":2,"block_hash":"{block}","block_height":4}}}}"#,
        txid = "44".repeat(32),
        block = "55".repeat(32),
    )
}

async fn poll_job_text(
    router: &Router,
    job_id: &str,
    budget: Duration,
) -> (serde_json::Value, String) {
    let deadline = std::time::Instant::now() + budget;
    while std::time::Instant::now() < deadline {
        let response = router
            .clone()
            .oneshot(with_peer(
                Request::get(format!("/v1/verify/{job_id}"))
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .expect("router response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let text = String::from_utf8(bytes.to_vec()).expect("utf-8");
        let body: serde_json::Value = serde_json::from_str(&text).expect("job json");
        if body["terminal"] == true {
            return (body, text);
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    panic!("verify job {job_id} did not finish within {budget:?}");
}
