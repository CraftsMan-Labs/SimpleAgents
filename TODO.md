# End-to-End Tracing (Jaeger + PostHog) Plan

Date: 2026-02-20
Scope: Workflow runtime + bindings + FFI context propagation inside this repo
Decisions:
- Structured trace context object with optional raw fields
- Trace ID returned in both top-level field and `metadata.telemetry.trace_id`
- Default payload mode: full payload (toggle-ready for redaction)
- Retention: 30 days (config-driven)
- Multi-tenant attributes required
- Mandatory handler span for each custom handler invocation

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Master tasks

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| T1 | Define telemetry config + trace context contract | Stable cross-language API contract is required before implementation | Typed telemetry/trace context model with structured fields and optional raw `traceparent`/`tracestate`/`baggage` | completed |
| T2 | Implement workflow tracer integration in runtime | Core span creation must happen in source-of-truth workflow crate | `workflow.run`, `workflow.node.execute`, and mandatory `handler.invoke` spans emitted with tenant + timing attrs | completed |
| T3 | Add payload policy abstraction (full/redacted) | Full payload now and redaction later must share one code path | Default full payload behavior with configurable redaction toggle without API breakage | completed |
| T4 | Return correlation identifiers in workflow outputs | External API layer needs direct trace correlation without internal API server | Output includes top-level `trace_id` and `metadata.telemetry.trace_id` | completed |
| T5 | Add context/options surfaces across bindings + FFI | External API integrations call bindings/FFI, so context must propagate there | Python/Node/Go/FFI can pass telemetry + trace context into workflow execution | completed |
| T6 | Add tests for propagation, payload mode, and span coverage | Prevent regressions in tracing and cross-language parity | Unit/integration tests validate parent-child propagation, mandatory handler spans, and output trace ID contracts | completed |
| T7 | Document architecture, data flow, and ops config | Team needs clear implementation + operation guidance | Docs include flow, required attrs, retention config, and Jaeger/PostHog correlation model | completed |
| T8 | Final verification and readiness report | Confirm all touched surfaces work together | Relevant tests/checks pass and readiness notes are documented | completed |

## Data workflow (within repo scope)

```mermaid
flowchart TD
  A[External API Layer] -->|structured trace context + optional raw fields| B[Binding/FFI Entry]
  B --> C[Workflow Runner]
  C --> D[workflow.run span]
  D --> E[workflow.node.execute spans]
  E --> F[handler.invoke spans (mandatory)]
  F --> G[Workflow Output]
  G --> H[top-level trace_id]
  G --> I[metadata.telemetry.trace_id]
```

## Correlation and tenancy contract

- Required span/event attributes: `trace_id`, `span_id`, `workspace_id`, `user_id`, `request_id`, `workflow_id`, `run_id`, `node_id`, `handler_lang`
- Payload mode defaults to full payload and is toggle-ready for redaction
- Retention policy target is 30 days and must be configuration-driven in docs and operator setup

## Execution notes

- Runtime contract and tracer plumbing are source-of-truth in `crates/simple-agents-workflow/**`.
- Bindings/FFI should remain backward compatible by adding context/options variants.
- No internal API server is required; external API only needs to pass context and read returned trace IDs.

## Verification completed

- `cargo test -p simple-agents-workflow --lib --tests`
- `cargo check -p simple-agents-ffi`
- `cargo check -p simple-agents-napi`
- `cargo check -p simple-agents-py`
- `make test-go-bindings`

