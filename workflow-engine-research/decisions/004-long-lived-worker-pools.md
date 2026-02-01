# ADR-004: Long-Lived Worker Pools

## Status
Accepted

## Context
Workflow nodes execute code in multiple languages (Python, Go, TypeScript). We need to decide between spawning a new process per request vs maintaining a pool of long-lived worker processes.

Requirements:
- **Throughput**: Handle 10K+ req/sec total across all workers
- **Latency**: Minimize cold-start overhead
- **Memory**: Predictable memory footprint for autoscaling
- **Concurrency**: High per-worker concurrency without per-request overhead
- **Scalability**: Support burst traffic without spawning excessive processes

## Decision
Use **long-lived worker pools** with a small number of worker processes per language runtime.

Architecture:
- **Pool size**: 2-10 workers per language (configurable)
- **Worker lifecycle**: Start on engine init, keep alive until shutdown
- **Request distribution**: Round-robin with health-aware routing
- **Concurrency**: Each worker handles multiple requests concurrently (async I/O)
- **Warm pools**: Pre-start workers on engine initialization

## Alternatives Considered

### 1. **Per-Request Process Spawn**
- **Pros**: Simple isolation, no shared state concerns
- **Cons**:
  - Cold start penalty (50-500ms for Python, 10-50ms for Go)
  - High overhead at scale (spawning 1M processes for 1M requests)
  - Resource exhaustion under burst traffic
- **Rejected**: Cannot meet throughput requirements

### 2. **Single Shared Worker Per Language**
- **Pros**: Minimal resource usage, simple management
- **Cons**:
  - Single point of failure
  - No parallelism for CPU-bound tasks
  - Head-of-line blocking
- **Rejected**: Insufficient concurrency

### 3. **Lambda/Serverless Function Per Request**
- **Pros**: Infinite scalability, managed infrastructure
- **Cons**:
  - Cold start latency
  - Cost at scale
  - Not suitable for local runner
- **Rejected**: Not aligned with real-time execution focus

### 4. **Thread Pool Within Single Process**
- **Pros**: Lowest overhead, shared memory
- **Cons**:
  - Python GIL limits parallelism
  - Requires thread-safe code
  - Hard to isolate failures
- **Rejected**: Doesn't work well with Python's threading model

### 5. **Auto-Scaling Worker Pool**
- **Pros**: Adapts to load dynamically
- **Cons**:
  - Complexity in scale-up/scale-down logic
  - Still has cold-start latency during scale-up
  - Harder to reason about resource usage
- **Considered**: Good future enhancement, but start with fixed pool

## Consequences

### Positive
- **Low latency**: No per-request process spawn (save 50-500ms)
- **Predictable resources**: Fixed pool size = predictable memory footprint
- **High throughput**: Each worker handles 1K+ req/sec via async I/O
- **Simple scaling**: Scale by adding more workers or engine instances
- **Warm caches**: Workers can maintain warm caches (imports, models, etc.)

### Negative
- **Resource overhead**: Workers consume memory even when idle
- **Fixed concurrency**: Pool size limits parallelism
- **Failure domain**: Worker crash affects all requests in-flight on that worker
- **State leakage risk**: Need to ensure workers don't leak state between requests

## Implementation Notes

### Worker Pool Manager

```rust
pub struct WorkerPool {
    language: Language,
    workers: Vec<WorkerHandle>,
    distributor: WorkerDistributor,
    health_tracker: Arc<HealthTracker>,
}

impl WorkerPool {
    pub async fn new(config: PoolConfig) -> Result<Self> {
        let mut workers = Vec::with_capacity(config.pool_size);

        // Pre-start all workers (warm pool)
        for id in 0..config.pool_size {
            let worker = WorkerHandle::spawn(WorkerConfig {
                id: WorkerId::new(config.language, id),
                port: config.base_port + id,
                language: config.language,
            }).await?;

            workers.push(worker);
        }

        Ok(Self {
            language: config.language,
            workers,
            distributor: WorkerDistributor::RoundRobin,
            health_tracker: Arc::new(HealthTracker::new()),
        })
    }

    pub async fn execute(&self, request: ExecuteNodeRequest) -> Result<Value> {
        // Select healthy worker
        let worker = self.distributor.select(&self.workers, &self.health_tracker).await?;

        // Execute with timeout
        let result = tokio::time::timeout(
            request.timeout,
            worker.execute(request)
        ).await??;

        Ok(result)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        for worker in &mut self.workers {
            worker.shutdown().await?;
        }
        Ok(())
    }
}
```

