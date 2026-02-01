# Workflow Engine Research Summary

## Executive Summary

This document summarizes the complete research and design work for the SimpleAgents Workflow Engine—a DAG-based orchestration system that extends SimpleAgents with workflow capabilities while maintaining zero breaking changes.

**Core Value Proposition**: Enable production-grade agentic workflows that are testable, replayable, observable, and portable across languages (Rust, Python, TypeScript, Go).

---

## Key Decisions

### 1. Canonical IR: YAML/JSON (ADR-001)
**Decision**: Use YAML as primary format with JSON as alternative, both mapping to same Rust types via Serde.

**Why**: Human-readable, portable, version-controllable, excellent tooling support.

**Tradeoff**: No compile-time checking (mitigated with validation and code DSL).

### 2. Expression Language: CEL (ADR-002)
**Decision**: Use CEL (Common Expression Language) for conditions, routing, and data transformations.

**Why**: Industry standard (Kubernetes, Google Cloud), sandboxed, portable across languages.

**Tradeoff**: FFI overhead if using cel-go (mitigated with native Rust CEL interpreter fallback).

### 3. Worker Protocol: gRPC (ADR-003)
**Decision**: gRPC with Protocol Buffers for multi-language worker communication.

**Why**: High performance (<5ms overhead), streaming support, type-safe contracts, cross-language.

**Tradeoff**: More complex than HTTP, binary debugging (mitigated with grpcurl, health checks).

### 4. Worker Lifecycle: Long-Lived Pools (ADR-004)
**Decision**: Maintain pools of 2-10 long-lived worker processes per language, not per-request spawning.

**Why**: Eliminate cold-start penalty (50-500ms), predictable resource usage, high throughput (10K+ req/sec per worker).

**Tradeoff**: Workers consume memory when idle (mitigated with configurable pool sizes).

### 5. Health Tracking: Circuit Breakers (ADR-005)
**Decision**: Active health checks with circuit breaker pattern (Closed → Open → Half-Open).

**Why**: Fast failure detection (1-5s), graceful degradation, auto-recovery.

**Tradeoff**: Health check overhead (mitigated—only ~1-2ms per worker every 5s).

### 6. Authoring: Code DSL + YAML (ADR-006)
**Decision**: Provide builder-pattern DSLs for Rust, Python, TypeScript, Go that compile to canonical IR.

**Why**: Type safety, IDE support, discoverability, while keeping YAML for portability.

**Tradeoff**: Maintenance burden across 4 languages (mitigated with code generation where possible).

### 7. State Model: Hierarchical Scoping (ADR-007)
**Decision**: Three-layer state: global state, scoped state (parent-child), and immutable node outputs.

**Why**: Security (isolation), composability (subgraphs), flexibility.

**Tradeoff**: Complexity in capability enforcement (mitigated with clear token model).

### 8. Replayability: Event-Sourced Traces (ADR-008)
**Decision**: Record append-only execution trace with all node inputs/outputs for deterministic replay.

**Why**: Debugging, testing, compliance, resume-from-failure.

**Tradeoff**: Storage overhead (mitigated with compression, pluggable backends).

### 9. Streaming: First-Class Support (ADR-009)
**Decision**: All nodes can stream outputs; schema validation deferred until stream completion.

**Why**: Real-time feedback for LLMs, better UX, efficient data transfer.

**Tradeoff**: Complexity in partial vs final state handling (mitigated with clear semantics).

### 10. Testing: Golden Traces (ADR-010)
**Decision**: Snapshot testing with golden traces (insta crate) for deterministic validation.

**Why**: Easy to write, easy to review, catches regressions.

**Tradeoff**: Brittle if outputs change frequently (mitigated with flexible matchers).

### 11. Node Types: 15 Core Types (ADR-011)
**Decision**: Support 15 node types across 5 categories: Execution, Control Flow, Concurrency, Composition, Reliability.

**Why**: Cover 90% of use cases, prevent scope creep, extensible for custom nodes.

**Tradeoff**: API surface is larger (mitigated with consistent patterns, good docs).

---

## System Architecture

```
┌─────────────────────────────────────┐
│   Workflow Engine (NEW - 5 crates) │
│   - Graph executor                  │
│   - State manager                   │
│   - CEL evaluator                   │
│   - Worker RPC                      │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│   SimpleAgents Core (EXISTING)      │
│   - Provider abstraction            │
│   - Healing system                  │
│   - Routing strategies              │
│   - Caching                         │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│   Language Workers (NEW)            │
│   - Python, Go, TypeScript          │
│   - gRPC servers                    │
└─────────────────────────────────────┘
```

