//! Read-only chain reads for `pcoc-index rebuild`.
//!
//! Esplora is the public HTTP API (default `https://mempool.space/api`).
//! RPC is Bitcoin Core JSON-RPC and calls only `getblockhash`,
//! `getblockheader`, and `getblock`.

use crate::document::IndexPlant;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::time::Duration;

pub const DEFAULT_ESPLORA_BASE: &str = "https://mempool.space/api";
/// How many recent blocks to fetch again and compare, looking for a reorg.
pub const DEFAULT_REORG_DEPTH: u32 = 6;
/// Pause between Esplora calls. The public API answers 429 when this is too small.
pub const DEFAULT_MIN_INTERVAL_MS: u64 = 250;
pub const DEFAULT_MAX_RETRIES: u32 = 6;

#[derive(Clone, Debug)]
pub struct BlockHeader {
    /// Display-order block hash, lowercase hex. Bitcoin Core / Esplora `id`.
    pub hash: String,
    pub height: u32,
    /// Display-order previous block hash. Absent at height 0.
    pub previous_hash: Option<String>,
    /// Header `nTime`, unix seconds.
    pub time: u64,
    /// 80-byte header, the bytes whose double SHA-256 is `hash`.
    pub raw: [u8; 80],
}

#[derive(Clone, Debug)]
pub struct FoundPlant {
    pub root_hex: String,
    pub txid: String,
    pub vout: u32,
}

impl FoundPlant {
    pub fn to_index(&self, block_hash: &str, block_height: u32) -> IndexPlant {
        IndexPlant {
            root: self.root_hex.clone(),
            tree_size: None,
            txid: self.txid.clone(),
            vout: self.vout,
            block_hash: block_hash.to_owned(),
            block_height,
        }
    }
}

