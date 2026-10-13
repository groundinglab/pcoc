//! In-process Bari and portable-index mocks for the reference producer.
//!
//! Observe is the producer's tip-sequence argument. Produce does not GET an
//! HTTP tip. The mock on `127.0.0.1` answers `POST /waypoint`, `GET /path/{leaf}`,
//! and `GET /index/{height}`.
//!
//! main @ 546d24f L426 and L434.

#![allow(dead_code)]

use super::support;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Difficulty for a drive that does not judge the fee. The fee drive posts at
/// [`SPEC_BITS`]. main @ 546d24f L775.
pub const POST_BITS: u32 = 8;
/// The one Hashcash difficulty. main @ 546d24f L775.
pub const SPEC_BITS: u32 = 21;
pub const PUBLIC_KEY: [u8; 32] = [0x11; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserveKind {
    Seed,
    AfterCheckpoint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    Open {
        stage: u32,
    },
    Observe {
        kind: ObserveKind,
        hash: [u8; 32],
        height: u64,
    },
}

#[derive(Clone, Debug)]
pub struct PostHit {
    pub stage: u32,
    pub waypoint: [u8; 32],
    pub tip_hash: [u8; 32],
    pub tip_height: u64,
    pub content_type: String,
    pub nonce: String,
    pub status: u16,
    pub path_status: Option<u16>,
}

#[derive(Clone, Debug)]
pub struct Tip {
    pub hash: [u8; 32],
    pub height: u64,
}

#[derive(Clone, Debug)]
pub struct DrivePlan {
    pub legs: [u32; 18],
    /// Stage indexes whose waypoint POST returns an empty 400.
    pub miss_post: Vec<u32>,
    /// Stage indexes whose path ask returns 404.
    pub miss_path: Vec<u32>,
    /// Stage indexes whose waypoint POST keeps this answer, including a retry.
    pub post_fault: Vec<(u32, PostFault)>,
    pub capture_stderr: bool,
    /// Leading zero bits the drive names for the waypoint POST.
    pub bits: u32,
    /// When set, the path dials stay at the client's own schedule.
    /// Otherwise the one-minute stagger and the second quick GET are confined
    /// away from this drive. They are not zeroed on a drive that judges asks.
    pub judge_asks: bool,
}

#[derive(Clone, Debug)]
pub struct DriveReport {
    pub bytes: Result<Vec<u8>, String>,
    pub posts: Vec<PostHit>,
    pub timeline: Vec<Mark>,
    pub stderr: String,
    pub tips: Vec<Tip>,
    pub legs: [u32; 18],
    /// Every waypoint POST the mock recorded, including a retry.
    pub http_posts: Vec<HttpPost>,
    /// Every `GET /path/{leaf}`. H is the Hashcash tip. The mock holds H+2
    /// until a GET, then H+3, then H+4.
    pub path_asks: Vec<PathAsk>,
}

#[derive(Clone, Debug)]
pub struct HttpPost {
    pub stage: u32,
    pub status: u16,
}

#[derive(Clone, Debug)]
pub struct PathAsk {
    pub stage: u32,
    pub height: u64,
}

pub fn even_legs() -> [u32; 18] {
    [1; 18]
}

/// Eighteen stages, with stage 3 recording two legs before its interrupt.
pub fn two_leg_stage() -> [u32; 18] {
    let mut legs = even_legs();
    legs[3] = 2;
    legs
}

pub fn tips(n: usize) -> Vec<Tip> {
    (0..n)
        .map(|i| {
            let mut hash = [0u8; 32];
            hash[0] = (i >> 8) as u8;
            hash[1] = i as u8;
            hash[2] = 0x5A;
            Tip {
                hash,
                height: 10_000 + (i as u64) * 10,
            }
        })
        .collect()
}

/// `T` for a mock header. Unix seconds, inside `0 … 2^32 − 1`.
pub fn header_time(height: u64) -> i128 {
    1_600_000_000 + i128::from(height) * 10
}

/// One-leaf canonical path. The block hash bytes are the display-order bytes
/// the path body stores. `root` is SHA-256(0x1A ‖ leaf) via `sha2`.
pub fn canonical_path(waypoint: &[u8; 32], block_height: u32) -> Vec<u8> {
    let root = support::merkle_leaf_root(waypoint);
    let txid = support::sha256(&[b"txid".as_slice(), waypoint.as_slice()].concat());
    let block_hash = support::sha256(&[b"blk".as_slice(), waypoint.as_slice()].concat());
    let mut bytes = Vec::with_capacity(146);
    bytes.push(0x01);
    bytes.extend_from_slice(waypoint);
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes.extend_from_slice(&1u32.to_be_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&root);
    bytes.extend_from_slice(&txid);
    bytes.extend_from_slice(&1u32.to_be_bytes());
    bytes.extend_from_slice(&block_hash);
    bytes.extend_from_slice(&block_height.to_be_bytes());
    debug_assert_eq!(bytes.len(), 146);
    bytes
}

/// Hashcash check from the SPEC text, using `sha2`. Difficulty is the `bits`
/// the drive names for the post.
pub fn hashcash_holds(tip: &[u8; 32], waypoint: &[u8; 32], nonce: u64, bits: u32) -> bool {
    let mut challenge_pre = b"bari-hc-v1".to_vec();
    challenge_pre.extend_from_slice(tip);
    let challenge = support::sha256(&challenge_pre);
    let mut body = Vec::with_capacity(72);
    body.extend_from_slice(&challenge);
    body.extend_from_slice(waypoint);
    body.extend_from_slice(&nonce.to_le_bytes());
    let digest = support::sha256(&body);
    leading_zero_bits(&digest) >= bits
}

fn leading_zero_bits(digest: &[u8; 32]) -> u32 {
    let mut bits = 0u32;
    for byte in digest {
        if *byte == 0 {
            bits += 8;
            continue;
        }
        bits += byte.leading_zeros();
        break;
    }
    bits
}

struct Clock {
    tips: Vec<Tip>,
    legs: [u32; 18],
    need_seed: bool,
    stage: u32,
    /// After-checkpoint reads still due at `tips[stage]` before the interrupt.
    reads_left: u32,
    current: Tip,
    timeline: Vec<Mark>,
}

impl Clock {
    fn new(tips: Vec<Tip>, legs: [u32; 18]) -> Self {
        let current = tips[0].clone();
        Self {
            tips,
            legs,
            need_seed: true,
            stage: 0,
            reads_left: 0,
            current,
            timeline: Vec::new(),
        }
    }

    /// Seed, then the stage's own start header until the scripted interrupt.
    /// The interrupt is the first read with a different hash and a greater
    /// height. main @ 546d24f L440 and L442.
    fn observe(&mut self) -> Result<([u8; 32], u64), hike::ProduceFailure> {
        if self.need_seed {
            self.need_seed = false;
            self.stage = 0;
            self.reads_left = self.legs[0];
            return Ok(self.note(ObserveKind::Seed, 0));
        }
        if self.reads_left > 1 {
            self.reads_left -= 1;
            return Ok(self.note(ObserveKind::AfterCheckpoint, self.stage as usize));
        }
        let interrupt = self.stage as usize + 1;
        let noted = self.note(ObserveKind::AfterCheckpoint, interrupt);
        self.stage = self.stage.saturating_add(1);
        if (self.stage as usize) < self.legs.len() {
            self.reads_left = self.legs[self.stage as usize];
        } else {
            self.reads_left = 0;
        }
        Ok(noted)
    }

    fn note(&mut self, kind: ObserveKind, index: usize) -> ([u8; 32], u64) {
        while self.tips.len() <= index {
            let n = self.tips.len();
            let mut hash = [0u8; 32];
            hash[0] = (n >> 8) as u8;
            hash[1] = n as u8;
            hash[2] = 0x5A;
            hash[3] = 0xC3;
            self.tips.push(Tip {
                hash,
                height: 10_000 + (n as u64) * 10,
            });
        }
        let tip = self.tips[index].clone();
        self.current = tip.clone();
        self.timeline.push(Mark::Observe {
            kind,
            hash: tip.hash,
            height: tip.height,
        });
        (tip.hash, tip.height)
    }
}

struct PathServe {
    body: Vec<u8>,
    miss: bool,
}

struct MockState {
    posts: Vec<RawPost>,
    paths: HashMap<String, PathServe>,
    index: HashMap<u64, Option<String>>,
    next_post_miss: bool,
    next_path_miss: bool,
    next_block_height: u32,
    /// Stays set for every POST of this stage, including a retry.
    post_fault: Option<PostFault>,
    open_stage: u32,
    ask_stage: u32,
    /// Hashcash height H. None until that observe. Asks hold H+2, then H+3, then H+4.
    ask_base: Option<u64>,
    /// GETs already recorded for this stage. The held tip does not advance until a GET.
    ask_gets: u32,
    path_asks: Vec<PathAsk>,
}

/// How the mock answers one stage's waypoint POST.
///
/// `Challenge` is the body the producer classifies as a Hashcash tip mismatch.
/// main @ 546d24f L777 and L779.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostFault {
    Http500,
    Http400,
    Dropped,
    Challenge,
}

