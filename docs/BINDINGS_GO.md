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
  client, err := simpleagents.NewClientWithProvider(simpleagents.ProviderConfig{
    Provider: "openai",
    APIKey:   "sk-...",
  })
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

- `NewClientWithProvider` accepts explicit credentials (`ProviderConfig`).
- `NewClientFromEnv` remains available for environment-based fallback.
- `CompleteMessages` returns structured data including tool calls, usage, and optional healing/coercion metadata.
- Always call `Close()` to release the underlying FFI client.

## Workflow YAML Runner (Rust-backed)

Go binding now exposes the Rust YAML workflow runner through FFI:

```go
out, err := client.RunWorkflowYAML(
    context.Background(),
    "examples/workflow_email/email-intake-classification.yaml",
    map[string]any{"email_text": "Termination request, second warning already issued"},
)
if err != nil {
    panic(err)
}

fmt.Println(out.TerminalOutput)
fmt.Println(out.StepTimings)   // per-node elapsed ms
fmt.Println(out.TotalElapsedMS)
```

This method delegates to Rust `simple-agents-workflow` as the source of truth.

New typed workflow facade:

- `Run(ctx, request, flags)` - sync run
- `RunAsync(ctx, request, flags)` - async/future-style run
- `Stream(ctx, request, flags, onEvent)` - live workflow events + final output

`request` is messages-first and typed:

- `WorkflowPath string`
- `Input *TypedWorkflowInput` (must include `messages`)

### Custom workers (`custom_worker`)

Go binding now fails fast before FFI execution when the workflow contains `custom_worker`, with an actionable error (use Python/WASM executor path or remove `custom_worker` from this runtime).

The Rust contract for handlers remains: interpolated **`config.payload`** and **`context`** (`input`, `nodes`, `globals`, optional `trace`). See [YAML_WORKFLOW_SYSTEM.md](YAML_WORKFLOW_SYSTEM.md).

**Practical options:** Python package with `handlers.py`, WASM `workflowOptions.functions`, workflows without `custom_worker`, or app-side orchestration.

Workflow event parity with Python is available via:

- `RunWorkflowYAMLWithEvents(ctx, workflowPath, workflowInput, options)` for collected events in output.
- `RunWorkflowYAMLStream(ctx, workflowPath, workflowInput, onEvent)` for live callbacks plus final output.

Workflow telemetry options follow Rust runner semantics:

- `Telemetry.SampleRate` must be between `0.0` and `1.0`.
- Sampling is deterministic per trace id.
- Final output metadata includes `metadata.telemetry.sampled`.

Tracing exporter env configuration is shared across runtimes:

- `SIMPLE_AGENTS_TRACING_ENABLED`
- `OTEL_EXPORTER_OTLP_ENDPOINT`
- `OTEL_EXPORTER_OTLP_PROTOCOL` (`grpc` or `http/protobuf`)
- `OTEL_EXPORTER_OTLP_HEADERS`
- `OTEL_SERVICE_NAME`
