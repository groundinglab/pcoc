//! A live produce on mainnet gossip, wired the way `satoshi-hike-produce
//! --profile live` wires it, in a worker thread.
//!
//! Posts go to Bari (`https://bari.groundinglab.io` unless the settings name
//! another), and headers come from the named gossip peer or the mainnet DNS
//! seeds. The object is written to a temporary sibling, synced, and renamed
//! into place only after the run returns. A stop writes nothing.

use super::adapter::{drive, is_stop};
use super::eventlog::{parse_line, Follower, Tail};
use super::events::ProduceEvent;
use crate::format::now_unix_ms;
use crate::hex;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub type Emit = Arc<dyn Fn(ProduceEvent) + Send + Sync>;

#[derive(Clone, Debug)]
pub struct LiveConfig {
    pub public_key: [u8; 32],
    pub output: PathBuf,
    /// `host:port`. `None` starts from the mainnet DNS seeds.
    pub gossip_peer: Option<String>,
}

pub struct LiveRun {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl LiveRun {
    /// Ask the run to stop after the leg in progress. No object is written.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    pub fn finished(&self) -> bool {
        self.thread.as_ref().is_none_or(|t| t.is_finished())
    }
}

/// The arguments the CLI would get. The pinned parser checks them.
pub fn produce_args(config: &LiveConfig) -> Vec<String> {
    let mut args = vec![
        "--pubkey-hex".to_owned(),
        hex::encode(&config.public_key),
        "--output".to_owned(),
        config.output.to_string_lossy().into_owned(),
        "--profile".to_owned(),
        "live".to_owned(),
        "--bari-url".to_owned(),
        crate::endpoints::BARI.to_owned(),
    ];
    if let Some(peer) = config.gossip_peer.as_ref().filter(|p| !p.trim().is_empty()) {
        args.push("--gossip-peer".to_owned());
        args.push(peer.trim().to_owned());
    }
    args
}

pub fn start(config: LiveConfig, emit: Emit) -> LiveRun {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("pcoc-produce".into())
        .spawn(move || run(config, &thread_stop, emit))
        .expect("spawn produce thread");
    LiveRun {
        stop,
        thread: Some(thread),
    }
}

fn run(config: LiveConfig, stop: &AtomicBool, emit: Emit) {
    let parsed = match hike::parse_produce(&produce_args(&config)) {
        Ok(parsed) => parsed,
        Err(_) => {
            emit(ProduceEvent::Failed {
                message: "the produce settings were refused by the SDK parser".into(),
            });
            return;
        }
    };
    if let Err(err) = hike::parent_dir_exists(&parsed.output) {
        emit(ProduceEvent::Failed {
            message: err.to_string(),
        });
        return;
    }
    let (dial, network) = match parsed.clock {
        hike::ProduceClock::Gossip { dial, network } => (dial, network),
        hike::ProduceClock::Http { .. } => {
            emit(ProduceEvent::Failed {
                message: "HTTP tip is not a produce Observe".into(),
            });
            return;
        }
    };
    emit(ProduceEvent::Started {
        output: config.output.clone(),
        public_key_hex: hex::encode(&parsed.public_key),
        at_ms: now_unix_ms(),
    });

    let mut book = hike::LivePathBook::new(hike::produce_http_agent(), parsed.bari_url.clone());
    let log_path = hike::produce_event_log_path(&parsed.output);
    match book.open_event_log(&parsed.output) {
        Ok(()) => emit(ProduceEvent::Log(format!(
            "event log {}",
            log_path.display()
        ))),
        Err(err) => emit(ProduceEvent::Log(format!(
            "event log {}: {err}",
            log_path.display()
        ))),
    }
    let follow_done = Arc::new(AtomicBool::new(false));
    let follower = spawn_follower(log_path, Arc::clone(&follow_done), Arc::clone(&emit));

    let clock = match dial {
        hike::GossipDial::Named(peer) => {
            emit(ProduceEvent::Log(format!("gossip peer {peer}")));
            hike::HeaderClock::start(peer, network)
        }
        hike::GossipDial::MainnetDnsSeeds => {
            emit(ProduceEvent::Log(
                "gossip from the mainnet DNS seeds".into(),
            ));
            hike::HeaderClock::start_mainnet_dns_seeds()
        }
    };
    let bits = hike::effective_bits();
    let produced = produce(&clock, book, bits, parsed.public_key, stop, &emit);
    clock.shutdown();
    let ops = hike::take_ops_capture();

    follow_done.store(true, Ordering::SeqCst);
    let _ = follower.join();

    let (path_lines, other): (Vec<&str>, Vec<&str>) = ops
        .lines()
        .partition(|l| l.starts_with("path_skip ") || l.starts_with("not fetched by design"));
    for line in other {
        emit(ProduceEvent::Log(super::eventlog::redact(line)));
    }

    match produced {
        Ok(_) if stop.load(Ordering::SeqCst) => emit(ProduceEvent::Stopped),
        Ok(bytes) => {
            if !path_lines.is_empty() {
                emit(ProduceEvent::PathLines(
                    path_lines.iter().map(|s| s.to_string()).collect(),
                ));
            }
            match write_atomically(&config.output, &bytes) {
                Ok(()) => emit(ProduceEvent::ObjectWritten {
                    path: config.output.clone(),
                    bytes: bytes.len() as u64,
                    sha256: Sha256::digest(&bytes).into(),
                    at_ms: now_unix_ms(),
                }),
                Err(err) => emit(ProduceEvent::Failed {
                    message: format!("could not write the object: {err}"),
                }),
            }
        }
        Err(err) if is_stop(&err) => emit(ProduceEvent::Stopped),
        Err(err) => emit(ProduceEvent::Failed {
            message: err.message,
        }),
    }
}

