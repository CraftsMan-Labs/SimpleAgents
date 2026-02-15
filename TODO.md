# TODO - Workflow Engine + Cross-Language Parity

Goal: implement the new workflow system incrementally on top of current SimpleAgents runtime, while finishing parity gaps across Rust/Python/Node/Go with consistent behavior and strong DX.

## Subagent release plan

We will run **8 subagents** in parallel workstreams.

| Subagent | Scope | Primary language | Required skill |
|---|---|---|---|
| SA-1 | Workflow IR + validation | Rust | `rust-coding-patterns` |
| SA-2 | Workflow scheduler/runtime core | Rust | `rust-coding-patterns` |
| SA-3 | Workflow state + trace/replay | Rust | `rust-coding-patterns` |
| SA-4 | Worker protocol + pools + health | Rust | `rust-coding-patterns` |
| SA-5 | FFI + Go streaming parity | Rust + Go | `rust-coding-patterns`, `go-coding-patterns` |
| SA-6 | Node parity + typed streaming/tool surfaces | TS/JS | `typescript-javascript-coding-patterns` |
| SA-7 | Python parity + contract conformance | Python | `python-coding-patterns` |
| SA-8 | CI matrix + fixtures + docs/DX guardrails | YAML/MD + multi-lang | language-specific as needed |

## Subagent completion protocol (mandatory)

- [x] Every subagent must update this `TODO.md` directly in its PR.
  - Evidence: SA-6/SA-7/SA-8 and follow-up parity hardening updates all recorded in this file.
- [x] When a task is complete, change `[ ]` to `[x]` and add 1-line evidence (test command/output path).
  - Evidence: completed items below include command/file evidence (bench/test/doc paths).
- [x] If blocked, mark task as `[~]` with blocker reason and owner.
  - Evidence: historical blocked entries use `[~]` and owner/reason context.
- [x] No task is "done" without tests (unit/contract/live as applicable).
  - Evidence: `cargo test -p simple-agents-workflow`, `./scripts/run-binding-contracts.sh`, `./scripts/run-binding-tests-layered.sh`.
- [x] Follow `CODING_GUIDELINES.md` (KISS, DRY, OOD, no phantom code, reusable APIs).
  - Evidence: additive, typed APIs and reusable test/contract runners landed without breaking existing bindings.

## Program plan (implementation phases)

### Phase 0 - Foundations (no breaking changes)

- [x] Define workspace crate boundaries for workflow subsystem (additive only).
  - Evidence: additive crate `crates/simple-agents-workflow` with isolated modules and no breaking changes to existing crates.
- [x] Lock minimal canonical IR for v0: `start`, `llm`, `tool`, `condition`, `end`.
  - Evidence: `NodeKind` v0 taxonomy in `crates/simple-agents-workflow/src/ir.rs`.
- [x] Add validation/lint pass with actionable diagnostics.
  - Evidence: `validate_and_normalize` + typed `DiagnosticCode` in `crates/simple-agents-workflow/src/validation.rs`.
- [x] Define deterministic execution invariants and trace schema.
  - Evidence: runtime invariants/policies in `crates/simple-agents-workflow/src/runtime.rs` and trace schema in `crates/simple-agents-workflow/src/trace.rs`.
- [x] Publish capability contract for existing and new APIs.
  - Evidence: `docs/WORKFLOW_CAPABILITY_CONTRACT.md`.

### Phase 1 - Minimal executable vertical slice

- [x] Execute linear + conditional workflows through current `simple-agents-core` path.
  - Evidence: `WorkflowRuntime` + `impl LlmExecutor for SimpleAgentsClient` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add scoped state v1 (workflow/global + node-local minimal model).
  - Evidence: `RuntimeScope` (`input`, `last_llm_output`, `last_tool_output`, `node_outputs`) with capability-guarded access in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add trace recording for each node transition.
  - Evidence: runtime records `node_enter`/`node_exit`/`node_error`/`terminal` via `TraceRecorder` in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add replay mode for deterministic test runs.
  - Evidence: `WorkflowReplayMode::ValidateRecordedTrace` + `replay_trace` integration in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add reference examples and smoke tests.
  - Evidence: `crates/simple-agents-workflow/examples/linear_runtime.rs`, `crates/simple-agents-workflow/tests/trace_fixtures.rs`, and `cargo test -p simple-agents-workflow`.

