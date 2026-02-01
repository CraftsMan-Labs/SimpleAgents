# Workflow Engine Architecture

## Overview

The workflow engine extends SimpleAgents with DAG orchestration capabilities while maintaining zero breaking changes to existing APIs. It introduces a new orchestration layer that leverages existing provider, healing, routing, and streaming infrastructure.

## System Architecture

```
┌────────────────────────────────────────────────────────────┐
│                   User Applications                        │
│  (Rust, Python, TypeScript, Go code using SimpleAgents)   │
└────────────────────────────────────────────────────────────┘
                          │
                          ↓
┌────────────────────────────────────────────────────────────┐
│              Language Bindings Layer                       │
│  ┌──────────┬──────────┬──────────┬──────────┐           │
│  │  Python  │  Node.js │    Go    │  C FFI   │           │
│  │  (PyO3)  │  (NAPI)  │  (cgo)   │ (extern) │           │
│  └──────────┴──────────┴──────────┴──────────┘           │
│              Workflow DSL APIs                             │
└────────────────────────────────────────────────────────────┘
                          │
                          ↓
┌────────────────────────────────────────────────────────────┐
│              Workflow Engine Layer (NEW)                   │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  simple-agents-workflow (facade)                     │ │
│  │  - Builder DSL                                       │ │
│  │  - Workflow serialization                           │ │
│  │  - Local runner                                     │ │
│  └──────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  simple-agents-workflow-engine                       │ │
│  │  - DAG executor with topological ordering            │ │
│  │  - State manager (hierarchical scoping)             │ │
│  │  - Node scheduler (concurrency control)             │ │
│  │  - Trace recorder (replayability)                   │ │
│  └──────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  simple-agents-workflow-expressions                  │ │
│  │  - CEL expression evaluator                         │ │
│  │  - Expression caching                               │ │
│  │  - Validation                                       │ │
│  └──────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  simple-agents-workflow-workers                      │ │
│  │  - gRPC worker pool                                 │ │
│  │  - Health tracking                                  │ │
│  │  - Circuit breaker                                  │ │
│  └──────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  simple-agents-workflow-types (pure types)          │ │
│  │  - WorkflowGraph, NodeDefinition, EdgeDefinition    │ │
│  │  - Serde serialization                              │ │
│  └──────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
                          │
                          ↓
┌────────────────────────────────────────────────────────────┐
│         SimpleAgents Core (EXISTING - no changes)          │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  SimpleAgentsClient                                  │ │
│  │  - complete() - single request API                  │ │
│  │  - stream() - streaming API                         │ │
│  │  - Provider registry                                │ │
│  │  - Router engine                                    │ │
│  └──────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  Provider Abstraction (simple-agent-type)            │ │
│  │  - Provider trait (transform/execute)               │ │
│  │  - CompletionRequest/Response                       │ │
│  └──────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  Healing System (simple-agents-healing)              │ │
│  │  - JSON parsing with error recovery                 │ │
│  │  - Schema coercion                                  │ │
│  └──────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  Router (simple-agents-router)                       │ │
│  │  - Routing strategies (latency, cost, fallback)     │ │
│  │  - Circuit breaker                                  │ │
│  └──────────────────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  Cache (simple-agents-cache)                         │ │
│  │  - LRU cache with TTL                               │ │
│  └──────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
                          │
                          ↓
┌────────────────────────────────────────────────────────────┐
│            Language Worker Processes (NEW)                 │
│  ┌──────────┬──────────┬──────────────────────┐          │
│  │  Python  │    Go    │    TypeScript        │          │
│  │  Worker  │  Worker  │      Worker          │          │
│  │  (gRPC)  │  (gRPC)  │      (gRPC)          │          │
│  └──────────┴──────────┴──────────────────────┘          │
└────────────────────────────────────────────────────────────┘
```

## Core Principles

### 1. Zero Breaking Changes

All existing SimpleAgents APIs remain unchanged:

```rust
// Existing code continues to work
let client = SimpleAgentsClient::builder()
    .with_provider(Arc::new(openai_provider))
    .build()?;

let response = client.complete(&request).await?;  // ✅ No changes
```

Workflow functionality is opt-in:

