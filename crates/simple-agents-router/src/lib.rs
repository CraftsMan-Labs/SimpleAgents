//! Routing implementations for SimpleAgents.
//!
//! Provides routers that coordinate multiple providers with different
//! selection strategies.

mod circuit_breaker;
mod cost;
mod fallback;
mod health;
mod latency;
mod retry;
mod round_robin;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState};
pub use cost::{CostRouter, CostRouterConfig, ProviderCost};
pub use fallback::{FallbackRouter, FallbackRouterConfig};
pub use health::{HealthTracker, HealthTrackerConfig};
pub use latency::{LatencyRouter, LatencyRouterConfig};
pub use retry::{execute_with_retry, RetryPolicy};
pub use round_robin::RoundRobinRouter;
