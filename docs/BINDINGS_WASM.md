# Browser/WASM Binding (Preview)

This document defines the current browser-compatible binding surface for
`simple-agents-wasm` and how it maps to `simple-agents-node`.

## Current status

- Package path: `bindings/wasm/simple-agents-wasm`
- Maturity: preview (WS1/WS2 rollout stage)
- Runtime target: modern browsers and fetch-capable JS runtimes
- Runtime engine: Rust `wasm-bindgen` core (`bindings/wasm/simple-agents-wasm/rust`) with JS fallback for non-wasm environments

## Contract goals

- Keep method naming and result shapes aligned with `simple-agents-node`.
- Preserve message and completion type semantics where possible.
- Keep browser-incompatible APIs explicit (fail fast with actionable errors).

## Supported APIs

- `new Client(provider, config)` where `provider` is `openai` or `openrouter`
- `complete(model, promptOrMessages, options?)`
- `stream(model, promptOrMessages, onChunk, options?)`
- `streamEvents(model, promptOrMessages, onEvent, options?)`
- `runWorkflowYamlString(yamlText, workflowInput, workflowOptions?)`
- `run(request)` / `runAsync(request)` / `streamWorkflow(request, onEvent?)` (typed, messages-first workflow facade)
- `createWorkflowStreamPrinter({ splitThinking? })` from `simple-agents-wasm/workflow_stream_printer` for a default streaming `onEvent` implementation (Node uses `process.stdout.write` when available; browsers fall back to `console.log`)

## Deliberate differences vs Node binding

- Browser config is explicit object-based; env-driven provider config is not used.
- `healed_json` and `schema` modes are not yet implemented.
- Path-based workflow methods are not supported in browser runtime.
- Usage metadata in streams may be unavailable (`usageAvailable: false`) depending on provider stream payloads.

## Browser-safe workflow direction

WASM/browser flow support is string/object based:

- Implemented: `runWorkflowYamlString(yamlText, workflowInput, workflowOptions?)`
  - Supports step DSL (`steps`) and graph YAML (`entry_node` + `nodes` + `edges`) for `llm_call`, `switch`, and `custom_worker` node types.
  - `custom_worker` requires exact handler lookup in `workflowOptions.functions`:
    - without `handler_file`: `workflowOptions.functions[handler]`
    - with `handler_file`: `workflowOptions.functions["<handler_file>#<handler>"]`
  - Each handler is called as **`(args, graphContext)`** (sync or async/Promise). The first argument includes resolved **`payload`**, **`handler`**, **`nodeId`**, and related metadata; the second is the live graph context (input, nodes, globals). Payload interpolation and `nodes.*.output` template rules match [YAML_WORKFLOW_SYSTEM.md](YAML_WORKFLOW_SYSTEM.md). This differs from Python’s kwargs-only `context` / `payload` call shape, but the **data** is the same.
- Not supported in browser: `runWorkflowYaml(workflowPath, ...)`

Workflow result contract is strict: `runWorkflowYamlString` expects outputs containing `workflow_id` and `outputs`, and throws if the backend response shape is incompatible.

Unified workflow request shape for browser runtime:

- `workflow_yaml: string` (inline YAML text; browser runtime does not support local filesystem path loading)
- `messages: MessageInput[]` (required)
- optional `context`, `media`, `input`, `execution`, `workflow_options`

`workflow_options.telemetry` and `workflow_options.trace` use explicit TypeScript interfaces in `index.d.ts` (`WorkflowTelemetryConfig`, `WorkflowTraceConfig`, etc., snake_case field names to match the Rust runner). `input` may include optional workflow-specific fields (some older examples used a scalar like `email_text`); prefer `messages` as the primary chat payload and keep extra keys only when templates reference `input.*`.

Note: completion APIs already use `stream(...)`, so workflow streaming is exposed as `streamWorkflow(...)` to avoid method-name collision on the shared `Client` surface.

## Security and deployment notes

- BYOK credentials are provided at runtime and must not be persisted by default.
- Browser mode still depends on provider CORS behavior.
- Use server fallback only when CORS/provider constraints require it.

## Local development workflow

- Install package deps: `cd bindings/wasm/simple-agents-wasm && npm install`
- Build wasm + JS package: `make build-wasm`
- Run wasm binding tests: `make test-wasm`
- Run repository contract/parity checks: `make test-binding-contracts` and `make test-binding-layers`

## Rust + JavaScript quality gates

- Rust formatting: `cargo fmt --manifest-path bindings/wasm/simple-agents-wasm/rust/Cargo.toml -- --check`
- Rust linting: `cargo clippy --manifest-path bindings/wasm/simple-agents-wasm/rust/Cargo.toml --all-targets`
- JS tests (Node runtime fallback + parity): `cd bindings/wasm/simple-agents-wasm && npm test`

## Type-checking and editor support

- Public TS surface is maintained in `bindings/wasm/simple-agents-wasm/index.d.ts`.
- Keep `index.d.ts` aligned with runtime behavior for both Rust-backed and JS fallback execution.
- Use TypeScript-aware editors (`tsserver`) to validate consumer-facing types and callback signatures when updating APIs.
