//! Token-bucket rate limiter keyed on arbitrary strings (typically IP addresses).
//!
//! The implementation is intentionally simple: a sliding window that resets
//! once `window` duration has elapsed since the first request in that window.
//! Each [`RateLimiter`] is `Send + Sync` and requires no external state store.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// In-process rate limiter.
///
/// # Example
///
/// ```rust
/// use stack_payment::rate_limit::RateLimiter; // internal crate path
/// let rl = RateLimiter::new(10, 60);
/// assert!(rl.check("192.168.1.1"));
/// ```
pub struct RateLimiter {
    max_requests: u32,
    window: Duration,
    state: RwLock<HashMap<String, (u32, Instant)>>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// * `max_requests` — maximum requests allowed per `window_seconds`.
    /// * `window_seconds` — length of the sliding window.
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(window_seconds),
            state: RwLock::new(HashMap::new()),
        }
    }

    /// Check whether `key` is within its rate limit.
    ///
    /// Returns `true` if the request is allowed and `false` if the caller
    /// should respond with `429 Too Many Requests`.
    pub fn check(&self, key: &str) -> bool {
        let mut map = self.state.write().unwrap_or_else(|p| p.into_inner());
        let now = Instant::now();

        // Opportunistic GC: evict expired entries when the map grows large to
        // prevent unbounded memory growth under high-cardinality IP sets.
        if map.len() > 10_000 {
            map.retain(|_, (_, ts)| now.duration_since(*ts) < self.window);
        }

        let entry = map.entry(key.to_string()).or_insert((0, now));

        if now.duration_since(entry.1) >= self.window {
            // Window has elapsed — start a fresh one.
            *entry = (1, now);
            true
        } else if entry.0 < self.max_requests {
            entry.0 += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_max() {
        let rl = RateLimiter::new(3, 60);
        assert!(rl.check("10.0.0.1"));
        assert!(rl.check("10.0.0.1"));
        assert!(rl.check("10.0.0.1"));
        assert!(!rl.check("10.0.0.1"));
    }

    #[test]
    fn different_keys_are_independent() {
        let rl = RateLimiter::new(1, 60);
        assert!(rl.check("10.0.0.1"));
        assert!(!rl.check("10.0.0.1"));
        // Different IP starts a fresh window.
        assert!(rl.check("10.0.0.2"));
    }

    #[test]
    fn window_reset() {
        let rl = RateLimiter::new(1, 0); // 0-second window expires immediately
        assert!(rl.check("10.0.0.1"));
        // After the zero-duration window, the next call should open a new window.
        assert!(rl.check("10.0.0.1"));
    }
}