**Zero Breaking Changes**: All existing SimpleAgents APIs continue to work unchanged.

---

## 15 Node Types

### Execution Nodes
1. **LlmCall**: Invoke LLM provider (uses existing SimpleAgentsClient)
2. **CustomWorker**: Execute code in Python/Go/TypeScript via gRPC
3. **Transform**: Data transformation using CEL expressions

### Control Flow Nodes
4. **Switch**: Conditional branching with multi-way conditions
5. **Loop**: Iteration with condition (while semantics)
6. **Filter**: Predicate-based filtering, drop/short-circuit

### Concurrency Nodes
7. **Parallel**: Fan-out to multiple nodes concurrently
8. **Merge**: Fan-in with policies (first/all/quorum)
9. **Map**: Parallel iteration over collection
10. **Reduce**: Aggregation with pluggable functions

### Composition Nodes
11. **Subgraph**: Nested workflow invocation with state isolation
12. **Batch**: Windowing/batching (collect N items or time window)

### Reliability Nodes
13. **Cache**: Explicit cache read/write with memoization
14. **Retry**: Retry with exponential backoff, max attempts
15. **HumanApproval**: Human-in-the-loop for approval/edit/override

---

## State Model

### Three Layers

1. **Global State**: `HashMap<String, Value>`
   - Accessible to all nodes
   - Survives across subgraphs
   - Used for workflow-wide configuration

2. **Scoped State**: Hierarchical parent-child scoping
   - Each subgraph has its own scope
   - Inherits from parent scope
   - Capability tokens control access

3. **Node Outputs**: `$.nodes.<id>.output`
   - Immutable references
   - JSON path syntax
   - Type-safe via output schemas

### Capability Tokens

Control access to resources:
- `model:openai:*` - Can use any OpenAI model
- `secret:api_key` - Can read specific secret
- `api:https://api.example.com` - Can call specific API

Enforced at executor level, not worker level (for security).

---

## Expression System

### CEL Expressions

```yaml
# Conditional
condition: '$.nodes.analyze.output.sentiment == "positive"'

# Comparison
condition: '$.nodes.analyze.output.confidence > 0.8'

# Logic
condition: 'sentiment == "positive" && confidence > 0.7'

# Math
expression: '($.nodes.count.output + 1) * 2'

# Functions
condition: 'size($.nodes.fetch.output.items) > 10'

# Ternary
expression: 'confidence > 0.8 ? "high" : "low"'
```

### Expression Test Harness

DSL for testing expressions:

```yaml
expression_tests:
  - name: positive_sentiment
    expression: 'sentiment == "positive"'
    context:
      sentiment: positive
    expected: true

  - name: high_confidence
    expression: 'confidence > 0.8'
    context:
      confidence: 0.9
    expected: true
```

---

## Trace Recording & Replayability

### Event-Sourced Trace

```json
{
  "execution_id": "exec_abc123",
  "graph_id": "sentiment-analysis",
  "graph_version": "1.0.0",
  "events": [
    {
      "type": "NodeStarted",
      "timestamp": "2024-01-01T00:00:00Z",
      "node_id": "analyze",
      "input": {"text": "Great product!"}
    },
    {
      "type": "NodeCompleted",
      "timestamp": "2024-01-01T00:00:02Z",
      "node_id": "analyze",
      "output": {"sentiment": "positive", "confidence": 0.95},
      "duration_ms": 2000
    }
  ]
}
```

### Replay

```rust
// Load trace
let trace = TraceLoader::load("exec_abc123").await?;

// Replay from start
let result = engine.replay(&trace, ReplayOptions::default()).await?;

// Replay from specific node
let result = engine.replay(&trace, ReplayOptions {
    from_node: Some("route".into()),
    ..Default::default()
}).await?;
```

### LLM Response Caching

Cached in trace for deterministic replay:
```json
{
  "type": "NodeCompleted",
  "node_id": "analyze",
  "llm_response": {
    "provider": "openai",
    "model": "gpt-4",
    "cached": true,
    "output": "..."
  }
}
```

---

## Streaming Model

### All Nodes Can Stream

```rust
// LLM node streams tokens
for await chunk in node.execute_stream(input) {
    emit_chunk(chunk);  // Progressive output
}

// Final schema validation after stream completes
let final_output = collect_chunks(chunks);
validate_schema(final_output)?;
transition_to_next_node(final_output);
```