#[derive(Debug)]
pub enum ChainError {
    Http { status: u16, url: String },
    Message(String),
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http { status, url } => write!(f, "HTTP {status} for {url}"),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ChainError {}

pub trait ChainSource {
    fn tip_height(&self) -> Result<u32, ChainError>;
    fn header_at_height(&self, height: u32) -> Result<BlockHeader, ChainError>;
    /// BARI-tagged outputs in this block, in transaction order.
    fn plants_in_block(&self, header: &BlockHeader) -> Result<Vec<FoundPlant>, ChainError>;
}

/// Bitcoin header hash: double SHA-256, displayed reversed (Bitcoin display order).
pub fn header_hash_display(raw: &[u8; 80]) -> String {
    let once = Sha256::digest(raw);
    let twice = Sha256::digest(once);
    let mut displayed = [0u8; 32];
    for (i, byte) in twice.iter().enumerate() {
        displayed[31 - i] = *byte;
    }
    hex_encode(&displayed)
}

pub fn previous_hash_display(raw: &[u8; 80]) -> String {
    let mut displayed = [0u8; 32];
    for i in 0..32 {
        displayed[31 - i] = raw[4 + i];
    }
    hex_encode(&displayed)
}

pub fn header_time(raw: &[u8; 80]) -> u64 {
    u64::from(u32::from_le_bytes(raw[68..72].try_into().expect("4 bytes")))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex_exact<const N: usize>(text: &str) -> Result<[u8; N], ChainError> {
    if text.len() != N * 2 {
        return Err(ChainError::Message(format!(
            "expected {N} bytes of hex, got {} characters",
            text.len()
        )));
    }
    let mut out = [0u8; N];
    let bytes = text.as_bytes();
    for i in 0..N {
        let hi = hex_val(bytes[i * 2]).ok_or_else(|| ChainError::Message("bad hex".to_owned()))?;
        let lo =
            hex_val(bytes[i * 2 + 1]).ok_or_else(|| ChainError::Message("bad hex".to_owned()))?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// One HTTP GET that can be replaced in tests.
pub trait HttpGet {
    fn get(&self, url: &str) -> Result<HttpResponse, ChainError>;
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    /// `Retry-After` in seconds, when the server sent one.
    pub retry_after: Option<u64>,
}

pub struct UreqGet {
    agent: ureq::Agent,
}

impl UreqGet {
    pub fn new(timeout: Duration) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            .timeout_connect(timeout)
            .timeout_read(timeout)
            .redirects(0)
            .build();
        Self { agent }
    }
}

impl HttpGet for UreqGet {
    fn get(&self, url: &str) -> Result<HttpResponse, ChainError> {
        match self.agent.get(url).call() {
            Ok(response) => {
                let status = response.status();
                Ok(HttpResponse {
                    status,
                    body: read_body(response)?,
                    retry_after: None,
                })
            }
            Err(ureq::Error::Status(status, response)) => {
                let retry_after = response
                    .header("retry-after")
                    .and_then(|value| value.parse::<u64>().ok());
                Ok(HttpResponse {
                    status,
                    body: read_body(response).unwrap_or_default(),
                    retry_after,
                })
            }
            Err(other) => Err(ChainError::Message(other.to_string())),
        }
    }
}

fn read_body(response: ureq::Response) -> Result<Vec<u8>, ChainError> {
    let mut body = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|error| ChainError::Message(error.to_string()))?;
    Ok(body)
}

/// Pauses between calls and retries HTTP 429.
pub struct LimitedGet<G> {
    inner: G,
    min_interval: Duration,
    max_retries: u32,
    /// Test hook. Production sleeps.
    sleeper: Box<dyn Fn(Duration) + Send + Sync>,
    waits: std::sync::Mutex<Vec<Duration>>,
    last: std::sync::Mutex<Option<std::time::Instant>>,
}

impl<G> LimitedGet<G> {
    pub fn new(inner: G, min_interval: Duration, max_retries: u32) -> Self {
        Self {
            inner,
            min_interval,
            max_retries,
            sleeper: Box::new(std::thread::sleep),
            waits: std::sync::Mutex::new(Vec::new()),
            last: std::sync::Mutex::new(None),
        }
    }

    pub fn with_sleeper<F>(mut self, sleeper: F) -> Self
    where
        F: Fn(Duration) + Send + Sync + 'static,
    {
        self.sleeper = Box::new(sleeper);
        self
    }

    fn pace(&self) {
        let mut last = self.last.lock().expect("pace lock");
        if let Some(then) = *last {
            let elapsed = then.elapsed();
            if elapsed < self.min_interval {
                let wait = self.min_interval - elapsed;
                self.waits.lock().expect("waits").push(wait);
                (self.sleeper)(wait);
            }
        }
        *last = Some(std::time::Instant::now());
    }

    fn note_backoff(&self, wait: Duration) {
        self.waits.lock().expect("waits").push(wait);
        (self.sleeper)(wait);
        *self.last.lock().expect("pace lock") = Some(std::time::Instant::now());
    }
}

impl<G: HttpGet> HttpGet for LimitedGet<G> {
    fn get(&self, url: &str) -> Result<HttpResponse, ChainError> {
        let mut attempt = 0u32;
        loop {
            self.pace();
            let response = self.inner.get(url)?;
            if response.status != 429 {
                return Ok(response);
            }
            if attempt >= self.max_retries {
                return Ok(response);
            }
            let wait = response
                .retry_after
                .map(Duration::from_secs)
                .filter(|wait| !wait.is_zero())
                .unwrap_or_else(|| backoff(self.min_interval, attempt));
            self.note_backoff(wait);
            attempt += 1;
        }
    }
}

fn backoff(min_interval: Duration, attempt: u32) -> Duration {
    let factor = 1u32 << attempt.min(5);
    min_interval
        .saturating_mul(factor)
        .max(Duration::from_millis(50))
}

pub struct EsploraSource<G> {
    base: String,
    http: G,
}

impl<G> EsploraSource<G> {
    pub fn new(base: impl Into<String>, http: G) -> Self {
        let base = base.into().trim_end_matches('/').to_owned();
        Self { base, http }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base, path.trim_start_matches('/'))
    }
}

impl<G: HttpGet> EsploraSource<G> {
    fn get_ok(&self, path: &str) -> Result<Vec<u8>, ChainError> {
        let url = self.url(path);
        let response = self.http.get(&url)?;
        if response.status == 200 {
            return Ok(response.body);
        }
        Err(ChainError::Http {
            status: response.status,
            url,
        })
    }

