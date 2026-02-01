# Workflow Engine Implementation Plan (30 Weeks)

## Overview

This document outlines a 30-week plan to implement the workflow engine for SimpleAgents. The implementation is divided into 9 phases, each with clear deliverables, success criteria, and dependencies.

## Principles

- **Zero breaking changes**: All existing SimpleAgents APIs continue to work
- **Incremental delivery**: Each phase produces a working artifact
- **Test-driven**: Write tests before implementation
- **Production-ready**: Focus on reliability, observability, and performance

## Phase Summary

| Phase | Weeks | Focus | Key Deliverable |
|-------|-------|-------|-----------------|
| 1 | 1-3 | Foundation | Linear DAG executor with LLM nodes |
| 2 | 4-6 | Control Flow | Branching, loops, CEL expressions |
| 3 | 7-9 | Concurrency | Parallel execution, map/reduce |
| 4 | 10-13 | Multi-Language | gRPC workers, Python/Go/TS nodes |
| 5 | 14-16 | State & Composition | State scoping, subgraphs |
| 6 | 17-19 | Replayability | Trace recording, deterministic replay |
| 7 | 20-22 | Observability | Distributed tracing, metrics, dashboards |
| 8 | 23-26 | Language Bindings | Python/Node/Go DSL libraries |
| 9 | 27-30 | Production Hardening | Docs, examples, performance tuning |

---

## Phase 1: Foundation (Weeks 1-3)

### Goal
Build the core DAG executor that can run linear workflows with LLM nodes.

### Deliverables

#### Week 1: Project Setup & Type System
- **Crate structure**:
  ```
  crates/
  ├── simple-agents-workflow-types/      # Pure types
  ├── simple-agents-workflow-engine/     # Executor
  ├── simple-agents-workflow-expressions/ # CEL (stub)
  ├── simple-agents-workflow-workers/    # Workers (stub)
  └── simple-agents-workflow/            # Facade
  ```
- **Core types**:
  - `WorkflowGraph`, `NodeDefinition`, `EdgeDefinition`
  - `NodeType` enum with 15 variants
  - `ExecutionContext`, `NodeOutput`
- **Serialization**:
  - Implement `Serialize`/`Deserialize` for all types
  - YAML/JSON round-trip tests
- **Validation**:
  - Graph validation (no dangling edges, entry node exists)
  - Cycle detection (topological sort)

#### Week 2: Linear DAG Executor
- **Executor core**:
  - `WorkflowEngine::execute(graph, input)` → `Result<Value>`
  - Topological ordering of nodes
  - Sequential execution (no parallelism yet)
- **LLM node integration**:
  - `LlmCallNode` implementation using existing `SimpleAgentsClient`
  - Prompt template rendering (Handlebars or similar)
  - Input/output schema validation (JSON Schema)
- **State management**:
  - Global state (`HashMap<String, Value>`)
  - Node output storage (`HashMap<NodeId, Value>`)
  - Node output references (`$.nodes.node_id.output`)

#### Week 3: Transform Node & Testing
- **Transform node**:
  - `TransformNode` for data transformation
  - CEL expression evaluation (stub with simple JSON path)
- **Testing infrastructure**:
  - Local runner (no external dependencies)
  - Example workflows (linear LLM chains)
  - Unit tests for executor, state manager
- **Documentation**:
  - Getting started guide
  - Example: Simple LLM chain (prompt → LLM → transform)

### Success Criteria
- ✅ Can execute a 3-node linear workflow (LLM → LLM → Transform)
- ✅ YAML workflow definitions load and execute
- ✅ State is correctly passed between nodes
- ✅ All tests pass

### Dependencies
- Existing: `simple-agents` crate for LLM calls
- New: `serde_yaml`, `serde_json`, `tokio`

---

## Phase 2: Control Flow (Weeks 4-6)

### Goal
Add conditional branching, loops, and CEL expression evaluation.

### Deliverables

#### Week 4: CEL Expression Engine
- **CEL integration**:
  - Evaluate CEL via `cel-interpreter` crate or FFI to cel-go
  - Expression caching (parse once, reuse)
  - Expression validation at graph compile time
