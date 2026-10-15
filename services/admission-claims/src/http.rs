//! `GET /health` and `POST /badge`.
//!
//! Handler order for a badge: rate limit, parse and verify, insert the nonce,
//! read the access line, demo check, `BEGIN IMMEDIATE`, then publish the
//! stored bytes.

use crate::award::{self, badge_row};
use crate::clock::{Clock, SystemClock};
use crate::config::{Config, Role};
use crate::error::Fail;
use crate::key::{self, IssuerKey};
use crate::logproof;
use crate::nostr;
use crate::publish::{self, artifact_index_matches, render, RelaySender};
use crate::rate::RateLimiter;
use crate::s3::R2Client;
use crate::store::{BadgeRow, Store};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, RawQuery, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT: &str = env!("CLAIMS_COMMIT");

struct Inner {
    cfg: Config,
    store: Option<Store>,
    issuer: Option<IssuerKey>,
    clock: Arc<dyn Clock>,
    http: reqwest::Client,
    relay: RelaySender,
    r2: Option<R2Client>,
    ip_limit: RateLimiter,
    pk_limit: RateLimiter,
    avail_limit: RateLimiter,
    logged_pubkey: AtomicBool,
}

#[derive(Clone)]
pub struct Service {
    inner: Arc<Inner>,
}

impl Service {
    pub fn open(cfg: Config) -> Result<Self, String> {
        let relay = RelaySender::with_public(
            RelaySender::websocket(cfg.relay_url.clone(), cfg.relay_timeout),
            cfg.public_relays.clone(),
            cfg.relay_timeout,
        );
        Self::open_with(cfg, Arc::new(SystemClock), relay)
    }

    pub fn open_with(
        cfg: Config,
        clock: Arc<dyn Clock>,
        relay: RelaySender,
    ) -> Result<Self, String> {
        let issuer = key::load_for_role(&cfg)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(cfg.gate_timeout)
            .build()
            .map_err(|err| format!("could not build gate client: {err}"))?;
        let r2 = if cfg.role == Role::Writer && !cfg.dir_mode() {
            Some(R2Client::new(
                &cfg.r2_account_id,
                &cfg.r2_bucket,
                &cfg.r2_token_file,
                cfg.r2_timeout,
            )?)
        } else {
            None
        };
        let store = match cfg.role {
            Role::Writer => Some(Store::open_writer(&cfg.db_path)?),
            Role::Mirror => {
                if cfg.db_path.exists() {
                    Some(Store::open_read(&cfg.db_path)?)
                } else {
                    None
                }
            }
        };
        if let (Role::Writer, Some(store)) = (cfg.role, store.as_ref()) {
            if store.name_count()? > 0 && cfg.name_event_kind != crate::nostr::KIND_NAME {
                return Err("name_event_kind is frozen once a name row exists".into());
            }
        }
        let service = Self {
            inner: Arc::new(Inner {
                ip_limit: RateLimiter::per_minute(cfg.claim_per_ip_per_min),
                pk_limit: RateLimiter::per_minute(cfg.claim_per_pubkey_per_min),
                avail_limit: RateLimiter::per_minute(cfg.availability_per_ip_per_min),
                cfg,
                store,
                issuer,
                clock,
                http,
                relay,
                r2,
                logged_pubkey: AtomicBool::new(false),
            }),
        };
        Ok(service)
    }

    pub async fn prepare(&self) -> Result<(), String> {
        if self.inner.cfg.role != Role::Writer {
            return Ok(());
        }
        self.note_pubkey();
        self.ensure_definition()?;
        let now = self.inner.clock.unix_s();
        publish::publish_stored(
            self.store()?,
            &self.inner.cfg,
            &self.inner.relay,
            self.inner.r2.as_ref(),
            now,
        )
        .await?;
        let _ = self.inner.cfg.backup_interval_secs;
        Ok(())
    }