### Streaming Edges

```yaml
nodes:
  - id: llm
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
    streaming: true  # Enable streaming

edges:
  - from: llm
    to: transform
    streaming: true  # Stream chunks to next node
```

### Backpressure

```rust
// Streaming with backpressure
let (tx, rx) = mpsc::channel(buffer_size: 10);

// Producer (LLM)
tokio::spawn(async move {
    for chunk in llm_stream {
        tx.send(chunk).await?;  // Blocks if buffer full
    }
});

// Consumer (next node)
while let Some(chunk) = rx.recv().await {
    process_chunk(chunk);
}
```

---

## Multi-Language Workers

### Worker Architecture

```
Rust Core
    ↓ gRPC
┌─────────────┬─────────────┬─────────────┐
│   Python    │     Go      │ TypeScript  │
│   Worker    │   Worker    │   Worker    │
│   (pool=4)  │  (pool=2)   │  (pool=3)   │
└─────────────┴─────────────┴─────────────┘
```

### Handler Registration

**Python:**
```python
from simple_agents.worker import WorkerServer, handler

@handler("ProcessData")
async def process_data(input: dict) -> dict:
    # Custom logic
    return {"result": "processed"}

server = WorkerServer()
server.start(port=50051)
```

**Go:**
```go
import worker "github.com/simple-agents/workflow-go/worker"

func ProcessData(input map[string]interface{}) (map[string]interface{}, error) {
    // Custom logic
    return map[string]interface{}{"result": "processed"}, nil
}

func main() {
    server := worker.NewServer()
    server.Register("ProcessData", ProcessData)
    server.Start(":50051")
}
```

**TypeScript:**
```typescript
import { WorkerServer, handler } from '@simple-agents/workflow/worker';

@handler('ProcessData')
async function processData(input: any): Promise<any> {
  // Custom logic
  return { result: 'processed' };
}

const server = new WorkerServer();
server.start(50051);
```

### Health Checks

```protobuf
service WorkerService {
  rpc Health(HealthRequest) returns (HealthResponse);
}

message HealthResponse {
  HealthStatus status = 1;  // SERVING, NOT_SERVING, DRAINING
  uint64 uptime_seconds = 2;
  uint64 requests_in_flight = 3;
  uint64 memory_used_bytes = 4;
}
```

---

## Observability

### Distributed Tracing (OpenTelemetry)

```rust
// Span per node
let span = tracer.span_builder("execute_node")
    .with_attributes([
        ("node.id", node_id),
        ("node.type", node_type),
        ("workflow.id", workflow_id),
    ])
    .start(&tracer);

let _guard = span.enter();
let result = execute_node(node).await?;

span.set_attribute("node.duration_ms", duration.as_millis());
span.set_attribute("node.status", "success");
```

### Metrics (Prometheus)

```
# Node execution metrics
workflow_node_executions_total{node_type="llm_call", status="success"} 1234
workflow_node_duration_seconds{node_type="llm_call", quantile="0.95"} 2.5

# Worker pool metrics
workflow_worker_health{worker_id="python-0", language="python"} 1
workflow_worker_requests_in_flight{worker_id="python-0"} 3

# Workflow execution metrics
workflow_executions_total{graph_id="sentiment-analysis", status="success"} 567
workflow_execution_duration_seconds{graph_id="sentiment-analysis", quantile="0.99"} 5.2
```

### Debug Tools

```bash
# View execution trace
workflow trace exec_abc123

# Replay workflow
workflow replay exec_abc123 --from analyze

# Inspect node state
workflow inspect exec_abc123 analyze
# Output:
# Node: analyze
# Type: llm_call
# Input: {"text": "Great product!"}
# Output: {"sentiment": "positive", "confidence": 0.95}
# Duration: 2000ms
# Status: success
```

---

## Testing Strategy

### 1. Unit Tests (Cargo test)
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_switch_node_positive_branch() {
        let node = SwitchNode { /* ... */ };
        let input = json!({"sentiment": "positive"});
        let target = node.evaluate_branch(input)?;
        assert_eq!(target, "celebrate");
    }
}
```

### 2. Golden Traces (insta)
```rust
#[test]
fn test_sentiment_workflow() {
    let workflow = load_workflow("sentiment-analysis.yaml");
    let result = execute_workflow(workflow, test_input()).await?;

    // Snapshot entire execution trace
    insta::assert_yaml_snapshot!(result.trace);
}
```

### 3. Contract Tests (Workers)
```python
# tests/test_python_worker.py
def test_worker_contract():
    # Start worker
    worker = start_python_worker()

    # Call via gRPC
    client = WorkerServiceClient("localhost:50051")
    response = client.ExecuteNode(ExecuteNodeRequest(
        handler="ProcessData",
        input=json.dumps({"key": "value"})
    ))

    # Verify response format
    assert response.final is not None
    output = json.loads(response.final)
    assert "result" in output