```rust
// New workflow API (feature-gated)
#[cfg(feature = "workflow")]
use simple_agents::workflow::WorkflowGraph;

let graph = WorkflowGraph::from_yaml("workflow.yaml")?;
let result = client.execute_workflow(&graph, input).await?;
```

### 2. Reuse Existing Infrastructure

Workflow nodes leverage existing components:

**LLM Nodes** → Use `Provider` trait via `SimpleAgentsClient`
```rust
// Inside workflow executor
async fn execute_llm_call(&self, ctx: &ExecutionContext) -> Result<NodeOutput> {
    let request = CompletionRequest::builder()
        .model(ctx.node.model)
        .messages(ctx.get_messages()?)
        .build()?;

    // Uses existing routing, caching, healing, streaming
    let response = self.agents_client.complete(&request).await?;

    Ok(NodeOutput::from(response))
}
```

**Healing** → JSON Schema validation reuses healing system
**Routing** → LLM nodes benefit from existing routing strategies
**Caching** → LLM nodes automatically cached
**Streaming** → Flows through workflow edges

### 3. Language Agnostic Execution

Nodes can be implemented in any supported language:

```yaml
# workflow.yaml
nodes:
  - id: analyze
    type: llm_call
    provider: openai
    model: gpt-4

  - id: process
    type: custom_worker
    language: python
    handler: ProcessData

  - id: validate
    type: custom_worker
    language: go
    handler: ValidateOutput

  - id: format
    type: custom_worker
    language: typescript
    handler: FormatResults

edges:
  - from: analyze
    to: process
  - from: process
    to: validate
  - from: validate
    to: format
```

### 4. Deterministic and Replayable

Every execution produces a trace:

```json
{
  "execution_id": "exec_123",
  "graph_id": "workflow_v1",
  "started_at": "2026-01-31T10:00:00Z",
  "events": [
    {
      "node_id": "analyze",
      "timestamp": "2026-01-31T10:00:01Z",
      "input": {"prompt": "Analyze this data"},
      "output": {"analysis": "..."},
      "metadata": {
        "provider": "openai",
        "model": "gpt-4",
        "tokens": 150
      }
    }
  ]
}
```

Replay from trace or resume from failure:

```bash
# Replay entire workflow
workflow-cli replay trace.json

# Resume from node 5
workflow-cli resume trace.json --from-node node_5
```

## Crate Organization

### simple-agents-workflow-types

**Purpose**: Pure type definitions with no business logic

**Key Types**:
```rust
pub struct WorkflowGraph {
    pub id: GraphId,
    pub version: Version,
    pub nodes: HashMap<NodeId, NodeDefinition>,
    pub edges: Vec<EdgeDefinition>,
    pub defaults: GraphDefaults,
}

pub struct NodeDefinition {
    pub id: NodeId,
    pub node_type: NodeType,
    pub input_schema: Option<JsonSchema>,
    pub output_schema: Option<JsonSchema>,
    pub config: NodeConfig,
}

pub enum NodeType {
    LlmCall { provider: String, model: String },
    Switch { branches: Vec<SwitchBranch> },
    Parallel { nodes: Vec<NodeId> },
    CustomWorker { language: Language, handler: String },
    // ... 15 total node types
}
```

**Dependencies**: Only `serde`, `serde_json`

### simple-agents-workflow-engine

**Purpose**: Core execution engine

**Key Components**:
```rust
pub struct WorkflowExecutor {
    graph: Arc<WorkflowGraph>,
    state: Arc<StateManager>,
    evaluator: Arc<dyn ExpressionEvaluator>,
    workers: Arc<WorkerPool>,
    agents_client: Arc<SimpleAgentsClient>,
    tracer: Arc<TraceRecorder>,
    semaphore: Arc<Semaphore>,
}

impl WorkflowExecutor {
    pub async fn execute(&self, input: Value) -> Result<ExecutionResult>;
    async fn execute_node(&self, node_id: &NodeId, ctx: ExecutionContext) -> Result<NodeOutput>;
    async fn execute_edges(&self, from: &NodeId, output: NodeOutput) -> Result<()>;
}

pub struct StateManager {
    global: Arc<RwLock<HashMap<String, Value>>>,
    scopes: Arc<RwLock<HashMap<NodeId, ScopedState>>>,
    capabilities: Arc<CapabilityRegistry>,
}

pub struct TraceRecorder {
    storage: Arc<dyn TraceStorage>,
}
```

