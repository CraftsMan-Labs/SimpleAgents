# Rust Core Systems Feature Inventory

Date: 2026-02-11
Scope: All Rust crates under `crates/` in this workspace.

## 1) Cargo Feature Flags (compile-time)

Only two crates currently expose Cargo feature flags.

| Crate | Default features | Named features | Notes |
|---|---|---|---|
| `simple-agent-type` | n/a | none | No `[features]` section |
| `simple-agents-cache` | n/a | none | No `[features]` section |
| `simple-agents-cli` | n/a | none | No `[features]` section |
| `simple-agents-core` | n/a | none | No `[features]` section |
| `simple-agents-ffi` | n/a | none | No `[features]` section |
| `simple-agents-healing` | `[]` | `regex-support = ["regex"]` | Enables regex-based unquoted-key fixing in parser |
| `simple-agents-macros` | n/a | none | No `[features]` section |
| `simple-agents-napi` | n/a | none | No `[features]` section |
| `simple-agents-providers` | `[]` | `prometheus = ["dep:metrics-exporter-prometheus"]` | Enables Prometheus exporter support in metrics module |
| `simple-agents-py` | n/a | none | No `[features]` section |
| `simple-agents-router` | n/a | none | No `[features]` section |

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

### `simple-agents-router`
- Routing strategies: `RoundRobinRouter`, `LatencyRouter`, `CostRouter`, `FallbackRouter`.
- Resilience helpers: `CircuitBreaker`, `RetryPolicy`, `HealthTracker`.
- Retry helper (`execute_with_retry`) for router-level execution paths.

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
- Schema model (`Schema`, `ObjectSchema`, `Field`, `StreamAnnotation`).
- Streaming parse support (`StreamingParser`, `PartialExtractor`).
- Optional regex-powered unquoted-key repair (`regex-support`).

### `simple-agents-cache`
- `InMemoryCache` with TTL, expiry cleanup, and LRU-style eviction.
- `NoOpCache` for disabled caching/testing.
- Re-exports `Cache` trait from `simple-agent-type`.

### `simple-agents-cli`
- CLI subcommands: `complete`, `chat`, `benchmark`, `test-provider`.
- Config file support (TOML/YAML) for providers/defaults/routing.
- Output formats: plain, JSON, Markdown.

### `simple-agents-ffi`
- C-compatible API for client lifecycle and completions.
- Completion helpers: `sa_complete`, `sa_complete_messages_json`.
- Error handling helpers: `sa_last_error_message`, `sa_string_free`.
- Completion modes exposed as JSON options (`standard`, `healed_json`, `schema`).

### `simple-agents-napi`
- Node bindings with a `Client` class.
- `complete()` supports standard/healed-json/schema modes.
- `stream()` supports standard streaming callbacks.
- Schema parsing bridge for structured/coerced responses.

### `simple-agents-py`
- Python bindings via PyO3.
- Client builder, streaming iterators, and structured streaming events.
- Healing parser/coercion helpers and schema builder utilities.
- Routing/cache/healing configuration APIs exposed to Python.

### `simple-agents-macros`
- `#[derive(PartialType)]` proc macro.
- Generates partial structs + merge/from-partial helpers for streaming workflows.

## 3) Feature Summary

- Compile-time feature flags are intentionally minimal today.
- Most functionality is exposed as runtime capabilities via crate APIs and configuration.
- Current compile-time toggles focus on:
  - parser enhancement (`regex-support`)
  - metrics exporter support (`prometheus`)
