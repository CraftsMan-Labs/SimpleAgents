use simple_agents_router::{HealthTracker, HealthTrackerConfig};
use simple_agent_type::prelude::ProviderHealth;
use std::time::Duration;

#[test]
fn health_tracker_uses_config_thresholds() {
    let config = HealthTrackerConfig {
        degrade_threshold: 0.5,
        unavailable_threshold: 0.9,
        latency_alpha: 0.2,
    };
    let tracker = HealthTracker::new(1, config);

    tracker.record_failure(0, Some(Duration::from_millis(10)));
    tracker.record_failure(0, Some(Duration::from_millis(10)));
    tracker.record_success(0, Duration::from_millis(10));

    let health = tracker.health(0).unwrap();
    assert_eq!(health, ProviderHealth::Degraded);
}
