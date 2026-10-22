//! Bari HTTP: `POST /waypoint`, `GET /path/{leaf}`, `GET /ops`, and `GET /health`.
//! `GET /index` and `GET /index/{height}` are not this process.
//!
//! `GET /ops` is the operator page. `BARI_OPS_FIXTURE=1` serves it from the
//! built-in fixture, with no Bitcoin node.
//! `GET /health` is the readiness probe. It is served with `/ops`.

use crate::clock::{BariClock, ClockError, PostAccept, TransitionReport};
use crate::hashcash::{self, HASHCASH_HEADER_HEADER, HASHCASH_NONCE_HEADER};
use crate::operator;
use crate::ops;
use crate::storage::StoredPortableIndex;
use crate::{decode_hash32, encode_bitcoin_id, encode_hash32, Hash32};
use axum::body::to_bytes;
use axum::extract::Request;
use axum::extract::{Path, State};
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

struct ServiceState {
    clock: BariClock,
    /// Leading zero bits `POST /waypoint` requires.
    /// [`BariService::new`] stores 21. Tests pass a lower count.
    hashcash_bits: u32,
    /// SPEC §13.1 serialization of Bari's public origin.
    origin: String,
    /// `BARI_OPS_FIXTURE=1`. The ops page is the fixture. The clock is unused.
    ops_fixture: bool,
    instance: String,
    /// Process start. The operator page prints uptime from this instant.
    started: std::time::SystemTime,
}

/// Commit stamped by `build.rs`. Not read from the environment at request time.
pub fn compiled_commit() -> &'static str {
    env!("BARI_COMMIT")
}

/// `GET /health` body. `tip` is the height Bari is serving.
pub fn health_json(tip: u32) -> String {
    serde_json::json!({
        "version": operator::VERSION,
        "commit": compiled_commit(),
        "tip": tip,
    })
    .to_string()
}

/// Producer stops on this sentence. The waypoint itself was rejected.
const PROOF_REJECTED_BODY: &str = "X-Bari-Hashcash-Nonce rejected";

#[derive(Clone)]
pub struct BariService {
    state: Arc<Mutex<ServiceState>>,
}

impl BariService {
    /// Production constructor. The Hashcash difficulty is 21 (SPEC P13.5).
    /// `origin` is Bari's public URL; it is stored as the §13.1 serialization.
    pub fn new(clock: BariClock, origin: &str) -> Result<Self, String> {
        let origin = hashcash::bari_origin(origin)?;
        Ok(Self::with_hashcash_bits(
            clock,
            hashcash::HASHCASH_BITS,
            origin,
        ))
    }

    /// In-process tests pass a difficulty in `1..=21` and an already
    /// serialized origin. The `bari` binary calls [`Self::new`].
    pub fn with_hashcash_bits(
        clock: BariClock,
        hashcash_bits: u32,
        origin: impl Into<String>,
    ) -> Self {
        assert!(
            (1..=hashcash::HASHCASH_BITS).contains(&hashcash_bits),
            "hashcash bits must be 1..=21"
        );
        let ops_fixture = std::env::var("BARI_OPS_FIXTURE")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let instance = std::env::var("BARI_INSTANCE").unwrap_or_else(|_| {
            if ops_fixture {
                "fixture".to_owned()
            } else {
                "local".to_owned()
            }
        });
        Self {
            state: Arc::new(Mutex::new(ServiceState {
                clock,
                hashcash_bits,
                origin: origin.into(),
                ops_fixture,
                instance,
                started: std::time::SystemTime::now(),
            })),
        }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/waypoint", post(submit_waypoint))
            .route("/path/{leaf}", get(fetch_path))
            .route("/ops", get(ops_page))
            .route("/health", get(health))
            .with_state(self.clone())
    }

    /// Live portable-index snapshot (same blob Bari still writes under `index/`).
    /// Public HTTP for this blob is not a route of Bari.
    pub fn index_snapshot(&self) -> crate::clock::IndexSnapshot {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clock
            .index_snapshot()
    }

    /// Retained portable-index blob for a tip height, if still on disk.
    pub fn index_at_height(
        &self,
        height: u32,
    ) -> Result<Option<StoredPortableIndex>, crate::storage::StorageError> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clock
            .index_at_height(height)
    }

    /// Advance from a tip the plane sequence already observed.
    ///
    /// Same hash is a no-op. Does not call `best_tip`.
    pub fn on_tip(
        &self,
        tip: crate::clock::ChainTip,
    ) -> Result<Option<TransitionReport>, ClockError> {
        let prepared = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.clock.current_tip().hash == tip.hash {
                return Ok(None);
            }
            state.clock.on_tip_prepare(tip)?
        };
        let (report, job) = prepared;
        if let Some(job) = job {
            let sealed = job.execute()?;
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.clock.install_sealed(sealed)?;
        }
        Ok(Some(report))
    }

    /// Tip poll: handoff under lock, seal outside accept lock, install pending.
    ///
    /// Live RPC wiring. The test plane uses [`Self::on_tip`] instead.
    pub fn poll_once(&self) -> Result<Option<TransitionReport>, ClockError> {
        let prepared = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.clock.poll_once_prepare()?
        };
        let Some((report, job)) = prepared else {
            return Ok(None);
        };
        if let Some(job) = job {
            let sealed = job.execute()?;
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.clock.install_sealed(sealed)?;
        }
        Ok(Some(report))
    }

    pub fn queue_tick(&self) -> Result<usize, ClockError> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clock
            .queue_tick()
    }

    pub fn queue_len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clock
            .queue_len()
    }

    pub fn collecting_len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clock
            .collecting_len()
    }

    pub fn pool_len(&self) -> usize {
        self.queue_len()
    }

    /// Verify Hashcash against the fee header and append under one lock.
    /// A tip poll cannot land between the proof check and the pool write.
    ///
    /// The header must be current, the previous tip, or (on the synthetic
    /// plane) one block after. A header outside that window is rejected.
    fn accept_paid(
        &self,
        waypoint: Hash32,
        nonce: u64,
        header: &[u8; 80],
    ) -> Result<PaidAccept, ClockError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tip = state.clock.current_tip();
        let network = state.clock.network().to_owned();
        let window = hashcash::fee_window(header, &tip.hash, tip.previous_hash.as_ref(), &network);
        if window == hashcash::FeeWindow::Outside {
            return Ok(PaidAccept::Rejected);
        }
        let bits = state.hashcash_bits;
        let origin = state.origin.clone();
        let hash = hashcash::header_hash(header);
        let challenge = hashcash::challenge(&origin, &hash);
        if !hashcash::verify(&challenge, &waypoint, nonce, bits) {
            return Ok(PaidAccept::Rejected);
        }
        match state.clock.post_waypoint(waypoint)? {
            PostAccept::Queued => Ok(PaidAccept::Appended),
            PostAccept::Duplicate => Ok(PaidAccept::AlreadyHeld),
            PostAccept::Full => Ok(PaidAccept::Full),
        }
    }
}

