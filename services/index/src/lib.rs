//! Read-only portable-index HTTP surface (`index.groundinglab.io`).
//!
//! Serves version-2 `{version,network,header,plant,plants}` documents from
//! `height-XXXXXXXXXX.json` files. Bind defaults to `127.0.0.1:38081`.
//! `GET /index` is the current tip. `GET /index/{height}` is the height fetch.
//! `GET /index/since` streams mainnet heights as newline-delimited JSON.
//! `GET /health` reports this build's version and commit. It is not an index document.
//! Produce Observe is the plane gossip peer, not this process.
//! These routes do not use Hashcash. CORS allows `https://groundinglab.io`,
//! `https://www.groundinglab.io`, and loopback HTTP (`localhost`, `127.0.0.1`, `[::1]`).

use crate::document::IndexDocument;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use pcoc_bari::storage::{IndexCache, StorageError};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::sync::Arc;

pub mod chain;
pub mod document;
pub mod rebuild;

/// Crate version on `GET /health`. Not the index document `version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Commit stamped by `build.rs`. Not read from the environment at request time.
pub fn compiled_commit() -> &'static str {
    env!("INDEX_COMMIT")
}

fn health_value() -> Value {
    serde_json::json!({
        "version": VERSION,
        "commit": compiled_commit(),
    })
}

/// Largest `limit` `GET /index/since` accepts.
///
/// A larger `limit` is **400**. Omitting `limit` does not apply this cap:
/// the response then includes every matching mainnet record from the start
/// height. Documented in the service README.
pub const SINCE_LIMIT_MAX: u32 = 10_000;

/// Origins allowed to fetch the portable index from a browser (CORS).
/// `https://groundinglab.io` and `https://www.groundinglab.io`, plus loopback HTTP
/// (`localhost`, `127.0.0.1`, `[::1]`) with an optional port. No other origin is allowed.
pub fn origin_allowed(origin: &str) -> bool {
    if origin == "https://groundinglab.io" || origin == "https://www.groundinglab.io" {
        return true;
    }
    if let Some(rest) = origin.strip_prefix("http://localhost") {
        return rest.is_empty() || rest.starts_with(':');
    }
    if let Some(rest) = origin.strip_prefix("http://127.0.0.1") {
        return rest.is_empty() || rest.starts_with(':');
    }
    if let Some(rest) = origin.strip_prefix("http://[::1]") {
        return rest.is_empty() || rest.starts_with(':');
    }
    false
}

fn cors_headers_for(request_headers: &HeaderMap) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let Some(origin) = request_headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return headers;
    };
    if !origin_allowed(origin) {
        return headers;
    }
    if let Ok(value) = HeaderValue::from_str(origin) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
    }
    headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Accept, Content-Type"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
    headers
}

fn with_index_cors(request_headers: &HeaderMap, mut response: Response) -> Response {
    for (name, value) in cors_headers_for(request_headers).iter() {
        response.headers_mut().insert(name, value.clone());
    }
    response
}

async fn index_options(headers: HeaderMap) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    for (name, value) in cors_headers_for(&headers).iter() {
        response.headers_mut().insert(name, value.clone());
    }
    if !response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, OPTIONS"),
        );
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Accept, Content-Type"),
        );
    }
    response
}

#[derive(Clone)]
pub struct IndexService {
    cache: Arc<IndexCache>,
    /// Height → unix seconds, used only when a sequencer directory is configured.
    /// Not applied when the document already has `header.time` or
    /// `header.timestamp_unix_ms`.
    clocks: Arc<HashMap<u32, u64>>,
    /// Production floor. Tip selection and `/index/since` ignore a lower
    /// height. `GET /index/{height}` also serves the median-time lookback
    /// of [`pcoc_bari::MTP_LOOKBACK`] headers below this value.
    /// `0` serves every retained file. The binary sets [`pcoc_bari::BORN_HEIGHT`].
    born_height: u32,
}

impl IndexService {
    pub fn new(cache: IndexCache) -> Self {
        Self {
            cache: Arc::new(cache),
            clocks: Arc::new(HashMap::new()),
            born_height: 0,
        }
    }

    pub fn open(data_dir: impl AsRef<std::path::Path>) -> Result<Self, StorageError> {
        Self::open_with_sequencer(data_dir, None)
    }

    /// `sequencer_dir` is optional. `None` serves the cache files as stored.
    pub fn open_with_sequencer(
        data_dir: impl AsRef<std::path::Path>,
        sequencer_dir: Option<&std::path::Path>,
    ) -> Result<Self, StorageError> {
        let clocks = match sequencer_dir {
            Some(dir) => load_sequencer_clocks(dir)?,
            None => HashMap::new(),
        };
        Ok(Self {
            cache: Arc::new(IndexCache::open(data_dir)?),
            clocks: Arc::new(clocks),
            born_height: 0,
        })
    }