```

### 4. Integration Tests
```rust
#[tokio::test]
async fn test_multi_language_workflow() {
    // Start all workers
    let workers = start_worker_pool().await?;

    // Execute workflow with Python, Go, and TS nodes
    let workflow = WorkflowGraph::builder()
        .node(Node::custom_worker("py").language(Language::Python))
        .node(Node::custom_worker("go").language(Language::Go))
        .node(Node::custom_worker("ts").language(Language::TypeScript))
        .build()?;

    let result = engine.execute(&workflow, input).await?;
    assert!(result.is_ok());
}
```

### 5. Performance Benchmarks (Criterion)
```rust
fn benchmark_executor(c: &mut Criterion) {
    c.bench_function("linear_workflow_10_nodes", |b| {
        b.iter(|| {
            execute_workflow(linear_10_node_workflow, test_input()).await
        });
    });
}
```

---

## Language DSL Examples

### Rust DSL
```rust
let workflow = WorkflowGraph::builder()
    .id("sentiment")
    .node(Node::llm_call("analyze")
        .provider(Provider::OpenAI)
        .model("gpt-4"))
    .node(Node::switch("route")
        .branch(Branch::when("sentiment == 'positive'").target("celebrate")))
    .edge(Edge::from("analyze").to("route"))
    .build()?;

let engine = WorkflowEngine::new()?;
let result = engine.execute(&workflow, input).await?;
```

### Python DSL
```python
workflow = (
    WorkflowGraph()
    .id("sentiment")
    .node(Node.llm_call("analyze")
        .provider(Provider.OPENAI)
        .model("gpt-4"))
    .node(Node.switch("route")
        .branch_when("sentiment == 'positive'", target="celebrate"))
    .edge(Edge.from_("analyze").to("route"))
    .build()
)

engine = WorkflowEngine()
result = await engine.execute(workflow, input)
```

### TypeScript DSL
```typescript
const workflow = new WorkflowGraph()
  .id('sentiment')
  .node(Node.llmCall('analyze')
    .provider(Provider.OpenAI)
    .model('gpt-4'))
  .node(Node.switch('route')
    .branch({ condition: "sentiment == 'positive'", target: 'celebrate' }))
  .edge(Edge.from('analyze').to('route'))
  .build();

const engine = new WorkflowEngine();
const result = await engine.execute(workflow, input);
```

### Go DSL
```go
workflow := workflow.NewGraph().
    ID("sentiment").
    Node(workflow.LLMCall("analyze").
        Provider(workflow.ProviderOpenAI).
        Model("gpt-4")).
    Node(workflow.Switch("route").
        Branch("sentiment == 'positive'", "celebrate")).
    Edge(workflow.NewEdge("analyze", "route")).
    Build()

engine := workflow.NewEngine()
result, err := engine.Execute(ctx, workflow, input)
```

### All Compile to Same YAML
```yaml
id: sentiment
version: 1.0.0
nodes:
  - id: analyze
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
  - id: route
    node_type:
      switch:
        branches:
          - condition: "sentiment == 'positive'"
            target: celebrate
edges:
  - from: analyze
    to: route
```

---

## Performance Targets

### Latency
- **Overhead per node**: <10ms (excluding LLM call time)
- **Worker RPC latency**: 2-5ms (localhost gRPC)
- **Expression evaluation**: <1ms (CEL cached)
- **State access**: <0.1ms (in-memory HashMap)

### Throughput
- **Simple workflows**: 1000+ executions/sec (single machine)
- **Per-worker throughput**: 1000+ req/sec (async I/O)
- **Parallel nodes**: 10+ concurrent nodes per workflow

### Resource Usage
- **Memory**: <1GB for 10 concurrent workflows
- **Worker pool**: ~1GB total (4 Python + 2 Go + 3 TS workers)
- **Trace storage**: ~10KB per workflow execution (compressed)

---

## Security Model

### Capability-Based Access Control

```yaml
nodes:
  - id: analyze
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
    capabilities:
      - model:openai:gpt-4
      - secret:openai_api_key

  - id: risky_api_call
    node_type:
      custom_worker:
        language: python
        handler: CallExternalAPI
    capabilities:
      - api:https://api.example.com
      - secret:api_key
