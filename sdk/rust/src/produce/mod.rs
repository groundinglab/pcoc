//! SPEC §13 producer. The walk is `drive`. Posts, asks, and the log are other threads.
//!
//! `HeaderSource` is the hand-off for P3. The core did not define it. P3 implements
//! the trait in `header`. This module is the only caller.

mod ask;
pub mod bari_client;
pub mod clock;
mod drive;
mod events;
pub mod header;
mod post;
mod state;

use crate::bari_rule::origin_from_bari_url;
use crate::constants::FILL;
use crate::hashcash::hashcash_solve_spec;
use crate::object::{encode_object, make_object};
use crate::profile::Profile;
use bari_client::Client;
use clock::SharedClock;
use header::Source;
use post::PostJob;
use state::Run;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub use bari_client::{BariClient, BariResponse, PathGet, WaypointPost};
pub use clock::{Clock, MockClock, SystemClock};
pub use header::{HeaderSource, Observed, TipRead};

pub(crate) type Solver = Arc<dyn Fn(&[u8; 32], &[u8; 32]) -> Option<(u64, [u8; 32])> + Send + Sync>;

/// Inputs for one run. The default plane is the test plane: `live` is false and no event log is written.
pub struct ProduceConfig {
    pub profile: Profile,
    pub public_key: [u8; 32],
    pub bari_url: String,
    pub source: Source,
    pub client: Client,
    pub clock: SharedClock,
    /// Hike object path. The live log, when `live` is set, is this path plus the suffix `.events`.
    pub object_path: Option<PathBuf>,
    pub live: bool,
    pub cancel: Arc<AtomicBool>,
}

/// The object, the stderr step, and the walk timers.
///
/// Drop joins the background threads. The object is already encoded; drop does not decide the bytes.
/// A test that still needs a resend advances the clock before dropping this value.
pub struct Produced {
    pub bytes: Vec<u8>,
    pub stderr_text: String,
    pub legs: u64,
    pub walk_ns: u64,
    pub leg_ns: u64,
    pub asks: u64,
    shutdown: Shutdown,
}

impl Drop for Produced {
    fn drop(&mut self) {
        self.shutdown.stop();
    }
}

/// SPEC §13. Hashcash at difficulty 21. Returns `None` when the run is cancelled,
/// `{bari-url}` is not an origin this file can serialize, or the object path cannot be written.
pub fn produce(cfg: ProduceConfig) -> Option<Produced> {
    let solver: Solver = Arc::new(|challenge, waypoint| hashcash_solve_spec(challenge, waypoint));
    produce_with(cfg, solver)
}

