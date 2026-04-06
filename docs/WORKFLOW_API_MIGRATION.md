# Workflow API Migration (Breaking)

This release unifies workflow execution around typed, messages-first request envelopes and explicit execution flags.

## Canonical request model

All bindings now align to the same conceptual fields:

- `workflow_path` (or `workflow_yaml` for browser/WASM)
- `messages` (required)
- optional `context`, `media`, `input`
- optional `execution` flags:
  - `healing`
  - `workflow_streaming`
  - `node_llm_streaming`
  - optional `model`
- optional `workflow_options` (`telemetry`, `trace`, `model`)

## Method mapping

- Sync run: `run(...)` / `Run(...)`
- Async run: `run_async(...)` / `runAsync(...)` / `RunAsync(...)`
- Streaming run: `stream(...)` / `streamWorkflow(...)` / `Stream(...)`

Some language surfaces keep legacy names for compatibility (`runWorkflowYaml*`), but the new typed request-based entrypoints are the source of truth.

## Provider configuration

- Python: explicit provider parameters already supported by `Client(...)`.
- Node: use `Client.withProviderConfig({ provider, apiKey, apiBase? })` for explicit credentials.
- Go: use `NewClientWithProvider(ProviderConfig{Provider, APIKey, APIBase?})`.
- Legacy env-based constructors remain available for compatibility.

## Custom worker behavior

- YAML remains source of truth for `custom_worker.handler`.
- Python executes local file handlers (`context` + `payload` kwargs ABI).
- Node and Go now fail fast with actionable validation errors when `custom_worker` nodes are present without runtime executor support.
- WASM uses `workflowOptions.functions`.

## Streaming semantics

- `workflow_streaming=true` is only valid for stream entrypoints.
- At the **request `execution` layer**, `healing=true` and `node_llm_streaming=true` cannot both be true; validation rejects that combination (structured healing and node-level LLM streaming are mutually exclusive there). Per-node YAML `heal` / `stream` still apply independently.

## Migration checklist

1. Move workflow inputs to a messages-first request object.
2. Move execution toggles to `execution.{healing,workflow_streaming,node_llm_streaming}`.
3. Prefer explicit provider credentials over implicit env wiring.
4. Switch to typed run/run_async/stream entrypoints in each binding.
5. Re-test workflows with `custom_worker` in the target runtime.
