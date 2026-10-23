//! Bari waypoint client. SPEC §13.1 and §13.6.
//!
//! Posts and path asks run on their own threads. `enqueue_post` and
//! `enqueue_header` return without waiting for the network (Rule A6).
//! The client sends the waypoint bytes and the two Hashcash headers. It sends
//! no secret. One resend,  the caller chooses the gap; the live default is 30s.

use crate::source::HeldTip;
use pcoc::constants::{ANCHOR_WINDOW_OFFSETS, HASHCASH_DIFFICULTY};
use pcoc::hashcash::{hashcash_challenge, hashcash_solve, header_hash, nonce_decimal};
use pcoc::hexutil::hex_encode;
use pcoc::path::{decode_path, validate_for};
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Public Bari the desktop app already names. No trailing slash.
pub const DEFAULT_BARI_URL: &str = "https://bari.groundinglab.io";

/// SPEC §13.1. The only resend gap on a live run.
pub const RESEND_AFTER: Duration = Duration::from_secs(30);

const MISS_PHRASES: [&str; 5] = [
    "still in the pool",
    "sealed but not in a block yet",
    "not included",
    "already archived after it was served",
    "unknown",
];

/// What the walk asks Bari to do. The walk does not wait for the outcome.
pub struct PostRequest {
    pub stage: usize,
    pub waypoint: [u8; 32],
    pub beacon_height: u64,
}

pub trait BariHub: Send + Sync {
    fn enqueue_post(&self, request: PostRequest);
    fn enqueue_header(&self, height: u64);
    /// A checked path already in hand for `stage` (`W_stage`). `None` is a skip.
    fn path(&self, stage: usize) -> Option<Vec<u8>>;
}

/// Ignores every post and ask. Fail-closed runs use this so nothing is sent.
pub struct SilentBari;

impl BariHub for SilentBari {
    fn enqueue_post(&self, _request: PostRequest) {}
    fn enqueue_header(&self, _height: u64) {}
    fn path(&self, _stage: usize) -> Option<Vec<u8>> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveEvent {
    Terrain,
    Leg {
        stage: usize,
        leg: usize,
        leg_result: u64,
    },
    Interrupt {
        stage: usize,
        height: u64,
    },
    /// A header the source learned. Not itself an interrupt.
    Header {
        height: u64,
        hash: [u8; 32],
        n_time: u32,
    },
    Waypoint {
        stage: usize,
        waypoint: [u8; 32],
    },
    Posted {
        stage: usize,
        waypoint: [u8; 32],
        attempt: u8,
        status: Option<u16>,
        succeeded: bool,
        dropped: bool,
    },
    Fetch {
        stage: usize,
        waypoint: [u8; 32],
        status: Option<u16>,
        worked: bool,
        at_height: u64,
        block: Option<[u8; 32]>,
        block_height: Option<u64>,
        miss: Option<String>,
    },
    Checked {
        waypoint: [u8; 32],
        checked: bool,
    },
    Alert {
        name: String,
        stage: usize,
        height: Option<u64>,
    },
    Closed,
}

pub type Progress = Arc<dyn Fn(LiveEvent) + Send + Sync>;

#[derive(Clone)]
struct Plant {
    stage: usize,
    waypoint: [u8; 32],
    height: u64,
    settled: bool,
    accepted: bool,
    path: Option<Vec<u8>>,
    asked: [bool; 3],
}

struct State {
    plants: Mutex<[Option<Plant>; 18]>,
    inflight: AtomicUsize,
    posts: AtomicUsize,
    fetches: AtomicUsize,
}

/// HTTP Bari. `retry_after` is 30 seconds on a live run and shorter in a test.
pub struct HttpBari {
    base: String,
    origin: String,
    retry_after: Duration,
    tip: Arc<dyn Fn() -> Option<HeldTip> + Send + Sync>,
    progress: Progress,
    agent: ureq::Agent,
    state: Arc<State>,
}

impl HttpBari {
    pub fn open(
        bari_url: &str,
        retry_after: Duration,
        tip: Arc<dyn Fn() -> Option<HeldTip> + Send + Sync>,
        progress: Progress,
    ) -> Result<Self, String> {
        let (base, origin) = bari_endpoint(bari_url)?;
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .build();
        Ok(Self {
            base,
            origin,
            retry_after,
            tip,
            progress,
            agent,
            state: Arc::new(State {
                plants: Mutex::new(std::array::from_fn(|_| None)),
                inflight: AtomicUsize::new(0),
                posts: AtomicUsize::new(0),
                fetches: AtomicUsize::new(0),
            }),
        })
    }

