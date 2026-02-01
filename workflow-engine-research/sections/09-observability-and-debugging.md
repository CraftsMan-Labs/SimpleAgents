# Observability and Debugging

## Recommended Repo
- https://github.com/open-telemetry/opentelemetry-rust

## Why This Repo
- Standard tracing/metrics implementation for Rust.
- Fits node-level spans, lineage, and structured metrics.

## Pros
- Standardized tracing and metrics with wide tooling support.
- Easy to export to common backends (OTLP, Jaeger, etc.).

## Cons
- Requires disciplined instrumentation to be useful.
- Trace data volume can grow quickly at high throughput.

## What We Want To Build From This
- Per-node spans with structured attributes (node id, graph id, status).
- Lineage tracking for inputs/outputs and decision paths.

## Why
- Debugging complex workflows requires consistent tracing and metrics.

## Sources
- https://opentelemetry.io/docs/languages/rust/
- https://opentelemetry.io/docs/languages/rust/getting-started/

## Notes
- Use OpenTelemetry tracing conventions for per-node spans and workflow lineage.
