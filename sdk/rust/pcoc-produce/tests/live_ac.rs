//! Acceptance tests for the live producer. No real network and no real hike.
//! A test that would dial Bitcoin is not in this file.

use pcoc::profile::Profile;
use pcoc::verify::{EmptyIndex, Verified};
use pcoc_produce::bari::{
    bari_endpoint, BariHub, HttpBari, LiveEvent, PostRequest, Progress, SilentBari,
};
use pcoc_produce::live::{produce_live, write_atomically, LiveRunOpts, LiveStop};
use pcoc_produce::source::{FailClosedSource, HeldTip, TipSource, FAIL_CLOSED};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn profile() -> Profile {
    Profile::conformance_small(32, 64, 65_536, 1).expect("profile")
}

fn tip_at(height: u64) -> HeldTip {
    let mut hash = [0u8; 32];
    hash[..8].copy_from_slice(&height.to_le_bytes());
    let mut header = [0u8; 80];
    header[..8].copy_from_slice(&height.to_le_bytes());
    HeldTip {
        hash,
        height,
        header,
        n_time: 1_700_000_000u32.saturating_add(height as u32),
    }
}

fn noop() -> Progress {
    Arc::new(|_: LiveEvent| {})
}

struct StepSource {
    headers: Vec<HeldTip>,
    n: AtomicUsize,
}

impl TipSource for StepSource {
    fn tip(&self) -> Option<HeldTip> {
        let index = self.n.fetch_add(1, Ordering::SeqCst);
        let index = index.min(self.headers.len() - 1);
        Some(self.headers[index].clone())
    }
    fn failed(&self) -> Option<String> {
        None
    }
    fn drain_learned(&self) -> Vec<HeldTip> {
        Vec::new()
    }
}

struct LockBari {
    net: Arc<Mutex<()>>,
}

impl BariHub for LockBari {
    fn enqueue_post(&self, _request: PostRequest) {
        let net = Arc::clone(&self.net);
        let _ = std::thread::Builder::new()
            .name("blocked-post".into())
            .spawn(move || {
                let _guard = net.lock().expect("net");
            });
    }
    fn enqueue_header(&self, _height: u64) {}
    fn path(&self, _stage: usize) -> Option<Vec<u8>> {
        None
    }
}

