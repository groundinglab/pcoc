//! Live produce. Start hike runs the same producer as `pcoc produce --live`.
//! Stop sets the cancel flag. The walk finishes the current leg, writes no
//! object, and the gossip read returns.

use super::events::{AskOutcome, ProduceEvent};
use pcoc::profile::Profile;
use pcoc_produce::bari::{BariHub, HttpBari, LiveEvent, Progress, DEFAULT_BARI_URL, RESEND_AFTER};
use pcoc_produce::live::{produce_live, LiveRunOpts, LiveStop};
use pcoc_produce::source::{GossipSource, HeldTip, PeerCensus, PeerSnapshot, TipSource};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long a finished produce thread may sit in a running phase before the
/// window treats the exit as unexpected. The terminal event usually wins.
pub const UNEXPECTED_EXIT: Duration = Duration::from_secs(1);

/// Panic, or a produce thread that ended with no terminal event.
pub const PRODUCE_THREAD_EXITED: &str = "produce thread exited";

/// How often the peer panel is told about handshake changes.
const PEER_POLL: Duration = Duration::from_millis(200);

pub type Emit = Arc<dyn Fn(ProduceEvent) + Send + Sync>;

/// Kept for a source that reports the absent-source sentence.
pub const LIVE_HEADER_SOURCE: &str = "live header source is not in this build";

#[derive(Clone, Debug)]
pub struct LiveConfig {
    pub public_key: [u8; 32],
    pub output: PathBuf,
    /// `host:port`. Empty selects the DNS seed list.
    pub gossip_peer: Option<String>,
    pub bari_url: String,
}

impl LiveConfig {
    pub fn new(public_key: [u8; 32], output: PathBuf, gossip_peer: Option<String>) -> Self {
        Self {
            public_key,
            output,
            gossip_peer,
            bari_url: DEFAULT_BARI_URL.to_owned(),
        }
    }
}

pub struct LiveRun {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    /// Detached on drop. It exits when `stop` is set or the feed is marked stopped.
    peers: Option<JoinHandle<()>>,
    /// Latest handshake census. The 1s ticker reads this so a `Started` event
    /// cannot leave the panel stuck after the watch already published.
    census: Option<Arc<PeerCensus>>,
}

impl LiveRun {
    /// Ask the run to stop. A leg already started finishes. No object is written.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    pub fn finished(&self) -> bool {
        self.thread
            .as_ref()
            .is_none_or(|thread| thread.is_finished())
    }

    pub fn census(&self) -> Option<Arc<PeerCensus>> {
        self.census.clone()
    }
}

impl Drop for LiveRun {
    fn drop(&mut self) {
        // Detach. Joining the produce thread here would wait out the current leg.
        drop(self.thread.take());
        drop(self.peers.take());
    }
}

pub fn start(config: LiveConfig, emit: Emit) -> LiveRun {
    let stop = Arc::new(AtomicBool::new(false));
    let peer = config.gossip_peer.clone().unwrap_or_default();
    let source = GossipSource::spawn(&peer, Arc::clone(&stop));
    let census = source.census();
    let progress = map_progress(&emit, Some(Arc::clone(&census)));
    let peers = spawn_peer_watch(Arc::clone(&census), Arc::clone(&emit), Arc::clone(&stop));
    let bari = match HttpBari::open(
        &config.bari_url,
        RESEND_AFTER,
        source.tip_fn(),
        Arc::clone(&progress),
    ) {
        Ok(bari) => Arc::new(bari) as Arc<dyn BariHub>,
        Err(message) => {
            emit(ProduceEvent::Failed { message });
            stop.store(true, Ordering::SeqCst);
            return LiveRun {
                stop,
                thread: None,
                peers: Some(peers),
                census: Some(census),
            };
        }
    };
    spawn(
        config,
        Profile::production(),
        Arc::new(source),
        bari,
        stop,
        emit,
        progress,
        Some(peers),
        Some(census),
    )
}

/// Desktop tests pass a mock source and a mock Bari. This does not dial.
pub fn start_with(
    config: LiveConfig,
    profile: Profile,
    source: Arc<dyn TipSource>,
    bari: Arc<dyn BariHub>,
    emit: Emit,
) -> LiveRun {
    let stop = Arc::new(AtomicBool::new(false));
    let progress = map_progress(&emit, None);
    spawn(
        config, profile, source, bari, stop, emit, progress, None, None,
    )
}

