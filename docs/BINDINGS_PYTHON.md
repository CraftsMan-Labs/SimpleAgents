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

Each yielded item is a `StreamChunk` with `.content`, `.finish_reason`, `.model`, and `.index`.

## Structured Output (Schema + Streaming)

Pass a JSON Schema dict as `schema=...`. When `stream=True`, the iterator yields `PyStructuredEvent` objects; when `stream=False`, the response is the usual `ResponseWithMetadata` (healing/coercion is applied at the caller level via `heal_json` / `coerce_to_schema`).

```python
from simple_agents_py import Client

client = Client("openai")
schema = {
    "type": "object",
    "properties": {"name": {"type": "string"}, "age": {"type": "number"}},
    "required": ["name", "age"],
}
messages = [{"role": "user", "content": "Extract name and age: Alice is 28."}]

# Non-streaming: returns ResponseWithMetadata
result = client.complete("gpt-4o-mini", messages, schema=schema)
print(result.content)

# Streaming: yields PyStructuredEvent objects
for event in client.complete("gpt-4o-mini", messages, schema=schema, stream=True):
    if event.is_complete:
        print(event.value)          # parsed Python dict
        print(event.confidence)     # float 0.0–1.0
        print(event.was_healed)     # bool
    else:
        print(event.partial_value)  # partially parsed value
```

`schema` must be a `dict`; passing any other type raises `RuntimeError`.

## Healing Utilities

Use `heal_json` and `coerce_to_schema` to parse and validate LLM output directly:

```python
from simple_agents_py import heal_json, coerce_to_schema

result = heal_json('{"name": "Sam", "age": 30,}')  # trailing comma fixed automatically
print(result.value)        # Python dict
print(result.confidence)   # float 0.0–1.0
print(result.was_healed)   # bool
print(result.flags)        # list of strings describing applied repairs

schema = {
    "type": "object",
    "properties": {"name": {"type": "string"}, "age": {"type": "number"}},
    "required": ["name", "age"],
}
coerced = coerce_to_schema(result.value, schema)
print(coerced.value)       # coerced Python dict
print(coerced.was_coerced) # bool
```

### Incremental streaming parser

`StreamingParser` accumulates token chunks and heals incrementally:

```python
from simple_agents_py import StreamingParser

parser = StreamingParser()
parser.feed('{"name": "Sam"')
partial = parser.try_parse()   # ParseResult or None
parser.feed(', "age": 30}')
final = parser.finalize()      # ParseResult (raises if buffer is empty)
print(final.value, final.was_healed)
```

## Exported Types

| Name | Description |
|------|-------------|
| `Client` | Main completion and workflow client |
| `ResponseWithMetadata` | Non-streaming `complete()` result: `.content`, `.model`, `.provider`, `.finish_reason`, `.latency_ms`, `.usage`, `.tool_calls` |
| `StreamChunk` | One streaming delta: `.content`, `.finish_reason`, `.model`, `.index` |
| `PyStreamIterator` | Iterator returned by `complete(..., stream=True)` without schema |
| `PyStructuredStreamIterator` | Iterator returned by `complete(..., stream=True, schema=...)` |
| `PyStructuredEvent` | Item from `PyStructuredStreamIterator`: `.is_partial`, `.is_complete`, `.value`, `.partial_value`, `.confidence`, `.was_healed` |
| `ParseResult` | Result of `heal_json()` or `StreamingParser.finalize()`: `.value`, `.confidence`, `.was_healed`, `.flags` |
| `CoercionResult` | Result of `coerce_to_schema()`: `.value`, `.confidence`, `.was_coerced`, `.flags` |
| `HealedJsonResult` | Raw string result from healing: `.content`, `.confidence`, `.was_healed`, `.flags` |
| `StreamingParser` | Incremental JSON healing parser |

## Notes

- `Client` reads provider API keys from environment variables when `api_key` is omitted (e.g. `OPENAI_API_KEY`).
- `complete()` accepts a prompt string or a list of message dicts.
- `stream=True` and `heal=True` cannot be combined; pass one or the other.
- `schema` must be a `dict` when provided.

## Workflow YAML Runner (Rust-backed)

Workflow execution is driven by the Rust `simple-agents-workflow` crate. The Python `Client` exposes two methods for running YAML workflows.

### Client methods

- `client.run_workflow(request)` — run to completion, returns output dict
- `client.stream_workflow(request, on_event=None, include_events_in_output=False)` — run with live events

**`request` shape for `run_workflow` / `stream_workflow`**

```python
request = {
    "workflow_path": "workflow.yaml",
    "messages": [
        {"role": "user", "content": "Classify this email about an invoice from Google."},
    ],
}
result = client.run_workflow(request)
print(result["terminal_output"])
print(result["step_timings"])       # per-node elapsed ms + optional token usage
print(result["llm_node_metrics"])   # token/tps metrics by node id
print(result["total_elapsed_ms"])   # end-to-end runtime
print(result["total_tokens"])
```

Optional top-level keys:

- `input` — extra workflow fields merged into runner input (for `input.*` references in YAML).
- `execution` — `healing`, `workflow_streaming`, `node_llm_streaming`, `split_stream_deltas`, optional `model`.
- `workflow_options` — `telemetry`, `trace`, `model` (matches Rust `YamlWorkflowRunOptions`).