    fn get_json(&self, path: &str) -> Result<Value, ChainError> {
        let body = self.get_ok(path)?;
        serde_json::from_slice(&body)
            .map_err(|error| ChainError::Message(format!("esplora JSON at {path}: {error}")))
    }
}

impl<G: HttpGet> ChainSource for EsploraSource<G> {
    fn tip_height(&self) -> Result<u32, ChainError> {
        let body = self.get_ok("blocks/tip/height")?;
        let text =
            std::str::from_utf8(&body).map_err(|error| ChainError::Message(error.to_string()))?;
        text.trim()
            .parse::<u32>()
            .map_err(|error| ChainError::Message(format!("tip height: {error}")))
    }

    fn header_at_height(&self, height: u32) -> Result<BlockHeader, ChainError> {
        let hash_body = self.get_ok(&format!("block-height/{height}"))?;
        let hash = std::str::from_utf8(&hash_body)
            .map_err(|error| ChainError::Message(error.to_string()))?
            .trim()
            .to_owned();
        let meta = self.get_json(&format!("block/{hash}"))?;
        let previous = meta
            .get("previousblockhash")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let time = meta
            .get("timestamp")
            .and_then(Value::as_u64)
            .ok_or_else(|| ChainError::Message(format!("block {hash} has no timestamp")))?;
        let reported = meta
            .get("height")
            .and_then(Value::as_u64)
            .ok_or_else(|| ChainError::Message(format!("block {hash} has no height")))?;
        if reported != u64::from(height) {
            return Err(ChainError::Message(format!(
                "block {hash} reports height {reported}, asked for {height}"
            )));
        }
        let raw_body = self.get_ok(&format!("block/{hash}/header"))?;
        let raw_text = std::str::from_utf8(&raw_body)
            .map_err(|error| ChainError::Message(error.to_string()))?
            .trim();
        let raw = decode_hex_exact::<80>(raw_text)?;
        Ok(BlockHeader {
            hash,
            height,
            previous_hash: previous,
            time,
            raw,
        })
    }

