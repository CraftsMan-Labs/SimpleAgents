# Go Binding

Go bindings live in `bindings/go` and use the C FFI from `simple-agents-ffi`.

## Build Prerequisites

1. Build the Rust FFI library:

```bash
cargo build -p simple-agents-ffi --release
```

2. Ensure the linker can find `libsimple_agents_ffi` (example for Linux):

```bash
export LD_LIBRARY_PATH="$PWD/target/release:$LD_LIBRARY_PATH"
```

## Quick Start

```go
package main

import (
  "context"
  "fmt"

  "simpleagents"
)

func main() {
  client, err := simpleagents.NewClientFromEnv("openai")
  if err != nil {
    panic(err)
  }
  defer client.Close()

  result, err := client.CompleteMessages(
    context.Background(),
    "gpt-4",
    []simpleagents.Message{{Role: "user", Content: "Hello from Go"}},
    simpleagents.CompleteOptions{Mode: "standard"},
  )
  if err != nil {
    panic(err)
  }

  fmt.Println(result.Content)
}
```

## Prompt-Based API

```go
text, err := client.Complete("gpt-4", "Hello", 128, 0.7)
```

## Modes and Schema Coercion

`CompleteOptions.Mode` supports `standard`, `healed_json`, and `schema`. When using `schema`, pass `CompleteOptions.Schema` as a JSON-serializable value.

## Notes

- `NewClientFromEnv` reads provider keys from environment variables (e.g., `OPENAI_API_KEY`).
- `CompleteMessages` returns structured data including tool calls, usage, and optional healing/coercion metadata.
- Always call `Close()` to release the underlying FFI client.
