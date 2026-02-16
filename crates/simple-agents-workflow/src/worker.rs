use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// Worker protocol request payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerRequest {
    /// Stable request identifier supplied by the workflow runtime.
    pub request_id: String,
    /// Workflow name associated with this request.
    pub workflow_name: String,
    /// Node id associated with this request.
    pub node_id: String,
    /// Optional execution timeout budget.
    pub timeout_ms: Option<u64>,
    /// Worker operation to execute.
    pub operation: WorkerOperation,
}

/// Worker protocol operation variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkerOperation {
    /// Execute an LLM-like operation against worker host.
    Llm {
        /// Model name.
        model: String,
        /// Prompt text.
        prompt: String,
        /// Deterministic scoped input.
        scoped_input: Value,
    },
    /// Execute a tool operation against worker host.
    Tool {
        /// Tool name.
        tool: String,
        /// Tool node static input payload.
        input: Value,
        /// Deterministic scoped input.
        scoped_input: Value,
    },
}

/// Worker protocol response payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerResponse {
    /// Request identifier for correlation.
    pub request_id: String,
    /// Worker id that served the request.
    pub worker_id: String,
    /// Result payload.
    pub result: WorkerResult,
    /// Wall-clock execution latency.
    pub elapsed_ms: u64,
}

/// Worker protocol result variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WorkerResult {
    /// Successful request execution payload.
    Success {
        /// Worker-produced output.
        output: Value,
    },
    /// Failed request execution payload.
    Error {
        /// Structured worker error.
        error: WorkerProtocolError,
    },
}

/// Protocol-level error payload returned by workers and pool guards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerProtocolError {
    /// Programmatic error code.
    pub code: WorkerErrorCode,
    /// Human-readable diagnostic.
    pub message: String,
    /// Indicates whether a caller can retry safely.
    pub retryable: bool,
}

/// Stable worker error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerErrorCode {
    /// Request could not be accepted by the pool queue.
    QueueFull,
    /// Worker was unavailable.
    Unavailable,
    /// Request timed out.
    Timeout,
    /// Request failed during worker execution.
    ExecutionFailed,
    /// Request rejected by circuit breaker hooks.
    CircuitOpen,
    /// Request cancelled before completion.
    Cancelled,
    /// Request violated runtime security policy.
    InvalidRequest,
}

/// Pool-level errors surfaced to runtime callers.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkerPoolError {
    /// Worker queue reached configured limit.
    #[error("worker queue is full")]
    QueueFull,
    /// No healthy worker could accept the request.
    #[error("no healthy worker available")]
    NoHealthyWorker,
    /// Worker-side execution failed.
    #[error("worker execution failed: {0:?}")]
    Worker(WorkerProtocolError),
    /// Request timed out while waiting for worker completion.
    #[error("worker request timed out")]
    Timeout,
    /// Request was cancelled because pool is shutting down.
    #[error("worker pool is shutting down")]
    ShuttingDown,
    /// Request rejected by circuit breaker hook.
    #[error("request rejected by circuit breaker")]
    CircuitOpen,
    /// Request violated runtime security policy.
    #[error("worker request rejected: {reason}")]
    InvalidRequest {
        /// Rejection reason.
        reason: String,
    },
}

/// Worker lifecycle and scheduling configuration.
#[derive(Debug, Clone)]
pub struct WorkerPoolOptions {
    /// Maximum queue depth per worker.
    pub queue_capacity: usize,
    /// Interval between worker health probes.
    pub health_probe_interval: Duration,
    /// Consecutive failures needed before marking worker unavailable.
    pub unavailable_after_failures: u32,
    /// Default timeout used when request timeout is not set.
    pub default_request_timeout: Option<Duration>,
    /// Security guards for request payload and budgets.
    pub security_policy: WorkerSecurityPolicy,
}

/// Request hardening limits for worker pool submit path.
#[derive(Debug, Clone)]
pub struct WorkerSecurityPolicy {
    /// Maximum request timeout accepted by pool.
    pub max_request_timeout_ms: u64,
    /// Maximum serialized request payload size in bytes.
    pub max_request_payload_bytes: usize,
    /// Maximum length for request/workflow/node identifiers.
    pub max_identifier_length: usize,
}