    /// Production floor for this process. A test sets its own. `0` serves the
    /// whole cache. Stored files below the header floor are not deleted.
    pub fn with_born_height(mut self, born_height: u32) -> Self {
        self.born_height = born_height;
        self
    }

    /// Lowest height `GET /index/{height}` returns. The
    /// [`pcoc_bari::MTP_LOOKBACK`] headers below [`Self::born_height`] are
    /// included. Tip selection and `/index/since` use `born_height` itself.
    fn header_floor(&self) -> u32 {
        self.born_height.saturating_sub(pcoc_bari::MTP_LOOKBACK)
    }

    pub fn clock_count(&self) -> usize {
        self.clocks.len()
    }

    fn with_clock(&self, mut tip: IndexDocument) -> IndexDocument {
        // A stored `header.timestamp_unix_ms` is left as stored.
        // `header.time` is filled only when both time fields are absent.
        if tip.header.timestamp_unix_ms.is_some() {
            return tip;
        }
        if tip.header.time.is_none() {
            if let Some(secs) = self.clocks.get(&tip.header.height) {
                tip.header.time = Some(*secs);
            }
        }
        tip
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(fetch_health))
            .route("/index", get(fetch_index).options(index_options))
            // Registered before `/index/{height}`. Otherwise that route parses
            // `since` as a u32 and answers 400 (`Cannot parse since to a u32`).
            .route(
                "/index/since",
                get(fetch_index_since).options(index_options),
            )
            .route(
                "/index/{height}",
                get(fetch_index_at_height).options(index_options),
            )
            .with_state(self.clone())
    }

    pub fn load_latest(&self) -> Result<Option<IndexDocument>, CacheError> {
        self.load_file(self.latest_height()?)
    }

    pub fn load_at_height(&self, height: u32) -> Result<Option<IndexDocument>, CacheError> {
        if height < self.header_floor() {
            return Ok(None);
        }
        let path = self.cache.tip_path(height);
        if !path.exists() {
            return Ok(None);
        }
        self.read_document(&path).map(Some)
    }

    fn latest_height(&self) -> Result<Option<u32>, CacheError> {
        let mut best: Option<u32> = None;
        for entry in fs::read_dir(self.cache.dir())? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(rest) = name
                .strip_prefix("height-")
                .and_then(|s| s.strip_suffix(".json"))
            else {
                continue;
            };
            if rest.len() != 10 || !rest.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(height) = rest.parse::<u32>() else {
                continue;
            };
            if height < self.born_height {
                continue;
            }
            if best.is_none_or(|current| height > current) {
                best = Some(height);
            }
        }
        Ok(best)
    }

    fn load_file(&self, height: Option<u32>) -> Result<Option<IndexDocument>, CacheError> {
        let Some(height) = height else {
            return Ok(None);
        };
        self.load_at_height(height)
    }

    fn read_document(&self, path: &std::path::Path) -> Result<IndexDocument, CacheError> {
        let bytes = fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

/// A cache read failed. The HTTP layer answers 500 and an empty body.
#[derive(Debug)]
pub enum CacheError {
    Storage(StorageError),
    Io(io::Error),
    Json(serde_json::Error),
}

impl std::error::Error for CacheError {}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
        }
    }
}

impl From<io::Error> for CacheError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

async fn fetch_health() -> Json<Value> {
    Json(health_value())
}