pub(crate) fn produce_with(cfg: ProduceConfig, solver: Solver) -> Option<Produced> {
    let origin = origin_from_bari_url(&cfg.bari_url).ok()?;
    let (log_tx, log_join) = match open_log(&cfg) {
        Some((tx, join)) => (Some(tx), Some(join)),
        None => (None, None),
    };
    let run = Run::new(Arc::clone(&cfg.cancel), log_tx.is_some(), log_tx);
    let (post_tx, post_joins) = post::spawn(
        Arc::clone(&run),
        Arc::clone(&cfg.source),
        Arc::clone(&cfg.client),
        Arc::clone(&cfg.clock),
        origin,
        cfg.bari_url.clone(),
        solver,
        Arc::clone(&cfg.cancel),
    );
    let post_tx_walk = post_tx.clone();
    let run_ask = Arc::clone(&run);
    let source_ask = Arc::clone(&cfg.source);
    let client_ask = Arc::clone(&cfg.client);
    let bari_url = cfg.bari_url.clone();
    let ask_join = thread::Builder::new()
        .name("pcoc-ask".into())
        .spawn(move || ask_loop(run_ask, source_ask, client_ask, bari_url))
        .expect("ask thread");
    let run_walk = Arc::clone(&run);
    let source_walk = Arc::clone(&cfg.source);
    let profile = cfg.profile.clone();
    let public_key = cfg.public_key;
    let walk_join = thread::Builder::new()
        .name("pcoc-walk".into())
        .spawn(move || drive::walk(&run_walk, source_walk.as_ref(), &profile, &public_key, &post_tx_walk))
        .expect("walk thread");

    let shutdown = Shutdown {
        cancel: Arc::clone(&cfg.cancel),
        clock: Arc::clone(&cfg.clock),
        source: Arc::clone(&cfg.source),
        post_tx: Some(post_tx),
        posts: post_joins,
        ask: Some(ask_join),
        log: log_join,
        run: Some(Arc::clone(&run)),
    };

    let walked = match walk_join.join() {
        Ok(Some(walked)) => walked,
        _ => return None,
    };

    let paths = {
        let guard = run.paths.lock().unwrap();
        run.closed.store(true, Ordering::SeqCst);
        guard.clone()
    };
    cfg.source.wake();

    let mut stages = walked.stages;
    for g in 1..=16 {
        let body = match paths[g].clone() {
            Some(p) if !p.is_empty() => p,
            _ => Vec::new(),
        };
        stages[g].path = Some(body);
    }
    stages[17].path = Some(Vec::new());

    let mut stderr_text = String::new();
    for g in 1..=16 {
        if stages[g].path.as_ref().map(|p| p.is_empty()).unwrap_or(true) {
            stderr_text.push_str(&format!("path_skip {g}\n"));
        }
    }
    stderr_text.push_str("not fetched by design 17\n");

    let seed_beacon = stages[0].beacon;
    let obj = make_object(
        FILL,
        cfg.profile.slot_bytes as u64,
        cfg.profile.n_slots,
        cfg.profile.sample_bytes as u64,
        cfg.profile.steps_per_leg,
        cfg.public_key,
        seed_beacon,
        stages,
        walked.last_interrupt,
        walked.last_interrupt_height,
    );
    let bytes = encode_object(&obj);
    if let Some(path) = &cfg.object_path {
        if std::fs::write(path, &bytes).is_err() {
            return None;
        }
    }
    let _ = std::io::stderr().write_all(stderr_text.as_bytes());
    let asks = run.asks.load(Ordering::Relaxed);
    Some(Produced {
        bytes,
        stderr_text,
        legs: walked.legs,
        walk_ns: walked.walk_ns,
        leg_ns: walked.leg_ns,
        asks,
        shutdown,
    })
}

fn ask_loop(run: Arc<Run>, source: Source, client: Client, bari_url: String) {
    loop {
        if run.closed.load(Ordering::SeqCst) || run.cancel.load(Ordering::SeqCst) {
            break;
        }
        let batch = source.take_learned();
        if !batch.is_empty() {
            for h in batch {
                run.note_block(h.height, &h.natural);
                run.learned.lock().unwrap().push(h);
            }
        }
        ask::dispatch(&run, client.as_ref(), &bari_url);
        if run.closed.load(Ordering::SeqCst) || run.cancel.load(Ordering::SeqCst) {
            break;
        }
        source.wait_learned();
    }
}

fn open_log(cfg: &ProduceConfig) -> Option<(mpsc::SyncSender<String>, JoinHandle<()>)> {
    if !cfg.live {
        return None;
    }
    let object = cfg.object_path.as_ref()?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(events_path(object))
        .ok()?;
    let (tx, rx) = mpsc::sync_channel(256);
    let cancel = Arc::clone(&cfg.cancel);
    let join = thread::Builder::new()
        .name("pcoc-log".into())
        .spawn(move || events::write_loop(file, rx, cancel))
        .expect("log thread");
    Some((tx, join))
}

pub(crate) fn events_path(object: &Path) -> PathBuf {
    let mut name = object.file_name().unwrap_or_default().to_os_string();
    name.push(".events");
    match object.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

struct Shutdown {
    cancel: Arc<AtomicBool>,
    clock: SharedClock,
    source: Source,
    post_tx: Option<Sender<PostJob>>,
    posts: Vec<JoinHandle<()>>,
    ask: Option<JoinHandle<()>>,
    log: Option<JoinHandle<()>>,
    run: Option<Arc<Run>>,
}

impl Shutdown {
    fn stop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        self.clock.wake();
        self.source.wake();
        self.post_tx.take();
        for handle in self.posts.drain(..) {
            let _ = handle.join();
        }
        if let Some(handle) = self.ask.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.log.take() {
            let _ = handle.join();
        }
        self.run.take();
    }
}

impl Drop for Shutdown {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests;