## Streaming cleanup follow-up (2026-02-20)

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| S1 | Move stream sanitization to core runtime | Example-only filtering leaked model preamble/reasoning tokens to terminal callbacks | Core `node_stream_delta` events emit only JSON-object payload content | completed |
| S2 | Remove stale stream fallback messaging in example chat | Legacy messaging referenced disabled streaming paths that no longer apply | Example output matches current runtime behavior and stays concise | completed |
| S3 | Add regression tests for delta filtering | Prevent reintroduction of reasoning/preamble leak regressions | New tests cover prefix/suffix stripping and braces inside string handling | completed |
| S4 | Add token attribution fields to stream events | Consumers need per-token routing and terminal-step attribution for observability and UI | Token events include `step_id`, `token_kind`, and `is_terminal_node_token` in core runtime events | completed |
| S5 | Add YAML flag for streaming JSON as plain text | Chat clients need optional human-readable streaming without hardcoded key extraction | `llm_call.stream_json_as_text=true` emits non-thinking output as `key: value` lines from core runtime | completed |

## Chat-history cross-language examples (2026-02-21)

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| C1 | Add Node interactive chat-history runner | Users need parity with Python `run_with_chat_history.py` in JS environments | `examples/workflow_email/node/run_with_chat_history.js` supports multi-turn `messages` workflow input and trace JSONL logging | completed |
| C2 | Add Go interactive chat-history runner | Users need parity with Python `run_with_chat_history.py` in Go environments | `bindings/go/examples/workflow_chat_history/main.go` supports multi-turn `messages` workflow input and trace JSONL logging | completed |
| C3 | Update docs and run instructions | New examples should be discoverable from existing workflow docs | Updated `examples/workflow_email/*` and `docs/EXAMPLES.md` to include Node/Go chat-history commands | completed |
| C5 | Add Python make target for chat-history runs | Developers need one consistent command style across Python and Go for shared YAML testing | `make run-python-chat-history WORKFLOW_YAML=... PY_CHAT_FLAGS=...` runs Python chat-history workflow with shared flags | completed |
| C6 | Add Node/Bun make target for chat-history runs | Developers need the same command style across JS/TS runtimes when a YAML workflow is shared | `make run-node-chat-history WORKFLOW_YAML=... NODE_CHAT_FLAGS=... JS_RUNTIME=node|bun` runs chat-history workflow with Node or Bun | completed |
| C7 | Fix Node workflow stream callback payload wiring | Node/Bun stream callbacks received null first-arg payloads, dropping streamed events in example runner | NAPI workflow stream emits callback payloads consistently and runner handles callback error-first signature to display stream deltas | completed |

## Python-to-Bindings parity plan (2026-02-21)

Scope: close feature gaps where Python binding supports capabilities not yet exposed in Go and Node/TS.

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| P1 | Freeze parity matrix and acceptance criteria | Work needs one source-of-truth so parity is testable and reviewable | Matrix mapping Python APIs/features to Node/Go status with explicit P0/P1 priorities and acceptance checks | pending |
| P2 | Add workflow event APIs in FFI | Go binding depends on FFI; no FFI event API means no Go parity for workflow streaming/events | New FFI surfaces for workflow run with recorded events and callback stream events, with backward compatibility | completed |
| P3 | Add Node workflow event parity APIs | Node currently lacks Python-equivalent workflow stream/event surfaces | `runWorkflowYaml*` Node APIs support include-events and live callback streaming with typed event payloads | completed |
| P4 | Add Go workflow event parity APIs | Go currently lacks Python-equivalent workflow stream/event surfaces | Go `Client` adds include-events and channel/callback-style workflow event streaming APIs | completed |
| P5 | Align workflow output contracts across bindings | Go currently exposes a narrower workflow output shape than Rust/Python | Node type defs + Go structs include token totals, LLM node metrics, tps, and optional events consistently | completed |
| P6 | Upgrade chat-history examples to full parity | Example scripts should prove parity in real usage, not only API signatures | Node/Go chat-history runners support `--include-events`, `--stream`, `--show-thinking`, `--show-step-json` | completed |
| P7 | Expand parity fixtures and contract tests | Prevent regressions and enforce parity expectations in CI | Updated `parity-fixtures` + Node/Go/Python contract tests for workflow event methods and payload shape | completed |
| P8 | Documentation and verification gates | Users need clear usage docs and maintainers need reproducible quality gates | Updated binding/example docs and successful runs of `make test-node`, `make test-go-bindings`, `make test-binding-contracts`, `make test-binding-layers` | completed |

