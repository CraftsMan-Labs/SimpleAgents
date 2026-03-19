# YAML Workflow System Guide

This guide shows how to design, run, and troubleshoot YAML workflows in SimpleAgents.
By the end, you will understand the workflow file model, supported node behavior, schema contracts, runtime telemetry, and practical production checks.

## Prerequisites

- Familiarity with [Quick Start](/QUICKSTART) and [Usage Guide](/USAGE)
- A runnable workspace with `cargo` and optional `uv` for Python examples
- Basic JSON schema knowledge for `llm_call` output contracts

## Quick Path

1. Create a minimal YAML workflow with one `llm_call` node.
2. Add explicit `config.output_schema` for structured output stability.
3. Run workflow via Rust API or examples runner.
4. Render the workflow graph to Mermaid for fast wiring validation.
5. Inspect trace/timing fields and iterate.

Minimal workflow skeleton:

```yaml
id: my-workflow
version: 1.0.0
entry_node: first_node

nodes:
  - id: first_node
    node_type:
      llm_call:
        model: gpt-4.1
    config:
      output_schema:
        type: object
        properties:
          status: { type: string }
        required: [status]
        additionalProperties: false
      prompt: |
        Return {"status":"ok"}

edges:
  - from: first_node
    to: first_node
```

Required top-level fields are `id`, `entry_node`, and non-empty `nodes`.

## Mental Model

| Layer | What it does |
|---|---|
| YAML authoring | Defines graph, prompts, routing, workers, and state updates |
| Runtime model | Converts YAML to canonical IR when compatible, otherwise runs YAML-specific path |
| Execution + telemetry | Runs node-by-node and emits trace, timings, and event diagnostics |

Keep product logic in YAML; treat runtime output as verification and observability material.

## Supported Node Types

- `llm_call`: structured LLM generation with optional tools and streaming flags
- `switch`: condition-driven routing with deterministic default
- `custom_worker`: deterministic external logic handler

### `llm_call` essentials

```yaml
node_type:
  llm_call:
    model: gpt-4.1
    stream: false
    heal: true
    messages_path: input.messages
    append_prompt_as_user: true
config:
  output_schema: { ...json schema... }
  prompt: |
    ...
```

Behavior notes:

- `model` is required.
- `config.output_schema` should be explicit for every `llm_call`.
- `config.schema` is accepted as an alias but prefer `output_schema`.
- If schema is omitted, runtime falls back to permissive object behavior.

Tool calling (per-node strict format):

- `tools_format`: `openai` or `simplified`
- `tools`, `tool_choice`, `max_tool_roundtrips`, `tool_calls_global_key`
- Mixed tool declaration formats in one node fail validation.
- Tool output schema mismatch hard-fails node execution.

### `switch` essentials

```yaml
node_type:
  switch:
    branches:
      - condition: '$.nodes.classifier.output.category == "x"'
        target: branch_x
    default: fallback_node
```

Always define deterministic `default` behavior.

### `custom_worker` essentials

```yaml
node_type:
  custom_worker:
    handler: GetRagData
config:
  payload:
    topic: termination
```

Worker context includes trace correlation fields under `context.trace` so external code can propagate telemetry.

## Prompt Context and Run Memory

Templates can resolve from:

- `input.*`
- `nodes.<node_id>.output.*`
- `globals.*`

Memory updates are available via:

- `config.set_globals`
- `config.update_globals` with `set|append|increment|merge`

Use globals for run-level state, not for long-term secret storage.

## Chat-History Workflows

Pass chat arrays in `input.messages`:

```json
{
  "email_text": "optional scalar input",
  "messages": [
    {"role":"system","content":"..."},
    {"role":"user","content":"..."}
  ]
}
```

Supported `role` values: `system`, `user`, `assistant`, `tool` (requires `tool_call_id`).

## Running Workflows

Rust API:

```rust
use serde_json::json;
use simple_agents_workflow::run_workflow_yaml_file_with_client;

let output = run_workflow_yaml_file_with_client(
    std::path::Path::new("examples/workflow_email/email-unified-chat-intake-classification.yaml"),
    &json!({ "email_text": "Need replacement", "messages": [] }),
    &client,
).await?;
```

Python examples:

```bash
uv run --directory examples python workflow_email/run_with_chat_history.py
uv run --directory examples python workflow_email/run_with_unified_system.py
```

Graph visualization:

```bash
cargo run -p simple-agents-cli -- workflow mermaid examples/workflow_email/python-intern-fun-interview-system.yaml
```

## Telemetry and Diagnostics

Workflow outputs include:

- `trace` node order
- `step_timings` per node
- `total_elapsed_ms`
- `trace_id`
- `metadata.telemetry.trace_id`
- `metadata.telemetry.sampled`

Runtime options can include telemetry sampling, payload mode, tool trace mode, retention, and tenant context. Use `conversation_id` to group multi-turn traces reliably.
`telemetry.sample_rate` must be between `0.0` and `1.0` and is applied deterministically per trace id.

Exporter configuration is environment-driven and shared across tracing backends:

- `SIMPLE_AGENTS_TRACING_ENABLED`
- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL` (`grpc` or `http/protobuf`)
- `OTEL_EXPORTER_OTLP_HEADERS`
- `OTEL_SERVICE_NAME`

## Design Patterns That Work Well

1. Classifier node -> `switch` router -> action node
2. LLM action plus deterministic guardrail worker
3. One-question-at-a-time interview/chat progression
4. Explicit output schema for every `llm_call`
5. Explicit closed terminal states for completed sessions

## Troubleshooting

### Stale Python bindings in examples

```bash
uv sync --directory examples --reinstall-package simple-agents-py
```

### Graph validation issues

Render Mermaid output first to confirm parse and wiring:

```bash
cargo run -p simple-agents-cli -- workflow mermaid examples/workflow_email/email-unified-chat-intake-classification.yaml
```

### Non-deterministic routing behavior

Verify every `switch` has a deterministic `default` and branch paths point to existing node ids.

### Schema drift in LLM output

Define `config.output_schema` on every `llm_call` node and keep it strict (`additionalProperties: false` where appropriate).

## Production Checklist

- Every `llm_call` has explicit `config.output_schema`.
- Every `switch` defines deterministic default routing.
- Sensitive logic is represented in deterministic worker nodes where needed.
- Trace/timing output is captured and retained for audit/debug use.
- Session-close states are explicitly modeled.

## Next Steps

- Use [Workflow Debugging UX](/WORKFLOW_DEBUGGING) for replay and retry inspection.
- Tune runtime characteristics in [Workflow Performance](/WORKFLOW_PERFORMANCE).
- Apply guardrails from [Workflow Security](/WORKFLOW_SECURITY).
- For YAML/code conversion, follow [Workflow DSL Migration Cookbook](/WORKFLOW_DSL_MIGRATION_COOKBOOK).
