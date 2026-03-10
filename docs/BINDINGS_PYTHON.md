# Python Binding (simple-agents-py)

Python bindings are provided by `simple-agents-py` (PyO3). They expose a high-level `Client` plus advanced helpers for healing and schema coercion.

## Installation

```bash
pip install simple-agents-py
```

## Quick Start

```python
from simple_agents_py import Client

client = Client("openai")
response = client.complete("gpt-4", "Hello from Python!", max_tokens=128, temperature=0.7)
print(response.content)
```

## Streaming

```python
from simple_agents_py import Client

client = Client("openai")
messages = [{"role": "user", "content": "Say hello in one sentence."}]
for chunk in client.complete("gpt-4o-mini", messages, max_tokens=64, stream=True):
    if chunk.content:
        print(chunk.content, end="", flush=True)
print()
```

## Structured Output (Schema)

```python
from simple_agents_py import Client

client = Client("openai")
schema = {
    "type": "object",
    "properties": {"name": {"type": "string"}, "age": {"type": "number"}},
    "required": ["name", "age"],
}
messages = [{"role": "user", "content": "Extract name and age: Alice is 28."}]
json_text = client.complete("gpt-4o-mini", messages, schema=schema, schema_name="person")
print(json_text)
```

When `stream=True` with `schema=...`, the iterator yields structured events with partial and complete values.

## Healing

```python
from simple_agents_py import Client

client = Client("openai")
messages = [{"role": "user", "content": "Return JSON: {\"name\":\"Sam\",\"age\":30}"}]
healed = client.complete(
    "gpt-4o-mini",
    messages,
    response_format="json",
    heal=True,
)
print(healed.content, healed.was_healed, healed.confidence)
```

Healing is enabled by default for structured outputs. You can disable it per client:

```python
client = Client("openai", healing=False)
```

## ClientBuilder (Routing, Cache, Middleware)

```python
from simple_agents_py import (
    CacheConfig,
    ClientBuilder,
    HealingConfig,
    ProviderConfig,
    RoutingPolicy,
)

class TimingMiddleware:
    def before_request(self, request):
        print("sending", request.model)

client = (
    ClientBuilder()
    .add_provider_config(ProviderConfig("openai", api_key="sk-..."))
    .with_routing_policy(RoutingPolicy.direct())
    .with_cache_config(CacheConfig(ttl_seconds=60))
    .with_healing(HealingConfig(enabled=True, min_confidence=0.7))
    .add_middleware(TimingMiddleware())
    .build()
)
print(client.complete("gpt-4o-mini", "Give me one idea.").content)
```

## Schema Utilities

```python
from simple_agents_py import SchemaBuilder, heal_json, coerce_to_schema

builder = SchemaBuilder()
builder.field("name", "string", required=True)
builder.field("age", "int", required=True)
schema = builder.build()

result = heal_json('{"name": "Sam", "age": 30}')
coerced = coerce_to_schema(result.value, schema)
print(coerced.value)
```

## Notes

- `Client` reads provider API keys from environment variables when `api_key` is omitted.
- `complete()` accepts a prompt string or a list of message dicts.
- `response_format="json"` enables JSON parsing when paired with `heal=True`.

## Workflow YAML Runner (Rust-backed)

Python binding now exposes Rust workflow YAML execution directly:

```python
from simple_agents_py import Client

client = Client("openai", api_base="https://...", api_key="...")
result = client.run_email_workflow_yaml(
    "examples/workflow_email/email-intake-classification.yaml",
    "Termination request, second warning already issued",
)

print(result["terminal_output"])
print(result["step_timings"])      # per-node elapsed ms + optional token usage
print(result["llm_node_metrics"])  # llm node token/tps metrics by node id
print(result["total_elapsed_ms"])  # end-to-end runtime
print(result["total_input_tokens"])
print(result["total_output_tokens"])
print(result["total_tokens"])
print(result["total_thinking_tokens"])  # null when provider does not expose it
print(result["tokens_per_second"])      # completion tokens / second
```

To collect workflow events without live callbacks, set `include_events=True`:

```python
result = client.run_email_workflow_yaml(
    "examples/workflow_email/email-intake-classification.yaml",
    "Termination request, second warning already issued",
    include_events=True,
)

print(result["events"][0]["event_type"])
```

This method delegates to Rust `simple-agents-workflow` as the source of truth.

For chat-history workflows, use `run_workflow_yaml(...)` with structured workflow input:

```python
result = client.run_workflow_yaml(
    "examples/workflow_email/email-intake-classification.yaml",
    {
        "email_text": "Termination request, second warning already issued",
        "messages": [
            {"role": "system", "content": "You are an HR classifier."},
            {"role": "user", "content": "Termination request, second warning already issued"},
        ],
    },
)
```

### Live Workflow Events + LLM Deltas

`Client.run_email_workflow_yaml_stream(...)` emits live workflow events to a Python callback while running:

```python
def on_event(event: dict[str, object]) -> None:
    if event.get("event_type") == "node_stream_delta":
        print(event.get("delta", ""), end="", flush=True)
    else:
        print(event)

result = client.run_email_workflow_yaml_stream(
    "examples/workflow_email/email-intake-classification.yaml",
    "Termination request, second warning already issued",
    on_event=on_event,
    workflow_options={"telemetry": {"nerdstats": True}},
)
```

Notes:

- Streamability is node-aware; non-streamable nodes emit status events with explanatory text.
- Structured `node_stream_delta` content is sanitized to JSON object payload content, so reasoning/preamble/trailing chatter is not forwarded to callbacks.
- If a YAML `llm_call` sets `stream_json_as_text: true`, non-thinking stream tokens are emitted as plain text lines (`key: value`) instead of raw JSON token chunks.
- Token stream events include token attribution fields:
  - `step_id`: workflow step/node id for token attribution
  - `token_kind`: `output` or `thinking`
  - `is_terminal_node_token`: `true` when token is emitted from a terminal node
- `node_llm_input_resolved` is emitted before each `llm_call` with `metadata` containing:
  - resolved `prompt` and `prompt_template`
  - selected `model`, `schema`, and effective stream/heal flags
  - `bindings[]` entries that map each template expression to its source path and resolved value
- `workflow_completed` includes `metadata.nerdstats` by default (`telemetry.nerdstats=true`), with end-of-run timing/token metrics for turn-level summaries.
- When available for streamed runs, nerdstats includes `ttft_ms` (time-to-first-token in milliseconds).
- Nerdstats uses `step_details` for per-node timing details.
- Nerdstats no longer includes `llm_node_metrics`; use `llm_node_models` (`node_id -> resolved model`) for model attribution.
- For providers that do not emit token usage on streaming responses, nerdstats includes `token_metrics_available=false`, `token_metrics_source="provider_stream_usage_unavailable"`, and `llm_nodes_without_usage`.
- Disable nerdstats emission for streaming callbacks with `workflow_options={"telemetry": {"nerdstats": False}}`.
- `workflow_options["telemetry"]["sample_rate"]` controls deterministic per-trace sampling (`0.0` to `1.0` inclusive).
- Workflow output metadata includes `metadata.telemetry.sampled` so callers can branch on sampled vs unsampled traces.
- You can pass chat/session identity into trace metadata with `workflow_options={"trace": {"tenant": {"conversation_id": "<uuid>"}}}`; it is attached to workflow trace attributes and output metadata.
