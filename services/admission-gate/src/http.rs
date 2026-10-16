//! HTTP door.
//!
//! ```text
//! POST /v1/verify            -> 202 { "job_id": "…" }
//! GET  /v1/verify/{job_id}   -> 200 status and, when measured, the section 14 record
//! GET  /v1/health            -> 200 version, commit, spec kind, config summary
//! POST /v1/sign-in           -> locked §15.1 wire
//! GET  /v1/sign-in/result/{nonce} -> { "outcome" }
//! GET  /latest.json          -> unsigned tip, after a durable line, plus policy
//! GET  /log/{YYYY-MM-DD}.jsonl -> decision lines for that UTC date
//! GET  /log/{YYYY-MM-DD}/{seq} -> one decision object
//! GET  /v1/policy            -> the bound admission policy
//! ```
//!
//! The `/v1` prefix stays because the website and the phone already call it.
//! `POST /v1/admit`, `POST /v1/challenge`, and the friendly-list flag are not
//! routes. This process does not mint a session.

use crate::config::Config;
use crate::door::{Door, LockedThorough, ThoroughJob};
use crate::jobs::{EnqueueError, JobStore};
use crate::policy::{self, AdmissionPolicy};
use crate::ratelimit::RateLimiter;
use crate::upload;
use crate::verifier::SPEC_KIND_LABEL;
use crate::wire::{self, LockedParse};
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

pub const SERVICE: &str = "admission-service";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Git SHA recorded at build time. See `build.rs`.
pub const COMMIT: &str = env!("ADMISSION_COMMIT");
pub use crate::verifier::SPEC_NAME;

#[derive(Clone)]
pub struct Service {
    cfg: Arc<Config>,
    jobs: JobStore,
    limiter: Arc<RateLimiter>,
    door: Door,
}

