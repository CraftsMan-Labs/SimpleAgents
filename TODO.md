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
