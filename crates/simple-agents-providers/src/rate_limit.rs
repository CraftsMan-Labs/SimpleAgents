//! Rate limiting for provider requests.
//!
//! This module provides rate limiting using the token bucket algorithm
//! via the `governor` crate. Supports both per-instance and shared rate limits.

use governor::{
    clock::DefaultClock,
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorRateLimiter,
};
use simple_agent_type::config::{RateLimitConfig, RateLimitScope};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::{Arc, RwLock};

/// Rate limiter for provider requests.
///
/// Uses token bucket algorithm to limit requests per second with burst support.
#[derive(Clone)]
pub struct RateLimiter {
    limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    ///
    /// # Arguments
    /// - `requests_per_second`: Maximum requests per second
    /// - `burst_size`: Maximum burst size (concurrent requests)
    ///
    /// # Example
    /// ```
    /// use simple_agents_providers::rate_limit::RateLimiter;
    ///
    /// let limiter = RateLimiter::new(10, 20);
    /// ```
    pub fn new(requests_per_second: u32, burst_size: u32) -> Self {
        let quota = Quota::per_second(
            NonZeroU32::new(requests_per_second).expect("requests_per_second must be > 0"),
        )
        .allow_burst(NonZeroU32::new(burst_size).expect("burst_size must be > 0"));

        Self {
            limiter: Arc::new(GovernorRateLimiter::direct(quota)),
        }
    }

    /// Wait until a request is allowed by the rate limiter.
    ///
    /// This method blocks asynchronously until the request can proceed.
    pub async fn until_ready(&self) {
        self.limiter.until_ready().await;
    }

    /// Check if a request is allowed immediately without waiting.
    ///
    /// Returns `true` if the request is allowed, `false` otherwise.
    pub fn check(&self) -> bool {
        self.limiter.check().is_ok()
    }
}

/// Shared rate limiter registry.
///
/// Maintains rate limiters per API key for shared rate limiting.
pub struct SharedRateLimiters {
    limiters: RwLock<HashMap<String, RateLimiter>>,
    config: RateLimitConfig,
}

impl SharedRateLimiters {
    /// Create a new shared rate limiter registry.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            limiters: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Get or create a rate limiter for the given key.
    pub fn get_or_create(&self, key: impl Into<String>) -> RateLimiter {
        let key = key.into();

        // Try read lock first for fast path
        {
            let limiters = self.limiters.read().unwrap();
            if let Some(limiter) = limiters.get(&key) {
                return limiter.clone();
            }
        }

        // Need to create a new limiter - acquire write lock
        let mut limiters = self.limiters.write().unwrap();

        // Double-check after acquiring write lock (another thread may have created it)
        if let Some(limiter) = limiters.get(&key) {
            return limiter.clone();
        }

        // Create new limiter
        let limiter = RateLimiter::new(self.config.requests_per_second, self.config.burst_size);
        limiters.insert(key, limiter.clone());
        limiter
    }
}

/// Rate limiter wrapper that handles both per-instance and shared modes.
#[derive(Clone)]
pub enum MaybeRateLimiter {
    /// No rate limiting
    None,
    /// Per-instance rate limiting
    PerInstance(RateLimiter),
    /// Shared rate limiting (requires key)
    Shared(Arc<SharedRateLimiters>),
}

impl MaybeRateLimiter {
    /// Create a rate limiter from configuration.
    pub fn from_config(config: &RateLimitConfig) -> Self {
        if !config.enabled {
            return MaybeRateLimiter::None;
        }

        match config.scope {
            RateLimitScope::PerInstance => {
                let limiter = RateLimiter::new(config.requests_per_second, config.burst_size);
                MaybeRateLimiter::PerInstance(limiter)
            }
            RateLimitScope::Shared => {
                let shared = Arc::new(SharedRateLimiters::new(config.clone()));
                MaybeRateLimiter::Shared(shared)
            }
        }
    }

    /// Wait until a request is allowed.
    ///
    /// For shared limiters, requires an API key or identifier.
    pub async fn until_ready(&self, key: Option<&str>) {
        match self {
            MaybeRateLimiter::None => {
                // No rate limiting - allow immediately
            }
            MaybeRateLimiter::PerInstance(limiter) => {
                limiter.until_ready().await;
            }
            MaybeRateLimiter::Shared(shared) => {
                let key = key.unwrap_or("default");
                let limiter = shared.get_or_create(key);
                limiter.until_ready().await;
            }
        }
    }

    /// Check if a request is allowed immediately.
    pub fn check(&self, key: Option<&str>) -> bool {
        match self {
            MaybeRateLimiter::None => true,
            MaybeRateLimiter::PerInstance(limiter) => limiter.check(),
            MaybeRateLimiter::Shared(shared) => {
                let key = key.unwrap_or("default");
                let limiter = shared.get_or_create(key);
                limiter.check()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::Instant;

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = RateLimiter::new(10, 20);
        assert!(limiter.check());
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_burst() {
        let limiter = RateLimiter::new(10, 5);

        // Should allow burst of 5 requests immediately
        for _ in 0..5 {
            assert!(limiter.check(), "Burst request should be allowed");
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_enforces_limit() {
        let limiter = RateLimiter::new(10, 2);

        // Use up burst
        assert!(limiter.check());
        assert!(limiter.check());

        // Next request should not be allowed immediately
        assert!(!limiter.check(), "Request beyond burst should be denied");

        // But should be allowed after waiting
        let start = Instant::now();
        limiter.until_ready().await;
        let elapsed = start.elapsed();

        // Should have waited approximately 100ms (1/10 second)
        assert!(
            elapsed >= Duration::from_millis(50),
            "Should have waited for rate limit"
        );
    }

    #[tokio::test]
    async fn test_shared_rate_limiters() {
        let config = RateLimitConfig::shared(10, 5);
        let shared = SharedRateLimiters::new(config);

        let limiter1 = shared.get_or_create("key1");
        let limiter2 = shared.get_or_create("key1");

        // Should get the same limiter for the same key
        for _ in 0..5 {
            assert!(limiter1.check());
        }

        // Second limiter should share the same quota
        assert!(!limiter2.check(), "Shared limiter should share quota");
    }

    #[tokio::test]
    async fn test_maybe_rate_limiter_disabled() {
        let config = RateLimitConfig::disabled();
        let maybe = MaybeRateLimiter::from_config(&config);

        // Should allow unlimited requests
        for _ in 0..1000 {
            assert!(maybe.check(None));
        }
    }

    #[tokio::test]
    async fn test_maybe_rate_limiter_per_instance() {
        let config = RateLimitConfig::new(10, 2);
        let maybe = MaybeRateLimiter::from_config(&config);

        // Should allow burst
        assert!(maybe.check(None));
        assert!(maybe.check(None));

        // Should deny next immediate request
        assert!(!maybe.check(None));
    }

    #[tokio::test]
    async fn test_maybe_rate_limiter_shared() {
        let config = RateLimitConfig::shared(10, 2);
        let maybe = MaybeRateLimiter::from_config(&config);

        // Should allow burst for key1
        assert!(maybe.check(Some("key1")));
        assert!(maybe.check(Some("key1")));
        assert!(!maybe.check(Some("key1")));

        // Should allow burst for key2 (different limiter)
        assert!(maybe.check(Some("key2")));
        assert!(maybe.check(Some("key2")));
    }
}
