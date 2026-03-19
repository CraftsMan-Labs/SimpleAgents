# Active TODO

Date: 2026-03-18
Purpose: Langfuse OTEL API integration as a clean-break tracing foundation for current and future third-party OTEL backends.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Master tasks

| ID | Task | Why this is needed | Expected outcome | Status |
|---|---|---|---|---|
| LF1 | Define clean-break tracing env contract | Current tracing envs are Jaeger-specific and not protocol-flexible; we need one minimal vendor-agnostic contract | New canonical env schema finalized: `SIMPLE_AGENTS_TRACING_ENABLED`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_PROTOCOL`, `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_SERVICE_NAME` | completed |
| LF2 | Refactor tracing bootstrap into typed config + factories | Existing `tracing.rs` mixes env parsing, exporter creation, and provider lifecycle, making extension risky | `TracingConfig`, `TracingConfigLoader`, and exporter/provider factory boundaries introduced with single-responsibility modules | completed |
| LF3 | Add OTLP HTTP/protobuf exporter path for Langfuse API | Langfuse OTEL ingest endpoint requires OTLP over HTTP; current runtime is gRPC-only | Runtime can initialize OTLP exporter via `grpc` or `http/protobuf` based on unified env config | completed |
| LF4 | Keep Jaeger path first-class under same config model | We must preserve existing Jaeger compatibility while adding Langfuse | Jaeger/collector flow works unchanged by setting endpoint+protocol only; no backend-specific code paths required | completed |
| LF5 | Normalize span attributes via shared mapper | Trace attributes are repeated in multiple workflow execution paths and are inconsistent across span types | Common attribute helpers apply stable canonical attributes (`trace_id`, `tenant.*`, workflow/node identifiers) across workflow/node/tool/handler spans | completed |
| LF6 | Add Langfuse-friendly alias attributes without breaking canonical keys | Langfuse query/filter UX benefits from conventional keys, but internal contracts should stay stable | Each relevant span includes `langfuse.user.id`/`langfuse.session.id` aliases (and optional `user.id`/`session.id`) alongside existing canonical attrs | completed |
| LF7 | Expand tracing test suite for config and exporter matrix | Clean-break refactor must be hardened against config regressions and protocol miswiring | Unit tests cover env parsing, validation, protocol selection, header parsing, and provider creation behavior | completed |
| LF8 | Add runtime-level tracing regression tests in workflow runner | Span attribute consistency and trace metadata behavior are critical runtime contracts | Tests assert consistent attribute propagation and no regressions in trace ID, sampling, and tenant metadata output | completed |
| LF9 | Add integration tests for Langfuse-style OTLP HTTP ingestion | We need confidence that emitted traces can be delivered to HTTP OTLP endpoints with required headers | Mock HTTP OTLP test validates request endpoint, headers, and successful exporter flush path | pending |
| LF10 | Update cross-language docs and operational runbooks | This is a clean-break config change and needs exact setup instructions to avoid rollout failures | Updated docs for Rust/Go/Node/Python users, plus a focused OTEL configuration guide with Jaeger and Langfuse examples and troubleshooting | completed |
| LF11 | Run full quality gates and release validation | Refactor touches core runtime and observability contracts; must pass all gates before merge | `make fmt`, `make clippy`, `make test-rust`, `make test-binding-contracts`, and `make test-binding-layers` pass with no regressions | blocked |

## Technical notes

- Rust remains source-of-truth for tracing behavior and schema.
- Bindings should remain pass-through for workflow options (no duplicated backend logic).
- Single endpoint per process remains the default design; multi-destination fan-out is delegated to OTEL Collector.
- LF11 is currently blocked by existing local Node/N-API environment issues (`index.node` missing in layered tests and unresolved `napi_*` symbols during node contract build).