```

### Enforcement
```rust
fn check_capabilities(node: &Node, required: &[Capability]) -> Result<()> {
    for cap in required {
        if !node.capabilities.contains(cap) {
            return Err(Error::InsufficientCapabilities {
                node: node.id,
                required: cap.clone(),
            });
        }
    }
    Ok(())
}
```

### Process Isolation
- Workers run in separate processes (process-level isolation)
- Future: cgroups for resource limits (CPU, memory, network)
- Future: WASM for sandboxed execution

---

## Known Limitations

1. **LLM Non-Determinism**: Replays may differ due to LLM variability (mitigated with cached responses)
2. **CEL FFI Overhead**: If using cel-go via FFI, ~1-2ms overhead per expression (mitigated with native Rust CEL)
3. **Worker Memory**: Long-lived workers consume memory when idle (mitigated with configurable pool sizes, restarts)
4. **Trace Storage**: Large workflows generate large traces (mitigated with compression, pluggable backends)
5. **Cross-Language Parity**: Keeping 4 DSLs in sync is complex (mitigated with code generation, tests)

---

## Future Enhancements

### Phase 10+ (Beyond 30 Weeks)

1. **Visual Workflow Editor**: Drag-and-drop workflow authoring with code generation
2. **Workflow Marketplace**: Share and reuse workflows (templates, subgraphs)
3. **Auto-Scaling Worker Pools**: Dynamic pool sizing based on load
4. **Distributed Execution**: Multi-machine workflow execution (Kubernetes)
5. **WASM Workers**: Sandboxed execution without separate processes
6. **Advanced Scheduling**: Cron-based triggers, event-driven execution
7. **Cost Optimization**: Smart routing to cheapest/fastest LLM provider
8. **Multi-Tenancy**: Isolate workflows by tenant with quotas
9. **A/B Testing**: Run multiple workflow versions simultaneously, compare results
10. **ML Integration**: Train models from workflow execution data

---

## Integration with Existing SimpleAgents

### Zero Breaking Changes

```rust
// Existing API continues to work
let client = SimpleAgentsClient::new()?;
let response = client.complete(request).await?;

// NEW: Workflow API (separate namespace)
use simple_agents_workflow::prelude::*;

let workflow = WorkflowGraph::builder()
    .node(Node::llm_call("analyze").model("gpt-4"))
    .build()?;

let engine = WorkflowEngine::new()?;
let result = engine.execute(&workflow, input).await?;
```

### LlmCall Node Uses Existing Client

```rust
impl LlmCallNode {
    async fn execute(&self, ctx: &ExecutionContext) -> Result<Value> {
        // Use existing SimpleAgentsClient
        let client = ctx.client();

        let request = CompletionRequest {
            model: self.model.clone(),
            messages: render_prompt(&self.prompt, ctx.input())?,
            // ...
        };

        let response = client.complete(request).await?;

        // Return structured output
        Ok(serde_json::to_value(&response.content)?)
    }
}
```

### Healing System Integration

```rust
// Workflow nodes can use healing for structured outputs
impl LlmCallNode {
    async fn execute(&self, ctx: &ExecutionContext) -> Result<Value> {
        let client = ctx.client();

        // Use healing to ensure valid JSON
        let response = client.complete_with_healing(
            request,
            self.output_schema.clone(),
        ).await?;

        // Guaranteed to match schema
        Ok(response)
    }
}
```

---

## Deployment Patterns

### 1. Single-Machine Deployment
```
┌─────────────────────────────────┐
│  Workflow Engine (Rust binary)  │
│  - Executor                      │
│  - State manager                 │
│  - Worker pools (Py, Go, TS)    │
└─────────────────────────────────┘
```

**Use case**: Development, small-scale production

### 2. Multi-Machine Deployment (Future)
```
┌─────────────────┐  ┌─────────────────┐
│  Engine Node 1  │  │  Engine Node 2  │
│  - Executor     │  │  - Executor     │
│  - Workers      │  │  - Workers      │
└─────────────────┘  └─────────────────┘
        ↓                     ↓
┌─────────────────────────────────────┐
│       Shared State (Redis)          │
└─────────────────────────────────────┘
        ↓
