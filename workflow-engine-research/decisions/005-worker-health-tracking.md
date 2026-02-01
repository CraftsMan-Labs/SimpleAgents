# ADR-005: Worker Health Tracking and Circuit Breaking

## Status
Accepted

## Context
Long-lived worker processes can fail, become slow, or enter degraded states. We need a system to:
- Detect unhealthy workers
- Route requests away from failing workers
- Prevent cascading failures
- Automatically recover when workers return to health

Requirements:
- **Fast failure detection**: Detect failed workers within 1-2 seconds
- **Graceful degradation**: Continue operating with partial worker pool
- **Circuit breaking**: Prevent overwhelming struggling workers
- **Auto-recovery**: Automatically use workers when they recover
- **Observability**: Expose health metrics for monitoring

## Decision
Implement **active health checking with circuit breaker pattern** for each worker.

Components:
- **Health Checker**: Periodic gRPC health checks (every 5s)
- **Circuit Breaker**: Per-worker state machine (Closed → Open → Half-Open)
- **Request Router**: Health-aware worker selection
- **Metrics Tracker**: Record health transitions and failure rates

## Alternatives Considered

### 1. **Passive Health Checks Only**
- **Pros**: No overhead, detect failures on actual requests
- **Cons**:
  - Slower detection (only on request failure)
  - User requests affected by unhealthy workers
- **Rejected**: Want to detect failures proactively

### 2. **Heartbeat from Workers**
- **Pros**: Workers actively signal liveness
- **Cons**:
  - Requires workers to implement heartbeat logic
  - Missed heartbeat could be network issue, not worker issue
- **Rejected**: Pull-based is simpler and more reliable

### 3. **Process-Level Health Only**
- **Pros**: Simple (just check if process is alive)
- **Cons**:
  - Doesn't detect application-level issues (deadlock, OOM)
  - Process can be alive but unresponsive
- **Rejected**: Insufficient granularity

### 4. **External Health Service (Consul, etcd)**
- **Pros**: Distributed health tracking, service discovery
- **Cons**:
  - Additional dependency
  - Overkill for single-machine deployment
  - Added latency for health queries
- **Rejected**: Not aligned with local runner focus

### 5. **No Health Tracking (Fail on Error)**
- **Pros**: Simplest implementation
- **Cons**:
  - Poor user experience (errors instead of retries)
  - Cascading failures
- **Rejected**: Enterprise-grade reliability requires health tracking

## Consequences

### Positive
- **Fast failure detection**: 1-5 second detection time
- **Graceful degradation**: Continue with healthy workers
- **Circuit breaking**: Prevent overwhelming failing workers
- **Auto-recovery**: Workers automatically re-added when healthy
- **Better UX**: Transparent failover to healthy workers

### Negative
- **Overhead**: Health check requests every 5s per worker
- **Complexity**: State machine for each worker
- **False positives**: Network blips can trigger unnecessary circuit breaks
- **Delayed recovery**: Half-open state delays full recovery

## Implementation Notes

### Health State Machine

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Healthy,      // Worker is healthy, accept all requests
    Degraded,     // Worker is slow but functional, reduce traffic
    Unhealthy,    // Worker is failing, no new requests
    Unknown,      // Initial state or health check failed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,       // Normal operation
    Open,         // Circuit tripped, reject all requests
    HalfOpen,     // Testing recovery, allow limited requests
}

pub struct WorkerHealth {
    pub worker_id: WorkerId,
    pub health_state: HealthState,
    pub circuit_state: CircuitState,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
    pub last_health_check: Instant,
    pub failure_rate: f64,  // Rolling window
}
```

### Health Tracker

```rust
pub struct HealthTracker {
    workers: Arc<RwLock<HashMap<WorkerId, WorkerHealth>>>,
    config: HealthConfig,
}

pub struct HealthConfig {
    pub check_interval: Duration,           // 5s
    pub check_timeout: Duration,            // 2s
    pub failure_threshold: u32,             // 3 consecutive failures → Unhealthy
    pub success_threshold: u32,             // 2 consecutive successes → Healthy
    pub half_open_requests: usize,          // 5 requests in half-open
    pub degraded_latency_threshold: Duration, // 5s response → Degraded
}

