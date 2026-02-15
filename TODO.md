# TODO - Workflow Engine Research Execution Backlog

Goal: execute the remaining `workflow-engine-research` phases to production readiness while preserving additive compatibility with existing SimpleAgents APIs.

## Canonical Context (read first)

- Research index: `workflow-engine-research/README.md`
- Feature inventory: `workflow-engine-research/features.md`
- Master phase plan: `workflow-engine-research/implementation-plan.md`
- Architecture design: `workflow-engine-research/design/architecture.md`
- Execution semantics: `workflow-engine-research/design/execution-model.md`
- Worker protocol design: `workflow-engine-research/design/worker-protocol.md`

## Current Baseline Already Implemented (reference only)

- IR + validation baseline: `crates/simple-agents-workflow/src/ir.rs`, `crates/simple-agents-workflow/src/validation.rs`
- Runtime baseline (linear + condition + retry/timeout/cancel): `crates/simple-agents-workflow/src/runtime.rs`
- Trace/replay baseline: `crates/simple-agents-workflow/src/trace.rs`, `crates/simple-agents-workflow/src/recorder.rs`, `crates/simple-agents-workflow/src/replay.rs`
- Worker pool baseline (in-process protocol/health/backpressure): `crates/simple-agents-workflow/src/worker.rs`
- Debug baseline + benchmarks: `crates/simple-agents-workflow/src/debug.rs`, `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`
- Binding parity baseline + CI gates: `parity-fixtures/binding_contract.json`, `.github/workflows/bindings-ci.yml`

## Working Model for This File

- Mark each item `[x]` only after code + tests + docs evidence.
- Add one evidence line under completed items: command and/or file path.
- If blocked, mark `[~]` with owner and blocker.
- Keep changes additive unless explicitly approved.

---

## Phase Delta Board (Remaining Work)

### Phase 2 - Control Flow Completion

- [x] Add expression engine foundation (parse/cache/validate/evaluate) and wire runtime conditions to it.
  - Target files: `crates/simple-agents-workflow/src/runtime.rs`, `crates/simple-agents-workflow/src/expressions.rs`
  - Evidence: `cargo test -p simple-agents-workflow` (includes `expressions::tests::*`).
- [x] Add CEL-compatible backend integration behind the expression engine abstraction.
  - Target files: `crates/simple-agents-workflow/src/expressions.rs`, `crates/simple-agents-workflow/Cargo.toml`
  - Design refs: `workflow-engine-research/sections/03-expression-system.md`, `workflow-engine-research/decisions/002-cel-expression-language.md`
  - Evidence: `ExpressionBackend::{Native,CelCompatible}` and `expressions::tests::supports_cel_compatible_backend_path` in `crates/simple-agents-workflow/src/expressions.rs`.
- [x] Implement `Loop` node semantics with max-iteration safety and accumulator behavior.
  - Target files: `crates/simple-agents-workflow/src/ir.rs`, `crates/simple-agents-workflow/src/runtime.rs`
  - Evidence: `runtime::tests::{executes_loop_until_condition_fails,fails_when_loop_exceeds_max_iterations}` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add expression fixture harness (input + expression + expected result/error).
  - Target files: `crates/simple-agents-workflow/tests/expression_fixtures.rs`, `crates/simple-agents-workflow/tests/fixtures/expression_cases.json`
  - Evidence: `cargo test -p simple-agents-workflow --test expression_fixtures`.

### Phase 3 - Concurrency Scheduler + Core Nodes

- [x] Implement DAG-ready scheduler that can run independent nodes concurrently with graph-level max in-flight limits.
  - Target files: new `crates/simple-agents-workflow/src/scheduler.rs`, `crates/simple-agents-workflow/src/runtime.rs`
  - Design refs: `workflow-engine-research/sections/06-concurrency-and-backpressure.md`
  - Evidence: `scheduler::tests::{respects_max_in_flight_limit,runs_concurrently_when_limit_above_one}` in `crates/simple-agents-workflow/src/scheduler.rs`.