    pub fn config(&self) -> &Config {
        &self.inner.cfg
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health).options(preflight))
            .route("/badge", post(post_badge).options(preflight))
            .route("/name", post(post_name).options(preflight))
            .route("/name/available", get(get_available).options(preflight))
            .fallback(unknown)
            .layer(DefaultBodyLimit::max(64 * 1024))
            .with_state(self.clone())
    }

    fn store(&self) -> Result<&Store, String> {
        self.inner
            .store
            .as_ref()
            .ok_or_else(|| "database is not open".to_owned())
    }

    fn note_pubkey(&self) {
        if self.inner.logged_pubkey.swap(true, Ordering::Relaxed) {
            return;
        }
        if let Some(key) = &self.inner.issuer {
            tracing::info!(issuer_pubkey = %key.pubkey_hex(), "issuer public key loaded");
        }
    }

    fn ensure_definition(&self) -> Result<(), String> {
        let store = self.store()?;
        let cfg = &self.inner.cfg;
        let stored = store.definition(&cfg.badge_d)?;
        if let Some(row) = &stored {
            let same = crate::nostr::Event::from_json(&row.event_json)
                .map(|event| event.tags == award::definition_tags(cfg))
                .unwrap_or(false);
            if same {
                return Ok(());
            }
        }
        let key = self
            .inner
            .issuer
            .as_ref()
            .ok_or_else(|| "writer has no issuer key".to_owned())?;
        let event = award::definition_event(cfg, key.secret(), self.inner.clock.unix_s())?;
        if stored.is_some() {
            tracing::warn!(event_id = %event.id, "badge definition re-signed with new tags");
            store.replace_definition(&cfg.badge_d, &event)?;
        } else {
            store.insert_definition(&cfg.badge_d, &event)?;
        }
        Ok(())
    }

    fn artifact_index_ok(&self) -> bool {
        let Ok(store) = self.store() else {
            return files_only_ok(&self.inner.cfg);
        };
        let Ok(definitions) = store.definitions() else {
            return false;
        };
        let Ok(badges) = store.badges() else {
            return false;
        };
        let Ok(names) = store.names() else {
            return false;
        };
        let Ok(files) = render(&definitions, &badges, &names) else {
            return false;
        };
        artifact_index_matches(&self.inner.cfg.artifact_dir, &files)
    }
}

fn files_only_ok(cfg: &Config) -> bool {
    let index = match std::fs::read(cfg.artifact_dir.join("claims/index.json")) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let nostr = match std::fs::read(cfg.artifact_dir.join(".well-known/nostr.json")) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    crate::nostr_json::from_index(&index).ok().as_deref() == Some(nostr.as_slice())
}

#[derive(Serialize)]
struct HealthBody<'a> {
    role: &'a str,
    version: &'a str,
    commit: &'a str,
    db: &'a str,
    key: &'a str,
    outbox: i64,
    artifact_index_ok: bool,
    now: i64,
}

async fn health(State(svc): State<Service>, headers: HeaderMap) -> Response {
    let db_ok = svc
        .inner
        .store
        .as_ref()
        .map(|store| store.db_ok())
        .unwrap_or(true);
    let outbox = svc
        .inner
        .store
        .as_ref()
        .and_then(|store| store.unacked().ok())
        .unwrap_or(0);
    let body = HealthBody {
        role: match svc.inner.cfg.role {
            Role::Writer => "writer",
            Role::Mirror => "mirror",
        },
        version: VERSION,
        commit: COMMIT,
        db: if db_ok { "ok" } else { "error" },
        key: if svc.inner.issuer.is_some() {
            "loaded"
        } else {
            "absent"
        },
        outbox,
        artifact_index_ok: svc.artifact_index_ok(),
        now: svc.inner.clock.unix_s(),
    };
    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    with_cors(&svc, &headers, (status, Json(body)).into_response())
}

async fn post_badge(State(svc): State<Service>, headers: HeaderMap, body: Bytes) -> Response {
    let response = match claim(&svc, &headers, &body).await {
        Ok(response) => {
            tracing::warn!("badge claim answered with an award");
            response
        }
        Err(fail) => {
            tracing::warn!(code = fail.code, status = fail.status.as_u16(), "badge claim refused");
            fail.into_response()
        }
    };
    with_cors(&svc, &headers, response)
}

