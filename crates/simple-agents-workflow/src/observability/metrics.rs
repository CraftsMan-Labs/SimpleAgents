use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Worker health state used by metrics adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerHealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Prometheus-friendly workflow metrics surface.
pub trait WorkflowMetrics: Send + Sync {
    fn observe_node_latency(&self, workflow_name: &str, node_id: &str, latency: Duration);
    fn increment_node_success(&self, workflow_name: &str, node_id: &str);
    fn increment_node_failure(&self, workflow_name: &str, node_id: &str);
    fn set_queue_depth(&self, queue_name: &str, depth: u64);
    fn set_worker_health(&self, worker_id: &str, health: WorkerHealthState);
}

/// No-op metrics adapter.
#[derive(Debug, Default)]
pub struct NoopWorkflowMetrics;

impl WorkflowMetrics for NoopWorkflowMetrics {
    fn observe_node_latency(&self, _workflow_name: &str, _node_id: &str, _latency: Duration) {}

    fn increment_node_success(&self, _workflow_name: &str, _node_id: &str) {}

    fn increment_node_failure(&self, _workflow_name: &str, _node_id: &str) {}

    fn set_queue_depth(&self, _queue_name: &str, _depth: u64) {}

    fn set_worker_health(&self, _worker_id: &str, _health: WorkerHealthState) {}
}

/// In-memory adapter useful for tests and lightweight local diagnostics.
#[derive(Debug, Clone, Default)]
pub struct InMemoryWorkflowMetrics {
    inner: Arc<Mutex<MetricsState>>,
}

#[derive(Debug, Default)]
struct MetricsState {
    success: BTreeMap<(String, String), u64>,
    failure: BTreeMap<(String, String), u64>,
    latency_ms: BTreeMap<(String, String), Vec<u128>>,
    queue_depth: BTreeMap<String, u64>,
    worker_health: BTreeMap<String, WorkerHealthState>,
}

impl InMemoryWorkflowMetrics {
    pub fn snapshot(&self) -> InMemoryMetricsSnapshot {
        let state = self.inner.lock().expect("metrics mutex poisoned");
        InMemoryMetricsSnapshot {
            success: state.success.clone(),
            failure: state.failure.clone(),
            latency_ms: state.latency_ms.clone(),
            queue_depth: state.queue_depth.clone(),
            worker_health: state.worker_health.clone(),
        }
    }
}

impl WorkflowMetrics for InMemoryWorkflowMetrics {
    fn observe_node_latency(&self, workflow_name: &str, node_id: &str, latency: Duration) {
        let mut state = self.inner.lock().expect("metrics mutex poisoned");
        state
            .latency_ms
            .entry((workflow_name.to_string(), node_id.to_string()))
            .or_default()
            .push(latency.as_millis());
    }

    fn increment_node_success(&self, workflow_name: &str, node_id: &str) {
        let mut state = self.inner.lock().expect("metrics mutex poisoned");
        *state
            .success
            .entry((workflow_name.to_string(), node_id.to_string()))
            .or_insert(0) += 1;
    }

    fn increment_node_failure(&self, workflow_name: &str, node_id: &str) {
        let mut state = self.inner.lock().expect("metrics mutex poisoned");
        *state
            .failure
            .entry((workflow_name.to_string(), node_id.to_string()))
            .or_insert(0) += 1;
    }

    fn set_queue_depth(&self, queue_name: &str, depth: u64) {
        let mut state = self.inner.lock().expect("metrics mutex poisoned");
        state.queue_depth.insert(queue_name.to_string(), depth);
    }

    fn set_worker_health(&self, worker_id: &str, health: WorkerHealthState) {
        let mut state = self.inner.lock().expect("metrics mutex poisoned");
        state.worker_health.insert(worker_id.to_string(), health);
    }
}

/// Snapshot view of in-memory metrics state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InMemoryMetricsSnapshot {
    pub success: BTreeMap<(String, String), u64>,
    pub failure: BTreeMap<(String, String), u64>,
    pub latency_ms: BTreeMap<(String, String), Vec<u128>>,
    pub queue_depth: BTreeMap<String, u64>,
    pub worker_health: BTreeMap<String, WorkerHealthState>,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{InMemoryWorkflowMetrics, WorkerHealthState, WorkflowMetrics};

    #[test]
    fn records_metrics_in_memory() {
        let metrics = InMemoryWorkflowMetrics::default();
        metrics.observe_node_latency("wf", "llm", Duration::from_millis(23));
        metrics.increment_node_success("wf", "llm");
        metrics.increment_node_failure("wf", "tool");
        metrics.set_queue_depth("worker-q", 7);
        metrics.set_worker_health("worker-1", WorkerHealthState::Degraded);

        let snap = metrics.snapshot();
        assert_eq!(
            snap.success.get(&("wf".to_string(), "llm".to_string())),
            Some(&1)
        );
        assert_eq!(
            snap.failure.get(&("wf".to_string(), "tool".to_string())),
            Some(&1)
        );
        assert_eq!(
            snap.latency_ms.get(&("wf".to_string(), "llm".to_string())),
            Some(&vec![23])
        );
        assert_eq!(snap.queue_depth.get("worker-q"), Some(&7));
        assert_eq!(
            snap.worker_health.get("worker-1"),
            Some(&WorkerHealthState::Degraded)
        );
    }
}