- [x] Implement `Parallel` + `Merge` nodes with `first`/`all`/`quorum` policies.
  - Target files: `crates/simple-agents-workflow/src/ir.rs`, `crates/simple-agents-workflow/src/runtime.rs`
  - Evidence: `runtime::tests::executes_parallel_then_merge_all` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Implement `Map` + `Reduce` nodes with bounded fan-out.
  - Target files: `crates/simple-agents-workflow/src/ir.rs`, `crates/simple-agents-workflow/src/runtime.rs`
  - Evidence: `runtime::tests::{executes_map_reduce_sum,fails_map_when_items_path_is_not_array,rejects_map_when_item_count_exceeds_limit}` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add perf comparisons (sequential vs concurrent) and regression thresholds.
  - Target files: `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`
  - Evidence: benches `sequential_execute` + `concurrent_execute` and regression guard in `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`; verified by `cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10`.

### Phase 4 - gRPC Multi-Language Worker Runtime

- [x] Add worker proto and generated client/server contracts.
  - Target files: `crates/simple-agents-workflow-workers/proto/worker.proto` (new crate/path per plan)
  - Design refs: `workflow-engine-research/design/worker-protocol.md`, `workflow-engine-research/decisions/003-grpc-worker-protocol.md`
- [x] Implement Rust gRPC worker client/pool integration (health + routing + retries).
  - Target files: planned crate path `crates/simple-agents-workflow-workers/src/`
- [x] Land Python worker runtime and contract tests.
  - Target files: `workers/python/worker.py` (new), `workers/python/tests/`
- [x] Land Go worker runtime and contract tests.
  - Target files: `workers/go/worker.go` (new), `workers/go/tests/`
- [x] Land TypeScript worker runtime and contract tests.
  - Target files: `workers/typescript/worker.ts` (new), `workers/typescript/tests/`

### Phase 5 - State/Composition Feature Set

- [x] Extend scoped state to hierarchical parent-child model with explicit capability token checks.
  - Target files: `crates/simple-agents-workflow/src/runtime.rs`, new `crates/simple-agents-workflow/src/state/`
  - Design refs: `workflow-engine-research/design/state-scoping.md`, `workflow-engine-research/sections/04-state-and-data-model.md`
  - Evidence: `state::tests::{enforces_capabilities,supports_parent_child_visibility}` in `crates/simple-agents-workflow/src/state/mod.rs`.
- [x] Implement `Subgraph` node with isolated sub-scope and graph registry lookup.
  - Target files: `crates/simple-agents-workflow/src/ir.rs`, `crates/simple-agents-workflow/src/runtime.rs`
  - Evidence: `runtime::tests::executes_subgraph_via_registry` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Implement `Batch` + `Filter` nodes with deterministic behavior.
  - Target files: `crates/simple-agents-workflow/src/ir.rs`, `crates/simple-agents-workflow/src/runtime.rs`
  - Evidence: `runtime::tests::executes_batch_and_filter_nodes` in `crates/simple-agents-workflow/src/runtime.rs`.

### Phase 6 - Replayability Hardening

- [x] Add checkpoint storage abstraction and filesystem backend.
  - Target files: new `crates/simple-agents-workflow/src/checkpoint.rs`
  - Evidence: `CheckpointStore` + `FilesystemCheckpointStore` and `checkpoint::tests::round_trips_checkpoint_on_filesystem` in `crates/simple-agents-workflow/src/checkpoint.rs`.
- [x] Implement resume-from-failure-node execution path.
  - Target files: `crates/simple-agents-workflow/src/runtime.rs`, `crates/simple-agents-workflow/src/replay.rs`
  - Evidence: `WorkflowRuntime::execute_resume_from_failure` and `runtime::tests::resumes_from_checkpoint` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add replay mode options for cache policy (`always`, `refresh`, `mixed`).
  - Target files: `crates/simple-agents-workflow/src/replay.rs`
  - Evidence: `ReplayCachePolicy::{Always,Refresh,Mixed}` and `replay::tests::supports_cache_policy_options` in `crates/simple-agents-workflow/src/replay.rs`.

