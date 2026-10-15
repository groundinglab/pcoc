//! In-process counters. `0` disables a counter. `/health` is not counted.

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

impl RateLimiter {
    pub fn per_minute(per_window: u32) -> Self {
        Self {
            per_window,
            window: Duration::from_secs(60),
            seen: Mutex::new(HashMap::new()),
        }
    }

    pub fn admit(&self, client: &str) -> bool {
        if self.per_window == 0 {
            return true;
        }
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if seen.len() > 8192 {
            seen.retain(|_, window| window.started.elapsed() < self.window);
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
            return false;
        }
        entry.hits += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_is_per_client_and_zero_is_off() {
        let limiter = RateLimiter::per_minute(2);
        assert!(limiter.admit("10.0.0.1"));
        assert!(limiter.admit("10.0.0.1"));
        assert!(!limiter.admit("10.0.0.1"));
        assert!(limiter.admit("10.0.0.2"));
        let off = RateLimiter::per_minute(0);
        for _ in 0..20 {
            assert!(off.admit("same"));
        }
    }
}