impl Service {
    pub fn open(cfg: Config) -> Result<Self, String> {
        let cfg = Arc::new(cfg);
        let limiter = Arc::new(RateLimiter::per_minute(cfg.rate_per_min));
        let door = Door::open(&cfg.log_dir, cfg.policy())?;
        let jobs = JobStore::new(cfg.clone());
        for job in door.unsettled() {
            spawn_thorough(door.clone(), jobs.clone(), job);
        }
        Ok(Self {
            jobs,
            cfg,
            limiter,
            door,
        })
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    pub fn door(&self) -> Door {
        self.door.clone()
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/v1/health", get(health).options(preflight))
            .route("/v1/policy", get(get_policy).options(preflight))
            .route("/v1/verify", post(post_verify).options(preflight))
            .route("/v1/verify/{job_id}", get(get_verify).options(preflight))
            .route("/v1/sign-in", post(post_sign_in).options(preflight))
            .route(
                "/v1/sign-in/result/{nonce}",
                get(get_sign_in_result).options(preflight),
            )
            .route("/latest.json", get(get_latest).options(preflight))
            .route("/log/{day}/{seq}", get(get_line).options(preflight))
            .route("/log/{name}", get(get_day).options(preflight))
            .fallback(unknown_route)
            .layer(axum::extract::DefaultBodyLimit::max(
                self.cfg.max_body_bytes(),
            ))
            .with_state(self.clone())
    }
}

#[derive(Serialize)]
struct HealthBody {
    ok: bool,
    service: &'static str,
    version: &'static str,
    commit: &'static str,
    spec: &'static str,
    spec_kind: &'static str,
    config: ConfigSummary,
    jobs: JobCounts,
}

#[derive(Serialize)]
struct ConfigSummary {
    bind: String,
    index_url: String,
    max_object_bytes: usize,
    job_retention_secs: u64,
    max_jobs: usize,
    max_concurrent: usize,
    rate_per_min: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_ip_header: Option<String>,
    allowed_origins: Vec<String>,
    extra_origins: Vec<String>,
    bind_threads: usize,
    index_timeout_secs: u64,
    policy_id: String,
}

#[derive(Serialize)]
struct JobCounts {
    tracked: usize,
    in_flight: usize,
}

async fn health(State(svc): State<Service>, headers: HeaderMap) -> Response {
    let (tracked, in_flight) = svc.jobs.counts();
    let body = HealthBody {
        ok: true,
        service: SERVICE,
        version: VERSION,
        commit: COMMIT,
        spec: SPEC_NAME,
        spec_kind: SPEC_KIND_LABEL,
        config: ConfigSummary {
            bind: svc.cfg.bind.to_string(),
            index_url: svc.cfg.index_url.clone(),
            max_object_bytes: svc.cfg.max_object_bytes,
            job_retention_secs: svc.cfg.job_retention.as_secs(),
            max_jobs: svc.cfg.max_jobs,
            max_concurrent: svc.cfg.max_concurrent_verify,
            rate_per_min: svc.cfg.rate_per_min,
            client_ip_header: svc.cfg.client_ip_header.clone(),
            allowed_origins: svc.cfg.allowed_origins.clone(),
            extra_origins: svc.cfg.extra_origins.clone(),
            bind_threads: svc.cfg.bind_threads,
            index_timeout_secs: svc.cfg.index_timeout.as_secs(),
            policy_id: svc.cfg.policy.policy_id.clone(),
        },
        jobs: JobCounts { tracked, in_flight },
    };
    ok_response(&svc, &headers, serde_json::to_value(body).expect("health"))
}

async fn unknown_route(State(svc): State<Service>, headers: HeaderMap) -> Response {
    error_response(
        &svc,
        &headers,
        StatusCode::NOT_FOUND,
        "not_found",
        "no such route",
    )
}

async fn post_verify(
    State(svc): State<Service>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let object = match read_uploaded_object(&svc, &headers, peer, body) {
        Ok(bytes) => bytes,
        Err(refusal) => return refusal.into_response(&svc, &headers),
    };
    match svc.jobs.enqueue(object) {
        Ok(job_id) => with_cors(
            &svc,
            &headers,
            (StatusCode::ACCEPTED, Json(json!({ "job_id": job_id }))).into_response(),
        ),
        Err(EnqueueError::Busy) => error_response(
            &svc,
            &headers,
            StatusCode::SERVICE_UNAVAILABLE,
            "busy",
            "the verify door is at capacity; retry shortly",
        ),
    }
}

struct Refusal {
    status: StatusCode,
    code: &'static str,
    message: String,
    retry_after_secs: Option<u64>,
}

impl Refusal {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after_secs: None,
        }
    }

    fn into_response(self, svc: &Service, headers: &HeaderMap) -> Response {
        let mut response = error_response(svc, headers, self.status, self.code, self.message);
        if let Some(secs) = self.retry_after_secs {
            if let Ok(value) = HeaderValue::from_str(&secs.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }
        response
    }
}

fn rate_limit(svc: &Service, headers: &HeaderMap, peer: SocketAddr) -> Result<(), Refusal> {
    let client = client_key(svc, headers, peer);
    match svc.limiter.admit(&client) {
        Ok(()) => Ok(()),
        Err(rejected) => Err(Refusal {
            retry_after_secs: Some(rejected.retry_after_secs),
            ..Refusal::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                format!(
                    "the door accepts {} verify and sign-in requests per minute per client; retry in {} s",
                    rejected.per_window, rejected.retry_after_secs
                ),
            )
        }),
    }
}

fn read_uploaded_object(
    svc: &Service,
    headers: &HeaderMap,
    peer: SocketAddr,
    body: Result<Bytes, BytesRejection>,
) -> Result<Vec<u8>, Refusal> {
    rate_limit(svc, headers, peer)?;
    let body = match body {
        Ok(bytes) => bytes,
        Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return Err(Refusal::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "oversize",
                format!(
                    "request body is over the {} byte transport limit; objects up to {} bytes are accepted",
                    svc.cfg.max_body_bytes(),
                    svc.cfg.max_object_bytes
                ),
            ))
        }
        Err(rejection) => {
            return Err(Refusal::new(
                StatusCode::BAD_REQUEST,
                "bad_request",
                format!(
                    "the transport rejected the request body before the object was read: {}",
                    rejection.body_text()
                ),
            ))
        }
    };
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    match upload::extract_object(content_type, &body, svc.cfg.max_object_bytes) {
        Ok(bytes) => Ok(bytes),
        Err(err) => {
            let status = match err {
                upload::UploadError::Oversize { .. } => StatusCode::PAYLOAD_TOO_LARGE,
                upload::UploadError::UnsupportedMediaType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                _ => StatusCode::BAD_REQUEST,
            };
            Err(Refusal::new(status, err.code(), err.message()))
        }
    }
}