### Phase 7 - Observability Stack

- [x] Add OpenTelemetry spans per node and cross-worker trace propagation.
  - Target files: new `crates/simple-agents-workflow/src/observability/tracing.rs`
  - Evidence: tracing adapter API (`WorkflowTracer`/`WorkflowSpan`/`TraceContext`) and tests in `crates/simple-agents-workflow/src/observability/tracing.rs`.
- [x] Add Prometheus metrics for node latency, success/failure, queue depth, worker health.
  - Target files: new `crates/simple-agents-workflow/src/observability/metrics.rs`
  - Evidence: metrics adapter API (`WorkflowMetrics`) with in-memory implementation and tests in `crates/simple-agents-workflow/src/observability/metrics.rs`.
- [x] Add CLI debug surfaces for `trace`, `replay --from`, and `inspect` workflows.
  - Target files: CLI package path and docs in `docs/WORKFLOW_DEBUGGING.md`
  - Evidence: CLI subcommands `workflow trace`, `workflow replay`, `workflow inspect` in `crates/simple-agents-cli/src/main.rs` and docs in `docs/WORKFLOW_DEBUGGING.md`.

### Phase 8 - DSL and IR Authoring Maturity

- [x] Add workflow-DSL to canonical-IR golden tests across Rust/Python/Node/Go for advanced node set.
  - Target files: `parity-fixtures/`, binding test suites under `crates/simple-agents-py/tests/`, `crates/simple-agents-napi/test/`, `bindings/go/`
  - Evidence: `parity-fixtures/workflow_dsl_ir_golden.json`, `cargo test -p simple-agents-workflow --test workflow_dsl_ir_fixtures`, `uv run --directory crates/simple-agents-py --with "pytest>=8.0" pytest tests/test_contract_fixtures.py`, `npm --prefix crates/simple-agents-napi run test:contract`, `go test ./... -run 'TestGoBindingsFollowSharedContractFixture|TestWorkflowDSLFixturePreservesCanonicalIRWires' -count=1` (run in `bindings/go` with existing CGO env flags).
- [x] Add migration docs for YAML <-> code DSL with examples.
  - Target files: `docs/QUICKSTART.md`, `docs/ARCHITECTURE.md`, new cookbook docs
  - Evidence: `docs/QUICKSTART.md`, `docs/ARCHITECTURE.md`, `docs/WORKFLOW_DSL_MIGRATION_COOKBOOK.md`, `docs/DOCS_MAP.md`.

### Phase 9 - Production Hardening

- [x] Run performance profiling and optimize for target overhead and throughput.
  - Target files: `crates/simple-agents-workflow/benches/`, profiling docs under `docs/`
- Evidence: `docs/WORKFLOW_PERFORMANCE.md`, `.github/workflows/workflow-benches.yml`, `crates/simple-agents-workflow/benches/runtime_benchmarks.rs`.
- [x] Add security hardening: expression complexity limits, worker sandbox constraints, secret handling contracts.
  - Target files: runtime/worker modules + new security docs under `docs/`
- Evidence: `docs/WORKFLOW_SECURITY.md`, `crates/simple-agents-workflow/src/expressions.rs`, `crates/simple-agents-workflow/src/runtime.rs`, `crates/simple-agents-workflow/src/worker.rs`.
- [x] Expand example library to 10+ workflows and release checklist.
  - Target files: `workflow-engine-research/examples/`, `docs/`, `CHANGELOG.md` and release docs
- Evidence: `workflow-engine-research/examples/`, `docs/RELEASE_CHECKLIST.md`, `CHANGELOG.md`, `workflow-engine-research/README.md`.

---

## Node-Type Completion Checklist (from Research Features)