fn spawn(
    config: LiveConfig,
    profile: Profile,
    source: Arc<dyn TipSource>,
    bari: Arc<dyn BariHub>,
    stop: Arc<AtomicBool>,
    emit: Emit,
    progress: Progress,
    peers: Option<JoinHandle<()>>,
    census: Option<Arc<PeerCensus>>,
) -> LiveRun {
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("pcoc-produce".into())
        .spawn(move || run(config, profile, source, bari, &thread_stop, emit, progress))
        .expect("spawn produce thread");
    LiveRun {
        stop,
        thread: Some(thread),
        peers,
        census,
    }
}

/// `true` when a finished produce thread has stayed in a running phase long
/// enough that the terminal event is not merely in flight.
pub fn produce_exit_is_unexpected(
    running: bool,
    finished: bool,
    seen_for: Option<Duration>,
) -> bool {
    running && finished && seen_for.is_some_and(|waited| waited >= UNEXPECTED_EXIT)
}

fn spawn_peer_watch(census: Arc<PeerCensus>, emit: Emit, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("pcoc-peers".into())
        .spawn(move || {
            let mut prev = None;
            while !stop.load(Ordering::SeqCst) {
                let snap = census.snapshot();
                let stopped = snap.as_ref().is_some_and(|item| item.feed_stopped);
                for event in census_events(&mut prev, snap) {
                    emit(event);
                }
                if stopped {
                    break;
                }
                std::thread::sleep(PEER_POLL);
            }
        })
        .expect("spawn peer watch")
}

/// Events for a census change. `prev` is updated. The first unknown snapshot emits nothing.
pub fn census_events(
    prev: &mut Option<PeerSnapshot>,
    snap: Option<PeerSnapshot>,
) -> Vec<ProduceEvent> {
    let Some(snap) = snap else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let changed = prev.as_ref().is_none_or(|prior| {
        prior.handshake != snap.handshake
            || prior.sockets != snap.sockets
            || prior.addrs != snap.addrs
    });
    if changed {
        out.push(ProduceEvent::Gossip {
            peers: snap.handshake,
            sockets: snap.sockets,
            addrs: snap.addrs.clone(),
        });
    }
    let stopped = snap.feed_stopped && prev.as_ref().is_none_or(|prior| !prior.feed_stopped);
    *prev = Some(snap);
    if stopped {
        out.push(ProduceEvent::TipFeedStopped);
    }
    out
}

fn run(
    config: LiveConfig,
    profile: Profile,
    source: Arc<dyn TipSource>,
    bari: Arc<dyn BariHub>,
    stop: &AtomicBool,
    emit: Emit,
    progress: Progress,
) {
    let emit_panic = Arc::clone(&emit);
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_body(config, profile, source, bari, stop, &emit, &progress);
    }))
    .is_err();
    if panicked {
        emit_panic(ProduceEvent::Failed {
            message: PRODUCE_THREAD_EXITED.to_owned(),
        });
    }
}

fn run_body(
    config: LiveConfig,
    profile: Profile,
    source: Arc<dyn TipSource>,
    bari: Arc<dyn BariHub>,
    stop: &AtomicBool,
    emit: &Emit,
    progress: &Progress,
) {
    emit(ProduceEvent::Started {
        output: config.output.clone(),
        public_key_hex: pcoc::hexutil::hex_encode(&config.public_key),
        at_ms: now_ms(),
    });
    let opts = LiveRunOpts {
        public_key: &config.public_key,
        profile: &profile,
        out: &config.output,
        cancel: stop,
    };
    match produce_live(&opts, source.as_ref(), bari.as_ref(), progress) {
        Ok(product) => match write_atomically(&config.output, &product.bytes) {
            Ok(()) => {
                let lines = product.stderr.lines().map(str::to_owned).collect();
                emit(ProduceEvent::PathLines(lines));
                emit(ProduceEvent::ObjectWritten {
                    path: config.output,
                    bytes: product.bytes.len() as u64,
                    sha256: pcoc::hash::sha256(&product.bytes),
                    at_ms: now_ms(),
                });
            }
            Err(err) => emit(ProduceEvent::Failed {
                message: format!("could not write the object: {err}"),
            }),
        },
        Err(LiveStop::Cancelled) => emit(ProduceEvent::Stopped),
        Err(LiveStop::Failed(message)) => emit(ProduceEvent::Failed { message }),
    }
}

/// A positive handshake count to attach to a tip. Zero stays unknown here:
/// the peer watch publishes zero together with the socket count.
fn handshake_if_known(census: Option<&PeerCensus>) -> Option<u32> {
    let snap = census?.snapshot()?;
    (snap.handshake > 0).then_some(snap.handshake)
}

