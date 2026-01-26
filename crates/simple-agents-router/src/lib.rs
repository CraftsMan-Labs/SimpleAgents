//! Routing implementations for SimpleAgents.
//!
//! Provides routers that coordinate multiple providers with different
//! selection strategies.

mod round_robin;
mod latency;
mod cost;
mod fallback;
mod circuit_breaker;
mod health;
mod retry;

pub use round_robin::RoundRobinRouter;
pub use latency::{LatencyRouter, LatencyRouterConfig};
pub use cost::{CostRouter, CostRouterConfig, ProviderCost};
pub use fallback::{FallbackRouter, FallbackRouterConfig};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState};
pub use health::{HealthTracker, HealthTrackerConfig};
pub use retry::{execute_with_retry, RetryPolicy};
