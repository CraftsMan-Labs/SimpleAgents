# API Reference (Surface Map)

This document provides a concise map of the API surface. For exhaustive API docs, use docs.rs for each crate.

## Crate Index

- `simple-agent-type`: core request/response contracts and traits.
- `simple-agents-core`: unified client orchestration.
- `simple-agents-providers`: provider implementations and utilities.
- `simple-agents-healing`: JSON parsing and schema coercion.
- `simple-agents-workflow`: YAML workflow engine.
- `simple-agents-napi`: Node.js bindings.
- `simple-agents-py`: Python bindings.

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

## `simple-agents-workflow`

YAML workflow engine:
- `WorkflowClient` wrapping `SimpleAgentsClient` with `run`, `stream`, `resume` methods.
- `yaml_runner::workflow_execution::{run, stream}` — low-level async execution functions.
- `YamlWorkflowExecutionRequest`, `YamlWorkflowSource`, `YamlWorkflowExecutorBinding`.
- `YamlWorkflowRunOptions`, `YamlWorkflowExecutionFlags`.
- `WorkflowRunOutput`, `WorkflowCheckpoint`.

## `simple-agents-healing`

Healing APIs:
- `JsonishParser` + `ParserConfig`.
- `CoercionEngine` + `CoercionConfig`.
- `Schema`, `ObjectSchema`, `Field`.
- `StreamingParser`.

## Bindings

### `simple-agents-napi`
Node.js `Client` with TypeScript types:
- `client.complete(messages, opts?)`.
- `client.stream(workflowPath, messages, opts?)`, `client.run(workflowPath, messages, opts?)`, `client.resume(checkpoint, messages, opts?)`.
- `MessageInput`, `RunOptions` types.

### `simple-agents-py`
Python `Client` via PyO3:
- `client.complete(messages, *, model=None, ...)`.
- `client.run(workflow_path, messages, *, tools=None, options=None)`.
- `client.stream(workflow_path, messages, *, on_event=None, tools=None, options=None)`.
- `client.resume(checkpoint, messages, *, on_event=None)`.
- `Message`, `Role`, `ContentPart` typed helpers.

### WASM (`simple-agents-wasm`)
- `Client.runWorkflowYamlString(yaml, input)` / `Client.runYamlString(yaml, input)`.

## Cargo Features

- `simple-agents-healing`: `regex-support`.
- `simple-agents-providers`: `prometheus`.