async fn claim(svc: &Service, headers: &HeaderMap, body: &[u8]) -> Result<Response, Fail> {
    if svc.inner.cfg.role != Role::Writer {
        return Err(Fail::unavailable());
    }
    let ip = client_ip(headers, &svc.inner.cfg.client_ip_header);
    if !svc.inner.ip_limit.admit(&ip) {
        return Err(Fail::rate_limited());
    }
    if let Some(content_type) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    {
        if !content_type
            .to_ascii_lowercase()
            .starts_with("application/json")
        {
            return Err(Fail::malformed());
        }
    }
    let now = svc.inner.clock.unix_s();
    let parsed = nostr::parse_badge_request(body)?;
    if !svc.inner.pk_limit.admit(&parsed.pubkey) {
        return Err(Fail::rate_limited());
    }
    let store = svc.store().map_err(|_| Fail::unavailable())?;
    let admission = logproof::prove(logproof::ProofRequest {
        client: &svc.inner.http,
        gate_base: &svc.inner.cfg.gate_base,
        day: &parsed.day,
        seq: parsed.seq,
        now,
        window_secs: svc.inner.cfg.claim_window_secs,
        future_skew: svc.inner.cfg.line_future_skew_secs,
        max_bytes: svc.inner.cfg.max_log_bytes,
        timeout: svc.inner.cfg.gate_timeout,
    })
    .await?;
    if admission.pubkey != parsed.pubkey {
        return Err(Fail::no_access());
    }
    if svc
        .inner
        .cfg
        .demo_object_sha256
        .iter()
        .any(|hash| hash == &admission.object_sha256)
    {
        return Err(Fail::demo());
    }
    let key = svc.inner.issuer.as_ref().ok_or_else(Fail::unavailable)?;
    let built = award::award_event(&svc.inner.cfg, key.secret(), &admission, now)
        .map_err(|_| Fail::unavailable())?;
    let draft = badge_row(&built, &admission);
    let (stored, _fresh) = store.insert_badge(&draft).map_err(|err| {
        tracing::warn!(error = %err, "badge row was not stored");
        Fail::unavailable()
    })?;
    if let Err(err) = publish::publish_stored(
        store,
        &svc.inner.cfg,
        &svc.inner.relay,
        svc.inner.r2.as_ref(),
        now,
    )
    .await
    {
        tracing::warn!(error = %err, "publish left the row in the outbox");
    }
    ok_award(&svc.inner.cfg, &stored)
}

async fn post_name(State(svc): State<Service>, headers: HeaderMap, body: Bytes) -> Response {
    let response = match claim_name(&svc, &headers, &body).await {
        Ok(response) => {
            tracing::warn!("name claim answered with a record");
            response
        }
        Err(fail) => {
            tracing::warn!(code = fail.code, status = fail.status.as_u16(), "name claim refused");
            fail.into_response()
        }
    };
    with_cors(&svc, &headers, response)
}

fn log_blocked(svc: &Service, name: &str, who: &str, ip: &str) {
    tracing::warn!(
        name = %name,
        npub = %who,
        ip = %ip,
        unix_s = svc.inner.clock.unix_s(),
        "blocked name attempt"
    );
}

/// Rules, then the block list. Shared by the claim and the availability check.
fn check_name(svc: &Service, name: &str, who: &str, ip: &str) -> Result<(), Fail> {
    if !crate::names::valid(name) {
        return Err(Fail::invalid());
    }
    if crate::names::blocked(&svc.inner.cfg.blocked_names_file, name) {
        log_blocked(svc, name, who, ip);
        return Err(Fail::reserved());
    }
    Ok(())
}

