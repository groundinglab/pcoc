//! Per-client limiter for verify posts and sign-in posts.
//!
//! `GET /v1/verify/{job_id}` and `GET /v1/health` are not limited.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    per_window: u32,
    window: Duration,
    seen: Mutex<HashMap<String, Window>>,
}

struct Window {
    started: Instant,
    hits: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rejected {
    pub retry_after_secs: u64,
    pub per_window: u32,
}

impl RateLimiter {
    pub fn per_minute(per_window: u32) -> Self {
        Self {
            per_window,
            window: Duration::from_secs(60),
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// Count one upload from `client`. `Err` means the caller is over budget.
    pub fn admit(&self, client: &str) -> Result<(), Rejected> {
        if self.per_window == 0 {
            return Ok(());
        }
        let mut seen = self.seen.lock().expect("rate map");
        if seen.len() > 8192 {
            seen.retain(|_, w| w.started.elapsed() < self.window);
        }
        let entry = seen.entry(client.to_owned()).or_insert(Window {
            started: Instant::now(),
            hits: 0,
        });
        if entry.started.elapsed() >= self.window {
            entry.started = Instant::now();
            entry.hits = 0;
        }
        if entry.hits >= self.per_window {
            let elapsed = entry.started.elapsed();
            return Err(Rejected {
                retry_after_secs: self.window.saturating_sub(elapsed).as_secs() + 1,
                per_window: self.per_window,
            });
        }
        entry.hits += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_is_per_client() {
        let limiter = RateLimiter::per_minute(2);
        assert!(limiter.admit("203.0.113.1").is_ok());
        assert!(limiter.admit("203.0.113.1").is_ok());
        assert!(limiter.admit("203.0.113.1").is_err());
        assert!(limiter.admit("203.0.113.2").is_ok());
    }

    #[test]
    fn zero_disables_the_limiter() {
        let limiter = RateLimiter::per_minute(0);
        for _ in 0..1000 {
            assert!(limiter.admit("203.0.113.1").is_ok());
        }
    }
}
