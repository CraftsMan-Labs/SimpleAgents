# API Reference (Surface Map)

This document provides a concise map of the API surface. For exhaustive API docs, use docs.rs for each crate.

## Crate Index

- `simple-agent-type`: core request/response contracts and traits.
- `simple-agents-core`: unified client orchestration.
- `simple-agents-providers`: provider implementations and utilities.
- `simple-agents-router`: routing strategies and resilience helpers.
- `simple-agents-healing`: JSON parsing and schema coercion.
- `simple-agents-cache`: cache implementations.
- `simple-agents-cli`: CLI entrypoints.
- `simple-agents-ffi`: C ABI surface.
- `simple-agents-napi`: Node bindings.
- `simple-agents-py`: Python bindings.
- `simple-agents-macros`: proc macros.

## `simple-agent-type`

Key types and traits:
- `CompletionRequest`, `CompletionResponse`, `CompletionChunk`.
- `Message`, `Role`, `FinishReason`.
- `Provider`, `ProviderRequest`, `ProviderResponse`.
- `Cache`, `CacheKey`.
- `SimpleAgentsError`, `ProviderError`, `ValidationError`.
- Tool calling types: `ToolDefinition`, `ToolChoice`, `ToolCall`.

## `simple-agents-core`

Primary client APIs:
- `SimpleAgentsClient`, `SimpleAgentsClientBuilder`.
- `CompletionOptions`, `CompletionMode`, `CompletionOutcome`.
- `RoutingMode`.
- `HealingSettings`.
- `Middleware` trait for lifecycle hooks.

## `simple-agents-providers`

Provider implementations:
- `openai::OpenAIProvider`.
- `anthropic::AnthropicProvider`.
- `openrouter::OpenRouterProvider`.

Utilities and subsystems:
- Retry helpers and rate limiting.
- Streaming and structured streaming helpers.
- Metrics instrumentation (optional `prometheus` feature).

## `simple-agents-router`

Routing strategies and helpers:
- `RoundRobinRouter`, `LatencyRouter`, `CostRouter`, `FallbackRouter`.
- `CircuitBreaker`, `RetryPolicy`, `HealthTracker`.

## `simple-agents-healing`

Healing APIs:
- `JsonishParser` + `ParserConfig`.
- `CoercionEngine` + `CoercionConfig`.
- `Schema`, `ObjectSchema`, `Field`.
- `StreamingParser`.

## `simple-agents-cache`

Cache implementations:
- `InMemoryCache`.
- `NoOpCache`.

## `simple-agents-cli`

CLI surface:
- `complete`, `chat`, `benchmark`, `test-provider`.
- `workflow trace`, `workflow replay`, `workflow inspect`.
- `workflow mermaid <workflow.yaml|workflow.json>` for graph visualization.
- Config-driven provider and routing setup.

## Bindings

- `simple-agents-ffi`: C ABI for completions.
- `simple-agents-napi`: Node `Client` bindings (standard/healed/schema).
- `simple-agents-py`: Python client builder, streaming iterators, schema helpers.

## `simple-agents-macros`

- `#[derive(PartialType)]` for partial structs used in streaming workflows.

## Cargo Features

- `simple-agents-healing`: `regex-support`.
- `simple-agents-providers`: `prometheus`.