- **Supported expressions**:
  - JSON path: `$.nodes.analyze.output.sentiment`
  - Comparisons: `confidence > 0.8`
  - Logic: `sentiment == "positive" && confidence > 0.7`
  - Math: `(total + 1) * 2`
  - Functions: `size(items) > 10`
- **Testing**:
  - Expression test harness (fixtures → expected result)
  - Error handling for invalid expressions

#### Week 5: Switch Node (Conditional Branching)
- **Switch node implementation**:
  - Evaluate condition expressions
  - Select target node based on branch condition
  - Default branch fallback
- **Multi-way branching**:
  - Support for range conditions (not just binary)
  - First-match semantics
- **Testing**:
  - Example: Sentiment routing (positive → celebrate, negative → investigate)

#### Week 6: Loop Node
- **Loop node implementation**:
  - Iteration with condition (`while` semantics)
  - Accumulator state
  - Max iterations safety limit
- **Testing**:
  - Example: Retry with exponential backoff
  - Loop termination conditions

### Success Criteria
- ✅ Can execute workflows with conditional branches (Switch node)
- ✅ Can execute workflows with loops
- ✅ CEL expressions evaluate correctly
- ✅ Complex branching logic works (multi-way switch)

### Dependencies
- Phase 1 complete
- New: `cel-interpreter` or FFI bindings to cel-go

---

## Phase 3: Concurrency (Weeks 7-9)

### Goal
Enable parallel node execution, map/reduce patterns, and merge nodes.

### Deliverables

#### Week 7: Parallel Execution
- **Parallel executor**:
  - Identify independent nodes in topological sort
  - Execute independent nodes concurrently (Tokio tasks)
  - Wait for all predecessors before executing a node
- **Concurrency control**:
  - Max concurrent nodes limit (graph-level config)
  - Backpressure when limit reached
- **Testing**:
  - Parallel fan-out example (3 LLM calls in parallel)
  - Performance comparison (sequential vs parallel)

#### Week 8: Map/Reduce Nodes
- **Map node**:
  - Iterate over collection
  - Execute subgraph for each item in parallel
  - Collect results
- **Reduce node**:
  - Aggregate collection into single value
  - Pluggable aggregation functions (sum, concat, custom)
- **Testing**:
  - Example: Parallel document summarization (map LLM calls, reduce summaries)

#### Week 9: Merge Node
- **Merge node**:
  - Fan-in from multiple parallel branches
  - Merge policies:
    - `first`: Return first completed branch
    - `all`: Wait for all branches, return array
    - `quorum`: Wait for N branches
  - Timeout handling
- **Testing**:
  - Example: Multi-provider LLM call (race between OpenAI, Anthropic, Google)

### Success Criteria
- ✅ Can execute nodes in parallel when independent
- ✅ Map/reduce patterns work correctly
- ✅ Merge node implements all policies (first, all, quorum)
- ✅ Performance improvement from parallelism (benchmarks)

### Dependencies
- Phase 2 complete
- No new external dependencies

---

## Phase 4: Multi-Language Workers (Weeks 10-13)

### Goal
Enable workflow nodes to execute code in Python, Go, and TypeScript via gRPC.

### Deliverables

#### Week 10: gRPC Protocol & Rust Client
- **Proto definition**:
  - `WorkerService` with `ExecuteNode` and `Health` RPCs
  - Protobuf message definitions (`ExecuteNodeRequest`, `ExecuteNodeResponse`)
  - Streaming support for progressive results
- **Rust client**:
  - `WorkerPool` struct (ADR-004)
  - `WorkerHandle` for each worker process
  - gRPC client via Tonic
- **Testing**:
  - Mock gRPC server for testing

#### Week 11: Python Worker
- **Python worker process**:
  - gRPC server (`grpcio`)
  - Handler registration system
  - Example handlers (data processing, API calls)
- **Worker pool integration**:
  - Start Python workers on engine init
  - Health checks
  - Request routing
- **Testing**:
  - Contract tests (Rust client ↔ Python server)
  - Example workflow with Python node

#### Week 12: Go & TypeScript Workers
- **Go worker**:
  - gRPC server (`grpc-go`)
  - Handler registration
  - Example: High-performance data transformation
- **TypeScript worker**:
  - gRPC server (`@grpc/grpc-js`)
  - Handler registration
  - Example: API integration with fetch