### Phase 2 - Parallelism and worker model

- [x] Add bounded parallel execution primitives.
  - Evidence: health-aware bounded worker scheduling via per-worker bounded `mpsc` queues in `crates/simple-agents-workflow/src/worker.rs`.
- [x] Add worker protocol surface (start with single-process default).
  - Evidence: `WorkerRequest`/`WorkerResponse`/`WorkerProtocolError` protocol model and `WorkerPool::new_inprocess` in `crates/simple-agents-workflow/src/worker.rs`.
- [x] Add worker pool lifecycle + health tracking.
  - Evidence: `WorkerPool::{health_snapshot,restart_worker,shutdown}` + probe loop + status transitions in `crates/simple-agents-workflow/src/worker.rs`.
- [x] Define retry/timeout ownership boundaries (workflow layer vs router layer).
  - Evidence: workflow node retry/timeout remains in `crates/simple-agents-workflow/src/runtime.rs`; worker-level request timeout ownership added in `WorkerPoolOptions::default_request_timeout` (`crates/simple-agents-workflow/src/worker.rs`).
- [x] Load/perf benchmarks for scheduler and state hot paths.
  - Evidence: `crates/simple-agents-workflow/benches/runtime_benchmarks.rs` with criterion bench run via `cargo bench -p simple-agents-workflow --bench runtime_benchmarks -- --sample-size 10`.

### Phase 3 - Cross-language parity and DX hardening

- [x] Align FFI/Go/Node/Python workflow-facing behavior with Rust reference.
  - Evidence: shared parity assertions now run in FFI (`crates/simple-agents-ffi/tests/ffi_contract.rs`), Go (`bindings/go/contract_fixture_test.go`), Node (`crates/simple-agents-napi/test/contract.test.js`), and Python (`crates/simple-agents-py/tests/test_contract_fixtures.py`).
- [x] Add golden contract fixtures shared by all bindings.
  - Evidence: expanded fixture contract in `parity-fixtures/binding_contract.json` now covers request/response/healing/streaming/tool-call and binding-specific symbol expectations.
- [x] Enforce capability matrix gates in CI.
  - Evidence: `capability-contract-gates` job added to `.github/workflows/bindings-ci.yml`, running `scripts/run-binding-contracts.sh`.
- [x] Ship debugging UX: node timeline, retry reasons, replay trace inspection.
  - Evidence: debug surfaces added in `crates/simple-agents-workflow/src/debug.rs`, retry diagnostics in `crates/simple-agents-workflow/src/runtime.rs`, and example `crates/simple-agents-workflow/examples/debug_inspection.rs`.
- [x] Complete docs onboarding path (quickstart + advanced patterns + troubleshooting).
  - Evidence: docs updates in `docs/QUICKSTART.md`, `docs/ARCHITECTURE.md`, `docs/CAPABILITY_MATRIX.md`, and new `docs/TROUBLESHOOTING.md`.

## Subagent task board

### SA-1 (Workflow IR + validation)

- [x] Create workflow IR module/crate with versioned schema and serde contracts.
  - Evidence: added `crates/simple-agents-workflow` with IR structs in `crates/simple-agents-workflow/src/ir.rs`.
- [x] Implement IR parser/validator with deterministic normalization.
  - Evidence: `validate_and_normalize` implemented in `crates/simple-agents-workflow/src/validation.rs`.
- [x] Add lint diagnostics for missing edges, unreachable nodes, invalid refs.
  - Evidence: `DiagnosticCode::{UnknownTarget,UnreachableNode,NoPathToEnd,...}` in `crates/simple-agents-workflow/src/validation.rs`.
- [x] Add unit + property tests for parser/validator robustness.
  - Evidence: `cargo test -p simple-agents-workflow` passed (6 unit/property tests + 1 doc test).
- [x] Add docs comments and usage examples.
  - Evidence: crate docs example added in `crates/simple-agents-workflow/src/lib.rs`.

### SA-2 (Workflow scheduler/runtime)