    fn plants_in_block(&self, header: &BlockHeader) -> Result<Vec<FoundPlant>, ChainError> {
        // One raw block, then a local scan for the 39-byte BARI tag.
        // Paging `/block/:hash/txs` is one HTTP call per 25 transactions.
        let body = self.get_ok(&format!("block/{}/raw", header.hash))?;
        plants_in_raw_block(&body)
    }
}

/// Scan a serialized block for `OP_RETURN` `BARI` ‖ `0x02` ‖ root.
///
/// Order is transaction order. A block with more than one plant does not
/// match Bari's `plants` array order: Bari writes serving batches, which is
/// off-chain. The verifier matches any one plant (specification P14.5), so
/// the set is what a check uses.
pub fn plants_in_raw_block(raw: &[u8]) -> Result<Vec<FoundPlant>, ChainError> {
    if raw.len() < 80 {
        return Err(ChainError::Message(
            "block is shorter than a header".to_owned(),
        ));
    }
    let mut i = 80usize;
    let (ntx, next) = read_compact(raw, i)?;
    i = next;
    let mut found = Vec::new();
    for _ in 0..ntx {
        let (plants, next) = read_tx_plants(raw, i)?;
        found.extend(plants);
        i = next;
    }
    if i != raw.len() {
        return Err(ChainError::Message(format!(
            "block parse consumed {i} of {} bytes",
            raw.len()
        )));
    }
    Ok(found)
}

fn read_tx_plants(raw: &[u8], mut i: usize) -> Result<(Vec<FoundPlant>, usize), ChainError> {
    let version = read_exact(raw, i, 4)?;
    i += 4;
    let segwit = raw.get(i) == Some(&0) && raw.get(i + 1) == Some(&1);
    if segwit {
        i += 2;
    }
    let mut classic = Vec::with_capacity(256);
    classic.extend_from_slice(version);
    let (vin_n, j) = read_compact(raw, i)?;
    classic.extend_from_slice(&raw[i..j]);
    i = j;
    for _ in 0..vin_n {
        let start = i;
        i = advance(i, 36)?;
        let (slen, k) = read_compact(raw, i)?;
        i = advance(k, slen)?;
        i = advance(i, 4)?;
        if i > raw.len() {
            return Err(ChainError::Message(
                "tx input overruns the block".to_owned(),
            ));
        }
        classic.extend_from_slice(&raw[start..i]);
    }
    let (vout_n, j) = read_compact(raw, i)?;
    classic.extend_from_slice(&raw[i..j]);
    i = j;
    let mut scripts = Vec::new();
    for _ in 0..vout_n {
        let start = i;
        i = advance(i, 8)?;
        let (slen, k) = read_compact(raw, i)?;
        let end = advance(k, slen)?;
        if end > raw.len() {
            return Err(ChainError::Message(
                "tx output overruns the block".to_owned(),
            ));
        }
        scripts.push(raw[k..end].to_vec());
        i = end;
        classic.extend_from_slice(&raw[start..i]);
    }
    if segwit {
        for _ in 0..vin_n {
            let (count, j) = read_compact(raw, i)?;
            i = j;
            for _ in 0..count {
                let (len, j) = read_compact(raw, i)?;
                i = advance(j, len)?;
                if i > raw.len() {
                    return Err(ChainError::Message("witness overruns the block".to_owned()));
                }
            }
        }
    }
    let lock = read_exact(raw, i, 4)?;
    i += 4;
    classic.extend_from_slice(lock);
    let txid = txid_display(&classic);
    let mut found = Vec::new();
    for (vout, script) in scripts.iter().enumerate() {
        let Some(root) = crate::document::bari_root_from_script(script) else {
            continue;
        };
        found.push(FoundPlant {
            root_hex: root,
            txid: txid.clone(),
            vout: u32::try_from(vout).unwrap_or(u32::MAX),
        });
    }
    Ok((found, i))
}

fn advance(at: usize, by: u64) -> Result<usize, ChainError> {
    let by =
        usize::try_from(by).map_err(|_| ChainError::Message("length does not fit".to_owned()))?;
    at.checked_add(by)
        .ok_or_else(|| ChainError::Message("tx overrun".to_owned()))
}

fn txid_display(classic: &[u8]) -> String {
    let once = Sha256::digest(classic);
    let twice = Sha256::digest(once);
    let mut displayed = [0u8; 32];
    for (i, byte) in twice.iter().enumerate() {
        displayed[31 - i] = *byte;
    }
    hex_encode(&displayed)
}

fn read_exact(raw: &[u8], i: usize, n: usize) -> Result<&[u8], ChainError> {
    raw.get(i..i + n)
        .ok_or_else(|| ChainError::Message("block ended inside a transaction".to_owned()))
}

fn read_compact(raw: &[u8], i: usize) -> Result<(u64, usize), ChainError> {
    let first = *raw
        .get(i)
        .ok_or_else(|| ChainError::Message("compact size overruns the block".to_owned()))?;
    match first {
        n @ 0..=252 => Ok((u64::from(n), i + 1)),
        253 => {
            let bytes = read_exact(raw, i + 1, 2)?;
            Ok((
                u64::from(u16::from_le_bytes(bytes.try_into().unwrap())),
                i + 3,
            ))
        }
        254 => {
            let bytes = read_exact(raw, i + 1, 4)?;
            Ok((
                u64::from(u32::from_le_bytes(bytes.try_into().unwrap())),
                i + 5,
            ))
        }
        255 => Err(ChainError::Message(
            "compact size above 2^32 is not a block this reader accepts".to_owned(),
        )),
    }
}

#[derive(Clone, Debug)]
pub struct RpcAuth {
    pub username: String,
    pub password: String,
}

impl RpcAuth {
    /// `user:password` as Bitcoin Core writes it in `.cookie`.
    pub fn from_cookie(cookie: &str) -> Result<Self, ChainError> {
        let (username, password) = cookie.trim().split_once(':').ok_or_else(|| {
            ChainError::Message("Bitcoin cookie must contain user:password".to_owned())
        })?;
        if username.is_empty() || password.is_empty() {
            return Err(ChainError::Message(
                "Bitcoin cookie user and password must be non-empty".to_owned(),
            ));
        }
        Ok(Self {
            username: username.to_owned(),
            password: password.to_owned(),
        })
    }
}

pub struct RpcSource {
    url: String,
    auth_header: String,
    agent: ureq::Agent,
}

impl RpcSource {
    pub fn new(url: impl Into<String>, auth: RpcAuth, timeout: Duration) -> Self {
        let token = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{}:{}", auth.username, auth.password).as_bytes(),
        );
        let agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            .timeout_connect(timeout)
            .timeout_read(timeout)
            .redirects(0)
            .build();
        Self {
            url: url.into(),
            auth_header: format!("Basic {token}"),
            agent,
        }
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        let body = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "pcoc-index",
            "method": method,
            "params": params,
        });
        let response = self
            .agent
            .post(&self.url)
            .set("Authorization", &self.auth_header)
            .set("Content-Type", "application/json")
            .send_json(body)
            .map_err(|error| match error {
                ureq::Error::Status(status, response) => {
                    let text = response.into_string().unwrap_or_default();
                    ChainError::Message(format!("RPC HTTP {status}: {text}"))
                }
                other => ChainError::Message(other.to_string()),
            })?;
        let value: Value = response
            .into_json()
            .map_err(|error| ChainError::Message(error.to_string()))?;
        if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
            return Err(ChainError::Message(format!("RPC {method}: {error}")));
        }
        value
            .get("result")
            .cloned()
            .ok_or_else(|| ChainError::Message(format!("RPC {method} returned no result")))
    }
}