/// Sentence `post_waypoint` treats as a Hashcash challenge mismatch.
pub const CHALLENGE_BODY: &str = "Hashcash proof invalid for live tip challenge";

#[derive(Clone)]
struct RawPost {
    stage: u32,
    waypoint: [u8; 32],
    content_type: String,
    nonce: String,
    status: u16,
}

struct Shared {
    state: Mutex<MockState>,
    stop: AtomicBool,
}

pub struct IndexMock {
    base: String,
    shared: Arc<Shared>,
    join: Option<JoinHandle<()>>,
    addr: std::net::SocketAddr,
}

impl IndexMock {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("index bind");
        let addr = listener.local_addr().expect("index addr");
        let shared = Arc::new(Shared {
            state: Mutex::new(MockState {
                posts: Vec::new(),
                paths: HashMap::new(),
                index: HashMap::new(),
                next_post_miss: false,
                next_path_miss: false,
                next_block_height: 0,
                post_fault: None,
                open_stage: 0,
                ask_stage: 0,
                ask_base: None,
                ask_gets: 0,
                path_asks: Vec::new(),
            }),
            stop: AtomicBool::new(false),
        });
        let thread_shared = Arc::clone(&shared);
        let join = std::thread::spawn(move || serve(listener, thread_shared));
        Self {
            base: format!("http://{addr}"),
            shared,
            join: Some(join),
            addr,
        }
    }

    pub fn url(&self) -> &str {
        &self.base
    }

    pub fn set_document(&self, height: u64, body: Option<String>) {
        self.shared
            .state
            .lock()
            .expect("index state")
            .index
            .insert(height, body);
    }
}