- **Testing**:
  - Contract tests for both languages
  - Multi-language workflow example

#### Week 13: Health Tracking & Circuit Breaking
- **Health tracker** (ADR-005):
  - Active health checks (periodic gRPC Health calls)
  - Circuit breaker state machine (Closed → Open → Half-Open)
  - Health-aware request routing
- **Graceful degradation**:
  - Continue with healthy workers when some fail
  - Automatic recovery when workers return
- **Testing**:
  - Simulate worker failures
  - Verify circuit breaking behavior

### Success Criteria
- ✅ Can execute Python, Go, and TypeScript nodes via gRPC
- ✅ Worker pool manages long-lived worker processes
- ✅ Health tracking detects and recovers from failures
- ✅ Multi-language workflow executes successfully

### Dependencies
- Phase 3 complete
- New: `tonic` (gRPC), `prost` (protobuf)
- Worker runtimes: Python 3.11+, Go 1.21+, Node 20+

---

## Phase 5: State & Composition (Weeks 14-16)

### Goal
Implement hierarchical state scoping, capability-based access control, and subgraph nodes.

### Deliverables

#### Week 14: Hierarchical State Scoping
- **State scoping model** (ADR-007):
  - Global state (accessible to all nodes)
  - Scoped state (parent-child hierarchy)
  - Node output references (immutable)
- **Capability tokens**:
  - Grant/deny node access to specific resources
  - Resource types: `model:*`, `secret:*`, `api:*`
  - Capability inheritance and delegation
- **Testing**:
  - Test state isolation between scopes
  - Verify capability enforcement

#### Week 15: Subgraph Node
- **Subgraph node**:
  - Invoke another workflow as a node
  - Pass inputs to subgraph
  - Receive outputs from subgraph
  - State isolation (subgraph has its own scoped state)
- **Graph-to-graph references**:
  - Version resolution (latest compatible version)
  - Subgraph registry
- **Testing**:
  - Nested subgraph example (3 levels deep)
  - State isolation verification

#### Week 16: Batch & Filter Nodes
- **Batch node**:
  - Collect N items or time window
  - Emit batch when full
  - Windowing strategies (tumbling, sliding)
- **Filter node**:
  - Predicate-based filtering
  - Drop items that don't match
  - Short-circuit on condition
- **Testing**:
  - Batch processing example (window aggregation)
  - Filter example (quality threshold)

### Success Criteria
- ✅ State scoping enforces isolation
- ✅ Capability tokens control resource access
- ✅ Subgraph nodes execute nested workflows
- ✅ Batch and filter nodes work correctly

### Dependencies
- Phase 4 complete
- No new external dependencies

---

## Phase 6: Replayability (Weeks 17-19)

### Goal
Record execution traces for debugging, testing, and deterministic replay.

### Deliverables

#### Week 17: Trace Recording
- **Trace format** (ADR-008):
  - Event-sourced trace (append-only log of events)
  - Events: NodeStarted, NodeCompleted, NodeFailed, EdgeTraversed
  - Store inputs, outputs, and timestamps
- **Trace storage**:
  - Filesystem backend (JSON files)
  - Trace ID (UUID per execution)
- **Testing**:
  - Record trace for example workflow
  - Verify trace contains all events

#### Week 18: Deterministic Replay
- **Replay engine**:
  - Load trace from storage
  - Replay execution from start or specific node
  - Use recorded outputs instead of re-executing
- **LLM response caching**:
  - Cache LLM responses in trace
  - Reuse cached responses on replay
  - Configurable cache behavior (always use cache vs refresh)
- **Testing**:
  - Replay workflow from trace
  - Verify identical results

#### Week 19: Checkpoints & Resume
- **Checkpoint system**:
  - Save workflow state at intervals
  - Resume from checkpoint on failure
  - Checkpoint strategy (time-based, node-based)
- **Resume from failure**:
  - Detect failed node
  - Resume execution from that node
  - Skip already-completed nodes
- **Testing**:
  - Simulate failure mid-workflow
  - Resume and verify completion

### Success Criteria
- ✅ Execution traces record all events
- ✅ Can replay workflow from trace deterministically
- ✅ Can resume workflow from checkpoint after failure
- ✅ LLM response caching works for replay