async fn claim_name(svc: &Service, headers: &HeaderMap, body: &[u8]) -> Result<Response, Fail> {
    if svc.inner.cfg.role != Role::Writer {
        return Err(Fail::unavailable());
    }
    let ip = client_ip(headers, &svc.inner.cfg.client_ip_header);
    if !svc.inner.ip_limit.admit(&ip) {
        return Err(Fail::rate_limited());
    }
    let parsed = nostr::parse_badge_request(body)?;
    let value: Value = serde_json::from_slice(body).map_err(|_| Fail::malformed())?;
    let name = value.get("name").and_then(Value::as_str).ok_or_else(Fail::malformed)?;
    let npub = value.get("npub").and_then(Value::as_str).unwrap_or("");
    if !svc.inner.pk_limit.admit(&parsed.pubkey) {
        return Err(Fail::rate_limited());
    }
    check_name(svc, name, npub, &ip)?;
    let store = svc.store().map_err(|_| Fail::unavailable())?;
    // Already done? Answer before the window check so a retry still gets its record.
    if let Some(existing) = store
        .name_by(crate::store::NameKey::Pubkey, &parsed.pubkey)
        .map_err(|_| Fail::unavailable())?
    {
        if existing.name == name {
            return ok_name(svc, &existing);
        }
        return Err(Fail::new(StatusCode::CONFLICT, crate::error::HAS_NAME));
    }
    let now = svc.inner.clock.unix_s();
    let admission = logproof::prove(logproof::ProofRequest {
        client: &svc.inner.http,
        gate_base: &svc.inner.cfg.gate_base,
        day: &parsed.day,
        seq: parsed.seq,
        now,
        window_secs: svc.inner.cfg.claim_window_secs,
        future_skew: svc.inner.cfg.line_future_skew_secs,
        max_bytes: svc.inner.cfg.max_log_bytes,
        timeout: svc.inner.cfg.gate_timeout,
    })
    .await?;
    if admission.pubkey != parsed.pubkey {
        return Err(Fail::no_access());
    }
    if svc
        .inner
        .cfg
        .demo_object_sha256
        .iter()
        .any(|hash| hash == &admission.object_sha256)
    {
        return Err(Fail::demo());
    }
    let key = svc.inner.issuer.as_ref().ok_or_else(Fail::unavailable)?;
    let event = award::name_event(&svc.inner.cfg, key.secret(), name, &admission, now)
        .map_err(|_| Fail::unavailable())?;
    let row = crate::store::NameRow {
        name: name.to_owned(),
        pubkey: parsed.pubkey.clone(),
        event_id: event.id.clone(),
        event_json: event.to_json(),
        created_unix_s: event.created_at,
    };
    let stored = match store.insert_name(&row).map_err(|err| {
        tracing::warn!(error = %err, "name row was not stored");
        Fail::unavailable()
    })? {
        crate::store::NameInsert::Fresh(row) | crate::store::NameInsert::Existing(row) => row,
        crate::store::NameInsert::HasName(_) => {
            return Err(Fail::new(StatusCode::CONFLICT, crate::error::HAS_NAME))
        }
        crate::store::NameInsert::Taken => return Err(Fail::taken()),
    };
    if let Err(err) = publish::publish_stored(
        store,
        &svc.inner.cfg,
        &svc.inner.relay,
        svc.inner.r2.as_ref(),
        now,
    )
    .await
    {
        tracing::warn!(error = %err, "publish left the name in the outbox");
    }
    ok_name(svc, &stored)
}

fn ok_name(svc: &Service, row: &crate::store::NameRow) -> Result<Response, Fail> {
    let event: Value = serde_json::from_str(&row.event_json).map_err(|_| Fail::unavailable())?;
    let body = json!({
        "name": row.name,
        "nip05": format!("{}@groundinglab.io", row.name),
        "pubkey": row.pubkey,
        "record": event,
    });
    let _ = svc;
    Ok((StatusCode::OK, Json(body)).into_response())
}

/// `GET /name/available?name=`: free, taken, reserved or invalid. Advisory.
async fn get_available(
    State(svc): State<Service>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let ip = client_ip(&headers, &svc.inner.cfg.client_ip_header);
    let response = if !svc.inner.avail_limit.admit(&ip) {
        Fail::rate_limited().into_response()
    } else {
        let name = query
            .unwrap_or_default()
            .split('&')
            .find_map(|pair| pair.strip_prefix("name=").map(str::to_owned))
            .unwrap_or_default();
        let status = match check_name(&svc, &name, "(availability check)", &ip) {
            Err(fail) => fail.code,
            Ok(()) => match svc.store().and_then(|store| {
                store.name_by(crate::store::NameKey::Name, &name)
            }) {
                Ok(Some(_)) => "taken",
                Ok(None) => "free",
                Err(_) => "unavailable",
            },
        };
        (StatusCode::OK, Json(json!({ "name": name, "status": status }))).into_response()
    };
    with_cors(&svc, &headers, response)
}

fn ok_award(cfg: &Config, row: &BadgeRow) -> Result<Response, Fail> {
    let event: Value = serde_json::from_str(&row.event_json).map_err(|_| Fail::unavailable())?;
    let npub = nostr::npub_from_hex(&row.pubkey).map_err(|_| Fail::unavailable())?;
    let body = json!({
        "award": event,
        "admitted_unix_s": row.admitted_unix_s,
        "badge_url": format!("{}/badge/{npub}", cfg.public_site_origin),
    });
    Ok((StatusCode::OK, Json(body)).into_response())
}

fn client_ip(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_owned()
}

async fn preflight(State(svc): State<Service>, headers: HeaderMap) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response = with_cors(&svc, &headers, response);
    response
}

async fn unknown() -> Response {
    Fail::malformed().into_response()
}

fn with_cors(svc: &Service, headers: &HeaderMap, mut response: Response) -> Response {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return response;
    };
    if !svc
        .inner
        .cfg
        .allowed_origins
        .iter()
        .any(|allowed| allowed == origin)
    {
        return response;
    }
    if let Ok(value) = HeaderValue::from_str(origin) {
        response
            .headers_mut()
            .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("content-type"),
        );
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("origin"));
    }
    response
}
