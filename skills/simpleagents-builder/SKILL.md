---
name: simpleagents-builder
description: This skill should be used when the user asks to create, improve, or validate YAML agent workflows, especially requests like "build an agent YAML", "design workflow YAML", "add routing in YAML", or "make interview/email workflow nodes and edges".
---

# SimpleAgentsBuilder

Author YAML workflows that are deterministic, testable, and compatible with SimpleAgents graph execution.

## Use This Skill For

- Create new graph YAML workflows (`entry_node`, `nodes`, `edges`)
- Refactor existing YAML agent flows
- Add routing, guardrails, and one-question-at-a-time behavior
- Add or tighten `output_schema` for `llm_call` nodes

## Core Rules

1. Model the workflow as a graph, not a linear prompt script.
2. Define `config.output_schema` for every `llm_call` node.
3. Keep switch routing deterministic and explicit.
4. Keep chat systems one-question-at-a-time unless user asks otherwise.
5. Keep business policy in prompts/routing, not hidden in bindings.
6. For `custom_worker`, verify `handler` matches a real function in `handler_file` (defaults to `handlers.py` next to the YAML when omitted). For **`simple-agents-py`**, that function must use **`def handler_name(*, context, payload):`** (keyword-only); put node parameters in YAML `config.payload` and read shared workflow input from `context["input"]`. File-based `handlers.py` runs automatically in **Python**. For **`simple-agents-node`**, pass **`customWorker`** (or **`customWorkerDispatch`** on legacy workflow APIs) with a function `(req) => unknown` where `req` is `{ handler, handlerFile?, payload, context }` (see `docs/BINDINGS_NODE.md`). **Go** packaged APIs do not execute local handlers today; **WASM** uses `workflowOptions.functions` with a JS `(args, graphContext)` signature (see repo `docs/BINDINGS_*.md` and `docs/YAML_WORKFLOW_SYSTEM.md`).
7. Prefer unified workflow run APIs: Python `Client.run` / `run_async` / `stream`, Node `executeWorkflowYaml` / `executeWorkflowYamlStream`, Go `Run` / `RunAsync` / `Stream`, WASM `streamWorkflow` (see `docs/WORKFLOW_API_MIGRATION.md`). Use legacy `run_workflow_yaml*` / `runWorkflowYaml*` only for compatibility. Keep streaming/healing controlled by YAML `execution` / node flags and `workflow_options`, not ad-hoc wrapper selection.

## Required Structure

Use this skeleton:

```yaml
id: workflow-id
version: 1.0.0
entry_node: start_node

nodes:
  - id: start_node
    node_type:
      llm_call:
        model: gemini-3-flash
        messages_path: input.messages
        append_prompt_as_user: true
    config:
      output_schema:
        type: object
        properties:
          state:
            type: string
        required: [state]
        additionalProperties: false
      prompt: |
        Return JSON only.

edges:
  - from: start_node
    to: next_node
```

## Node Design Pattern

- `detect_*` node: classify intent/state with strict enum schema.
- `route_*` node (`switch`): deterministic branching by JSON-path conditions.
- worker/action nodes: `llm_call` for generation, `custom_worker` only when handler is intentional.
- terminal behavior: explicit node with final message/question.

Example `custom_worker` declaration (with `config.payload` for handler inputs):

```yaml
- id: rag_lookup
  node_type:
    custom_worker:
      handler: get_rag_data
      handler_file: handlers.py
  config:
    payload:
      topic: probation
```

## Routing Pattern

For state-based routing:

```yaml
node_type:
  switch:
    branches:
      - condition: '$.nodes.detect.output.state == "ready"'
        target: generate
    default: clarify
```

Keep conditions simple (`==`, `!=`) and tied to a stable node output path.

## Prompting Pattern

- Instruct: `Return JSON only`.
- Give exact response shape.
- Encode policy as explicit bullet rules.
- Keep each node prompt single-responsibility.

## Validation Checklist

Before finalizing YAML:

- IDs are unique
- `entry_node` exists
- Every `switch` target exists
- Every `llm_call` has `output_schema`
- Required fields align with routing conditions
- `edges` cover intended flow transitions
- No ambiguous multi-question prompts in interview/chat flows
- For `custom_worker` nodes, `handler` matches a function in `handler_file` (defaults to `handlers.py` next to the YAML when omitted); for Python runs, signature is `*, context, payload`.
- Workflow examples use the unified run/stream APIs, not legacy email-specific wrappers

For examples and reusable templates, read:
- `references/patterns.md`
- `references/checklist.md`

Working examples (under `examples/workflow_email/` and `skills/simpleagents-builder/examples/`):
- `examples/workflow_email/email-chat-draft-or-clarify.yaml`
- `skills/simpleagents-builder/examples/python-intern-fun-interview-system.yaml`
