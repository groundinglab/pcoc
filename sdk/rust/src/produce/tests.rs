//! Rule I4. The few runs that prove AC2.1–AC2.6. Clocks are mock clocks.

use super::bari_client::{BariClient, BariResponse, PathGet, WaypointPost};
use super::clock::{Clock, MockClock};
use super::events;
use super::header::{HeaderSource, Observed, Source, TipRead};
use super::{events_path, produce_with, ProduceConfig, Produced, Solver};
use crate::constants::KIND;
use crate::hash::sha256;
use crate::hexutil::{hex_decode_exact, hex_encode};
use crate::hike::produce_hike;
use crate::jsonutil::unknown_kind_record;
use crate::object::{decode_object, encode_object};
use crate::profile::Profile;
use crate::terrain::terrain_from_seed;
use crate::verify::{verify, EmptyIndex, Verified};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const GENESIS_HEX: &str = "0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c";
const BARI: &str = "https://bari.example";

fn pk_a() -> [u8; 32] {
    hex_decode_exact("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9").unwrap()
}

fn genesis() -> [u8; 80] {
    hex_decode_exact(GENESIS_HEX).unwrap()
}

fn beacon_named(name: &str) -> [u8; 32] {
    sha256(format!("pcoc-pack-2.2/{name}").as_bytes())
}

fn obs(natural: [u8; 32], height: u64) -> Observed {
    Observed {
        natural,
        height,
        raw: None,
    }
}

fn chain(seed_h: u64) -> Vec<Observed> {
    let mut v = vec![obs(beacon_named("beacon/seed"), seed_h)];
    for k in 0..18 {
        v.push(obs(
            beacon_named(&format!("beacon/{k}")),
            seed_h + 1 + k as u64,
        ));
    }
    v
}

fn interrupts(seed_h: u64) -> [([u8; 32], u64); 18] {
    let mut out = [([0u8; 32], 0u64); 18];
    for k in 0..18 {
        out[k] = (beacon_named(&format!("beacon/{k}")), seed_h + 1 + k as u64);
    }
    out
}

fn tiny() -> Profile {
    Profile::conformance_small(32, 64, 4096, 20).unwrap()
}

fn heavy() -> Profile {
    Profile::conformance_small(32, 64, 65536, 80_000).unwrap()
}

fn tmpdir(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "pcoc-p2-{}-{}-{}",
        std::process::id(),
        label,
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn fast_solver() -> Solver {
    Arc::new(|_challenge, _waypoint| Some((0, [0u8; 32])))
}

fn ns(d: Duration) -> u64 {
    d.as_nanos().min(u128::from(u64::MAX)) as u64
}

struct Mem {
    chain: Vec<Observed>,
    legs: [u32; 18],
    served_seed: bool,
    stage: usize,
    leg_in_stage: u32,
    down: u8,
    fail_remaining: u32,
    reorg: Option<(usize, u32, Observed)>,
    learned: VecDeque<Observed>,
    wake: bool,
    stall_wait: bool,
    auto_learn: bool,
    hold_i17: bool,
    tips: u64,
    watch_height: Option<u64>,
    watched_at: Option<Instant>,
}

struct MemSource {
    inner: Mutex<Mem>,
    cv: Condvar,
}

impl MemSource {
    fn new(chain: Vec<Observed>, legs: [u32; 18]) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Mem {
                chain,
                legs,
                served_seed: false,
                stage: 0,
                leg_in_stage: 0,
                down: 0,
                fail_remaining: 0,
                reorg: None,
                learned: VecDeque::new(),
                wake: false,
                stall_wait: false,
                auto_learn: true,
                hold_i17: false,
                tips: 0,
                watch_height: Some(1003),
                watched_at: None,
            }),
            cv: Condvar::new(),
        })
    }

    fn tips(&self) -> u64 {
        self.inner.lock().unwrap().tips
    }

    fn set_down(&self, down: u8) {
        self.inner.lock().unwrap().down = down;
    }

    fn set_hold_i17(&self, hold: bool) {
        self.inner.lock().unwrap().hold_i17 = hold;
    }

    fn watched_at(&self) -> Option<Instant> {
        self.inner.lock().unwrap().watched_at
    }

    fn preload(&self, headers: Vec<Observed>) {
        let mut g = self.inner.lock().unwrap();
        for h in headers {
            g.learned.push_back(h);
        }
        g.wake = true;
        self.cv.notify_all();
    }
}

