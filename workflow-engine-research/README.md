# Workflow Engine Research & Design

This directory contains all research, design documents, architecture decision records (ADRs), and examples for the SimpleAgents Workflow Engine.

## Directory Structure

```
workflow-engine-research/
├── README.md                       # This file
├── questionAndAnswer.md            # Requirements Q&A
├── features.md                     # Feature list
├── basePlan.md                     # Overall research plan
│
├── sections/                       # Research by topic
│   ├── 01-canonical-ir-and-dsl.md
│   ├── 02-execution-engine-semantics.md
│   ├── 03-expression-system.md
│   ├── 04-state-and-data-model.md
│   ├── 05-determinism-and-replayability.md
│   ├── 06-concurrency-and-backpressure.md
│   ├── 07-language-workers-and-rpc.md
│   ├── 08-security-and-isolation.md
│   ├── 09-observability-and-debugging.md
│   ├── 10-testing-and-harness.md
│   └── 11-deployment-and-performance-targets.md
│
├── design/                         # Detailed design documents
│   ├── architecture.md             # Overall system architecture
│   ├── ir-schema.md                # Canonical IR specification
│   ├── execution-model.md          # DAG execution semantics
│   ├── state-scoping.md            # State management & scoping
│   └── worker-protocol.md          # gRPC worker protocol
│
├── decisions/                      # Architecture Decision Records (ADRs)
│   ├── 001-canonical-ir-format.md
│   ├── 002-cel-expression-language.md
│   └── 003-grpc-worker-protocol.md
│
└── examples/                       # Example workflows
    ├── simple-linear.yaml
    ├── conditional-routing.yaml
    ├── parallel-processing.yaml
    └── multi-language.yaml
```

## Quick Start

### 1. Understand the Requirements

Start with these files to understand what we're building:

1. **[questionAndAnswer.md](./questionAndAnswer.md)** - Complete Q&A defining requirements
2. **[features.md](./features.md)** - Feature list extracted from Q&A
3. **[basePlan.md](./basePlan.md)** - Research themes and deliverables

### 2. Review the Design

Read the design documents in this order:

1. **[design/architecture.md](./design/architecture.md)** - Start here! Overall system architecture
2. **[design/ir-schema.md](./design/ir-schema.md)** - How workflows are defined (YAML/JSON)
3. **[design/execution-model.md](./design/execution-model.md)** - How workflows are executed
4. **[design/state-scoping.md](./design/state-scoping.md)** - State management and security
5. **[design/worker-protocol.md](./design/worker-protocol.md)** - Multi-language RPC protocol

### 3. Understand Key Decisions

Review the ADRs to understand why we made certain choices:

1. **[decisions/001-canonical-ir-format.md](./decisions/001-canonical-ir-format.md)** - Why YAML/JSON for IR
2. **[decisions/002-cel-expression-language.md](./decisions/002-cel-expression-language.md)** - Why CEL for expressions
3. **[decisions/003-grpc-worker-protocol.md](./decisions/003-grpc-worker-protocol.md)** - Why gRPC for workers

### 4. Explore Examples

See workflows in action:

1. **[examples/simple-linear.yaml](./examples/simple-linear.yaml)** - Basic workflow (LLM + transform)
2. **[examples/conditional-routing.yaml](./examples/conditional-routing.yaml)** - Switch node, branching
3. **[examples/parallel-processing.yaml](./examples/parallel-processing.yaml)** - Parallel fan-out/fan-in
4. **[examples/multi-language.yaml](./examples/multi-language.yaml)** - Python, Go, TypeScript nodes

## Key Concepts

### Workflow Engine Architecture

