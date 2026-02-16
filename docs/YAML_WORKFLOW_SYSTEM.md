# YAML Workflow System Guide

This is the comprehensive guide for designing, running, debugging, and productionizing YAML workflows in SimpleAgents.

It covers:

- workflow file structure and node model
- chat-history workflows (`input.messages`)
- structured output contracts (`config.output_schema`)
- custom workers and handler execution
- observability (events, traces, timings)
- CLI visualization and replay tooling
- practical design patterns and anti-patterns

## 1) Mental Model

Think in three layers:

1. **Workflow authoring (YAML)**: graph + prompts + routing + worker calls.
2. **Canonical runtime model (IR)**: YAML is converted when compatible; fallback path is used for YAML-specific features.
3. **Execution + telemetry**: runtime executes node-by-node and emits trace/timing/event data.

The YAML authoring format is where your product logic should live.

## 2) File Skeleton

```yaml
id: my-workflow
version: 1.0.0

metadata:
  name: "My Workflow"
  description: "What this workflow does"
  tags: ["example"]

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
    to: next_node
```

Required top-level fields:

- `id`
- `entry_node`
- `nodes` (non-empty)

`version`/`metadata` are optional but strongly recommended.

## 3) Supported Node Types

- `llm_call`
  - model-driven structured generation
- `switch`
  - condition-based routing
- `custom_worker`
  - deterministic/non-LLM external logic (Python handlers in examples)

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

Field behavior:

- `model`: required
- `messages_path`: optional path to chat messages (e.g. `input.messages`)
- `append_prompt_as_user`: if true, appends resolved prompt as final user message
- `stream`: request stream mode where applicable
- `heal`: enables healing mode

### `switch`

```yaml
node_type:
  switch:
    branches:
      - condition: '$.nodes.classifier.output.category == "x"'
        target: branch_x
    default: fallback_node
```

### `custom_worker`

```yaml
node_type:
  custom_worker:
    handler: GetRagData
config:
  payload:
    topic: termination
```

## 4) Output Schema (Important)

Use `config.output_schema` for every `llm_call` node.

```yaml
config:
  output_schema:
    type: object
    properties:
      decision:
        type: string
        enum: [continue, terminated]
      message:
        type: string
    required: [decision, message]
    additionalProperties: false
```

Notes:

- `config.schema` is accepted as an alias.
- If omitted, runtime falls back to a permissive object schema.
- Best practice: always define schema explicitly to avoid drift.

## 5) Prompt Templating and Context

Templates can read from:

- `input.*` (workflow input)
- `nodes.<node_id>.output.*` (prior node outputs)
- `globals.*` (memory values)

Example:

```yaml
prompt: |
  Category: {{ nodes.classify_top_level.output.category }}
  User text: {{ input.email_text }}
  Memory: {{ globals.last_policy }}
```

## 6) Globals / Memory

YAML supports mutable memory:

- `config.set_globals`
- `config.update_globals` with `op: set|append|increment|merge`

Use this for conversation/workflow state that should persist within a run.

## 7) Chat-History Workflows

Pass message arrays as `input.messages`:

```json
{
  "email_text": "optional scalar input",
  "messages": [
    {"role":"system","content":"..."},
    {"role":"user","content":"..."}
  ]
}
```

`role` values:

- `system`
- `user`
- `assistant`
- `tool` (requires `tool_call_id`)

## 8) Running Workflows

### Rust API

```rust
use serde_json::json;
use simple_agents_workflow::run_workflow_yaml_file_with_client;

let output = run_workflow_yaml_file_with_client(
    std::path::Path::new("examples/workflow_email/email-unified-chat-intake-classification.yaml"),
    &json!({ "email_text": "Need replacement", "messages": [] }),
    &client,
).await?;
```

### Python Examples

- Chat runner:
  - `uv run --directory examples python workflow_email/run_with_chat_history.py`
- Unified system runner:
  - `uv run --directory examples python workflow_email/run_with_unified_system.py`
- Interview workflow:
  - `uv run --directory examples python workflow_email/run_with_chat_history.py --workflow examples/workflow_email/python-intern-fun-interview-system.yaml`

## 9) Visualize Workflow Graph

CLI Mermaid command:

```bash
cargo run -p simple-agents-cli -- workflow mermaid examples/workflow_email/python-intern-fun-interview-system.yaml
```

Output is Mermaid `flowchart TD` text for docs, PRs, and debugging.

## 10) Telemetry and Traces

Workflow outputs include:

- `trace` (node order)
- `step_timings` per node
- `total_elapsed_ms`

In Python chat runner, per-turn records are persisted as JSONL trace files.

## 11) Design Patterns That Work Well

1. **State classifier -> switch router -> action node**
2. **Action + deterministic guardrails via custom workers**
3. **One-question-at-a-time interview/chat progression**
4. **Always explicit output schema for every LLM node**
5. **Close-loop termination states** (no reopening in same session)

## 12) Common Pitfalls

- Missing `output_schema` on `llm_call` nodes.
- Switch conditions referencing nonexistent node paths.
- Using multi-question prompts when workflow expects one-step progression.
- Forgetting to rebuild local Python binding in `examples` env after Rust changes.

## 13) Troubleshooting

- Stale examples binding:

```bash
uv sync --directory examples --reinstall-package simple-agents-py
```

- Validate YAML graph quickly:

```bash
cargo run -p simple-agents-cli -- workflow mermaid examples/workflow_email/email-unified-chat-intake-classification.yaml
```

If Mermaid renders, the YAML parsed and graph wiring is valid enough for visualization.

## 14) Production Checklist

- Every `llm_call` has explicit `config.output_schema`.
- Every `switch` has deterministic `default` route.
- High-risk or policy-critical decisions are represented as deterministic nodes.
- Session-close states (e.g., terminated) are modeled and enforced.
- Trace/timing output is captured and retained for auditing.