    pub fn post_settled(&self, stage: usize) -> bool {
        self.state.plants.lock().expect("plants")[stage]
            .as_ref()
            .is_some_and(|plant| plant.settled)
    }

    pub fn asks_idle(&self) -> bool {
        self.state.inflight.load(Ordering::SeqCst) == 0
    }

    pub fn post_count(&self) -> usize {
        self.state.posts.load(Ordering::SeqCst)
    }

    pub fn fetch_count(&self) -> usize {
        self.state.fetches.load(Ordering::SeqCst)
    }
}

impl BariHub for HttpBari {
    fn enqueue_post(&self, request: PostRequest) {
        if !(1..=17).contains(&request.stage) {
            return;
        }
        {
            let mut plants = self.state.plants.lock().expect("plants");
            if plants[request.stage].is_some() {
                return;
            }
            plants[request.stage] = Some(Plant {
                stage: request.stage,
                waypoint: request.waypoint,
                height: request.beacon_height,
                settled: false,
                accepted: false,
                path: None,
                asked: [false; 3],
            });
        }
        let ctx = PostCtx {
            base: self.base.clone(),
            origin: self.origin.clone(),
            retry: self.retry_after,
            tip: Arc::clone(&self.tip),
            progress: Arc::clone(&self.progress),
            agent: self.agent.clone(),
            state: Arc::clone(&self.state),
            request,
        };
        let _ = std::thread::Builder::new()
            .name(format!("pcoc-bari-{}", ctx.request.stage))
            .spawn(move || post_worker(ctx));
    }

