# Worker RPC Protocol

## Overview

The worker protocol enables multi-language node execution via gRPC. Long-lived worker processes handle requests from the Rust workflow engine, allowing nodes to be implemented in Python, Go, TypeScript, and other languages.

## Architecture

```
┌────────────────────────────────────────┐
│     Workflow Engine (Rust)             │
│  ┌──────────────────────────────────┐  │
│  │    WorkerPool                    │  │
│  │  - Health tracking               │  │
│  │  - Load balancing                │  │
│  │  - Circuit breaker               │  │
│  └──────────────────────────────────┘  │
└────────────────┬───────────────────────┘
                 │ gRPC
        ┌────────┴────────┐
        │                 │
┌───────▼──────┐  ┌──────▼────────┐  ┌──────▼────────┐
│   Python     │  │      Go       │  │  TypeScript   │
│   Worker     │  │    Worker     │  │    Worker     │
│   (gRPC      │  │   (gRPC       │  │   (gRPC       │
│   server)    │  │   server)     │  │   server)     │
└──────────────┘  └───────────────┘  └───────────────┘
```

## Proto Definition

```protobuf
syntax = "proto3";

package workflow.worker.v1;

service WorkerService {
  // Execute a node
  rpc ExecuteNode(ExecuteNodeRequest) returns (stream ExecuteNodeResponse);

  // Health check
  rpc Health(HealthRequest) returns (HealthResponse);

  // List available handlers
  rpc ListHandlers(ListHandlersRequest) returns (ListHandlersResponse);
}

message ExecuteNodeRequest {
  // Unique execution ID for tracing
  string execution_id = 1;

  // Node ID being executed
  string node_id = 2;

  // Handler name (function/class to invoke)
  string handler = 3;

  // Input data (JSON-encoded)
  bytes input = 4;

  // Execution context metadata
  map<string, string> metadata = 5;

  // Timeout for execution
  optional uint32 timeout_ms = 6;
}

message ExecuteNodeResponse {
  oneof response {
    // Streaming chunk (partial result)
    bytes chunk = 1;

    // Final output (complete result)
    bytes final = 2;

    // Error occurred
    Error error = 3;

    // Progress update
    Progress progress = 4;
  }
}

message Error {
  // Error message
  string message = 1;

  // Error code (for categorization)
  string code = 2;

  // Whether error is retryable
  bool retryable = 3;

  // Stack trace (for debugging)
  optional string stack_trace = 4;
}

message Progress {
  // Progress percentage (0-100)
  float percent = 1;

  // Status message
  string message = 2;
}

message HealthRequest {}

message HealthResponse {
  enum Status {
    UNKNOWN = 0;
    SERVING = 1;
    NOT_SERVING = 2;
  }

  Status status = 1;

  // Worker metadata
  string worker_id = 2;
  string language = 3;
  string version = 4;

  // Resource usage
  optional ResourceUsage resources = 5;
}

message ResourceUsage {
  // Memory usage in bytes
  uint64 memory_bytes = 1;

  // CPU usage percentage
  float cpu_percent = 2;

  // Number of active requests
  uint32 active_requests = 3;
}

message ListHandlersRequest {}

message ListHandlersResponse {
  repeated HandlerInfo handlers = 1;
}

message HandlerInfo {
  // Handler name
  string name = 1;

  // Description
  string description = 2;

  // Input schema (JSON Schema)
  optional string input_schema = 3;

  // Output schema (JSON Schema)
  optional string output_schema = 4;
}
```

## Rust Worker Pool