impl Default for WorkerPoolOptions {
    fn default() -> Self {
        Self {
            queue_capacity: 64,
            health_probe_interval: Duration::from_secs(5),
            unavailable_after_failures: 3,
            default_request_timeout: Some(Duration::from_secs(30)),
            security_policy: WorkerSecurityPolicy::default(),
        }
    }
}

impl Default for WorkerSecurityPolicy {
    fn default() -> Self {
        Self {
            max_request_timeout_ms: 120_000,
            max_request_payload_bytes: 256 * 1024,
            max_identifier_length: 128,
        }
    }
}

/// Health state for one pool worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHealth {
    /// Worker id.
    pub worker_id: String,
    /// Current status.
    pub status: WorkerHealthStatus,
    /// Number of consecutive failures observed.
    pub consecutive_failures: u32,
    /// Last probe timestamp in unix milliseconds.
    pub last_probe_unix_ms: Option<u64>,
}

/// Coarse worker health statuses used by scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerHealthStatus {
    /// Worker is healthy and can receive traffic.
    Healthy,
    /// Worker has transient failures and may still receive traffic.
    Degraded,
    /// Worker is unavailable and should not receive traffic.
    Unavailable,
}

impl WorkerHealth {
    fn new(worker_id: String) -> Self {
        Self {
            worker_id,
            status: WorkerHealthStatus::Healthy,
            consecutive_failures: 0,
            last_probe_unix_ms: None,
        }
    }

    fn is_schedulable(&self) -> bool {
        !matches!(self.status, WorkerHealthStatus::Unavailable)
    }
}

/// Host-implemented worker behavior for in-process worker pools.
#[async_trait]
pub trait WorkerHandler: Send + Sync {
    /// Handles one worker protocol request.
    async fn handle(&self, request: WorkerRequest) -> Result<Value, WorkerProtocolError>;

    /// Performs worker health probe.
    async fn probe_health(&self) -> WorkerHealthStatus {
        WorkerHealthStatus::Healthy
    }
}

/// Optional hook surface for circuit breaker integration.
#[async_trait]
pub trait CircuitBreakerHooks: Send + Sync {
    /// Returns false to reject a request before queueing.
    fn allow_request(&self, _worker_id: &str, _request: &WorkerRequest) -> bool {
        true
    }

    /// Called after a request is accepted for execution.
    async fn on_request_accepted(&self, _worker_id: &str, _request: &WorkerRequest) {}

    /// Called after a request succeeds.
    async fn on_request_success(&self, _worker_id: &str, _response: &WorkerResponse) {}

    /// Called after a request fails.
    async fn on_request_failure(&self, _worker_id: &str, _error: &WorkerProtocolError) {}

    /// Called after a request is rejected.
    async fn on_request_rejected(
        &self,
        _worker_id: Option<&str>,
        _request: &WorkerRequest,
        _reason: WorkerErrorCode,
    ) {
    }
}

struct WorkItem {
    request: WorkerRequest,
    response_tx: oneshot::Sender<Result<WorkerResponse, WorkerPoolError>>,
}

struct WorkerSlot {
    worker_id: String,
    sender: mpsc::Sender<WorkItem>,
    shutdown_tx: watch::Sender<bool>,
    worker_task: JoinHandle<()>,
    probe_task: JoinHandle<()>,
    health: Arc<RwLock<WorkerHealth>>,
    handler: Arc<dyn WorkerHandler>,
}

/// In-process worker pool with bounded queues and health-aware routing.
pub struct WorkerPool {
    options: WorkerPoolOptions,
    slots: Mutex<Vec<WorkerSlot>>,
    next_worker: AtomicUsize,
    hooks: Option<Arc<dyn CircuitBreakerHooks>>,
}