impl Drop for IndexMock {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct Bari {
    shared: Arc<Shared>,
    join: Option<JoinHandle<()>>,
    addr: std::net::SocketAddr,
    base: String,
}

impl Bari {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bari bind");
        let addr = listener.local_addr().expect("bari addr");
        let shared = Arc::new(Shared {
            state: Mutex::new(MockState {
                posts: Vec::new(),
                paths: HashMap::new(),
                index: HashMap::new(),
                next_post_miss: false,
                next_path_miss: false,
                next_block_height: 0,
                post_fault: None,
                open_stage: 0,
                ask_stage: 0,
                ask_base: None,
                ask_gets: 0,
                path_asks: Vec::new(),
            }),
            stop: AtomicBool::new(false),
        });
        let thread_shared = Arc::clone(&shared);
        let join = std::thread::spawn(move || serve(listener, thread_shared));
        Self {
            shared,
            join: Some(join),
            addr,
            base: format!("http://{addr}"),
        }
    }
}

impl Drop for Bari {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn serve(listener: TcpListener, shared: Arc<Shared>) {
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok((mut stream, _)) = listener.accept() else {
            break;
        };
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let Some(req) = read_http(&mut stream) else {
            continue;
        };
        match respond(&shared, &req) {
            Reply::Http {
                status,
                reason,
                body,
            } => {
                let _ = write_http(&mut stream, &(status, reason, body));
            }
            Reply::Drop => {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }
}

enum Reply {
    Http {
        status: u16,
        reason: &'static str,
        body: Vec<u8>,
    },
    Drop,
}

struct RawReq {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn header<'a>(req: &'a RawReq, name: &str) -> &'a str {
    req.headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .unwrap_or("")
}

fn http(status: u16, reason: &'static str, body: Vec<u8>) -> Reply {
    Reply::Http {
        status,
        reason,
        body,
    }
}

fn respond(shared: &Shared, req: &RawReq) -> Reply {
    let mut state = shared.state.lock().expect("mock state");
    if req.method == "POST" && req.path == "/waypoint" {
        let miss = state.next_post_miss;
        state.next_post_miss = false;
        let path_miss = state.next_path_miss;
        state.next_path_miss = false;
        let fault = state.post_fault;
        if header(req, "content-type") != "application/octet-stream" || req.body.len() != 32 {
            return http(400, "Bad Request", Vec::new());
        }
        let mut waypoint = [0u8; 32];
        waypoint.copy_from_slice(&req.body);
        let status = match fault {
            Some(PostFault::Http500) => 500,
            Some(PostFault::Http400) | Some(PostFault::Challenge) => 400,
            Some(PostFault::Dropped) => 0,
            None if miss => 400,
            None => 204,
        };
        let open_stage = state.open_stage;
        state.posts.push(RawPost {
            stage: open_stage,
            waypoint,
            content_type: header(req, "content-type").to_owned(),
            nonce: header(req, "x-bari-hashcash-nonce").to_owned(),
            status,
        });
        if let Some(fault) = fault {
            return match fault {
                PostFault::Http500 => http(500, "Internal Server Error", Vec::new()),
                PostFault::Http400 => http(400, "Bad Request", Vec::new()),
                PostFault::Challenge => {
                    http(400, "Bad Request", CHALLENGE_BODY.as_bytes().to_vec())
                }
                PostFault::Dropped => Reply::Drop,
            };
        }
        if status == 204 {
            let height = state.next_block_height;
            let leaf = support::hex_natural(&waypoint);
            state.paths.insert(
                format!("/path/{leaf}"),
                PathServe {
                    body: canonical_path(&waypoint, height),
                    miss: path_miss,
                },
            );
            return http(204, "No Content", Vec::new());
        }
        return http(400, "Bad Request", Vec::new());
    }
    if req.method == "GET" && req.path.starts_with("/path/") {
        let height = match state.ask_base {
            Some(h) => h
                .saturating_add(2)
                .saturating_add(u64::from(state.ask_gets)),
            None => 0,
        };
        let ask = PathAsk {
            stage: state.ask_stage,
            height,
        };
        state.ask_gets = state.ask_gets.saturating_add(1);
        state.path_asks.push(ask);
        let Some(serve) = state.paths.get(&req.path) else {
            return http(404, "Not Found", Vec::new());
        };
        if serve.miss {
            return http(404, "Not Found", Vec::new());
        }
        return http(200, "OK", serve.body.clone());
    }
    if let Some(height) = req.path.strip_prefix("/index/") {
        if req.method == "GET" {
            let Ok(height) = height.parse::<u64>() else {
                return http(404, "Not Found", Vec::new());
            };
            return match state.index.get(&height) {
                Some(Some(body)) => http(200, "OK", body.as_bytes().to_vec()),
                _ => http(404, "Not Found", Vec::new()),
            };
        }
    }
    http(404, "Not Found", Vec::new())
}

fn read_http(stream: &mut TcpStream) -> Option<RawReq> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 2048];
    let header_end = loop {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > 1_048_576 {
            return None;
        }
    };
    let head = std::str::from_utf8(&buf[..header_end]).ok()?;
    let mut lines = head.split("\r\n");
    let request = lines.next()?;
    let mut parts = request.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();
    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse().unwrap_or(0);
        }
        headers.push((name.to_owned(), value.to_owned()));
    }
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);
    Some(RawReq {
        method,
        path,
        headers,
        body,
    })
}