```rust
pub struct WorkerPool {
    /// Workers by language
    workers: HashMap<Language, Vec<WorkerClient>>,

    /// Health tracker
    health: Arc<HealthTracker>,

    /// Configuration
    config: WorkerPoolConfig,
}

pub struct WorkerPoolConfig {
    /// Max workers per language
    pub max_workers_per_language: usize,

    /// Health check interval
    pub health_check_interval: Duration,

    /// Request timeout
    pub default_timeout: Duration,

    /// Circuit breaker threshold
    pub circuit_breaker_threshold: usize,
}

impl WorkerPool {
    pub async fn new(config: WorkerPoolConfig) -> Result<Self> {
        let mut workers = HashMap::new();

        // Start language workers
        for language in &[Language::Python, Language::Go, Language::TypeScript] {
            let lang_workers = Self::start_workers(language, &config).await?;
            workers.insert(language.clone(), lang_workers);
        }

        // Start health checking
        let health = Arc::new(HealthTracker::new());
        tokio::spawn(Self::health_check_loop(workers.clone(), health.clone(), config.health_check_interval));

        Ok(Self {
            workers,
            health,
            config,
        })
    }

    async fn start_workers(language: &Language, config: &WorkerPoolConfig) -> Result<Vec<WorkerClient>> {
        let mut workers = vec![];

        for i in 0..config.max_workers_per_language {
            let worker = WorkerClient::start(language, i).await?;
            workers.push(worker);
        }

        Ok(workers)
    }

    /// Execute handler on a worker
    pub async fn execute(
        &self,
        language: &Language,
        handler: &str,
        input: Value,
        metadata: HashMap<String, String>,
    ) -> Result<WorkerOutput> {
        // Select healthy worker
        let worker = self.select_worker(language).await?;

        // Execute with timeout
        let timeout = self.config.default_timeout;
        let result = tokio::time::timeout(
            timeout,
            worker.execute(handler, input, metadata),
        ).await;

        match result {
            Ok(Ok(output)) => {
                self.health.record_success(&worker.id).await;
                Ok(output)
            }
            Ok(Err(e)) => {
                self.health.record_failure(&worker.id).await;
                Err(e)
            }
            Err(_) => {
                self.health.record_timeout(&worker.id).await;
                Err(SimpleAgentsError::WorkerTimeout)
            }
        }
    }

    async fn select_worker(&self, language: &Language) -> Result<&WorkerClient> {
        let workers = self.workers.get(language)
            .ok_or(SimpleAgentsError::UnsupportedLanguage(language.clone()))?;

        // Filter healthy workers
        let healthy: Vec<_> = workers.iter()
            .filter(|w| self.health.is_healthy(&w.id))
            .collect();

        if healthy.is_empty() {
            return Err(SimpleAgentsError::NoHealthyWorkers(language.clone()));
        }

        // Round-robin selection
        let index = self.health.next_index(language).await;
        Ok(healthy[index % healthy.len()])
    }

    async fn health_check_loop(
        workers: HashMap<Language, Vec<WorkerClient>>,
        health: Arc<HealthTracker>,
        interval: Duration,
    ) {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            for (language, lang_workers) in &workers {
                for worker in lang_workers {
                    match worker.health_check().await {
                        Ok(response) if response.status == HealthStatus::Serving => {
                            health.mark_healthy(&worker.id).await;
                        }
                        _ => {
                            health.mark_unhealthy(&worker.id).await;
                        }
                    }
                }
            }
        }
    }
}
```

## Worker Client

```rust
pub struct WorkerClient {
    pub id: WorkerId,
    pub language: Language,
    client: Arc<Mutex<WorkerServiceClient<Channel>>>,
}

impl WorkerClient {
    pub async fn start(language: &Language, index: usize) -> Result<Self> {
        let addr = format!("http://localhost:{}", 50051 + index);

        // Start worker process
        let mut child = Command::new(Self::worker_command(language))
            .arg("--port")
            .arg((50051 + index).to_string())
            .spawn()?;

        // Wait for worker to start
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Connect via gRPC
        let channel = Channel::from_shared(addr)?
            .connect()
            .await?;

        let client = WorkerServiceClient::new(channel);

        Ok(Self {
            id: WorkerId::new(),
            language: language.clone(),
            client: Arc::new(Mutex::new(client)),
        })
    }

    fn worker_command(language: &Language) -> &str {
        match language {
            Language::Python => "python",
            Language::Go => "./workers/go/worker",
            Language::TypeScript => "node",
        }
    }

    pub async fn execute(
        &self,
        handler: &str,
        input: Value,
        metadata: HashMap<String, String>,
    ) -> Result<WorkerOutput> {
        let mut client = self.client.lock().await;

        let request = ExecuteNodeRequest {
            execution_id: Uuid::new_v4().to_string(),
            node_id: "".to_string(), // Set by executor
            handler: handler.to_string(),
            input: serde_json::to_vec(&input)?,
            metadata,
            timeout_ms: Some(30000),
        };

        let mut stream = client.execute_node(request).await?.into_inner();

        let mut chunks = vec![];
        let mut final_output = None;

        while let Some(response) = stream.message().await? {
            match response.response {
                Some(Response::Chunk(data)) => {
                    chunks.push(data);
                }
                Some(Response::Final(data)) => {
                    final_output = Some(serde_json::from_slice(&data)?);
                    break;
                }
                Some(Response::Error(err)) => {
                    return Err(SimpleAgentsError::WorkerError(err.message));
                }
                Some(Response::Progress(prog)) => {
                    // Log progress
                    tracing::info!("Progress: {}% - {}", prog.percent, prog.message);
                }
                None => {}
            }
        }

        if let Some(output) = final_output {
            Ok(WorkerOutput {
                value: output,
                streaming: !chunks.is_empty(),
                latency: Duration::default(), // Measured separately
            })
        } else {
            Err(SimpleAgentsError::WorkerNoOutput)
        }
    }

    pub async fn health_check(&self) -> Result<HealthResponse> {
        let mut client = self.client.lock().await;
        let response = client.health(HealthRequest {}).await?.into_inner();
        Ok(response)
    }
}
```

