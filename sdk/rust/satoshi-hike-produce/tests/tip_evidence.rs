//! A waypoint HTTP 400 is a missed POST. Produce finishes the hike, writes the
//! object, and returns. The tip source is a finite script: each stage closes
//! on the next header, with no synth interval and no `wait_tips_after`.
//!
//! This is the short replacement for the full-size drive. The PR job runs it.
//! The nightly full-size job does not.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const TIP_MISMATCH_BODY: &[u8] = b"Hashcash proof invalid for live tip challenge";

struct OpsCapture;

impl OpsCapture {
    fn start() -> Self {
        hike::begin_ops_capture();
        Self
    }

    fn finish(self) -> String {
        let text = hike::take_ops_capture();
        std::mem::forget(self);
        text
    }
}

impl Drop for OpsCapture {
    fn drop(&mut self) {
        let _ = hike::take_ops_capture();
    }
}

/// One settlement at a time. The capture buffer and the transient-wait env are process-wide.
fn one_hike() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn local_listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, format!("127.0.0.1:{port}"))
}

fn wait_until_accepts(peer: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if TcpStream::connect(peer).is_ok() {
            return;
        }
        if Instant::now() > deadline {
            panic!("mocked gossip peer did not accept on {peer}");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// Handshake only. The synth interval is long, so this peer never mines.
/// The hike's tips come from [`scripted_tips`], not from this socket.
fn establish_gossip(peer: &str) {
    let clock = hike::HeaderClock::start(peer, hike::GossipNetwork::Regtest);
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let _ = clock.observe();
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(5))
        .expect("mocked gossip peer did not establish");
}

fn scripted_tips() -> Vec<([u8; 32], u64)> {
    // One seed plus one interrupt for each of the 18 stages.
    (0..19)
        .map(|i| {
            let mut hash = [0u8; 32];
            hash[0] = i as u8;
            hash[31] = 0x5a;
            (hash, 100 + i)
        })
        .collect()
}

struct TipScript {
    tips: Vec<([u8; 32], u64)>,
    at: usize,
}

impl TipScript {
    fn next(&mut self) -> Result<([u8; 32], u64), hike::ProduceFailure> {
        let tip = self.tips.get(self.at).copied().ok_or_else(|| {
            hike::ProduceFailure::new("tip script ended before the hike returned")
        })?;
        self.at += 1;
        Ok(tip)
    }

    fn current(&self) -> ([u8; 32], u64) {
        self.tips[self.at - 1]
    }
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut seen = Vec::new();
    let mut buf = [0u8; 2048];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                seen.extend_from_slice(&buf[..n]);
                let Some(split) = seen.windows(4).position(|window| window == b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&seen[..split]);
                if seen.len() >= split + 4 + content_length(&headers) {
                    break;
                }
            }
        }
    }
    seen
}

fn serve_tip_mismatch(listener: TcpListener, posts: Arc<AtomicUsize>, gets: Arc<AtomicUsize>) {
    loop {
        let Ok((mut stream, _)) = listener.accept() else {
            break;
        };
        let seen = read_request(&mut stream);
        let head = String::from_utf8_lossy(&seen);
        if head.starts_with("GET ") {
            gets.fetch_add(1, Ordering::SeqCst);
        } else if head.starts_with("POST ") {
            posts.fetch_add(1, Ordering::SeqCst);
        }
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            TIP_MISMATCH_BODY.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(TIP_MISMATCH_BODY);
        let _ = stream.flush();
    }
}

fn produce_args(shape: &[&str], peer: &str, bari_url: &str) -> hike::ProduceArgs {
    let mut args = vec![
        "--pubkey-hex".to_owned(),
        "ab".repeat(32),
        "--output".to_owned(),
        "out.cbor".to_owned(),
    ];
    for flag in shape {
        args.push((*flag).to_owned());
        if *flag == "--gossip-peer" {
            args.push(peer.to_owned());
        } else if *flag == "--bari-url" {
            args.push(bari_url.to_owned());
        }
    }
    hike::parse_produce(&args).expect("produce usage")
}