/// Adapter trait for worker pools used by runtime integrations.
#[async_trait]
pub trait WorkerPoolClient: Send + Sync {
    /// Submits one request to the underlying worker pool.
    async fn submit(&self, request: WorkerRequest) -> Result<WorkerResponse, WorkerPoolError>;

    /// Returns a snapshot of worker health state.
    async fn health_snapshot(&self) -> Vec<WorkerHealth>;
}

#[async_trait]
impl WorkerPoolClient for WorkerPool {
    async fn submit(&self, request: WorkerRequest) -> Result<WorkerResponse, WorkerPoolError> {
        WorkerPool::submit(self, request).await
    }

    async fn health_snapshot(&self) -> Vec<WorkerHealth> {
        WorkerPool::health_snapshot(self).await
    }
}

impl WorkerPool {
    /// Creates and starts an in-process worker pool.
    pub fn new_inprocess(
        handlers: Vec<Arc<dyn WorkerHandler>>,
        options: WorkerPoolOptions,
        hooks: Option<Arc<dyn CircuitBreakerHooks>>,
    ) -> Result<Self, WorkerPoolError> {
        if handlers.is_empty() {
            return Err(WorkerPoolError::NoHealthyWorker);
        }

        let mut slots = Vec::with_capacity(handlers.len());
        for (index, handler) in handlers.into_iter().enumerate() {
            let worker_id = format!("worker-{}", index);
            slots.push(spawn_worker_slot(
                worker_id,
                handler,
                options.queue_capacity,
                options.health_probe_interval,
                options.unavailable_after_failures,
            ));
        }

        Ok(Self {
            options,
            slots: Mutex::new(slots),
            next_worker: AtomicUsize::new(0),
            hooks,
        })
    }

    /// Submits one request to the pool and waits for completion.
    pub async fn submit(&self, request: WorkerRequest) -> Result<WorkerResponse, WorkerPoolError> {
        validate_request_contract(&request, &self.options.security_policy)?;
        let (slot_index, worker_id, sender) = self.select_worker(&request).await?;

        if let Some(hooks) = &self.hooks {
            if !hooks.allow_request(&worker_id, &request) {
                hooks
                    .on_request_rejected(Some(&worker_id), &request, WorkerErrorCode::CircuitOpen)
                    .await;
                return Err(WorkerPoolError::CircuitOpen);
            }
            hooks.on_request_accepted(&worker_id, &request).await;
        }

        let (response_tx, response_rx) = oneshot::channel();
        let work_item = WorkItem {
            request: request.clone(),
            response_tx,
        };

        sender
            .try_send(work_item)
            .map_err(|_| WorkerPoolError::QueueFull)?;

        let timeout_budget = request
            .timeout_ms
            .map(Duration::from_millis)
            .or(self.options.default_request_timeout);

        let outcome = if let Some(duration) = timeout_budget {
            match timeout(duration, response_rx).await {
                Ok(result) => result,
                Err(_) => {
                    self.mark_unavailable(slot_index).await;
                    if let Some(hooks) = &self.hooks {
                        hooks
                            .on_request_rejected(
                                Some(&worker_id),
                                &request,
                                WorkerErrorCode::Timeout,
                            )
                            .await;
                    }
                    return Err(WorkerPoolError::Timeout);
                }
            }
        } else {
            response_rx.await
        };

        let response = outcome.map_err(|_| WorkerPoolError::ShuttingDown)??;

        if let Some(hooks) = &self.hooks {
            match &response.result {
                WorkerResult::Success { .. } => {
                    hooks.on_request_success(&worker_id, &response).await
                }
                WorkerResult::Error { error } => hooks.on_request_failure(&worker_id, error).await,
            }
        }

        match &response.result {
            WorkerResult::Success { .. } => Ok(response),
            WorkerResult::Error { error } => Err(WorkerPoolError::Worker(error.clone())),
        }
    }