- [x] Implement runtime execution engine for minimal node set.
  - Evidence: `WorkflowRuntime` added in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Ensure cancellation-safe async execution and bounded concurrency.
  - Evidence: cancellation checks before/between attempts, bounded execution via `max_steps`, and node retry/timeout policies in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Integrate with existing `simple-agents-core` request path (no duplicate provider logic).
  - Evidence: `impl LlmExecutor for SimpleAgentsClient` uses `SimpleAgentsClient::complete`.
- [x] Add retry/timeouts at node policy layer with explicit ownership rules.
  - Evidence: `NodeExecutionPolicy` + runtime-owned retry/timeout wrappers (`execute_llm_with_policy`, `execute_tool_with_policy`) in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add integration tests for happy/failure/cancel paths.
  - Evidence: runtime tests for happy path, conditional path, missing tool handler, tool failure, step limit in `crates/simple-agents-workflow/src/runtime.rs`.

### SA-3 (State + trace/replay)

- [x] Implement scoped state model (workflow scope + local scope).
  - Evidence: runtime scope model and per-node scoped input/output tracking in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Add capability checks for read/write access boundaries.
  - Evidence: `ScopeCapability` + `ScopeAccessError` guards and enforcement tests in `crates/simple-agents-workflow/src/runtime.rs`.
- [x] Implement trace event schema and recorder.
  - Evidence: `crates/simple-agents-workflow/src/trace.rs` and `crates/simple-agents-workflow/src/recorder.rs`.
- [x] Implement replay executor from recorded traces.
  - Evidence: `replay_trace` and replay validations in `crates/simple-agents-workflow/src/replay.rs`.
- [x] Add golden trace fixtures for deterministic verification.
  - Evidence: fixtures in `crates/simple-agents-workflow/tests/fixtures/linear_trace.json` and `crates/simple-agents-workflow/tests/fixtures/invalid_missing_terminal_trace.json` with fixture tests in `crates/simple-agents-workflow/tests/trace_fixtures.rs`.

### SA-4 (Worker protocol + pools + health)

- [x] Define worker protocol interfaces (request/response/error semantics).
  - Evidence: `WorkerRequest`, `WorkerOperation`, `WorkerResponse`, `WorkerResult`, `WorkerProtocolError`, `WorkerErrorCode` in `crates/simple-agents-workflow/src/worker.rs`.
- [x] Implement baseline worker pool manager with health probes.
  - Evidence: in-process `WorkerPool` with per-worker run loop + probe loop in `crates/simple-agents-workflow/src/worker.rs`.
- [x] Add backpressure and queue limits.
  - Evidence: bounded `mpsc::channel(queue_capacity)` and `try_send` rejection path (`WorkerPoolError::QueueFull`) in `crates/simple-agents-workflow/src/worker.rs`.
- [x] Add circuit-breaker integration points without duplicating router internals.
  - Evidence: optional `CircuitBreakerHooks` integration in `WorkerPool::submit` in `crates/simple-agents-workflow/src/worker.rs`.
- [x] Add chaos tests (worker restart/unavailable/slow worker).
  - Evidence: `worker::tests::{marks_worker_unavailable_after_failures_and_recovers_on_restart,returns_timeout_for_slow_worker}` and `cargo test -p simple-agents-workflow`.

### SA-5 (FFI + Go parity)

- [x] Implement `sa_stream_messages` in Rust FFI (callback-based stream API).
  - Evidence: `sa_stream_messages` + `SAStreamCallback` + `FfiStreamEvent` in `crates/simple-agents-ffi/src/lib.rs` and declaration in `crates/simple-agents-ffi/include/simple_agents.h`.
- [x] Implement Go `StreamMessages(ctx, ...)` channel API with cancellation and no leaks.
  - Evidence: `StreamMessages` channel API + callback bridge + context cancellation path in `bindings/go/simpleagents.go`.
- [x] Add Go streaming tests (unit + env-gated live).
  - Evidence: `TestStreamMessagesUninitializedClient` in `bindings/go/simpleagents_test.go` and env-gated `TestLiveStreamMessages` in `bindings/go/simpleagents_live_test.go`.
