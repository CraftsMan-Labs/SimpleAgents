---
name: simpleagentsbuilder
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
    config:
      output_schema:
        type: object
        properties:
          state:
            type: string
        required: [state]
        additionalProperties: false
      user_input_prompt: |
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
- Put user instructions in `config.user_input_prompt`; use `config.node_system_prompt` for optional system-level guardrails.
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

For examples and reusable templates, read:
- `references/patterns.md`
- `references/checklist.md`

Working examples:
- `examples/minimal-chat.yaml`
- `examples/email-classification.yaml`
