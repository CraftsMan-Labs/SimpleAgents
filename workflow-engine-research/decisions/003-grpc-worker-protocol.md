# ADR-003: gRPC for Multi-Language Worker Protocol

## Status
Accepted

## Context
Workflow nodes need to execute code in multiple languages (Python, Go, TypeScript). We need an RPC protocol for the Rust core to communicate with language workers.

Requirements:
- **Performance**: Low latency (<5ms overhead)
- **Streaming**: Support progressive results
- **Bi-directional**: Workers can send progress updates
- **Type-safe**: Strongly typed contracts
- **Cross-language**: Works in Rust, Python, Go, TypeScript
- **Versioning**: Protocol evolution without breaking changes

## Decision
Use **gRPC with Protocol Buffers (proto3)** as the worker RPC protocol.

Architecture:
- **Rust core**: gRPC client (via Tonic)
- **Language workers**: gRPC servers (Python: grpcio, Go: grpc-go, TS: @grpc/grpc-js)
- **Long-lived workers**: Pool of warm worker processes (not per-request spawn)

## Alternatives Considered

### 1. **HTTP/REST JSON**
- **Pros**: Simple, widely supported, easy debugging
- **Cons**: Higher latency, no streaming, more overhead
- **Verdict**: Too slow for hot path

### 2. **MessagePack over TCP**
- **Pros**: Fast binary format, smaller payloads
- **Cons**: No standard RPC framework, manual protocol design
- **Verdict**: Too low-level, reinventing RPC

### 3. **ZeroMQ**
- **Pros**: Very fast, flexible patterns
- **Cons**: No RPC abstraction, manual serialization, less tooling
- **Verdict**: Too low-level for this use case

### 4. **Thrift**
- **Pros**: Similar to gRPC, multi-language
- **Cons**: Less active community, fewer language bindings
- **Verdict**: gRPC has better momentum

### 5. **NATS/Redis for messaging**
- **Pros**: Distributed, pub/sub patterns
- **Cons**: Adds dependency, higher latency, overkill for local workers
- **Verdict**: Better for distributed systems, not single-machine workers

### 6. **Stdin/Stdout IPC**
- **Pros**: Simple, no network
- **Cons**: No streaming, fragile, hard to debug
- **Verdict**: Too brittle

## Consequences

### Positive
- **Performance**: HTTP/2 multiplexing, binary encoding
- **Streaming**: Server streaming for progress updates
- **Type safety**: Protobuf generates types for all languages
- **Tooling**: grpcurl, grpc-health-probe, observability
- **Versioning**: Backward-compatible proto evolution
- **Standard**: Industry standard (Google, CNCF)

### Negative
- **Complexity**: More setup than simple HTTP
- **Binary debugging**: Need tools like grpcurl
- **Proto compilation**: Build step for each language

## Implementation Notes

### Proto Definition
```protobuf
service WorkerService {
  rpc ExecuteNode(ExecuteNodeRequest) returns (stream ExecuteNodeResponse);
  rpc Health(HealthRequest) returns (HealthResponse);
}

message ExecuteNodeRequest {
  string execution_id = 1;
  string handler = 2;
  bytes input = 3;  // JSON-encoded
  map<string, string> metadata = 4;
}

message ExecuteNodeResponse {
  oneof response {
    bytes chunk = 1;      // Streaming
    bytes final = 2;      // Final result
    Error error = 3;
    Progress progress = 4;
  }
}
```

### Rust Client (Tonic)
```rust
use tonic::transport::Channel;
use worker_service_client::WorkerServiceClient;

let channel = Channel::from_static("http://localhost:50051")
    .connect()
    .await?;

let mut client = WorkerServiceClient::new(channel);

let request = ExecuteNodeRequest {
    execution_id: exec_id.to_string(),
    handler: "ProcessData".to_string(),
    input: serde_json::to_vec(&input)?,
    metadata: HashMap::new(),
};

let mut stream = client.execute_node(request).await?.into_inner();

while let Some(response) = stream.message().await? {
    match response.response {
        Some(Response::Final(data)) => {
            let result: Value = serde_json::from_slice(&data)?;
            return Ok(result);
        }
        Some(Response::Error(err)) => return Err(err.into()),
        _ => {}
    }
}
```

### Python Server (grpcio)
```python
from concurrent import futures
import grpc
from workflow_pb2_grpc import WorkerServiceServicer, add_WorkerServiceServicer_to_server

class PythonWorker(WorkerServiceServicer):
    async def ExecuteNode(self, request, context):
        input_data = json.loads(request.input)
        result = await process(input_data)
        yield ExecuteNodeResponse(final=json.dumps(result).encode())

server = grpc.aio.server(futures.ThreadPoolExecutor(max_workers=10))
add_WorkerServiceServicer_to_server(PythonWorker(), server)
server.add_insecure_port('[::]:50051')
await server.start()
```

### Performance Characteristics
- **Latency**: 2-5ms per RPC (local)
- **Throughput**: 10K+ req/sec per worker
- **Overhead**: ~200 bytes per request (HTTP/2 headers + protobuf)

### Health Checking
```rust
// Health check loop
let mut interval = tokio::time::interval(Duration::from_secs(10));

loop {
    interval.tick().await;

    let response = client.health(HealthRequest {}).await?;

    if response.status != HealthStatus::Serving {
        mark_unhealthy(&worker_id).await;
    }
}
```

### Error Handling
```protobuf
message Error {
  string message = 1;
  string code = 2;
  bool retryable = 3;  // Distinguish transient vs permanent errors
  optional string stack_trace = 4;
}
```

### Versioning Strategy
- **Backward compatibility**: Add new fields with defaults
- **Proto version**: Include in package name (`workflow.worker.v1`)
- **Feature detection**: Workers advertise supported features in Health response

## Security Considerations
- **TLS**: Enable for production (via tonic::transport::ServerTlsConfig)
- **Authentication**: Add metadata for API keys/tokens
- **Rate limiting**: Implement in worker pool
- **Sandboxing**: Workers run in separate processes (process isolation)

## Related Decisions
- ADR-004: Long-Lived Worker Pools
- ADR-005: Worker Health Tracking