    /// Returns current worker health snapshot.
    pub async fn health_snapshot(&self) -> Vec<WorkerHealth> {
        let (health_refs, worker_count) = {
            let slots = self.slots.lock().await;
            (
                slots
                    .iter()
                    .map(|slot| Arc::clone(&slot.health))
                    .collect::<Vec<_>>(),
                slots.len(),
            )
        };

        let mut snapshot = Vec::with_capacity(worker_count);
        for health in health_refs {
            snapshot.push(health.read().await.clone());
        }
        snapshot
    }

    /// Restarts one worker task in place and resets health counters.
    pub async fn restart_worker(&self, worker_id: &str) -> Result<(), WorkerPoolError> {
        let mut slots = self.slots.lock().await;
        let slot_index = slots
            .iter()
            .position(|slot| slot.worker_id == worker_id)
            .ok_or(WorkerPoolError::NoHealthyWorker)?;

        let old_slot = &slots[slot_index];
        let _ = old_slot.shutdown_tx.send(true);

        let replacement = spawn_worker_slot(
            worker_id.to_string(),
            Arc::clone(&old_slot.handler),
            self.options.queue_capacity,
            self.options.health_probe_interval,
            self.options.unavailable_after_failures,
        );
        slots[slot_index] = replacement;
        Ok(())
    }

    /// Gracefully shuts down all worker and probe tasks.
    pub async fn shutdown(&self) {
        let mut slots = self.slots.lock().await;
        for slot in slots.iter_mut() {
            let _ = slot.shutdown_tx.send(true);
            slot.worker_task.abort();
            slot.probe_task.abort();
        }
    }

    async fn select_worker(
        &self,
        request: &WorkerRequest,
    ) -> Result<(usize, String, mpsc::Sender<WorkItem>), WorkerPoolError> {
        let candidates = {
            let slots = self.slots.lock().await;
            if slots.is_empty() {
                Vec::new()
            } else {
                let start = self.next_worker.fetch_add(1, Ordering::Relaxed) % slots.len();
                let mut candidates = Vec::with_capacity(slots.len());
                for offset in 0..slots.len() {
                    let idx = (start + offset) % slots.len();
                    let slot = &slots[idx];
                    candidates.push((
                        idx,
                        slot.worker_id.clone(),
                        slot.sender.clone(),
                        Arc::clone(&slot.health),
                    ));
                }
                candidates
            }
        };

        if candidates.is_empty() {
            if let Some(hooks) = &self.hooks {
                hooks
                    .on_request_rejected(None, request, WorkerErrorCode::Unavailable)
                    .await;
            }
            return Err(WorkerPoolError::NoHealthyWorker);
        }

        for (idx, worker_id, sender, health_ref) in candidates {
            if health_ref.read().await.is_schedulable() {
                return Ok((idx, worker_id, sender));
            }
        }

        if let Some(hooks) = &self.hooks {
            hooks
                .on_request_rejected(None, request, WorkerErrorCode::Unavailable)
                .await;
        }
        Err(WorkerPoolError::NoHealthyWorker)
    }

    async fn mark_unavailable(&self, slot_index: usize) {
        let health_ref = {
            let slots = self.slots.lock().await;
            slots.get(slot_index).map(|slot| Arc::clone(&slot.health))
        };

        if let Some(health_ref) = health_ref {
            let mut health = health_ref.write().await;
            health.status = WorkerHealthStatus::Unavailable;
            health.consecutive_failures = health.consecutive_failures.saturating_add(1);
            health.last_probe_unix_ms = Some(now_unix_ms());
        }
    }
}