### Dependencies
- Phase 5 complete
- New: Storage backend (start with filesystem, later S3/DB)

---

## Phase 7: Observability (Weeks 20-22)

### Goal
Add distributed tracing, metrics, and debugging tools.

### Deliverables

#### Week 20: Distributed Tracing
- **OpenTelemetry integration**:
  - Span per node execution
  - Trace propagation across workers
  - Span attributes (node ID, type, inputs, outputs)
- **Trace export**:
  - Export to Jaeger, Zipkin, or Honeycomb
  - Local development UI
- **Testing**:
  - Verify traces appear in UI
  - Check span hierarchy matches DAG

#### Week 21: Metrics & Monitoring
- **Prometheus metrics**:
  - Node execution count, duration, success/failure rate
  - Worker pool health metrics
  - Queue depth, throughput
- **Dashboards**:
  - Grafana dashboard templates
  - Pre-built queries for common metrics
- **Testing**:
  - Verify metrics are exported
  - Load test and observe metrics

#### Week 22: Debug Tooling
- **Debug snapshots**:
  - Capture full state at any node
  - Inspect inputs, outputs, and scoped state
- **Lineage tracking**:
  - Track data flow from input to output
  - Visualize dependencies
- **CLI tools**:
  - `workflow trace <execution-id>` - View execution trace
  - `workflow replay <execution-id> --from <node>` - Replay from node
  - `workflow inspect <execution-id> <node>` - Inspect node state
- **Testing**:
  - Use debug tools on example workflows

### Success Criteria
- ✅ Distributed traces show node-level spans
- ✅ Metrics exported to Prometheus
- ✅ Debug tools help diagnose issues
- ✅ Grafana dashboards visualize workflow health

### Dependencies
- Phase 6 complete
- New: `opentelemetry`, `prometheus`

---

## Phase 8: Language Bindings (Weeks 23-26)

### Goal
Provide Python, TypeScript, and Go DSL libraries for authoring workflows.

### Deliverables

#### Week 23: Python DSL (PyO3)
- **Python bindings** (ADR-006):
  - `simple_agents.workflow` package
  - Builder classes (`WorkflowGraph`, `Node`, `Edge`)
  - Type hints for IDE support
- **Examples**:
  - Python-native workflow authoring
  - Compile to YAML/JSON
- **Testing**:
  - Python unit tests
  - Verify Python DSL → YAML parity

#### Week 24: TypeScript DSL (NAPI)
- **TypeScript bindings**:
  - `@simple-agents/workflow` npm package
  - Type-safe builders with TypeScript generics
  - JSDoc for autocomplete
- **Examples**:
  - TypeScript workflow authoring
  - Integration with existing Node.js projects
- **Testing**:
  - Jest unit tests
  - Verify TS DSL → YAML parity

#### Week 25: Go DSL (cgo)
- **Go bindings**:
  - `github.com/simple-agents/workflow-go` module
  - Idiomatic Go API (builders, options pattern)
- **Examples**:
  - Go workflow authoring
  - Integration with Go services
- **Testing**:
  - Go unit tests
  - Verify Go DSL → YAML parity

#### Week 26: DSL Testing & Documentation
- **Cross-language tests**:
  - Verify all DSLs produce identical IR
  - Golden file tests (DSL → YAML snapshot)
- **Documentation**:
  - API reference for each language
  - Migration guide (YAML ↔ Code)
  - Examples in all 4 languages

### Success Criteria
- ✅ Can author workflows in Python, TypeScript, Go, and Rust
- ✅ All DSLs compile to identical canonical IR
- ✅ Comprehensive API documentation
- ✅ Examples in all languages

### Dependencies
- Phase 7 complete
- New: `pyo3` (Python), `napi-rs` (Node), cgo (Go)

---

## Phase 9: Production Hardening (Weeks 27-30)

### Goal
Prepare for production deployment with docs, examples, and performance tuning.

### Deliverables

#### Week 27: Performance Optimization
- **Profiling**:
  - CPU profiling (flamegraphs)
  - Memory profiling (heap snapshots)
  - Identify bottlenecks
- **Optimizations**:
  - Reduce allocations (object pooling)
  - Optimize state access (sharding, caching)
  - Worker connection pooling
