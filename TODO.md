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
