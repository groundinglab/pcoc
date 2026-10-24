//! Header source for a live hike.
//!
//! `tip` returns the latest header already held. It does not dial, sleep, or
//! take a lock that a peer read holds (Rule I3). Peer reads run on the gossip
//! thread. That thread keeps the peer swap and the fail-closed quorum bound
//! from `pcoc-gossip`.

use pcoc_gossip::{open_cancellable, HeaderError, TipTick, ABSENT_LIVE_SOURCE};
pub use pcoc_gossip::{PeerCensus, PeerSnapshot};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// One header the walk may use. Copied by value so the walk holds no lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldTip {
    pub hash: [u8; 32],
    pub height: u64,
    pub header: [u8; 80],
    pub n_time: u32,
}

impl From<TipTick> for HeldTip {
    fn from(tick: TipTick) -> Self {
        Self {
            hash: tick.hash,
            height: tick.height,
            header: tick.header,
            n_time: tick.n_time,
        }
    }
}

/// What the walk is allowed to call. Every method returns at once.
pub trait TipSource: Send + Sync {
    fn tip(&self) -> Option<HeldTip>;
    fn failed(&self) -> Option<String>;
    /// Headers learned since the previous drain, oldest first.
    fn drain_learned(&self) -> Vec<HeldTip>;
    fn take_alerts(&self) -> Vec<String> {
        Vec::new()
    }
    /// Block until a tip is published or `timeout` elapses. Not used after the
    /// first leg exists. The default does not wait.
    fn wait_for_tip(&self, _timeout: Duration) -> bool {
        false
    }
}

/// A source that will not produce. The hike stops and writes no object.
pub struct FailClosedSource {
    message: String,
}

impl FailClosedSource {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl TipSource for FailClosedSource {
    fn tip(&self) -> Option<HeldTip> {
        None
    }
    fn failed(&self) -> Option<String> {
        Some(self.message.clone())
    }
    fn drain_learned(&self) -> Vec<HeldTip> {
        Vec::new()
    }
}

/// Test and desktop-mock source. `tip` copies the held header and does not
/// touch `net`.
pub struct ManualSource {
    tip: Mutex<Option<HeldTip>>,
    learned: Mutex<Vec<HeldTip>>,
    failed: Mutex<Option<String>>,
    /// A stand-in for a peer read. `tip` does not lock this.
    pub net: Mutex<()>,
}

impl ManualSource {
    pub fn holding(tip: HeldTip) -> Self {
        Self {
            tip: Mutex::new(Some(tip)),
            learned: Mutex::new(Vec::new()),
            failed: Mutex::new(None),
            net: Mutex::new(()),
        }
    }

    pub fn publish(&self, tip: HeldTip) {
        self.learned.lock().expect("learned").push(tip.clone());
        *self.tip.lock().expect("tip") = Some(tip);
    }
}

impl TipSource for ManualSource {
    fn tip(&self) -> Option<HeldTip> {
        self.tip.lock().expect("tip").clone()
    }
    fn failed(&self) -> Option<String> {
        self.failed.lock().expect("failed").clone()
    }
    fn drain_learned(&self) -> Vec<HeldTip> {
        std::mem::take(&mut *self.learned.lock().expect("learned"))
    }
}

struct Snap {
    tip: Mutex<Option<HeldTip>>,
    learned: Mutex<Vec<HeldTip>>,
    failed: Mutex<Option<String>>,
    alerts: Mutex<Vec<String>>,
    seen: AtomicU64,
    cv: Condvar,
    cv_mu: Mutex<()>,
}

impl Snap {
    fn publish(&self, tip: HeldTip) {
        self.learned.lock().expect("learned").push(tip.clone());
        *self.tip.lock().expect("tip") = Some(tip);
        let _guard = self.cv_mu.lock().expect("cv");
        self.cv.notify_all();
    }

    fn fail(&self, message: String) {
        *self.failed.lock().expect("failed") = Some(message);
        let _guard = self.cv_mu.lock().expect("cv");
        self.cv.notify_all();
    }