- **Benchmarks**:
  - Throughput tests (req/sec)
  - Latency percentiles (p50, p95, p99)
  - Resource usage (memory, CPU)
- **Targets**:
  - 1000+ simple workflows/sec on single machine
  - <10ms overhead per node (excluding LLM calls)
  - <1GB memory for 10 concurrent workflows

#### Week 28: Security Hardening
- **Input validation**:
  - Sanitize user inputs
  - Limit expression complexity
  - Prevent injection attacks
- **Sandboxing**:
  - Process isolation for workers
  - Resource limits (CPU, memory, network)
  - Capability enforcement
- **Secret management**:
  - Encrypted secret storage
  - Integration with external secret managers (AWS Secrets Manager, etc.)
- **Testing**:
  - Security audit
  - Penetration testing

#### Week 29: Documentation
- **User guide**:
  - Getting started tutorial
  - Concepts (nodes, edges, state, expressions)
  - Best practices
- **API reference**:
  - Rustdoc, Pydoc, Typedoc, Godoc
  - Generated from code
- **Architecture guide**:
  - System architecture diagram
  - Data flow diagrams
  - Deployment patterns
- **Cookbook**:
  - Common workflows (API integration, data processing, multi-step LLM)
  - Performance tips
  - Troubleshooting guide

#### Week 30: Example Library & Release
- **Example workflows**:
  - Customer support automation
  - Document processing pipeline
  - Multi-agent research assistant
  - Data validation and enrichment
- **Release checklist**:
  - Changelog
  - Migration guide (for SimpleAgents users)
  - Semantic versioning
  - CI/CD pipeline
- **Community**:
  - GitHub repository structure
  - Contribution guide
  - Issue templates
  - Discord/Slack channel

### Success Criteria
- ✅ Performance meets targets (1000+ workflows/sec)
- ✅ Security audit passes
- ✅ Comprehensive documentation
- ✅ 10+ example workflows
- ✅ Ready for production use

### Dependencies
- Phase 8 complete
- All phases complete and tested

---

## Milestones

### M1: Foundation Complete (Week 3)
- Linear DAG executor works
- LLM nodes integrated
- Basic state management

### M2: Control Flow Complete (Week 6)
- CEL expressions work
- Branching and loops implemented
- Complex workflows possible

### M3: Concurrency Complete (Week 9)
- Parallel execution
- Map/reduce patterns
- Performance gains from parallelism

### M4: Multi-Language Complete (Week 13)
- Python, Go, TypeScript workers
- gRPC communication
- Health tracking

### M5: Composition Complete (Week 16)
- State scoping
- Subgraphs
- Batch/filter nodes

### M6: Replayability Complete (Week 19)
- Trace recording
- Deterministic replay
- Resume from failure

### M7: Observability Complete (Week 22)
- Distributed tracing
- Metrics and dashboards
- Debug tools

### M8: Language Bindings Complete (Week 26)
- Python, TypeScript, Go DSLs
- Cross-language parity
- Documentation

### M9: Production Ready (Week 30)
- Performance optimized
- Security hardened
- Documentation complete
- Example library

---

## Risk Management

### High-Risk Items

1. **CEL Integration Complexity** (Phase 2)
   - Risk: CEL FFI may be complex or slow
   - Mitigation: Evaluate native Rust CEL interpreter as backup
   - Contingency: Start with simple JSON path, add CEL later

2. **Worker Pool Stability** (Phase 4)
   - Risk: Worker processes may crash or leak memory
   - Mitigation: Robust health checking, automatic restart
   - Contingency: Fallback to per-request spawning if needed

3. **Performance Targets** (Phase 9)
   - Risk: May not meet 1000 workflows/sec target
   - Mitigation: Profile early, optimize incrementally
   - Contingency: Document actual performance, set realistic expectations

4. **Language Binding Maintenance** (Phase 8)
   - Risk: Keeping 4 language DSLs in sync is complex
   - Mitigation: Generate bindings from canonical IR where possible
   - Contingency: Focus on Rust + Python only initially

### Medium-Risk Items

1. **Trace Storage Scalability** (Phase 6)
   - Risk: Filesystem storage may not scale
   - Mitigation: Design abstraction for pluggable backends
   - Contingency: Start with filesystem, add S3/DB later

