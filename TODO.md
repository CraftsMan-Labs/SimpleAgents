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
