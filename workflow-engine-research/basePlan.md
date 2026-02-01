# Base Research Plan (Rust-First Workflow Engine)

## Mission
- Ship agentic workflows to production quickly.
- Keep workflows testable, replayable, and observable.
- Provide a Rust core with language-agnostic node execution.

## Research Themes and Questions

### 1) Canonical IR and DSL
- Define the canonical node IR (JSON/YAML) and a code DSL that compiles to it.
- Decide required fields for node identity, inputs/outputs, schemas, permissions, runtime, retries, and streaming.
- Research how to model references to prior node outputs safely.
- Evaluate schema evolution and versioning within the IR.

### 2) Execution Engine Semantics
- Formalize the execution model for DAGs, loops, subgraphs, and fan-out/fan-in.
- Determine how merge/join policies work (first/all/quorum).
- Define how streaming interacts with node completion and edge transitions.
- Decide how compensation steps are modeled for fail-fast behavior.

### 3) Expression System
- Evaluate CEL as a primary expression language for conditions and routing.
- Identify how to support custom scripting and pluggable evaluators.
- Design a DSL test harness for expression validation (fixtures + expected branch).

### 4) State and Data Model
- Design hierarchical scoping + capability tokens.
- Decide on global vs scoped state boundaries and access control.
- Define schema validation rules (final payload validation before edge transition).
- Identify the best schema language (JSON Schema vs custom typed schema).

### 5) Determinism and Replayability
- Define replay requirements and where strict determinism is possible.
- Evaluate trace storage for inputs/outputs and decision points.
- Decide how to handle LLM non-determinism (cached responses, recorded traces).

### 6) Concurrency and Backpressure
- Research concurrency model for fan-out, worker pools, and rate limits.
- Define global vs node-level backpressure controls.
- Determine how to prevent per-request process spawn and prefer long-lived workers.

### 7) Language Workers and RPC
- Define the RPC contract for language workers (Rust/Python/Go/TS).
- Evaluate transport options (gRPC, HTTP, stdio) for local and remote workers.
- Determine worker lifecycle, warm pools, and resource limits.
- Evaluate WASM feasibility for sandboxing and portability.

### 8) Security and Isolation
- Define graph-based access policies and capability enforcement in the core.
- Research sandboxing strategies within a monolith.
- Decide what the secret manager stores vs config manager.

### 9) Observability and Debugging
- Define trace format and storage strategy.
- Identify metrics for latency, throughput, error rates, and cost.
- Ensure lineage tracking of node inputs/outputs.

### 10) Testing and Harness
- Define local runner behavior (fixture execution, deterministic replay, mocks).
- Determine golden trace format and validation strategy.
- Define contract tests for language workers.

### 11) Deployment and Performance Targets
- Research cold-start optimization in Rust (binary size, lazy init, cache warming).
- Define memory footprint targets per worker.
- Evaluate scaling strategies (pool size, max in-flight).

## Known Limitations and Risks
- Strict determinism is not always possible with LLMs.
- Cross-language worker consistency requires strict contract versioning.
- Streaming + validation introduces complexity in partial vs final handling.
- Graph-to-graph versioning may cause compatibility drift.
- Multi-runtime workers increase operational complexity.

## Pros and Cons of the Approach

### Pros
- Rust core provides performance, safety, and predictable memory usage.
- Canonical IR enables portability across languages and deployment targets.
- Long-lived workers minimize cold start and per-request overhead.
- Strong testing and replayability enable enterprise-grade reliability.

### Cons
- Higher upfront complexity to design a robust IR and execution semantics.
- Multi-language support adds maintenance burden.
- Replayability with LLMs requires careful trace storage and caching.
- Implementing human-in-the-loop and external integrations increases scope.

## Proposed Research Deliverables
- Draft canonical IR schema with examples.
- Expression engine evaluation doc (CEL + alternatives).
- Worker RPC contract spec.
- Execution semantics document (DAG, loop, fan-out/fan-in, streaming).
- Observability spec (trace + metrics).
- Testing harness design (local runner + golden traces).
- Security model and secret manager scope.

## Success Criteria
- Can define a workflow in DSL and compile to IR.
- Can execute a workflow with Rust core + one non-Rust worker.
- Can replay from any node using recorded trace data.
- Can run load tests without spawning per-request processes.
- Can validate schemas and stream outputs to clients.

## Immediate Next Steps
- Draft IR schema and node type taxonomy.
- Choose expression engine baseline (CEL or alternative).
- Pick an RPC transport for workers (prototype gRPC vs stdio).
- Define trace format and storage interface.