2. **Subgraph Versioning** (Phase 5)
   - Risk: Version resolution may be complex
   - Mitigation: Start simple (latest version only)
   - Contingency: Add semantic versioning later

3. **Documentation Quality** (Phase 9)
   - Risk: Documentation may be incomplete or unclear
   - Mitigation: Write docs as we build, user testing
   - Contingency: Iterate based on early user feedback

---

## Resource Requirements

### Team
- 2 Rust engineers (core engine, workers, bindings)
- 1 DevOps engineer (CI/CD, deployment, monitoring)
- 1 Technical writer (documentation, examples)

### Infrastructure
- CI/CD: GitHub Actions
- Testing: Pytest, Jest, Go test
- Monitoring: Prometheus + Grafana
- Tracing: Jaeger or Honeycomb
- Hosting: Self-hosted or cloud (AWS, GCP, Azure)

### External Dependencies
- Rust crates: tokio, serde, tonic, prost, cel-interpreter, opentelemetry, prometheus
- Python: grpcio, pyo3
- Go: grpc-go
- TypeScript: @grpc/grpc-js, napi-rs

---

## Success Metrics

### Functional Metrics
- All 15 node types implemented and tested
- All 4 language DSLs (Rust, Python, TypeScript, Go) working
- 100% test coverage on core execution engine
- 10+ example workflows

### Performance Metrics
- 1000+ workflows/sec throughput (single machine)
- <10ms overhead per node
- <1GB memory for 10 concurrent workflows
- P95 latency <100ms for simple workflows

### Quality Metrics
- Zero breaking changes to existing SimpleAgents API
- 90%+ documentation coverage
- Security audit passed
- Production-ready by week 30

---

## Next Steps

1. **Week 1**: Set up crate structure, define core types
2. **Week 2**: Implement linear DAG executor
3. **Week 3**: Add transform node, write tests
4. **Week 4**: Integrate CEL expression evaluator
5. Continue following the phase plan...

---

## Appendix A: Crate Dependency Graph

```
simple-agents-workflow (facade)
├── simple-agents-workflow-engine
│   ├── simple-agents-workflow-types
│   ├── simple-agents-workflow-expressions
│   ├── simple-agents-workflow-workers
│   └── simple-agents (existing)
├── simple-agents-workflow-expressions
│   └── simple-agents-workflow-types
├── simple-agents-workflow-workers
│   └── simple-agents-workflow-types
└── simple-agents-workflow-types
```

## Appendix B: Key Files to Implement

### Phase 1
- `crates/simple-agents-workflow-types/src/graph.rs`
- `crates/simple-agents-workflow-types/src/node.rs`
- `crates/simple-agents-workflow-engine/src/executor.rs`
- `crates/simple-agents-workflow-engine/src/state.rs`

### Phase 2
- `crates/simple-agents-workflow-expressions/src/cel.rs`
- `crates/simple-agents-workflow-engine/src/nodes/switch.rs`
- `crates/simple-agents-workflow-engine/src/nodes/loop.rs`

### Phase 3
- `crates/simple-agents-workflow-engine/src/scheduler.rs`
- `crates/simple-agents-workflow-engine/src/nodes/map.rs`
- `crates/simple-agents-workflow-engine/src/nodes/reduce.rs`

### Phase 4
- `crates/simple-agents-workflow-workers/proto/worker.proto`
- `crates/simple-agents-workflow-workers/src/pool.rs`
- `workers/python/worker.py`
- `workers/go/worker.go`
- `workers/typescript/worker.ts`

### Phase 5
- `crates/simple-agents-workflow-engine/src/state/scoping.rs`
- `crates/simple-agents-workflow-engine/src/nodes/subgraph.rs`

### Phase 6
- `crates/simple-agents-workflow-engine/src/trace/recorder.rs`
- `crates/simple-agents-workflow-engine/src/trace/replay.rs`

### Phase 7
- `crates/simple-agents-workflow-engine/src/observability/tracing.rs`
- `crates/simple-agents-workflow-engine/src/observability/metrics.rs`

### Phase 8
- `bindings/python/` (PyO3)
- `bindings/node/` (NAPI)
- `bindings/go/` (cgo)
