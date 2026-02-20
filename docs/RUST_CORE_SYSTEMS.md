# Rust Core Systems Guide

This document explains how the Rust crates in `crates/` fit together, what each crate owns, and where to extend behavior safely.

## Architecture Map

`simple-agent-type` is the contract layer used by all other Rust crates.

`simple-agents-core` composes:
- providers (`simple-agents-providers`)
- routing (`simple-agents-router`)
- caching (`simple-agents-cache`)
- healing (`simple-agents-healing`)

Language/runtime surfaces sit on top:
- CLI: `simple-agents-cli`
- C FFI: `simple-agents-ffi`
- Node: `simple-agents-napi`
- Python: `simple-agents-py`

Macro support:
- `simple-agents-macros`

## Crate Responsibilities

### `simple-agent-type`
- Canonical request/response/tool models.
- Traits for provider/cache/routing contracts.
- Validation and security primitives (`ApiKey`, request validation).
- Shared error model and healing/coercion metadata.

### `simple-agents-core`
- Primary orchestration client and builder (`SimpleAgentsClient`).
- Routing/cache/healing/middleware composition.
- Completion modes: `Standard`, `HealedJson`, `CoercedSchema`.
- Completion outcomes: response, stream, healed JSON, schema-coerced JSON.

### `simple-agents-providers`
- Concrete provider adapters (OpenAI, Anthropic, OpenRouter).
- Streaming transforms (SSE to unified stream chunks).
- Structured output handling and schema conversion.
- Healing integration, rate limiting, retry, metrics hooks.

### `simple-agents-router`
- Multi-provider strategy implementations:
  - round-robin
  - latency-based
  - cost-based
  - fallback
- Resilience helpers: circuit breaker, retry policy, health tracker.

### `simple-agents-healing`
- JSON-ish parsing for malformed model output.
- Schema coercion with confidence and coercion flags.
- Streaming parser and partial extraction helpers.
- Optional regex-powered unquoted-key fixing (`regex-support` Cargo feature).

### `simple-agents-cache`
- Cache implementations for core integration:
  - in-memory (TTL + eviction)
  - noop

### Interface crates
- `simple-agents-cli`: command-line UX (complete/chat/benchmark/test-provider + workflow trace/replay/inspect/mermaid).
- `simple-agents-ffi`: stable C ABI surface.
- `simple-agents-napi`: Node API.
- `simple-agents-py`: Python API + schema/healing helpers.
- `simple-agents-macros`: derive macro for partial streaming types.

## Extension Points For Developers

### Add a new provider
1. Implement `Provider` from `simple-agent-type`.
2. Add request/response transforms and streaming mapping.
3. Expose constructor + optional `from_env` helper.
4. Register provider via `SimpleAgentsClientBuilder::with_provider(...)`.

### Add a new routing strategy
1. Implement strategy in `simple-agents-router`.
2. Add mode wiring in `simple-agents-core/src/routing.rs`.
3. Expose config shape in bindings/CLI if needed.

### Add caching backend
1. Implement `Cache` trait.
2. Plug into `SimpleAgentsClientBuilder::with_cache(...)`.
3. Validate serialization and TTL semantics.

### Add healing behavior
1. Extend parser/coercion/schema/streaming modules in `simple-agents-healing`.
2. Track transformation flags + confidence impacts.
3. Surface behavior in core completion modes and bindings when relevant.

## Cargo Features That Matter

- `simple-agents-healing`
  - `regex-support`: enables regex-based unquoted-key repair path.
- `simple-agents-providers`
  - `prometheus`: enables metrics exporter module support.

Most other functionality is runtime-configurable instead of compile-time gated.

## Operational Notes

- Provider constructors commonly support environment-driven setup (`from_env` patterns).
- Streaming support is provider-specific but normalized into common chunk types.
- Healing and schema coercion are available both in Rust core flows and in bindings (with binding-specific limits for streaming schema modes).

## Reference

- Root feature inventory: [features.md (repo)](https://github.com/rishub/SimpleAgents/blob/main/features.md)