### Worker Handle

```rust
pub struct WorkerHandle {
    id: WorkerId,
    process: Child,
    client: WorkerServiceClient<Channel>,
    stats: Arc<RwLock<WorkerStats>>,
}

impl WorkerHandle {
    pub async fn spawn(config: WorkerConfig) -> Result<Self> {
        // Start worker process
        let process = Command::new(config.binary_path())
            .arg("--port")
            .arg(config.port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Wait for worker to be ready (with timeout)
        let client = Self::wait_for_ready(config.port, Duration::from_secs(10)).await?;

        Ok(Self {
            id: config.id,
            process,
            client,
            stats: Arc::new(RwLock::new(WorkerStats::default())),
        })
    }

    async fn wait_for_ready(port: u16, timeout: Duration) -> Result<WorkerServiceClient<Channel>> {
        let start = Instant::now();

        loop {
            match Channel::from_shared(format!("http://localhost:{}", port))?
                .connect()
                .await
            {
                Ok(channel) => {
                    let client = WorkerServiceClient::new(channel);
                    // Verify health
                    if client.health(HealthRequest {}).await.is_ok() {
                        return Ok(client);
                    }
                }
                Err(_) if start.elapsed() < timeout => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub async fn execute(&self, request: ExecuteNodeRequest) -> Result<Value> {
        let mut stats = self.stats.write().await;
        stats.requests_total += 1;
        stats.requests_in_flight += 1;
        drop(stats);

        let result = self.client.execute_node(request).await;

        let mut stats = self.stats.write().await;
        stats.requests_in_flight -= 1;
        if result.is_ok() {
            stats.requests_success += 1;
        } else {
            stats.requests_failed += 1;
        }

        result
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        // Graceful shutdown via gRPC
        let _ = self.client.shutdown(ShutdownRequest {}).await;

        // Wait for process to exit (with timeout)
        tokio::time::timeout(
            Duration::from_secs(5),
            self.process.wait()
        ).await??;

        Ok(())
    }
}
```

### Pool Configuration

```yaml
worker_pools:
  python:
    pool_size: 4
    base_port: 50100
    binary_path: /usr/bin/python3
    worker_script: workers/python/worker.py
    max_requests_per_worker: 10000  # Restart after N requests (prevent memory leaks)

  go:
    pool_size: 2
    base_port: 50200
    binary_path: ./workers/go/worker
    max_requests_per_worker: 50000

  typescript:
    pool_size: 3
    base_port: 50300
    binary_path: /usr/bin/node
    worker_script: workers/typescript/worker.js
    max_requests_per_worker: 20000
```

### Worker Selection Strategies

```rust
pub enum WorkerDistributor {
    RoundRobin,
    LeastConnections,
    Random,
    WeightedRandom(Vec<f64>),
}

impl WorkerDistributor {
    pub async fn select<'a>(
        &self,
        workers: &'a [WorkerHandle],
        health: &HealthTracker,
    ) -> Result<&'a WorkerHandle> {
        // Filter to healthy workers only
        let healthy: Vec<_> = workers.iter()
            .filter(|w| health.is_healthy(&w.id))
            .collect();

        if healthy.is_empty() {
            return Err(Error::NoHealthyWorkers);
        }

        let idx = match self {
            Self::RoundRobin => {
                // Thread-safe round-robin counter
                static COUNTER: AtomicUsize = AtomicUsize::new(0);
                COUNTER.fetch_add(1, Ordering::Relaxed) % healthy.len()
            }
            Self::LeastConnections => {
                // Find worker with fewest in-flight requests
                healthy.iter()
                    .enumerate()
                    .min_by_key(|(_, w)| w.stats.read().await.requests_in_flight)
                    .map(|(idx, _)| idx)
                    .unwrap()
            }
            Self::Random => {
                use rand::Rng;
                rand::thread_rng().gen_range(0..healthy.len())
            }
            Self::WeightedRandom(weights) => {
                use rand::distributions::WeightedIndex;
                use rand::prelude::*;

                let dist = WeightedIndex::new(weights)?;
                dist.sample(&mut rand::thread_rng())
            }
        };

        Ok(healthy[idx])
    }
}
```

