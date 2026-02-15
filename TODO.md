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

- [ ] Every subagent must update this `TODO.md` directly in its PR.
- [ ] When a task is complete, change `[ ]` to `[x]` and add 1-line evidence (test command/output path).
- [ ] If blocked, mark task as `[~]` with blocker reason and owner.
- [ ] No task is "done" without tests (unit/contract/live as applicable).
- [ ] Follow `CODING_GUIDELINES.md` (KISS, DRY, OOD, no phantom code, reusable APIs).

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

- [ ] Add bounded parallel execution primitives.
- [ ] Add worker protocol surface (start with single-process default).
- [ ] Add worker pool lifecycle + health tracking.
- [ ] Define retry/timeout ownership boundaries (workflow layer vs router layer).
- [ ] Load/perf benchmarks for scheduler and state hot paths.

### Phase 3 - Cross-language parity and DX hardening

- [ ] Align FFI/Go/Node/Python workflow-facing behavior with Rust reference.
- [ ] Add golden contract fixtures shared by all bindings.
- [ ] Enforce capability matrix gates in CI.
- [ ] Ship debugging UX: node timeline, retry reasons, replay trace inspection.
- [ ] Complete docs onboarding path (quickstart + advanced patterns + troubleshooting).

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

- [ ] Define worker protocol interfaces (request/response/error semantics).
- [ ] Implement baseline worker pool manager with health probes.
- [ ] Add backpressure and queue limits.
- [ ] Add circuit-breaker integration points without duplicating router internals.
- [ ] Add chaos tests (worker restart/unavailable/slow worker).

### SA-5 (FFI + Go parity)

- [ ] Implement `sa_stream_messages` in Rust FFI (callback-based stream API).
- [ ] Implement Go `StreamMessages(ctx, ...)` channel API with cancellation and no leaks.
- [ ] Add Go streaming tests (unit + env-gated live).
- [ ] Refactor Go API shape (`CompletePrompt`, `CompleteMessages`, `StreamMessages`).
- [ ] Refactor FFI payload mapping to shared typed DTO helpers.

### SA-6 (Node parity)

- [ ] Upgrade Node streaming typing surface (partials/events/errors).
- [ ] Add typed tool-call return models with stable TS contracts.
- [ ] Ensure `.d.ts` parity with runtime behavior and examples.
- [ ] Add Node parity contract tests against shared fixtures.
- [ ] Update Node docs with canonical env contract.

### SA-7 (Python parity)

- [ ] Validate Python API parity for workflow-facing and streaming behaviors.
- [ ] Add Python contract tests consuming shared fixtures.
- [ ] Ensure structured streaming semantics align with Rust reference.
- [ ] Add error mapping consistency tests.
- [ ] Update Python usage docs where parity behavior changes.

### SA-8 (CI + fixtures + DX)

- [ ] Add cross-language capability matrix and required minimum gates in CI.
- [ ] Add shared fixture repository for request/response/healing/streaming/tool-call.
- [ ] Add contract runner used by Rust/Python/Node/Go pipelines.
- [ ] Add docs updates in `docs/` for architecture, quickstart, and troubleshooting.
- [ ] Add contribution checklist enforcing skill usage and task checkoff discipline.

## Existing parity backlog (carried forward)

### Done

- [x] Node TypeScript declaration correctness fixed.
- [x] Go make/build/test baseline stabilized.
- [x] Go message-based completion + structured/healing output baseline added.
- [x] Binding CI workflow introduced.
- [x] Shared env template introduced.

### In progress

- [~] API parity with Python (missing streaming and advanced parity features).
- [~] Go validation coverage (streaming tests + schema-edge golden cases pending).
- [~] Credentials/fixtures parity (deterministic no-network fixtures pending).

### Pending

- [ ] Implement streaming in C FFI and Go bindings.
- [ ] Cross-language capability matrix and CI gating.
- [ ] Contract fixtures for parity tests.
- [ ] Node parity improvements.
- [ ] Refactor test layering (unit/contract/live).

## Ordered execution sequence

- [x] 1. SA-1 completes minimal IR + validator.
  - Evidence: `crates/simple-agents-workflow/src/ir.rs`, `crates/simple-agents-workflow/src/validation.rs`, `cargo test -p simple-agents-workflow`.
- [x] 2. SA-2 completes minimal runtime path wired to core.
  - Evidence: runtime + retry/timeout policies + cancellation tests in `crates/simple-agents-workflow/src/runtime.rs`; `cargo test -p simple-agents-workflow`.
- [x] 3. SA-3 lands trace/replay and deterministic fixtures.
  - Evidence: trace/recorder/replay + runtime integration + golden fixtures in `crates/simple-agents-workflow/src/trace.rs`, `crates/simple-agents-workflow/src/recorder.rs`, `crates/simple-agents-workflow/src/replay.rs`, `crates/simple-agents-workflow/tests/trace_fixtures.rs`.
- [ ] 4. SA-5 lands FFI + Go streaming parity.
- [ ] 5. SA-6 and SA-7 converge Node/Python parity on shared fixtures.
- [ ] 6. SA-4 lands worker pool/health model behind stable interfaces.
- [ ] 7. SA-8 enforces CI capability gates and docs completion.