- [x] Refactor Go API shape (`CompletePrompt`, `CompleteMessages`, `StreamMessages`).
  - Evidence: added `CompletePrompt` while preserving compatibility helpers in `bindings/go/simpleagents.go`.
- [x] Refactor FFI payload mapping to shared typed DTO helpers.
  - Evidence: typed streaming DTOs/events (`FfiStreamEvent`) and shared completion mapping helpers retained in `crates/simple-agents-ffi/src/lib.rs`.

### SA-6 (Node parity)

- [x] Upgrade Node streaming typing surface (partials/events/errors).
  - Evidence: added typed `StreamEvent` surface (`delta`/`error`/`done`) via `Client.stream_events` in `crates/simple-agents-napi/src/lib.rs`.
- [x] Add typed tool-call return models with stable TS contracts.
  - Evidence: added `ToolCallResult`/`ToolCallResultFunction` and wired `CompletionResult.tool_calls` in `crates/simple-agents-napi/src/lib.rs`.
- [x] Ensure `.d.ts` parity with runtime behavior and examples.
  - Evidence: regenerated declarations through `npm run build:debug` and verified symbol presence by fixture contract test in `crates/simple-agents-napi/test/contract.test.js`.
- [x] Add Node parity contract tests against shared fixtures.
  - Evidence: shared fixture `parity-fixtures/binding_contract.json` consumed by Node test `declaration and runtime exports follow shared contract fixture` in `crates/simple-agents-napi/test/contract.test.js`.
- [x] Update Node docs with canonical env contract.
  - Evidence: updated `crates/simple-agents-napi/README.md` with canonical env contract and typed `streamEvents` example.

### SA-7 (Python parity)

- [x] Validate Python API parity for workflow-facing and streaming behaviors.
  - Evidence: parity-focused checks added in `crates/simple-agents-py/tests/test_error_mapping_consistency.py` and streaming assertions updated in `crates/simple-agents-py/tests/test_streaming.py`.
- [x] Add Python contract tests consuming shared fixtures.
  - Evidence: `crates/simple-agents-py/tests/test_contract_fixtures.py` reads `parity-fixtures/binding_contract.json`.
- [x] Ensure structured streaming semantics align with Rust reference.
  - Evidence: finish reason mapping normalized to Rust parity values (`stop|length|content_filter|tool_calls`) in `crates/simple-agents-py/src/lib.rs`.
- [x] Add error mapping consistency tests.
  - Evidence: `crates/simple-agents-py/tests/test_error_mapping_consistency.py` validates stable `RuntimeError` mapping/messages.
- [x] Update Python usage docs where parity behavior changes.
  - Evidence: `crates/simple-agents-py/README.md` now documents canonical env contract and streaming finish-reason semantics.

### SA-8 (CI + fixtures + DX)

- [x] Add cross-language capability matrix and required minimum gates in CI.
  - Evidence: `docs/CAPABILITY_MATRIX.md` and CI gate job `capability-contract-gates` in `.github/workflows/bindings-ci.yml`.
- [x] Add shared fixture repository for request/response/healing/streaming/tool-call.
  - Evidence: expanded `parity-fixtures/binding_contract.json` with `shared_cases` sections for request/response/healing/streaming/tool_call.
- [x] Add contract runner used by Rust/Python/Node/Go pipelines.
  - Evidence: added `scripts/run-binding-contracts.sh` and `make test-binding-contracts` target.
- [x] Add docs updates in `docs/` for architecture, quickstart, and troubleshooting.
  - Evidence: updated `docs/ARCHITECTURE.md`, `docs/QUICKSTART.md`, `docs/DOCS_MAP.md`, `docs/index.md`, and added `docs/TROUBLESHOOTING.md`.
- [x] Add contribution checklist enforcing skill usage and task checkoff discipline.
  - Evidence: added `CONTRIBUTING.md` and linked guidance from `docs/DEVELOPMENT.md`.

## Existing parity backlog (carried forward)

### Done

- [x] Node TypeScript declaration correctness fixed.
- [x] Go make/build/test baseline stabilized.
- [x] Go message-based completion + structured/healing output baseline added.
- [x] Binding CI workflow introduced.
- [x] Shared env template introduced.

### In progress