**Validation:** `execution.healing` and `execution.node_llm_streaming` cannot both be `true`. Per-node YAML `heal` / `stream` still apply independently.

**Streaming with `stream_workflow`**

```python
def on_event(event: dict) -> None:
    print(event.get("event_type"))

streamed = client.stream_workflow(request, on_event=on_event)
```

### Workflow evals

Eval datasets are output-shaped golden records. Each row stores workflow `input` and `expected_output`. The runner executes the workflow and passes each case to your evaluator callback.

```python
from simple_agents_py import Client, output_subset, run_eval_suite

client = Client("openai")
report = run_eval_suite(
    client,
    workflow_path="workflows/friendly/friendly.yaml",
    dataset_path="evals/friendly/friendly-eval.dataset.jsonl",
    evaluator=output_subset,
)

print(report.status)
print(report.cases[0].evaluations[0].reason)
```

### Python-level workflow helpers (`simple_agents_py.workflow_stream`)

Higher-level helpers that add structured hooks, display modes, and Pydantic request coercion:

```python
from simple_agents_py.workflow_stream import (
    stream_workflow,
)
```

These accept the same `request` dict **or** a `WorkflowExecutionRequest` Pydantic model from `simple_agents_py.workflow_request`. They wrap `client.stream_workflow` / `client.run_workflow` with extra conveniences:

- `stream_workflow(client, request, hooks=None, *, on_event=None, stream_display=None)` — structured event hooks, terminal printing, and execution flag merging.
- `stream_display="merged"` prints merged `node_stream_delta` tokens to stdout.
- `stream_display="split"` prints thinking vs output deltas and forces `split_stream_deltas`.

### Pydantic request models (`simple_agents_py.workflow_request`)

```python
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
    WorkflowRunOptions,
    WorkflowTelemetryConfig,
    WorkflowInput,
)

request = WorkflowExecutionRequest(
    workflow_path="path/to/workflow.yaml",
    messages=[WorkflowMessage(role=WorkflowRole.user, content="Hello")],
    workflow_options=WorkflowRunOptions(
        telemetry=WorkflowTelemetryConfig(nerdstats=True)
    ),
)
```

Python multimodal workflow messages use OpenAI-style content parts such as `{"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}}`. The Node binding also accepts its typed `ContentPartInput` shape (`{ type: "image", mediaType, data }`); do not blindly copy multimodal payloads across languages without translating the part shape.

Requires `pip install simple-agents-py[pydantic]`.

### `custom_worker` handlers (Python)

For `custom_worker` nodes, the Rust runner loads `handler_file` (default: `handlers.py` next to the workflow YAML) and calls the function whose name **exactly** matches `handler`.

**Function signature.** Handlers are invoked with **only** keyword arguments:

```python
def my_handler(*, context: dict, payload: dict):
    ...
```

- **`payload`**: the resolved `config.payload` object from YAML after template interpolation.
- **`context`**: execution context with:
  - `input`: merged workflow input from your request (`messages` and any `input` fields).
  - `nodes`: map of completed node outputs.
  - `globals`: workflow globals map.
  - `trace` (when tracing is active): nested object with `context` and `tenant`.

**Return value.** Must be JSON-serializable. It becomes this node's output.

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

**Troubleshooting.** `TypeError: ... unexpected keyword argument 'context'` means the handler still uses an old signature. Use `*, context, payload` and read shared fields from `context["input"]`.

### Live workflow events + LLM deltas

Set `execution.workflow_streaming` to `true` when you want token deltas delivered to `on_event` (lifecycle events still fire either way).

```python
request = {
    "workflow_path": "workflow.yaml",
    "messages": [{"role": "user", "content": "Classify this email about an invoice from Google."}],
    "workflow_options": {"telemetry": {"nerdstats": True}},
    "execution": {
        "workflow_streaming": True,
        "node_llm_streaming": True,
    },
}

def on_event(event: dict) -> None:
    if event.get("event_type") == "node_stream_delta":
        print(event.get("delta", ""), end="", flush=True)
    else:
        print(event)

result = client.stream_workflow(request, on_event=on_event)
```

Notes:

- Streamability is node-aware; non-streamable nodes emit status events with explanatory text.
- Structured `node_stream_delta` content is sanitized to JSON object payload content.
- If a YAML `llm_call` sets `stream_json_as_text: true`, non-thinking stream tokens are emitted as plain text lines instead of raw JSON token chunks.
- Token stream events include `step_id`, `token_kind` (`output` or `thinking`), and `is_terminal_node_token`.
- `node_llm_input_resolved` is emitted before each `llm_call` with resolved `prompt`, `model`, `schema`, and `bindings[]`.
- `workflow_completed` includes `metadata.nerdstats` when `telemetry.nerdstats=true`.
- Disable nerdstats: `workflow_options={"telemetry": {"nerdstats": False}}`.
- `workflow_options["telemetry"]["sample_rate"]` controls deterministic per-trace sampling (`0.0` to `1.0`).
- Pass session identity: `workflow_options={"trace": {"tenant": {"conversation_id": "<uuid>"}}}`.

Tracing exporter env configuration:

- `SIMPLE_AGENTS_TRACING_ENABLED`
- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL` (`grpc` or `http/protobuf`)
- `OTEL_EXPORTER_OTLP_HEADERS`
- `OTEL_SERVICE_NAME`