## Python Worker Implementation

```python
# workers/python/worker.py
import grpc
from concurrent import futures
import json
import sys
import traceback
from typing import Dict, Any

from workflow_pb2 import (
    ExecuteNodeRequest,
    ExecuteNodeResponse,
    HealthRequest,
    HealthResponse,
    Error,
)
from workflow_pb2_grpc import WorkerServiceServicer, add_WorkerServiceServicer_to_server

class PythonWorker(WorkerServiceServicer):
    def __init__(self):
        self.handlers: Dict[str, callable] = {}
        self.worker_id = "python-worker-1"

    def register_handler(self, name: str, handler: callable):
        """Register a handler function"""
        self.handlers[name] = handler

    async def ExecuteNode(self, request: ExecuteNodeRequest, context):
        """Execute a node handler"""
        try:
            # Parse input
            input_data = json.loads(request.input)

            # Get handler
            if request.handler not in self.handlers:
                yield ExecuteNodeResponse(
                    error=Error(
                        message=f"Handler not found: {request.handler}",
                        code="HANDLER_NOT_FOUND",
                        retryable=False,
                    )
                )
                return

            handler = self.handlers[request.handler]

            # Execute handler
            if asyncio.iscoroutinefunction(handler):
                result = await handler(input_data, request.metadata)
            else:
                result = handler(input_data, request.metadata)

            # Return result
            output_bytes = json.dumps(result).encode('utf-8')
            yield ExecuteNodeResponse(final=output_bytes)

        except Exception as e:
            yield ExecuteNodeResponse(
                error=Error(
                    message=str(e),
                    code=type(e).__name__,
                    retryable=True,
                    stack_trace=traceback.format_exc(),
                )
            )

    def Health(self, request: HealthRequest, context):
        """Health check"""
        return HealthResponse(
            status=HealthResponse.SERVING,
            worker_id=self.worker_id,
            language="python",
            version="1.0.0",
        )

    def ListHandlers(self, request, context):
        """List available handlers"""
        handlers = [
            HandlerInfo(name=name, description=func.__doc__ or "")
            for name, func in self.handlers.items()
        ]
        return ListHandlersResponse(handlers=handlers)


def serve(port: int = 50051):
    worker = PythonWorker()

    # Register handlers
    from handlers import *
    worker.register_handler("ProcessData", ProcessData())
    worker.register_handler("ValidateInput", ValidateInput())

    # Start server
    server = grpc.aio.server(futures.ThreadPoolExecutor(max_workers=10))
    add_WorkerServiceServicer_to_server(worker, server)
    server.add_insecure_port(f'[::]:{port}')

    print(f"Python worker listening on port {port}")
    await server.start()
    await server.wait_for_termination()


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=50051)
    args = parser.parse_args()

    import asyncio
    asyncio.run(serve(args.port))
```