    fn enqueue_header(&self, height: u64) {
        let jobs = due_asks(&self.state, height);
        for job in jobs {
            self.state.inflight.fetch_add(1, Ordering::SeqCst);
            let base = self.base.clone();
            let progress = Arc::clone(&self.progress);
            let agent = self.agent.clone();
            let state = Arc::clone(&self.state);
            let spawned = std::thread::Builder::new()
                .name("pcoc-bari-path".into())
                .spawn(move || {
                    let _guard = Inflight(Arc::clone(&state));
                    ask_worker(base, progress, agent, state, job);
                });
            if spawned.is_err() {
                self.state.inflight.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    fn path(&self, stage: usize) -> Option<Vec<u8>> {
        self.state.plants.lock().expect("plants")[stage]
            .as_ref()
            .and_then(|plant| plant.path.clone())
    }
}

struct AskJob {
    stage: usize,
    waypoint: [u8; 32],
    posted_height: u64,
    at_height: u64,
}

struct Inflight(Arc<State>);

impl Drop for Inflight {
    fn drop(&mut self) {
        self.0.inflight.fetch_sub(1, Ordering::SeqCst);
    }
}

fn due_asks(state: &State, height: u64) -> Vec<AskJob> {
    let mut plants = state.plants.lock().expect("plants");
    let mut jobs = Vec::new();
    for slot in plants.iter_mut().flatten() {
        if slot.stage == 17 || !slot.accepted || slot.path.is_some() {
            continue;
        }
        for (index, offset) in ANCHOR_WINDOW_OFFSETS.iter().enumerate() {
            if slot.height.saturating_add(*offset) == height && !slot.asked[index] {
                slot.asked[index] = true;
                jobs.push(AskJob {
                    stage: slot.stage,
                    waypoint: slot.waypoint,
                    posted_height: slot.height,
                    at_height: height,
                });
                break;
            }
        }
    }
    jobs
}

struct PostCtx {
    base: String,
    origin: String,
    retry: Duration,
    tip: Arc<dyn Fn() -> Option<HeldTip> + Send + Sync>,
    progress: Progress,
    agent: ureq::Agent,
    state: Arc<State>,
    request: PostRequest,
}

fn post_worker(ctx: PostCtx) {
    let PostCtx {
        base,
        origin,
        retry,
        tip,
        progress,
        agent,
        state,
        request,
    } = ctx;
    let Some(held) = tip() else {
        settle(&state, request.stage, false);
        progress(LiveEvent::Alert {
            name: "fee_header_missing".into(),
            stage: request.stage,
            height: Some(request.beacon_height),
        });
        progress(LiveEvent::Posted {
            stage: request.stage,
            waypoint: request.waypoint,
            attempt: 1,
            status: None,
            succeeded: false,
            dropped: true,
        });
        return;
    };
    let Some((nonce, header_hex)) = solve(&origin, &held.header, &request.waypoint) else {
        settle(&state, request.stage, false);
        progress(LiveEvent::Alert {
            name: "fee_header_missing".into(),
            stage: request.stage,
            height: Some(request.beacon_height),
        });
        progress(LiveEvent::Posted {
            stage: request.stage,
            waypoint: request.waypoint,
            attempt: 1,
            status: None,
            succeeded: false,
            dropped: true,
        });
        return;
    };
    let url = format!("{base}/waypoint");
    let first = post_once(&agent, &url, &request.waypoint, &nonce, &header_hex);
    state.posts.fetch_add(1, Ordering::SeqCst);
    let first_ok = first.status == Some(204);
    let retryable = first.status.is_none_or(|code| matches!(code, 502..=504));
    progress(LiveEvent::Posted {
        stage: request.stage,
        waypoint: request.waypoint,
        attempt: 1,
        status: first.status,
        succeeded: first_ok,
        dropped: !first_ok && !retryable,
    });
    if first_ok {
        settle(&state, request.stage, true);
        return;
    }
    if !retryable {
        settle(&state, request.stage, false);
        return;
    }
    std::thread::sleep(retry);
    let second = post_once(&agent, &url, &request.waypoint, &nonce, &header_hex);
    state.posts.fetch_add(1, Ordering::SeqCst);
    let second_ok = second.status == Some(204);
    progress(LiveEvent::Posted {
        stage: request.stage,
        waypoint: request.waypoint,
        attempt: 2,
        status: second.status,
        succeeded: second_ok,
        dropped: !second_ok,
    });
    settle(&state, request.stage, second_ok);
}

fn settle(state: &State, stage: usize, accepted: bool) {
    if let Some(plant) = state.plants.lock().expect("plants")[stage].as_mut() {
        plant.settled = true;
        plant.accepted = accepted;
    }
}

struct Attempt {
    status: Option<u16>,
}

fn post_once(
    agent: &ureq::Agent,
    url: &str,
    body: &[u8; 32],
    nonce: &str,
    header_hex: &str,
) -> Attempt {
    let response = agent
        .post(url)
        .set("Content-Type", "application/octet-stream")
        .set("X-Bari-Hashcash-Nonce", nonce)
        .set("X-Bari-Hashcash-Header", header_hex)
        .send_bytes(body);
    Attempt {
        status: status_of(response),
    }
}

fn status_of(response: Result<ureq::Response, ureq::Error>) -> Option<u16> {
    match response {
        Ok(response) => Some(response.status()),
        Err(ureq::Error::Status(code, _)) => Some(code),
        Err(_) => None,
    }
}

fn solve(origin: &str, header: &[u8; 80], waypoint: &[u8; 32]) -> Option<(String, String)> {
    let hash = header_hash(header);
    let challenge = hashcash_challenge(origin, &hash);
    let (nonce, _) = hashcash_solve(&challenge, waypoint, HASHCASH_DIFFICULTY)?;
    Some((nonce_decimal(nonce), hex_encode(header)))
}

fn ask_worker(
    base: String,
    progress: Progress,
    agent: ureq::Agent,
    state: Arc<State>,
    job: AskJob,
) {
    let leaf = hex_encode(&job.waypoint);
    let url = format!("{base}/path/{leaf}");
    state.fetches.fetch_add(1, Ordering::SeqCst);
    let response = agent.get(&url).call();
    let (status, body) = match response {
        Ok(response) => {
            let status = response.status();
            (Some(status), read_limited(response))
        }
        Err(ureq::Error::Status(code, response)) => (Some(code), read_limited(response)),
        Err(_) => (None, Vec::new()),
    };
    let miss = miss_phrase(&body);
    let checked = status == Some(200) && accept_path(&body, &job.waypoint, job.posted_height);
    if status == Some(200) {
        progress(LiveEvent::Checked {
            waypoint: job.waypoint,
            checked,
        });
    }
    let (block, block_height) = if checked {
        decode_path(&body).map(|body| (Some(body.block_hash), Some(u64::from(body.block_height))))
    } else {
        None
    }
    .unwrap_or((None, None));
    if checked {
        if let Some(plant) = state.plants.lock().expect("plants")[job.stage].as_mut() {
            plant.path = Some(body.clone());
        }
    }
    progress(LiveEvent::Fetch {
        stage: job.stage,
        waypoint: job.waypoint,
        status,
        worked: checked,
        at_height: job.at_height,
        block,
        block_height,
        miss,
    });
}

fn read_limited(response: ureq::Response) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = response.into_reader().take(8192).read_to_end(&mut buf);
    buf
}

fn miss_phrase(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    MISS_PHRASES
        .iter()
        .find(|phrase| text == **phrase)
        .map(|phrase| (*phrase).to_owned())
}

fn accept_path(bytes: &[u8], waypoint: &[u8; 32], posted_height: u64) -> bool {
    let Some(body) = decode_path(bytes) else {
        return false;
    };
    if !validate_for(&body, waypoint) {
        return false;
    }
    let height = u64::from(body.block_height);
    ANCHOR_WINDOW_OFFSETS
        .iter()
        .any(|offset| posted_height.saturating_add(*offset) == height)
}

/// `(base, origin)`. The base has no trailing slash. The origin is RFC 6454.
pub fn bari_endpoint(raw: &str) -> Result<(String, String), String> {
    let raw = raw.trim();
    let Some((scheme, rest)) = raw.split_once("://") else {
        return Err("bari url needs http or https".into());
    };
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("bari url needs http or https".into());
    }
    if rest.contains('@') {
        return Err("bari url has userinfo".into());
    }
    let (authority, extra) = split_authority(rest)?;
    if extra.contains('?') || extra.contains('#') {
        return Err("bari url has a query or fragment".into());
    }
    let path = extra.trim_end_matches('/');
    if !path.is_empty() {
        return Err("bari url has a path".into());
    }
    let (host, port) = split_host_port(authority)?;
    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        return Err("bari url has no host".into());
    }
    let default = if scheme == "https" { 443 } else { 80 };
    let origin = match port {
        Some(port) if port != default => format!("{scheme}://{host}:{port}"),
        Some(_) | None => format!("{scheme}://{host}"),
    };
    Ok((origin.clone(), origin))
}

fn split_authority(rest: &str) -> Result<(&str, &str), String> {
    if rest.starts_with('[') {
        let Some(end) = rest.find(']') else {
            return Err("bari url has a bad host".into());
        };
        let mut authority_end = end + 1;
        if rest[authority_end..].starts_with(':') {
            let port = &rest[authority_end + 1..];
            let port_len = port.find(['/', '?', '#']).unwrap_or(port.len());
            authority_end += 1 + port_len;
        }
        return Ok((&rest[..authority_end], &rest[authority_end..]));
    }
    match rest.find(['/', '?', '#']) {
        Some(index) => Ok((&rest[..index], &rest[index..])),
        None => Ok((rest, "")),
    }
}

fn split_host_port(authority: &str) -> Result<(&str, Option<u16>), String> {
    if authority.starts_with('[') {
        let Some(end) = authority.find(']') else {
            return Err("bari url has a bad host".into());
        };
        let host = &authority[..=end];
        let port = match authority[end + 1..].strip_prefix(':') {
            Some(text) => Some(parse_port(text)?),
            None if authority.len() == end + 1 => None,
            _ => return Err("bari url has a bad host".into()),
        };
        return Ok((host, port));
    }
    if let Some((host, port)) = authority.rsplit_once(':') {
        if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) {
            return Ok((host, Some(parse_port(port)?)));
        }
    }
    Ok((authority, None))
}