- [x] `switch/if` (baseline exists, CEL-grade completion pending)
  - Evidence: `ir::tests::{condition_deserializes_switch_alias,condition_deserializes_if_alias}` and `runtime::tests::{executes_conditional_branching,executes_conditional_false_branch}` in `crates/simple-agents-workflow/src/`.
- [x] `loop`
  - Evidence: `runtime::tests::{executes_loop_until_condition_fails,fails_when_loop_exceeds_max_iterations}` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `map`
  - Evidence: `runtime::tests::{executes_map_reduce_sum,fails_map_when_items_path_is_not_array}` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `reduce`
  - Evidence: `runtime::tests::executes_map_reduce_sum` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `parallel`
  - Evidence: `runtime::tests::executes_parallel_then_merge_all` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `merge/join/aggregate`
  - Evidence: `runtime::tests::executes_parallel_then_merge_all` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `subgraph`
  - Evidence: `runtime::tests::executes_subgraph_via_registry` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `filter/guard`
  - Evidence: `runtime::tests::executes_batch_and_filter_nodes` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `batch/window`
  - Evidence: `runtime::tests::executes_batch_and_filter_nodes` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `debounce/throttle`
  - Evidence: `runtime::tests::executes_debounce_and_throttle_nodes` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `retry/compensate` node type (distinct from policy wrappers)
  - Evidence: `runtime::tests::{executes_retry_compensate_successfully_without_compensation,executes_retry_compensate_with_compensation_route}` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `human-in-the-loop`
  - Evidence: `runtime::tests::executes_event_cache_router_human_transform_nodes` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `cache read/write`
  - Evidence: `runtime::tests::executes_event_cache_router_human_transform_nodes` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `event trigger`
  - Evidence: `runtime::tests::{executes_event_cache_router_human_transform_nodes,routes_event_trigger_mismatch_to_fallback}` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `router/selector`
  - Evidence: `runtime::tests::executes_event_cache_router_human_transform_nodes` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] `transform` (canonical node shape + tests)
  - Evidence: `runtime::tests::executes_event_cache_router_human_transform_nodes` in `crates/simple-agents-workflow/src/runtime.rs`.

Reference feature source: `workflow-engine-research/features.md`

---

## Execution Context Diagram

```text
            +------------------------------+
            |  Workflow Definition (IR)    |
            |  start -> ... -> end         |
            +--------------+---------------+
                           |
                           v
            +------------------------------+
            | Workflow Runtime/Scheduler   |
            | - state scopes               |
            | - retries/timeouts           |
            | - trace + replay             |
            +------+---------------+-------+
                   |               |
           in-proc |               | gRPC workers (target)
                   v               v
      +-------------------+   +--------------------------+
      | simple-agents-core|   | python/go/ts worker procs|
      +-------------------+   +--------------------------+
```

## Sample Target Workflow (Phase 3+)

```yaml
id: research-assistant
version: v1
entry: start
nodes:
  - id: start
    type: start
    next: classify

  - id: classify
    type: llm
    model: gpt-4o-mini
    prompt: "Classify the request intent"
    next: route

  - id: route
    type: switch
    expression: "$.node_outputs.classify.intent == 'deep_research'"
    on_true: fanout
    on_false: quick_answer

  - id: fanout
    type: parallel
    branches: [web_worker, docs_worker, code_worker]
    next: merge

  - id: merge
    type: merge
    policy: quorum
    quorum: 2
    next: summarize

  - id: summarize
    type: llm
    model: gpt-4.1
    prompt: "Synthesize branch outputs"
    next: end

  - id: quick_answer
    type: transform
    expression: '{"mode":"quick"}'
    next: end

  - id: end
    type: end
```

## Sample Phase-by-Phase Verification Commands

- Core workflow crate tests: `cargo test -p simple-agents-workflow`
- Benchmarks: `cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10`
- Binding contract parity: `./scripts/run-binding-contracts.sh`
- Layered binding suites: `./scripts/run-binding-tests-layered.sh`
- Workspace sanity: `cargo check --workspace`