async fn get_verify(
    State(svc): State<Service>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> Response {
    match svc.jobs.snapshot(&job_id) {
        Some(body) => {
            let mut response = (StatusCode::OK, body).into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            with_cors(&svc, &headers, response)
        }
        None => error_response(
            &svc,
            &headers,
            StatusCode::NOT_FOUND,
            "not_found",
            "no such job: it expired, or the process restarted; upload the object again",
        ),
    }
}

async fn post_sign_in(
    State(svc): State<Service>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let locked_json = content_type.as_deref().is_some_and(is_json_mime);
    let bytes = match body {
        Ok(bytes) => bytes,
        Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            if let Err(refusal) = rate_limit(&svc, &headers, peer) {
                return refusal.into_response(&svc, &headers);
            }
            // The bytes were dropped, so this cannot record a website outcome.
            // A 413 is delivery OK.
            return error_response(
                &svc,
                &headers,
                StatusCode::PAYLOAD_TOO_LARGE,
                "oversize",
                format!(
                    "request body is over the {} byte transport limit",
                    svc.cfg.max_body_bytes()
                ),
            );
        }
        Err(rejection) => {
            return error_response(
                &svc,
                &headers,
                StatusCode::BAD_REQUEST,
                "bad_request",
                format!(
                    "the transport rejected the request body before the object was read: {}",
                    rejection.body_text()
                ),
            )
        }
    };
    if let Err(refusal) = rate_limit(&svc, &headers, peer) {
        if locked_json && refusal.code == "rate_limited" {
            svc.door.note_transport_rejected(&bytes);
            return bare_delivery(&svc, &headers, StatusCode::TOO_MANY_REQUESTS);
        }
        return refusal.into_response(&svc, &headers);
    }
    if !locked_json {
        return error_response(
            &svc,
            &headers,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "POST /v1/sign-in takes application/json",
        );
    }
    sign_in_locked(&svc, &headers, &bytes)
}

fn is_json_mime(content_type: &str) -> bool {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/json")
}

/// Six-field body. Step 1 (`malformed`) covers a value that is not one JSON
/// object and a nonce that is not 64 lowercase hex, including
/// `{object_b64, signature}`.
fn sign_in_locked(svc: &Service, headers: &HeaderMap, bytes: &[u8]) -> Response {
    let locked = match wire::classify_json(bytes) {
        Ok(LockedParse::Ready(body)) => body,
        Ok(LockedParse::NotJsonOrNonce) | Err(()) => {
            return bare_code(svc, headers, "malformed");
        }
        Ok(LockedParse::Grammar {
            nonce,
            object,
            object_sha,
        }) => {
            let matches = object_sha.is_some_and(|sha| crate::canon::sha256(&object) == sha);
            svc.door.note_grammar_miss(&nonce, matches);
            return bare_code(svc, headers, "malformed");
        }
    };
    if locked.object.len() > svc.cfg.max_object_bytes {
        svc.door.note_transport_rejected(bytes);
        return bare_delivery(svc, headers, StatusCode::PAYLOAD_TOO_LARGE);
    }
    let result = svc.door.sign_in_locked(locked);
    if let Some(job) = result.thorough.clone() {
        spawn_locked(svc.door.clone(), svc.jobs.clone(), job);
    }
    locked_response(svc, headers, result.status, result.body)
}

/// Delivery OK with no website-facing detail. The client shows
/// `Sign-in submitted` for any completed response other than the two
/// HTTP 400 bodies.
fn bare_delivery(svc: &Service, headers: &HeaderMap, status: StatusCode) -> Response {
    with_cors(svc, headers, status.into_response())
}

fn bare_code(svc: &Service, headers: &HeaderMap, code: &'static str) -> Response {
    let body = Json(json!({ "error": { "code": code } }));
    with_cors(
        svc,
        headers,
        (StatusCode::BAD_REQUEST, body).into_response(),
    )
}