```python
# workers/python/handlers.py
class ProcessData:
    """Process and transform data"""

    async def __call__(self, input: dict, metadata: dict) -> dict:
        # Custom processing logic
        text = input.get("text", "")
        processed = text.lower().strip()

        return {
            "processed": processed,
            "length": len(processed),
            "metadata": metadata,
        }


class ValidateInput:
    """Validate input data"""

    def __call__(self, input: dict, metadata: dict) -> dict:
        required_fields = ["name", "email"]

        missing = [f for f in required_fields if f not in input]

        if missing:
            raise ValueError(f"Missing required fields: {', '.join(missing)}")

        return {
            "valid": True,
            "input": input,
        }
```

## Go Worker Implementation

```go
// workers/go/worker.go
package main

import (
    "context"
    "encoding/json"
    "flag"
    "fmt"
    "log"
    "net"

    pb "github.com/yourorg/workflow/proto"
    "google.golang.org/grpc"
)

type WorkerServer struct {
    pb.UnimplementedWorkerServiceServer
    handlers map[string]Handler
    workerID string
}

type Handler interface {
    Execute(input map[string]interface{}, metadata map[string]string) (map[string]interface{}, error)
}

func NewWorkerServer() *WorkerServer {
    return &WorkerServer{
        handlers: make(map[string]Handler),
        workerID: "go-worker-1",
    }
}

func (s *WorkerServer) RegisterHandler(name string, handler Handler) {
    s.handlers[name] = handler
}

func (s *WorkerServer) ExecuteNode(req *pb.ExecuteNodeRequest, stream pb.WorkerService_ExecuteNodeServer) error {
    // Parse input
    var input map[string]interface{}
    if err := json.Unmarshal(req.Input, &input); err != nil {
        return stream.Send(&pb.ExecuteNodeResponse{
            Response: &pb.ExecuteNodeResponse_Error{
                Error: &pb.Error{
                    Message:   err.Error(),
                    Code:      "INVALID_INPUT",
                    Retryable: false,
                },
            },
        })
    }

    // Get handler
    handler, ok := s.handlers[req.Handler]
    if !ok {
        return stream.Send(&pb.ExecuteNodeResponse{
            Response: &pb.ExecuteNodeResponse_Error{
                Error: &pb.Error{
                    Message:   fmt.Sprintf("Handler not found: %s", req.Handler),
                    Code:      "HANDLER_NOT_FOUND",
                    Retryable: false,
                },
            },
        })
    }

    // Execute
    result, err := handler.Execute(input, req.Metadata)
    if err != nil {
        return stream.Send(&pb.ExecuteNodeResponse{
            Response: &pb.ExecuteNodeResponse_Error{
                Error: &pb.Error{
                    Message:   err.Error(),
                    Code:      "EXECUTION_ERROR",
                    Retryable: true,
                },
            },
        })
    }

    // Return result
    outputBytes, err := json.Marshal(result)
    if err != nil {
        return stream.Send(&pb.ExecuteNodeResponse{
            Response: &pb.ExecuteNodeResponse_Error{
                Error: &pb.Error{
                    Message:   err.Error(),
                    Code:      "SERIALIZATION_ERROR",
                    Retryable: false,
                },
            },
        })
    }

    return stream.Send(&pb.ExecuteNodeResponse{
        Response: &pb.ExecuteNodeResponse_Final{
            Final: outputBytes,
        },
    })
}

func (s *WorkerServer) Health(ctx context.Context, req *pb.HealthRequest) (*pb.HealthResponse, error) {
    return &pb.HealthResponse{
        Status:   pb.HealthResponse_SERVING,
        WorkerId: s.workerID,
        Language: "go",
        Version:  "1.0.0",
    }, nil
}

func main() {
    port := flag.Int("port", 50052, "Worker port")
    flag.Parse()

    lis, err := net.Listen("tcp", fmt.Sprintf(":%d", *port))
    if err != nil {
        log.Fatalf("Failed to listen: %v", err)
    }

    server := NewWorkerServer()

    // Register handlers
    server.RegisterHandler("ProcessData", &ProcessDataHandler{})

    grpcServer := grpc.NewServer()
    pb.RegisterWorkerServiceServer(grpcServer, server)

    log.Printf("Go worker listening on port %d", *port)
    if err := grpcServer.Serve(lis); err != nil {
        log.Fatalf("Failed to serve: %v", err)
    }
}
```