impl HeaderSource for MemSource {
    fn tip(&self) -> TipRead {
        let mut g = self.inner.lock().unwrap();
        g.tips += 1;
        if !g.served_seed {
            g.served_seed = true;
            let obs = g.chain[0].clone();
            learn(&mut g, &self.cv, obs.clone());
            return TipRead::Ready(obs);
        }
        if g.down == 1 {
            return TipRead::Failed;
        }
        if g.down == 2 {
            return TipRead::Empty;
        }
        if g.fail_remaining > 0 {
            g.fail_remaining -= 1;
            return TipRead::Failed;
        }
        if let Some((stage, at, header)) = g.reorg.clone() {
            if g.stage == stage && g.leg_in_stage + 1 == at {
                g.reorg = None;
                g.leg_in_stage += 1;
                learn(&mut g, &self.cv, header.clone());
                return TipRead::Ready(header);
            }
        }
        if g.stage >= 18 {
            return TipRead::Failed;
        }
        g.leg_in_stage += 1;
        let stage = g.stage;
        let n = g.legs[stage];
        if g.leg_in_stage < n {
            return TipRead::Ready(g.chain[stage].clone());
        }
        if stage == 17 && g.hold_i17 {
            return TipRead::Ready(g.chain[stage].clone());
        }
        let header = g.chain[stage + 1].clone();
        g.stage += 1;
        g.leg_in_stage = 0;
        learn(&mut g, &self.cv, header.clone());
        TipRead::Ready(header)
    }

    fn fee_header(&self) -> Option<[u8; 80]> {
        Some(genesis())
    }

    fn take_learned(&self) -> Vec<Observed> {
        let mut g = self.inner.lock().unwrap();
        g.learned.drain(..).collect()
    }

    fn wait_learned(&self) {
        let mut g = self.inner.lock().unwrap();
        loop {
            if g.wake {
                g.wake = false;
                return;
            }
            if !g.stall_wait && !g.learned.is_empty() {
                return;
            }
            g = self.cv.wait(g).unwrap();
        }
    }

    fn wake(&self) {
        let mut g = self.inner.lock().unwrap();
        g.wake = true;
        self.cv.notify_all();
    }
}

fn learn(g: &mut Mem, cv: &Condvar, header: Observed) {
    if g.watch_height == Some(header.height) && g.watched_at.is_none() {
        g.watched_at = Some(Instant::now());
    }
    if !g.auto_learn {
        return;
    }
    g.learned.push_back(header);
    if !g.stall_wait {
        g.wake = true;
        cv.notify_all();
    }
}

#[derive(Clone)]
enum One {
    Status(u16),
    NoStatus,
    Hang,
}

struct PostRec {
    body: [u8; 32],
    nonce: String,
    fee_hex: String,
    url: String,
    content_type: String,
    at_ms: u64,
}

struct GetRec {
    leaf: String,
    at: Instant,
}

struct BariInner {
    posts: Vec<PostRec>,
    gets: Vec<GetRec>,
    script: Vec<One>,
    default: One,
}

struct ScriptBari {
    inner: Mutex<BariInner>,
    clock: MockClock,
    cancel: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    blocked: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    gap: AtomicBool,
}

impl ScriptBari {
    fn new(clock: MockClock, cancel: Arc<AtomicBool>, default: One) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(BariInner {
                posts: Vec::new(),
                gets: Vec::new(),
                script: Vec::new(),
                default,
            }),
            clock,
            cancel,
            release: Arc::new(AtomicBool::new(false)),
            blocked: Arc::new(AtomicBool::new(false)),
            finished: Arc::new(AtomicBool::new(false)),
            gap: AtomicBool::new(false),
        })
    }

    fn post_snapshot(&self) -> Vec<PostRec> {
        let g = self.inner.lock().unwrap();
        g.posts
            .iter()
            .map(|p| PostRec {
                body: p.body,
                nonce: p.nonce.clone(),
                fee_hex: p.fee_hex.clone(),
                url: p.url.clone(),
                content_type: p.content_type.clone(),
                at_ms: p.at_ms,
            })
            .collect()
    }

    fn gets(&self) -> Vec<GetRec> {
        let g = self.inner.lock().unwrap();
        g.gets
            .iter()
            .map(|r| GetRec {
                leaf: r.leaf.clone(),
                at: r.at,
            })
            .collect()
    }
}