#[test]
fn tip_read_does_not_block_a_leg() {
    let headers: Vec<_> = (0..19).map(|i| tip_at(972_685 + i)).collect();
    let source = StepSource {
        headers,
        n: AtomicUsize::new(0),
    };
    let net = Arc::new(Mutex::new(()));
    let guard = net.lock().expect("net");
    let bari = LockBari {
        net: Arc::clone(&net),
    };
    let cancel = AtomicBool::new(false);
    let dir = std::env::temp_dir().join(format!("pcoc-tip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("hike.cbor");
    let profile = profile();
    let key = [0x11u8; 32];
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let opts = LiveRunOpts {
            public_key: &key,
            profile: &profile,
            out: &out,
            cancel: &cancel,
        };
        let started = Instant::now();
        let result = produce_live(&opts, &source, &bari, &noop());
        let _ = tx.send((started.elapsed(), result.is_ok()));
    });
    let message = rx.recv_timeout(Duration::from_secs(3));
    drop(guard);
    let _ = worker.join();
    let (elapsed, ok) = message.expect("the walk blocked on the peer lock");
    assert!(ok);
    assert!(
        elapsed < Duration::from_secs(2),
        "walk took {elapsed:?} while a peer lock was held"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fail_closed_source_writes_no_object() {
    let dir = std::env::temp_dir().join(format!("pcoc-closed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("hike.cbor");
    let cancel = AtomicBool::new(false);
    let profile = profile();
    let key = [0x22u8; 32];
    let opts = LiveRunOpts {
        public_key: &key,
        profile: &profile,
        out: &out,
        cancel: &cancel,
    };
    let err = produce_live(
        &opts,
        &FailClosedSource::new(FAIL_CLOSED),
        &SilentBari,
        &noop(),
    )
    .expect_err("fail closed");
    match err {
        LiveStop::Failed(message) => assert_eq!(message, FAIL_CLOSED),
        LiveStop::Cancelled => panic!("cancelled"),
    }
    assert!(!out.exists());
    assert!(!PathBuf::from(format!("{}.checkpoint", out.display())).exists());
    assert!(!PathBuf::from(format!("{}.events", out.display())).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

struct Hit {
    method: String,
    path: String,
    body: Vec<u8>,
    nonce: String,
    header: String,
    at: Instant,
}

type Script = Arc<dyn Fn(&Hit, &[Hit]) -> (u16, Vec<u8>) + Send + Sync>;

fn serve(script: Script) -> (String, Arc<Mutex<Vec<Hit>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&hits);
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let Ok(hit) = read_request(&mut stream) else {
                continue;
            };
            let response = {
                let mut guard = record.lock().expect("hits");
                let response = script(&hit, &guard);
                guard.push(hit);
                response
            };
            let (status, body) = response;
            let reason = match status {
                200 => "OK",
                204 => "No Content",
                402 => "Payment Required",
                503 => "Service Unavailable",
                _ => "No",
            };
            let head = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    (format!("http://127.0.0.1:{port}"), hits)
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Hit> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte)?;
        if n == 0 {
            break;
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8192 {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    let request = lines.next().unwrap_or("");
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("").to_owned();
    let path = parts.next().unwrap_or("").to_owned();
    let mut length = 0usize;
    let mut nonce = String::new();
    let mut header = String::new();
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
        if let Some(value) = line.split_once(':') {
            if value.0.eq_ignore_ascii_case("X-Bari-Hashcash-Nonce") {
                nonce = value.1.trim().to_owned();
            }
            if value.0.eq_ignore_ascii_case("X-Bari-Hashcash-Header") {
                header = value.1.trim().to_owned();
            }
        }
    }
    let mut body = vec![0u8; length];
    if length > 0 {
        stream.read_exact(&mut body)?;
    }
    Ok(Hit {
        method,
        path,
        body,
        nonce,
        header,
        at: Instant::now(),
    })
}

#[test]
fn bari_post_is_async_and_retries_once_after_backoff() {
    let (url, hits) = serve(Arc::new(|hit, prior| {
        if hit.method == "POST" {
            let posts = prior.iter().filter(|item| item.method == "POST").count();
            if posts == 0 {
                return (503, Vec::new());
            }
            return (204, Vec::new());
        }
        (404, b"unknown".to_vec())
    }));
    let (_base, origin) = bari_endpoint(&url).unwrap();
    assert!(origin.starts_with("http://127.0.0.1:"));
    let held = tip_at(972_685);
    let tip = Arc::new(move || Some(held.clone()));
    let progress = noop();
    let bari = HttpBari::open(&url, Duration::from_millis(40), tip, progress).unwrap();
    let started = Instant::now();
    bari.enqueue_post(PostRequest {
        stage: 1,
        waypoint: [0xab; 32],
        beacon_height: 972_686,
    });
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "enqueue waited for the post"
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline && bari.post_count() < 2 {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        bari.post_settled(1),
        "posts {} hits {}",
        bari.post_count(),
        hits.lock().unwrap().len()
    );
    let guard = hits.lock().unwrap();
    let posts: Vec<_> = guard.iter().filter(|hit| hit.method == "POST").collect();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].path, "/waypoint");
    assert_eq!(posts[0].body, [0xab; 32]);
    assert_eq!(posts[0].nonce, posts[1].nonce);
    assert_eq!(posts[0].header, posts[1].header);
    assert!(!posts[0].nonce.is_empty());
    assert_eq!(posts[0].header.len(), 160);
    assert!(posts[1].at.duration_since(posts[0].at) >= Duration::from_millis(30));
}

#[test]
fn a_402_is_not_retried() {
    let (url, hits) = serve(Arc::new(|_, _| (402, Vec::new())));
    let held = tip_at(10);
    let tip = Arc::new(move || Some(held.clone()));
    let bari = HttpBari::open(&url, Duration::from_millis(40), tip, noop()).unwrap();
    bari.enqueue_post(PostRequest {
        stage: 1,
        waypoint: [0xcd; 32],
        beacon_height: 11,
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline && !bari.post_settled(1) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(bari.post_settled(1));
    std::thread::sleep(Duration::from_millis(80));
    let posts = hits
        .lock()
        .unwrap()
        .iter()
        .filter(|hit| hit.method == "POST")
        .count();
    assert_eq!(posts, 1);
    assert_eq!(bari.post_count(), 1);
}

/// Advances one header per `tip` call and queues the next one for the following drain.
struct StepLearn {
    headers: Vec<HeldTip>,
    n: AtomicUsize,
    learned: Mutex<Vec<HeldTip>>,
}

impl TipSource for StepLearn {
    fn tip(&self) -> Option<HeldTip> {
        let index = self.n.fetch_add(1, Ordering::SeqCst);
        let index = index.min(self.headers.len() - 1);
        if index + 1 < self.headers.len() {
            self.learned
                .lock()
                .unwrap()
                .push(self.headers[index + 1].clone());
        }
        Some(self.headers[index].clone())
    }
    fn failed(&self) -> Option<String> {
        None
    }
    fn drain_learned(&self) -> Vec<HeldTip> {
        std::mem::take(&mut *self.learned.lock().unwrap())
    }
}

struct MemPlant {
    waypoint: [u8; 32],
    height: u64,
    accepted: bool,
    path: Option<Vec<u8>>,
}

struct MemBari {
    plants: Mutex<[Option<MemPlant>; 18]>,
    posts: AtomicUsize,
}

impl BariHub for MemBari {
    fn enqueue_post(&self, request: PostRequest) {
        self.posts.fetch_add(1, Ordering::SeqCst);
        self.plants.lock().unwrap()[request.stage] = Some(MemPlant {
            waypoint: request.waypoint,
            height: request.beacon_height,
            accepted: true,
            path: None,
        });
    }
    fn enqueue_header(&self, height: u64) {
        let mut plants = self.plants.lock().unwrap();
        for plant in plants.iter_mut().flatten() {
            if plant.height == 0 || !plant.accepted || plant.path.is_some() {
                continue;
            }
            if plant.height.saturating_add(2) == height {
                let block = u32::try_from(height).unwrap_or(u32::MAX);
                plant.path = Some(one_leaf_path(&plant.waypoint, block));
            }
        }
    }
    fn path(&self, stage: usize) -> Option<Vec<u8>> {
        self.plants.lock().unwrap()[stage]
            .as_ref()
            .and_then(|plant| plant.path.clone())
    }
}

fn one_leaf_path(waypoint: &[u8; 32], block_height: u32) -> Vec<u8> {
    let root = pcoc::hash::sha256_parts(&[&[0x1au8], waypoint]);
    let mut out = Vec::with_capacity(146);
    out.push(0x01);
    out.extend_from_slice(waypoint);
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&1u32.to_be_bytes());
    out.push(0);
    out.extend_from_slice(&root);
    out.extend_from_slice(&[7u8; 32]);
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&[9u8; 32]);
    out.extend_from_slice(&block_height.to_be_bytes());
    out
}

#[test]
fn conformance_small_object_verifies_against_mocks() {
    const BASE: u64 = 972_685;
    let headers: Vec<_> = (0..19).map(|i| tip_at(BASE + i)).collect();
    let source = StepLearn {
        headers,
        n: AtomicUsize::new(0),
        learned: Mutex::new(Vec::new()),
    };
    let bari = MemBari {
        plants: Mutex::new(std::array::from_fn(|_| None)),
        posts: AtomicUsize::new(0),
    };
    let dir = std::env::temp_dir().join(format!("pcoc-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("hike.cbor");
    let profile = profile();
    let key = [0x44u8; 32];
    let cancel = AtomicBool::new(false);
    let opts = LiveRunOpts {
        public_key: &key,
        profile: &profile,
        out: &out,
        cancel: &cancel,
    };
    let product = produce_live(&opts, &source, &bari, &noop()).expect("produce");
    write_atomically(&out, &product.bytes).unwrap();
    let obj = pcoc::decode_object(&product.bytes).expect("decode");
    assert_eq!(obj.kind, pcoc::constants::KIND);
    assert!(obj.stages.iter().all(|stage| stage.beacon_height >= BASE));
    assert!(obj.last_interrupt_height >= BASE);
    let anchored = obj.stages[1..=16]
        .iter()
        .filter(|stage| stage.path.as_ref().is_some_and(|path| !path.is_empty()))
        .count();
    assert!(anchored >= 6, "anchored {anchored}");
    assert!(obj.stages[17]
        .path
        .as_ref()
        .is_some_and(|path| path.is_empty()));
    assert!(product.stderr.ends_with("not fetched by design 17\n"));
    assert!(
        !product.stderr.contains("path_skip 1\n"),
        "{}",
        product.stderr
    );
    let mut index = EmptyIndex;
    match pcoc::verify(&product.bytes, &profile, &mut index) {
        Verified::Record(text) => assert!(text.contains("\"shape\":\"match\""), "{text}"),
        other => panic!("verifier did not return a record: {other:?}"),
    }
    assert_eq!(bari.posts.load(Ordering::SeqCst), 17);
    let checkpoint = std::fs::read_to_string(format!("{}.checkpoint", out.display())).unwrap();
    assert!(checkpoint.contains("\"resume\":false"));
    assert!(checkpoint.contains("\"closed\":true"));
    let _ = std::fs::remove_dir_all(&dir);
}