**Dependencies**:
- `simple-agents-workflow-types`
- `simple-agent-type` (for Result, Provider trait)
- `simple-agents-core` (for SimpleAgentsClient)
- `tokio`, `serde_json`

### simple-agents-workflow-expressions

**Purpose**: Expression evaluation (CEL + pluggable)

**Key Components**:
```rust
#[async_trait]
pub trait ExpressionEvaluator: Send + Sync {
    async fn evaluate(&self, expr: &Expression, ctx: &EvaluationContext) -> Result<Value>;
    fn validate(&self, expr: &Expression) -> Result<()>;
}

pub struct CelEvaluator {
    cache: Arc<RwLock<HashMap<String, CelProgram>>>,
}

pub struct EvaluationContext {
    pub state: HashMap<String, Value>,
    pub node_outputs: HashMap<NodeId, Value>,
}
```

**Dependencies**:
- `simple-agents-workflow-types`
- `cel-interpreter` or FFI to cel-go
- `serde_json`

### simple-agents-workflow-workers

**Purpose**: Multi-language RPC worker system

**Key Components**:
```rust
pub struct WorkerPool {
    workers: HashMap<Language, Vec<WorkerClient>>,
    health: Arc<HealthTracker>,
}

pub struct WorkerClient {
    client: Arc<Mutex<WorkerServiceClient<Channel>>>,
    id: WorkerId,
    language: Language,
}

impl WorkerPool {
    pub async fn execute(&self, language: &Language, handler: &str, input: Value) -> Result<WorkerOutput>;
}
```

**Dependencies**:
- `simple-agents-workflow-types`
- `tonic` (gRPC client/server)
- `prost` (protobuf)

### simple-agents-workflow

**Purpose**: High-level facade and DSL

**Key Components**:
```rust
pub struct WorkflowBuilder {
    graph: WorkflowGraph,
}

impl WorkflowBuilder {
    pub fn new(name: &str) -> Self;
    pub fn add_node(&mut self, node: NodeDefinition) -> &mut Self;
    pub fn add_edge(&mut self, from: NodeId, to: NodeId) -> &mut Self;
    pub fn build(self) -> Result<WorkflowGraph>;
}

// Extend SimpleAgentsClient
impl SimpleAgentsClient {
    #[cfg(feature = "workflow")]
    pub async fn execute_workflow(&self, graph: &WorkflowGraph, input: Value) -> Result<WorkflowResult>;
}
```

**Dependencies**: All workflow crates + `simple-agents-core`

## Execution Model

### DAG Traversal

```rust
impl WorkflowExecutor {
    async fn execute(&self, input: Value) -> Result<ExecutionResult> {
        // 1. Initialize context
        let ctx = ExecutionContext::new(ExecutionId::new(), input, self.state.clone());

        // 2. Start trace recording
        self.tracer.record_start(&ctx).await?;

        // 3. Execute from entry node
        let result = self.execute_node(&self.graph.entry_node, ctx).await?;

        // 4. Complete trace
        self.tracer.record_completion(&ctx, &result).await?;

        Ok(result)
    }

    async fn execute_node(&self, node_id: &NodeId, ctx: ExecutionContext) -> Result<NodeOutput> {
        let node = self.graph.nodes.get(node_id)?;

        // Validate input schema
        if let Some(schema) = &node.input_schema {
            validate_schema(&ctx.input, schema)?;
        }

        // Check capabilities
        self.state.check_capabilities(&ctx, &node.required_capabilities)?;

        // Acquire concurrency permit
        let _permit = self.semaphore.acquire().await?;

        // Execute based on node type
        let output = match &node.node_type {
            NodeType::LlmCall { provider, model } => {
                self.execute_llm_call(provider, model, &ctx).await?
            }
            NodeType::Switch { branches } => {
                self.execute_switch(branches, &ctx).await?
            }
            NodeType::Parallel { nodes } => {
                self.execute_parallel(nodes, &ctx).await?
            }
            NodeType::CustomWorker { language, handler } => {
                self.execute_worker(language, handler, &ctx).await?
            }
            // ... other node types
        };

        // Validate output schema
        if let Some(schema) = &node.output_schema {
            validate_schema(&output.value, schema)?;
        }

        // Record trace
        self.tracer.record_node_execution(node_id, &ctx, &output).await?;

        // Follow outgoing edges
        self.execute_edges(node_id, output, &ctx).await
    }
}
```