impl BariClient for ScriptBari {
    fn post_waypoint(&self, post: &WaypointPost) -> BariResponse {
        let mode = {
            let mut g = self.inner.lock().unwrap();
            let i = g.posts.len();
            let mode = g.script.get(i).cloned().unwrap_or_else(|| g.default.clone());
            g.posts.push(PostRec {
                body: post.body,
                nonce: post.nonce.clone(),
                fee_hex: post.fee_header_hex.clone(),
                url: post.url.clone(),
                content_type: post.content_type.to_string(),
                at_ms: self.clock.now_ms(),
            });
            mode
        };
        match mode {
            One::Hang => {
                while !self.cancel.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(20));
                }
                BariResponse::NoStatus
            }
            One::NoStatus => BariResponse::NoStatus,
            One::Status(status) => BariResponse::Http {
                status,
                body: Vec::new(),
            },
        }
    }

    fn get_path(&self, ask: &PathGet) -> BariResponse {
        let n = {
            let mut g = self.inner.lock().unwrap();
            g.gets.push(GetRec {
                leaf: ask.leaf_hex.clone(),
                at: Instant::now(),
            });
            g.gets.len()
        };
        if self.gap.load(Ordering::SeqCst) && n == 2 {
            self.blocked.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) && !self.cancel.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(20));
            }
            self.finished.store(true, Ordering::SeqCst);
            let wp = hex_decode_exact::<32>(&ask.leaf_hex).unwrap_or([0; 32]);
            return BariResponse::Http {
                status: 200,
                body: path_for(&wp, 1004),
            };
        }
        if self.gap.load(Ordering::SeqCst) && n == 1 {
            let wp = hex_decode_exact::<32>(&ask.leaf_hex).unwrap_or([0; 32]);
            return BariResponse::Http {
                status: 200,
                body: path_for(&wp, 1003),
            };
        }
        BariResponse::Http {
            status: 404,
            body: b"unknown".to_vec(),
        }
    }
}

fn path_for(waypoint: &[u8; 32], height: u32) -> Vec<u8> {
    let tag = [0x1au8];
    let root = crate::hash::sha256_parts(&[&tag, waypoint]);
    let mut body = Vec::with_capacity(146);
    body.push(0x01);
    body.extend_from_slice(waypoint);
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&1u32.to_be_bytes());
    body.push(0);
    body.extend_from_slice(&root);
    body.extend_from_slice(&[0u8; 32]);
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&[7u8; 32]);
    body.extend_from_slice(&height.to_be_bytes());
    body
}

fn run(
    profile: Profile,
    legs: [u32; 18],
    source_setup: impl FnOnce(&MemSource),
    bari_setup: impl FnOnce(&ScriptBari),
    live: bool,
    path: Option<PathBuf>,
    solver: Solver,
) -> (Produced, Arc<MemSource>, Arc<ScriptBari>, MockClock) {
    let clock = MockClock::new(0);
    let cancel = Arc::new(AtomicBool::new(false));
    let source = MemSource::new(chain(1000), legs);
    source_setup(&source);
    let bari = ScriptBari::new(clock.clone(), Arc::clone(&cancel), One::Status(204));
    bari_setup(&bari);
    let cfg = ProduceConfig {
        profile,
        public_key: pk_a(),
        bari_url: BARI.into(),
        source: source.clone() as Source,
        client: bari.clone(),
        clock: Arc::new(clock.clone()),
        object_path: path,
        live,
        cancel,
    };
    let produced = produce_with(cfg, solver).expect("object");
    (produced, source, bari, clock)
}

