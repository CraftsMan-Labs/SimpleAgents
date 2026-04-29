# OTEL Configuration

OpenTelemetry setup for workflow examples lives in [`TRACING_ARCHITECTURE.md`](TRACING_ARCHITECTURE.md).

For the local Jaeger examples, set:

```bash
export SIMPLE_AGENTS_TRACING_ENABLED=true
export OTEL_EXPORTER_OTLP_PROTOCOL=grpc
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export OTEL_EXPORTER_OTLP_HEADERS=
export OTEL_SERVICE_NAME=simple-agents-examples
```

Then start an OTLP-compatible collector such as Jaeger before running the tracing examples.