fn locked_response(
    svc: &Service,
    headers: &HeaderMap,
    status: StatusCode,
    body: Value,
) -> Response {
    with_cors(svc, headers, (status, Json(body)).into_response())
}

fn spawn_locked(door: Door, jobs: JobStore, job: LockedThorough) {
    tokio::spawn(async move {
        // A prior thorough accept with policy_id skips measurement.
        if job.prior_accept.is_some() {
            door.settle_locked(job, Ok(crate::verifier::Report::Unreadable));
            return;
        }
        let object = job.object.clone();
        let report = jobs.measure(object).await;
        door.settle_locked(job, report);
    });
}

async fn get_sign_in_result(
    State(svc): State<Service>,
    headers: HeaderMap,
    Path(nonce): Path<String>,
) -> Response {
    let Some(nonce) = wire::nonce_from_path(&nonce) else {
        return error_response(
            &svc,
            &headers,
            StatusCode::NOT_FOUND,
            "not_found",
            "no such route",
        );
    };
    // Unknown nonce: {"outcome":"pending"}. Consumed pending adds submitted.
    // accepted adds public_key, object_sha256, and numeric bookends.
    let body = svc.door.sign_in_result(&nonce);
    with_cors(&svc, &headers, (StatusCode::OK, Json(body)).into_response())
}

fn spawn_thorough(door: Door, jobs: JobStore, job: ThoroughJob) {
    tokio::spawn(async move {
        let object = job.object.clone();
        let report = jobs.measure(object).await;
        door.settle(job, report).await;
    });
}

async fn get_latest(State(svc): State<Service>, headers: HeaderMap) -> Response {
    match svc.door.latest_bytes() {
        Some(bytes) => bytes_response(&svc, &headers, "application/json", bytes),
        None => error_response(
            &svc,
            &headers,
            StatusCode::NOT_FOUND,
            "not_found",
            "the decision log has no line yet",
        ),
    }
}

async fn get_policy(State(svc): State<Service>, headers: HeaderMap) -> Response {
    ok_response(&svc, &headers, policy_body(svc.door.policy()))
}

fn policy_body(policy: &AdmissionPolicy) -> Value {
    policy::public_document(policy)
}

async fn get_line(
    State(svc): State<Service>,
    headers: HeaderMap,
    Path((day, seq)): Path<(String, String)>,
) -> Response {
    let Some(day) = utc_date(&day) else {
        return error_response(
            &svc,
            &headers,
            StatusCode::NOT_FOUND,
            "not_found",
            "no such log",
        );
    };
    let Ok(seq) = seq.parse::<u64>() else {
        return error_response(
            &svc,
            &headers,
            StatusCode::NOT_FOUND,
            "not_found",
            "no such log",
        );
    };
    match svc.door.line_bytes(day, seq) {
        Some(bytes) => bytes_response(&svc, &headers, "application/json", bytes),
        None => error_response(
            &svc,
            &headers,
            StatusCode::NOT_FOUND,
            "not_found",
            "no decision line with that seq falls on that UTC date",
        ),
    }
}

async fn get_day(
    State(svc): State<Service>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    let Some(day) = utc_day_name(&name) else {
        return error_response(
            &svc,
            &headers,
            StatusCode::NOT_FOUND,
            "not_found",
            "no such log",
        );
    };
    match svc.door.day_bytes(day) {
        Some(bytes) => bytes_response(&svc, &headers, "application/x-ndjson", bytes),
        None => error_response(
            &svc,
            &headers,
            StatusCode::NOT_FOUND,
            "not_found",
            "no decision line falls on that UTC date",
        ),
    }
}

fn utc_day_name(name: &str) -> Option<&str> {
    let day = name.strip_suffix(".jsonl")?;
    utc_date(day)
}

fn utc_date(day: &str) -> Option<&str> {
    let bytes = day.as_bytes();
    if bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        Some(day)
    } else {
        None
    }
}

