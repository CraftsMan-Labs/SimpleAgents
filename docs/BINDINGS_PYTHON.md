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
result = client.run_workflow_yaml(
    "examples/workflow_email/email-intake-classification.yaml",
    {"email_text": "Termination request, second warning already issued"},
)

print(result["terminal_output"])
print(result["step_timings"])      # per-node elapsed ms + optional token usage
print(result["llm_node_metrics"])  # llm node token/tps metrics by node id
print(result["total_elapsed_ms"])  # end-to-end runtime
print(result["total_input_tokens"])
print(result["total_output_tokens"])
print(result["total_tokens"])
print(result["total_reasoning_tokens"])  # null when provider does not expose it
print(result["tokens_per_second"])      # completion tokens / second
```

To collect workflow events without live callbacks, set `include_events=True`:

```python
result = client.run_workflow_yaml(
    "examples/workflow_email/email-intake-classification.yaml",
    {"email_text": "Termination request, second warning already issued"},
    include_events=True,
)

print(result["events"][0]["event_type"])
```

This method delegates to Rust `simple-agents-workflow` as the source of truth.

### `custom_worker` handlers (Python)

For `custom_worker` nodes, the Rust runner loads `handler_file` (default: `handlers.py` next to the workflow YAML) and calls the function whose name **exactly** matches `handler`.

**Function signature.** Handlers are invoked with **only** keyword arguments:

```python
def my_handler(*, context: dict, payload: dict):
    ...
```

- **`payload`**: the resolved `config.payload` object from YAML after template interpolation. Put per-node parameters here (for example `topic`, `company_name`).
- **`context`**: execution context with:
  - `input`: the workflow input you passed to `run_workflow_yaml` (for example `email_text`, `messages`).
  - `nodes`: map of completed node outputs (same structure templates use under `nodes.*`).
  - `globals`: workflow globals map.
  - `trace` (when tracing is active): nested object with `context` (trace ids, `traceparent`, etc.) and `tenant` (workspace/user/conversation ids when set via run options).

**Return value.** Must be JSON-serializable (dict, list, str, number, bool, null). It becomes this node’s output; downstream prompts use `nodes.<node_id>.output` and fields on that object.

**Minimal example.**

```yaml
# fragment
  - id: lookup
    node_type:
      custom_worker:
        handler: get_seller_name
    config:
      payload:
        company_name: "{{ nodes.extract_company.output.company_name }}"
```

```python
def get_seller_name(*, context, payload):
    name = (payload or {}).get("company_name") or "unknown"
    return {"company_name": name, "stakeholder_name": "..."}
```

**Troubleshooting.** `TypeError: ... unexpected keyword argument 'context'` means the handler still uses an old signature (positional `topic`, or `email_text=`). Use `*, context, payload` and read `email_text` from `context["input"]` if your workflow passes it there.

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

Unified typed facade (messages-first request object):

```python
request = {
    "workflow_path": "examples/workflow_email/email-intake-classification.yaml",
    "messages": [{"role": "user", "content": "Please process invoice #123"}],
    "execution": {
        "healing": False,
        "workflow_streaming": False,
        "node_llm_streaming": True,
    },
}

result = client.run(request)
result2 = client.run_async(request)
streamed = client.stream(request, on_event=lambda event: print(event.get("event_type")))
```

`execution.model` overrides `workflow_options.model` when both are provided.

### Workflow stream DX (`simple_agents_py.workflow_stream`)

- **Low-level (full control):** `Client.stream(request, on_event=...)`.
- **Structured hooks (recommended):** `stream_workflow(client, request, hooks=...)` or `stream_workflow(client, request, on_event=...)` — *not both*.
- **Typed requests (Pydantic):** `pip install simple-agents-py[pydantic]`, then `from simple_agents_py.workflow_request import WorkflowExecutionRequest` and pass the model to `stream_workflow` / `run_workflow_request` / `run_workflow_request_async` (no hand-maintained dicts). Plain `dict` requests still work; coercion uses `simple_agents_py.workflow_payload.workflow_execution_request_to_mapping`.

Return value is the same **`WorkflowRunOutput`** mapping as `run` / `run_async` / `stream` (`workflow_id`, `entry_node`, `trace`, `outputs`, `terminal_node`, `terminal_output`, `step_timings`, `llm_node_metrics`, `llm_node_models`, `total_elapsed_ms`, `ttft_ms`, token aggregates, `trace_id`, `metadata`, and `events` when recorded). Shapes match `WorkflowRunOutput` in the bundled `simple_agents_py.pyi`.

**Split thinking vs. merged stream deltas.** Set `execution["split_stream_deltas"] = True` on the request when you want separate thinking vs output stream events.

### Live Workflow Events + LLM Deltas

`Client.run_workflow_yaml_stream(...)` emits live workflow events to a Python callback while running:

```python
def on_event(event: dict[str, object]) -> None:
    if event.get("event_type") == "node_stream_delta":
        print(event.get("delta", ""), end="", flush=True)
    else:
        print(event)

result = client.run_workflow_yaml_stream(
    "examples/workflow_email/email-intake-classification.yaml",
    {"email_text": "Termination request, second warning already issued"},
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
- Nerdstats uses `step_details[].model_name` for model attribution.
- For providers that do not emit token usage on streaming responses, nerdstats includes `token_metrics_available=false`, `token_metrics_source="provider_stream_usage_unavailable"`, and `llm_nodes_without_usage`.
- Disable nerdstats emission for streaming callbacks with `workflow_options={"telemetry": {"nerdstats": False}}`.
- `workflow_options["telemetry"]["sample_rate"]` controls deterministic per-trace sampling (`0.0` to `1.0` inclusive).
- Workflow output metadata includes `metadata.telemetry.sampled` so callers can branch on sampled vs unsampled traces.
- You can pass chat/session identity into trace metadata with `workflow_options={"trace": {"tenant": {"conversation_id": "<uuid>"}}}`; it is attached to workflow trace attributes and output metadata.

Tracing exporter env configuration is shared across runtimes:

- `SIMPLE_AGENTS_TRACING_ENABLED`
- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL` (`grpc` or `http/protobuf`)
- `OTEL_EXPORTER_OTLP_HEADERS`
- `OTEL_SERVICE_NAME`
