//! Routing strategy trait for provider selection.
//!
//! Provides abstractions for routing requests across multiple providers.

use crate::config::ProviderConfig;
use crate::error::Result;
use crate::request::CompletionRequest;
use async_trait::async_trait;
use std::time::Duration;

/// Trait for routing strategies.
///
/// Implementations define how requests are routed across multiple providers:
/// - Round-robin: Distribute requests evenly
/// - Priority: Try providers in order
/// - Latency-based: Route to fastest provider
/// - Load-balancing: Consider provider load
/// - Cost-optimized: Route to cheapest provider
///
/// # Example Implementation
///
/// ```ignore
/// use simple_agents_types::router::RoutingStrategy;
/// use simple_agents_types::config::ProviderConfig;
/// use simple_agents_types::request::CompletionRequest;
/// use simple_agents_types::error::Result;
/// use async_trait::async_trait;
///
/// struct RoundRobinStrategy {
///     counter: AtomicUsize,
/// }
///
/// #[async_trait]
/// impl RoutingStrategy for RoundRobinStrategy {
///     async fn select_provider(
///         &self,
///         providers: &[ProviderConfig],
///         _request: &CompletionRequest,
///     ) -> Result<usize> {
///         if providers.is_empty() {
///             return Err(/* ... */);
///         }
///         let index = self.counter.fetch_add(1, Ordering::Relaxed);
///         Ok(index % providers.len())
///     }
/// }
/// ```
#[async_trait]
pub trait RoutingStrategy: Send + Sync {
    /// Select a provider index for the given request.
    ///
    /// # Arguments
    /// - `providers`: Available providers
    /// - `request`: The completion request
    ///
    /// # Returns
    /// Index of the selected provider in the `providers` slice.
    ///
    /// # Errors
    /// - If no suitable provider is found
    /// - If all providers are unavailable
    async fn select_provider(
        &self,
        providers: &[ProviderConfig],
        request: &CompletionRequest,
    ) -> Result<usize>;

    /// Report successful request completion.
    ///
    /// Used by latency-based and adaptive routing strategies to track
    /// provider performance.
    ///
    /// # Arguments
    /// - `provider_index`: Index of the provider that succeeded
    /// - `latency`: Request duration
    async fn report_success(&self, provider_index: usize, latency: Duration) {
        let _ = (provider_index, latency);
    }

    /// Report request failure.
    ///
    /// Used by reliability-tracking routing strategies.
    ///
    /// # Arguments
    /// - `provider_index`: Index of the provider that failed
    async fn report_failure(&self, provider_index: usize) {
        let _ = provider_index;
    }

    /// Get strategy name (for logging/debugging).
    fn name(&self) -> &str {
        "routing-strategy"
    }
}

/// Routing mode enum for common strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoutingMode {
    /// Try providers in priority order
    Priority,
    /// Distribute requests evenly (round-robin)
    RoundRobin,
    /// Route to provider with lowest latency
    LatencyBased,
    /// Random selection
    Random,
}

impl RoutingMode {
    /// Get a human-readable description.
    pub fn description(&self) -> &str {
        match self {
            Self::Priority => "Try providers in priority order",
            Self::RoundRobin => "Distribute requests evenly across providers",
            Self::LatencyBased => "Route to provider with lowest average latency",
            Self::Random => "Randomly select provider",
        }
    }
}

/// Provider health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderHealth {
    /// Provider is healthy
    Healthy,
    /// Provider is degraded (high error rate)
    Degraded,
    /// Provider is unavailable
    Unavailable,
}

impl ProviderHealth {
    /// Check if provider can be used.
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

/// Provider metrics for routing decisions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProviderMetrics {
    /// Total requests sent
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Average latency
    pub avg_latency: Duration,
    /// Current health status
    pub health: ProviderHealth,
}

impl Default for ProviderMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_latency: Duration::from_millis(0),
            health: ProviderHealth::Healthy,
        }
    }
}

impl ProviderMetrics {
    /// Calculate success rate (0.0-1.0).
    pub fn success_rate(&self) -> f32 {
        if self.total_requests == 0 {
            return 1.0;
        }
        self.successful_requests as f32 / self.total_requests as f32
    }

    /// Calculate failure rate (0.0-1.0).
    pub fn failure_rate(&self) -> f32 {
        1.0 - self.success_rate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_mode_description() {
        assert!(!RoutingMode::Priority.description().is_empty());
        assert!(!RoutingMode::RoundRobin.description().is_empty());
        assert!(!RoutingMode::LatencyBased.description().is_empty());
        assert!(!RoutingMode::Random.description().is_empty());
    }

    #[test]
    fn test_provider_health_is_available() {
        assert!(ProviderHealth::Healthy.is_available());
        assert!(ProviderHealth::Degraded.is_available());
        assert!(!ProviderHealth::Unavailable.is_available());
    }

    #[test]
    fn test_provider_metrics_default() {
        let metrics = ProviderMetrics::default();
        assert_eq!(metrics.total_requests, 0);
        assert_eq!(metrics.successful_requests, 0);
        assert_eq!(metrics.failed_requests, 0);
        assert_eq!(metrics.success_rate(), 1.0);
        assert_eq!(metrics.failure_rate(), 0.0);
    }

    #[test]
    fn test_provider_metrics_success_rate() {
        let metrics = ProviderMetrics {
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            avg_latency: Duration::from_millis(200),
            health: ProviderHealth::Healthy,
        };

        assert!((metrics.success_rate() - 0.95).abs() < 0.001);
        assert!((metrics.failure_rate() - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_provider_metrics_zero_requests() {
        let metrics = ProviderMetrics {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_latency: Duration::from_millis(0),
            health: ProviderHealth::Healthy,
        };

        // Default to 100% success rate when no data
        assert_eq!(metrics.success_rate(), 1.0);
        assert_eq!(metrics.failure_rate(), 0.0);
    }

    // Test that RoutingStrategy trait is object-safe
    #[test]
    fn test_routing_strategy_object_safety() {
        fn _assert_object_safe(_: &dyn RoutingStrategy) {}
    }
}