enum PaidAccept {
    Appended,
    AlreadyHeld,
    Full,
    Rejected,
}

fn log_waypoint(
    service: &BariService,
    accept: bool,
    status: StatusCode,
    reason: &str,
    waypoint: Option<&Hash32>,
    extra: Option<serde_json::Value>,
) {
    let state = service
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tip = state.clock.current_tip();
    let tip_height = tip.height;
    let tip_hash = encode_bitcoin_id(&tip.hash);
    drop(state);
    let mut fields = json!({
        "status": status.as_u16(),
        "reason": reason,
        "tip_height": tip_height,
        "tip_hash": tip_hash,
    });
    if let Some(waypoint) = waypoint {
        fields["waypoint"] = json!(encode_hash32(waypoint));
    }
    if let Some(serde_json::Value::Object(map)) = extra {
        if let Some(object) = fields.as_object_mut() {
            for (key, value) in map {
                object.insert(key, value);
            }
        }
    }
    if accept {
        ops::info(ops::WAYPOINT_ACCEPT, fields);
    } else {
        ops::warn(ops::WAYPOINT_REJECT, fields);
    }
}

async fn submit_waypoint(State(service): State<BariService>, request: Request) -> Response {
    if request.headers().get(header::CONTENT_TYPE)
        != Some(&HeaderValue::from_static("application/octet-stream"))
    {
        log_waypoint(
            &service,
            false,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be application/octet-stream",
            None,
            None,
        );
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be application/octet-stream",
        )
            .into_response();
    }
    let nonce_header = HeaderName::from_static(HASHCASH_NONCE_HEADER);
    let nonce = match request
        .headers()
        .get(&nonce_header)
        .and_then(|v| v.to_str().ok())
    {
        Some(raw) => match hashcash::parse_nonce(raw) {
            Some(n) => n,
            None => {
                log_waypoint(
                    &service,
                    false,
                    StatusCode::BAD_REQUEST,
                    "X-Bari-Hashcash-Nonce must be an unsigned decimal integer",
                    None,
                    None,
                );
                return (
                    StatusCode::BAD_REQUEST,
                    "X-Bari-Hashcash-Nonce must be an unsigned decimal integer",
                )
                    .into_response();
            }
        },
        None => {
            log_waypoint(
                &service,
                false,
                StatusCode::BAD_REQUEST,
                "X-Bari-Hashcash-Nonce header required",
                None,
                None,
            );
            return (
                StatusCode::BAD_REQUEST,
                "X-Bari-Hashcash-Nonce header required",
            )
                .into_response();
        }
    };
    let fee_header_name = HeaderName::from_static(HASHCASH_HEADER_HEADER);
    let fee_header = match request
        .headers()
        .get(&fee_header_name)
        .and_then(|value| value.to_str().ok())
        .and_then(hashcash::parse_fee_header)
    {
        Some(header) => header,
        None => {
            log_waypoint(
                &service,
                false,
                StatusCode::BAD_REQUEST,
                "X-Bari-Hashcash-Header must be 160 lowercase hex digits",
                None,
                None,
            );
            return (
                StatusCode::BAD_REQUEST,
                "X-Bari-Hashcash-Header must be 160 lowercase hex digits",
            )
                .into_response();
        }
    };
    let body = match to_bytes(request.into_body(), 33).await {
        Ok(body) => body,
        Err(_) => {
            log_waypoint(
                &service,
                false,
                StatusCode::BAD_REQUEST,
                "body must be exactly 32 raw bytes",
                None,
                None,
            );
            return (StatusCode::BAD_REQUEST, "body must be exactly 32 raw bytes").into_response();
        }
    };
    let Ok(waypoint) = <[u8; 32]>::try_from(body.as_ref()) else {
        log_waypoint(
            &service,
            false,
            StatusCode::BAD_REQUEST,
            "body must be exactly 32 raw bytes",
            None,
            Some(json!({ "body_len": body.len() })),
        );
        service
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clock
            .note_malformed();
        return (StatusCode::BAD_REQUEST, "body must be exactly 32 raw bytes").into_response();
    };
    match service.accept_paid(waypoint, nonce, &fee_header) {
        Ok(PaidAccept::Appended) => {
            log_waypoint(
                &service,
                true,
                StatusCode::NO_CONTENT,
                "appended",
                Some(&waypoint),
                None,
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(PaidAccept::AlreadyHeld) => {
            log_waypoint(
                &service,
                true,
                StatusCode::NO_CONTENT,
                "already held",
                Some(&waypoint),
                None,
            );
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(PaidAccept::Full) => {
            let soft_max = service
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clock
                .soft_max();
            log_waypoint(
                &service,
                false,
                StatusCode::SERVICE_UNAVAILABLE,
                "open pool reached BARI_POOL_SOFT_MAX",
                Some(&waypoint),
                Some(json!({ "soft_max": soft_max })),
            );
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Ok(PaidAccept::Rejected) => {
            log_waypoint(
                &service,
                false,
                StatusCode::BAD_REQUEST,
                PROOF_REJECTED_BODY,
                Some(&waypoint),
                None,
            );
            (StatusCode::BAD_REQUEST, PROOF_REJECTED_BODY).into_response()
        }
        Err(error) => {
            log_waypoint(
                &service,
                false,
                StatusCode::INTERNAL_SERVER_ERROR,
                &error.to_string(),
                Some(&waypoint),
                None,
            );
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn unix_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn node_reason<T>(
    result: Result<
        Result<Result<Option<T>, crate::clock::ChainError>, tokio::task::JoinError>,
        tokio::time::error::Elapsed,
    >,
) -> operator::NodeRead<T> {
    match result {
        Ok(Ok(Ok(Some(value)))) => operator::NodeRead::Ready(value),
        Ok(Ok(Ok(None))) => operator::NodeRead::Absent,
        Ok(Ok(Err(error))) => operator::NodeRead::Failed {
            reason: error.to_string(),
        },
        Ok(Err(error)) => operator::NodeRead::Failed {
            reason: error.to_string(),
        },
        Err(_) => operator::NodeRead::Failed {
            reason: format!(
                "node RPC timed out after {} s",
                crate::bitcoin_rpc::OPS_READ_TIMEOUT.as_secs()
            ),
        },
    }
}

async fn read_ops(backend: Arc<dyn crate::clock::ChainBackend>) -> operator::OpsReads {
    let fee_backend = Arc::clone(&backend);
    let wallet_backend = Arc::clone(&backend);
    let sync_backend = backend;
    let (fee, wallet, sync) = tokio::join!(
        tokio::time::timeout(
            crate::bitcoin_rpc::OPS_READ_TIMEOUT,
            tokio::task::spawn_blocking(move || fee_backend.fee_quote()),
        ),
        tokio::time::timeout(
            crate::bitcoin_rpc::OPS_READ_TIMEOUT,
            tokio::task::spawn_blocking(move || wallet_backend.chain_wallet()),
        ),
        tokio::time::timeout(
            crate::bitcoin_rpc::OPS_READ_TIMEOUT,
            tokio::task::spawn_blocking(move || sync_backend.chain_sync()),
        ),
    );
    let fee = match node_reason(fee) {
        operator::NodeRead::Ready(quote) => operator::NodeRead::Ready(operator::FeeReady {
            min_sat_vb: quote.min_sat_vb,
            floor_sat_vb: quote.floor_sat_vb,
            bid_sat_vb: quote.bid_sat_vb,
            source: quote.source,
        }),
        operator::NodeRead::Failed { reason } => operator::NodeRead::Failed { reason },
        operator::NodeRead::Absent => operator::NodeRead::Absent,
    };
    let wallet = match node_reason(wallet) {
        operator::NodeRead::Ready(wallet) => operator::NodeRead::Ready(operator::WalletReady {
            confirmed_sat: wallet.confirmed_sat,
            unconfirmed_sat: wallet.unconfirmed_sat,
            confirmed_coins: wallet.confirmed_coins,
            unconfirmed_coins: wallet.unconfirmed_coins,
            coins: wallet.coins,
            min_conf: wallet.min_conf,
            reserve_min: wallet.reserve_min,
            reserve_max: wallet.reserve_max,
            consolidate: wallet.consolidate,
        }),
        operator::NodeRead::Failed { reason } => operator::NodeRead::Failed { reason },
        operator::NodeRead::Absent => operator::NodeRead::Absent,
    };
    let sync = match node_reason(sync) {
        operator::NodeRead::Ready(sync) => operator::NodeRead::Ready(operator::SyncReady {
            blocks: sync.blocks,
            headers: sync.headers,
            initialblockdownload: sync.initialblockdownload,
        }),
        operator::NodeRead::Failed { reason } => operator::NodeRead::Failed { reason },
        operator::NodeRead::Absent => operator::NodeRead::Absent,
    };
    operator::OpsReads { fee, wallet, sync }
}

async fn render_ops(service: &BariService) -> String {
    let prepared = {
        let state = service
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.ops_fixture {
            let commit =
                std::env::var("BARI_COMMIT").unwrap_or_else(|_| compiled_commit().to_owned());
            return operator::render(&operator::fixture_view(&state.instance, &commit));
        }
        Some((
            state.clock.backend_handle(),
            state.instance.clone(),
            unix_ms(state.started),
            state.clock.missing_fee_txids(),
        ))
    };
    let Some((backend, instance, started_ms, missing)) = prepared else {
        return String::new();
    };
    if !missing.is_empty() {
        let lookup = std::sync::Arc::clone(&backend);
        let pending = missing;
        let found = tokio::time::timeout(
            crate::bitcoin_rpc::OPS_READ_TIMEOUT,
            tokio::task::spawn_blocking(move || {
                let mut fees = Vec::new();
                for (seq, txid) in pending {
                    if let Ok(Some(place)) = lookup.tx_place(&txid) {
                        if let Some(fee) = place.fee_sats {
                            fees.push((seq, fee));
                        }
                    }
                }
                fees
            }),
        )
        .await;
        if let Ok(Ok(fees)) = found {
            if !fees.is_empty() {
                let mut state = service
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.clock.apply_known_fees(&fees);
            }
        }
    }
    let memory = {
        let state = service
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.clock.ops_memory()
    };
    let reads = read_ops(backend).await;
    let now = unix_ms(SystemTime::now());
    let commit = compiled_commit().to_owned();
    let inputs = operator::inputs_from_memory(
        &memory,
        operator::ProcessFacts {
            version: crate::operator::VERSION.to_owned(),
            commit,
            instance,
            started_unix_ms: started_ms,
        },
    );
    let view = operator::build_ops_view(&inputs, &reads, now);
    let issues: Vec<(String, String)> = view
        .checks
        .iter()
        .filter(|check| !check.holds)
        .map(|check| (check.id.clone(), format!("{}{}", check.name, check.detail)))
        .collect();
    if !issues.is_empty() {
        let mut state = service
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.clock.note_page_checks(view.tip, &issues);
    }
    operator::render(&view)
}

async fn health(State(service): State<BariService>) -> Response {
    let tip = {
        let state = service
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.clock.current_tip().height
    };
    let mut response = (StatusCode::OK, health_json(tip)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

async fn ops_page(State(service): State<BariService>) -> Response {
    let html = render_ops(&service).await;
    let mut response = (StatusCode::OK, html).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

async fn fetch_path(
    State(service): State<BariService>,
    Path(encoded_leaf): Path<String>,
) -> Response {
    let leaf = match decode_hash32(&encoded_leaf) {
        Ok(leaf) => leaf,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    let (body, miss, serving_sequence, tip_height, tip_hash) = {
        let state = service
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tip = state.clock.current_tip();
        (
            state.clock.path_for(&leaf),
            state.clock.path_miss(&leaf),
            state.clock.serving_sequence_for(&leaf),
            tip.height,
            encode_bitcoin_id(&tip.hash),
        )
    };
    match body {
        Ok(Some(body)) => match body.encode() {
            Ok(bytes) => {
                ops::info(
                    ops::PATH_SERVE,
                    json!({
                        "status": 200,
                        "leaf": encode_hash32(&leaf),
                        "leaf_index": body.path.leaf_index,
                        "tree_size": body.path.tree_size,
                        "root": encode_hash32(&body.root),
                        "txid": encode_bitcoin_id(&body.txid),
                        "vout": body.vout,
                        "block_hash": encode_bitcoin_id(&body.block_hash),
                        "block_height": body.block_height,
                        "batch_sequence": serving_sequence,
                        "tip_height": tip_height,
                        "tip_hash": tip_hash,
                    }),
                );
                let mut response = (StatusCode::OK, bytes).into_response();
                response.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/octet-stream"),
                );
                response
            }
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Ok(None) => (StatusCode::NOT_FOUND, miss.as_str()).into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{
        ChainBackend, ChainError, ChainPostCheck, ChainTip, PlantError, PlantReceipt, SkipReason,
    };
    use crate::encode_hash32;
    use crate::hashcash;
    use crate::proof::CanonicalPathBody;
    use crate::storage::{FileStore, StoredInclusion};
    use std::sync::Mutex;
    use tower::ServiceExt;

    /// Low difficulty for in-process posts. Production `BariService::new` stays at 21.
    const TEST_BITS: u32 = 8;
    const TEST_ORIGIN: &str = "https://bari.test";

    fn stamp_header() -> [u8; 80] {
        hashcash::assemble_header(&[0x11; 32], 1_700_000_100, hashcash::SYNTH_NBITS, 1)
    }

    fn stamp_hash() -> Hash32 {
        hashcash::header_hash(&stamp_header())
    }

    struct ControllableState {
        tip: ChainTip,
        included: bool,
        plant_failure: Option<SkipReason>,
        plants: usize,
        best_tip_failures: usize,
        inclusion_lookup_failures: usize,
    }

    struct ControllableChain {
        state: Mutex<ControllableState>,
    }

    impl ControllableChain {
        fn new(tip: ChainTip) -> Self {
            Self {
                state: Mutex::new(ControllableState {
                    tip,
                    included: false,
                    plant_failure: None,
                    plants: 0,
                    best_tip_failures: 0,
                    inclusion_lookup_failures: 0,
                }),
            }
        }

        fn set_tip(&self, tip: ChainTip) {
            self.state.lock().unwrap().tip = tip;
        }

        fn set_included(&self, included: bool) {
            self.state.lock().unwrap().included = included;
        }
    }

    impl ChainBackend for ControllableChain {
        fn network(&self) -> Result<String, ChainError> {
            Ok("signet".to_owned())
        }

        fn initial_block_download(&self) -> Result<bool, ChainError> {
            Ok(false)
        }

        fn best_tip(&self) -> Result<ChainTip, ChainError> {
            let mut state = self.state.lock().unwrap();
            if state.best_tip_failures > 0 {
                state.best_tip_failures -= 1;
                return Err(ChainError("fail".into()));
            }
            Ok(state.tip)
        }

        fn plant(&self, _root: &Hash32) -> Result<PlantReceipt, PlantError> {
            let mut state = self.state.lock().unwrap();
            if let Some(reason) = state.plant_failure {
                return Err(PlantError::new(reason, "fail"));
            }
            state.plants += 1;
            let n = state.plants as u8;
            Ok(PlantReceipt {
                txid: [0xA0 + n; 32],
                vout: state.plants as u32,
                fee_rate_sat_vb: 1.0,
                fee_source: "test".into(),
                fee_cap_sat_vb: None,
                fee_sat: None,
                vsize: None,
            })
        }

        fn included_in_tip(
            &self,
            _txid: &Hash32,
            tip: &ChainTip,
        ) -> Result<Option<StoredInclusion>, ChainError> {
            let mut state = self.state.lock().unwrap();
            if state.inclusion_lookup_failures > 0 {
                state.inclusion_lookup_failures -= 1;
                return Err(ChainError("incl".into()));
            }
            Ok(state.included.then_some(StoredInclusion {
                block_hash: tip.hash,
                block_height: tip.height,
            }))
        }

        fn block_is_selected(&self, _block_hash: &Hash32) -> Result<bool, ChainError> {
            Ok(true)
        }

        fn post_check(&self, _body: &CanonicalPathBody) -> Result<ChainPostCheck, ChainError> {
            unreachable!("not used")
        }
    }

    fn app_at(
        tip: ChainTip,
    ) -> (
        BariService,
        Router,
        Arc<ControllableChain>,
        tempfile::TempDir,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(ControllableChain::new(tip));
        let clock = BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
        let service = BariService::with_hashcash_bits(clock, TEST_BITS, TEST_ORIGIN);
        let router = service.router();
        (service, router, chain, temp)
    }

    fn app() -> (BariService, Router) {
        let (service, router, _, temp) = app_at(ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 1,
            time: None,
        });
        std::mem::forget(temp);
        (service, router)
    }

    fn paid_post(bytes: Vec<u8>) -> axum::http::Request<axum::body::Body> {
        post_header(&stamp_header(), bytes, TEST_ORIGIN)
    }

    fn post_header(
        header: &[u8; 80],
        bytes: Vec<u8>,
        origin: &str,
    ) -> axum::http::Request<axum::body::Body> {
        let waypoint: Hash32 = bytes.as_slice().try_into().unwrap_or([0u8; 32]);
        let hash = hashcash::header_hash(header);
        let challenge = hashcash::challenge(origin, &hash);
        let nonce = if bytes.len() == 32 {
            hashcash::solve(&challenge, &waypoint, TEST_BITS)
        } else {
            0
        };
        axum::http::Request::post("/waypoint")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("X-Bari-Hashcash-Nonce", nonce.to_string())
            .header("X-Bari-Hashcash-Header", hex::encode(header))
            .body(axum::body::Body::from(bytes))
            .unwrap()
    }

    fn post_wp(bytes: Vec<u8>) -> axum::http::Request<axum::body::Body> {
        paid_post(bytes)
    }

    #[tokio::test]
    async fn s06_s07_s08_post_queue_and_duplicate_and_bad_size() {
        let (service, app) = app();
        assert_eq!(
            app.clone()
                .oneshot(post_wp(vec![0x42; 32]))
                .await
                .unwrap()
                .status(),
            204
        );
        assert_eq!(service.pool_len(), 1);
        assert_eq!(
            app.clone()
                .oneshot(post_wp(vec![0x42; 32]))
                .await
                .unwrap()
                .status(),
            204
        );
        assert_eq!(service.pool_len(), 1);
        assert_eq!(
            app.clone()
                .oneshot(post_wp(vec![0; 31]))
                .await
                .unwrap()
                .status(),
            400
        );
        assert_eq!(service.pool_len(), 1);
        assert_eq!(
            app.oneshot(post_wp(vec![0; 33])).await.unwrap().status(),
            400
        );
        assert_eq!(service.pool_len(), 1);
    }

    #[tokio::test]
    async fn s09_bad_content_type() {
        let (_, app) = app();
        let wrong = axum::http::Request::post("/waypoint")
            .header(header::CONTENT_TYPE, "text/plain")
            .body(axum::body::Body::from(vec![0; 32]))
            .unwrap();
        assert_eq!(app.oneshot(wrong).await.unwrap().status(), 415);
    }

    #[tokio::test]
    async fn soft_max_maps_to_503_when_enabled() {
        let temp = tempfile::tempdir().unwrap();
        let tip = ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 1,
            time: None,
        };
        let chain = Arc::new(ControllableChain::new(tip));
        let mut filled = BariClock::open_at_tip_with_soft_max(
            chain,
            FileStore::open(temp.path()).unwrap(),
            tip,
            Some(2),
        )
        .unwrap();
        // Fill soft max via clock (bypass hashcash); HTTP maps Full → 503.
        assert_eq!(
            filled.post_waypoint([0x01; 32]).unwrap(),
            PostAccept::Queued
        );
        assert_eq!(
            filled.post_waypoint([0x02; 32]).unwrap(),
            PostAccept::Queued
        );
        let service = BariService::with_hashcash_bits(filled, TEST_BITS, TEST_ORIGIN);
        let app = service.router();
        let mut extra = [0xff; 32];
        extra[0] = 7;
        assert_eq!(
            app.oneshot(paid_post(extra.to_vec()))
                .await
                .unwrap()
                .status(),
            503
        );
        let extra_hex = encode_hash32(&extra);
        let rejected = crate::ops::lines_containing(crate::ops::WAYPOINT_REJECT);
        let full = rejected.iter().any(|line| {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                return false;
            };
            value["status"] == 503
                && value["reason"] == "open pool reached BARI_POOL_SOFT_MAX"
                && value["waypoint"] == extra_hex
                && value["soft_max"] == 2
                && value["level"] == "warn"
        });
        assert!(full, "{rejected:?}");
    }

    #[tokio::test]
    async fn s04_s05_post_and_queue_tick_do_not_publish() {
        let (service, app, _chain, _temp) = app_at(ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 10,
            time: None,
        });
        let before = service.index_snapshot();
        assert_eq!(before.header.height, 10);
        app.clone()
            .oneshot(paid_post(vec![0x11; 32]))
            .await
            .unwrap();
        assert_eq!(service.index_snapshot(), before);
        assert_eq!(service.queue_tick().unwrap(), 0);
        assert_eq!(service.index_snapshot(), before);
        // Open pool holds the accept; QueueTick is a no-op.
        assert_eq!(service.pool_len(), 1);
        assert_eq!(service.queue_len(), 1);
        assert_eq!(service.collecting_len(), 1);
    }

    #[tokio::test]
    async fn s19_index_keys_version_2_no_retired_keys() {
        let (service, _app) = app();
        let stored = service.index_snapshot().to_stored();
        let value = serde_json::to_value(&stored).unwrap();
        assert_eq!(value["version"], 2);
        assert!(value.get("network").is_some());
        assert!(value["header"].get("hash").is_some());
        assert!(value["header"].get("height").is_some());
        assert!(value.get("plant").is_some());
        assert!(value["plant"].is_null());
        assert!(value.get("processed_tip").is_none());
        assert!(value.get("plants").is_none());
        assert!(value.get("queryable_batch_sequence").is_none());
    }

    #[tokio::test]
    async fn s21_s22_path_miss_and_malformed_and_immediate() {
        let (_, app) = app();
        let absent = axum::http::Request::get(format!("/path/{}", "00".repeat(32)))
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(app.clone().oneshot(absent).await.unwrap().status(), 404);
        let malformed = axum::http::Request::get(format!("/path/{}", "AA".repeat(32)))
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(app.oneshot(malformed).await.unwrap().status(), 400);
    }

    #[tokio::test]
    async fn s14_s16_http_serving_window() {
        let (service, app, chain, _temp) = app_at(ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 100,
            time: None,
        });
        let leaf = [0x71; 32];
        assert_eq!(
            app.clone()
                .oneshot(paid_post(leaf.to_vec()))
                .await
                .unwrap()
                .status(),
            204
        );
        // T6: immediate path miss
        let fetch = |app: Router, leaf: Hash32| {
            let app = app.clone();
            async move {
                app.oneshot(
                    axum::http::Request::get(format!("/path/{}", encode_hash32(&leaf)))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status()
            }
        };
        assert_eq!(fetch(app.clone(), leaf).await, 404);

        chain.set_tip(ChainTip {
            hash: [2; 32],
            previous_hash: Some([1; 32]),
            height: 101,
            time: None,
        });
        service.poll_once().unwrap();
        // pending, not serving
        assert_eq!(fetch(app.clone(), leaf).await, 404);

        chain.set_included(true);
        chain.set_tip(ChainTip {
            hash: [3; 32],
            previous_hash: Some([2; 32]),
            height: 102,
            time: None,
        });
        service.poll_once().unwrap();
        let snap = service.index_snapshot();
        assert!(snap.plant.is_some());
        let stored = snap.to_stored();
        assert_eq!(stored.version, 2);
        assert!(stored.plant.is_some());
        assert_eq!(fetch(app.clone(), leaf).await, 200);

        // Planted at 101. The confirmation window is fixed at 3 tips, through height 104.
        chain.set_tip(ChainTip {
            hash: [4; 32],
            previous_hash: Some([3; 32]),
            height: 103,
            time: None,
        });
        service.poll_once().unwrap();
        assert_eq!(fetch(app.clone(), leaf).await, 200);
        chain.set_tip(ChainTip {
            hash: [5; 32],
            previous_hash: Some([4; 32]),
            height: 104,
            time: None,
        });
        service.poll_once().unwrap();
        assert_eq!(fetch(app.clone(), leaf).await, 200);
        chain.set_tip(ChainTip {
            hash: [6; 32],
            previous_hash: Some([5; 32]),
            height: 105,
            time: None,
        });
        service.poll_once().unwrap();
        assert_eq!(fetch(app.clone(), leaf).await, 200);
        assert_eq!(fetch(app.clone(), leaf).await, 200);
    }

    #[tokio::test]
    async fn s24_router_exposes_waypoint_and_path_only_no_public_index() {
        let (_, app) = app();
        let health = axum::http::Request::get("/health")
            .body(axum::body::Body::empty())
            .unwrap();
        let health = app.clone().oneshot(health).await.unwrap();
        assert_eq!(health.status(), 200);
        let health_body: serde_json::Value =
            serde_json::from_str(&response_text(health).await).unwrap();
        assert_eq!(health_body["version"], operator::VERSION);
        assert_eq!(health_body["commit"], compiled_commit());
        assert_ne!(health_body["commit"], "–");
        assert_eq!(health_body["tip"], 1);
        // This router does not serve the tip or a height fetch (404).
        let tip = axum::http::Request::get("/index")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(app.clone().oneshot(tip).await.unwrap().status(), 404);
        let by_height = axum::http::Request::get("/index/1")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(app.clone().oneshot(by_height).await.unwrap().status(), 404);
        let junk = axum::http::Request::get("/not-a-surface")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(app.oneshot(junk).await.unwrap().status(), 404);
    }

    #[tokio::test]
    async fn health_and_system_tile_use_the_compiled_commit_and_process_uptime() {
        let (_service, app) = app();
        let health = app
            .clone()
            .oneshot(
                axum::http::Request::get("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);
        assert_eq!(
            health
                .headers()
                .get(header::CONTENT_TYPE)
                .map(|v| v.as_bytes()),
            Some(b"application/json".as_slice())
        );
        let body: serde_json::Value = serde_json::from_str(&response_text(health).await).unwrap();
        assert_eq!(body["version"], crate::operator::VERSION);
        assert_eq!(body["commit"], compiled_commit());
        assert!(!compiled_commit().is_empty());
        assert_ne!(body["commit"].as_str().unwrap(), "–");
        assert_eq!(body["tip"], 1);

        let ops = app
            .oneshot(
                axum::http::Request::get("/ops")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ops.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(ops.into_body(), 2 * 1024 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(html.contains(&format!("<code>{}</code>", compiled_commit())));
        assert!(html.contains("data-view=\"system\""));
        assert!(!html.contains("since –"));
        assert!(
            html.contains(" min") || html.contains("h ") || html.contains("d "),
            "{html}"
        );
    }

    #[tokio::test]
    async fn bari_still_retains_index_blobs_on_disk_without_serving_them() {
        let tip0 = ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 100,
            time: None,
        };
        let (service, app, chain, _temp) = app_at(tip0);
        chain.set_tip(ChainTip {
            hash: [2; 32],
            previous_hash: Some([1; 32]),
            height: 101,
            time: None,
        });
        service.poll_once().unwrap();

        let live = service.index_snapshot().to_stored();
        assert_eq!(live.version, 2);
        assert_eq!(live.header.height, 101);

        let retained = service.index_at_height(101).unwrap();
        assert_eq!(retained.as_ref().map(|t| t.header.height), Some(101));
        assert_eq!(retained.as_ref().map(|t| t.version), Some(2));
        assert!(service.index_at_height(999999).unwrap().is_none());

        // Router must not expose public index HTTP anymore.
        let tip = app
            .clone()
            .oneshot(
                axum::http::Request::get("/index")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(tip.status(), 404);
        let by_height = app
            .oneshot(
                axum::http::Request::get("/index/101")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(by_height.status(), 404);
    }

    #[tokio::test]
    async fn hashcash_required_and_wrong_proof_rejected() {
        let (_, router) = app();
        let bare = axum::http::Request::post("/waypoint")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(axum::body::Body::from(vec![0x42; 32]))
            .unwrap();
        assert_eq!(router.clone().oneshot(bare).await.unwrap().status(), 400);
        let header = stamp_header();
        let wp = [0x44u8; 32];
        let challenge = hashcash::challenge(TEST_ORIGIN, &hashcash::header_hash(&header));
        let mut bad_nonce = 0u64;
        while hashcash::verify(&challenge, &wp, bad_nonce, TEST_BITS) {
            bad_nonce += 1;
        }
        let req = axum::http::Request::post("/waypoint")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("X-Bari-Hashcash-Nonce", bad_nonce.to_string())
            .header("X-Bari-Hashcash-Header", hex::encode(header))
            .body(axum::body::Body::from(wp.to_vec()))
            .unwrap();
        assert_eq!(router.oneshot(req).await.unwrap().status(), 400);
    }

    #[tokio::test]
    async fn hashcash_bound_to_a_different_header_is_rejected() {
        let (service, router) = app();
        let live_header = stamp_header();
        let stale_header = hashcash::assemble_header(&[0x09; 32], 1, hashcash::SYNTH_NBITS, 9);
        let waypoint = [0x44u8; 32];
        let response = router
            .clone()
            .oneshot(post_header(&stale_header, waypoint.to_vec(), TEST_ORIGIN))
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        let text = response_text(response).await;
        assert_eq!(text, PROOF_REJECTED_BODY);
        assert_eq!(service.pool_len(), 0);
        assert_eq!(
            router
                .oneshot(post_header(&live_header, waypoint.to_vec(), TEST_ORIGIN))
                .await
                .unwrap()
                .status(),
            204
        );
        assert_eq!(service.pool_len(), 1);
    }

    fn ops_for(event: &str, needle: &str) -> Vec<serde_json::Value> {
        crate::ops::lines_containing(event)
            .iter()
            .filter(|line| needle.is_empty() || line.contains(needle))
            .map(|line| serde_json::from_str(line).expect("ops line is json"))
            .collect()
    }

    #[tokio::test]
    async fn ops_logs_waypoint_accept_reject_and_path_serve() {
        let (service, app, chain, _temp) = app_at(ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 80,
            time: None,
        });
        let wrong = axum::http::Request::post("/waypoint")
            .header(header::CONTENT_TYPE, "text/plain")
            .body(axum::body::Body::from(vec![0x61; 32]))
            .unwrap();
        assert_eq!(app.clone().oneshot(wrong).await.unwrap().status(), 415);
        let rejected_type = ops_for(crate::ops::WAYPOINT_REJECT, "application/octet-stream");
        assert!(
            rejected_type.iter().any(|line| {
                line["level"] == "warn"
                    && line["status"] == 415
                    && line["reason"] == "Content-Type must be application/octet-stream"
                    && line.get("waypoint").is_none()
            }),
            "{rejected_type:?}"
        );

        let leaf = [0x62; 32];
        let leaf_hex = encode_hash32(&leaf);
        let header = stamp_header();
        let challenge = hashcash::challenge(TEST_ORIGIN, &hashcash::header_hash(&header));
        let mut bad_nonce = 0u64;
        while hashcash::verify(&challenge, &leaf, bad_nonce, TEST_BITS) {
            bad_nonce += 1;
        }
        let bad = axum::http::Request::post("/waypoint")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("X-Bari-Hashcash-Nonce", bad_nonce.to_string())
            .header("X-Bari-Hashcash-Header", hex::encode(header))
            .body(axum::body::Body::from(leaf.to_vec()))
            .unwrap();
        assert_eq!(app.clone().oneshot(bad).await.unwrap().status(), 400);
        let rejected_proof = ops_for(crate::ops::WAYPOINT_REJECT, &leaf_hex);
        assert!(
            rejected_proof.iter().any(|line| {
                line["status"] == 400
                    && line["reason"] == PROOF_REJECTED_BODY
                    && line["waypoint"] == leaf_hex
            }),
            "{rejected_proof:?}"
        );
        assert!(ops_for(crate::ops::OPEN_POOL_ADMIT, &leaf_hex).is_empty());

        assert_eq!(
            app.clone()
                .oneshot(post_wp(vec![0; 31]))
                .await
                .unwrap()
                .status(),
            400
        );
        let short = ops_for(
            crate::ops::WAYPOINT_REJECT,
            "body must be exactly 32 raw bytes",
        );
        assert!(
            short
                .iter()
                .any(|line| line["body_len"] == 31 && line["status"] == 400),
            "{short:?}"
        );

        assert_eq!(
            app.clone()
                .oneshot(paid_post(leaf.to_vec()))
                .await
                .unwrap()
                .status(),
            204
        );
        let accepted = ops_for(crate::ops::WAYPOINT_ACCEPT, &leaf_hex);
        assert!(
            accepted.iter().any(|line| {
                line["level"] == "info"
                    && line["status"] == 204
                    && line["reason"] == "appended"
                    && line["waypoint"] == leaf_hex
                    && line["tip_height"] == 80
            }),
            "{accepted:?}"
        );
        let pooled = ops_for(crate::ops::OPEN_POOL_ADMIT, &leaf_hex);
        assert_eq!(pooled.len(), 1, "{pooled:?}");
        assert_eq!(pooled[0]["leaf_count"], 1);
        assert_eq!(pooled[0]["leaf_ids"][0], leaf_hex);

        assert_eq!(
            app.clone()
                .oneshot(paid_post(leaf.to_vec()))
                .await
                .unwrap()
                .status(),
            204
        );
        let held = ops_for(crate::ops::WAYPOINT_ACCEPT, &leaf_hex);
        assert!(
            held.iter().any(|line| line["reason"] == "already held"),
            "{held:?}"
        );
        assert_eq!(ops_for(crate::ops::OPEN_POOL_ADMIT, &leaf_hex).len(), 1);
        assert_eq!(service.pool_len(), 1);

        chain.set_tip(ChainTip {
            hash: [2; 32],
            previous_hash: Some([1; 32]),
            height: 81,
            time: None,
        });
        service.poll_once().unwrap();
        chain.set_included(true);
        chain.set_tip(ChainTip {
            hash: [3; 32],
            previous_hash: Some([2; 32]),
            height: 82,
            time: None,
        });
        service.poll_once().unwrap();
        let fetch = app
            .clone()
            .oneshot(
                axum::http::Request::get(format!("/path/{leaf_hex}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(fetch.status(), 200);
        let served = ops_for(crate::ops::PATH_SERVE, &leaf_hex);
        assert_eq!(served.len(), 1, "{served:?}");
        assert_eq!(served[0]["level"], "info");
        assert_eq!(served[0]["status"], 200);
        assert_eq!(served[0]["leaf"], leaf_hex);
        assert_eq!(served[0]["tree_size"], 1);
        assert!(served[0]["txid"].as_str().is_some());
        assert_eq!(served[0]["batch_sequence"], 0);

        chain.set_tip(ChainTip {
            hash: [4; 32],
            previous_hash: Some([3; 32]),
            height: 83,
            time: None,
        });
        service.poll_once().unwrap();
        assert_eq!(
            app.clone()
                .oneshot(
                    axum::http::Request::get(format!("/path/{leaf_hex}"))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status(),
            200
        );
        chain.set_tip(ChainTip {
            hash: [5; 32],
            previous_hash: Some([4; 32]),
            height: 84,
            time: None,
        });
        service.poll_once().unwrap();
        chain.set_tip(ChainTip {
            hash: [6; 32],
            previous_hash: Some([5; 32]),
            height: 85,
            time: None,
        });
        service.poll_once().unwrap();
        let archived = ops_for(crate::ops::PATH_ARCHIVE, &leaf_hex);
        assert!(archived.is_empty(), "{archived:?}");
        let after = app
            .clone()
            .oneshot(
                axum::http::Request::get(format!("/path/{leaf_hex}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(after.status(), 200);
        let again = app
            .oneshot(
                axum::http::Request::get(format!("/path/{leaf_hex}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(again.status(), 200);
        let first_bytes = to_bytes(after.into_body(), 4096).await.unwrap();
        let second_bytes = to_bytes(again.into_body(), 4096).await.unwrap();
        assert_eq!(first_bytes, second_bytes);
        let body = CanonicalPathBody::decode(&first_bytes).unwrap();
        assert_eq!(body.block_height, 82);
    }

    async fn response_text(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn missing_path_says_which_case_and_a_served_path_validates() {
        let (service, app, chain, _temp) = app_at(ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 100,
            time: None,
        });
        let leaf = [0x81; 32];
        let unknown = [0x82; 32];

        async fn miss(app: Router, leaf: Hash32) -> (StatusCode, String) {
            let response = app
                .oneshot(
                    axum::http::Request::get(format!("/path/{}", encode_hash32(&leaf)))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            (status, response_text(response).await)
        }

        let (status, text) = miss(app.clone(), unknown).await;
        assert_eq!(status, 404);
        assert_eq!(text, "unknown");

        assert_eq!(
            app.clone()
                .oneshot(paid_post(leaf.to_vec()))
                .await
                .unwrap()
                .status(),
            204
        );
        let (status, text) = miss(app.clone(), leaf).await;
        assert_eq!(status, 404);
        assert_eq!(text, "still in the pool");

        chain.set_tip(ChainTip {
            hash: [2; 32],
            previous_hash: Some([1; 32]),
            height: 101,
            time: None,
        });
        service.poll_once().unwrap();
        let (status, text) = miss(app.clone(), leaf).await;
        assert_eq!(status, 404);
        assert_eq!(text, "in mempool, not in a block yet");

        chain.set_included(false);
        chain.set_tip(ChainTip {
            hash: [3; 32],
            previous_hash: Some([2; 32]),
            height: 102,
            time: None,
        });
        service.poll_once().unwrap();
        let (status, text) = miss(app.clone(), leaf).await;
        assert_eq!(
            (status, text.as_str()),
            (StatusCode::NOT_FOUND, "in mempool, not in a block yet")
        );

        chain.set_tip(ChainTip {
            hash: [4; 32],
            previous_hash: Some([3; 32]),
            height: 103,
            time: None,
        });
        service.poll_once().unwrap();
        chain.set_tip(ChainTip {
            hash: [5; 32],
            previous_hash: Some([4; 32]),
            height: 104,
            time: None,
        });
        service.poll_once().unwrap();
        let (status, text) = miss(app.clone(), leaf).await;
        assert_eq!(
            (status, text.as_str()),
            (StatusCode::NOT_FOUND, "in mempool, not in a block yet")
        );
        chain.set_included(true);
        chain.set_tip(ChainTip {
            hash: [6; 32],
            previous_hash: Some([5; 32]),
            height: 105,
            time: None,
        });
        service.poll_once().unwrap();
        let response = app
            .oneshot(
                axum::http::Request::get(format!("/path/{}", encode_hash32(&leaf)))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let body = CanonicalPathBody::decode(&bytes).unwrap();
        body.validate_for(&leaf).unwrap();
        assert_eq!(body.block_height, 105);
        assert_eq!(chain.state.lock().unwrap().plants, 1);
    }

    #[tokio::test]
    async fn served_path_is_200_bytes_for_that_waypoint() {
        let (service, app, chain, _temp) = app_at(ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 50,
            time: None,
        });
        let leaf = [0x83; 32];
        assert_eq!(
            app.clone()
                .oneshot(paid_post(leaf.to_vec()))
                .await
                .unwrap()
                .status(),
            204
        );
        chain.set_tip(ChainTip {
            hash: [2; 32],
            previous_hash: Some([1; 32]),
            height: 51,
            time: None,
        });
        service.poll_once().unwrap();
        chain.set_included(true);
        chain.set_tip(ChainTip {
            hash: [3; 32],
            previous_hash: Some([2; 32]),
            height: 52,
            time: None,
        });
        service.poll_once().unwrap();
        let response = app
            .oneshot(
                axum::http::Request::get(format!("/path/{}", encode_hash32(&leaf)))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let body = CanonicalPathBody::decode(&bytes).unwrap();
        body.validate_for(&leaf).unwrap();
        assert_eq!(body.block_height, 52);
    }

    /// The path body names the block that holds the tree, plus the txid and Merkle path.
    #[tokio::test]
    async fn get_path_returns_inclusion_height_txid_and_merkle_path() {
        let (service, app, chain, _temp) = app_at(ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 50,
            time: None,
        });
        let leaf = [0x91; 32];
        assert_eq!(
            app.clone()
                .oneshot(paid_post(leaf.to_vec()))
                .await
                .unwrap()
                .status(),
            204
        );
        chain.set_tip(ChainTip {
            hash: [2; 32],
            previous_hash: Some([1; 32]),
            height: 51,
            time: None,
        });
        service.poll_once().unwrap();
        chain.set_included(true);
        let inclusion = ChainTip {
            hash: [3; 32],
            previous_hash: Some([2; 32]),
            height: 52,
            time: None,
        };
        chain.set_tip(inclusion);
        service.poll_once().unwrap();
        let response = app
            .oneshot(
                axum::http::Request::get(format!("/path/{}", encode_hash32(&leaf)))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let body = CanonicalPathBody::decode(&bytes).unwrap();
        body.validate_for(&leaf).unwrap();
        body.path.verify(&body.root).unwrap();
        assert_eq!(body.path.leaf, leaf);
        assert_eq!(body.txid, [0xA1; 32]);
        assert_eq!(body.vout, 1);
        assert_eq!(body.block_hash, inclusion.hash);
        // Inclusion block, not the post height (50) and not the seal tip (51).
        assert_eq!(body.block_height, inclusion.height);
        assert_eq!(body.block_height, 52);
    }

    /// SPEC §13.6: a header whose hash is the previous tip is accepted.
    /// A header outside current/before/after is the plain proof rejection.
    #[tokio::test]
    async fn before_header_is_accepted_and_an_outside_header_is_rejected() {
        let before =
            hashcash::assemble_header(&[0x21; 32], 1_700_000_000, hashcash::SYNTH_NBITS, 3);
        let before_hash = hashcash::header_hash(&before);
        let (service, app, chain, _temp) = app_at(ChainTip {
            hash: stamp_hash(),
            previous_hash: Some(before_hash),
            height: 20,
            time: None,
        });
        let leaf = [0x84; 32];
        let short = app.clone().oneshot(post_wp(vec![0x11; 31])).await.unwrap();
        assert_eq!(short.status(), 400);
        assert_eq!(
            response_text(short).await,
            "body must be exactly 32 raw bytes"
        );

        let outside = hashcash::assemble_header(&[0x99; 32], 1, hashcash::SYNTH_NBITS, 9);
        let response = app
            .clone()
            .oneshot(post_header(&outside, leaf.to_vec(), TEST_ORIGIN))
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(response_text(response).await, PROOF_REJECTED_BODY);
        assert_eq!(service.pool_len(), 0);
        assert_eq!(chain.state.lock().unwrap().plants, 0);

        let live = hashcash::challenge(TEST_ORIGIN, &stamp_hash());
        let weak = hashcash::solve(&live, &leaf, 1);
        if !hashcash::verify(&live, &leaf, weak, TEST_BITS) {
            let weak_post = axum::http::Request::post("/waypoint")
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .header("X-Bari-Hashcash-Nonce", weak.to_string())
                .header("X-Bari-Hashcash-Header", hex::encode(stamp_header()))
                .body(axum::body::Body::from(leaf.to_vec()))
                .unwrap();
            let response = app.clone().oneshot(weak_post).await.unwrap();
            assert_eq!(response.status(), 400);
            assert_eq!(response_text(response).await, PROOF_REJECTED_BODY);
        }
        assert_eq!(service.pool_len(), 0);

        assert_eq!(
            app.oneshot(post_header(&before, leaf.to_vec(), TEST_ORIGIN))
                .await
                .unwrap()
                .status(),
            204
        );
        assert_eq!(service.pool_len(), 1);
    }

    #[tokio::test]
    async fn production_constructor_rejects_an_8_bit_stamp() {
        let temp = tempfile::tempdir().unwrap();
        let tip = ChainTip {
            hash: stamp_hash(),
            previous_hash: None,
            height: 1,
            time: None,
        };
        let chain = Arc::new(ControllableChain::new(tip));
        let clock = BariClock::open(chain, FileStore::open(temp.path()).unwrap()).unwrap();
        let service = BariService::new(clock, TEST_ORIGIN).unwrap();
        let app = service.router();
        let waypoint = [0x42; 32];
        let header = stamp_header();
        let challenge = hashcash::challenge(TEST_ORIGIN, &hashcash::header_hash(&header));
        let nonce = hashcash::solve(&challenge, &waypoint, 8);
        assert!(hashcash::verify(&challenge, &waypoint, nonce, 8));
        assert!(!hashcash::verify(
            &challenge,
            &waypoint,
            nonce,
            hashcash::HASHCASH_BITS
        ));
        let response = app
            .oneshot(
                axum::http::Request::post("/waypoint")
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header("X-Bari-Hashcash-Nonce", nonce.to_string())
                    .header("X-Bari-Hashcash-Header", hex::encode(header))
                    .body(axum::body::Body::from(waypoint.to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(response_text(response).await, PROOF_REJECTED_BODY);
        assert_eq!(service.pool_len(), 0);
    }

    /// Candidate fee, solved at 21 bits, is accepted when the tip hash is that
    /// header and the origin is the candidate origin. Another origin is refused.
    #[tokio::test]
    async fn candidate_fee_is_accepted_and_another_origin_is_refused() {
        let fields = candidate_fields();
        let origin = fields["hashcash.bari_origin"].clone();
        let header = hashcash::parse_fee_header(&fields["hashcash.fee_header"]).unwrap();
        let hash = hashcash::header_hash(&header);
        let waypoint = hex::decode(&fields["hashcash.waypoint"]).unwrap();
        let nonce = fields["hashcash.nonce"].clone();
        let temp = tempfile::tempdir().unwrap();
        let tip = ChainTip {
            hash,
            previous_hash: None,
            height: 1,
            time: None,
        };
        let chain = Arc::new(ControllableChain::new(tip));
        let clock = BariClock::open(chain, FileStore::open(temp.path()).unwrap()).unwrap();
        let service = BariService::new(clock, &origin).unwrap();
        let app = service.router();
        let wrong = BariService::with_hashcash_bits(
            {
                let temp = tempfile::tempdir().unwrap();
                let chain = Arc::new(ControllableChain::new(tip));
                let clock = BariClock::open(chain, FileStore::open(temp.path()).unwrap()).unwrap();
                std::mem::forget(temp);
                clock
            },
            hashcash::HASHCASH_BITS,
            "https://other.example",
        );
        let refused = wrong
            .router()
            .oneshot(candidate_post(&header, &nonce, &waypoint))
            .await
            .unwrap();
        assert_eq!(refused.status(), 400);
        assert_eq!(response_text(refused).await, PROOF_REJECTED_BODY);
        let accepted = app
            .oneshot(candidate_post(&header, &nonce, &waypoint))
            .await
            .unwrap();
        assert_eq!(accepted.status(), 204);
        assert_eq!(service.pool_len(), 1);
    }

    fn candidate_post(
        header: &[u8; 80],
        nonce: &str,
        waypoint: &[u8],
    ) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::post("/waypoint")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("X-Bari-Hashcash-Nonce", nonce)
            .header("X-Bari-Hashcash-Header", hex::encode(header))
            .body(axum::body::Body::from(waypoint.to_vec()))
            .unwrap()
    }

    fn candidate_fields() -> std::collections::HashMap<String, String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../sdk/rust/candidates/hashcash.txt");
        let text = std::fs::read_to_string(path).unwrap();
        text.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let (key, value) = line.split_once('\t').unwrap();
                (key.to_owned(), value.to_owned())
            })
            .collect()
    }
}
