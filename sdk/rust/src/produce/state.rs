//! Shared producer state. The walk never holds these locks across a leg.

use super::events;
use super::header::Observed;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

pub(crate) const PHASE_EMPTY: u8 = 0;
pub(crate) const PHASE_POSTING: u8 = 1;
pub(crate) const PHASE_PLANTED: u8 = 2;
pub(crate) const PHASE_DROPPED: u8 = 3;

#[derive(Clone)]
pub(crate) struct Plant {
    pub waypoint: [u8; 32],
    pub height: u64,
    pub phase: u8,
    pub asked: [bool; 3],
}

impl Default for Plant {
    fn default() -> Self {
        Self {
            waypoint: [0u8; 32],
            height: 0,
            phase: PHASE_EMPTY,
            asked: [false; 3],
        }
    }
}

pub(crate) struct Run {
    pub closed: AtomicBool,
    pub cancel: Arc<AtomicBool>,
    pub stage: AtomicUsize,
    pub legs: AtomicU64,
    pub asks: AtomicU64,
    pub paths: Mutex<[Option<Vec<u8>>; 17]>,
    pub plants: Mutex<[Plant; 18]>,
    pub learned: Mutex<Vec<Observed>>,
    last_block_height: Mutex<Option<u64>>,
    log_tx: Option<SyncSender<String>>,
    log_failed: AtomicBool,
    live: bool,
}

impl Run {
    pub(crate) fn new(cancel: Arc<AtomicBool>, live: bool, log_tx: Option<SyncSender<String>>) -> Arc<Self> {
        Arc::new(Self {
            closed: AtomicBool::new(false),
            cancel,
            stage: AtomicUsize::new(0),
            legs: AtomicU64::new(0),
            asks: AtomicU64::new(0),
            paths: Mutex::new(std::array::from_fn(|_| None)),
            plants: Mutex::new(std::array::from_fn(|_| Plant::default())),
            learned: Mutex::new(Vec::new()),
            last_block_height: Mutex::new(None),
            log_tx,
            log_failed: AtomicBool::new(false),
            live,
        })
    }

    pub(crate) fn emit(&self, line: String) {
        let Some(tx) = &self.log_tx else {
            return;
        };
        if self.log_failed.swap(false, Ordering::Relaxed) {
            let alert = events::alert_line("log_append_failed", self.stage.load(Ordering::Relaxed), None);
            if tx.try_send(alert).is_err() {
                self.log_failed.store(true, Ordering::Relaxed);
                return;
            }
        }
        if let Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) = tx.try_send(line) {
            self.log_failed.store(true, Ordering::Relaxed);
        }
    }

    /// SPEC §13.5 block record. A header that does not advance the height is ignored.
    pub(crate) fn note_block(&self, height: u64, natural: &[u8; 32]) {
        if !self.live {
            return;
        }
        let mut last = self.last_block_height.lock().unwrap();
        if last.is_some_and(|h| height <= h) {
            return;
        }
        *last = Some(height);
        drop(last);
        self.emit(events::block_line(height, natural));
    }

    pub(crate) fn path_present(&self, stage: usize) -> bool {
        self.paths.lock().unwrap()[stage]
            .as_ref()
            .is_some_and(|p| !p.is_empty())
    }

    /// True when this run appends the section 13.5 log.
    pub(crate) fn live_log(&self) -> bool {
        self.live
    }
}