fn bytes_response(
    svc: &Service,
    request_headers: &HeaderMap,
    content_type: &'static str,
    bytes: Vec<u8>,
) -> Response {
    let response = (
        [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
        bytes,
    )
        .into_response();
    with_cors(svc, request_headers, response)
}

async fn preflight(State(svc): State<Service>, headers: HeaderMap) -> Response {
    with_cors(&svc, &headers, StatusCode::NO_CONTENT.into_response())
}

fn ok_response(svc: &Service, request_headers: &HeaderMap, body: Value) -> Response {
    with_cors(svc, request_headers, Json(body).into_response())
}

fn error_response(
    svc: &Service,
    request_headers: &HeaderMap,
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
) -> Response {
    let body = Json(json!({ "error": { "code": code, "message": message.into() } }));
    with_cors(svc, request_headers, (status, body).into_response())
}

fn with_cors(svc: &Service, request_headers: &HeaderMap, mut response: Response) -> Response {
    for (name, value) in cors_headers_for(&svc.cfg, request_headers).iter() {
        response.headers_mut().insert(name, value.clone());
    }
    response
}

fn client_key(svc: &Service, headers: &HeaderMap, peer: SocketAddr) -> String {
    if let Some(name) = &svc.cfg.client_ip_header {
        if let Some(value) = headers
            .get(name.as_str())
            .and_then(|value| value.to_str().ok())
        {
            let first = value.split(',').next().unwrap_or("").trim();
            if !first.is_empty() {
                return first.to_owned();
            }
        }
    }
    peer.ip().to_string()
}

fn origin_allowed(cfg: &Config, origin: &str) -> bool {
    if cfg.allowed_origins.iter().any(|allowed| allowed == origin)
        || cfg.extra_origins.iter().any(|allowed| allowed == origin)
    {
        return true;
    }
    for loopback in [
        "http://localhost",
        "https://localhost",
        "http://127.0.0.1",
        "http://[::1]",
    ] {
        if let Some(rest) = origin.strip_prefix(loopback) {
            if rest.is_empty() || rest.starts_with(':') {
                return true;
            }
        }
    }
    false
}

fn cors_headers_for(cfg: &Config, request_headers: &HeaderMap) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Accept, Content-Type"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
    let Some(origin) = request_headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return headers;
    };
    if origin_allowed(cfg, origin) {
        if let Ok(value) = HeaderValue::from_str(origin) {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
        }
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn the_default_allows_no_site() {
        assert!(!origin_allowed(&cfg(), "https://groundinglab.io"));
        assert!(!origin_allowed(&cfg(), "https://site.example"));
    }

    #[test]
    fn configured_origins_are_allowed() {
        let mut cfg = cfg();
        cfg.allowed_origins = vec![
            "https://groundinglab.io".to_owned(),
            "https://www.groundinglab.io".to_owned(),
        ];
        assert!(origin_allowed(&cfg, "https://groundinglab.io"));
        assert!(origin_allowed(&cfg, "https://www.groundinglab.io"));
        assert!(!origin_allowed(&cfg, "http://groundinglab.io.evil.example"));
    }

    #[test]
    fn loopback_any_port_is_allowed_for_lab_pages() {
        assert!(origin_allowed(&cfg(), "http://127.0.0.1:8080"));
        assert!(origin_allowed(&cfg(), "http://localhost:8080"));
        assert!(origin_allowed(&cfg(), "http://[::1]:8080"));
    }

    #[test]
    fn private_lan_http_is_not_allowed() {
        assert!(!origin_allowed(&cfg(), "http://198.51.100.5:8080"));
        assert!(!origin_allowed(&cfg(), "http://lab.example:8080"));
    }

    #[test]
    fn unrelated_origin_is_refused() {
        assert!(!origin_allowed(&cfg(), "https://evil.example"));
        assert!(!origin_allowed(&cfg(), "http://groundinglab.io.evil.example"));
    }

    #[test]
    fn extra_origins_are_honoured() {
        let mut cfg = cfg();
        cfg.extra_origins = vec!["https://preview.example".to_owned()];
        assert!(origin_allowed(&cfg, "https://preview.example"));
    }

    #[test]
    fn an_empty_allow_list_keeps_loopback_only() {
        let mut cfg = cfg();
        cfg.allowed_origins.clear();
        assert!(!origin_allowed(&cfg, "https://groundinglab.io"));
        assert!(!origin_allowed(&cfg, "https://www.groundinglab.io"));
        assert!(origin_allowed(&cfg, "http://127.0.0.1:8080"));
    }
}