┌─────────────────────────────────────┐
│    Trace Storage (S3/PostgreSQL)    │
└─────────────────────────────────────┘
```

**Use case**: High-scale production, multi-tenant

### 3. Kubernetes Deployment (Future)
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: workflow-engine
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: engine
        image: simple-agents/workflow-engine:latest
        env:
        - name: WORKER_POOL_SIZE
          value: "4"
        - name: TRACE_STORAGE
          value: "s3://bucket/traces"
```

**Use case**: Cloud-native deployments

---

## Migration Path for Existing Users

### Step 1: Add Dependency
```toml
[dependencies]
simple-agents = "0.x"
simple-agents-workflow = "0.1"  # NEW
```

### Step 2: Convert Existing Code to Workflow (Optional)

**Before (existing API)**:
```rust
let client = SimpleAgentsClient::new()?;

let response1 = client.complete(request1).await?;
let response2 = client.complete(request2).await?;
let final_response = client.complete(request3).await?;
```

**After (workflow)**:
```rust
let workflow = WorkflowGraph::builder()
    .node(Node::llm_call("step1").model("gpt-4"))
    .node(Node::llm_call("step2").model("gpt-4"))
    .node(Node::llm_call("step3").model("gpt-4"))
    .edge(Edge::from("step1").to("step2"))
    .edge(Edge::from("step2").to("step3"))
    .build()?;

let engine = WorkflowEngine::new()?;
let result = engine.execute(&workflow, input).await?;
```

**Benefits**:
- Trace recording for debugging
- Easy to add branching/loops later
- Parallel execution if steps are independent
- Replayability

---

## Questions & Answers

### Q: Why not use Temporal/Airflow/Prefect?
**A**: Those are excellent tools but designed for different use cases:
- **Temporal**: Queue-based, eventual consistency, heavyweight
- **Airflow**: Batch processing, not real-time, Python-only
- **Prefect**: Similar to Airflow

We need **real-time, agentic execution** with low latency and multi-language support.

### Q: Why gRPC instead of HTTP?
**A**: Performance. gRPC gives us:
- 2-5ms latency vs 10-20ms for HTTP
- Binary encoding (smaller payloads)
- Streaming support built-in
- Type-safe contracts

### Q: Why long-lived workers instead of per-request spawning?
**A**: Performance. Spawning a Python process takes 50-500ms. At 1000 req/sec, that's unsustainable.

### Q: How does this differ from LangChain?
**A**: LangChain is a Python library for chaining LLM calls. We provide:
- Multi-language support (Rust, Python, TypeScript, Go)
- Production-grade reliability (health checks, circuit breakers, retries)
- Replayability and debugging (trace recording)
- Portable workflow definitions (YAML/JSON)

### Q: What if I only use Rust?
**A**: You don't need the worker pools. Just use Rust nodes (LlmCall, Transform, etc.) and skip CustomWorker nodes.

### Q: Can I use this without SimpleAgents?
**A**: Technically yes (it's a separate crate), but LlmCall nodes depend on SimpleAgentsClient. You'd need to provide your own LLM integration.

---

## Conclusion

The Workflow Engine extends SimpleAgents with production-grade DAG orchestration capabilities while maintaining zero breaking changes. The 30-week implementation plan delivers incremental value, starting with basic linear workflows and culminating in a fully-featured system with multi-language support, replayability, observability, and language-specific DSLs.

**Key Strengths**:
- ✅ Real-time, agentic execution (not queue-based)
- ✅ Multi-language (Rust, Python, TypeScript, Go)
- ✅ Production-ready (health checks, circuit breakers, retries)
- ✅ Testable (golden traces, replay)
- ✅ Observable (distributed tracing, metrics)
- ✅ Portable (YAML/JSON IR + code DSLs)

**Next Steps**:
1. Review and approve research
2. Set up crate structure (Phase 1, Week 1)
3. Implement linear DAG executor (Phase 1, Weeks 2-3)
4. Continue following implementation plan

---

## Appendix: Research Documentation

All research is documented in `workflow-engine-research/`:

- **Requirements**: `questionAndAnswer.md`, `features.md`
- **ADRs**: `decisions/001-011-*.md` (11 architecture decisions)
- **Design**: `design/*.md` (5 detailed design docs)
- **Examples**: `examples/*.yaml` (4 example workflows)
- **Implementation Plan**: `implementation-plan.md` (30-week roadmap)
- **This Summary**: `research.md`