    fn alert(&self, name: &str) {
        self.alerts.lock().expect("alerts").push(name.to_owned());
    }
}

/// Live gossip. The walk calls [`TipSource::tip`]. The peer session runs here.
pub struct GossipSource {
    snap: Arc<Snap>,
    census: Arc<PeerCensus>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl GossipSource {
    /// `peer` empty selects the Bitcoin Core DNS seed list inside `pcoc-gossip`.
    /// `stop` is the producer's cancel flag. Drop sets it and joins the thread.
    pub fn spawn(peer: &str, stop: Arc<AtomicBool>) -> Self {
        let census = PeerCensus::new();
        let snap = Arc::new(Snap {
            tip: Mutex::new(None),
            learned: Mutex::new(Vec::new()),
            failed: Mutex::new(None),
            alerts: Mutex::new(Vec::new()),
            seen: AtomicU64::new(0),
            cv: Condvar::new(),
            cv_mu: Mutex::new(()),
        });
        let thread_snap = Arc::clone(&snap);
        let thread_stop = Arc::clone(&stop);
        let thread_census = Arc::clone(&census);
        let peer = peer.to_owned();
        let thread = std::thread::Builder::new()
            .name("pcoc-gossip".into())
            .spawn(move || gossip_loop(peer, thread_stop, thread_snap, thread_census))
            .expect("spawn gossip thread");
        Self {
            snap,
            census,
            stop,
            thread: Mutex::new(Some(thread)),
        }
    }

    /// Handshake census the gossip thread updates. `None` until the first count.
    pub fn census(&self) -> Arc<PeerCensus> {
        Arc::clone(&self.census)
    }

    pub fn tip_fn(&self) -> Arc<dyn Fn() -> Option<HeldTip> + Send + Sync> {
        let snap = Arc::clone(&self.snap);
        Arc::new(move || snap.tip.lock().expect("tip").clone())
    }

    pub fn seen_height(&self) -> u64 {
        self.snap.seen.load(Ordering::SeqCst)
    }
}

impl Drop for GossipSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.lock().expect("join").take() {
            let _ = thread.join();
        }
    }
}

impl TipSource for GossipSource {
    fn tip(&self) -> Option<HeldTip> {
        let tip = self.snap.tip.lock().expect("tip").clone();
        if let Some(tip) = &tip {
            self.snap.seen.store(tip.height, Ordering::SeqCst);
        }
        tip
    }

    fn failed(&self) -> Option<String> {
        self.snap.failed.lock().expect("failed").clone()
    }

    fn drain_learned(&self) -> Vec<HeldTip> {
        std::mem::take(&mut *self.snap.learned.lock().expect("learned"))
    }

    fn take_alerts(&self) -> Vec<String> {
        std::mem::take(&mut *self.snap.alerts.lock().expect("alerts"))
    }

    fn wait_for_tip(&self, timeout: Duration) -> bool {
        let guard = self.snap.cv_mu.lock().expect("cv");
        if self.snap.tip.lock().expect("tip").is_some() || self.failed().is_some() {
            return self.snap.tip.lock().expect("tip").is_some();
        }
        let (_guard, _) = self.snap.cv.wait_timeout(guard, timeout).expect("cv wait");
        self.snap.tip.lock().expect("tip").is_some()
    }
}

struct FeedGuard {
    census: Arc<PeerCensus>,
    stop: Arc<AtomicBool>,
}

impl Drop for FeedGuard {
    fn drop(&mut self) {
        // A producer Stop sets `stop` before the thread returns. Any other
        // exit — panic, or the read loop ending — means the sockets are gone.
        if !self.stop.load(Ordering::SeqCst) {
            self.census.mark_stopped();
        }
    }
}

fn gossip_loop(peer: String, stop: Arc<AtomicBool>, snap: Arc<Snap>, census: Arc<PeerCensus>) {
    let _guard = FeedGuard {
        census: Arc::clone(&census),
        stop: Arc::clone(&stop),
    };
    let mut clock = open_cancellable(&peer, Some(Arc::clone(&stop)));
    clock.watch_peers(census);
    let mut last: Option<[u8; 32]> = None;
    while !stop.load(Ordering::SeqCst) {
        match clock.wait(last) {
            Ok(ticks) => {
                for tick in ticks {
                    let held = HeldTip::from(tick);
                    last = Some(held.hash);
                    snap.publish(held);
                }
            }
            Err(err) if err.message == "cancelled" => break,
            Err(err) if absent(&err) => {
                snap.fail(err.message);
                break;
            }
            Err(err) => {
                snap.alert(alert_name(&err.message));
                if stop.load(Ordering::SeqCst) {
                    break;
                }
            }
        }
    }
}

fn absent(err: &HeaderError) -> bool {
    err.message == ABSENT_LIVE_SOURCE
}

fn alert_name(message: &str) -> &'static str {
    if message == "too few peers" {
        "header_peers_low"
    } else {
        "header_source_stalled"
    }
}

/// The sentence a fail-closed source returns. No object is written after it.
pub const FAIL_CLOSED: &str = "header source failed closed";
