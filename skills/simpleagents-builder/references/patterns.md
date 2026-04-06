# YAML AgentFactory Patterns

## 1) Detect -> Route -> Act

Use this as default architecture for chat workflows.

1. `detect_*` (`llm_call`) classifies state
2. `route_*` (`switch`) chooses next node
3. `act_*` (`llm_call` or `custom_worker`) produces result

## 2) Output Schema Discipline

For each `llm_call`, always define:

- `type: object`
- narrow `properties`
- strict `required`
- `additionalProperties: false`

Avoid open-ended schemas in routing-critical nodes.

## 3) One-Question Interview Loops

For interview/coaching flows:

- detection node decides readiness
- evaluation node outputs `status`, `reason`, `next_prompt`
- continuation node asks exactly one question

## 4) Policy-First Prompting

Keep policy explicit in prompt rules, for example:

- hard failure rule
- missing-context rule
- sequencing rule

This keeps behavior auditable and predictable.

## 5) Custom Worker Usage

Use `custom_worker` only when a named handler is intentional.
If handler logic is expected in app/editor code, keep payload structure stable:

```yaml
node_type:
  custom_worker:
    handler: GetRagData
config:
  payload:
    topic: terminated
```

**Python (`simple-agents-py`).** Implement handlers as `def handler_name(*, context: dict, payload: dict):`. Read node-specific fields from `payload` (matches `config.payload` after interpolation). Read workflow-wide input from `context["input"]` (for example `email_text` or `messages`). Return a JSON-serializable object; downstream templates use `nodes.<id>.output.<field>`.

**Other bindings.** Shipped **Node** and **Go** workflow APIs do not run `handlers.py`; **WASM** registers JS functions keyed by handler name. See `docs/YAML_WORKFLOW_SYSTEM.md` and `docs/BINDINGS_*.md`.

## 6) Common State Enum

Reusable state enum for assistant workflows:

- `ready`
- `missing_scenario`
- `capabilities_query`
- optional policy states (`terminated_already`, `policy_violation`)