impl HealthTracker {
    pub fn new(config: HealthConfig) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn register_worker(&self, worker_id: WorkerId) {
        let mut workers = self.workers.write().await;
        workers.insert(worker_id, WorkerHealth {
            worker_id,
            health_state: HealthState::Unknown,
            circuit_state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_health_check: Instant::now(),
            failure_rate: 0.0,
        });
    }

    pub async fn is_healthy(&self, worker_id: &WorkerId) -> bool {
        let workers = self.workers.read().await;
        workers.get(worker_id)
            .map(|h| h.health_state == HealthState::Healthy && h.circuit_state == CircuitState::Closed)
            .unwrap_or(false)
    }

    pub async fn record_success(&self, worker_id: &WorkerId, latency: Duration) {
        let mut workers = self.workers.write().await;
        if let Some(health) = workers.get_mut(worker_id) {
            health.consecutive_successes += 1;
            health.consecutive_failures = 0;

            // Update health state based on latency
            if latency > self.config.degraded_latency_threshold {
                health.health_state = HealthState::Degraded;
            } else if health.consecutive_successes >= self.config.success_threshold {
                health.health_state = HealthState::Healthy;
            }

            // Update circuit state
            match health.circuit_state {
                CircuitState::HalfOpen => {
                    if health.consecutive_successes >= self.config.success_threshold {
                        health.circuit_state = CircuitState::Closed;
                        info!("Circuit closed for worker {}", worker_id);
                    }
                }
                _ => {}
            }
        }
    }

    pub async fn record_failure(&self, worker_id: &WorkerId, error: &Error) {
        let mut workers = self.workers.write().await;
        if let Some(health) = workers.get_mut(worker_id) {
            health.consecutive_failures += 1;
            health.consecutive_successes = 0;

            // Update health state
            if health.consecutive_failures >= self.config.failure_threshold {
                health.health_state = HealthState::Unhealthy;
                health.circuit_state = CircuitState::Open;
                warn!("Worker {} marked unhealthy after {} failures", worker_id, health.consecutive_failures);
            }
        }
    }

    pub async fn can_attempt_request(&self, worker_id: &WorkerId) -> bool {
        let workers = self.workers.read().await;
        if let Some(health) = workers.get(worker_id) {
            match health.circuit_state {
                CircuitState::Closed => true,
                CircuitState::Open => {
                    // Check if it's time to enter half-open state
                    let elapsed = health.last_health_check.elapsed();
                    if elapsed > Duration::from_secs(30) {
                        drop(workers);
                        self.enter_half_open(worker_id).await;
                        true
                    } else {
                        false
                    }
                }
                CircuitState::HalfOpen => {
                    // Allow limited requests in half-open state
                    health.consecutive_successes < self.config.half_open_requests
                }
            }
        } else {
            false
        }
    }

    async fn enter_half_open(&self, worker_id: &WorkerId) {
        let mut workers = self.workers.write().await;
        if let Some(health) = workers.get_mut(worker_id) {
            health.circuit_state = CircuitState::HalfOpen;
            health.consecutive_successes = 0;
            health.consecutive_failures = 0;
            info!("Circuit half-open for worker {}", worker_id);
        }
    }
}
```

### Active Health Checking

```rust
pub struct HealthChecker {
    tracker: Arc<HealthTracker>,
    workers: Vec<WorkerHandle>,
    shutdown: CancellationToken,
}

impl HealthChecker {
    pub fn start(
        tracker: Arc<HealthTracker>,
        workers: Vec<WorkerHandle>,
    ) -> Self {
        let shutdown = CancellationToken::new();
        let checker = Self {
            tracker,
            workers,
            shutdown: shutdown.clone(),
        };

        // Spawn health check loop
        tokio::spawn(checker.clone().health_check_loop());

        checker
    }