fn produce(
    clock: &hike::HeaderClock,
    book: hike::LivePathBook,
    bits: u32,
    public_key: [u8; 32],
    stop: &AtomicBool,
    emit: &Emit,
) -> Result<Vec<u8>, hike::ProduceFailure> {
    let book = std::cell::RefCell::new(book);
    let tip = |hash: [u8; 32], height: u64| {
        emit(ProduceEvent::Tip {
            height,
            natural_hash: hash,
            peers: None,
            header_seen_unix_ms: now_unix_ms(),
        })
    };
    hike::begin_ops_capture();
    drive(
        public_key,
        hike::Sizes::production(),
        stop,
        &now_unix_ms,
        &|e| emit(e),
        || {
            let (hash, height, header_time) = next_tip(clock, &mut book.borrow_mut(), stop, emit)?;
            Ok((hash, height, header_time))
        },
        |waypoint, beacon_height| {
            let (now_tip, height) = hike::note_produce_headers(clock, &mut book.borrow_mut())?;
            hike::emit_tip_in_hashcash(&now_tip, height);
            book.borrow_mut().post_observing(
                waypoint,
                &now_tip,
                height,
                beacon_height,
                bits,
                |after| {
                    clock.headers_since(after).map(|ticks| {
                        ticks
                            .into_iter()
                            .map(|t| {
                                tip(t.hash, t.height);
                                (t.hash, t.height)
                            })
                            .collect()
                    })
                },
            )?;
            Ok(Vec::new())
        },
        |stages, cut_off| {
            book.borrow_mut().finish(stages, cut_off, |after| loop {
                if stop.load(Ordering::SeqCst) {
                    return Err(hike::ProduceFailure::new(super::adapter::STOPPED));
                }
                match clock.poll_tips_after(after) {
                    Ok(Some(ticks)) => {
                        return Ok(ticks
                            .into_iter()
                            .map(|t| {
                                tip(t.hash, t.height);
                                (t.hash, t.height)
                            })
                            .collect());
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(40)),
                    Err(err) => return Err(err),
                }
            })
        },
    )
}