### Concurrency Control

```rust
// Parallel fan-out
async fn execute_parallel(&self, node_ids: &[NodeId], ctx: &ExecutionContext) -> Result<NodeOutput> {
    let tasks: Vec<_> = node_ids
        .iter()
        .map(|id| {
            let executor = self.clone();
            let ctx = ctx.clone();
            let id = id.clone();
            tokio::spawn(async move {
                executor.execute_node(&id, ctx).await
            })
        })
        .collect();

    let results = futures::future::try_join_all(tasks).await?;

    Ok(NodeOutput {
        value: json!(results.iter().map(|r| &r.value).collect::<Vec<_>>()),
        streaming: false,
    })
}
```

## Data Flow

### State Management

```rust
pub struct StateManager {
    global: Arc<RwLock<HashMap<String, Value>>>,
    scopes: Arc<RwLock<HashMap<NodeId, ScopedState>>>,
}

pub struct ScopedState {
    local: HashMap<String, Value>,
    parent: Option<NodeId>,
    tokens: Vec<CapabilityToken>,
}

impl StateManager {
    // Hierarchical lookup: local → parent → global
    pub async fn get(&self, node_id: &NodeId, key: &str) -> Option<Value> {
        let scopes = self.scopes.read().await;

        // Check local scope
        if let Some(scope) = scopes.get(node_id) {
            if let Some(val) = scope.local.get(key) {
                return Some(val.clone());
            }

            // Check parent recursively
            if let Some(parent) = &scope.parent {
                return self.get(parent, key).await;
            }
        }

        // Check global
        let global = self.global.read().await;
        global.get(key).cloned()
    }
}
```

### Node Output References

```yaml
# Workflow can reference previous node outputs
nodes:
  - id: analyze
    type: llm_call
    model: gpt-4

  - id: summarize
    type: llm_call
    model: gpt-4
    prompt: "Summarize this analysis: $.nodes.analyze.output"
```

Implemented via JSON path resolution:

```rust
fn resolve_references(template: &str, ctx: &ExecutionContext) -> Result<String> {
    let re = Regex::new(r"\$\.nodes\.(\w+)\.output")?;

    let result = re.replace_all(template, |caps: &Captures| {
        let node_id = &caps[1];
        ctx.node_outputs
            .get(node_id)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string())
    });

    Ok(result.to_string())
}
```

## Error Handling

### Fail-Fast with Compensation

```rust
impl WorkflowExecutor {
    async fn execute_node(&self, node_id: &NodeId, ctx: ExecutionContext) -> Result<NodeOutput> {
        match self.execute_node_inner(node_id, ctx.clone()).await {
            Ok(output) => Ok(output),
            Err(e) => {
                // Check for compensation node
                if let Some(compensation_id) = self.graph.get_compensation_node(node_id) {
                    self.execute_node(&compensation_id, ctx).await?;
                }
                Err(e)
            }
        }
    }
}
```

### Retry Policies

```yaml
nodes:
  - id: unreliable_api
    type: custom_worker
    language: python
    handler: CallExternalAPI
    retry:
      max_attempts: 3
      backoff:
        type: exponential
        initial: 1s
        max: 30s
        multiplier: 2.0
```

## Performance Characteristics

### Memory Footprint

- **Rust core**: ~10MB base + ~5MB per workflow graph
- **Worker processes**: ~30MB per Python worker, ~20MB per Go worker
- **State**: O(nodes × avg_output_size)

### Latency

- **Node execution overhead**: <1ms (DAG traversal, validation)
- **RPC overhead**: ~2-5ms per worker call (gRPC)
- **Expression evaluation**: <0.1ms per CEL expression (cached)