    async fn health_check_loop(self) {
        let mut interval = tokio::time::interval(self.tracker.config.check_interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.check_all_workers().await;
                }
                _ = self.shutdown.cancelled() => {
                    info!("Health checker shutting down");
                    break;
                }
            }
        }
    }

    async fn check_all_workers(&self) {
        let futures: Vec<_> = self.workers.iter()
            .map(|worker| self.check_worker(worker))
            .collect();

        futures::future::join_all(futures).await;
    }

    async fn check_worker(&self, worker: &WorkerHandle) {
        let start = Instant::now();

        let result = tokio::time::timeout(
            self.tracker.config.check_timeout,
            worker.client.health(HealthRequest {})
        ).await;

        let latency = start.elapsed();

        match result {
            Ok(Ok(response)) => {
                if response.status == HealthStatus::Serving {
                    self.tracker.record_success(&worker.id, latency).await;
                } else {
                    self.tracker.record_failure(&worker.id, &Error::WorkerNotServing).await;
                }
            }
            Ok(Err(e)) => {
                error!("Health check failed for worker {}: {}", worker.id, e);
                self.tracker.record_failure(&worker.id, &e).await;
            }
            Err(_timeout) => {
                error!("Health check timeout for worker {}", worker.id);
                self.tracker.record_failure(&worker.id, &Error::HealthCheckTimeout).await;
            }
        }
    }

    pub async fn shutdown(&self) {
        self.shutdown.cancel();
    }
}
```

### Health-Aware Request Routing

```rust
impl WorkerPool {
    pub async fn execute(&self, request: ExecuteNodeRequest) -> Result<Value> {
        // Retry with different workers if one fails
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 0..max_retries {
            // Select healthy worker
            match self.select_healthy_worker().await {
                Ok(worker) => {
                    // Check circuit breaker
                    if !self.health_tracker.can_attempt_request(&worker.id).await {
                        debug!("Worker {} circuit open, trying next worker", worker.id);
                        continue;
                    }

                    // Execute request
                    let start = Instant::now();
                    match worker.execute(request.clone()).await {
                        Ok(result) => {
                            let latency = start.elapsed();
                            self.health_tracker.record_success(&worker.id, latency).await;
                            return Ok(result);
                        }
                        Err(e) => {
                            self.health_tracker.record_failure(&worker.id, &e).await;
                            last_error = Some(e);

                            if attempt < max_retries - 1 {
                                warn!("Request failed on worker {}, retrying with different worker (attempt {}/{})",
                                    worker.id, attempt + 1, max_retries);
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(Error::NoHealthyWorkers);
                }
            }
        }

        Err(last_error.unwrap_or(Error::AllRetriesFailed))
    }

    async fn select_healthy_worker(&self) -> Result<&WorkerHandle> {
        let healthy_workers: Vec<_> = self.workers.iter()
            .filter(|w| {
                // Check both health state and circuit state
                let is_healthy = self.health_tracker.is_healthy(&w.id).await;
                let can_attempt = self.health_tracker.can_attempt_request(&w.id).await;
                is_healthy || can_attempt
            })
            .collect();

        if healthy_workers.is_empty() {
            return Err(Error::NoHealthyWorkers);
        }

        // Use configured distribution strategy
        Ok(self.distributor.select(&healthy_workers, &self.health_tracker).await?)
    }
}
```

### Extended gRPC Health Protocol

```protobuf
service WorkerService {
  rpc ExecuteNode(ExecuteNodeRequest) returns (stream ExecuteNodeResponse);
  rpc Health(HealthRequest) returns (HealthResponse);
  rpc Metrics(MetricsRequest) returns (MetricsResponse);
}

message HealthRequest {}

message HealthResponse {
  HealthStatus status = 1;
  optional string message = 2;

  // Extended health info
  uint64 uptime_seconds = 3;
  uint64 requests_in_flight = 4;
  uint64 memory_used_bytes = 5;
  float cpu_percent = 6;
}

enum HealthStatus {
  UNKNOWN = 0;
  SERVING = 1;      // Healthy, accepting requests
  NOT_SERVING = 2;  // Unhealthy, don't send requests
  DRAINING = 3;     // Graceful shutdown, finish in-flight only
}

message MetricsRequest {}

message MetricsResponse {
  uint64 requests_total = 1;
  uint64 requests_success = 2;
  uint64 requests_failed = 3;
  double requests_per_second = 4;
  double avg_latency_ms = 5;
  double p95_latency_ms = 6;
  double p99_latency_ms = 7;
}
```

### Health Metrics

```rust
pub struct HealthMetrics {
    pub worker_id: WorkerId,
    pub uptime: Duration,
    pub requests_total: u64,
    pub requests_in_flight: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub health_checks_total: u64,
    pub health_checks_failed: u64,
    pub circuit_trips: u64,
    pub last_failure: Option<Instant>,
}

impl HealthTracker {
    pub async fn get_metrics(&self, worker_id: &WorkerId) -> Option<HealthMetrics> {
        let workers = self.workers.read().await;
        workers.get(worker_id).map(|h| HealthMetrics {
            worker_id: *worker_id,
            // ... populate from WorkerHealth
        })
    }

    pub async fn export_prometheus(&self) -> String {
        let workers = self.workers.read().await;
        let mut output = String::new();

        for (id, health) in workers.iter() {
            output.push_str(&format!(
                "worker_health{{worker_id=\"{}\"}} {}\n",
                id,
                match health.health_state {
                    HealthState::Healthy => 1,
                    HealthState::Degraded => 0.5,
                    HealthState::Unhealthy => 0,
                    HealthState::Unknown => -1,
                }
            ));

            output.push_str(&format!(
                "worker_circuit_state{{worker_id=\"{}\"}} {}\n",
                id,
                match health.circuit_state {
                    CircuitState::Closed => 0,
                    CircuitState::Open => 1,
                    CircuitState::HalfOpen => 2,
                }
            ));

            output.push_str(&format!(
                "worker_consecutive_failures{{worker_id=\"{}\"}} {}\n",
                id, health.consecutive_failures
            ));
        }

        output
    }
}
```

### Graceful Shutdown

```rust
impl WorkerPool {
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Initiating graceful shutdown of worker pool");

        // Mark all workers as draining (no new requests)
        for worker in &self.workers {
            self.health_tracker.mark_draining(worker.id).await;
        }

        // Wait for in-flight requests to complete (with timeout)
        let shutdown_timeout = Duration::from_secs(30);
        let start = Instant::now();

        while start.elapsed() < shutdown_timeout {
            let all_idle = self.workers.iter().all(|w| {
                let stats = w.stats.read().await;
                stats.requests_in_flight == 0
            });

            if all_idle {
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Shutdown workers
        for worker in &mut self.workers {
            if let Err(e) = worker.shutdown().await {
                error!("Failed to shutdown worker {}: {}", worker.id, e);
            }
        }

        Ok(())
    }
}
```

## Configuration Example

```yaml
health_tracking:
  check_interval: 5s
  check_timeout: 2s
  failure_threshold: 3      # 3 consecutive failures → circuit open
  success_threshold: 2      # 2 consecutive successes → circuit closed
  half_open_requests: 5     # Test 5 requests in half-open state
  degraded_latency_threshold: 5s

circuit_breaker:
  open_duration: 30s        # Stay open for 30s before trying half-open
  half_open_duration: 10s   # Stay half-open for max 10s
```

## Performance Impact

- **Health check overhead**: ~1-2ms per worker every 5s (negligible)
- **Routing overhead**: ~0.1ms to check health state per request
- **Memory overhead**: ~1KB per worker for health state
- **Failure detection**: 1-5 seconds (depends on check interval)
- **Recovery time**: 10-40 seconds (open → half-open → closed)

## Related Decisions
- ADR-003: gRPC Worker Protocol
- ADR-004: Long-Lived Worker Pools

## Future Enhancements
- Adaptive health check intervals based on failure rate
- Health prediction using ML (predict failures before they happen)
- Cross-worker health correlation (detect systemic issues)
- Custom health check probes (application-specific health logic)