fn assert_tip_disagreement_exits(shape: &[&str], profile: hike::PlaneProfile) {
    let _one = one_hike();
    let started = Instant::now();
    let capture = OpsCapture::start();

    let (synth_listener, peer) = local_listener();
    drop(synth_listener);
    let synth_peer = peer.clone();
    thread::spawn(move || {
        let _ = hike::serve_synth(&synth_peer, Duration::from_secs(3600));
    });
    wait_until_accepts(&peer);
    establish_gossip(&peer);

    let posts = Arc::new(AtomicUsize::new(0));
    let gets = Arc::new(AtomicUsize::new(0));
    let (bari_listener, bari_addr) = local_listener();
    let posts_srv = Arc::clone(&posts);
    let gets_srv = Arc::clone(&gets);
    thread::spawn(move || serve_tip_mismatch(bari_listener, posts_srv, gets_srv));
    let bari_url = format!("http://{bari_addr}");

    let parsed = produce_args(shape, &peer, &bari_url);
    assert_eq!(parsed.profile, profile);
    assert!(
        matches!(
            parsed.clock,
            hike::ProduceClock::Gossip {
                dial: hike::GossipDial::Named(_),
                network: hike::GossipNetwork::Regtest,
            }
        ),
        "tip disagreement stays on the gossip clock"
    );

    let previous_waits = std::env::var("PCOC_PRODUCE_TRANSIENT_WAITS").ok();
    std::env::set_var("PCOC_PRODUCE_TRANSIENT_WAITS", "0");

    let dir = std::env::temp_dir().join(format!(
        "satoshi-hike-produce-tip-evidence-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("out.cbor");

    let tips = Arc::new(Mutex::new(TipScript {
        tips: scripted_tips(),
        at: 0,
    }));
    let book = Rc::new(RefCell::new(hike::LivePathBook::new(
        hike::produce_http_agent(),
        bari_url,
    )));
    let observes = Arc::new(AtomicUsize::new(0));
    let produced = {
        let tips_observe = Arc::clone(&tips);
        let tips_open = Arc::clone(&tips);
        let observes_post = Arc::clone(&observes);
        let book_open = Rc::clone(&book);
        let book_finish = Rc::clone(&book);
        hike::produce_with_settlement_sized(
            parsed.public_key,
            hike::Sizes::conformance_small(),
            move || {
                tips_observe
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .next()
            },
            move |waypoint, beacon_height| {
                let (tip, height) = tips_open
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .current();
                hike::emit_tip_in_hashcash(&tip, height);
                book_open.borrow_mut().post_observing(
                    waypoint,
                    &tip,
                    height,
                    beacon_height,
                    8,
                    |_after| {
                        let n = observes_post.fetch_add(1, Ordering::SeqCst);
                        let mut hash = [0xcd; 32];
                        hash[0] = n as u8;
                        Ok(vec![(hash, 1_000 + n as u64)])
                    },
                )?;
                Ok(Vec::new())
            },
            move |stages, cut_off| {
                book_finish.borrow_mut().finish(stages, cut_off, |_after| {
                    // Every POST missed, so no plant is open. Asking the clock
                    // here would be the hang. Settlement must return without it.
                    Err(hike::ProduceFailure::new(
                        "finish waited for a tip after disagreement",
                    ))
                })
            },
        )
    };

    match previous_waits {
        Some(value) => std::env::set_var("PCOC_PRODUCE_TRANSIENT_WAITS", value),
        None => std::env::remove_var("PCOC_PRODUCE_TRANSIENT_WAITS"),
    }

    let stderr = capture.finish();
    let bytes = produced.expect(&stderr);
    std::fs::write(&output, &bytes).unwrap();
    let stdout = String::from_utf8(
        hike::Report::produce_pass(output.to_str().unwrap().to_owned()).to_bytes(),
    )
    .unwrap();

    assert!(
        started.elapsed() < Duration::from_secs(10),
        "tip disagreement took {:?}",
        started.elapsed()
    );
    assert!(stdout.contains("\"result\":\"present\""), "{stdout}");
    assert!(!stdout.contains("\"result\":\"fail\""), "{stdout}");
    assert!(!stdout.contains("\"result\":\"pass\""), "{stdout}");
    assert!(output.exists(), "{stderr}");
    assert!(!bytes.is_empty());

    let estab = format!("gossip_estab call_site=gossip_peer peer={peer}");
    let estab_at = stderr.find(&estab).expect(&stderr);
    let tip_at = stderr
        .find("tip_in_hashcash call_site=post_waypoint tip_natural=")
        .expect(&stderr);
    let miss_at = stderr
        .find("waypoint_post call_site=post_waypoint status=400 continued=yes")
        .expect(&stderr);
    assert!(estab_at < tip_at && tip_at < miss_at, "{stderr}");

    let tip_line = stderr
        .lines()
        .find(|line| line.starts_with("tip_in_hashcash "))
        .unwrap();
    let rest = tip_line
        .strip_prefix("tip_in_hashcash call_site=post_waypoint tip_natural=")
        .unwrap();
    let (hex, height) = rest.split_once(" height=").unwrap();
    assert_eq!(hex.len(), 64, "{tip_line}");
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()), "{tip_line}");
    assert!(height.parse::<u64>().is_ok(), "{tip_line}");
    assert!(!stderr.to_ascii_lowercase().contains("clear"), "{stderr}");
    assert!(!stderr.contains("connectivity_wait"), "{stderr}");
    assert!(stderr.contains("path_skip 1\n"), "{stderr}");
    assert!(stderr.contains("not fetched by design 17\n"), "{stderr}");
    assert_eq!(
        stderr.matches("leg_count=1 beacon_height=").count(),
        18,
        "{stderr}"
    );
    // W_1 … W_17. Exact tips-differ text Observes once and POSTs one fresh nonce.
    assert_eq!(posts.load(Ordering::SeqCst), 34, "{stderr}");
    assert_eq!(observes.load(Ordering::SeqCst), 17, "{stderr}");
    assert_eq!(gets.load(Ordering::SeqCst), 0, "{stderr}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_profile_tip_disagreement_fails_the_run() {
    assert_tip_disagreement_exits(&["--gossip-peer", "--bari-url"], hike::PlaneProfile::Test);
}

#[test]
fn gossip_logs_estab_and_tip_before_waypoint_400() {
    assert_tip_disagreement_exits(
        &[
            "--profile",
            "slow-lan",
            "--gossip-network",
            "regtest",
            "--gossip-peer",
            "--bari-url",
        ],
        hike::PlaneProfile::SlowLan,
    );
}
