Cross-language bindings (JS/TS and Go):

- [ ] API surface design: mirror Python Client/builder/healing API in JS/TS (N-API) and Go; pin schema for messages/tool-calls/structured output to avoid drift and reuse `simple-agent-type` structs.
  - Status: Node `Client` exposes `complete`/`stream` via `CompletionRequest` builder and provider-from-env helpers (openai/anthropic/openrouter) with `mode` support (`standard`/`healed_json`/`schema` + inline schema value parsing); Go wrapper exposes `NewClientFromEnv` + prompt-only `Complete`.
  - Gaps: JS/TS still lacks typed tool-call returns/partial streaming types and streaming healing; Go lacks context support, streaming, and message-based inputs.
- [ ] JS/TS bindings: implement N-API module, emit `.d.ts` from Rust/TS types, provide async methods for completion/streaming/structured/healing; add npm packaging scripts and README with usage.
  - Status: Node bindings now surface `mode` (`standard`/`healed_json`/`schema`) with inline schema parsing from JSON values, returning healed/coerced metadata; README updated; build/test/publish npm scripts in place.
  - Gaps: streaming still standard-only; `.d.ts` remains handwritten (not generated from Rust/types) and publish metadata/prebuild matrix still missing.
- [ ] JS/TS validation/tests: parity tests with Python (basic completion, streaming, structured JSON, healing) plus a runnable example akin to `examples/full_api_example.rs` and `examples/python_client.py`; mock transport for unit tests + live toggle via env (CUSTOM_API_*).
  - Status: added `node --test` live smoke for `complete` + `healed_json` (env-gated) in `crates/simple-agents-napi/test/basic.test.js`; npm `pretest` builds addon.
  - Next: add mock transport contract tests (no-network), streaming coverage, structured/schema fixtures, and a documented runnable example in the package.
  - TODO: wire `make test-node` into CI matrix with env toggle.
- [ ] Go bindings: choose FFI approach (cgo to Rust FFI crate vs Go shim); expose idiomatic Go Client with context support and streaming channels; add module metadata (go.mod) and docs.
  - Status: `bindings/go` uses cgo against `simple-agents-ffi` with `Complete(model, prompt, maxTokens, temperature)`; go.mod + minimal README exist.
  - Next: decide long-term FFI shape, add context-aware client, message-based requests, streaming/tool-calls/structured outputs, and richer docs.
  - Draft C FFI additions (to expose messages + streaming):
    ```c
    typedef struct {
        const char *role;
        const char *content;
        const char *name;
        const char *tool_call_id;
    } SAMessage;

    typedef void (*sa_stream_cb)(const char *chunk_json, void *user_data);

    SAClient *sa_client_new_from_env(const char *provider_name);
    char *sa_complete_messages(SAClient *client, const char *model, const SAMessage *msgs, size_t len, int32_t max_tokens, float temperature);
    int sa_stream_messages(SAClient *client, const char *model, const SAMessage *msgs, size_t len, int32_t max_tokens, float temperature, sa_stream_cb cb, void *user_data);
    ```
  - Draft Go wrapper shape:
    ```go
    type Message struct {
        Role string
        Content string
        Name string
        ToolCallID string
    }

    func (c *Client) Complete(ctx context.Context, model string, messages []Message, opts Options) (Result, error) {
        // marshal messages -> C array, respect ctx.Done
    }

    func (c *Client) Stream(ctx context.Context, model string, messages []Message, opts Options) (<-chan Chunk, <-chan error) {
        // bridge sa_stream_messages via callback -> goroutine sends on channel
    }
    ```
- [ ] Go validation/tests: parity tests for completion/streaming/structured/healing; runnable example mirroring Rust/Python; mockable transport plus optional live env (CUSTOM_API_*).
  - Status: no automated tests or runnable example beyond README snippet.
  - Next: add unit/integration coverage with mock transport + env-gated live runs, and a sample program mirroring Rust/Python demos.
  - TODO: add `make test-go-bindings` (build FFI + run Go tests/examples) and include in CI.
- [ ] CI/publish: extend release to build/test/publish npm package and Go module; add `make` targets and version sync hooks; ensure tagging bumps JS/TS/Go versions alongside Rust/Python.
  - Status: Makefile now includes `build-node`, `test-node`, `publish-node`, and `test-go-bindings`; npm build/test/publish scripts present; no CI jobs yet.
  - Next: wire CI for npm/go publish and version syncing across languages; add prebuild matrix for npm publish.
- [ ] Credentials/fixtures: document required envs (CUSTOM_API_BASE, CUSTOM_API_KEY, CUSTOM_API_MODEL or provider-specific overrides), provide dummy/local mode for tests, and add sample .env templates for examples.
  - Status: Node README/example expect `OPENAI_API_KEY`/`OPENAI_MODEL`; no shared .env template for bindings tests.
  - Next: document env matrix (incl. CUSTOM_API_*), add sample .env and dummy/local fixtures for automated tests/examples.