### Concurrency Per Worker

Each worker handles multiple requests concurrently using async I/O:

**Python Worker:**
```python
import asyncio
import grpc
from concurrent import futures

class PythonWorker(WorkerServiceServicer):
    async def ExecuteNode(self, request, context):
        # Each request runs in its own asyncio task
        # Python worker can handle 100+ concurrent requests via async I/O
        result = await self.execute_handler(request)
        yield ExecuteNodeResponse(final=result)

server = grpc.aio.server(
    futures.ThreadPoolExecutor(max_workers=10),  # Thread pool for I/O
    options=[
        ('grpc.max_concurrent_streams', 100),     # Max concurrent gRPC streams
    ]
)
```

**Go Worker:**
```go
// Go worker handles requests via goroutines (cheap concurrency)
func (s *workerServer) ExecuteNode(req *pb.ExecuteNodeRequest, stream pb.WorkerService_ExecuteNodeServer) error {
    // Each request runs in its own goroutine
    // Go worker can handle 1000+ concurrent requests
    result, err := s.executeHandler(req)
    if err != nil {
        return err
    }
    return stream.Send(&pb.ExecuteNodeResponse{
        Response: &pb.ExecuteNodeResponse_Final{Final: result},
    })
}
```

### Memory Management

```rust
pub struct WorkerStats {
    pub requests_total: u64,
    pub requests_in_flight: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub memory_used_bytes: u64,
    pub uptime_seconds: u64,
}

impl WorkerPool {
    async fn check_worker_health(&self) {
        for worker in &self.workers {
            let stats = worker.stats.read().await;

            // Restart worker if it exceeds request limit (prevent memory leaks)
            if stats.requests_total >= self.config.max_requests_per_worker {
                warn!("Worker {} exceeded max requests, restarting", worker.id);
                self.restart_worker(worker.id).await;
            }

            // Monitor memory usage
            if stats.memory_used_bytes > self.config.max_memory_bytes {
                warn!("Worker {} exceeded memory limit, restarting", worker.id);
                self.restart_worker(worker.id).await;
            }
        }
    }

    async fn restart_worker(&self, id: WorkerId) -> Result<()> {
        // Graceful restart: stop sending new requests, wait for in-flight to complete
        self.health_tracker.mark_draining(id).await;

        // Wait for in-flight requests to complete (with timeout)
        tokio::time::timeout(
            Duration::from_secs(30),
            self.wait_for_idle(id)
        ).await?;

        // Shutdown old worker
        let old_worker = self.workers.iter_mut().find(|w| w.id == id).unwrap();
        old_worker.shutdown().await?;

        // Start new worker
        let new_worker = WorkerHandle::spawn(self.config.clone()).await?;
        *old_worker = new_worker;

        self.health_tracker.mark_healthy(id).await;

        Ok(())
    }
}
```

## Performance Characteristics

### Latency Breakdown

```
Per-request latency (warm worker):
- Worker selection:        0.1ms
- gRPC serialization:      0.5ms
- Network (localhost):     0.1ms
- Worker execution:        Variable (10ms - 10s)
- Response deserialization: 0.5ms
Total overhead:            ~1.2ms

Cold start (first request):
- Process spawn:           50-500ms (Python), 10-50ms (Go)
- Import/init:             100-1000ms (language-dependent)
- gRPC server start:       50ms
Total cold start:          200-1500ms
```

### Throughput

```
Per worker (assuming 100ms avg execution time):
- Sequential:     10 req/sec
- 100 concurrent: 1000 req/sec

Pool of 4 Python workers with 100 concurrent each:
- Total: 4000 req/sec

Pool of 2 Go workers with 1000 concurrent each:
- Total: 2000 req/sec
```

### Resource Usage

```
Memory per worker (approximate):
- Python: 50-200 MB (depends on imports)
- Go:     10-50 MB
- Node:   50-150 MB

Pool of 10 workers (4 Python + 2 Go + 4 Node):
- Total memory: ~1 GB
```

## Related Decisions
- ADR-003: gRPC Worker Protocol
- ADR-005: Worker Health Tracking

## Future Enhancements
- Auto-scaling worker pools based on load
- Worker affinity (route same graph to same worker for cache hits)
- Multi-zone worker pools for geographic distribution
- WASM workers for sandboxed execution
