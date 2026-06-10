//! Rate limiting with Strategy Pattern.

use crate::config::RATE_WINDOW_SECS;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Strategy trait for rate limiting — enables testability with mock time.
/// This is a pure domain abstraction with no worker dependencies.
///
/// # Example
///
/// ```rust,ignore
/// // This trait is typically used in implementations like:
/// //
/// // struct MyRateLimiter;
/// //
/// // impl RateLimiter for MyRateLimiter {
/// //     fn check(&self, max_per_window: u32) -> bool {
/// //         // Implementation would go here
/// //     }
/// // }
/// ```
pub trait RateLimiter: Send + Sync {
    /// Returns true if the request is allowed, false if rate limit exceeded.
    fn check(&self, max_per_window: u32) -> bool;
}

/// Lock-free rate limiter using atomic counters.
/// Uses a fn pointer for time source so tests can inject mock time.
pub struct AtomicRateLimiter {
    count: AtomicU32,
    window_start: AtomicU64,
    time_source: fn() -> u64,
}

impl AtomicRateLimiter {
    /// Create a new AtomicRateLimiter with the given initial counter values and time source.
    pub const fn new(count: u32, window_start: u64, time_source: fn() -> u64) -> Self {
        Self {
            count: AtomicU32::new(count),
            window_start: AtomicU64::new(window_start),
            time_source,
        }
    }

    /// Check if a request is allowed under the rate limit.
    /// Uses atomic counters for lock-free concurrent access.
    /// Returns false if the rate limit has been exceeded.
    pub fn check_rate_limit(&self, max_per_window: u32) -> bool {
        let now = (self.time_source)();
        let last_reset = self.window_start.load(Ordering::Relaxed);

        // Reset window if expired
        if now > last_reset && now - last_reset > RATE_WINDOW_SECS {
            // CAS to prevent double-reset race between concurrent requests
            if self
                .window_start
                .compare_exchange(last_reset, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                self.count.store(0, Ordering::Relaxed);
            }
        }

        let current = self.count.fetch_add(1, Ordering::Relaxed);
        current < max_per_window
    }
}

// SAFETY: In a single-threaded WASM environment, it's safe to implement Sync.
// This is only safe because we're in a single-threaded environment.
unsafe impl Sync for AtomicRateLimiter {}

impl RateLimiter for AtomicRateLimiter {
    fn check(&self, max_per_window: u32) -> bool {
        self.check_rate_limit(max_per_window)
    }
}

/// Monotonic counter-based approximate time for WASM (no SystemTime available).
/// Uses a global request counter divided by estimated requests/sec rate.
pub fn now_approx_secs() -> u64 {
    static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    count / 100
}

/// Beacon endpoint: 100 requests per window
pub static BEACON_RATE_LIMITER: AtomicRateLimiter =
    AtomicRateLimiter::new(0, 0, crate::domain::rate_limit::now_approx_secs);

/// Stats endpoints: 200 requests per window
pub static STATS_RATE_LIMITER: AtomicRateLimiter =
    AtomicRateLimiter::new(0, 0, crate::domain::rate_limit::now_approx_secs);

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_time() -> u64 {
        static MOCK_NOW: AtomicU64 = AtomicU64::new(0);
        MOCK_NOW.load(Ordering::Relaxed)
    }

    #[test]
    fn rate_limit_allows_under_limit() {
        let test_limiter = AtomicRateLimiter::new(0, 0, mock_time);

        for _ in 0..5 {
            assert!(test_limiter.check(10));
        }
    }

    #[test]
    fn rate_limit_blocks_over_limit() {
        static MOCK_NOW: AtomicU64 = AtomicU64::new(0);
        fn mock_time_fn() -> u64 {
            MOCK_NOW.load(Ordering::Relaxed)
        }

        let test_limiter = AtomicRateLimiter::new(0, 0, mock_time_fn);

        // Set a high counter to avoid window reset during test
        MOCK_NOW.store(10000, Ordering::Relaxed);

        for _ in 0..10 {
            test_limiter.check_rate_limit(10);
        }
        // 11th request should be blocked
        assert!(!test_limiter.check(10));
    }

    #[test]
    fn rate_limit_window_reset() {
        static MOCK_NOW: AtomicU64 = AtomicU64::new(0);

        fn mock_time_fn() -> u64 {
            MOCK_NOW.load(Ordering::Relaxed)
        }

        let test_limiter = AtomicRateLimiter::new(0, 0, mock_time_fn);

        // Test window reset by advancing mock time beyond window
        MOCK_NOW.store(200, Ordering::Relaxed); // Advance time beyond window

        // This should trigger a window reset
        let allowed = test_limiter.check_rate_limit(10);
        assert!(allowed);
    }
}