- [x] API parity with Python (missing streaming and advanced parity features).
  - Evidence: SA-7 parity tasks completed with fixture + error/streaming coverage and docs updates.
- [x] Go validation coverage (streaming tests + schema-edge golden cases pending).
  - Evidence: option validation + schema-edge golden fixtures in `bindings/go/testdata/schema_option_cases.json` and tests in `bindings/go/simpleagents_test.go`; live streaming tests isolated in `bindings/go/simpleagents_live_test.go`.
- [x] Credentials/fixtures parity (deterministic no-network fixtures established).
  - Evidence: fixture-backed no-network parity checks now run in FFI/Go/Node/Python via `parity-fixtures/binding_contract.json`.

### Pending

- [x] Implement streaming in C FFI and Go bindings.
  - Evidence: `sa_stream_messages` in `crates/simple-agents-ffi/src/lib.rs` and `StreamMessages` in `bindings/go/simpleagents.go`; verified with `cargo test -p simple-agents-ffi` and `go test ./...` in `bindings/go`.
- [x] Cross-language capability matrix and CI gating.
  - Evidence: capability matrix docs and CI contract gate in `.github/workflows/bindings-ci.yml`.
- [x] Contract fixtures for parity tests.
  - Evidence: shared fixture expanded and consumed by tests across `crates/simple-agents-ffi/tests/ffi_contract.rs`, `bindings/go/contract_fixture_test.go`, `crates/simple-agents-napi/test/contract.test.js`, and `crates/simple-agents-py/tests/test_contract_fixtures.py`.
- [x] Node parity improvements.
  - Evidence: SA-6 Node streaming/type/docs/tests updates in `crates/simple-agents-napi/` and shared fixture checks.
- [x] Refactor test layering (unit/contract/live).
  - Evidence: layered runners/scripts and suites added (`scripts/run-binding-tests-layered.sh`, `Makefile` target `test-binding-layers`, Node `test:unit|test:contract|test:live`, Go unit/contract/live split, Python layered commands in README).

## Ordered execution sequence

- [x] 1. SA-1 completes minimal IR + validator.
  - Evidence: `crates/simple-agents-workflow/src/ir.rs`, `crates/simple-agents-workflow/src/validation.rs`, `cargo test -p simple-agents-workflow`.
- [x] 2. SA-2 completes minimal runtime path wired to core.
  - Evidence: runtime + retry/timeout policies + cancellation tests in `crates/simple-agents-workflow/src/runtime.rs`; `cargo test -p simple-agents-workflow`.
- [x] 3. SA-3 lands trace/replay and deterministic fixtures.
  - Evidence: trace/recorder/replay + runtime integration + golden fixtures in `crates/simple-agents-workflow/src/trace.rs`, `crates/simple-agents-workflow/src/recorder.rs`, `crates/simple-agents-workflow/src/replay.rs`, `crates/simple-agents-workflow/tests/trace_fixtures.rs`.
- [x] 4. SA-5 lands FFI + Go streaming parity.
  - Evidence: streaming callback contract in `crates/simple-agents-ffi/include/simple_agents.h`, Rust implementation in `crates/simple-agents-ffi/src/lib.rs`, and Go channel API/tests in `bindings/go/simpleagents.go` and `bindings/go/simpleagents_test.go`.
- [x] 5. SA-6 and SA-7 converge Node/Python parity on shared fixtures.
  - Evidence: shared fixture `parity-fixtures/binding_contract.json` consumed by Node (`crates/simple-agents-napi/test/contract.test.js`) and Python (`crates/simple-agents-py/tests/test_contract_fixtures.py`) with updated parity APIs.
- [x] 6. SA-4 lands worker pool/health model behind stable interfaces.
  - Evidence: new worker protocol/pool module exported from `crates/simple-agents-workflow/src/lib.rs` and validated by `cargo test -p simple-agents-workflow`.
- [x] 7. SA-8 enforces CI capability gates and docs completion.
  - Evidence: CI gate + contract runner landed (`.github/workflows/bindings-ci.yml`, `scripts/run-binding-contracts.sh`) with docs updates (`docs/CAPABILITY_MATRIX.md`, `docs/TROUBLESHOOTING.md`).