#[test]
fn ask_and_walk_name_no_timer() {
    let ask = include_str!("ask.rs");
    let drive = include_str!("drive.rs");
    for (name, src) in [("ask.rs", ask), ("drive.rs", drive)] {
        for token in ["thread::sleep", "wait_until", "Duration"] {
            assert!(!src.contains(token), "{name} contains {token}");
        }
    }
}

#[test]
fn object_matches_fixture_hike_and_kind_is_current() {
    let profile = tiny();
    let legs = [1u32; 18];
    let (produced, _, _, _) = run(profile.clone(), legs, |_| {}, |_| {}, false, None, fast_solver());
    let pk = pk_a();
    let seed = beacon_named("beacon/seed");
    let (terrain, _, _) = terrain_from_seed(&pk, &seed, &profile);
    let (obj, _, _) = produce_hike(&profile, &pk, &seed, 1000, &interrupts(1000), &legs, &terrain);
    let expect = encode_object(&obj);
    assert_eq!(produced.bytes, expect);
    let decoded = decode_object(&produced.bytes).unwrap();
    assert_eq!(decoded.kind, KIND);
    assert!(produced.stderr_text.ends_with("not fetched by design 17\n"));
    assert!(produced.stderr_text.contains("path_skip 1\n"));
    drop(produced);

    let labels = [
        "",
        "satoshi-hike-v2.0-object",
        "v2.1-pcoc-object",
        "v2.2-pcoc-objecT",
    ];
    for label in labels {
        let bytes = crate::cbor::encode_map(&[("kind".into(), crate::cbor::encode_text(label))]);
        let mut draw = || None;
        match verify(&bytes, &profile, &mut EmptyIndex, Some(&mut draw)) {
            Verified::UnknownKind(rec) => assert_eq!(rec, unknown_kind_record(label)),
            other => panic!("label {label:?} -> {other:?}"),
        }
    }
}

#[test]
fn nothing_pauses_a_leg_and_rates() {
    let profile = heavy();
    let legs = [4u32; 18];
    let dir_ok = tmpdir("healthy");
    let (healthy, source, bari, _) = run(
        profile.clone(),
        legs,
        |_| {},
        |_| {},
        true,
        Some(dir_ok.join("hike")),
        fast_solver(),
    );
    let ask_at = bari.gets().first().map(|g| g.at);
    let learned_at = source.watched_at();
    let (learned_at, ask_at) = (learned_at.expect("due header"), ask_at.expect("ask"));
    let lag = ask_at.saturating_duration_since(learned_at);
    eprintln!("P2-MEASURE ask_lag_us={}", lag.as_micros());
    assert!(lag < Duration::from_secs(1), "ask lag {lag:?}");
    assert!(healthy.asks >= 1);

    let pk = pk_a();
    let seed = beacon_named("beacon/seed");
    let (terrain, _, _) = terrain_from_seed(&pk, &seed, &profile);
    let core_at = Instant::now();
    let _ = produce_hike(&profile, &pk, &seed, 1000, &interrupts(1000), &legs, &terrain);
    let core_ns = ns(core_at.elapsed());
    eprintln!(
        "P2-MEASURE legs={} asks={} walk_ns={} leg_ns={} core_ns={} walk_over_core={:.3}",
        healthy.legs,
        healthy.asks,
        healthy.walk_ns,
        healthy.leg_ns,
        core_ns,
        healthy.walk_ns as f64 / core_ns as f64
    );
    assert_eq!(healthy.legs, 72);
    assert!(
        healthy.walk_ns as f64 >= core_ns as f64 * 0.85,
        "walk faster than the core hike: {} vs {core_ns}",
        healthy.walk_ns
    );
    let span = healthy.walk_ns as f64 / 1e9;
    let asks = healthy.asks as f64;
    eprintln!(
        "P2-MEASURE legs_per_s_now={:.4} legs_per_s_plus_15s={:.6} legs_per_s_plus_30s={:.6} legs_per_s_plus_45s={:.6}",
        healthy.legs as f64 / span,
        healthy.legs as f64 / (span + 15.0 * asks),
        healthy.legs as f64 / (span + 30.0 * asks),
        healthy.legs as f64 / (span + 45.0 * asks)
    );
    assert!(healthy.walk_ns < healthy.asks.saturating_mul(15_000_000_000));
    let healthy_ns = healthy.walk_ns;
    drop(healthy);

    let dir_bad = tmpdir("unhealthy");
    std::fs::create_dir(dir_bad.join("hike.events")).unwrap();
    let slow: Solver = Arc::new(|_, _| {
        thread::sleep(Duration::from_millis(80));
        Some((0, [0u8; 32]))
    });
    let (unhealthy, _, _, _) = run(
        profile,
        legs,
        |src| {
            let mut g = src.inner.lock().unwrap();
            g.stall_wait = true;
            g.auto_learn = false;
        },
        |bari| {
            bari.inner.lock().unwrap().script = vec![One::Hang, One::Status(503)];
        },
        true,
        Some(dir_bad.join("hike")),
        slow,
    );
    let ratio = unhealthy.walk_ns as f64 / healthy_ns as f64;
    eprintln!(
        "P2-MEASURE healthy_walk_ns={healthy_ns} unhealthy_walk_ns={} ratio={ratio:.3}",
        unhealthy.walk_ns
    );
    assert_eq!(unhealthy.legs, 72);
    assert!(std::fs::metadata(dir_bad.join("hike")).unwrap().is_file());
    let hi = healthy_ns.max(unhealthy.walk_ns) as f64;
    let lo = healthy_ns.min(unhealthy.walk_ns) as f64;
    assert!(hi <= lo * 1.25, "walk diverged healthy {healthy_ns} unhealthy {}", unhealthy.walk_ns);
    drop(unhealthy);
    let _ = std::fs::remove_dir_all(&dir_ok);
    let _ = std::fs::remove_dir_all(&dir_bad);
}

