Cross-language bindings (JS/TS and Go):

- [ ] API surface design: mirror Python Client/builder/healing API in JS/TS (N-API) and Go; pin schema for messages/tool-calls/structured output to avoid drift and reuse `simple-agent-type` structs.
- [ ] JS/TS bindings: implement N-API module, emit `.d.ts` from Rust/TS types, provide async methods for completion/streaming/structured/healing; add npm packaging scripts and README with usage.
- [ ] JS/TS validation/tests: parity tests with Python (basic completion, streaming, structured JSON, healing) plus a runnable example akin to `examples/full_api_example.rs` and `examples/python_client.py`; mock transport for unit tests + live toggle via env (CUSTOM_API_*).
- [ ] Go bindings: choose FFI approach (cgo to Rust FFI crate vs Go shim); expose idiomatic Go Client with context support and streaming channels; add module metadata (go.mod) and docs.
- [ ] Go validation/tests: parity tests for completion/streaming/structured/healing; runnable example mirroring Rust/Python; mockable transport plus optional live env (CUSTOM_API_*).
- [ ] CI/publish: extend release to build/test/publish npm package and Go module; add `make` targets and version sync hooks; ensure tagging bumps JS/TS/Go versions alongside Rust/Python.
- [ ] Credentials/fixtures: document required envs (CUSTOM_API_BASE, CUSTOM_API_KEY, CUSTOM_API_MODEL or provider-specific overrides), provide dummy/local mode for tests, and add sample .env templates for examples.
