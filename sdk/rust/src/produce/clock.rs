//! Clock for the one resend wait. The ask path does not use this trait.
//! The walk does not call it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
    /// Return when `now_ms() >= deadline_ms`, or when `cancel` is set.
    fn wait_until(&self, deadline_ms: u64, cancel: &AtomicBool);
    /// Unblock a `wait_until` that is still parked.
    fn wake(&self);
}

pub type SharedClock = Arc<dyn Clock>;

/// Wall clock. The resend wait polls so a cancel is noticed within 50 ms.
#[derive(Clone, Copy, Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn wait_until(&self, deadline_ms: u64, cancel: &AtomicBool) {
        loop {
            if cancel.load(Ordering::SeqCst) || self.now_ms() >= deadline_ms {
                return;
            }
            let left = deadline_ms.saturating_sub(self.now_ms());
            thread::sleep(Duration::from_millis(left.min(50).max(1)));
        }
    }

    fn wake(&self) {}
}

/// Mock clock. `advance` moves time and wakes waiters. It does not sleep.
#[derive(Clone)]
pub struct MockClock {
    inner: Arc<MockInner>,
}

struct MockInner {
    now: Mutex<u64>,
    cv: Condvar,
    /// How many `wait_until` calls are inside the wait. Tests use this so an
    /// advance happens after the deadline is captured.
    waiters: std::sync::atomic::AtomicUsize,
}

impl MockClock {
    pub fn new(start_ms: u64) -> Self {
        Self {
            inner: Arc::new(MockInner {
                now: Mutex::new(start_ms),
                cv: Condvar::new(),
                waiters: std::sync::atomic::AtomicUsize::new(0),
            }),
        }
    }

    pub fn advance_to(&self, ms: u64) {
        let mut now = self.inner.now.lock().unwrap();
        if ms > *now {
            *now = ms;
        }
        self.inner.cv.notify_all();
    }

    pub fn advance(&self, by_ms: u64) {
        let mut now = self.inner.now.lock().unwrap();
        *now = now.saturating_add(by_ms);
        self.inner.cv.notify_all();
    }

    pub fn waiters(&self) -> usize {
        self.inner.waiters.load(std::sync::atomic::Ordering::SeqCst)
    }
}

struct Waiters(Arc<MockInner>);

impl Drop for Waiters {
    fn drop(&mut self) {
        self.0.waiters.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Clock for MockClock {
    fn now_ms(&self) -> u64 {
        *self.inner.now.lock().unwrap()
    }

    fn wait_until(&self, deadline_ms: u64, cancel: &AtomicBool) {
        self.inner.waiters.fetch_add(1, Ordering::SeqCst);
        let _dec = Waiters(Arc::clone(&self.inner));
        let mut now = self.inner.now.lock().unwrap();
        while *now < deadline_ms && !cancel.load(Ordering::SeqCst) {
            now = self.inner.cv.wait(now).unwrap();
        }
    }

    fn wake(&self) {
        self.inner.cv.notify_all();
    }
}