/// The next Observe. The first call is the seed: the tip the peer already
/// has, from `getheaders`, not a header mined after Start. Later calls return
/// that same tip at once until gossip delivers a newer header.
fn next_tip(
    clock: &hike::HeaderClock,
    book: &mut hike::LivePathBook,
    stop: &AtomicBool,
    emit: &Emit,
) -> Result<([u8; 32], u64, u32), hike::ProduceFailure> {
    let mut told = None;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Err(hike::ProduceFailure::new(super::adapter::STOPPED));
        }
        let peers = clock.connected_peers();
        if told != Some(peers) {
            emit(ProduceEvent::Gossip { peers });
            told = Some(peers);
        }
        match clock.poll_headers_since(book.last_tip()) {
            Ok(Some(ticks)) => {
                let mut newest = None;
                for tick in ticks {
                    book.on_tip(tick.hash, tick.height)?;
                    newest = Some(tick);
                }
                let tick =
                    newest.ok_or_else(|| hike::ProduceFailure::new("clock read failed: reorg"))?;
                return Ok((tick.hash, tick.height, tick.n_time));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(err) => return Err(err),
        }
    }
}

fn spawn_follower(path: PathBuf, done: Arc<AtomicBool>, emit: Emit) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("pcoc-produce-events".into())
        .spawn(move || {
            let mut tail = Tail::new(&path);
            let mut follower = Follower::default();
            loop {
                let last = done.load(Ordering::SeqCst);
                for line in tail.poll() {
                    if let Some(record) = parse_line(&line) {
                        for event in follower.apply(&record) {
                            emit(event);
                        }
                    }
                }
                if last {
                    return;
                }
                std::thread::sleep(Duration::from_millis(400));
            }
        })
        .expect("spawn event log follower")
}

