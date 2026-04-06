# Go bindings

Go bindings for SimpleAgents using cgo and the C FFI.

## Build

```sh
cargo build -p simple-agents-ffi --release
```

Ensure the dynamic library is on your loader path (for example `LD_LIBRARY_PATH=target/release`).

## Environment contract

Examples that need credentials use the same env names as other bindings (see each example’s header). Typical workflow examples use:

- `WORKFLOW_API_KEY` or `CUSTOM_API_KEY`
- `WORKFLOW_API_BASE` or `CUSTOM_API_BASE` (optional)
- `WORKFLOW_PROVIDER` (optional; used when mapping keys for documentation; `NewClient` takes key and base URL explicitly)

The `examples/client` example uses `PROVIDER`, `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, and optional `CUSTOM_API_BASE`.

## Usage

```go
package main

import (
    "context"
    "encoding/json"
    "fmt"
    "log"
    "os"
    "time"

    "simpleagents"
)

func main() {
    client, err := simpleagents.NewClient(os.Getenv("OPENAI_API_KEY"), "")
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    req, _ := json.Marshal(map[string]any{
        "model": "gpt-4",
        "messages": []map[string]string{
            {"role": "user", "content": "Say hello in one sentence."},
        },
        "max_tokens":  64,
        "temperature": 0.2,
    })

    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancel()

    resJSON, err := client.Complete(ctx, req)
    if err != nil {
        log.Fatal(err)
    }
    fmt.Println(string(resJSON))
}
```

See runnable example: `bindings/go/examples/client/main.go`.

## API summary

- `NewClient(apiKey, baseURL string) (*Client, error)` — empty `baseURL` uses the default OpenAI-compatible endpoint.
- `(*Client).Complete(ctx, requestJSON []byte) ([]byte, error)` — JSON `CompletionRequest` in, `CompletionResponse` JSON out.
- `(*Client).StreamComplete(ctx, requestJSON, onChunk)` — request must include `"stream": true`.
- `(*Client).Run(ctx, workflowPath, inputJSON []byte) ([]byte, error)` — workflow output JSON (`YamlWorkflowRunOutput`).
- `(*Client).Stream(ctx, workflowPath, inputJSON, onEvent)` — one JSON event string per callback.
- `(*Client).Resume(ctx, checkpointJSON []byte) ([]byte, error)`
- `ParseWorkflowOutput(raw []byte)` — convenience `map[string]interface{}` parse.
- `(*Client).Close()`

Runtime `custom_worker` executors are not available from Go; workflows that require them cannot complete through this FFI alone.

## Test layers

- Recommended (repo-root, reproducible env/linker setup): `make test-go-bindings`
- Build-only check (repo-root): `make release-go`
- Direct local invocation from `bindings/go/` (equivalent env wiring):

```sh
CGO_CFLAGS="-I$(pwd)/../../crates/simple-agents-ffi/include" \
CGO_LDFLAGS="-L$(pwd)/../../target/release" \
GOCACHE="$(pwd)/.gocache" \
LD_LIBRARY_PATH="$(pwd)/../../target/release:${LD_LIBRARY_PATH}" \
go test ./...
```

- Unit: `go test ./... -short`
- Contract: `go test ./... -run 'TestGoBindingsFollowSharedContractFixture'`
- Live: `go test ./... -run 'TestLive'` (requires `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, and optional `CUSTOM_API_BASE`)