async fn fetch_index(State(service): State<IndexService>, headers: HeaderMap) -> Response {
    match service.load_latest() {
        Ok(Some(tip)) => with_index_cors(&headers, Json(service.with_clock(tip)).into_response()),
        Ok(None) => with_index_cors(&headers, StatusCode::NOT_FOUND.into_response()),
        Err(_) => with_index_cors(&headers, StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
}

async fn fetch_index_at_height(
    State(service): State<IndexService>,
    Path(height): Path<u32>,
    headers: HeaderMap,
) -> Response {
    match service.load_at_height(height) {
        Ok(Some(tip)) => with_index_cors(&headers, Json(service.with_clock(tip)).into_response()),
        Ok(None) => with_index_cors(&headers, StatusCode::NOT_FOUND.into_response()),
        Err(_) => with_index_cors(&headers, StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
}

#[derive(Debug, Deserialize)]
struct SinceQuery {
    height: Option<u32>,
    limit: Option<u32>,
}

/// One NDJSON record. Same compact JSON `Json<IndexDocument>` writes
/// for `GET /index/{height}`, then a newline.
fn index_ndjson_line(index: &IndexDocument) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(index)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Filename heights, ascending. Same `height-` + ten digits + `.json` rule as
/// `IndexCache::load_latest`.
fn cached_heights(dir: &std::path::Path) -> Result<Vec<u32>, StorageError> {
    let mut heights = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(rest) = name
            .strip_prefix("height-")
            .and_then(|s| s.strip_suffix(".json"))
        else {
            continue;
        };
        if rest.len() != 10 || !rest.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(height) = rest.parse::<u32>() else {
            continue;
        };
        heights.push(height);
    }
    heights.sort_unstable();
    heights.dedup();
    Ok(heights)
}

struct SinceLines {
    service: IndexService,
    heights: Vec<u32>,
    index: usize,
    start: Option<u32>,
    limit: Option<u32>,
    emitted: u32,
    born_height: u32,
}

impl Iterator for SinceLines {
    type Item = Result<Vec<u8>, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index >= self.heights.len() {
                return None;
            }
            let height = self.heights[self.index];
            self.index += 1;
            if height < self.born_height {
                continue;
            }
            if let Some(start) = self.start {
                if height < start {
                    continue;
                }
            }
            if let Some(limit) = self.limit {
                if self.emitted >= limit {
                    return None;
                }
            }
            let tip = match self.service.load_at_height(height) {
                Ok(Some(tip)) => tip,
                // Gap: no file at this height (the height GET would be
                // 404). Store no line and keep going. Do not stop the stream.
                Ok(None) => continue,
                Err(error) => return Some(Err(std::io::Error::other(error.to_string()))),
            };
            if tip.network != "main" {
                continue;
            }
            let tipped = self.service.with_clock(tip);
            match index_ndjson_line(&tipped) {
                Ok(bytes) => {
                    self.emitted = self.emitted.saturating_add(1);
                    return Some(Ok(bytes));
                }
                Err(error) => return Some(Err(std::io::Error::other(error.to_string()))),
            }
        }
    }
}

async fn fetch_index_since(
    State(service): State<IndexService>,
    Query(query): Query<SinceQuery>,
    headers: HeaderMap,
) -> Response {
    if let Some(limit) = query.limit {
        if limit > SINCE_LIMIT_MAX {
            return with_index_cors(&headers, StatusCode::BAD_REQUEST.into_response());
        }
    }
    let heights = match cached_heights(service.cache.dir()) {
        Ok(heights) => heights,
        Err(_) => {
            return with_index_cors(&headers, StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
    };
    let born_height = service.born_height;
    let stream = futures_util::stream::iter(SinceLines {
        service,
        heights,
        index: 0,
        start: query.height,
        limit: query.limit,
        emitted: 0,
        born_height,
    });
    let response = (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-ndjson"),
        )],
        Body::from_stream(stream),
    )
        .into_response();
    with_index_cors(&headers, response)
}

/// Unix seconds for one height, from sequencer persistence.
///
/// `state.json` `headers[].minted_unix_ms` wins. Heights still at zero are
/// filled from `events.jsonl`: `initialized` is height 0, `header_minted` uses
/// `payload.height`, and the clock is that line's `observed_unix_ms`.
pub fn load_sequencer_clocks(dir: &std::path::Path) -> Result<HashMap<u32, u64>, StorageError> {
    if !dir.is_dir() {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("sequencer dir {} is not a directory", dir.display()),
        )));
    }
    let state_path = dir.join("state.json");
    let events_path = dir.join("events.jsonl");
    if !state_path.is_file() && !events_path.is_file() {
        return Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "sequencer dir {} has neither state.json nor events.jsonl",
                dir.display()
            ),
        )));
    }

    let mut clocks = HashMap::new();
    if state_path.is_file() {
        let value: Value = serde_json::from_slice(&fs::read(&state_path)?)?;
        if let Some(headers) = value.get("headers").and_then(Value::as_array) {
            for header in headers {
                let Some(height) = header.get("height").and_then(Value::as_u64) else {
                    continue;
                };
                let Ok(height) = u32::try_from(height) else {
                    continue;
                };
                let Some(ms) = header.get("minted_unix_ms").and_then(Value::as_u64) else {
                    continue;
                };
                if let Some(secs) = unix_secs(ms) {
                    clocks.insert(height, secs);
                }
            }
        }
    }
    if events_path.is_file() {
        let text = fs::read_to_string(&events_path)?;
        for line in text.lines() {
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(ms) = value.get("observed_unix_ms").and_then(Value::as_u64) else {
                continue;
            };
            let Some(secs) = unix_secs(ms) else {
                continue;
            };
            match value.get("event").and_then(Value::as_str) {
                Some("initialized") => {
                    clocks.entry(0).or_insert(secs);
                }
                Some("header_minted") => {
                    if let Some(height) = value
                        .pointer("/payload/height")
                        .and_then(Value::as_u64)
                        .and_then(|height| u32::try_from(height).ok())
                    {
                        clocks.entry(height).or_insert(secs);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(clocks)
}

fn unix_secs(unix_ms: u64) -> Option<u64> {
    let secs = unix_ms / 1000;
    (secs > 0).then_some(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{IndexHeader, IndexPlant};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn write_tip(dir: &std::path::Path, height: u32, network: &str) {
        let tip = IndexDocument {
            version: 2,
            network: network.to_owned(),
            header: IndexHeader {
                hash: "aa".repeat(32),
                height,
                time: None,
                timestamp_unix_ms: None,
            },
            plant: None,
            plants: Vec::new(),
        };
        let path = dir.join(format!("height-{height:010}.json"));
        std::fs::write(path, serde_json::to_vec_pretty(&tip).unwrap()).unwrap();
    }

    async fn body_json(response: Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn health_reports_version_and_commit_without_changing_the_index_document() {
        let temp = tempfile::tempdir().unwrap();
        write_tip(temp.path(), 19001, "main");
        let app = IndexService::open(temp.path()).unwrap().router();

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
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let health_json = body_json(health).await;
        assert_eq!(health_json["version"], VERSION);
        assert_eq!(health_json["version"], "0.1.6");
        assert_eq!(health_json["commit"], compiled_commit());
        assert!(compiled_commit().len() >= 7, "{}", compiled_commit());
        let mut keys: Vec<&str> = health_json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["commit", "version"]);

        let tip = app
            .oneshot(
                axum::http::Request::get("/index")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(tip.status(), StatusCode::OK);
        let tip_json = body_json(tip).await;
        assert_eq!(tip_json["version"], 2);
        assert!(tip_json.get("commit").is_none(), "{tip_json}");
    }

    #[tokio::test]
    async fn health_on_an_empty_cache() {
        let temp = tempfile::tempdir().unwrap();
        let app = IndexService::open(temp.path()).unwrap().router();
        let health = app
            .oneshot(
                axum::http::Request::get("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);
        let health_json = body_json(health).await;
        assert_eq!(health_json["version"], VERSION);
        assert_eq!(health_json["commit"], compiled_commit());
    }

    #[tokio::test]
    async fn tip_and_by_height_from_fixture_cache() {
        let temp = tempfile::tempdir().unwrap();
        write_tip(temp.path(), 19001, "synthetic");
        write_tip(temp.path(), 19002, "synthetic");
        let service = IndexService::open(temp.path()).unwrap();
        let app = service.router();

        let tip = app
            .clone()
            .oneshot(
                axum::http::Request::get("/index")
                    .header(header::ORIGIN, "https://groundinglab.io")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(tip.status(), 200);
        assert_eq!(
            tip.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://groundinglab.io"
        );
        let tip_json = body_json(tip).await;
        assert_eq!(tip_json["version"], 2);
        assert_eq!(tip_json["network"], "synthetic");
        assert_eq!(tip_json["header"]["height"], 19002);
        assert!(tip_json["plant"].is_null());

        let at = app
            .clone()
            .oneshot(
                axum::http::Request::get("/index/19001")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(at.status(), 200);
        let at_json = body_json(at).await;
        assert_eq!(at_json["header"]["height"], 19001);

        let miss = app
            .clone()
            .oneshot(
                axum::http::Request::get("/index/999999")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(miss.status(), 404);

        let preflight = app
            .oneshot(
                axum::http::Request::builder()
                    .method("OPTIONS")
                    .uri("/index")
                    .header(header::ORIGIN, "https://groundinglab.io")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            preflight
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://groundinglab.io"
        );
    }

    #[tokio::test]
    async fn sequencer_events_fill_a_missing_header_time_and_a_stored_time_wins() {
        let index = tempfile::tempdir().unwrap();
        write_tip(index.path(), 7, "synthetic");
        let stored = IndexDocument {
            version: 2,
            network: "synthetic".to_owned(),
            header: IndexHeader {
                hash: "bb".repeat(32),
                height: 8,
                time: Some(1_800_000_000),
                timestamp_unix_ms: None,
            },
            plant: None,
            plants: Vec::new(),
        };
        std::fs::write(
            index.path().join("height-0000000008.json"),
            serde_json::to_vec_pretty(&stored).unwrap(),
        )
        .unwrap();

        let seq = tempfile::tempdir().unwrap();
        std::fs::write(
            seq.path().join("state.json"),
            r#"{"headers":[{"height":7,"minted_unix_ms":0},{"height":8,"minted_unix_ms":1900000000000}]}"#,
        )
        .unwrap();
        std::fs::write(
            seq.path().join("events.jsonl"),
            "{\"event\":\"header_minted\",\"observed_unix_ms\":1700000007000,\"payload\":{\"height\":7}}\n",
        )
        .unwrap();

        let service = IndexService::open_with_sequencer(index.path(), Some(seq.path())).unwrap();
        assert_eq!(service.clock_count(), 2);
        let app = service.router();

        let at7 = app
            .clone()
            .oneshot(
                axum::http::Request::get("/index/7")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let at7 = body_json(at7).await;
        assert_eq!(at7["header"]["time"], 1_700_000_007);
        assert!(at7["header"].get("timestamp_unix_ms").is_none());

        let at8 = app
            .oneshot(
                axum::http::Request::get("/index/8")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let at8 = body_json(at8).await;
        assert_eq!(at8["header"]["time"], 1_800_000_000);
    }

    #[tokio::test]
    async fn observation_stamp_is_not_shadowed_by_a_sequencer_clock() {
        let index = tempfile::tempdir().unwrap();
        let stored = IndexDocument {
            version: 2,
            network: "synthetic".to_owned(),
            header: IndexHeader {
                hash: "cc".repeat(32),
                height: 9,
                time: None,
                timestamp_unix_ms: Some(1_700_000_009_500),
            },
            plant: None,
            plants: Vec::new(),
        };
        std::fs::write(
            index.path().join("height-0000000009.json"),
            serde_json::to_vec_pretty(&stored).unwrap(),
        )
        .unwrap();
        let seq = tempfile::tempdir().unwrap();
        std::fs::write(
            seq.path().join("state.json"),
            r#"{"headers":[{"height":9,"minted_unix_ms":1800000000000}]}"#,
        )
        .unwrap();
        let service = IndexService::open_with_sequencer(index.path(), Some(seq.path())).unwrap();
        let app = service.router();
        let response = app
            .oneshot(
                axum::http::Request::get("/index/9")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = body_json(response).await;
        assert!(body["header"].get("time").is_none());
        assert_eq!(body["header"]["timestamp_unix_ms"], 1_700_000_009_500u64);
    }

    #[test]
    fn cors_allows_groundinglab_and_loopback_only() {
        assert!(origin_allowed("https://groundinglab.io"));
        assert!(origin_allowed("https://www.groundinglab.io"));
        assert!(!origin_allowed("http://www.groundinglab.io"));
        assert!(!origin_allowed("https://www.groundinglab.io.evil"));
        assert!(!origin_allowed("https://ww.groundinglab.io"));
        assert!(origin_allowed("http://localhost:5173"));
        assert!(origin_allowed("http://127.0.0.1:8080"));
        assert!(origin_allowed("http://[::1]:8080"));
        assert!(!origin_allowed("https://evil.example"));
        assert!(!origin_allowed("http://example.lan:3000"));
        assert!(!origin_allowed("http://localhost.evil"));
        assert!(!origin_allowed("https://groundinglab.io.evil"));
        assert!(!origin_allowed("https://localhost"));
    }

    #[tokio::test]
    async fn www_origin_is_reflected_on_index() {
        let temp = tempfile::tempdir().unwrap();
        let service = IndexService::open(temp.path()).unwrap();
        let app = service.router();

        let get = app
            .clone()
            .oneshot(
                axum::http::Request::get("/index")
                    .header(header::ORIGIN, "https://www.groundinglab.io")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            get.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://www.groundinglab.io"
        );

        let preflight = app
            .oneshot(
                axum::http::Request::builder()
                    .method("OPTIONS")
                    .uri("/index")
                    .header(header::ORIGIN, "https://www.groundinglab.io")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            preflight
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://www.groundinglab.io"
        );
    }

    #[tokio::test]
    async fn empty_cache_is_404_and_height_preflight_allows_pcoc_app() {
        let temp = tempfile::tempdir().unwrap();
        let service = IndexService::open(temp.path()).unwrap();
        let app = service.router();

        let empty = app
            .clone()
            .oneshot(
                axum::http::Request::get("/index")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(empty.status(), StatusCode::NOT_FOUND);

        let preflight = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("OPTIONS")
                    .uri("/index/19001")
                    .header(header::ORIGIN, "https://groundinglab.io")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            preflight
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://groundinglab.io"
        );

        let not_a_height = app
            .clone()
            .oneshot(
                axum::http::Request::get("/index/not-a-height")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(not_a_height.status(), StatusCode::BAD_REQUEST);

        let denied = app
            .oneshot(
                axum::http::Request::get("/index")
                    .header(header::ORIGIN, "https://evil.example")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::NOT_FOUND);
        assert!(denied
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
    }

    fn write_stored(dir: &std::path::Path, tip: &IndexDocument) {
        let path = dir.join(format!("height-{:010}.json", tip.header.height));
        std::fs::write(path, serde_json::to_vec_pretty(tip).unwrap()).unwrap();
    }

    fn plant(block_height: u32, marker: &str) -> IndexPlant {
        IndexPlant {
            root: marker.repeat(32),
            tree_size: Some(2),
            txid: "ab".repeat(32),
            vout: 1,
            block_hash: "cd".repeat(32),
            block_height,
        }
    }

    fn stored(
        height: u32,
        network: &str,
        plant: Option<IndexPlant>,
        plants: Vec<IndexPlant>,
    ) -> IndexDocument {
        IndexDocument {
            version: 2,
            network: network.to_owned(),
            header: IndexHeader {
                hash: format!("{height:02x}").repeat(32),
                height,
                time: None,
                timestamp_unix_ms: None,
            },
            plant,
            plants,
        }
    }

    async fn body_bytes(response: Response) -> Vec<u8> {
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec()
    }

    fn ndjson_lines(bytes: &[u8]) -> Vec<Vec<u8>> {
        if bytes.is_empty() {
            return Vec::new();
        }
        assert_eq!(
            *bytes.last().unwrap(),
            b'\n',
            "ndjson records end with a newline"
        );
        bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| line.to_vec())
            .collect()
    }

    async fn get(app: &axum::Router, uri: &str) -> Response {
        app.clone()
            .oneshot(
                axum::http::Request::get(uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn since_empty_cache_is_an_empty_ndjson_body() {
        let temp = tempfile::tempdir().unwrap();
        let service = IndexService::open(temp.path()).unwrap();
        let app = service.router();
        let response = get(&app, "/index/since").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-ndjson"
        );
        let bytes = body_bytes(response).await;
        assert!(bytes.is_empty());
        assert!(ndjson_lines(&bytes).is_empty());
    }

    #[tokio::test]
    async fn since_defaults_filters_limit_and_mainnet_only() {
        let temp = tempfile::tempdir().unwrap();
        write_stored(temp.path(), &stored(5, "synthetic", None, Vec::new()));
        write_stored(temp.path(), &stored(6, "main", None, Vec::new()));
        write_stored(
            temp.path(),
            &stored(7, "main", Some(plant(5, "11")), Vec::new()),
        );
        write_stored(temp.path(), &stored(8, "signet", None, Vec::new()));
        write_stored(
            temp.path(),
            &stored(
                9,
                "main",
                Some(plant(9, "22")),
                vec![plant(9, "22"), plant(9, "33")],
            ),
        );
        let service = IndexService::open(temp.path()).unwrap();
        let app = service.router();

        let all = get(&app, "/index/since").await;
        assert_eq!(all.status(), StatusCode::OK);
        assert_eq!(
            all.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-ndjson"
        );
        let all_bytes = body_bytes(all).await;
        let lines = ndjson_lines(&all_bytes);
        assert_eq!(lines.len(), 3, "synthetic and signet files are omitted");
        let heights: Vec<u64> = lines
            .iter()
            .map(|line| {
                serde_json::from_slice::<serde_json::Value>(line).unwrap()["header"]["height"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        assert_eq!(heights, vec![6, 7, 9]);

        let from_seven = get(&app, "/index/since?height=7").await;
        assert_eq!(from_seven.status(), StatusCode::OK);
        let from_seven = ndjson_lines(&body_bytes(from_seven).await);
        let from_heights: Vec<u64> = from_seven
            .iter()
            .map(|line| {
                serde_json::from_slice::<serde_json::Value>(line).unwrap()["header"]["height"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        assert_eq!(from_heights, vec![7, 9]);

        let limited = get(&app, "/index/since?limit=1").await;
        let limited = ndjson_lines(&body_bytes(limited).await);
        assert_eq!(limited.len(), 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&limited[0]).unwrap()["header"]["height"],
            6
        );

        // A non-mainnet file between two mainnet files does not consume `limit`.
        let skipped = get(&app, "/index/since?height=7&limit=2").await;
        let skipped = ndjson_lines(&body_bytes(skipped).await);
        assert_eq!(skipped.len(), 2);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&skipped[0]).unwrap()["header"]["height"],
            7
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&skipped[1]).unwrap()["header"]["height"],
            9
        );

        let past = get(&app, "/index/since?height=100").await;
        assert_eq!(past.status(), StatusCode::OK);
        assert!(body_bytes(past).await.is_empty());

        let zero = get(&app, "/index/since?limit=0").await;
        assert_eq!(zero.status(), StatusCode::OK);
        assert_eq!(
            zero.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-ndjson"
        );
        assert!(body_bytes(zero).await.is_empty());

        let at_cap = get(&app, &format!("/index/since?limit={SINCE_LIMIT_MAX}")).await;
        assert_eq!(at_cap.status(), StatusCode::OK);
        assert_eq!(ndjson_lines(&body_bytes(at_cap).await).len(), 3);

        let over = get(&app, &format!("/index/since?limit={}", SINCE_LIMIT_MAX + 1)).await;
        assert_eq!(over.status(), StatusCode::BAD_REQUEST);
        assert!(body_bytes(over).await.is_empty());

        let bad_height = get(&app, "/index/since?height=nope").await;
        assert_eq!(bad_height.status(), StatusCode::BAD_REQUEST);

        let synthetic_only = tempfile::tempdir().unwrap();
        write_stored(
            synthetic_only.path(),
            &stored(4, "synthetic", None, Vec::new()),
        );
        let synthetic_app = IndexService::open(synthetic_only.path()).unwrap().router();
        let only_synthetic = get(&synthetic_app, "/index/since").await;
        assert_eq!(only_synthetic.status(), StatusCode::OK);
        assert!(body_bytes(only_synthetic).await.is_empty());
        let still_there = get(&synthetic_app, "/index/4").await;
        assert_eq!(still_there.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn since_lines_are_byte_identical_to_single_height_gets() {
        let index = tempfile::tempdir().unwrap();
        let plain = stored(6, "main", None, Vec::new());
        let mut carried = stored(7, "main", Some(plant(5, "11")), Vec::new());
        carried.header.timestamp_unix_ms = Some(1_700_000_007_000);
        let mut block = stored(
            9,
            "main",
            Some(plant(9, "22")),
            vec![plant(9, "22"), plant(9, "33")],
        );
        block.header.time = Some(1_700_000_009);
        write_stored(index.path(), &plain);
        write_stored(index.path(), &carried);
        write_stored(index.path(), &block);
        write_stored(index.path(), &stored(8, "synthetic", None, Vec::new()));

        let seq = tempfile::tempdir().unwrap();
        std::fs::write(
            seq.path().join("state.json"),
            r#"{"headers":[{"height":6,"minted_unix_ms":1700000006000}]}"#,
        )
        .unwrap();
        let service = IndexService::open_with_sequencer(index.path(), Some(seq.path())).unwrap();
        let app = service.router();

        let since = get(&app, "/index/since").await;
        assert_eq!(since.status(), StatusCode::OK);
        let lines = ndjson_lines(&body_bytes(since).await);
        assert_eq!(lines.len(), 3);

        for (line, height) in lines.iter().zip([6u32, 7, 9]) {
            let single = get(&app, &format!("/index/{height}")).await;
            assert_eq!(single.status(), StatusCode::OK);
            assert_eq!(
                single.headers().get(header::CONTENT_TYPE).unwrap(),
                "application/json"
            );
            let single_bytes = body_bytes(single).await;
            assert_eq!(
                line.as_slice(),
                single_bytes.as_slice(),
                "height {height} ndjson line differs from GET /index/{{height}}"
            );
        }

        let plain_json: serde_json::Value = serde_json::from_slice(&lines[0]).unwrap();
        assert!(plain_json.get("plants").is_none());
        assert!(plain_json["plant"].is_null());
        assert_eq!(plain_json["header"]["time"], 1_700_000_006);

        let carried_json: serde_json::Value = serde_json::from_slice(&lines[1]).unwrap();
        assert!(carried_json.get("plants").is_none());
        assert_eq!(carried_json["plant"]["block_height"], 5);
        assert_eq!(
            carried_json["header"]["timestamp_unix_ms"],
            1_700_000_007_000u64
        );
        assert!(carried_json["header"].get("time").is_none());

        let block_json: serde_json::Value = serde_json::from_slice(&lines[2]).unwrap();
        assert_eq!(block_json["plants"].as_array().unwrap().len(), 2);
        assert_eq!(block_json["plants"][0]["block_height"], 9);
        assert_eq!(block_json["plants"][1]["block_height"], 9);
        assert_ne!(
            block_json["plants"][0]["root"],
            block_json["plants"][1]["root"]
        );
        assert_eq!(block_json["header"]["time"], 1_700_000_009);
        assert!(block_json["header"].get("timestamp_unix_ms").is_none());
    }

    #[tokio::test]
    async fn since_preflight_allows_pcoc_app() {
        let temp = tempfile::tempdir().unwrap();
        let app = IndexService::open(temp.path()).unwrap().router();
        let preflight = app
            .oneshot(
                axum::http::Request::builder()
                    .method("OPTIONS")
                    .uri("/index/since")
                    .header(header::ORIGIN, "https://groundinglab.io")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            preflight
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://groundinglab.io"
        );
        assert_eq!(
            preflight
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                .unwrap(),
            "GET, OPTIONS"
        );
    }

    #[tokio::test]
    async fn since_route_is_not_a_height_parse() {
        let temp = tempfile::tempdir().unwrap();
        write_stored(temp.path(), &stored(969873, "main", None, Vec::new()));
        let app = IndexService::open(temp.path()).unwrap().router();

        let since = get(&app, "/index/since").await;
        assert_eq!(since.status(), StatusCode::OK);
        assert_ne!(since.status(), StatusCode::BAD_REQUEST);
        let since_body = body_bytes(since).await;
        let since_text = String::from_utf8(since_body).unwrap();
        assert!(
            !since_text.contains("Cannot parse"),
            "GET /index/since was handled as a height: {since_text}"
        );

        let with_height = get(&app, "/index/since?height=969873").await;
        assert_eq!(with_height.status(), StatusCode::OK);
        let with_height = String::from_utf8(body_bytes(with_height).await).unwrap();
        assert!(!with_height.contains("Cannot parse"));

        let not_a_height = get(&app, "/index/not-a-height").await;
        assert_eq!(not_a_height.status(), StatusCode::BAD_REQUEST);
        let err = String::from_utf8(body_bytes(not_a_height).await).unwrap();
        assert!(
            err.contains("Cannot parse"),
            "a non-u32 height path is still rejected, got {err}"
        );
    }

    #[tokio::test]
    async fn since_skips_a_gap_and_keeps_surrounding_mainnet_records() {
        let temp = tempfile::tempdir().unwrap();
        // 969874 is the live hole: no file. Neighbours are mainnet.
        write_stored(temp.path(), &stored(969872, "main", None, Vec::new()));
        write_stored(temp.path(), &stored(969873, "main", None, Vec::new()));
        let mut above = stored(
            969875,
            "main",
            Some(plant(969875, "44")),
            vec![plant(969875, "44")],
        );
        above.header.time = Some(1_700_000_875);
        write_stored(temp.path(), &above);
        let app = IndexService::open(temp.path()).unwrap().router();

        let missing = get(&app, "/index/969874").await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert!(body_bytes(missing).await.is_empty());

        let since = get(&app, "/index/since?height=969872").await;
        assert_eq!(since.status(), StatusCode::OK);
        assert_eq!(
            since.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-ndjson"
        );
        let lines = ndjson_lines(&body_bytes(since).await);
        let heights: Vec<u64> = lines
            .iter()
            .map(|line| {
                serde_json::from_slice::<serde_json::Value>(line).unwrap()["header"]["height"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        assert_eq!(heights, vec![969872, 969873, 969875]);

        for (line, height) in lines.iter().zip([969872u32, 969873, 969875]) {
            let single = get(&app, &format!("/index/{height}")).await;
            assert_eq!(single.status(), StatusCode::OK);
            assert_eq!(line.as_slice(), body_bytes(single).await.as_slice());
        }

        let from_the_hole = get(&app, "/index/since?height=969874").await;
        assert_eq!(from_the_hole.status(), StatusCode::OK);
        let from_the_hole = ndjson_lines(&body_bytes(from_the_hole).await);
        assert_eq!(from_the_hole.len(), 1);
        let only = serde_json::from_slice::<serde_json::Value>(&from_the_hole[0]).unwrap();
        assert_eq!(only["header"]["height"], 969875);
        let single = get(&app, "/index/969875").await;
        assert_eq!(
            from_the_hole[0].as_slice(),
            body_bytes(single).await.as_slice()
        );
    }

    /// This test sets its own floor. Lookback headers are served by height
    /// and are not the tip or a `/index/since` line. A file below the
    /// lookback stays on disk and is 404.
    #[tokio::test]
    async fn lookback_headers_are_served_and_are_not_production_heights() {
        let temp = tempfile::tempdir().unwrap();
        let born = 40u32;
        let lookback = pcoc_bari::MTP_LOOKBACK;
        let floor = born - lookback;
        let below = floor - 1;
        write_stored(temp.path(), &stored(below, "main", None, Vec::new()));
        write_stored(temp.path(), &stored(floor, "main", None, Vec::new()));
        write_stored(temp.path(), &stored(born - 1, "main", None, Vec::new()));
        write_stored(temp.path(), &stored(born, "main", None, Vec::new()));
        let below_path = temp.path().join(format!("height-{below:010}.json"));
        assert!(below_path.is_file());
        let app = IndexService::open(temp.path())
            .unwrap()
            .with_born_height(born)
            .router();

        let hidden = get(&app, &format!("/index/{below}")).await;
        assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
        assert!(below_path.is_file(), "the stored file is left in place");

        for height in [floor, born - 1, born] {
            let served = get(&app, &format!("/index/{height}")).await;
            assert_eq!(served.status(), StatusCode::OK, "{height}");
            let body =
                serde_json::from_slice::<serde_json::Value>(&body_bytes(served).await).unwrap();
            assert_eq!(body["header"]["height"], height);
        }

        let tip = get(&app, "/index").await;
        assert_eq!(tip.status(), StatusCode::OK);
        let tip_json = serde_json::from_slice::<serde_json::Value>(&body_bytes(tip).await).unwrap();
        assert_eq!(tip_json["header"]["height"], born);

        let since = get(&app, "/index/since").await;
        let lines = ndjson_lines(&body_bytes(since).await);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&lines[0]).unwrap()["header"]["height"],
            born
        );

        let since_from_window = get(&app, &format!("/index/since?height={floor}")).await;
        let lines = ndjson_lines(&body_bytes(since_from_window).await);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&lines[0]).unwrap()["header"]["height"],
            born
        );
    }
}
