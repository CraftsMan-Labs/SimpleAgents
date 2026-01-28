//! Circuit breaker implementation for provider resilience.
//!
//! Tracks provider failures and opens the circuit after a threshold,
//! then allows half-open probes after a cooldown.

use simple_agent_type::prelude::{ProviderError, SimpleAgentsError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Circuit breaker configuration.
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Cooldown before moving from open to half-open.
    pub open_duration: Duration,
    /// Number of consecutive successes to close from half-open.
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            open_duration: Duration::from_secs(10),
            success_threshold: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InternalState {
    Closed,
    Open { opened_at: Instant },
    HalfOpen,
}

/// Public circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Circuit is closed and requests flow normally.
    Closed,
    /// Circuit is open and requests should be rejected.
    Open,
    /// Circuit is half-open and probing for recovery.
    HalfOpen,
}

#[derive(Debug)]
struct CircuitBreakerInner {
    state: InternalState,
    consecutive_failures: u32,
    consecutive_successes: u32,
}

impl CircuitBreakerInner {
    fn new() -> Self {
        Self {
            state: InternalState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
        }
    }
}

/// Circuit breaker for a provider.
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    inner: Mutex<CircuitBreakerInner>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(CircuitBreakerInner::new()),
        }
    }

    /// Check if the circuit allows a request.
    pub fn allow_request(&self) -> bool {
        let mut inner = self.inner.lock().expect("circuit breaker lock poisoned");
        match inner.state {
            InternalState::Closed => true,
            InternalState::Open { opened_at } => {
                if opened_at.elapsed() >= self.config.open_duration {
                    inner.state = InternalState::HalfOpen;
                    inner.consecutive_successes = 0;
                    inner.consecutive_failures = 0;
                    true
                } else {
                    false
                }
            }
            InternalState::HalfOpen => true,
        }
    }

    /// Record a successful request.
    pub fn record_success(&self) {
        let mut inner = self.inner.lock().expect("circuit breaker lock poisoned");
        match inner.state {
            InternalState::Closed => {
                inner.consecutive_failures = 0;
            }
            InternalState::HalfOpen => {
                inner.consecutive_successes = inner.consecutive_successes.saturating_add(1);
                if inner.consecutive_successes >= self.config.success_threshold {
                    inner.state = InternalState::Closed;
                    inner.consecutive_failures = 0;
                    inner.consecutive_successes = 0;
                }
            }
            InternalState::Open { .. } => {}
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        let mut inner = self.inner.lock().expect("circuit breaker lock poisoned");
        match inner.state {
            InternalState::Closed => {
                inner.consecutive_failures = inner.consecutive_failures.saturating_add(1);
                if inner.consecutive_failures >= self.config.failure_threshold {
                    inner.state = InternalState::Open {
                        opened_at: Instant::now(),
                    };
                }
            }
            InternalState::HalfOpen => {
                inner.state = InternalState::Open {
                    opened_at: Instant::now(),
                };
                inner.consecutive_failures = 1;
                inner.consecutive_successes = 0;
            }
            InternalState::Open { .. } => {}
        }
    }

    /// Record a result to update circuit state.
    pub fn record_result(&self, result: &std::result::Result<(), SimpleAgentsError>) {
        match result {
            Ok(_) => self.record_success(),
            Err(error) => {
                if matches!(
                    error,
                    SimpleAgentsError::Provider(
                        ProviderError::RateLimit { .. }
                            | ProviderError::Timeout(_)
                            | ProviderError::ServerError(_)
                    ) | SimpleAgentsError::Network(_)
                ) {
                    self.record_failure();
                }
            }
        }
    }

    /// Current circuit state.
    pub fn state(&self) -> CircuitBreakerState {
        let inner = self.inner.lock().expect("circuit breaker lock poisoned");
        match inner.state {
            InternalState::Closed => CircuitBreakerState::Closed,
            InternalState::Open { .. } => CircuitBreakerState::Open,
            InternalState::HalfOpen => CircuitBreakerState::HalfOpen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_after_failures() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            open_duration: Duration::from_secs(10),
            success_threshold: 1,
        });

        assert!(breaker.allow_request());
        breaker.record_failure();
        assert!(breaker.allow_request());
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitBreakerState::Open);
        assert!(!breaker.allow_request());
    }

    #[test]
    fn closes_after_success_in_half_open() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            open_duration: Duration::from_millis(0),
            success_threshold: 1,
        });

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitBreakerState::Open);
        assert!(breaker.allow_request());
        assert_eq!(breaker.state(), CircuitBreakerState::HalfOpen);
        breaker.record_success();
        assert_eq!(breaker.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn reopens_on_failure_in_half_open() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            open_duration: Duration::from_millis(0),
            success_threshold: 2,
        });

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitBreakerState::Open);
        assert!(breaker.allow_request());
        assert_eq!(breaker.state(), CircuitBreakerState::HalfOpen);
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitBreakerState::Open);
    }
}