fn spawn_worker_slot(
    worker_id: String,
    handler: Arc<dyn WorkerHandler>,
    queue_capacity: usize,
    probe_interval: Duration,
    unavailable_after_failures: u32,
) -> WorkerSlot {
    let (sender, mut receiver) = mpsc::channel::<WorkItem>(queue_capacity);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let health = Arc::new(RwLock::new(WorkerHealth::new(worker_id.clone())));

    let worker_id_for_loop = worker_id.clone();
    let handler_for_loop = Arc::clone(&handler);
    let health_for_loop = Arc::clone(&health);
    let mut shutdown_worker_rx = shutdown_rx.clone();
    let worker_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                maybe_item = receiver.recv() => {
                    let Some(item) = maybe_item else {
                        break;
                    };

                    let started = std::time::Instant::now();
                    let result = handler_for_loop.handle(item.request.clone()).await;
                    let elapsed_ms = started.elapsed().as_millis() as u64;
                    let response = match result {
                        Ok(output) => {
                            update_health_on_success(&health_for_loop).await;
                            WorkerResponse {
                                request_id: item.request.request_id.clone(),
                                worker_id: worker_id_for_loop.clone(),
                                result: WorkerResult::Success { output },
                                elapsed_ms,
                            }
                        }
                        Err(error) => {
                            update_health_on_failure(
                                &health_for_loop,
                                unavailable_after_failures,
                            )
                            .await;
                            WorkerResponse {
                                request_id: item.request.request_id.clone(),
                                worker_id: worker_id_for_loop.clone(),
                                result: WorkerResult::Error { error },
                                elapsed_ms,
                            }
                        }
                    };
                    let _ = item.response_tx.send(Ok(response));
                }
                changed = shutdown_worker_rx.changed() => {
                    if changed.is_ok() && *shutdown_worker_rx.borrow() {
                        break;
                    }
                }
            }
        }
    });

    let worker_id_for_probe = worker_id.clone();
    let handler_for_probe = Arc::clone(&handler);
    let health_for_probe = Arc::clone(&health);
    let mut shutdown_probe_rx = shutdown_rx.clone();
    let probe_task = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(probe_interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let status = handler_for_probe.probe_health().await;
                    let mut health = health_for_probe.write().await;
                    health.status = status;
                    if status == WorkerHealthStatus::Healthy {
                        health.consecutive_failures = 0;
                    }
                    health.last_probe_unix_ms = Some(now_unix_ms());
                }
                changed = shutdown_probe_rx.changed() => {
                    if changed.is_ok() && *shutdown_probe_rx.borrow() {
                        break;
                    }
                }
            }
        }
        let mut health = health_for_probe.write().await;
        health.status = WorkerHealthStatus::Unavailable;
        health.last_probe_unix_ms = Some(now_unix_ms());
        health.worker_id = worker_id_for_probe;
    });

    WorkerSlot {
        worker_id,
        sender,
        shutdown_tx,
        worker_task,
        probe_task,
        health,
        handler,
    }
}

async fn update_health_on_success(health_ref: &Arc<RwLock<WorkerHealth>>) {
    let mut health = health_ref.write().await;
    health.status = WorkerHealthStatus::Healthy;
    health.consecutive_failures = 0;
    health.last_probe_unix_ms = Some(now_unix_ms());
}

