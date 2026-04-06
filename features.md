# Rust Core Systems Feature Inventory

Date: 2026-02-11
Scope: All Rust crates under `crates/` in this workspace.

## 1) Cargo Feature Flags (compile-time)

Only two crates currently expose Cargo feature flags.

| Crate | Default features | Named features | Notes |
|---|---|---|---|
| `simple-agent-type` | n/a | none | No `[features]` section |
| `simple-agents-core` | n/a | none | No `[features]` section |
| `simple-agents-healing` | `[]` | `regex-support = ["regex"]` | Enables regex-based unquoted-key fixing in parser |
| `simple-agents-napi` | n/a | none | No `[features]` section |
| `simple-agents-providers` | `[]` | `prometheus = ["dep:metrics-exporter-prometheus"]` | Enables Prometheus exporter support in metrics module |
| `simple-agents-py` | n/a | none | No `[features]` section |
| `simple-agents-workflow` | n/a | none | No `[features]` section |

## 2) Crate Runtime/System Features (API-level)

### `simple-agent-type`
- Core contracts and shared types (`Provider`, `Cache`, `RoutingStrategy`).
- Unified request/response model (`CompletionRequest`, `CompletionResponse`, streaming chunks).
- Tool calling data model (`ToolDefinition`, `ToolChoice`, `ToolCall`).
- Validation and security primitives (`ApiKey`, request validation, structured errors).
- Healing/coercion metadata model (`CoercionFlag`, `CoercionResult`).

### `simple-agents-core`
- Unified client orchestration (`SimpleAgentsClient`, `SimpleAgentsClientBuilder`).
- Completion outcomes: standard response, streaming response, healed JSON, schema-coerced JSON.
- Routing integration via `RoutingMode` (direct, round-robin, latency, cost, fallback).
- Optional cache integration (cache TTL, `Cache` trait objects).
- Healing settings (`HealingSettings`) and completion modes (`CompletionMode`).
- Middleware lifecycle hooks (before/after/error/cache-hit/after-stream).

### `simple-agents-workflow`
- YAML workflow engine: load, validate, execute multi-step LLM graphs.
- `WorkflowClient` wrapping `SimpleAgentsClient` with `run`, `stream`, `resume`.
- `workflow_execution::{run, stream}` low-level async entry points.
- Execution flags: `YamlWorkflowExecutionFlags` (healing, streaming, split-deltas).
- Options: `YamlWorkflowRunOptions` (telemetry, trace context, model override).
- Checkpoint/resume via `WorkflowCheckpoint`.
- Observability: nerdstats, telemetry, tracing.

### `simple-agents-providers`
- Provider implementations: OpenAI, Anthropic, OpenRouter.
- Request/response transforms into unified types.
- Streaming (SSE) support for providers that expose it.
- Structured streaming helpers and healing integration.
- Rate limiting and retry utilities.
- Metrics instrumentation with optional Prometheus exporter (`prometheus` feature).

### `simple-agents-healing`
- JSON-ish parser (`JsonishParser`) with configurable parsing rules.
- Schema coercion engine (`CoercionEngine`) with confidence + flags.
- Schema model (`Schema`, `ObjectSchema`, `Field`).
- Streaming parse support (`StreamingParser`).
- Optional regex-powered unquoted-key repair (`regex-support`).

### `simple-agents-napi`
- Node.js `Client` class via NAPI.
- `complete()` for direct LLM calls (standard/healed-json/schema modes).
- `run(workflowPath, messages, opts?)`, `stream(...)`, `resume(checkpoint, opts?)`.
- `MessageInput`, `RunOptions` TypeScript types.

### `simple-agents-py`
- Python `Client` via PyO3.
- `complete()`, `stream_complete()` for direct LLM calls.
- `run(workflow_path, messages, *, tools=None, options=None)`.
- `stream(workflow_path, messages, *, on_event=None, tools=None, options=None)`.
- `resume(checkpoint, *, options=None)`.
- Typed helpers: `Message`, `Role`, `ContentPart`.

## 3) Feature Summary

- Compile-time feature flags are intentionally minimal today.
- Most functionality is exposed as runtime capabilities via crate APIs and configuration.
- Current compile-time toggles focus on:
  - parser enhancement (`regex-support`)
  - metrics exporter support (`prometheus`)