impl ChainSource for RpcSource {
    fn tip_height(&self) -> Result<u32, ChainError> {
        let hash = self.call("getbestblockhash", serde_json::json!([]))?;
        let hash = hash
            .as_str()
            .ok_or_else(|| ChainError::Message("getbestblockhash was not a string".to_owned()))?;
        let header = self.call("getblockheader", serde_json::json!([hash]))?;
        json_u32(&header, "height")
    }

    fn header_at_height(&self, height: u32) -> Result<BlockHeader, ChainError> {
        let hash = self.call("getblockhash", serde_json::json!([height]))?;
        let hash = hash
            .as_str()
            .ok_or_else(|| ChainError::Message("getblockhash was not a string".to_owned()))?
            .to_owned();
        let header = self.call("getblockheader", serde_json::json!([hash]))?;
        let previous = header
            .get("previousblockhash")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let time = header
            .get("time")
            .and_then(Value::as_u64)
            .ok_or_else(|| ChainError::Message(format!("header {hash} has no time")))?;
        let raw_hex = header
            .get("hex")
            .or_else(|| header.get("data"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ChainError::Message(format!(
                    "getblockheader {hash} omitted hex; Bitcoin Core verbose header includes it"
                ))
            })?;
        let raw = decode_hex_exact::<80>(raw_hex)?;
        let reported = json_u32(&header, "height")?;
        if reported != height {
            return Err(ChainError::Message(format!(
                "header {hash} reports height {reported}, asked for {height}"
            )));
        }
        Ok(BlockHeader {
            hash,
            height,
            previous_hash: previous,
            time,
            raw,
        })
    }

    fn plants_in_block(&self, header: &BlockHeader) -> Result<Vec<FoundPlant>, ChainError> {
        // verbosity 2: transactions decoded, no prevout. Read-only.
        let block = self.call("getblock", serde_json::json!([header.hash, 2]))?;
        let txs = block.get("tx").and_then(Value::as_array).ok_or_else(|| {
            ChainError::Message("getblock verbosity 2 has no tx array".to_owned())
        })?;
        let mut found = Vec::new();
        for tx in txs {
            let txid = tx
                .get("txid")
                .and_then(Value::as_str)
                .ok_or_else(|| ChainError::Message("getblock tx has no txid".to_owned()))?;
            let outputs = tx
                .get("vout")
                .and_then(Value::as_array)
                .ok_or_else(|| ChainError::Message(format!("tx {txid} has no vout")))?;
            for output in outputs {
                let script = output
                    .pointer("/scriptPubKey/hex")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let Some(root) = crate::document::bari_root_hex(script) else {
                    continue;
                };
                let vout = output
                    .get("n")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| ChainError::Message(format!("tx {txid} output has no n")))?;
                found.push(FoundPlant {
                    root_hex: root,
                    txid: txid.to_owned(),
                    vout,
                });
            }
        }
        Ok(found)
    }
}