fn map_progress(emit: &Emit, census: Option<Arc<PeerCensus>>) -> Progress {
    let emit = Arc::clone(emit);
    let legs = Arc::new(AtomicU64::new(0));
    let seeded = Arc::new(AtomicBool::new(false));
    Arc::new(move |event| {
        let at_ms = now_ms();
        match event {
            LiveEvent::Terrain => emit(ProduceEvent::TerrainFilled { at_ms }),
            LiveEvent::Leg { stage, leg, .. } => {
                let stored = legs.fetch_add(1, Ordering::SeqCst) + 1;
                emit(ProduceEvent::LegRecorded {
                    stage,
                    j: leg as u64,
                    stored_in_stage: stored,
                    at_ms,
                });
            }
            LiveEvent::Interrupt { stage, height } => {
                let stored = legs.swap(0, Ordering::SeqCst);
                emit(ProduceEvent::StageClosed {
                    stage,
                    legs: stored,
                    interrupt_height: height,
                });
            }
            LiveEvent::Header {
                height,
                hash,
                n_time,
            } => {
                if !seeded.swap(true, Ordering::SeqCst) {
                    emit(ProduceEvent::SeedBeacon {
                        height,
                        natural_hash: hash,
                        header_time_unix_s: n_time,
                        at_ms,
                    });
                }
                emit(ProduceEvent::Tip {
                    height,
                    natural_hash: hash,
                    peers: handshake_if_known(census.as_deref()),
                    header_seen_unix_ms: at_ms,
                });
            }
            LiveEvent::Waypoint { .. } => {}
            LiveEvent::Posted {
                stage,
                status,
                succeeded,
                ..
            } => emit(ProduceEvent::WaypointPosted {
                g: stage as u64,
                appended: succeeded,
                status,
            }),
            LiveEvent::Fetch {
                stage,
                worked,
                miss,
                at_height,
                ..
            } => {
                let outcome = if worked {
                    AskOutcome::Served
                } else {
                    AskOutcome::Miss(miss)
                };
                emit(ProduceEvent::PathAsk {
                    g: stage as u64,
                    at_height,
                    outcome,
                });
            }
            LiveEvent::Checked { .. } => {}
            LiveEvent::Alert { name, .. } => emit(ProduceEvent::Log(name)),
            LiveEvent::Closed => emit(ProduceEvent::Settling),
        }
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// Temporary sibling, sync, then rename. A failure leaves no partial object.
pub fn write_atomically(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    pcoc_produce::live::write_atomically(dest, bytes)
}

/// One header for a mock source. The bytes are not a Bitcoin header.
pub fn mock_tip(height: u64) -> HeldTip {
    let mut hash = [0u8; 32];
    hash[..8].copy_from_slice(&height.to_le_bytes());
    HeldTip {
        hash,
        height,
        header: [0u8; 80],
        n_time: 1_700_000_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcoc_produce::bari::SilentBari;
    use pcoc_produce::source::{FailClosedSource, ManualSource, FAIL_CLOSED};
    use std::time::Duration;

    fn small() -> Profile {
        Profile::conformance_small(32, 64, 65_536, 1).expect("profile")
    }

    #[test]
    fn fail_closed_source_writes_no_object() {
        let dir = std::env::temp_dir().join(format!("pcoc-live-closed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let output = dir.join("hike.cbor");
        let (tx, rx) = std::sync::mpsc::channel();
        let emit: Emit = Arc::new(move |event| {
            let _ = tx.send(event);
        });
        let run = start_with(
            LiveConfig::new([0x11; 32], output.clone(), None),
            small(),
            Arc::new(FailClosedSource::new(FAIL_CLOSED)),
            Arc::new(SilentBari),
            emit,
        );
        let event = rx.recv_timeout(Duration::from_secs(5)).expect("event");
        assert!(matches!(event, ProduceEvent::Started { .. }));
        let event = rx.recv_timeout(Duration::from_secs(5)).expect("failed");
        match event {
            ProduceEvent::Failed { message } => assert_eq!(message, FAIL_CLOSED),
            other => panic!("expected the fail-closed error, got {other:?}"),
        }
        for _ in 0..50 {
            if run.finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(run.finished());
        assert!(!output.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stop_cancels_before_an_object_is_written() {
        let dir = std::env::temp_dir().join(format!("pcoc-live-stop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let output = dir.join("hike.cbor");
        let (tx, rx) = std::sync::mpsc::channel();
        let emit: Emit = Arc::new(move |event| {
            let _ = tx.send(event);
        });
        let run = start_with(
            LiveConfig::new([0x22; 32], output.clone(), Some("192.0.2.7:8333".into())),
            small(),
            Arc::new(ManualSource::holding(mock_tip(972_685))),
            Arc::new(SilentBari),
            emit,
        );
        let mut saw_leg = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(ProduceEvent::LegRecorded { .. }) => {
                    saw_leg = true;
                    break;
                }
                Ok(ProduceEvent::ObjectWritten { .. }) => panic!("object before stop"),
                Ok(_) => {}
                Err(_) => {}
            }
        }
        assert!(saw_leg, "the walk did not record a leg");
        run.stop();
        let mut saw_stop = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(ProduceEvent::Stopped) => {
                    saw_stop = true;
                    break;
                }
                Ok(ProduceEvent::ObjectWritten { .. }) => panic!("stop still wrote an object"),
                Ok(ProduceEvent::Failed { message }) => panic!("stop failed: {message}"),
                Ok(_) => {}
                Err(_) if run.finished() => break,
                Err(_) => {}
            }
        }
        assert!(saw_stop);
        assert!(
            run.finished() || {
                std::thread::sleep(Duration::from_millis(200));
                run.finished()
            }
        );
        assert!(!output.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn census_events_emit_real_counts_and_a_stopped_feed() {
        let mut prev = None;
        assert!(census_events(&mut prev, None).is_empty());
        assert!(prev.is_none());

        let connecting = census_events(
            &mut prev,
            Some(PeerSnapshot {
                handshake: 0,
                sockets: 2,
                addrs: Vec::new(),
                feed_stopped: false,
            }),
        );
        assert_eq!(
            connecting,
            vec![ProduceEvent::Gossip {
                peers: 0,
                sockets: 2,
                addrs: Vec::new(),
            }]
        );

        let live = census_events(
            &mut prev,
            Some(PeerSnapshot {
                handshake: 2,
                sockets: 2,
                addrs: vec!["192.168.20.153:8333".into(), "173.212.199.219:8333".into()],
                feed_stopped: false,
            }),
        );
        match &live[..] {
            [ProduceEvent::Gossip {
                peers,
                sockets,
                addrs,
            }] => {
                assert_eq!(*peers, 2);
                assert_eq!(*sockets, 2);
                assert_eq!(addrs.len(), 2);
            }
            other => panic!("expected one gossip event, got {other:?}"),
        }

        let stopped = census_events(
            &mut prev,
            Some(PeerSnapshot {
                handshake: 0,
                sockets: 0,
                addrs: Vec::new(),
                feed_stopped: true,
            }),
        );
        assert!(stopped.iter().any(|event| matches!(
            event,
            ProduceEvent::Gossip {
                peers: 0,
                sockets: 0,
                ..
            }
        )));
        assert!(stopped
            .iter()
            .any(|event| matches!(event, ProduceEvent::TipFeedStopped)));
    }

    #[test]
    fn peer_watch_emits_a_mock_handshake_count() {
        let census = PeerCensus::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let emit: Emit = Arc::new(move |event| {
            let _ = tx.send(event);
        });
        let stop = Arc::new(AtomicBool::new(false));
        let handle = spawn_peer_watch(census.clone(), emit, Arc::clone(&stop));
        census.write(
            2,
            2,
            vec!["192.168.20.153:8333".into(), "173.212.199.219:8333".into()],
        );
        let event = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("gossip event");
        match event {
            ProduceEvent::Gossip {
                peers,
                sockets,
                addrs,
            } => {
                assert_eq!(peers, 2);
                assert_eq!(sockets, 2);
                assert_eq!(
                    addrs,
                    vec![
                        "192.168.20.153:8333".to_owned(),
                        "173.212.199.219:8333".to_owned()
                    ]
                );
            }
            other => panic!("expected Gossip, got {other:?}"),
        }
        census.mark_stopped();
        let mut saw_stop = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(ProduceEvent::TipFeedStopped) => {
                    saw_stop = true;
                    break;
                }
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert!(saw_stop, "a dead gossip feed must emit TipFeedStopped");
        stop.store(true, Ordering::SeqCst);
        handle.join().expect("peer watch");
    }

    #[test]
    fn unexpected_exit_waits_for_an_in_flight_terminal_event() {
        assert!(!produce_exit_is_unexpected(true, true, None));
        assert!(!produce_exit_is_unexpected(
            true,
            true,
            Some(Duration::from_millis(200))
        ));
        assert!(produce_exit_is_unexpected(
            true,
            true,
            Some(UNEXPECTED_EXIT)
        ));
        assert!(!produce_exit_is_unexpected(
            false,
            true,
            Some(UNEXPECTED_EXIT)
        ));
        assert!(!produce_exit_is_unexpected(
            true,
            false,
            Some(UNEXPECTED_EXIT)
        ));
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
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