#[test]
fn transport_resend_is_one_and_402_is_final() {
    for (label, default) in [
        ("nostatus", One::NoStatus),
        ("502", One::Status(502)),
        ("503", One::Status(503)),
        ("504", One::Status(504)),
    ] {
        transport_case(label, default, 2, false);
    }
    transport_case("402", One::Status(402), 1, true);
}

fn transport_case(label: &str, default: One, attempts: usize, answered: bool) {
    let dir = tmpdir(label);
    let path = dir.join("hike");
    let clock = MockClock::new(0);
    let cancel = Arc::new(AtomicBool::new(false));
    let source = MemSource::new(chain(1000), [1u32; 18]);
    let bari = ScriptBari::new(clock.clone(), Arc::clone(&cancel), default);
    let cfg = ProduceConfig {
        profile: tiny(),
        public_key: pk_a(),
        bari_url: BARI.into(),
        source: source.clone() as Source,
        client: bari.clone(),
        clock: Arc::new(clock.clone()),
        object_path: Some(path.clone()),
        live: true,
        cancel,
    };
    let produced = produce_with(cfg, fast_solver()).expect("object");
    if attempts == 2 {
        pump(&clock, &bari);
    } else {
        wait_posts(&bari, 17);
        clock.advance(30_000);
        thread::sleep(Duration::from_millis(50));
    }
    let posts = bari.post_snapshot();
    let mut by: std::collections::BTreeMap<[u8; 32], Vec<PostRec>> = std::collections::BTreeMap::new();
    for post in posts {
        assert_eq!(post.content_type, "application/octet-stream");
        assert_eq!(post.url, format!("{BARI}/waypoint"));
        by.entry(post.body).or_default().push(post);
    }
    assert_eq!(by.len(), 17, "{label}");
    for group in by.values_mut() {
        group.sort_by_key(|p| p.at_ms);
        assert_eq!(group.len(), attempts, "{label}");
        if attempts == 2 {
            assert_eq!(group[1].at_ms - group[0].at_ms, 30_000, "{label}");
            assert_eq!(group[0].nonce, group[1].nonce);
            assert_eq!(group[0].fee_hex, group[1].fee_hex);
        }
    }
    let stderr = produced.stderr_text.clone();
    drop(produced);
    let log = std::fs::read_to_string(events_path(&path)).unwrap();
    let obj = decode_object(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(obj.last_interrupt_height, 1018);
    for g in 1..=17 {
        let wp = obj.stages[g].waypoint;
        if answered {
            let line = events::posted_line(g, &wp, 1, true, false, true, Some(402));
            assert!(log.contains(line.trim_end()), "{label} missing {line}");
        } else {
            let first = events::posted_line(g, &wp, 1, false, false, false, None);
            let second = events::posted_line(g, &wp, 2, false, false, true, None);
            assert!(log.contains(first.trim_end()), "{label} {g} first");
            assert!(log.contains(second.trim_end()), "{label} {g} second");
        }
    }
    assert!(stderr.ends_with("not fetched by design 17\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

fn wait_posts(bari: &ScriptBari, n: usize) {
    let start = Instant::now();
    while bari.post_snapshot().len() < n {
        assert!(start.elapsed() < Duration::from_secs(3), "posts {}", bari.post_snapshot().len());
        thread::sleep(Duration::from_millis(5));
    }
}

fn pump(clock: &MockClock, bari: &ScriptBari) {
    let start = Instant::now();
    loop {
        let posts = bari.post_snapshot();
        let mut counts: std::collections::BTreeMap<[u8; 32], usize> = std::collections::BTreeMap::new();
        for post in &posts {
            *counts.entry(post.body).or_default() += 1;
        }
        if counts.len() == 17 && counts.values().all(|n| *n >= 2) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(4), "pump {}", posts.len());
        let wait_for = Instant::now();
        while clock.waiters() == 0 && wait_for.elapsed() < Duration::from_millis(300) {
            thread::sleep(Duration::from_millis(5));
        }
        if clock.waiters() > 0 {
            clock.advance(30_000);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn clock_failures_and_reorg_still_close() {
    let dir = tmpdir("failread");
    let path = dir.join("hike");
    let (produced, _, _, _) = run(
        tiny(),
        [1u32; 18],
        |src| src.inner.lock().unwrap().fail_remaining = 5,
        |_| {},
        true,
        Some(path.clone()),
        fast_solver(),
    );
    let obj = decode_object(&produced.bytes).unwrap();
    assert_eq!(obj.stages[0].leg_count, 6);
    assert_eq!(obj.last_interrupt_height, 1018);
    assert_eq!(obj.last_interrupt, beacon_named("beacon/17"));
    drop(produced);
    let log = std::fs::read_to_string(events_path(&path)).unwrap();
    assert_eq!(log.matches("clock_read_failed").count(), 1);
    let _ = std::fs::remove_dir_all(&dir);

    let dir = tmpdir("down");
    let path = dir.join("hike");
    let clock = MockClock::new(0);
    let cancel = Arc::new(AtomicBool::new(false));
    let source = MemSource::new(chain(1000), [1u32; 18]);
    source.set_down(2);
    let bari = ScriptBari::new(clock.clone(), Arc::clone(&cancel), One::Status(204));
    let cfg = ProduceConfig {
        profile: tiny(),
        public_key: pk_a(),
        bari_url: BARI.into(),
        source: source.clone() as Source,
        client: bari,
        clock: Arc::new(clock.clone()),
        object_path: Some(path.clone()),
        live: true,
        cancel,
    };
    let handle = thread::spawn(move || produce_with(cfg, fast_solver()));
    let start = Instant::now();
    while source.tips() < 8 {
        assert!(start.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(5));
    }
    let mid = source.tips();
    clock.advance(1_200_000);
    thread::sleep(Duration::from_millis(30));
    let later = source.tips();
    let elapsed = start.elapsed();
    source.set_down(0);
    source.wake();
    let produced = handle.join().unwrap().expect("object");
    assert!(later > mid);
    assert!(elapsed < Duration::from_secs(5), "down took {elapsed:?}");
    assert!(produced.legs > mid);
    let obj = decode_object(&produced.bytes).unwrap();
    assert_eq!(obj.last_interrupt_height, 1018);
    drop(produced);
    let log = std::fs::read_to_string(events_path(&path)).unwrap();
    assert_eq!(log.matches("header_source_stalled").count(), 1);
    let _ = std::fs::remove_dir_all(&dir);

    let (produced, _, _, _) = run(
        tiny(),
        {
            let mut legs = [1u32; 18];
            legs[0] = 3;
            legs
        },
        |src| {
            src.inner.lock().unwrap().reorg = Some((
                0,
                1,
                obs(sha256(b"pcoc-pack-2.2/reorg"), 1000),
            ));
        },
        |_| {},
        false,
        None,
        fast_solver(),
    );
    let obj = decode_object(&produced.bytes).unwrap();
    assert_eq!(obj.stages[0].leg_count, 3);
    assert_eq!(obj.last_interrupt, beacon_named("beacon/17"));
    assert_eq!(obj.last_interrupt_height, 1018);
}

#[test]
fn close_at_i17_keeps_arrived_path_and_drops_the_rest() {
    let dir = tmpdir("close");
    let path = dir.join("hike");
    let clock = MockClock::new(0);
    let cancel = Arc::new(AtomicBool::new(false));
    let source = MemSource::new(chain(1000), [1u32; 18]);
    source.set_hold_i17(true);
    let bari = ScriptBari::new(clock.clone(), Arc::clone(&cancel), One::Status(204));
    bari.gap.store(true, Ordering::SeqCst);
    let cfg = ProduceConfig {
        profile: tiny(),
        public_key: pk_a(),
        bari_url: BARI.into(),
        source: source.clone() as Source,
        client: bari.clone(),
        clock: Arc::new(clock),
        object_path: Some(path.clone()),
        live: false,
        cancel,
    };
    let handle = thread::spawn(move || produce_with(cfg, fast_solver()));
    let start = Instant::now();
    while !bari.blocked.load(Ordering::SeqCst) {
        assert!(start.elapsed() < Duration::from_secs(3), "ask did not block");
        thread::sleep(Duration::from_millis(5));
    }
    source.set_hold_i17(false);
    let produced = handle.join().unwrap().expect("object");
    assert!(bari.blocked.load(Ordering::SeqCst));
    assert!(!bari.finished.load(Ordering::SeqCst));
    let on_disk = std::fs::read(&path).unwrap();
    assert_eq!(on_disk, produced.bytes);
    let obj = decode_object(&produced.bytes).unwrap();
    assert_eq!(obj.last_interrupt_height, 1018);
    assert_eq!(obj.stages[1].path.as_ref().unwrap().len(), 146);
    assert!(obj.stages[2].path.as_ref().unwrap().is_empty());
    assert!(produced.stderr_text.contains("path_skip 2\n"));
    assert!(!produced.stderr_text.contains("path_skip 1\n"));
    assert!(produced.stderr_text.ends_with("not fetched by design 17\n"));
    let bytes = produced.bytes.clone();
    bari.release.store(true, Ordering::SeqCst);
    drop(produced);
    assert!(bari.finished.load(Ordering::SeqCst));
    let obj = decode_object(&bytes).unwrap();
    assert!(obj.stages[2].path.as_ref().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn jump_does_not_ask_w17() {
    let mut headers = chain(1000);
    headers[18].height = 1022;
    let source = MemSource::new(headers, [1u32; 18]);
    source.preload(vec![
        obs(sha256(b"pcoc-pack-2.2/extra/1019"), 1019),
        obs(sha256(b"pcoc-pack-2.2/extra/1020"), 1020),
        obs(sha256(b"pcoc-pack-2.2/extra/1021"), 1021),
    ]);
    let clock = MockClock::new(0);
    let cancel = Arc::new(AtomicBool::new(false));
    let bari = ScriptBari::new(clock.clone(), Arc::clone(&cancel), One::Status(204));
    let cfg = ProduceConfig {
        profile: tiny(),
        public_key: pk_a(),
        bari_url: BARI.into(),
        source: source as Source,
        client: bari.clone(),
        clock: Arc::new(clock),
        object_path: None,
        live: false,
        cancel,
    };
    let produced = produce_with(cfg, fast_solver()).expect("object");
    let obj = decode_object(&produced.bytes).unwrap();
    assert_eq!(obj.last_interrupt_height, 1022);
    assert!(obj.stages[17].path.as_ref().unwrap().is_empty());
    let leaf = hex_encode(&obj.stages[17].waypoint);
    assert!(bari.gets().iter().all(|g| g.leaf != leaf));
    assert!(produced.stderr_text.ends_with("not fetched by design 17\n"));
}
