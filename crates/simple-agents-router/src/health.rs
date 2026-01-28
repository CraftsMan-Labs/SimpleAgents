//! Provider health tracking for routing decisions.
//!
//! Maintains per-provider metrics and health state.

use simple_agent_type::prelude::{ProviderHealth, ProviderMetrics};
use std::sync::Mutex;
use std::time::Duration;

/// Configuration for health tracking.
#[derive(Debug, Clone, Copy)]
pub struct HealthTrackerConfig {
    /// Failure rate above which providers are degraded.
    pub degrade_threshold: f32,
    /// Failure rate above which providers are marked unavailable.
    pub unavailable_threshold: f32,
    /// Exponential moving average factor for latency.
    pub latency_alpha: f64,
}

impl Default for HealthTrackerConfig {
    fn default() -> Self {
        Self {
            degrade_threshold: 0.2,
            unavailable_threshold: 0.5,
            latency_alpha: 0.2,
        }
    }
}

/// Health tracker for providers.
#[derive(Debug)]
pub struct HealthTracker {
    metrics: Mutex<Vec<ProviderMetrics>>,
    config: HealthTrackerConfig,
}

impl HealthTracker {
    /// Create a tracker for the given number of providers.
    pub fn new(provider_count: usize, config: HealthTrackerConfig) -> Self {
        let metrics = vec![ProviderMetrics::default(); provider_count];
        Self {
            metrics: Mutex::new(metrics),
            config,
        }
    }

    /// Record a successful request.
    pub fn record_success(&self, provider_index: usize, latency: Duration) {
        let mut metrics = self.metrics.lock().expect("health tracker lock poisoned");
        if let Some(entry) = metrics.get_mut(provider_index) {
            entry.total_requests = entry.total_requests.saturating_add(1);
            entry.successful_requests = entry.successful_requests.saturating_add(1);
            entry.avg_latency =
                update_latency(entry.avg_latency, latency, self.config.latency_alpha);
            entry.health = compute_health_with_config(entry, self.config);
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self, provider_index: usize, latency: Option<Duration>) {
        let mut metrics = self.metrics.lock().expect("health tracker lock poisoned");
        if let Some(entry) = metrics.get_mut(provider_index) {
            entry.total_requests = entry.total_requests.saturating_add(1);
            entry.failed_requests = entry.failed_requests.saturating_add(1);
            if let Some(value) = latency {
                entry.avg_latency =
                    update_latency(entry.avg_latency, value, self.config.latency_alpha);
            }
            entry.health = compute_health_with_config(entry, self.config);
        }
    }

    /// Get metrics for a provider.
    pub fn metrics(&self, provider_index: usize) -> Option<ProviderMetrics> {
        let metrics = self.metrics.lock().expect("health tracker lock poisoned");
        metrics.get(provider_index).copied()
    }

    /// Get health for a provider.
    pub fn health(&self, provider_index: usize) -> Option<ProviderHealth> {
        self.metrics(provider_index).map(|entry| entry.health)
    }
}

fn update_latency(current: Duration, new_value: Duration, alpha: f64) -> Duration {
    if current.as_millis() == 0 {
        return new_value;
    }
    let current_ms = current.as_secs_f64() * 1000.0;
    let new_ms = new_value.as_secs_f64() * 1000.0;
    let ema = (alpha * new_ms) + ((1.0 - alpha) * current_ms);
    Duration::from_millis(ema.max(0.0) as u64)
}

fn compute_health_with_config(
    metrics: &ProviderMetrics,
    config: HealthTrackerConfig,
) -> ProviderHealth {
    let failure_rate = metrics.failure_rate();
    if failure_rate >= config.unavailable_threshold {
        ProviderHealth::Unavailable
    } else if failure_rate >= config.degrade_threshold {
        ProviderHealth::Degraded
    } else {
        ProviderHealth::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_updates_metrics() {
        let tracker = HealthTracker::new(1, HealthTrackerConfig::default());
        tracker.record_success(0, Duration::from_millis(100));
        let metrics = tracker.metrics(0).unwrap();
        assert_eq!(metrics.total_requests, 1);
        assert_eq!(metrics.successful_requests, 1);
        assert_eq!(metrics.failed_requests, 0);
        assert_eq!(metrics.health, ProviderHealth::Healthy);
    }

    #[test]
    fn failures_degrade_health() {
        let config = HealthTrackerConfig {
            degrade_threshold: 0.2,
            unavailable_threshold: 0.5,
            latency_alpha: 0.2,
        };
        let tracker = HealthTracker::new(1, config);

        tracker.record_failure(0, Some(Duration::from_millis(50)));
        tracker.record_failure(0, Some(Duration::from_millis(50)));
        tracker.record_success(0, Duration::from_millis(50));
        tracker.record_failure(0, Some(Duration::from_millis(50)));

        let metrics = tracker.metrics(0).unwrap();
        assert_eq!(metrics.total_requests, 4);
        assert_eq!(metrics.failed_requests, 3);
        assert_eq!(metrics.health, ProviderHealth::Unavailable);
    }

    #[test]
    fn metrics_out_of_range_returns_none() {
        let tracker = HealthTracker::new(1, HealthTrackerConfig::default());
        assert!(tracker.metrics(5).is_none());
        assert!(tracker.health(2).is_none());
    }
}