fn parse_port(text: &str) -> Result<u16, String> {
    text.parse::<u16>()
        .map_err(|_| "bari url has a bad port".into())
}

/// One JSON progress line, including the newline. Same shape as the rig for
/// `terrain`, `leg`, `interrupt`, and `closed`.
pub fn event_json(event: &LiveEvent) -> String {
    match event {
        LiveEvent::Terrain => "{\"event\":\"terrain\"}\n".to_owned(),
        LiveEvent::Leg {
            stage,
            leg,
            leg_result,
        } => format!(
            "{{\"event\":\"leg\",\"stage\":{stage},\"leg\":{leg},\"leg_result\":\"{}\"}}\n",
            hex_encode(&leg_result.to_le_bytes())
        ),
        LiveEvent::Interrupt { stage, height } => {
            format!("{{\"event\":\"interrupt\",\"stage\":{stage},\"height\":{height}}}\n")
        }
        LiveEvent::Header { height, hash, .. } => format!(
            "{{\"event\":\"header\",\"height\":{height},\"tip\":\"{}\"}}\n",
            hex_encode(hash)
        ),
        LiveEvent::Waypoint { stage, waypoint } => format!(
            "{{\"event\":\"waypoint\",\"stage\":{stage},\"waypoint\":\"{}\"}}\n",
            hex_encode(waypoint)
        ),
        LiveEvent::Posted {
            stage,
            attempt,
            status,
            succeeded,
            dropped,
            ..
        } => {
            let mut line = format!(
                "{{\"event\":\"posted\",\"stage\":{stage},\"attempt\":{attempt},\"succeeded\":{succeeded},\"dropped\":{dropped}"
            );
            if let Some(status) = status {
                line.push_str(&format!(",\"status\":{status}"));
            }
            line.push_str("}\n");
            line
        }
        LiveEvent::Fetch {
            stage,
            status,
            worked,
            ..
        } => {
            let mut line = format!("{{\"event\":\"fetch\",\"stage\":{stage},\"worked\":{worked}");
            if let Some(status) = status {
                line.push_str(&format!(",\"status\":{status}"));
            }
            line.push_str("}\n");
            line
        }
        LiveEvent::Checked { checked, .. } => {
            format!("{{\"event\":\"checked\",\"checked\":{checked}}}\n")
        }
        LiveEvent::Alert { name, stage, height } => match height {
            Some(height) => format!(
                "{{\"event\":\"alert\",\"alert\":\"{name}\",\"stage\":{stage},\"height\":{height}}}\n"
            ),
            None => format!("{{\"event\":\"alert\",\"alert\":\"{name}\",\"stage\":{stage}}}\n"),
        },
        LiveEvent::Closed => "{\"event\":\"closed\"}\n".to_owned(),
    }
}