### Throughput

- **Target**: 1M requests with <10 workers
- **Concurrency**: Configurable semaphore (default: 100 in-flight)
- **Worker pool**: Long-lived workers (no per-request spawn)

## Security Model

### Capability Tokens

```rust
pub struct CapabilityToken {
    pub id: String,
    pub allowed_models: Option<Vec<String>>,
    pub allowed_resources: Option<Vec<String>>,
    pub denied: Option<Vec<String>>,
}
```

Example enforcement:

```rust
impl StateManager {
    pub fn check_capabilities(&self, ctx: &ExecutionContext, required: &[String]) -> Result<()> {
        let scope = self.scopes.read().await.get(&ctx.node_id).cloned();

        for req in required {
            let mut granted = false;

            if let Some(scope) = &scope {
                for token in &scope.tokens {
                    if let Some(allowed) = &token.allowed_models {
                        if allowed.contains(req) {
                            granted = true;
                            break;
                        }
                    }
                }
            }

            if !granted {
                return Err(SimpleAgentsError::CapabilityDenied(req.clone()));
            }
        }

        Ok(())
    }
}
```

## Observability

### OpenTelemetry Integration

```rust
use opentelemetry::trace::{Span, Tracer};

impl WorkflowExecutor {
    async fn execute_node(&self, node_id: &NodeId, ctx: ExecutionContext) -> Result<NodeOutput> {
        let tracer = opentelemetry::global::tracer("workflow");
        let mut span = tracer.start(&format!("node.{}", node_id));

        span.set_attribute("node.id", node_id.to_string());
        span.set_attribute("graph.id", self.graph.id.to_string());
        span.set_attribute("execution.id", ctx.execution_id.to_string());

        let result = self.execute_node_inner(node_id, ctx).await;

        match &result {
            Ok(output) => {
                span.set_attribute("node.status", "success");
                span.set_attribute("output.size", output.value.to_string().len() as i64);
            }
            Err(e) => {
                span.set_attribute("node.status", "error");
                span.set_attribute("error.message", e.to_string());
            }
        }

        span.end();
        result
    }
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_linear_workflow() {
        let graph = WorkflowGraph {
            id: "test".into(),
            entry_node: "node1".into(),
            nodes: hashmap! {
                "node1".into() => NodeDefinition {
                    id: "node1".into(),
                    node_type: NodeType::Transform {
                        expression: Expression::new("input.value * 2")
                    },
                    ..Default::default()
                },
            },
            edges: vec![],
            ..Default::default()
        };

        let executor = WorkflowExecutor::new(graph, ...).unwrap();
        let result = executor.execute(json!({"value": 5})).await.unwrap();

        assert_eq!(result.value, json!(10));
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_multi_language_workflow() {
    // Start workers
    let python_worker = start_python_worker().await;
    let go_worker = start_go_worker().await;

    let graph = WorkflowGraph::from_yaml("tests/fixtures/multi_lang.yaml")?;
    let executor = WorkflowExecutor::new(graph, ...)?;

    let result = executor.execute(json!({"input": "test"})).await?;

    assert!(result.success);

    // Cleanup
    python_worker.shutdown().await;
    go_worker.shutdown().await;
}
```

### Golden Trace Tests

```rust
#[test]
fn test_golden_trace() {
    let trace = execute_and_record("tests/fixtures/workflow.yaml", input)?;
    insta::assert_json_snapshot!(trace);
}
```

## Migration Path

### For Existing Users

No changes required - all existing code continues to work:

```rust
// This still works
let client = SimpleAgentsClient::builder()
    .with_provider(Arc::new(openai))
    .build()?;

let response = client.complete(&request).await?;
```

### For New Workflow Users

Opt-in to workflow features:

```rust
use simple_agents::workflow::WorkflowBuilder;

let workflow = WorkflowBuilder::new("my-workflow")
    .add_node(Node::llm_call("openai", "gpt-4"))
    .add_node(Node::transform("$.output.summary"))
    .build()?;

let client = SimpleAgentsClient::builder()
    .with_provider(Arc::new(openai))
    .build()?;

let result = client.execute_workflow(&workflow, input).await?;
```
