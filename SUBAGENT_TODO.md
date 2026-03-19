# SUBAGENT TODO

Purpose: Scratchpad for active subagent assignments.

## Status values

- `pending`
- `in_progress`
- `completed`
- `blocked`

## Scratchpad assignments

| Parent TODO | Subagent | Scope | Why | Expected Outcome | Status | Notes |
|---|---|---|---|---|---|---|
| LF1 | SA-Tracing-Config-Contract | `crates/simple-agents-workflow/src/observability/tracing.rs`, `docs/OTEL_CONFIGURATION.md` | We need one clean-break env contract shared across Jaeger/Langfuse/future OTLP backends | Typed config schema and strict env parsing rules documented and enforced with validation tests | completed | Added canonical env constants, strict parser, and new OTEL configuration guide |
| LF2, LF3, LF4 | SA-Otel-Exporter-Factory | `crates/simple-agents-workflow/src/observability/tracing.rs`, `crates/simple-agents-workflow/Cargo.toml` | Exporter wiring must be protocol-agnostic and easy to extend | Factory creates OTLP gRPC and OTLP HTTP/protobuf exporters from the same config object; Jaeger and Langfuse are endpoint/protocol choices only | completed | Added protocol enum + factory wiring, enabled `http-proto` and `reqwest-client` features |
| LF5, LF6 | SA-Workflow-Span-Attributes | `crates/simple-agents-workflow/src/yaml_runner.rs` | Current attribute application is duplicated and inconsistent between span types | Shared helpers apply canonical attrs and Langfuse-friendly aliases consistently for workflow/node/tool/handler spans | completed | Added shared helpers + alias attributes (`langfuse.user.id`, `langfuse.session.id`) and reused them across span callsites |
| LF7 | SA-Tracing-Unit-Tests | `crates/simple-agents-workflow/src/observability/tracing.rs` tests | Clean-break config refactor is high-risk without full parser/exporter coverage | Unit tests cover protocol parsing, header parsing, invalid env cases, and provider initialization behavior | completed | Added parser/config tests including negative header/protocol cases |
| LF8 | SA-Workflow-Tracing-Regression-Tests | `crates/simple-agents-workflow/src/yaml_runner.rs` tests | Runtime output contracts must stay stable while internals are refactored | Tests verify trace ID behavior, sampling flags, tenant metadata, and span-attribute helper behavior | completed | Added regression test for tenant + Langfuse alias attribute mapping |
| LF9 | SA-Otel-Http-Integration-Test | New integration test module under `crates/simple-agents-workflow/tests/` | Langfuse path requires OTLP HTTP correctness including headers and endpoint handling | Mock HTTP endpoint test proves exporter sends requests with configured headers and handles flush lifecycle | pending | Prefer lightweight test server dependency; keep tests hermetic |
| LF10 | SA-Docs-Cross-Bindings | `docs/TRACING_ARCHITECTURE.md`, `docs/YAML_WORKFLOW_SYSTEM.md`, `docs/BINDINGS_GO.md`, `docs/BINDINGS_NODE.md`, `docs/BINDINGS_PYTHON.md` | Clean-break changes must be clear for all language users and operators | Updated docs show unified env contract and concrete Jaeger/Langfuse setup examples | completed | Added new `docs/OTEL_CONFIGURATION.md` and updated tracing/bindings docs with clean-break env contract |
| LF11 | SA-Quality-Gates | Repository-wide checks via `Makefile` targets | Cross-cutting tracing changes require strong pre-merge verification | All required gates pass (`fmt`, `clippy`, `test-rust`, binding contract/layer checks) | blocked | `fmt`, `clippy`, `test-rust` passed. Binding tests fail due local Node/N-API setup (`index.node` missing and unresolved `napi_*` symbols) |
