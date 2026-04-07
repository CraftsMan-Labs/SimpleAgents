# Tracing Architecture (OTLP Backends)

This document defines tracing data flow for SimpleAgents workflow runs when used from an external API layer, with OTLP destinations such as Jaeger and Langfuse.

## Goals

- Correlate external API traces and in-repo workflow traces as one transaction.
- Emit mandatory per-handler spans for custom worker execution.
- Return trace correlation IDs in workflow outputs.
- Keep payload capture full by default while remaining toggle-ready for redaction.

## Correlation model

Required attributes across spans/events:

- `trace_id`
- `span_id`
- `workspace_id`
- `user_id`
- `conversation_id`
- `request_id`
- `workflow_id`
- `run_id`
- `node_id`
- `handler_lang`

## Runtime options contract

Workflow execution accepts a structured options object (`YamlWorkflowRunOptions`):

- `telemetry`
  - `enabled` (`true` by default)
  - `sample_rate` (`1.0` by default)
  - `payload_mode` (`full_payload` default, `redacted_payload` optional)
  - `retention_days` (`30` by default)
  - `multi_tenant` (`true` by default)
- `trace`
  - `context`: `trace_id`, `span_id`, `parent_span_id`, `traceparent`, `tracestate`, `baggage`
  - `tenant`: `workspace_id`, `user_id`, `conversation_id`, `request_id`, `run_id`

## Runtime exporter env contract

Tracing exporter configuration is clean-break and OTLP-native:

- `SIMPLE_AGENTS_TRACING_ENABLED`
- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL` (`grpc` or `http/protobuf`)
- `OTEL_EXPORTER_OTLP_HEADERS` (`k=v,k2=v2`)
- `OTEL_SERVICE_NAME`

Notes:

- For `http/protobuf`, endpoint can be a base OTLP URL; traces are emitted to the traces signal path (`/v1/traces`).
- OTLP headers are applied in both `grpc` and `http/protobuf` modes.

### Jaeger / Collector example

```bash
export SIMPLE_AGENTS_TRACING_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export OTEL_EXPORTER_OTLP_PROTOCOL=grpc
export OTEL_SERVICE_NAME=simple-agents-workflow
```

### Langfuse API example

```bash
export SIMPLE_AGENTS_TRACING_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=https://cloud.langfuse.com/api/public/otel
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Basic <base64(public_key:secret_key)>,x-langfuse-ingestion-version=4"
export OTEL_SERVICE_NAME=simple-agents-workflow
```

## Output contract

Workflow results include correlation IDs in both locations:

- top-level `trace_id`
- `metadata.telemetry.trace_id`
- `metadata.telemetry.sampled`
- `metadata.trace.tenant.*` (including `conversation_id` when provided)

Sampling behavior:

- `sample_rate` is validated at runtime and must be between `0.0` and `1.0` (inclusive).
- Sampling decision is deterministic per `trace_id` (same `trace_id` + same `sample_rate` => same sampled outcome).
- `trace_id` is still returned even when a run is not sampled, so logs/events can stay correlated.

## Programmatic Correlation In External Code

### What is possible

- Reuse the same `trace_id` across multiple turns/runs.
- Pass `conversation_id` (UUID) for chat/session grouping.
- Correlate external handler telemetry with workflow telemetry using shared IDs.
- Custom worker `context` now includes an injected `trace` block automatically.

Important: traces are append-only. You do not mutate old emitted spans; each run adds new spans/events.

### Python example (streaming)

Use the unified request shape with `client.stream` (same tracing options as `run` / `run_async`):

```python
request = {
    "workflow_path": "workflow.yaml",
    "messages": workflow_messages,  # list of {role, content} dicts
    "workflow_options": {
        "telemetry": {"enabled": True, "nerdstats": True},
        "trace": {
            "context": {
                "trace_id": "4f6f4a0e7b6e4f7a85fd5ec3e3de31a9",
            },
            "tenant": {
                "conversation_id": "6e6d3125-b9f1-4af2-af1f-7cca024a2c42",
                "request_id": "turn-12",
                "run_id": "chat-turn-12",
            },
        },
    },
    "execution": {
        "workflow_streaming": True,
        "node_llm_streaming": True,
    },
}
result = client.stream_workflow(request, on_event=on_event)
```

### YAML custom worker payload forwarding example

If your external handler emits its own logs/metrics/spans, forward IDs in payload/context:

```yaml
- id: rag_lookup
  node_type:
    custom_worker:
      handler: get_rag_data
  config:
    payload:
      topic: termination_repeated_offense
      trace_id: "{{ input.trace_id }}"
      conversation_id: "{{ input.conversation_id }}"
```

### Automatic trace context in custom worker context

`YamlWorkflowCustomWorkerExecutor.execute(..., context)` receives this shape:

```json
{
  "input": { "...": "..." },
  "nodes": { "...": "..." },
  "globals": { "...": "..." },
  "trace": {
    "context": {
      "trace_id": "...",
      "span_id": "...",
      "parent_span_id": "...",
      "traceparent": "00-...-...-01",
      "tracestate": "...",
      "baggage": { "...": "..." }
    },
    "tenant": {
      "workspace_id": "...",
      "user_id": "...",
      "conversation_id": "...",
      "request_id": "...",
      "run_id": "..."
    }
  }
}
```

Use `context.trace.context.traceparent` (or `trace_id`) when starting external spans, and carry `context.trace.tenant.conversation_id` as a span attribute for chat-level grouping.

### Handler-side guidance

- Include `trace_id` and `conversation_id` in every handler log/event.
- If you have an OTel tracer in external code, set these as span attributes.
- Keep one trace per turn/request as the default operational model; use `conversation_id` to stitch multi-turn chats.

## Data flow

```mermaid
flowchart TD
  A[External API Layer] -->|structured trace context + optional raw fields| B[Binding/FFI Entry]
  B --> C[Workflow Runner]
  C --> D[workflow.run span]
  D --> E[workflow.node.execute spans]
  E --> F[handler.invoke spans (mandatory)]
  F --> G[Workflow Output]
  G --> H[top-level trace_id]
  G --> I[metadata.telemetry.trace_id]
```

## Backend fanout

Recommended deployment path:

- SDK/runtime spans -> OpenTelemetry Collector
- Collector -> Jaeger (trace UI + query)
- Jaeger storage -> OpenSearch (retention policy e.g. 30 days)
- Collector can fan out to multiple backends (e.g. Jaeger + Langfuse) when needed.

This keeps vendor interoperability through OpenTelemetry and avoids backend-specific runtime code paths.