The workflow engine is built as a **new orchestration layer** on top of existing SimpleAgents infrastructure:

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
```

**Zero Breaking Changes**: All existing SimpleAgents APIs continue to work unchanged.

### 15 Node Types

1. **LlmCall** - Invoke LLM provider
2. **Switch** - Conditional branching
3. **Loop** - Iteration with condition
4. **Map** - Parallel map over collection
5. **Reduce** - Aggregation
6. **Parallel** - Fan-out to multiple nodes
7. **Merge** - Fan-in with policies
8. **Subgraph** - Nested workflow invocation
9. **Filter** - Guard/short-circuit
10. **Batch** - Windowing/batching
11. **Transform** - Data transformation
12. **Cache** - Explicit cache control
13. **Retry** - Retry with backoff
14. **HumanApproval** - Human-in-the-loop
15. **CustomWorker** - Multi-language nodes

### Multi-Language Support

Nodes can be implemented in:
- **Rust** (native, in-process)
- **Python** (via gRPC worker)
- **Go** (via gRPC worker)
- **TypeScript** (via gRPC worker)

### State Model

Three layers of state:
1. **Global state**: Accessible to all nodes
2. **Scoped state**: Hierarchical parent-child scoping
3. **Node outputs**: Immutable references (`$.nodes.node_id.output`)

### Expression Language

**CEL (Common Expression Language)** for:
- Conditional branching
- Edge conditions
- Data transformations
- Filter predicates

## Implementation Phases (30 weeks)

| Phase | Weeks | Deliverable |
|-------|-------|-------------|
| 1: Foundation | 1-3 | Linear executor + LLM nodes |
| 2: Control Flow | 4-6 | Branching, loops, CEL |
| 3: Concurrency | 7-9 | Parallel, map/reduce |
| 4: Worker RPC | 10-13 | Multi-language nodes |
| 5: State & Capabilities | 14-16 | Scoping, subgraphs |
| 6: Replayability | 17-19 | Trace recording/replay |
| 7: Observability | 20-22 | Tracing, metrics |
| 8: Language Bindings | 23-26 | Python/Node/Go DSL |
| 9: Production | 27-30 | Hardening, docs |

## New Crates Structure

```
crates/
├── simple-agents-workflow-types/    # Pure types (Graph, Node, Edge)
├── simple-agents-workflow-engine/   # DAG executor, state, scheduler
├── simple-agents-workflow-expressions/ # CEL evaluator
├── simple-agents-workflow-workers/  # gRPC worker pool
└── simple-agents-workflow/          # High-level facade/DSL
```

## Example Workflow

```yaml
id: sentiment-analysis
version: 1.0.0
entry_node: analyze

nodes:
  - id: analyze
    node_type:
      llm_call:
        provider: openai
        model: gpt-4
        prompt: "Analyze sentiment: {{ input.text }}"

  - id: route
    node_type:
      switch:
        branches:
          - condition: '$.nodes.analyze.output.sentiment == "positive"'
            target: celebrate
          - condition: '$.nodes.analyze.output.sentiment == "negative"'
            target: investigate

  - id: celebrate
    node_type:
      transform:
        expression: '{"action": "send_thanks"}'

  - id: investigate
    node_type:
      custom_worker:
        language: python
        handler: InvestigateIssue

edges:
  - from: analyze
    to: route
```

## Critical Files for Implementation

When implementing, start with these files:

1. **`crates/simple-agents-workflow-types/src/graph.rs`** - Canonical IR types
2. **`crates/simple-agents-workflow-engine/src/executor.rs`** - DAG executor
3. **`crates/simple-agents-workflow-engine/src/state.rs`** - State manager
4. **`crates/simple-agents-workflow-expressions/src/cel.rs`** - CEL evaluator
5. **`crates/simple-agents-workflow-workers/proto/worker.proto`** - gRPC protocol

## Next Steps

1. **Read** [design/architecture.md](./design/architecture.md) for complete system overview
2. **Review** example workflows in [examples/](./examples/)
3. **Start** Phase 1 implementation (Foundation)

## Contributing

When adding new research or design documents:

1. **Design docs** → `design/` (detailed technical design with code)
2. **ADRs** → `decisions/` (architectural choices with alternatives)
3. **Examples** → `examples/` (working YAML workflows)
4. **Research** → `sections/` (exploratory research by topic)

## Questions?

Refer to:
- **Technical questions**: [design/architecture.md](./design/architecture.md)
- **Requirements**: [questionAndAnswer.md](./questionAndAnswer.md)
- **Feature list**: [features.md](./features.md)
- **Research topics**: [sections/](./sections/)