Verification commands executed for parity batch:

- `cargo check -p simple-agents-ffi`
- `cargo check -p simple-agents-napi`
- `make test-go-bindings`
- `make test-node`
- `make test-binding-contracts`
- `make test-binding-layers`

## Cross-binding deterministic scenario routing fix (2026-02-21)

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| D1 | Eliminate Go/Node first-turn routing drift for shared chat-history workflow | Same YAML + same prompt produced different terminal paths, which looks like binding/core inconsistency and breaks parity expectations | Workflow routing is stable across Go and Node runs for the same prompt, and stream display behavior is aligned for `--show-thinking` | completed |

Verification commands executed for D1:

- `printf 'Yo\n' | make run-go-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml GO_CHAT_FLAGS='--max-turns 1 --include-events --stream --show-thinking --show-step-json'`
- `printf 'Yo\n' | make run-node-chat-history JS_RUNTIME=bun WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml NODE_CHAT_FLAGS='--max-turns 1 --include-events --stream --show-thinking --show-step-json'`
- `cargo build -p simple-agents-ffi --release && CGO_CFLAGS='-I/home/rishub/Desktop/projects/rishub/SimpleAgents/crates/simple-agents-ffi/include' CGO_LDFLAGS='-L/home/rishub/Desktop/projects/rishub/SimpleAgents/target/release' LD_LIBRARY_PATH='/home/rishub/Desktop/projects/rishub/SimpleAgents/target/release:$LD_LIBRARY_PATH' GOCACHE='/home/rishub/Desktop/projects/rishub/SimpleAgents/.go-cache' go test ./...` (from `bindings/go`)
- `npm test` (from `crates/simple-agents-napi`)

## Cross-language stream token parity hardening (2026-02-21)

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| D2 | Align Python/Go/Node stream token metadata and fallback behavior | Users need identical token-level debugging semantics across bindings when `--stream --show-thinking` is enabled | Stream output includes token identifiers plus step/kind/terminal metadata in all runners; runners fall back to `node_stream_delta` when raw-thinking tokens are absent | completed |

Verification commands executed for D2:

- `cargo test -p simple-agents-workflow --lib --tests`
- `cargo build -p simple-agents-ffi --release && CGO_CFLAGS='-I/home/rishub/Desktop/projects/rishub/SimpleAgents/crates/simple-agents-ffi/include' CGO_LDFLAGS='-L/home/rishub/Desktop/projects/rishub/SimpleAgents/target/release' LD_LIBRARY_PATH='/home/rishub/Desktop/projects/rishub/SimpleAgents/target/release:$LD_LIBRARY_PATH' GOCACHE='/home/rishub/Desktop/projects/rishub/SimpleAgents/.go-cache' go test ./...` (from `bindings/go`)
- `npm test` (from `crates/simple-agents-napi`)
- `printf 'Hi\n' | make run-python-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml PY_CHAT_FLAGS='--max-turns 1 --stream --show-thinking --show-step-json --trace-dir workflow_email/traces'`
- `printf 'Hi\n' | make run-go-chat-history WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml GO_CHAT_FLAGS='--max-turns 1 --stream --show-thinking --show-step-json'`
- `printf 'Hi\n' | make run-node-chat-history JS_RUNTIME=bun WORKFLOW_YAML=examples/workflow_email/email-chat-draft-or-clarify.yaml NODE_CHAT_FLAGS='--max-turns 1 --stream --show-thinking --show-step-json'`

## Live streaming regression guards (2026-02-21)

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| D3 | Add live binding tests for explicit workflow stream event types | Streaming regressions should be caught against real provider behavior before release | Go and Node live suites assert explicit stream event contract (`node_stream_thinking_delta`, `node_stream_output_delta`, no legacy `node_stream_raw_delta`) | completed |

