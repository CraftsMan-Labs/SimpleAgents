Formatting fixes (from `cargo fmt --all -- --check`):

- [x] simple-agents-cli: reformat imports and long method calls in `crates/simple-agents-cli/src/main.rs`.
- [x] simple-agents-core: apply rustfmt to `examples/basic_client.rs`, `src/client.rs`, `src/healing.rs`, `src/lib.rs`, `src/routing.rs`, and `tests/client_integration.rs`.
- [x] simple-agents-ffi: rustfmt adjustments in `crates/simple-agents-ffi/src/lib.rs`.
- [x] simple-agents-healing: rustfmt coercion/number handling in `src/coercion.rs` and parser test formatting in `src/parser.rs`.
- [x] simple-agents-napi: reorder imports and wrap long lines in `crates/simple-agents-napi/src/lib.rs`.
- [x] simple-agents-providers: rustfmt across examples (`anthropic_*`, `cache_usage.rs`, `custom_api.rs`, `healing_fallback.rs`, `openai_*`, `openrouter_basic.rs`, `retry_demo.rs`, `streaming*`, `test_local_api.rs`), provider modules (`src/healing_integration.rs`, `src/openai/mod.rs`, `src/openrouter/mod.rs`, `src/schema_converter.rs`), and tests (`tests/healing_integration_tests.rs`, `tests/openai_integration.rs`).
- [x] simple-agents-py: apply rustfmt to `crates/simple-agents-py/src/lib.rs` (import ordering, wrapped closures, healing helpers).
- [x] simple-agents-router: rustfmt `examples/round_robin_router.rs`, `src/fallback.rs`, `src/latency.rs`, and `tests/health_tracker_integration.rs`.
- [x] examples: rustfmt `examples/full_api_example.rs`.

Cross-language bindings (JS/TS and Go):

- [ ] Design binding surface equivalent to Python API (Client, builder, healing options) for JS/TS (NAPI) and Go (cgo or pure Go FFI).
- [ ] JS/TS bindings: generate types from `simple-agent-type` (TS declarations), expose async API, and package via npm with build script and docs.
- [ ] JS/TS testing: add parity tests matching Python coverage (basic completion, streaming, structured output, healing) plus an example client mirroring `examples/full_api_example.rs` and `examples/python_client.py`.
- [ ] Go bindings: choose approach (cgo over FFI library vs. small Go shim), expose idiomatic Go API, and add module packaging metadata.
- [ ] Go testing: parity tests for completion/streaming/structured/healing and an example program mirroring the Rust/Python examples.
- [ ] CI/publish hooks: extend release automation to build/test/publish JS/TS package and Go module alongside Rust/Python; update `make` targets and version sync.
- [ ] Credentials/fixtures: document required API keys/endpoints (CUSTOM_API_BASE, CUSTOM_API_KEY, CUSTOM_API_MODEL or provider-specific equivalents) and provide mocked test mode to avoid real network where possible.
