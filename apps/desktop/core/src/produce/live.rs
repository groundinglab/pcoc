//! Live produce. Start hike runs the same producer as `pcoc produce --live`.
//! Stop sets the cancel flag. The walk finishes the current leg, writes no
//! object, and the gossip read returns.

use super::events::{AskOutcome, ProduceEvent};
use pcoc::profile::Profile;
use pcoc_produce::bari::{
    BariHub, HttpBari, LiveEvent, Progress, SilentBari, DEFAULT_BARI_URL, RESEND_AFTER,
};
use pcoc_produce::live::{produce_live, LiveRunOpts, LiveStop};
use pcoc_produce::source::{
    FailClosedSource, GossipSource, HeldTip, ManualSource, TipSource, FAIL_CLOSED,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

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
}

pub fn start(config: LiveConfig, emit: Emit) -> LiveRun {
    let stop = Arc::new(AtomicBool::new(false));
    let peer = config.gossip_peer.clone().unwrap_or_default();
    let source = GossipSource::spawn(&peer, Arc::clone(&stop));
    let progress = map_progress(&emit);
    let bari = match HttpBari::open(
        &config.bari_url,
        RESEND_AFTER,
        source.tip_fn(),
        Arc::clone(&progress),
    ) {
        Ok(bari) => Arc::new(bari) as Arc<dyn BariHub>,
        Err(message) => {
            emit(ProduceEvent::Failed { message });
            return LiveRun { stop, thread: None };
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
    let progress = map_progress(&emit);
    spawn(config, profile, source, bari, stop, emit, progress)
}

fn spawn(
    config: LiveConfig,
    profile: Profile,
    source: Arc<dyn TipSource>,
    bari: Arc<dyn BariHub>,
    stop: Arc<AtomicBool>,
    emit: Emit,
    progress: Progress,
) -> LiveRun {
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("pcoc-produce".into())
        .spawn(move || run(config, profile, source, bari, &thread_stop, emit, progress))
        .expect("spawn produce thread");
    LiveRun {
        stop,
        thread: Some(thread),
    }
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
    match produce_live(&opts, source.as_ref(), bari.as_ref(), &progress) {
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

fn map_progress(emit: &Emit) -> Progress {
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
                    peers: None,
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
