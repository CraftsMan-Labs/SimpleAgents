# Go bindings

Go bindings for SimpleAgents using cgo and the C FFI.

## Build

```sh
cargo build -p simple-agents-ffi --release
```

Ensure the dynamic library is on your loader path (for example `LD_LIBRARY_PATH=target/release`).

## Environment contract

Use the same canonical env contract as other bindings:

- `PROVIDER` - `openai`, `anthropic`, or `openrouter`
- `CUSTOM_API_KEY`
- `CUSTOM_API_BASE` (optional for providers with a base URL)
- `CUSTOM_API_MODEL`

The Go example maps `CUSTOM_API_*` to provider-specific variables expected by the Rust providers.

## Usage

```go
package main

import (
    "context"
    "fmt"
    "log"
    "time"

    "simpleagents"
)

func main() {
    client, err := simpleagents.NewClientFromEnv("openai")
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancel()

    maxTokens := int32(64)
    temperature := float32(0.2)

    result, err := client.CompleteMessages(
        ctx,
        "gpt-4",
        []simpleagents.Message{{Role: "user", Content: "Say hello in one sentence."}},
        simpleagents.CompleteOptions{MaxTokens: &maxTokens, Temperature: &temperature},
    )
    if err != nil {
        log.Fatal(err)
    }

    fmt.Println(result.Content)
}
```

See runnable example: `bindings/go/examples/client/main.go`.

## API summary

- `NewClientFromEnv(provider string) (*Client, error)`
- `(*Client).CompletePrompt(ctx, model, prompt, maxTokens, temperature)` (canonical prompt API)
- `(*Client).CompleteWithContext(ctx, model, prompt, maxTokens, temperature)` (compatibility alias)
- `(*Client).CompleteMessages(ctx, model, messages, opts)` (message API, structured/healing outputs)
- `(*Client).StreamMessages(ctx, model, messages, opts)` (streaming channel API)
- `(*Client).Complete(...)` (backward-compatible prompt helper)
- `(*Client).Close()`

## Option validation

`CompleteOptions.Mode` supports `standard`, `healed_json`, and `schema`.

- `schema` mode requires `CompleteOptions.Schema`.
- `Schema` is rejected for non-`schema` modes.
- Streaming currently supports only `standard` mode.

## Test layers

- Unit: `go test ./... -run 'TestValidate|TestCompleteMessagesUninitializedClient|TestCompleteWithContextUninitializedClient|TestStreamMessagesUninitializedClient'`
- Contract: `go test ./... -run 'TestGoBindingsFollowSharedContractFixture|TestValidateCompleteOptionsGoldenCases'`
- Live: `go test ./... -run 'TestLive'` (env-gated)
