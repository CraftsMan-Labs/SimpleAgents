# Workflow Engine Integration Guide

This guide explains how to integrate the workflow engine into the existing SimpleAgents codebase. It provides step-by-step instructions for Phase 1 implementation and beyond.

---

## Table of Contents

1. [Current SimpleAgents Structure](#current-simpleagents-structure)
2. [Proposed Crate Structure](#proposed-crate-structure)
3. [Phase 1 Setup (Week 1)](#phase-1-setup-week-1)
4. [Integration Points](#integration-points)
5. [Migration Strategy](#migration-strategy)
6. [Testing Strategy](#testing-strategy)
7. [CI/CD Updates](#cicd-updates)
8. [Documentation Updates](#documentation-updates)

---

## Current SimpleAgents Structure

Based on the existing repository:

```
SimpleAgents/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── simple-agents/            # Main client library
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── client.rs         # SimpleAgentsClient
│   │   │   ├── provider.rs       # Provider trait
│   │   │   └── ...
│   │   └── Cargo.toml
│   ├── simple-agents-type/       # Core types
│   ├── simple-agents-healing/    # JSON healing system
│   ├── simple-agents-router/     # Routing strategies
│   ├── simple-agents-cache/      # Caching layer
│   └── ...
├── examples/
│   └── python_client.py
└── README.md
```

---

## Proposed Crate Structure

Add 5 new crates for workflow engine:

```
SimpleAgents/
├── Cargo.toml                              # Updated workspace
├── crates/
│   ├── simple-agents/                      # EXISTING (unchanged)
│   ├── simple-agents-type/                 # EXISTING (unchanged)
│   ├── simple-agents-healing/              # EXISTING (unchanged)
│   ├── simple-agents-router/               # EXISTING (unchanged)
│   ├── simple-agents-cache/                # EXISTING (unchanged)
│   │
│   ├── simple-agents-workflow-types/       # NEW: Pure types
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── graph.rs                   # WorkflowGraph, NodeDefinition
│   │   │   ├── node.rs                    # NodeType enum
│   │   │   ├── edge.rs                    # EdgeDefinition
│   │   │   ├── state.rs                   # State types
│   │   │   └── validation.rs              # Graph validation
│   │   └── Cargo.toml
│   │
│   ├── simple-agents-workflow-engine/      # NEW: Execution engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── executor.rs                # DAG executor
│   │   │   ├── state.rs                   # State manager
│   │   │   ├── scheduler.rs               # Node scheduler
│   │   │   ├── nodes/                     # Node implementations
│   │   │   │   ├── mod.rs
│   │   │   │   ├── llm_call.rs
│   │   │   │   ├── transform.rs
│   │   │   │   ├── switch.rs
│   │   │   │   └── ...
│   │   │   ├── trace/                     # Trace recording
│   │   │   │   ├── recorder.rs
│   │   │   │   └── replay.rs
│   │   │   └── observability/             # Tracing, metrics
│   │   │       ├── tracing.rs
│   │   │       └── metrics.rs
│   │   └── Cargo.toml
│   │
│   ├── simple-agents-workflow-expressions/ # NEW: CEL evaluator
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── cel.rs                     # CEL integration
│   │   │   ├── cache.rs                   # Expression cache
│   │   │   └── validation.rs              # Expression validation
│   │   └── Cargo.toml
│   │
│   ├── simple-agents-workflow-workers/     # NEW: gRPC workers
│   │   ├── proto/
│   │   │   └── worker.proto               # gRPC protocol
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── pool.rs                    # WorkerPool
│   │   │   ├── handle.rs                  # WorkerHandle
│   │   │   ├── health.rs                  # HealthTracker
│   │   │   └── client.rs                  # gRPC client
│   │   ├── build.rs                       # Proto compilation
│   │   └── Cargo.toml
│   │
│   └── simple-agents-workflow/             # NEW: High-level facade
│       ├── src/
│       │   ├── lib.rs
│       │   ├── builder.rs                 # Builder DSL
│       │   ├── prelude.rs                 # Re-exports
│       │   └── local_runner.rs            # Testing utilities
│       └── Cargo.toml
│
├── workers/                                 # NEW: Language workers
│   ├── python/
│   │   ├── simple_agents_worker/
│   │   │   ├── __init__.py
│   │   │   ├── server.py              # gRPC server
│   │   │   └── handler.py             # Handler registration
│   │   ├── worker.py                  # Entry point
│   │   ├── pyproject.toml
│   │   └── README.md
│   ├── go/
│   │   ├── worker/
│   │   │   ├── server.go
│   │   │   └── handler.go
│   │   ├── main.go
│   │   ├── go.mod
│   │   └── README.md
│   └── typescript/
│       ├── src/
│       │   ├── server.ts
│       │   └── handler.ts
│       ├── worker.ts
│       ├── package.json
│       └── README.md
│
├── examples/
│   ├── workflows/                       # NEW: Workflow examples
│   │   ├── simple-linear.yaml
│   │   ├── conditional-routing.yaml
│   │   ├── parallel-processing.yaml
│   │   └── multi-language.yaml
│   └── python_client.py                # EXISTING
│
└── bindings/                            # NEW: Language bindings (Phase 8)
    ├── python/
    ├── node/
    └── go/
```

---

## Phase 1 Setup (Week 1)

### Step 1: Update Workspace Cargo.toml

**File**: `Cargo.toml`

```toml
[workspace]
members = [
    "crates/simple-agents",
    "crates/simple-agents-type",
    "crates/simple-agents-healing",
    "crates/simple-agents-router",
    "crates/simple-agents-cache",
    # ... existing crates ...

    # NEW: Workflow engine crates
    "crates/simple-agents-workflow-types",
    "crates/simple-agents-workflow-engine",
    "crates/simple-agents-workflow-expressions",
    "crates/simple-agents-workflow-workers",
    "crates/simple-agents-workflow",
]

[workspace.package]
version = "0.2.0"  # Bump minor version
authors = ["SimpleAgents Team"]
edition = "2021"
rust-version = "1.75"

[workspace.dependencies]
# Existing dependencies
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# NEW: Workflow engine dependencies
serde_yaml = "0.9"
cel-interpreter = "0.7"  # Or cel-go via FFI
tonic = "0.11"
prost = "0.12"
opentelemetry = "0.21"
prometheus = "0.13"
```

### Step 2: Create `simple-agents-workflow-types` Crate

**File**: `crates/simple-agents-workflow-types/Cargo.toml`

```toml
[package]
name = "simple-agents-workflow-types"
version.workspace = true
edition.workspace = true
authors.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
thiserror = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
```

**File**: `crates/simple-agents-workflow-types/src/lib.rs`

```rust
//! Pure types for workflow definitions.
//!
//! This crate contains all the types for defining workflows (graphs, nodes, edges)
//! with Serde serialization for YAML/JSON support.

mod graph;
mod node;
mod edge;
mod state;
mod validation;

pub use graph::*;
pub use node::*;
pub use edge::*;
pub use state::*;
pub use validation::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_yaml() {
        let graph = WorkflowGraph {
            id: "test".into(),
            version: "1.0.0".parse().unwrap(),
            nodes: Default::default(),
            edges: vec![],
            metadata: Default::default(),
            entry_node: "start".into(),
        };

        let yaml = serde_yaml::to_string(&graph).unwrap();
        let deserialized: WorkflowGraph = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(graph.id, deserialized.id);
    }
}
```

**File**: `crates/simple-agents-workflow-types/src/graph.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type GraphId = String;
pub type Version = semver::Version;
pub type NodeId = String;

/// Top-level workflow graph definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowGraph {
    /// Unique identifier for this workflow
    pub id: GraphId,

    /// Semantic version
    pub version: Version,

    /// All nodes in the graph
    pub nodes: HashMap<NodeId, NodeDefinition>,

    /// Edges connecting nodes
    pub edges: Vec<EdgeDefinition>,

    /// Entry node (where execution starts)
    pub entry_node: NodeId,

    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl WorkflowGraph {
    pub fn builder() -> WorkflowGraphBuilder {
        WorkflowGraphBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct WorkflowGraphBuilder {
    id: Option<GraphId>,
    version: Option<Version>,
    nodes: Vec<NodeDefinition>,
    edges: Vec<EdgeDefinition>,
    entry_node: Option<NodeId>,
    metadata: HashMap<String, serde_json::Value>,
}

impl WorkflowGraphBuilder {
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn version(mut self, version: impl Into<Version>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn node(mut self, node: NodeDefinition) -> Self {
        if self.entry_node.is_none() {
            self.entry_node = Some(node.id.clone());
        }
        self.nodes.push(node);
        self
    }

    pub fn edge(mut self, edge: EdgeDefinition) -> Self {
        self.edges.push(edge);
        self
    }

    pub fn build(self) -> Result<WorkflowGraph, crate::ValidationError> {
        let graph = WorkflowGraph {
            id: self.id.ok_or(crate::ValidationError::MissingField("id"))?,
            version: self.version.unwrap_or_else(|| "1.0.0".parse().unwrap()),
            nodes: self.nodes.into_iter()
                .map(|n| (n.id.clone(), n))
                .collect(),
            edges: self.edges,
            entry_node: self.entry_node.ok_or(crate::ValidationError::MissingField("entry_node"))?,
            metadata: self.metadata,
        };

        // Validate graph
        graph.validate()?;

        Ok(graph)
    }
}
```

**File**: `crates/simple-agents-workflow-types/src/node.rs`

```rust
use serde::{Deserialize, Serialize};

/// Node definition in the workflow graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeDefinition {
    pub id: NodeId,
    pub node_type: NodeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,  // CEL expression
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// All supported node types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeType {
    LlmCall(LlmCallNode),
    Transform(TransformNode),
    Switch(SwitchNode),
    Loop(LoopNode),
    Map(MapNode),
    Reduce(ReduceNode),
    Parallel(ParallelNode),
    Merge(MergeNode),
    Subgraph(SubgraphNode),
    Filter(FilterNode),
    Batch(BatchNode),
    Cache(CacheNode),
    Retry(RetryNode),
    HumanApproval(HumanApprovalNode),
    CustomWorker(CustomWorkerNode),
}

/// LLM call node (integrates with SimpleAgentsClient).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmCallNode {
    pub provider: String,  // "openai", "anthropic", etc.
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,  // JSON Schema
}

/// Transform node (CEL expression).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransformNode {
    pub expression: String,  // CEL expression
}

// ... Define other node types ...
```

**File**: `crates/simple-agents-workflow-types/src/edge.rs`

```rust
use serde::{Deserialize, Serialize};

/// Edge connecting two nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeDefinition {
    pub from: NodeId,
    pub to: NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,  // Optional CEL condition
}

pub struct Edge;

impl Edge {
    pub fn from(from: impl Into<NodeId>) -> EdgeBuilder {
        EdgeBuilder {
            from: from.into(),
            to: None,
            condition: None,
        }
    }
}

pub struct EdgeBuilder {
    from: NodeId,
    to: Option<NodeId>,
    condition: Option<String>,
}

impl EdgeBuilder {
    pub fn to(mut self, to: impl Into<NodeId>) -> Self {
        self.to = Some(to.into());
        self
    }

    pub fn condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }

    pub fn build(self) -> EdgeDefinition {
        EdgeDefinition {
            from: self.from,
            to: self.to.expect("to node required"),
            condition: self.condition,
        }
    }
}
```

**File**: `crates/simple-agents-workflow-types/src/validation.rs`

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Missing required field: {0}")]
    MissingField(&'static str),

    #[error("Invalid edge: {reason}")]
    InvalidEdge { reason: String },

    #[error("Cycle detected in graph")]
    CycleDetected,
}

impl WorkflowGraph {
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Check all edges reference existing nodes
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) {
                return Err(ValidationError::InvalidEdge {
                    reason: format!("Source node '{}' not found", edge.from),
                });
            }
            if !self.nodes.contains_key(&edge.to) {
                return Err(ValidationError::InvalidEdge {
                    reason: format!("Target node '{}' not found", edge.to),
                });
            }
        }

        // Check for cycles (topological sort)
        self.detect_cycles()?;

        Ok(())
    }

    fn detect_cycles(&self) -> Result<(), ValidationError> {
        // Implement topological sort
        // If sort fails, cycle exists
        let sorted = petgraph::algo::toposort(&self.to_digraph(), None)
            .map_err(|_| ValidationError::CycleDetected)?;

        Ok(())
    }
}
```

### Step 3: Create `simple-agents-workflow-engine` Crate

**File**: `crates/simple-agents-workflow-engine/Cargo.toml`

```toml
[package]
name = "simple-agents-workflow-engine"
version.workspace = true
edition.workspace = true

[dependencies]
simple-agents-workflow-types = { path = "../simple-agents-workflow-types" }
simple-agents = { path = "../simple-agents" }

tokio = { workspace = true }
serde_json = { workspace = true }
thiserror = "1"
tracing = "0.1"
```

**File**: `crates/simple-agents-workflow-engine/src/executor.rs`

```rust
use simple_agents_workflow_types::*;
use simple_agents::SimpleAgentsClient;
use std::collections::HashMap;
use serde_json::Value;

pub struct WorkflowEngine {
    client: SimpleAgentsClient,
}

impl WorkflowEngine {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: SimpleAgentsClient::new()?,
        })
    }

    pub async fn execute(&self, graph: &WorkflowGraph, input: Value) -> Result<Value> {
        let mut state = ExecutionState::new(input);

        // Topological sort to get execution order
        let order = topological_sort(graph)?;

        // Execute nodes in order
        for node_id in order {
            let node = &graph.nodes[&node_id];
            let output = self.execute_node(node, &state).await?;
            state.set_node_output(node_id, output);
        }

        // Return final node output
        Ok(state.get_final_output())
    }

    async fn execute_node(&self, node: &NodeDefinition, state: &ExecutionState) -> Result<Value> {
        match &node.node_type {
            NodeType::LlmCall(llm_node) => {
                self.execute_llm_call(llm_node, state).await
            }
            NodeType::Transform(transform_node) => {
                self.execute_transform(transform_node, state).await
            }
            // ... other node types ...
            _ => Err(anyhow::anyhow!("node type not implemented in this minimal example")),
        }
    }

    async fn execute_llm_call(&self, node: &LlmCallNode, state: &ExecutionState) -> Result<Value> {
        // Use existing SimpleAgentsClient
        let request = CompletionRequest {
            model: node.model.clone(),
            messages: vec![Message::user(&node.prompt)],
            // ...
        };

        let response = self.client.complete(request).await?;

        // Return as JSON value
        Ok(serde_json::to_value(&response.content)?)
    }
}

struct ExecutionState {
    global: HashMap<String, Value>,
    node_outputs: HashMap<NodeId, Value>,
}

impl ExecutionState {
    fn new(input: Value) -> Self {
        let mut global = HashMap::new();
        global.insert("input".to_string(), input);

        Self {
            global,
            node_outputs: HashMap::new(),
        }
    }

    fn set_node_output(&mut self, node_id: NodeId, output: Value) {
        self.node_outputs.insert(node_id, output);
    }

    fn get_node_output(&self, node_id: &NodeId) -> Option<&Value> {
        self.node_outputs.get(node_id)
    }

    fn get_final_output(&self) -> Value {
        // Return last node output
        self.node_outputs.values().last().cloned().unwrap_or(Value::Null)
    }
}
```

### Step 4: Create Simple Example

**File**: `examples/workflows/simple-linear.yaml`

```yaml
id: simple-linear
version: 1.0.0
entry_node: analyze

nodes:
  - id: analyze
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        prompt: "Analyze the sentiment of: {{ input.text }}"

  - id: transform
    node_type:
      transform:
        expression: '{"sentiment": $.nodes.analyze.output.sentiment}'

edges:
  - from: analyze
    to: transform
```

**File**: `examples/simple_workflow.rs`

```rust
use simple_agents_workflow::prelude::*;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    // Load workflow from YAML
    let yaml = std::fs::read_to_string("examples/workflows/simple-linear.yaml")?;
    let workflow: WorkflowGraph = serde_yaml::from_str(&yaml)?;

    // Execute
    let engine = WorkflowEngine::new()?;
    let result = engine.execute(&workflow, json!({
        "text": "This product is amazing!"
    })).await?;

    println!("Result: {}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
```

### Step 5: Run Tests

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run example
cargo run --example simple_workflow
```

---

## Integration Points

### 1. LlmCall Node → SimpleAgentsClient

**Location**: `crates/simple-agents-workflow-engine/src/nodes/llm_call.rs`

```rust
use simple_agents::{SimpleAgentsClient, CompletionRequest, Message};

impl LlmCallNode {
    pub async fn execute(&self, ctx: &ExecutionContext) -> Result<Value> {
        let client = ctx.client();  // Get existing SimpleAgentsClient

        let request = CompletionRequest {
            model: self.model.clone(),
            messages: vec![Message::user(&render_prompt(&self.prompt, ctx)?)],
            // Use existing routing, healing, caching from SimpleAgents
            ..Default::default()
        };

        // Use existing client (gets all benefits: healing, routing, caching)
        let response = client.complete(request).await?;

        // Return structured output
        Ok(serde_json::to_value(&response)?)
    }
}
```

### 2. Healing System Integration

```rust
impl LlmCallNode {
    pub async fn execute_with_healing(&self, ctx: &ExecutionContext) -> Result<Value> {
        if let Some(schema) = &self.output_schema {
            // Use existing healing system
            let response = ctx.client().complete_with_healing(
                request,
                schema.clone(),
            ).await?;

            // Guaranteed to match schema
            Ok(response)
        } else {
            // No schema, no healing
            self.execute(ctx).await
        }
    }
}
```

### 3. Router Integration

Workflows automatically benefit from existing routing strategies:

```rust
// Existing router config works transparently
let client = SimpleAgentsClient::builder()
    .with_router(Router::Latency)  // Existing feature
    .build()?;

let engine = WorkflowEngine::with_client(client)?;

// LlmCall nodes use configured router
let result = engine.execute(&workflow, input).await?;
```

### 4. Cache Integration

```rust
// Existing cache works transparently
let client = SimpleAgentsClient::builder()
    .with_cache(CacheConfig::default())  // Existing feature
    .build()?;

// Workflows benefit from cache
let engine = WorkflowEngine::with_client(client)?;
```

---

## Migration Strategy

### For Existing SimpleAgents Users

#### Option 1: No Changes Required
Existing code continues to work:

```rust
// Existing code (unchanged)
let client = SimpleAgentsClient::new()?;
let response = client.complete(request).await?;
```

#### Option 2: Gradual Adoption
Add workflows alongside existing code:

```rust
// Existing client
let client = SimpleAgentsClient::new()?;

// NEW: Use workflow for complex orchestration
let workflow = WorkflowGraph::builder()
    .node(Node::llm_call("analyze").model("gpt-4"))
    .build()?;

let engine = WorkflowEngine::with_client(client.clone())?;
let result = engine.execute(&workflow, input).await?;
```

#### Option 3: Full Migration
Convert sequential LLM calls to workflows:

**Before:**
```rust
let response1 = client.complete(request1).await?;
let response2 = client.complete(request2).await?;
let response3 = client.complete(request3).await?;
```

**After:**
```rust
let workflow = WorkflowGraph::builder()
    .node(Node::llm_call("step1"))
    .node(Node::llm_call("step2"))
    .node(Node::llm_call("step3"))
    .edge(Edge::from("step1").to("step2"))
    .edge(Edge::from("step2").to("step3"))
    .build()?;

let result = engine.execute(&workflow, input).await?;
```

**Benefits:**
- Trace recording
- Replay capability
- Easy to add branching
- Parallel execution (if steps independent)

---

## Testing Strategy

### Unit Tests

```rust
// crates/simple-agents-workflow-types/src/graph.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_validation() {
        let graph = WorkflowGraph::builder()
            .id("test")
            .node(NodeDefinition { /* ... */ })
            .edge(EdgeDefinition { /* ... */ })
            .build();

        assert!(graph.is_ok());
    }

    #[test]
    fn test_invalid_edge() {
        let graph = WorkflowGraph::builder()
            .id("test")
            .edge(EdgeDefinition {
                from: "nonexistent".into(),
                to: "also_nonexistent".into(),
                condition: None,
            })
            .build();

        assert!(matches!(graph, Err(ValidationError::InvalidEdge { .. })));
    }
}
```

### Integration Tests

```rust
// crates/simple-agents-workflow-engine/tests/integration_test.rs
use simple_agents_workflow::prelude::*;

#[tokio::test]
async fn test_linear_workflow() {
    let workflow = WorkflowGraph::builder()
        .id("test-linear")
        .node(Node::llm_call("node1").model("gpt-4"))
        .node(Node::transform("node2").expression("$.nodes.node1.output"))
        .edge(Edge::from("node1").to("node2"))
        .build()
        .unwrap();

    let engine = WorkflowEngine::new().unwrap();
    let result = engine.execute(&workflow, json!({"text": "test"})).await;

    assert!(result.is_ok());
}
```

### Example Tests

```bash
# Test all examples work
cargo test --examples
```

---

## CI/CD Updates

### GitHub Actions Workflow

**File**: `.github/workflows/ci.yml`

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache
        uses: Swatinem/rust-cache@v2

      - name: Build
        run: cargo build --workspace --all-features

      - name: Test
        run: cargo test --workspace --all-features

      - name: Clippy
        run: cargo clippy --workspace -- -D warnings

      - name: Format
        run: cargo fmt --check

  # NEW: Worker tests (Phase 4+)
  test-workers:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Set up Python
        uses: actions/setup-python@v4
        with:
          python-version: '3.11'

      - name: Set up Go
        uses: actions/setup-go@v4
        with:
          go-version: '1.21'

      - name: Set up Node
        uses: actions/setup-node@v3
        with:
          node-version: '20'

      - name: Install worker dependencies
        run: |
          pip install -r workers/python/requirements.txt
          cd workers/go && go mod download
          cd workers/typescript && npm install

      - name: Test workers
        run: |
          pytest workers/python/tests
          cd workers/go && go test ./...
          cd workers/typescript && npm test
```

---

## Documentation Updates

### Update Main README

**File**: `README.md`

```markdown
# SimpleAgents

... existing content ...

## NEW: Workflow Engine

SimpleAgents now includes a workflow engine for orchestrating complex agentic workflows.

### Quick Start

```rust
use simple_agents_workflow::prelude::*;

let workflow = WorkflowGraph::builder()
    .id("sentiment-analysis")
    .node(Node::llm_call("analyze")
        .provider(Provider::OpenAI)
        .model("gpt-4")
        .prompt("Analyze sentiment: {{ input.text }}"))
    .node(Node::switch("route")
        .branch(Branch::when("sentiment == 'positive'").target("celebrate")))
    .build()?;

let engine = WorkflowEngine::new()?;
let result = engine.execute(&workflow, input).await?;
```

### Features

- ✅ 15 node types (LLM, branching, loops, parallel, etc.)
- ✅ Multi-language workers (Python, Go, TypeScript)
- ✅ Trace recording and replay
- ✅ Distributed tracing and metrics
- ✅ Code DSL for Rust, Python, TypeScript, Go

See [README.md](README.md) for details.
```

### Create Workflow Guide

**File**: `docs/YAML_WORKFLOW_SYSTEM.md`

```markdown
# Workflow Engine Guide

Complete guide to using the SimpleAgents Workflow Engine.

## Table of Contents

1. [Introduction](#introduction)
2. [Core Concepts](#core-concepts)
3. [Node Types](#node-types)
4. [Examples](#examples)
5. [Testing](#testing)
6. [Production Deployment](#production-deployment)

... (See workflow-engine-research/ for full content)
```

---

## Appendix: Cargo.toml for Each New Crate

### simple-agents-workflow-types

```toml
[package]
name = "simple-agents-workflow-types"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
thiserror = "1"
uuid = { version = "1", features = ["v4", "serde"] }
semver = { version = "1", features = ["serde"] }
petgraph = "0.6"
```

### simple-agents-workflow-engine

```toml
[package]
name = "simple-agents-workflow-engine"
version.workspace = true
edition.workspace = true

[dependencies]
simple-agents-workflow-types = { path = "../simple-agents-workflow-types" }
simple-agents = { path = "../simple-agents" }

tokio = { workspace = true }
serde_json = { workspace = true }
thiserror = "1"
tracing = "0.1"
handlebars = "5"  # For prompt templates
```

### simple-agents-workflow-expressions

```toml
[package]
name = "simple-agents-workflow-expressions"
version.workspace = true
edition.workspace = true

[dependencies]
simple-agents-workflow-types = { path = "../simple-agents-workflow-types" }

cel-interpreter = "0.7"  # Or use FFI to cel-go
serde_json = { workspace = true }
thiserror = "1"
```

### simple-agents-workflow-workers

```toml
[package]
name = "simple-agents-workflow-workers"
version.workspace = true
edition.workspace = true

[dependencies]
simple-agents-workflow-types = { path = "../simple-agents-workflow-types" }

tonic = "0.11"
prost = "0.12"
tokio = { workspace = true }
thiserror = "1"

[build-dependencies]
tonic-build = "0.11"
```

### simple-agents-workflow

```toml
[package]
name = "simple-agents-workflow"
version.workspace = true
edition.workspace = true

[dependencies]
simple-agents-workflow-types = { path = "../simple-agents-workflow-types" }
simple-agents-workflow-engine = { path = "../simple-agents-workflow-engine" }
simple-agents-workflow-expressions = { path = "../simple-agents-workflow-expressions" }

# Re-export for convenience
pub use simple_agents_workflow_types as types;
pub use simple_agents_workflow_engine as engine;
```

---

## Next Steps

1. **Week 1**: Complete crate setup following this guide
2. **Week 2**: Implement linear DAG executor
3. **Week 3**: Add transform node and tests
4. Continue following [implementation-plan.md](implementation-plan.md)

---

## Questions?

Refer to:
- [implementation-plan.md](implementation-plan.md) - 30-week roadmap
- [research.md](research.md) - Research summary
- [design/architecture.md](design/architecture.md) - System architecture
- ADRs in [decisions/](decisions/) - Architecture decisions