Verification commands executed for D3:

- `cargo test -p simple-agents-workflow --lib --tests`
- `cargo build -p simple-agents-ffi --release && CGO_CFLAGS='-I/home/rishub/Desktop/projects/rishub/SimpleAgents/crates/simple-agents-ffi/include' CGO_LDFLAGS='-L/home/rishub/Desktop/projects/rishub/SimpleAgents/target/release' LD_LIBRARY_PATH='/home/rishub/Desktop/projects/rishub/SimpleAgents/target/release:$LD_LIBRARY_PATH' GOCACHE='/home/rishub/Desktop/projects/rishub/SimpleAgents/.go-cache' go test ./...` (from `bindings/go`)
- `npm --prefix crates/simple-agents-napi run test:live` (env-gated)

## YAML native tool-calling rollout plan (2026-02-22)

Scope: Add out-of-the-box tool calling for YAML `llm_call` nodes with strict per-node tool format, rich tracing, schema validation, and opt-in global storage while preserving backward compatibility.

Decisions locked:
- `tools_format` is strict per node (`openai` or `simplified`), default `openai`
- Tool result `output_schema` mismatches hard-fail the node
- Tool trace to globals is opt-in via YAML key only (no default global key)
- Anthropic remains disabled for tool-calling in this batch; provider update follows separately
- Tool trace mode is toggleable (`full`, `redacted`, `off`), default `full`

| ID | Task | What is being done | How it will be done | Draft code/context | Expected outcome | Status |
|---|---|---|---|---|---|---|
| YT1 | Extend YAML schema for tool-calling | Add additive `llm_call` fields: `tools_format`, `tools`, `tool_choice`, `max_tool_roundtrips`, `tool_calls_global_key` and telemetry toggle `tool_trace_mode` | Update `YamlLlmCall`, run options/config structs, serde defaults, and preserve existing behavior when tools are absent | Primary file: `crates/simple-agents-workflow/src/yaml_runner.rs`; tool structs reuse `simple-agent-type` models where possible | Existing YAML workflows remain valid; new fields parse with safe defaults | completed |
| YT2 | Add strict validation rules | Enforce tool format matching, tool name uniqueness, schema presence/shape checks, and bounded roundtrips at validation time | Extend `verify_yaml_workflow(...)` diagnostics with precise node/tool error codes and messages | Validation surface: `yaml_runner.rs` diagnostics table and parse checks | Validation fails early for malformed tool specs and passes unchanged workflows | completed |
| YT3 | Canonicalize tool declarations + request wiring | Normalize configured tool declarations into internal tool config for execution requests | Add normalization helpers for openai/simplified input and map into execution request fields | Wire through `YamlLlmExecutionRequest` and YAML->IR payload mapping so direct and IR paths align | One internal execution path regardless of authoring style | completed |
| YT4 | Implement runtime tool loop | Execute tool roundtrips in `llm_call` execution with default one roundtrip and configurable cap | In `BorrowedClientExecutor.complete_structured`, send tools to model, execute returned tool calls via custom worker handler name, append tool messages, continue until final answer or cap | Keep anthropic behavior unchanged by relying on provider errors for unsupported tool calling | Native YAML tool calling works for compatible providers; legacy behavior unchanged when no tools configured | completed |
| YT5 | Add tool I/O tracing + optional globals capture | Trace tool request/response payloads and status; optionally persist traces to globals via YAML key | Emit `node_tool_call_requested/completed/failed/roundtrip_completed` events using `tool_trace_mode`; persist per-node `tool_calls` and optional global key | Event schema: `YamlWorkflowEvent.metadata`; global write path via existing `set_globals/update_globals` context flow and explicit key setter | High-fidelity observability with configurable verbosity and optional global persistence | completed |
| YT6 | Enforce tool output schema at runtime | Validate tool outputs against optional per-tool `output_schema` before passing back to model and before global capture | Compile/validate JSON schema and hard-fail node on mismatch with actionable error payload/event | Add lightweight schema validation utility in workflow crate; test both pass/fail paths | Tool outputs are contract-safe and predictable for downstream prompts/globals | completed |
| YT7 | Add/expand tests (no stubs) | Add regression and feature tests for parser, validator, runtime loop, tracing, globals, IR parity, and compatibility | Extend existing `yaml_runner.rs` tests with real assertions and deterministic fixtures; run Rust test suite targets | Tests include openai+simplified modes, mismatch diagnostics, schema failure hard-fail, trace mode toggles, global key behavior | New feature is fully covered and existing behavior remains stable | completed |
| YT8 | Update docs and examples | Document new YAML contract, trace toggles, and non-breaking rollout details; add example snippets | Update `docs/YAML_WORKFLOW_SYSTEM.md` and workflow examples under `examples/workflow_email/` with `output_schema` and tool-calling snippets | Keep Anthropic caveat explicit in docs for this batch | Users can adopt tool-calling safely with clear migration/usage guidance | completed |
| YT9 | Verification gates and readiness notes | Run relevant checks and record outcomes with exact commands run | Execute focused and full test commands; capture any follow-up fixes before done | Primary checks: `cargo test -p simple-agents-workflow --lib --tests` plus formatting/lint/build checks as needed | Feature batch is releasable without regressions | completed |