async fn update_health_on_failure(
    health_ref: &Arc<RwLock<WorkerHealth>>,
    unavailable_after_failures: u32,
) {
    let mut health = health_ref.write().await;
    health.consecutive_failures = health.consecutive_failures.saturating_add(1);
    health.status = if health.consecutive_failures >= unavailable_after_failures {
        WorkerHealthStatus::Unavailable
    } else {
        WorkerHealthStatus::Degraded
    };
    health.last_probe_unix_ms = Some(now_unix_ms());
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn validate_request_contract(
    request: &WorkerRequest,
    policy: &WorkerSecurityPolicy,
) -> Result<(), WorkerPoolError> {
    if request.request_id.len() > policy.max_identifier_length {
        return Err(WorkerPoolError::InvalidRequest {
            reason: format!(
                "request_id length {} exceeds max {}",
                request.request_id.len(),
                policy.max_identifier_length
            ),
        });
    }
    if request.workflow_name.len() > policy.max_identifier_length {
        return Err(WorkerPoolError::InvalidRequest {
            reason: format!(
                "workflow_name length {} exceeds max {}",
                request.workflow_name.len(),
                policy.max_identifier_length
            ),
        });
    }
    if request.node_id.len() > policy.max_identifier_length {
        return Err(WorkerPoolError::InvalidRequest {
            reason: format!(
                "node_id length {} exceeds max {}",
                request.node_id.len(),
                policy.max_identifier_length
            ),
        });
    }
    if let Some(timeout_ms) = request.timeout_ms {
        if timeout_ms > policy.max_request_timeout_ms {
            return Err(WorkerPoolError::InvalidRequest {
                reason: format!(
                    "timeout_ms {} exceeds max {}",
                    timeout_ms, policy.max_request_timeout_ms
                ),
            });
        }
    }

    let payload_size = estimate_payload_size(request);
    if payload_size > policy.max_request_payload_bytes {
        return Err(WorkerPoolError::InvalidRequest {
            reason: format!(
                "request payload {} bytes exceeds max {}",
                payload_size, policy.max_request_payload_bytes
            ),
        });
    }
    Ok(())
}

fn estimate_payload_size(request: &WorkerRequest) -> usize {
    serde_json::to_vec(request)
        .map(|payload| payload.len())
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use serde_json::json;
    use tokio::time::sleep;

    use super::*;

    struct EchoWorker;

    #[async_trait]
    impl WorkerHandler for EchoWorker {
        async fn handle(&self, request: WorkerRequest) -> Result<Value, WorkerProtocolError> {
            Ok(json!({"node": request.node_id}))
        }
    }

    struct SlowWorker {
        delay: Duration,
    }

    #[async_trait]
    impl WorkerHandler for SlowWorker {
        async fn handle(&self, _request: WorkerRequest) -> Result<Value, WorkerProtocolError> {
            sleep(self.delay).await;
            Ok(json!({"status": "ok"}))
        }
    }

    struct FlakyWorker {
        available: AtomicBool,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl WorkerHandler for FlakyWorker {
        async fn handle(&self, _request: WorkerRequest) -> Result<Value, WorkerProtocolError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.available.load(Ordering::Relaxed) {
                Ok(json!({"status": "up"}))
            } else {
                Err(WorkerProtocolError {
                    code: WorkerErrorCode::Unavailable,
                    message: "worker unavailable".to_string(),
                    retryable: true,
                })
            }
        }

        async fn probe_health(&self) -> WorkerHealthStatus {
            if self.available.load(Ordering::Relaxed) {
                WorkerHealthStatus::Healthy
            } else {
                WorkerHealthStatus::Unavailable
            }
        }
    }

    fn sample_request(id: &str) -> WorkerRequest {
        WorkerRequest {
            request_id: id.to_string(),
            workflow_name: "wf".to_string(),
            node_id: "node-1".to_string(),
            timeout_ms: None,
            operation: WorkerOperation::Tool {
                tool: "echo".to_string(),
                input: json!({"x": 1}),
                scoped_input: json!({"input": {}}),
            },
        }
    }

    #[test]
    fn worker_protocol_roundtrip() {
        let request = sample_request("req-1");
        let serialized =
            serde_json::to_string(&request).expect("request serialization should work");
        let decoded: WorkerRequest =
            serde_json::from_str(&serialized).expect("request deserialization should work");
        assert_eq!(request, decoded);
    }

    #[tokio::test]
    async fn routes_requests_across_worker_pool() {
        let pool = WorkerPool::new_inprocess(
            vec![Arc::new(EchoWorker), Arc::new(EchoWorker)],
            WorkerPoolOptions {
                queue_capacity: 4,
                health_probe_interval: Duration::from_millis(10),
                ..WorkerPoolOptions::default()
            },
            None,
        )
        .expect("pool should initialize");

        let response = pool
            .submit(sample_request("req-2"))
            .await
            .expect("request should succeed");
        assert_eq!(response.request_id, "req-2");
        assert_eq!(
            response.result,
            WorkerResult::Success {
                output: json!({"node": "node-1"})
            }
        );

        let health = pool.health_snapshot().await;
        assert_eq!(health.len(), 2);
        assert!(health.iter().all(|entry| entry.is_schedulable()));

        pool.shutdown().await;
    }

    #[tokio::test]
    async fn enforces_queue_backpressure_limits() {
        let pool = WorkerPool::new_inprocess(
            vec![Arc::new(SlowWorker {
                delay: Duration::from_millis(80),
            })],
            WorkerPoolOptions {
                queue_capacity: 1,
                health_probe_interval: Duration::from_millis(100),
                default_request_timeout: Some(Duration::from_secs(1)),
                ..WorkerPoolOptions::default()
            },
            None,
        )
        .expect("pool should initialize");

        let first = pool.submit(sample_request("q1"));
        let second = pool.submit(sample_request("q2"));
        let third = pool.submit(sample_request("q3"));

        let (first_result, second_result, third_result) = tokio::join!(first, second, third);
        let failures = [&first_result, &second_result, &third_result]
            .iter()
            .filter(|result| matches!(result, Err(WorkerPoolError::QueueFull)))
            .count();
        let successes = [&first_result, &second_result, &third_result]
            .iter()
            .filter(|result| result.is_ok())
            .count();
        assert!(failures >= 1);
        assert!(successes >= 1);

        pool.shutdown().await;
    }

    #[tokio::test]
    async fn marks_worker_unavailable_after_failures_and_recovers_on_restart() {
        let flaky = Arc::new(FlakyWorker {
            available: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        });
        let pool = WorkerPool::new_inprocess(
            vec![Arc::clone(&flaky) as Arc<dyn WorkerHandler>],
            WorkerPoolOptions {
                queue_capacity: 2,
                unavailable_after_failures: 1,
                health_probe_interval: Duration::from_millis(15),
                default_request_timeout: Some(Duration::from_secs(1)),
                ..WorkerPoolOptions::default()
            },
            None,
        )
        .expect("pool should initialize");

        let error = pool
            .submit(sample_request("down"))
            .await
            .expect_err("request should fail while worker is unavailable");
        assert!(matches!(error, WorkerPoolError::Worker(_)));

        sleep(Duration::from_millis(25)).await;
        let health_before = pool.health_snapshot().await;
        assert_eq!(health_before[0].status, WorkerHealthStatus::Unavailable);

        flaky.available.store(true, Ordering::Relaxed);
        pool.restart_worker("worker-0")
            .await
            .expect("restart should succeed");

        sleep(Duration::from_millis(25)).await;
        let response = pool
            .submit(sample_request("up"))
            .await
            .expect("request should succeed after restart");
        assert_eq!(
            response.result,
            WorkerResult::Success {
                output: json!({"status": "up"})
            }
        );

        pool.shutdown().await;
    }

    #[tokio::test]
    async fn returns_timeout_for_slow_worker() {
        let pool = WorkerPool::new_inprocess(
            vec![Arc::new(SlowWorker {
                delay: Duration::from_millis(100),
            })],
            WorkerPoolOptions {
                queue_capacity: 2,
                default_request_timeout: Some(Duration::from_millis(5)),
                ..WorkerPoolOptions::default()
            },
            None,
        )
        .expect("pool should initialize");

        let error = pool
            .submit(sample_request("timeout"))
            .await
            .expect_err("request should time out");
        assert!(matches!(error, WorkerPoolError::Timeout));

        pool.shutdown().await;
    }

    #[tokio::test]
    async fn rejects_request_when_security_contract_is_violated() {
        let pool = WorkerPool::new_inprocess(
            vec![Arc::new(EchoWorker)],
            WorkerPoolOptions {
                security_policy: WorkerSecurityPolicy {
                    max_request_timeout_ms: 10,
                    max_request_payload_bytes: 256,
                    max_identifier_length: 12,
                },
                ..WorkerPoolOptions::default()
            },
            None,
        )
        .expect("pool should initialize");

        let mut request = sample_request("req-too-large");
        request.timeout_ms = Some(99);
        request.operation = WorkerOperation::Tool {
            tool: "echo".to_string(),
            input: json!({"payload": "x".repeat(1024)}),
            scoped_input: json!({"input": {}}),
        };

        let error = pool
            .submit(request)
            .await
            .expect_err("request should be rejected by security policy");

        assert!(matches!(error, WorkerPoolError::InvalidRequest { .. }));
        pool.shutdown().await;
    }
}
