# YAML Workflow System Guide

This guide explains how YAML workflows fit together after you have a first run working.
If you want the fastest setup path, start with [Workflow Quickstart](/WORKFLOW_QUICKSTART).

## Prerequisites

Use this guide when you want to move beyond "just make one workflow run" and learn how to:

- add branching
- add deterministic worker logic
- use globals and node outputs safely
- debug runtime behavior

Prerequisites:

- Familiarity with [Workflow Quickstart](/WORKFLOW_QUICKSTART)
- A runnable workspace with `cargo` and optional `uv` for Python examples
- Basic JSON schema knowledge for `llm_call` output contracts

## Quick Path

Keep your workflow development in this order:

1. Start with one `llm_call` node.
2. Add strict `config.output_schema`.
3. Validate graph shape with Mermaid output.
4. Add `switch` routing only when branching is needed.
5. Add `custom_worker` only for deterministic external logic.

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

```

Required top-level fields are `id`, `entry_node`, and non-empty `nodes`.
Add `edges` when your workflow has more than one execution step.

## Mental Model

| Layer | What it does |
|---|---|
| YAML authoring | Defines graph, prompts, routing, workers, and state updates |
| Runtime model | Converts YAML to canonical IR when compatible, otherwise runs YAML-specific path |
| Execution + telemetry | Runs node-by-node and emits trace, timings, and event diagnostics |

Keep product logic in YAML and use runtime output for verification and debugging.

The simplest pattern to reuse is:

1. classifier node
2. `switch` router
3. action node

## Supported Node Types

- `llm_call`: structured LLM generation with optional tools and streaming flags
- `switch`: condition-driven routing with deterministic default
- `custom_worker`: deterministic external logic handler

### `llm_call`

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

Use `llm_call` when the model should generate or classify something.

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

### `switch`

```yaml
node_type:
  switch:
    branches:
      - condition: '$.nodes.classifier.output.category == "x"'
        target: branch_x
    default: fallback_node
```

Use `switch` when routing should depend on a stable value from a previous node.

Always define deterministic `default` behavior.

### `custom_worker`

```yaml
node_type:
  custom_worker:
    handler: get_rag_data
    handler_file: handlers.py
config:
  payload:
    topic: termination
```

Use `custom_worker` when code must run deterministically outside the model.

- `handler`: exact function name to invoke (no name normalization).
- `handler_file` (optional): path to the handler module; defaults to `handlers.py` relative to the workflow YAML directory.
- `llm_call.provider` is not supported in YAML and is rejected.
- `custom_worker.language` is not supported in YAML and is rejected.

#### Inputs and outputs

- **`config.payload`**: arbitrary JSON object. Values are interpolated like other templates (`input.*`, `nodes.*`, `globals.*`) before the handler runs. Put every node-specific argument here (for example `topic`, `company_name`). The engine does not validate `payload` against a JSON Schema today (unlike `llm_call` + `config.output_schema`).
- **Execution context** passed to bindings: JSON object with at least `input` (workflow input), `nodes` (completed node outputs), and `globals`. When tracing is enabled, `trace` is added with correlation and tenant fields (see below).
- **Handler return value**: must be JSON-serializable. The runner stores it as this node’s structured output. Downstream templates use `nodes.<node_id>.output.<field>` when the handler returns an object (for example `nodes.rag_probation.output.topic`).

#### Binding support (where handlers actually run)

| Surface | Local file handlers | Notes |
|--------|---------------------|--------|
| **Python** (`simple-agents-py`) | Yes — default `handlers.py` next to the YAML | Handlers are called with keyword-only `context` and `payload`; see [BINDINGS_PYTHON.md](BINDINGS_PYTHON.md). |
| **Node** (`simple-agents-napi`) | No in-process executor yet | Runtime performs fail-fast validation when `custom_worker` nodes are present (includes node id + handler) instead of late node-time failure. See [BINDINGS_NODE.md](BINDINGS_NODE.md). |
| **WASM / browser** (`runWorkflowYamlString`) | Yes — register functions in `workflowOptions.functions` | JS signature is `(args, graphContext)`; see [BINDINGS_WASM.md](BINDINGS_WASM.md). |

Worker context includes trace correlation fields under `context.trace` so external code can propagate telemetry.

## A Good First Multi-Node Pattern

Use this when you want a workflow that decides whether to act or ask a follow-up question:

1. `detect_*` node classifies state
2. `switch` routes from that state
3. one branch asks a question
4. one branch performs the main action

Good example: `examples/workflow_email/email-chat-draft-or-clarify.yaml`

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

Pass chat arrays in `input.messages` (required for `messages_path: input.messages`). Optional extra keys on the same input object (for example legacy `email_text`) are fine if your prompts still reference `input.*`:

```json
{
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
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
    &json!({
        "messages": [
            {"role": "user", "content": "Need replacement for order 9921"}
        ]
    }),
    &client,
).await?;
```

Builder-style API (preferred for new code):

```rust
use serde_json::json;
use simple_agents_workflow::WorkflowRunner;

let output = WorkflowRunner::from_file(
    std::path::Path::new("examples/workflow_email/email-unified-chat-intake-classification.yaml"),
)
.with_client(&client)
.with_input(&json!({
    "messages": [
        {"role": "user", "content": "Need replacement for order 9921"}
    ]
}))
.run()
.await?;
```

Compatibility note:

- Existing `run_*` helper functions remain available as compatibility wrappers.
- New integrations should prefer `WorkflowRunner` to avoid combinatorial API growth.

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

You do not need telemetry to get started. Use it after the workflow already runs.

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