/// SPEC §13.5 record, one line. Keys are in the order that section lists.
pub fn event_log_line(event: &LiveEvent) -> Option<String> {
    let line = match event {
        LiveEvent::Interrupt { .. }
        | LiveEvent::Header { .. }
        | LiveEvent::Terrain
        | LiveEvent::Leg { .. }
        | LiveEvent::Closed => return None,
        LiveEvent::Waypoint { stage, waypoint } => format!(
            "{{\"event\":\"hike.produce.waypoint\",\"stage\":{stage},\"waypoint\":\"{}\"}}",
            hex_encode(waypoint)
        ),
        LiveEvent::Posted {
            stage,
            waypoint,
            attempt,
            status,
            succeeded,
            dropped,
        } => {
            let posted = status.is_some();
            let mut line = format!(
                "{{\"event\":\"hike.produce.posted\",\"stage\":{stage},\"waypoint\":\"{}\",\"attempt\":{attempt},\"posted\":{posted},\"succeeded\":{succeeded},\"dropped\":{dropped}",
                hex_encode(waypoint)
            );
            if let Some(status) = status {
                line.push_str(&format!(",\"status\":{status}"));
            }
            line.push('}');
            line
        }
        LiveEvent::Checked { waypoint, checked } => format!(
            "{{\"event\":\"hike.produce.checked\",\"waypoint\":\"{}\",\"checked\":{checked}}}",
            hex_encode(waypoint)
        ),
        LiveEvent::Fetch {
            waypoint,
            status,
            worked,
            block,
            block_height,
            miss,
            ..
        } => {
            let mut line = format!(
                "{{\"event\":\"hike.produce.fetch\",\"waypoint\":\"{}\",\"worked\":{worked}",
                hex_encode(waypoint)
            );
            if let Some(status) = status {
                line.push_str(&format!(",\"status\":{status}"));
            }
            if let (Some(block), Some(block_height)) = (block, block_height) {
                line.push_str(&format!(
                    ",\"block\":\"{}\",\"block_height\":{block_height}",
                    hex_encode(block)
                ));
            }
            if let Some(miss) = miss {
                line.push_str(&format!(",\"miss\":\"{miss}\""));
            }
            line.push('}');
            line
        }
        LiveEvent::Alert { name, stage, height } => match height {
            Some(height) => format!(
                "{{\"event\":\"hike.produce.alert\",\"alert\":\"{name}\",\"stage\":{stage},\"height\":{height}}}"
            ),
            None => format!(
                "{{\"event\":\"hike.produce.alert\",\"alert\":\"{name}\",\"stage\":{stage}}}"
            ),
        },
    };
    Some(line)
}

/// A block record. The walk sends this when it learns a header. Not an interrupt line.
pub fn block_log_line(height: u64, hash: &[u8; 32]) -> String {
    format!(
        "{{\"event\":\"hike.produce.block\",\"height\":{height},\"tip\":\"{}\"}}",
        hex_encode(hash)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_drops_default_port_and_a_trailing_slash() {
        let (base, origin) = bari_endpoint("https://bari.groundinglab.io/").unwrap();
        assert_eq!(base, "https://bari.groundinglab.io");
        assert_eq!(origin, base);
        let (base, _) = bari_endpoint("http://127.0.0.1:8080").unwrap();
        assert_eq!(base, "http://127.0.0.1:8080");
        let (base, _) = bari_endpoint("https://Example.COM:443").unwrap();
        assert_eq!(base, "https://example.com");
        let (base, _) = bari_endpoint("http://[::1]:9").unwrap();
        assert_eq!(base, "http://[::1]:9");
        assert!(bari_endpoint("https://user@host").is_err());
        assert!(bari_endpoint("https://host/extra").is_err());
        assert!(bari_endpoint("ftp://host").is_err());
    }
}
