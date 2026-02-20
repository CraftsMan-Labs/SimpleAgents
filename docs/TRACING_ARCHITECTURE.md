# Tracing Architecture (Jaeger + PostHog)

This document defines tracing data flow for SimpleAgents workflow runs when used from an external API layer.

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
  - `tenant`: `workspace_id`, `user_id`, `request_id`, `run_id`

## Output contract

Workflow results include correlation IDs in both locations:

- top-level `trace_id`
- `metadata.telemetry.trace_id`

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
- Collector or API side sink -> PostHog events with shared `trace_id`

This keeps vendor interoperability through OpenTelemetry while allowing product analytics correlation in PostHog.