Verification commands executed for YT batch:

- `cargo test -p simple-agents-workflow --lib --tests`
- `cargo fmt`

## Programmatic Trace Context SDK parity plan (2026-02-24)

Scope: expose typed, portable workflow trace context options across Python/Node/Go while preserving backward compatibility and validating parity with tests.

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| X1 | Freeze cross-language trace context contract | All SDKs need one stable and explicit propagation shape | Canonical schema for `workflow_options.trace.context` and `workflow_options.trace.tenant` with no breaking changes | completed |
| X2 | Add Node workflow email options parity | Node email helper currently lacks options despite generic workflow options support | `runEmailWorkflowYaml*` methods accept `workflowOptions` and propagate telemetry/trace fields | completed |
| X3 | Add Go typed run-options API | Map-based options are error-prone and not discoverable in editors | Typed `WorkflowRunOptions` helpers plus convenience methods preserving map API | completed |
| X4 | Align Python typing stubs with runtime signatures | Runtime supports workflow options on email/general methods but stubs lag | `simple_agents_py.pyi` exposes `workflow_options` on `run_email_workflow_yaml` and non-stream run methods | completed |
| X5 | Add binding tests for trace option propagation | Prevent regressions and verify transport parity by contract | Node/Go/Python tests assert options serialization, forwarding, and callback behavior | completed |
| X6 | Update docs and verify gates | Users and maintainers need clear usage + reproducible checks | Docs include copy-paste option examples and all relevant tests pass | pending |

## Cross-language all-YAML example runners (2026-02-24)

Scope: add Python/Node/Go example runners that execute every `examples/workflow_email/*.yaml` file with shared workflow input using SDK APIs.

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| E1 | Add Python all-YAML runner example | Python users need one command to validate all workflow YAML examples | `workflow_email/python/run_all_yaml_workflows.py` runs each YAML and prints pass/fail summary JSON | completed |
| E2 | Add Node all-YAML runner example | Node users need parity with Python for full workflow sample sweeps | `workflow_email/node/run_all_yaml_workflows.js` runs each YAML and prints pass/fail summary JSON | completed |
| E3 | Add Go all-YAML runner example | Go users need parity with Python/Node for sample sweeps | `bindings/go/examples/workflow_email_all/main.go` runs each YAML and prints pass/fail summary JSON | completed |
| E4 | Document run commands in workflow example docs | New runners must be discoverable in existing language docs | Updated top-level and language README files with exact commands | completed |