fn json_u32(value: &Value, key: &str) -> Result<u32, ChainError> {
    let n = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| ChainError::Message(format!("missing integer {key}")))?;
    u32::try_from(n).map_err(|_| ChainError::Message(format!("{key} exceeds u32")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Scripted {
        responses: Mutex<Vec<HttpResponse>>,
        hits: Mutex<Vec<String>>,
    }

    impl Scripted {
        fn new(responses: Vec<HttpResponse>) -> Self {
            Self {
                responses: Mutex::new(responses),
                hits: Mutex::new(Vec::new()),
            }
        }
    }

    impl HttpGet for Scripted {
        fn get(&self, url: &str) -> Result<HttpResponse, ChainError> {
            self.hits.lock().unwrap().push(url.to_owned());
            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| ChainError::Message(format!("no scripted response for {url}")))
        }
    }

    fn ok(body: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            body: body.as_bytes().to_vec(),
            retry_after: None,
        }
    }

    #[test]
    fn limited_get_backs_off_on_429_then_returns_200() {
        // pop() takes the last item first.
        let scripted = Scripted::new(vec![
            ok("done"),
            HttpResponse {
                status: 429,
                body: b"slow".to_vec(),
                retry_after: Some(2),
            },
        ]);
        let limited = LimitedGet::new(scripted, Duration::from_millis(100), 3).with_sleeper(|_| {});
        let response = limited.get("https://example.test/a").unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"done");
        let waits = limited.waits.lock().unwrap().clone();
        assert!(
            waits.iter().any(|wait| *wait == Duration::from_secs(2)),
            "honours Retry-After, waits={waits:?}"
        );
    }

    #[test]
    fn limited_get_paces_successive_calls() {
        let scripted = Scripted::new(vec![ok("b"), ok("a")]);
        let limited = LimitedGet::new(scripted, Duration::from_millis(80), 1).with_sleeper(|_| {});
        assert_eq!(limited.get("https://example.test/1").unwrap().body, b"a");
        assert_eq!(limited.get("https://example.test/2").unwrap().body, b"b");
        let waits = limited.waits.lock().unwrap().clone();
        assert_eq!(waits.len(), 1, "the second call waits out the interval");
        assert!(waits[0] > Duration::ZERO);
        assert!(waits[0] <= Duration::from_millis(80));
    }

    #[test]
    fn esplora_reads_header_fields() {
        // pop order: last pushed is first served. Sequence: tip, hash, meta, header.
        let scripted = Scripted::new(vec![
            ok(&"11".repeat(80)),
            ok(r#"{"id":"hh","height":5,"timestamp":100,"previousblockhash":"prev"}"#),
            ok("hh"),
            ok("5"),
        ]);
        let source = EsploraSource::new("https://example.test/api", scripted);
        assert_eq!(source.tip_height().unwrap(), 5);
        let header = source.header_at_height(5).unwrap();
        assert_eq!(header.hash, "hh");
        assert_eq!(header.previous_hash.as_deref(), Some("prev"));
        assert_eq!(header.time, 100);
        assert_eq!(header.raw, [0x11; 80]);
    }

    #[test]
    fn raw_block_scan_finds_the_39_byte_tag() {
        let root = [0xABu8; 32];
        let mut script = vec![0x6a, 0x25];
        script.extend_from_slice(b"BARI\x02");
        script.extend_from_slice(&root);
        assert_eq!(script.len(), 39);
        // version 1, one input (empty script, coinbase-shaped), one OP_RETURN output.
        let mut tx = Vec::new();
        tx.extend_from_slice(&1u32.to_le_bytes());
        tx.push(1); // vin count
        tx.extend_from_slice(&[0u8; 32]); // prev txid
        tx.extend_from_slice(&0u32.to_le_bytes());
        tx.push(0); // empty script
        tx.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
        tx.push(1); // vout count
        tx.extend_from_slice(&0u64.to_le_bytes());
        tx.push(script.len() as u8);
        tx.extend_from_slice(&script);
        tx.extend_from_slice(&0u32.to_le_bytes());
        let mut block = vec![0u8; 80];
        block.push(1); // one transaction
        block.extend_from_slice(&tx);
        let plants = plants_in_raw_block(&block).unwrap();
        assert_eq!(plants.len(), 1);
        assert_eq!(plants[0].vout, 0);
        assert_eq!(plants[0].root_hex, "ab".repeat(32));
        assert_eq!(plants[0].txid.len(), 64);
    }
}
