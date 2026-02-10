# Cross-Language Parity TODO (Rust / Python / Node / Go)

Goal: every capability implemented in Rust should be available consistently in Python, Node, and Go, with idiomatic APIs per language.

## Done

- [x] Node TypeScript declaration correctness fixed
  - Scope: fixed missing `Schema` type reference and invalid optional/required parameter ordering.
  - Why: broken `.d.ts` blocks adoption immediately for TS users.
  - Evidence: `crates/simple-agents-napi/index.d.ts` now type-checks with `tsc --noEmit index.d.ts`.

- [x] Go make/build/test baseline stabilized
  - Scope: fixed `make release-go` and `make test-go-bindings` invocation, env propagation, and cache handling.
  - Why: parity work is impossible if basic binding checks are flaky.
  - Evidence: `make release-go` and `make test-go-bindings` pass.

- [x] Go message-based completion + structured/healing output baseline added
  - Scope: added `sa_complete_messages_json` FFI endpoint and Go `CompleteMessages(ctx, ...)` API.
  - Why: Go needed to move beyond prompt-only API to align with Python-style message-first workflows.
  - Sample shape:
    ```go
    res, err := client.CompleteMessages(ctx, model, []simpleagents.Message{
      {Role: "user", Content: "Return JSON: {\"status\":\"ok\"}"},
    }, simpleagents.CompleteOptions{Mode: "healed_json"})
    ```

- [x] Binding CI workflow introduced
  - Scope: added `.github/workflows/bindings-ci.yml` for Go and Node checks.
  - Why: prevents silent parity regression.

- [x] Shared env template introduced
  - Scope: added `.env.example` with `PROVIDER`, `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, `CUSTOM_API_BASE`.
  - Why: one contract across bindings reduces setup drift.

## Partially Done

- [~] API parity with Python
  - Current state:
    - Python: broad surface (`complete` variants, streaming, structured streaming, tools, healing).
    - Node: `complete` + `stream`, but streaming remains standard-mode only.
    - Go: `Complete`, `CompleteWithContext`, `CompleteMessages`; no streaming yet.
  - Why incomplete: missing streaming and some advanced parity features.
  - Next sample target:
    ```go
    chunks, errs := client.StreamMessages(ctx, model, messages, opts)
    for c := range chunks { /* consume partials */ }
    if err := <-errs; err != nil { /* handle */ }
    ```

- [~] Go validation coverage
  - Current state: unit tests and env-gated live test exist.
  - Why incomplete: no streaming tests yet, no schema-edge golden cases.
  - Next sample target:
    ```go
    func TestStreamMessagesCancellation(t *testing.T) {
      ctx, cancel := context.WithCancel(context.Background())
      chunks, errs := client.StreamMessages(ctx, model, messages, opts)
      cancel()
      _ = chunks
      if err := <-errs; err == nil { t.Fatal("expected cancel error") }
    }
    ```

- [~] Credentials/fixtures parity
  - Current state: `.env.example` and updated binding docs exist.
  - Why incomplete: deterministic mock fixtures still missing for no-network contract testing.

## Pending

- [ ] Implement streaming in C FFI and Go bindings
  - Scope:
    - Add C callback-based streaming API.
    - Bridge to Go channel-based API with cancellation support.
  - Why: streaming is core product behavior and parity blocker.
  - Sample target C API:
    ```c
    typedef void (*sa_stream_cb)(const char *chunk_json, void *user_data);
    int sa_stream_messages(
      SAClient *client,
      const char *model,
      const SAMessage *messages,
      size_t messages_len,
      int32_t max_tokens,
      float temperature,
      float top_p,
      sa_stream_cb cb,
      void *user_data
    );
    ```
  - Sample target Go API:
    ```go
    func (c *Client) StreamMessages(
      ctx context.Context,
      model string,
      messages []Message,
      opts CompleteOptions,
    ) (<-chan StreamChunk, <-chan error)
    ```

- [ ] Cross-language capability matrix and CI gating
  - Scope: define capability table and assert minimum required features in CI.
  - Why: prevents future divergence between Python/Node/Go.
  - Sample matrix row:
    ```text
    capability           rust  python  node  go
    message_complete     yes   yes     yes   yes
    stream_standard      yes   yes     yes   no  <- blocker
    stream_structured    yes   yes     no    no
    ```

- [ ] Contract fixtures for parity tests
  - Scope: shared fixtures for request/response/healing/tool-call behaviors consumed by all bindings.
  - Why: same input should yield same semantic output across languages.
  - Sample fixture idea:
    ```json
    {
      "name": "healed_json_basic",
      "input": {"messages": [{"role": "user", "content": "Return malformed JSON"}]},
      "expect": {"was_healed": true}
    }
    ```

- [ ] Node parity improvements
  - Scope: typed tool-call returns and richer streaming/partial type surface.
  - Why: TS ergonomics and safety should match Python-level confidence.

## Refactor Tasks

- [ ] Refactor Go API toward explicit OOD shape
  - Current concern: mixed legacy prompt method and newer message method can drift.
  - Target design:
    - `CompletePrompt(ctx, ...)` (thin wrapper)
    - `CompleteMessages(ctx, ...)` (primary)
    - `StreamMessages(ctx, ...)` (primary streaming)
  - Why: explicit method-per-use-case is idiomatic Go and easier to test.
  - Sample:
    ```go
    func (c *Client) CompletePrompt(ctx context.Context, model, prompt string, opts CompleteOptions) (CompletionResult, error)
    func (c *Client) CompleteMessages(ctx context.Context, model string, messages []Message, opts CompleteOptions) (CompletionResult, error)
    ```

- [ ] Refactor FFI payload model to shared typed schema structs
  - Current concern: ad-hoc JSON serialization in FFI can drift from N-API/Python mappings.
  - Target: centralize response DTO mapping helpers in Rust and reuse across bindings.
  - Why: DRY and consistency.

- [ ] Refactor test layering
  - Current concern: live tests are present but mock/contract coverage is still thin.
  - Target layering:
    - unit (no network)
    - contract (shared fixtures)
    - live (env-gated)
  - Why: reliable CI + meaningful parity signal.

## Execution Order (Recommended)

- [ ] 1. Implement `sa_stream_messages` in Rust FFI.
- [ ] 2. Implement `StreamMessages` in Go with context cancellation and no goroutine leaks.
- [ ] 3. Add Go streaming unit + live tests.
- [ ] 4. Add shared parity fixtures and contract runner.
- [ ] 5. Enforce capability matrix in CI.
- [ ] 6. Upgrade Node streaming/tool-call type parity.
