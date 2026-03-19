# OTEL Exporter Configuration

This document defines the clean-break tracing exporter contract for workflow tracing.

## Environment keys

- `SIMPLE_AGENTS_TRACING_ENABLED`
- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL`
- `OTEL_EXPORTER_OTLP_HEADERS`
- `OTEL_SERVICE_NAME`

## Protocol values

- `grpc`
- `http/protobuf`

## Defaults

- `SIMPLE_AGENTS_TRACING_ENABLED=false`
- `OTEL_EXPORTER_OTLP_PROTOCOL=grpc`
- `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317` when protocol is `grpc`
- `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318` when protocol is `http/protobuf`
- `OTEL_SERVICE_NAME=simple-agents-workflow`

## Example: Jaeger or Collector

```bash
export SIMPLE_AGENTS_TRACING_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export OTEL_EXPORTER_OTLP_PROTOCOL=grpc
export OTEL_SERVICE_NAME=simple-agents-workflow
```

## Example: Langfuse

```bash
export SIMPLE_AGENTS_TRACING_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=https://cloud.langfuse.com/api/public/otel
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Basic <base64(public_key:secret_key)>,x-langfuse-ingestion-version=4"
export OTEL_SERVICE_NAME=simple-agents-workflow
```

## Notes

- Header entries must use `key=value` format and be comma-separated.
- For `OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf`, `OTEL_EXPORTER_OTLP_ENDPOINT` can be a base OTLP URL (for example `http://localhost:4318`); runtime sends traces to the traces signal endpoint (`.../v1/traces`).
- `OTEL_EXPORTER_OTLP_HEADERS` is applied for both `grpc` and `http/protobuf` exporters.
- Runtime uses a single OTLP destination per process. Use an OpenTelemetry Collector for fan-out.