/// Temporary sibling, sync, then rename. A failure leaves no partial object.
pub fn write_atomically(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let name = dest
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, dest)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_are_the_live_profile_and_parse() {
        let config = LiveConfig {
            public_key: hex::decode32(
                "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            )
            .unwrap(),
            output: PathBuf::from("/tmp/hike.cbor"),
            gossip_peer: None,
        };
        let parsed = hike::parse_produce(&produce_args(&config))
            .map_err(|_| ())
            .expect("parses");
        assert_eq!(parsed.profile, hike::PlaneProfile::Live);
        assert_eq!(parsed.bari_url, crate::endpoints::BARI);
        assert!(matches!(
            parsed.clock,
            hike::ProduceClock::Gossip {
                dial: hike::GossipDial::MainnetDnsSeeds,
                ..
            }
        ));

        let named = LiveConfig {
            gossip_peer: Some(" 192.0.2.7:8333 ".into()),
            ..config
        };
        let parsed = hike::parse_produce(&produce_args(&named))
            .map_err(|_| ())
            .expect("parses");
        assert!(
            matches!(parsed.clock, hike::ProduceClock::Gossip { dial: hike::GossipDial::Named(ref p), .. } if p == "192.0.2.7:8333")
        );
    }

    #[test]
    fn atomic_write_leaves_only_the_object() {
        let dir = std::env::temp_dir().join(format!("pcoc-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("hike.cbor");
        write_atomically(&dest, b"abc").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"abc");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A peer that already has a tip answers `getheaders` and does not mine
    /// another header. The seed Observe is that tip.
    #[test]
    fn seed_on_start_reads_the_tip_without_a_new_block() {
        let chain = regtest_chain(4);
        let height = chain.len() as u64 - 1;
        let peer = spawn_peer(chain, true);
        let clock = hike::HeaderClock::start(peer, hike::GossipNetwork::Regtest);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let tick = loop {
            match clock.poll_headers_since(None) {
                Ok(Some(ticks)) => break ticks.last().copied().unwrap(),
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(None) => {
                    clock.shutdown();
                    panic!("the peer's current tip did not arrive");
                }
                Err(err) => {
                    clock.shutdown();
                    panic!("{}", err.message);
                }
            }
        };
        assert_eq!(tick.height, height);
        let stop = AtomicBool::new(false);
        let mut book = hike::LivePathBook::new(hike::produce_http_agent(), "http://127.0.0.1:9");
        let emit: super::Emit = std::sync::Arc::new(|_| {});
        let first = super::next_tip(&clock, &mut book, &stop, &emit).expect("seed");
        let second = super::next_tip(&clock, &mut book, &stop, &emit).expect("same tip");
        assert_eq!(first.1, height);
        assert_eq!(second.0, first.0);
        assert_eq!(second.1, first.1);
        clock.shutdown();
    }

    /// Stop during the tip read returns, and a later start reads a tip.
    #[test]
    fn stop_during_the_tip_read_returns_and_restart_reads_the_tip() {
        let silent = spawn_peer(Vec::new(), false);
        let clock = hike::HeaderClock::start(silent, hike::GossipNetwork::Regtest);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_in = Arc::clone(&stop);
        let started = std::time::Instant::now();
        let handle = std::thread::spawn(move || {
            let mut book =
                hike::LivePathBook::new(hike::produce_http_agent(), "http://127.0.0.1:9");
            let emit: super::Emit = Arc::new(|_| {});
            let result = super::next_tip(&clock, &mut book, &stop_in, &emit);
            clock.shutdown();
            result
        });
        while !handle.is_finished() && started.elapsed() < Duration::from_millis(200) {
            std::thread::sleep(Duration::from_millis(20));
        }
        stop.store(true, Ordering::SeqCst);
        let err = handle.join().expect("stop thread").expect_err("stopped");
        assert!(
            super::is_stop(&err),
            "stop hung or returned {}",
            err.message
        );
        assert!(started.elapsed() < Duration::from_secs(2));

        let chain = regtest_chain(2);
        let height = chain.len() as u64 - 1;
        let peer = spawn_peer(chain, true);
        let clock = hike::HeaderClock::start(peer, hike::GossipNetwork::Regtest);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let tick = loop {
            match clock.poll_headers_since(None) {
                Ok(Some(ticks)) => break ticks.last().copied().unwrap(),
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                other => {
                    clock.shutdown();
                    panic!("restart did not read the tip: {other:?}");
                }
            }
        };
        assert_eq!(tick.height, height);
        clock.shutdown();
    }

    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;

    const REGTEST_MAGIC: [u8; 4] = [0xfa, 0xbf, 0xb5, 0xda];
    const GENESIS_MERKLE: [u8; 32] = [
        0x3b, 0xa3, 0xed, 0xfd, 0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e, 0x67, 0x76, 0x8f,
        0x61, 0x7f, 0xc8, 0x1b, 0xc3, 0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa, 0x4b, 0x1e,
        0x5e, 0x4a,
    ];

    fn spawn_peer(chain: Vec<[u8; 80]>, answer: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let _ = serve(stream, &chain, answer);
            }
        });
        addr.to_string()
    }

    fn serve(mut stream: TcpStream, chain: &[[u8; 80]], answer: bool) -> std::io::Result<()> {
        let _ = stream.set_nodelay(true);
        let mut version_sent = false;
        loop {
            let Some((command, payload)) = read_frame(&mut stream) else {
                return Ok(());
            };
            if command == "version" && !version_sent {
                version_sent = true;
                write_frame(
                    &mut stream,
                    "version",
                    &version_payload(chain.len() as i32 - 1),
                )?;
                write_frame(&mut stream, "verack", &[])?;
            } else if command == "ping" {
                write_frame(&mut stream, "pong", &payload)?;
            } else if command == "getheaders" && answer {
                let loc = locator(&payload).unwrap_or([0; 32]);
                let start = chain
                    .iter()
                    .position(|header| sha256d(header) == loc)
                    .map(|index| index + 1)
                    .unwrap_or(0);
                let batch: Vec<[u8; 80]> = chain.iter().skip(start).copied().take(2_000).collect();
                write_frame(&mut stream, "headers", &headers_payload(&batch))?;
            }
        }
    }

    fn regtest_chain(extra: usize) -> Vec<[u8; 80]> {
        let mut headers = vec![encode(
            1,
            [0; 32],
            GENESIS_MERKLE,
            1_296_688_602,
            0x207f_ffff,
            2,
        )];
        for i in 0..extra {
            let prev = sha256d(headers.last().unwrap());
            headers.push(mine(prev, 1_700_000_000 + i as u32));
        }
        headers
    }

    fn mine(prev: [u8; 32], time: u32) -> [u8; 80] {
        for nonce in 0..100_000 {
            let header = encode(1, prev, [0; 32], time, 0x207f_ffff, nonce);
            if pow_ok(&header) {
                return header;
            }
        }
        panic!("regtest header did not mine");
    }

    fn pow_ok(header: &[u8; 80]) -> bool {
        let hash = sha256d(header);
        let bits = u32::from_le_bytes(header[72..76].try_into().unwrap());
        let target = compact_target(bits);
        for i in (0..32).rev() {
            if hash[i] < target[i] {
                return true;
            }
            if hash[i] > target[i] {
                return false;
            }
        }
        true
    }

    fn compact_target(bits: u32) -> [u8; 32] {
        let exponent = (bits >> 24) as usize;
        let mantissa = bits & 0x007f_ffff;
        let mut target = [0u8; 32];
        if exponent > 3 {
            let start = exponent - 3;
            target[start] = (mantissa & 0xff) as u8;
            target[start + 1] = ((mantissa >> 8) & 0xff) as u8;
            target[start + 2] = ((mantissa >> 16) & 0xff) as u8;
        }
        target
    }

    fn encode(
        version: u32,
        prev: [u8; 32],
        merkle: [u8; 32],
        time: u32,
        bits: u32,
        nonce: u32,
    ) -> [u8; 80] {
        let mut header = [0u8; 80];
        header[0..4].copy_from_slice(&version.to_le_bytes());
        header[4..36].copy_from_slice(&prev);
        header[36..68].copy_from_slice(&merkle);
        header[68..72].copy_from_slice(&time.to_le_bytes());
        header[72..76].copy_from_slice(&bits.to_le_bytes());
        header[76..80].copy_from_slice(&nonce.to_le_bytes());
        header
    }

    fn sha256d(bytes: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        Sha256::digest(Sha256::digest(bytes)).into()
    }

    fn version_payload(height: i32) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&70_015i32.to_le_bytes());
        payload.extend_from_slice(&1u64.to_le_bytes());
        payload.extend_from_slice(&1_700_000_000i64.to_le_bytes());
        payload.extend_from_slice(&[0u8; 26]);
        payload.extend_from_slice(&[0u8; 26]);
        payload.extend_from_slice(&1u64.to_le_bytes());
        let agent = b"/mock:0.1/";
        payload.push(agent.len() as u8);
        payload.extend_from_slice(agent);
        payload.extend_from_slice(&height.to_le_bytes());
        payload.push(1);
        payload
    }

    fn locator(payload: &[u8]) -> Option<[u8; 32]> {
        if payload.len() < 37 || payload[4] != 1 {
            return None;
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&payload[5..37]);
        Some(hash)
    }

    fn headers_payload(headers: &[[u8; 80]]) -> Vec<u8> {
        let mut payload = vec![headers.len() as u8];
        for header in headers {
            payload.extend_from_slice(header);
            payload.push(0);
        }
        payload
    }

    fn write_frame(stream: &mut TcpStream, command: &str, payload: &[u8]) -> std::io::Result<()> {
        let mut command_bytes = [0u8; 12];
        command_bytes[..command.len()].copy_from_slice(command.as_bytes());
        let sum = sha256d(payload);
        let mut frame = Vec::with_capacity(24 + payload.len());
        frame.extend_from_slice(&REGTEST_MAGIC);
        frame.extend_from_slice(&command_bytes);
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(&sum[..4]);
        frame.extend_from_slice(payload);
        stream.write_all(&frame)
    }

    fn read_frame(stream: &mut TcpStream) -> Option<(String, Vec<u8>)> {
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
        let mut head = [0u8; 24];
        stream.read_exact(&mut head).ok()?;
        if head[..4] != REGTEST_MAGIC {
            return None;
        }
        let length = u32::from_le_bytes(head[16..20].try_into().unwrap()) as usize;
        let mut payload = vec![0u8; length];
        if length > 0 {
            stream.read_exact(&mut payload).ok()?;
        }
        let command = head[4..16]
            .iter()
            .take_while(|byte| **byte != 0)
            .map(|byte| *byte as char)
            .collect();
        Some((command, payload))
    }
}