fn write_http(stream: &mut TcpStream, response: &(u16, &str, Vec<u8>)) -> std::io::Result<()> {
    let (status, reason, body) = response;
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

/// Path-schedule dials are confined to drives that do not judge the ask.
/// POST retry budgets are never overridden: zeroing them would hide L779.
fn apply_dials(judge_asks: bool) {
    std::env::remove_var("PCOC_WAYPOINT_HTTP_400_WAITS");
    std::env::remove_var("PCOC_PRODUCE_TRANSIENT_WAITS");
    if judge_asks {
        std::env::remove_var("PCOC_PATH_STAGGER_MS");
        std::env::remove_var("PCOC_PATH_ABSENT_ATTEMPTS");
    } else {
        std::env::set_var("PCOC_PATH_STAGGER_MS", "0");
        std::env::set_var("PCOC_PATH_ABSENT_ATTEMPTS", "1");
    }
}

static DRIVE_LOCK: Mutex<()> = Mutex::new(());

/// One `hike::produce` against the mock. The stage-open hook is the test-profile
/// post and path ask: a skipped path is empty bytes, and a post error returns.
pub fn drive(plan: DrivePlan) -> DriveReport {
    let _guard = DRIVE_LOCK.lock().expect("drive lock");
    apply_dials(plan.judge_asks);
    let bits = plan.bits;
    let tips = tips(64);
    let bari = Bari::start();
    let clock = Mutex::new(Clock::new(tips.clone(), plan.legs));
    let posted_tips: Mutex<Vec<(u32, [u8; 32], [u8; 32], u64)>> = Mutex::new(Vec::new());
    let agent = hike::produce_http_agent();
    let bari_url = bari.base.clone();
    let miss_post = plan.miss_post.clone();
    let miss_path = plan.miss_path.clone();
    let post_fault = plan.post_fault.clone();
    let run = || {
        hike::produce(
            PUBLIC_KEY,
            || clock.lock().expect("clock").observe(),
            |waypoint, _beacon_height| {
                let stage = {
                    let mut clock = clock.lock().expect("clock");
                    // Stage 0 is not posted. The first open is W_1.
                    let stage = clock
                        .timeline
                        .iter()
                        .filter(|mark| matches!(mark, Mark::Open { .. }))
                        .count() as u32
                        + 1;
                    clock.timeline.push(Mark::Open { stage });
                    stage
                };
                {
                    let mut state = bari.shared.state.lock().expect("bari");
                    state.next_post_miss = miss_post.contains(&stage);
                    state.next_path_miss = miss_path.contains(&stage);
                    state.post_fault = post_fault
                        .iter()
                        .find(|(g, _)| *g == stage)
                        .map(|(_, fault)| *fault);
                    state.open_stage = stage;
                    state.ask_stage = stage;
                    state.ask_base = None;
                    state.ask_gets = 0;
                    state.next_block_height = {
                        let clock = clock.lock().expect("clock");
                        u32::try_from(clock.current.height + 2).unwrap_or(u32::MAX)
                    };
                }
                let mut hashcash_tip = ([0u8; 32], 0u64);
                let mut saw_hashcash = false;
                let fetched = hike::produce_open_stage(&agent, &bari_url, waypoint, bits, || {
                    let base = {
                        let clock = clock.lock().expect("clock");
                        clock.current.clone()
                    };
                    let (hash, height) = {
                        let mut state = bari.shared.state.lock().expect("bari");
                        if state.ask_base.is_none() {
                            state.ask_base = Some(base.height);
                            state.ask_gets = 0;
                            (base.hash, base.height)
                        } else {
                            let h = state.ask_base.expect("ask base");
                            let held = h
                                .saturating_add(2)
                                .saturating_add(u64::from(state.ask_gets));
                            let mut hash = base.hash;
                            hash[3] = hash[3].wrapping_add(2u8.wrapping_add(state.ask_gets as u8));
                            (hash, held)
                        }
                    };
                    if !saw_hashcash {
                        saw_hashcash = true;
                        hashcash_tip = (hash, height);
                    }
                    Ok((hash, height))
                });
                let (tip_hash, tip_height) = hashcash_tip;
                match fetched {
                    Ok(hike::PathFetch::Body(bytes)) => {
                        posted_tips
                            .lock()
                            .expect("posts")
                            .push((stage, *waypoint, tip_hash, tip_height));
                        Ok(bytes)
                    }
                    Ok(hike::PathFetch::Skipped) => {
                        posted_tips
                            .lock()
                            .expect("posts")
                            .push((stage, *waypoint, tip_hash, tip_height));
                        Ok(Vec::new())
                    }
                    Err(err) => {
                        posted_tips
                            .lock()
                            .expect("posts")
                            .push((stage, *waypoint, tip_hash, tip_height));
                        Err(err)
                    }
                }
            },
        )
    };
    let (produced, stderr) = if plan.capture_stderr {
        capture_stderr(run)
    } else {
        (run(), String::new())
    };
    let (raw_posts, path_asks) = {
        let state = bari.shared.state.lock().expect("bari");
        (state.posts.clone(), state.path_asks.clone())
    };
    let http_posts = raw_posts
        .iter()
        .map(|raw| HttpPost {
            stage: raw.stage,
            status: raw.status,
        })
        .collect();
    let paired = posted_tips.lock().expect("posts");
    let posts = paired
        .iter()
        .map(|(stage, waypoint, tip_hash, tip_height)| {
            let raw = raw_posts
                .iter()
                .find(|raw| raw.stage == *stage && raw.waypoint == *waypoint);
            PostHit {
                stage: *stage,
                waypoint: *waypoint,
                tip_hash: *tip_hash,
                tip_height: *tip_height,
                content_type: raw.map(|raw| raw.content_type.clone()).unwrap_or_default(),
                nonce: raw.map(|raw| raw.nonce.clone()).unwrap_or_default(),
                status: raw.map(|raw| raw.status).unwrap_or(0),
                path_status: None,
            }
        })
        .collect();
    let clock = clock.lock().expect("clock");
    DriveReport {
        bytes: produced.map_err(|err| err.message),
        posts,
        timeline: clock.timeline.clone(),
        stderr,
        tips,
        legs: plan.legs,
        http_posts,
        path_asks,
    }
}

fn capture_stderr<T>(f: impl FnOnce() -> T) -> (T, String) {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().expect("stderr");
    let mut fds = [0i32; 2];
    assert_eq!(unsafe { pipe(fds.as_mut_ptr()) }, 0);
    let saved = unsafe { dup(2) };
    assert!(saved >= 0);
    assert_eq!(unsafe { dup2(fds[1], 2) }, 2);
    unsafe { close(fds[1]) };
    let read_fd = fds[0];
    let reader = std::thread::spawn(move || {
        use std::os::unix::io::FromRawFd;
        let mut file = unsafe { std::fs::File::from_raw_fd(read_fd) };
        let mut buf = String::new();
        let _ = file.read_to_string(&mut buf);
        buf
    });
    let restore = StderrRestore { saved };
    let value = f();
    drop(restore);
    let stderr = reader.join().expect("stderr reader");
    (value, stderr)
}

struct StderrRestore {
    saved: i32,
}

impl Drop for StderrRestore {
    fn drop(&mut self) {
        unsafe {
            dup2(self.saved, 2);
            close(self.saved);
        }
    }
}

extern "C" {
    fn pipe(fds: *mut i32) -> i32;
    fn dup(fd: i32) -> i32;
    fn dup2(oldfd: i32, newfd: i32) -> i32;
    fn close(fd: i32) -> i32;
}

/// Lines Rule A3.4 names. Other stderr from the drive is not part of this record.
pub fn spec_path_stderr(stderr: &str) -> String {
    let mut out = String::new();
    for line in stderr.split_inclusive('\n') {
        if is_spec_path_line(line) {
            out.push_str(line);
        }
    }
    out
}

fn is_spec_path_line(line: &str) -> bool {
    if line == "not fetched by design 17\n" {
        return true;
    }
    let Some(rest) = line.strip_prefix("path_skip ") else {
        return false;
    };
    let Some(g) = rest.strip_suffix('\n') else {
        return false;
    };
    if g.is_empty() || (g.starts_with('0') && g != "0") {
        return false;
    }
    g.bytes().all(|b| b.is_ascii_digit())
}

pub fn verify_at(bytes: &[u8], index_url: &str) -> hike::Measurements {
    let agent = hike::http_agent();
    let url = index_url.to_owned();
    hike::verify(
        bytes,
        hike::AuditDraw::PinnedSeed([0u8; 32]),
        move |height| hike::read_index_beacon(&agent, &url, height).map_err(|_| ()),
        {
            let agent = hike::http_agent();
            let url = index_url.to_owned();
            move |loc| match hike::index_document(&agent, &url, u64::from(loc.block_height)) {
                Ok(document) => hike::proxy_matches(&document, loc),
                Err(_) => Err(()),
            }
        },
    )
}