## TypeScript Worker Implementation

```typescript
// workers/typescript/worker.ts
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import { HandlerRegistry } from './handlers';

const PROTO_PATH = './proto/worker.proto';

class WorkerServer {
  private handlers: Map<string, Handler>;
  private workerId: string;

  constructor() {
    this.handlers = new Map();
    this.workerId = 'typescript-worker-1';
  }

  registerHandler(name: string, handler: Handler) {
    this.handlers.set(name, handler);
  }

  async executeNode(call: grpc.ServerWritableStream<ExecuteNodeRequest, ExecuteNodeResponse>) {
    const request = call.request;

    try {
      // Parse input
      const input = JSON.parse(request.input.toString());

      // Get handler
      const handler = this.handlers.get(request.handler);
      if (!handler) {
        call.write({
          error: {
            message: `Handler not found: ${request.handler}`,
            code: 'HANDLER_NOT_FOUND',
            retryable: false,
          },
        });
        call.end();
        return;
      }

      // Execute
      const result = await handler.execute(input, request.metadata);

      // Return
      call.write({
        final: Buffer.from(JSON.stringify(result)),
      });
      call.end();
    } catch (error) {
      call.write({
        error: {
          message: error.message,
          code: error.name,
          retryable: true,
          stackTrace: error.stack,
        },
      });
      call.end();
    }
  }

  health(call: grpc.ServerUnaryCall<HealthRequest, HealthResponse>, callback: grpc.sendUnaryData<HealthResponse>) {
    callback(null, {
      status: 'SERVING',
      workerId: this.workerId,
      language: 'typescript',
      version: '1.0.0',
    });
  }
}

function serve(port: number = 50053) {
  const packageDefinition = protoLoader.loadSync(PROTO_PATH);
  const proto = grpc.loadPackageDefinition(packageDefinition);

  const server = new grpc.Server();
  const worker = new WorkerServer();

  // Register handlers
  import { ProcessData, ValidateInput } from './handlers';
  worker.registerHandler('ProcessData', new ProcessData());
  worker.registerHandler('ValidateInput', new ValidateInput());

  server.addService(proto.workflow.worker.v1.WorkerService.service, {
    executeNode: worker.executeNode.bind(worker),
    health: worker.health.bind(worker),
  });

  server.bindAsync(`0.0.0.0:${port}`, grpc.ServerCredentials.createInsecure(), () => {
    console.log(`TypeScript worker listening on port ${port}`);
    server.start();
  });
}

if (require.main === module) {
  const port = parseInt(process.argv[2] || '50053');
  serve(port);
}
```

## Error Handling

### Retryable vs Non-Retryable

```python
# Python example
class CustomHandler:
    def __call__(self, input: dict, metadata: dict) -> dict:
        try:
            result = self.process(input)
            return result
        except ValueError as e:
            # Non-retryable (bad input)
            raise ValueError(f"Invalid input: {e}") from e
        except ConnectionError as e:
            # Retryable (network issue)
            raise RetryableError(f"Network error: {e}") from e
```

### Circuit Breaker

```rust
pub struct HealthTracker {
    health: RwLock<HashMap<WorkerId, WorkerHealth>>,
    config: CircuitBreakerConfig,
}

pub struct WorkerHealth {
    pub status: HealthStatus,
    pub consecutive_failures: usize,
    pub last_success: Option<Instant>,
    pub last_failure: Option<Instant>,
}

impl HealthTracker {
    pub async fn record_failure(&self, worker_id: &WorkerId) {
        let mut health = self.health.write().await;
        let h = health.entry(worker_id.clone()).or_default();

        h.consecutive_failures += 1;
        h.last_failure = Some(Instant::now());

        // Open circuit if threshold exceeded
        if h.consecutive_failures >= self.config.failure_threshold {
            h.status = HealthStatus::Unhealthy;
        }
    }

    pub async fn record_success(&self, worker_id: &WorkerId) {
        let mut health = self.health.write().await;
        let h = health.entry(worker_id.clone()).or_default();

        h.consecutive_failures = 0;
        h.last_success = Some(Instant::now());
        h.status = HealthStatus::Healthy;
    }
}
```
